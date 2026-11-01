"""
Monitoring API Routes - Carrier-class monitoring endpoints
"""
from fastapi import APIRouter, Depends, HTTPException, Query
from sqlalchemy.orm import Session
from sqlalchemy import desc, func, and_, text
from typing import List, Optional, Dict, Any
from datetime import datetime, timedelta, timezone

from app.db.session import SessionLocal
from app.monitoring.rates import mariadb_rates
from app.models.monitoring import (
    KamailioMetrics, RtpengineMetrics, MariaDBMetrics,
    SystemMetrics, MonitoringAlert, MonitoringConfig,
    MonitoringKpiHourly, MonitoringKpiDaily,
)
from app.schemas.monitoring import (
    KamailioMetricsResponse, KamailioSummary,
    RtpengineMetricsResponse, RtpengineSummary,
    MariaDBMetricsResponse, MariaDBSummary,
    SystemMetricsResponse, SystemSummary,
    MonitoringAlertResponse, AlertRule,
    DashboardOverview, MetricsTimeSeries, TrunkStatus,
    ComponentHealth, HealthCheckResponse
)

router = APIRouter(tags=["monitoring"])


def get_db():
    db = SessionLocal()
    try:
        yield db
    finally:
        db.close()


def get_time_range(
    hours: int = Query(1, ge=1, le=168),
    minutes: int = Query(0, ge=0, le=59)
) -> tuple:
    """Calculate time range from query params"""
    end = datetime.utcnow()
    start = end - timedelta(hours=hours, minutes=minutes)
    return start, end


# ==================== CARRIER-CLASS HISTORY TIERS ====================
# Raw 1-min rows serve short ranges; hourly rollups serve days/weeks;
# daily rollups serve months. `resolution=auto` picks the cheapest tier that
# still has enough granularity. This keeps dashboard charts fast even with
# months of history (Vodafone/AT&T style: pre-aggregated tiers, not raw scans).

ROLLUP_SOURCE_MODELS = {
    "kamailio": KamailioMetrics,
    "rtpengine": RtpengineMetrics,
    "mariadb": MariaDBMetrics,
    "system": SystemMetrics,
}

MAX_TIMESERIES_HOURS = 9600  # 400 days (daily tier retention)


def _resolve_tier(hours: int, resolution: str) -> str:
    if resolution in ("raw", "hourly", "daily"):
        return resolution
    # auto
    if hours <= 12:
        return "raw"
    if hours <= 24 * 93:
        return "hourly"
    return "daily"


# KPI aliases: frontend KPI names -> real DB columns.
METRIC_ALIASES = {
    ("rtpengine", "avg_mos"): "mos_avg",
    ("rtpengine", "avg_jitter_ms"): "jitter_avg_ms",
    ("rtpengine", "avg_rtt_ms"): "rtt_avg_ms",
    ("mariadb", "threads_connected"): "threads_connected",
    ("mariadb", "connection_usage_percent"): "threads_connected",  # derived, see below
    ("system", "memory_percent"): "memory_percent",
}

# Derived KPIs computed from base column series (rate/ratio of counters).
# Each handler receives {col: [(ts, val), ...]} and returns [(ts, value)].
def _derive_rate(series):
    """Per-point rate from a cumulative counter: delta(value)/delta(t seconds)."""
    out = []
    prev = None
    for ts, v in series:
        if prev is None or v is None or prev[1] is None:
            out.append((ts, 0.0))
        else:
            dt = (ts - prev[0]).total_seconds()
            dv = (v or 0) - (prev[1] or 0)
            out.append((ts, round(max(0.0, dv / dt) if dt > 0 else 0.0, 2)))
        prev = (ts, v)
    return out


def _derive_ratio_pct(num, den):
    """Per-point 100*a/(a+b+...) from cumulative counters."""
    out = []
    for (ts, _), vals in _align_series(num, den):
        total = sum(vals)
        out.append((ts, round(100.0 * vals[0] / total, 2) if total > 0 else 100.0))
    return out


def _align_series(*series_list):
    """Align multiple [(ts, val)] series on timestamps (inner join)."""
    if not series_list:
        return
    base = series_list[0]
    maps = [{ts: v for ts, v in s} for s in series_list[1:]]
    for ts, v in base:
        vals = [v]
        ok = True
        for m in maps:
            if ts not in m or m[ts] is None:
                ok = False
                break
            vals.append(m[ts])
        if ok:
            yield (ts, v), vals


def _derive_shm_pct(used, free):
    out = []
    for (ts, _), vals in _align_series(used, free):
        u, f = vals
        tot = (u or 0) + (f or 0)
        out.append((ts, round(100.0 * (u or 0) / tot, 2) if tot > 0 else 0.0))
    return out


def _derive_sum(*series_list):
    out = []
    for (ts, _), vals in _align_series(*series_list):
        out.append((ts, round(sum(v or 0 for v in vals), 2)))
    return out


DERIVED_METRICS = {
    # KPI name -> (base columns, combiner)
    ("kamailio", "calls_per_second"): (["invite_received"], lambda b: _derive_rate(b["invite_received"])),
    ("kamailio", "success_rate"): (
        ["invite_200ok", "invite_4xx", "invite_5xx"],
        lambda b: _success_rate(b),
    ),
    ("kamailio", "shm_usage_percent"): (["shm_used_mb", "shm_free_mb"], lambda b: _derive_shm_pct(b["shm_used_mb"], b["shm_free_mb"])),
    ("kamailio", "dispatcher_health"): (["dispatcher_active_destinations"], lambda b: [(ts, v) for ts, v in b["dispatcher_active_destinations"]]),
    ("kamailio", "trunk_health"): (["trunks_online"], lambda b: [(ts, v) for ts, v in b["trunks_online"]]),
    ("mariadb", "connection_usage_percent"): (["threads_connected"], None),  # needs live max; handled below
    ("rtpengine", "packet_loss_percent"): (
        ["packets_in", "packets_out", "packets_lost_in", "packets_lost_out"],
        lambda b: _loss_pct(b),
    ),
    ("rtpengine", "media_throughput_mbps"): (
        ["bytes_in", "bytes_out"],
        lambda b: _throughput_mbps(b),
    ),
    ("system", "net_throughput_mbps"): (
        ["net_rx_mbps", "net_tx_mbps"],
        lambda b: _derive_sum(b["net_rx_mbps"], b["net_tx_mbps"]),
    ),
}


def _success_rate(b):
    out = []
    for (ts, _), vals in _align_series(b["invite_200ok"], b["invite_4xx"], b["invite_5xx"]):
        ok, e4, e5 = vals
        tot = (ok or 0) + (e4 or 0) + (e5 or 0)
        out.append((ts, round(100.0 * (ok or 0) / tot, 2) if tot > 0 else 100.0))
    return out


def _loss_pct(b):
    out = []
    series = [b["packets_in"], b["packets_out"], b["packets_lost_in"], b["packets_lost_out"]]
    for (ts, _), vals in _align_series(*series):
        pi, po, li, lo = vals
        tot = (pi or 0) + (po or 0)
        lost = (li or 0) + (lo or 0)
        out.append((ts, round(100.0 * lost / tot, 2) if tot > 0 else 0.0))
    return out


def _throughput_mbps(b):
    tot = _derive_sum(b["bytes_in"], b["bytes_out"])  # cumulative bytes
    out = []
    prev = None
    for ts, v in tot:
        if prev is None:
            out.append((ts, 0.0))
        else:
            dt = (ts - prev[0]).total_seconds()
            dv = max(0.0, (v or 0) - (prev[1] or 0))
            out.append((ts, round(dv * 8 / 1e6 / dt, 2) if dt > 0 else 0.0))
        prev = (ts, v)
    return out


def _fetch_base_series(db, component, col, start, tier):
    """Fetch a base column series at the requested tier (rollup avg or raw)."""
    model = ROLLUP_SOURCE_MODELS[component]
    if tier == "raw":
        rows = db.query(model.timestamp, getattr(model, col)).filter(
            model.timestamp >= start).order_by(model.timestamp).all()
        return [(r[0], r[1]) for r in rows]
    rollup_model = MonitoringKpiHourly if tier == "hourly" else MonitoringKpiDaily
    rows = db.query(rollup_model.bucket_start, rollup_model.avg_value).filter(
        rollup_model.component == component, rollup_model.metric == col,
        rollup_model.bucket_start >= start).order_by(rollup_model.bucket_start).all()
    if not rows:
        rows = db.query(model.timestamp, getattr(model, col)).filter(
            model.timestamp >= start).order_by(model.timestamp).all()
        return [(r[0], r[1]) for r in rows]
    return [(r[0], r[1]) for r in rows]


def query_timeseries(
    db: Session, component: str, metric: str, hours: int,
    resolution: str = "auto", max_points: int = 500, agg: str = "avg",
) -> "MetricsTimeSeries":
    """Shared history query across all components with tiered rollups.

    Supports real DB columns plus frontend KPI aliases (avg_mos -> mos_avg)
    and derived KPIs (calls_per_second, success_rate, *_usage_percent, ...).
    """
    model = ROLLUP_SOURCE_MODELS.get(component)
    if model is None:
        raise HTTPException(400, f"Invalid component. Valid: {list(ROLLUP_SOURCE_MODELS.keys())}")

    # 1) Alias KPI name -> real column
    metric = METRIC_ALIASES.get((component, metric), metric)

    valid_metrics = [c.key for c in model.__table__.columns if c.key not in ['id', 'timestamp', 'raw_data']]
    derived = DERIVED_METRICS.get((component, metric))

    tier = _resolve_tier(hours, resolution)
    start = datetime.utcnow() - timedelta(hours=hours)

    if derived is not None:
        base_cols, combine = derived
        if metric == "connection_usage_percent":
            # Needs live max_connections (stored per-row in max_used_connections
            # since the connection-limit fix).
            max_row = db.query(MariaDBMetrics.max_used_connections).order_by(
                MariaDBMetrics.timestamp.desc()).first()
            max_conn = (max_row[0] if max_row and max_row[0] else 151) or 151
            base = _fetch_base_series(db, component, "threads_connected", start, tier)
            data = [{"timestamp": ts.isoformat(),
                     "value": round(100.0 * (v or 0) / max_conn, 2)} for ts, v in base]
        elif metric == "dispatcher_health":
            # Not chartable as health string: expose active-destination count.
            base = _fetch_base_series(db, component, "dispatcher_active_destinations", start, tier)
            data = [{"timestamp": ts.isoformat(), "value": v} for ts, v in base]
        elif metric == "trunk_health":
            # Chart the number of trunks answering OPTIONS.
            base = _fetch_base_series(db, component, "trunks_online", start, tier)
            data = [{"timestamp": ts.isoformat(), "value": v} for ts, v in base]
        else:
            for col in base_cols:
                if col not in valid_metrics:
                    raise HTTPException(400, f"Invalid metric. Valid: {valid_metrics}")
            series = {c: _fetch_base_series(db, component, c, start, tier) for c in base_cols}
            data = [{"timestamp": ts.isoformat(), "value": v} for ts, v in combine(series)]
    elif metric not in valid_metrics:
        raise HTTPException(400, f"Invalid metric. Valid: {valid_metrics}")
    elif tier == "raw":
        rows = db.query(
            model.timestamp, getattr(model, metric)
        ).filter(model.timestamp >= start).order_by(model.timestamp).all()
        data = [{"timestamp": r[0].isoformat(), "value": r[1]} for r in rows]
    else:
        rollup_model = MonitoringKpiHourly if tier == "hourly" else MonitoringKpiDaily
        # agg=max charts each bucket's peak (e.g. concurrent calls over 24h,
        # where an hourly average flattens the busy-hour peaks). Derived
        # metrics above always use the average: a max of a counter is not a rate.
        value_col = rollup_model.max_value if agg == "max" else rollup_model.avg_value
        rows = db.query(
            rollup_model.bucket_start, value_col
        ).filter(
            rollup_model.component == component,
            rollup_model.metric == metric,
            rollup_model.bucket_start >= start,
        ).order_by(rollup_model.bucket_start).all()
        # Fall back to raw when the requested window predates rollup backfill
        # (e.g. first hours after deploying history tiers).
        if not rows:
            rows = db.query(
                model.timestamp, getattr(model, metric)
            ).filter(model.timestamp >= start).order_by(model.timestamp).all()
        data = [{"timestamp": r[0].isoformat(), "value": r[1]} for r in rows]

    # Downsample to max_points (stride + always keep last point)
    if max_points and len(data) > max_points:
        import math
        stride = math.ceil(len(data) / max_points)
        data = data[::stride]
    return MetricsTimeSeries(
        metric=metric, component=component, unit=_get_unit(metric), data=data,
    )


# ==================== DASHBOARD OVERVIEW ====================

@router.get("/overview", response_model=DashboardOverview)
def get_dashboard_overview(db: Session = Depends(get_db)):
    """Get complete dashboard overview - carrier-class single endpoint"""
    now = datetime.utcnow()
    minute_ago = now - timedelta(minutes=1)
    five_min_ago = now - timedelta(minutes=5)
    
    # Latest metrics
    kamailio_latest = db.query(KamailioMetrics).filter(
        KamailioMetrics.timestamp >= minute_ago
    ).order_by(desc(KamailioMetrics.timestamp)).first()
    
    rtpengine_latest = db.query(RtpengineMetrics).filter(
        RtpengineMetrics.timestamp >= minute_ago
    ).order_by(desc(RtpengineMetrics.timestamp)).first()
    
    mariadb_latest = db.query(MariaDBMetrics).filter(
        MariaDBMetrics.timestamp >= minute_ago
    ).order_by(desc(MariaDBMetrics.timestamp)).first()
    
    system_latest = db.query(SystemMetrics).filter(
        SystemMetrics.timestamp >= minute_ago
    ).order_by(desc(SystemMetrics.timestamp)).first()
    
    # 5-min aggregates for rates
    kamailio_5m = db.query(KamailioMetrics).filter(
        KamailioMetrics.timestamp >= five_min_ago
    ).all()
    
    # Alert counts
    active_alerts = db.query(MonitoringAlert).filter(
        MonitoringAlert.status == "firing"
    ).count()
    critical_alerts = db.query(MonitoringAlert).filter(
        and_(MonitoringAlert.status == "firing", MonitoringAlert.severity == "critical")
    ).count()
    warning_alerts = db.query(MonitoringAlert).filter(
        and_(MonitoringAlert.status == "firing", MonitoringAlert.severity == "warning")
    ).count()
    
    # Trunk health: last SIP OPTIONS result per enabled trunk (written by the collector)
    trunk_rows = db.execute(text(
        "SELECT name, COALESCE(trunk_type, 'public') AS trunk_type, host, COALESCE(port, 5060) AS port, "
        "COALESCE(sip_status, 'unknown') AS sip_status, sip_status_message AS message, "
        "last_options_response_code AS response_code, last_options_latency_ms AS latency_ms, "
        "last_options_check AS last_check "
        "FROM routing_trunks WHERE enabled = true ORDER BY name"
    )).mappings().all()
    trunks = [TrunkStatus(**r) for r in trunk_rows]
    # A result older than 5 min means the probe stopped: don't report it as live.
    def fresh(t):
        return bool(t.last_check) and \
            t.last_check.astimezone(timezone.utc).replace(tzinfo=None) >= now - timedelta(minutes=5)
    trunks_online = sum(1 for t in trunks if t.sip_status == "reachable" and fresh(t))
    trunks_total = len(trunks)
    if trunks_total == 0 or not any(fresh(t) for t in trunks):
        trunk_health = "unknown"
    elif trunks_online == trunks_total:
        trunk_health = "healthy"
    elif (trunks_total - trunks_online) / trunks_total > 0.5:
        trunk_health = "critical"
    else:
        trunk_health = "degraded"

    # Build summaries
    def calc_kamailio_summary():
        if not kamailio_latest:
            return KamailioSummary(
                active_calls=0, calls_per_second=0, success_rate=0,
                shm_usage_percent=0, active_transactions=0,
                dispatcher_health="unknown",
                dispatcher_active_destinations=0,
                dispatcher_inactive_destinations=0,
                trunk_health=trunk_health, trunks_online=trunks_online,
                trunks_total=trunks_total, trunks=trunks,
                last_update=now
            )
        
        k = kamailio_latest
        total_invites = k.invite_200ok + k.invite_4xx + k.invite_5xx
        success_rate = (k.invite_200ok / total_invites * 100) if total_invites > 0 else 100
        
        shm_total = k.shm_used_mb + k.shm_free_mb
        shm_pct = (k.shm_used_mb / shm_total * 100) if shm_total > 0 else 0
        
        # Dispatcher health
        total_dest = k.dispatcher_active_destinations + k.dispatcher_inactive_destinations
        if total_dest == 0:
            disp_health = "unknown"
        elif k.dispatcher_inactive_destinations == 0:
            disp_health = "healthy"
        elif k.dispatcher_inactive_destinations / total_dest > 0.5:
            disp_health = "critical"
        else:
            disp_health = "degraded"
        
        # Calls per second (from 5m window)
        if len(kamailio_5m) >= 2:
            first = min(kamailio_5m, key=lambda x: x.timestamp)
            last = max(kamailio_5m, key=lambda x: x.timestamp)
            delta_invites = last.invite_received - first.invite_received
            delta_time = (last.timestamp - first.timestamp).total_seconds()
            cps = delta_invites / delta_time if delta_time > 0 else 0
        else:
            cps = 0
        
        return KamailioSummary(
            active_calls=k.active_calls,
            calls_per_second=round(cps, 2),
            success_rate=round(success_rate, 2),
            shm_usage_percent=round(shm_pct, 2),
            active_transactions=k.active_transactions,
            dispatcher_health=disp_health,
            dispatcher_active_destinations=k.dispatcher_active_destinations or 0,
            dispatcher_inactive_destinations=k.dispatcher_inactive_destinations or 0,
            trunk_health=trunk_health, trunks_online=trunks_online,
            trunks_total=trunks_total, trunks=trunks,
            last_update=k.timestamp
        )
    
    def calc_rtpengine_summary():
        if not rtpengine_latest:
            return RtpengineSummary(
                active_sessions=0, port_usage_percent=0, packet_loss_percent=0,
                avg_mos=0, avg_jitter_ms=0, avg_rtt_ms=0,
                media_throughput_mbps=0, cpu_percent=0, memory_mb=0,
                status="unknown", last_update=now
            )
        
        r = rtpengine_latest
        total_pkts = r.packets_in + r.packets_out
        lost_pkts = r.packets_lost_in + r.packets_lost_out
        loss_pct = (lost_pkts / total_pkts * 100) if total_pkts > 0 else 0

        # NOTE: packets_in/out and bytes are CUMULATIVE relayed totals since
        # rtpengine boot (for forensics), not a per-minute rate. Throughput
        # rate comes from NG currentstatistics packetrate/byterate when
        # available; otherwise report 0 rather than a fake cumulative/60 value.
        raw = r.raw_data or {}
        ng_cur = raw.get("ng_currentstatistics", {}) if isinstance(raw, dict) else {}
        try:
            inst_byterate = float(ng_cur.get("byterate", 0) or 0)
        except Exception:
            inst_byterate = 0.0
        throughput = inst_byterate * 8 / 1e6  # bytes/s -> Mbps
        
        # Status determination - only evaluate MOS/loss when we have real media data.
        # mos_avg==0 / loss==0 with 0 sessions means "no data", not critical.
        has_media = (r.active_sessions or 0) > 0
        has_mos = (r.mos_avg or 0) > 0
        has_loss_data = total_pkts > 0
        if not has_media and not has_mos and not has_loss_data and (r.active_sessions or 0) == 0:
            # No RTP data available (collector only gets process metrics) -> unknown, not critical
            status = "unknown"
        elif r.cpu_percent > 80 or (has_loss_data and loss_pct > 5) or (has_mos and r.mos_avg < 3.5):
            status = "critical"
        elif r.cpu_percent > 60 or (has_loss_data and loss_pct > 1) or (has_mos and r.mos_avg < 4.0):
            status = "degraded"
        else:
            status = "healthy"
        
        return RtpengineSummary(
            active_sessions=r.active_sessions,
            port_usage_percent=round(r.port_usage_percent, 2),
            packet_loss_percent=round(loss_pct, 2),
            avg_mos=round(r.mos_avg, 2),
            avg_jitter_ms=round(r.jitter_avg_ms, 2),
            avg_rtt_ms=round(r.rtt_avg_ms, 2),
            media_throughput_mbps=round(throughput, 2),
            cpu_percent=round(r.cpu_percent, 2),
            memory_mb=round(r.memory_mb, 2),
            status=status,
            last_update=r.timestamp
        )
    
    def calc_mariadb_summary():
        if not mariadb_latest:
            return MariaDBSummary(
                connections_used=0, connections_max=0, connection_usage_percent=0,
                queries_per_second=0, slow_queries_per_minute=0,
                buffer_pool_usage_percent=0, replication_lag_seconds=None,
                deadlocks_per_minute=0, status="unknown", last_update=now
            )
        
        m = mariadb_latest
        slow_pm, deadlocks_pm = mariadb_rates(db, m)
        max_conn = m.max_used_connections or 151
        conn_pct = (m.threads_connected / max_conn * 100) if max_conn > 0 else 0
        
        bp_total = m.innodb_buffer_pool_used_mb + m.innodb_buffer_pool_free_mb
        bp_pct = (m.innodb_buffer_pool_used_mb / bp_total * 100) if bp_total > 0 else 0
        
        status = "healthy"
        if conn_pct > 90 or slow_pm > 10 or deadlocks_pm > 5:
            status = "critical"
        elif conn_pct > 70 or slow_pm > 5 or deadlocks_pm > 1:
            status = "degraded"
        
        return MariaDBSummary(
            connections_used=m.threads_connected,
            connections_max=max_conn,
            connection_usage_percent=round(conn_pct, 2),
            queries_per_second=round(m.queries_per_second or 0, 2),
            slow_queries_per_minute=slow_pm,
            buffer_pool_usage_percent=round(bp_pct, 2),
            replication_lag_seconds=m.seconds_behind_master,
            deadlocks_per_minute=deadlocks_pm,
            status=status,
            last_update=m.timestamp
        )
    
    def calc_system_summary():
        if not system_latest:
            return SystemSummary(
                cpu_percent=0, load_avg_1m=0, memory_percent=0,
                disk_percent=0, net_throughput_mbps=0,
                kamailio_status="unknown", rtpengine_status="unknown",
                freeswitch_status="unknown", postgres_status="unknown",
                last_update=now
            )
        
        s = system_latest
        net_tput = s.net_rx_mbps + s.net_tx_mbps
        
        # Component statuses based on process metrics
        def proc_status(cpu, mem_mb):
            if cpu > 80 or mem_mb > 2000:
                return "critical"
            elif cpu > 50 or mem_mb > 1000:
                return "degraded"
            return "healthy"
        
        return SystemSummary(
            cpu_percent=round(s.cpu_percent, 2),
            load_avg_1m=round(s.load_avg_1m, 2),
            memory_percent=round(s.memory_percent, 2),
            disk_percent=round(s.disk_percent, 2),
            net_throughput_mbps=round(net_tput, 2),
            kamailio_status=proc_status(s.kamailio_cpu_percent, s.kamailio_memory_mb),
            rtpengine_status=proc_status(s.rtpengine_cpu_percent, s.rtpengine_memory_mb),
            freeswitch_status=proc_status(s.freeswitch_cpu_percent, s.freeswitch_memory_mb),
            postgres_status=proc_status(s.postgres_cpu_percent, s.postgres_memory_mb),
            last_update=s.timestamp
        )
    
    return DashboardOverview(
        timestamp=now,
        kamailio=calc_kamailio_summary(),
        rtpengine=calc_rtpengine_summary(),
        mariadb=calc_mariadb_summary(),
        system=calc_system_summary(),
        active_alerts=active_alerts,
        critical_alerts=critical_alerts,
        warning_alerts=warning_alerts
    )


# ==================== KAMAILIO ENDPOINTS ====================

@router.get("/kamailio/latest", response_model=KamailioMetricsResponse)
def get_kamailio_latest(db: Session = Depends(get_db)):
    metric = db.query(KamailioMetrics).order_by(desc(KamailioMetrics.timestamp)).first()
    if not metric:
        raise HTTPException(404, "No Kamailio metrics found")
    return metric

@router.get("/kamailio/history", response_model=List[KamailioMetricsResponse])
def get_kamailio_history(
    start: datetime, end: datetime,
    limit: int = Query(60, le=1440),
    db: Session = Depends(get_db)
):
    metrics = db.query(KamailioMetrics).filter(
        and_(KamailioMetrics.timestamp >= start, KamailioMetrics.timestamp <= end)
    ).order_by(desc(KamailioMetrics.timestamp)).limit(limit).all()
    return metrics

@router.get("/kamailio/timeseries", response_model=MetricsTimeSeries)
def get_kamailio_timeseries(
    metric: str = Query(..., description="Metric name: active_calls, invite_received, shm_used_mb, etc."),
    hours: int = Query(1, ge=1, le=MAX_TIMESERIES_HOURS),
    resolution: str = Query("auto", pattern="^(auto|raw|hourly|daily)$"),
    max_points: int = Query(500, ge=10, le=5000),
    agg: str = Query("avg", pattern="^(avg|max)$"),
    db: Session = Depends(get_db)
):
    return query_timeseries(db, "kamailio", metric, hours, resolution, max_points, agg)


# ==================== RTPENGINE ENDPOINTS ====================

@router.get("/rtpengine/latest", response_model=RtpengineMetricsResponse)
def get_rtpengine_latest(db: Session = Depends(get_db)):
    metric = db.query(RtpengineMetrics).order_by(desc(RtpengineMetrics.timestamp)).first()
    if not metric:
        raise HTTPException(404, "No RTPengine metrics found")
    return metric

@router.get("/rtpengine/history", response_model=List[RtpengineMetricsResponse])
def get_rtpengine_history(
    start: datetime, end: datetime,
    limit: int = Query(60, le=1440),
    db: Session = Depends(get_db)
):
    metrics = db.query(RtpengineMetrics).filter(
        and_(RtpengineMetrics.timestamp >= start, RtpengineMetrics.timestamp <= end)
    ).order_by(desc(RtpengineMetrics.timestamp)).limit(limit).all()
    return metrics

@router.get("/rtpengine/timeseries", response_model=MetricsTimeSeries)
def get_rtpengine_timeseries(
    metric: str = Query(..., description="Metric name: active_sessions, port_usage_percent, mos_avg, etc."),
    hours: int = Query(1, ge=1, le=MAX_TIMESERIES_HOURS),
    resolution: str = Query("auto", pattern="^(auto|raw|hourly|daily)$"),
    max_points: int = Query(500, ge=10, le=5000),
    agg: str = Query("avg", pattern="^(avg|max)$"),
    db: Session = Depends(get_db)
):
    return query_timeseries(db, "rtpengine", metric, hours, resolution, max_points, agg)


# ==================== MARIADB ENDPOINTS ====================

@router.get("/mariadb/latest", response_model=MariaDBMetricsResponse)
def get_mariadb_latest(db: Session = Depends(get_db)):
    metric = db.query(MariaDBMetrics).order_by(desc(MariaDBMetrics.timestamp)).first()
    if not metric:
        raise HTTPException(404, "No MariaDB metrics found")
    return metric

@router.get("/mariadb/history", response_model=List[MariaDBMetricsResponse])
def get_mariadb_history(
    start: datetime, end: datetime,
    limit: int = Query(60, le=1440),
    db: Session = Depends(get_db)
):
    metrics = db.query(MariaDBMetrics).filter(
        and_(MariaDBMetrics.timestamp >= start, MariaDBMetrics.timestamp <= end)
    ).order_by(desc(MariaDBMetrics.timestamp)).limit(limit).all()
    return metrics

@router.get("/mariadb/timeseries", response_model=MetricsTimeSeries)
def get_mariadb_timeseries(
    metric: str = Query(..., description="Metric name: threads_connected, queries_per_second, etc."),
    hours: int = Query(1, ge=1, le=MAX_TIMESERIES_HOURS),
    resolution: str = Query("auto", pattern="^(auto|raw|hourly|daily)$"),
    max_points: int = Query(500, ge=10, le=5000),
    agg: str = Query("avg", pattern="^(avg|max)$"),
    db: Session = Depends(get_db)
):
    return query_timeseries(db, "mariadb", metric, hours, resolution, max_points, agg)


# ==================== SYSTEM ENDPOINTS ====================

@router.get("/system/latest", response_model=SystemMetricsResponse)
def get_system_latest(db: Session = Depends(get_db)):
    metric = db.query(SystemMetrics).order_by(desc(SystemMetrics.timestamp)).first()
    if not metric:
        raise HTTPException(404, "No System metrics found")
    return metric

@router.get("/system/history", response_model=List[SystemMetricsResponse])
def get_system_history(
    start: datetime, end: datetime,
    limit: int = Query(60, le=1440),
    db: Session = Depends(get_db)
):
    metrics = db.query(SystemMetrics).filter(
        and_(SystemMetrics.timestamp >= start, SystemMetrics.timestamp <= end)
    ).order_by(desc(SystemMetrics.timestamp)).limit(limit).all()
    return metrics

@router.get("/system/timeseries", response_model=MetricsTimeSeries)
def get_system_timeseries(
    metric: str = Query(..., description="Metric name: cpu_percent, memory_percent, net_rx_mbps, etc."),
    hours: int = Query(1, ge=1, le=MAX_TIMESERIES_HOURS),
    resolution: str = Query("auto", pattern="^(auto|raw|hourly|daily)$"),
    max_points: int = Query(500, ge=10, le=5000),
    agg: str = Query("avg", pattern="^(avg|max)$"),
    db: Session = Depends(get_db)
):
    return query_timeseries(db, "system", metric, hours, resolution, max_points, agg)


# ==================== ALERTS ENDPOINTS ====================

@router.get("/alerts", response_model=List[MonitoringAlertResponse])
def get_alerts(
    status: Optional[str] = Query(None, pattern="^(firing|resolved|acknowledged)$"),
    severity: Optional[str] = Query(None, pattern="^(critical|warning|info)$"),
    component: Optional[str] = Query(None),
    limit: int = Query(100, le=1000),
    db: Session = Depends(get_db)
):
    query = db.query(MonitoringAlert)
    
    if status:
        query = query.filter(MonitoringAlert.status == status)
    if severity:
        query = query.filter(MonitoringAlert.severity == severity)
    if component:
        query = query.filter(MonitoringAlert.component == component)
    
    alerts = query.order_by(desc(MonitoringAlert.timestamp)).limit(limit).all()
    return alerts

@router.post("/alerts/{alert_id}/acknowledge")
def acknowledge_alert(
    alert_id: int,
    acknowledged_by: str = Query(...),
    db: Session = Depends(get_db)
):
    alert = db.query(MonitoringAlert).filter(MonitoringAlert.id == alert_id).first()
    if not alert:
        raise HTTPException(404, "Alert not found")
    
    alert.status = "acknowledged"
    alert.acknowledged_by = acknowledged_by
    alert.acknowledged_at = datetime.utcnow()
    db.commit()
    return {"status": "acknowledged"}

@router.get("/history/status", response_model=Dict[str, Any])
def get_history_status(db: Session = Depends(get_db)):
    """Carrier-class history transparency: row counts, coverage and retention per tier."""
    from datetime import datetime as _dt
    now = _dt.utcnow()

    def _range(model):
        row = db.query(
            func.min(model.timestamp), func.max(model.timestamp), func.count(model.id)
        ).first()
        return {
            "rows": row[2] or 0,
            "oldest": row[0].isoformat() if row[0] else None,
            "newest": row[1].isoformat() if row[1] else None,
        }

    def _rollup_range(model):
        row = db.query(
            func.min(model.bucket_start), func.max(model.bucket_start), func.count(model.id)
        ).first()
        return {
            "rows": row[2] or 0,
            "oldest": row[0].isoformat() if row[0] else None,
            "newest": row[1].isoformat() if row[1] else None,
        }

    configs = {c.component: c.retention_days for c in db.query(MonitoringConfig).all()}
    return {
        "timestamp": now.isoformat(),
        "tiers": {
            "raw": {
                "interval": "1 minute",
                "retention_days": configs,
                "tables": {
                    "kamailio": _range(KamailioMetrics),
                    "rtpengine": _range(RtpengineMetrics),
                    "mariadb": _range(MariaDBMetrics),
                    "system": _range(SystemMetrics),
                },
                "note": "raw_data JSONB stripped after 48h; numeric columns kept full window",
            },
            "hourly": {
                "interval": "1 hour (avg/min/max)",
                "retention_days": 93,
                "rows": _rollup_range(MonitoringKpiHourly)["rows"],
                "coverage": _rollup_range(MonitoringKpiHourly),
            },
            "daily": {
                "interval": "1 day (avg/min/max)",
                "retention_days": 400,
                "rows": _rollup_range(MonitoringKpiDaily)["rows"],
                "coverage": _rollup_range(MonitoringKpiDaily),
            },
        },
        "resolution_policy": {
            "auto": "hours<=12 -> raw, hours<=2232 (93d) -> hourly, else daily",
        },
    }


@router.get("/config", response_model=List[Dict[str, Any]])
def get_monitoring_config(db: Session = Depends(get_db)):
    configs = db.query(MonitoringConfig).all()
    return [
        {
            "component": c.component,
            "thresholds": c.thresholds,
            "interval_seconds": c.interval_seconds,
            "retention_days": c.retention_days,
            "enabled": c.enabled,
            "updated_at": c.updated_at
        }
        for c in configs
    ]


# ==================== HEALTH CHECK ====================

@router.get("/health", response_model=HealthCheckResponse)
def health_check(db: Session = Depends(get_db)):
    """Comprehensive health check for all components"""
    now = datetime.utcnow()
    minute_ago = now - timedelta(minutes=1)
    
    components = []
    
    # Kamailio
    km = db.query(KamailioMetrics).filter(KamailioMetrics.timestamp >= minute_ago).first()
    if km:
        status = "healthy"
        if km.shm_used_mb > (km.shm_used_mb + km.shm_free_mb) * 0.9:
            status = "critical"
        elif km.db_errors > 0 or km.script_errors > 0:
            status = "degraded"
        components.append(ComponentHealth(
            component="kamailio",
            status=status,
            last_check=km.timestamp,
            message=f"Active calls: {km.active_calls}, SHM: {km.shm_used_mb:.0f}MB",
            metrics={"active_calls": km.active_calls, "shm_mb": km.shm_used_mb}
        ))
    else:
        components.append(ComponentHealth(component="kamailio", status="unknown", last_check=now, message="No recent metrics"))
    
    # RTPengine
    rm = db.query(RtpengineMetrics).filter(RtpengineMetrics.timestamp >= minute_ago).first()
    if rm:
        status = "healthy"
        if rm.cpu_percent > 80 or rm.port_usage_percent > 90:
            status = "critical"
        elif rm.cpu_percent > 60 or rm.port_usage_percent > 70:
            status = "degraded"
        components.append(ComponentHealth(
            component="rtpengine",
            status=status,
            last_check=rm.timestamp,
            message=f"Sessions: {rm.active_sessions}, CPU: {rm.cpu_percent}%",
            metrics={"active_sessions": rm.active_sessions, "cpu": rm.cpu_percent}
        ))
    else:
        components.append(ComponentHealth(component="rtpengine", status="unknown", last_check=now, message="No recent metrics"))
    
    # MariaDB
    mm = db.query(MariaDBMetrics).filter(MariaDBMetrics.timestamp >= minute_ago).order_by(desc(MariaDBMetrics.timestamp)).first()
    if mm:
        status = "healthy"
        conn_pct = (mm.threads_connected / max(mm.max_used_connections, 1)) * 100
        if conn_pct > 90 or mariadb_rates(db, mm)[1] > 5:
            status = "critical"
        elif conn_pct > 70:
            status = "degraded"
        components.append(ComponentHealth(
            component="mariadb",
            status=status,
            last_check=mm.timestamp,
            message=f"Connections: {mm.threads_connected}/{mm.max_used_connections}",
            metrics={"connections": mm.threads_connected, "qps": mm.queries_per_second}
        ))
    else:
        components.append(ComponentHealth(component="mariadb", status="unknown", last_check=now, message="No recent metrics"))
    
    # System
    sm = db.query(SystemMetrics).filter(SystemMetrics.timestamp >= minute_ago).first()
    if sm:
        status = "healthy"
        if sm.cpu_percent > 85 or sm.memory_percent > 90 or sm.disk_percent > 90:
            status = "critical"
        elif sm.cpu_percent > 70 or sm.memory_percent > 80 or sm.disk_percent > 80:
            status = "degraded"
        components.append(ComponentHealth(
            component="system",
            status=status,
            last_check=sm.timestamp,
            message=f"CPU: {sm.cpu_percent}%, Mem: {sm.memory_percent}%, Disk: {sm.disk_percent}%",
            metrics={"cpu": sm.cpu_percent, "memory": sm.memory_percent, "disk": sm.disk_percent}
        ))
    else:
        components.append(ComponentHealth(component="system", status="unknown", last_check=now, message="No recent metrics"))
    
    # Overall status
    statuses = [c.status for c in components]
    if "critical" in statuses:
        overall = "critical"
    elif "degraded" in statuses:
        overall = "degraded"
    elif "unknown" in statuses:
        overall = "degraded"
    else:
        overall = "healthy"
    
    return HealthCheckResponse(
        overall_status=overall,
        components=components,
        timestamp=now
    )


def _get_unit(metric: str) -> str:
    """Get display unit for metric"""
    units = {
        "active_calls": "calls", "active_dialogs": "dialogs", "active_sessions": "sessions",
        "shm_used_mb": "MB", "shm_free_mb": "MB", "pkg_used_mb": "MB",
        "memory_mb": "MB", "memory_used_mb": "MB", "memory_total_mb": "MB",
        "cpu_percent": "%", "memory_percent": "%", "disk_percent": "%",
        "port_usage_percent": "%", "connection_usage_percent": "%",
        "queries_per_second": "qps", "calls_per_second": "cps",
        "success_rate": "%", "shm_usage_percent": "%",
        "packet_loss_percent": "%", "net_throughput_mbps": "Mbps",
        "dispatcher_health": "gateways",
        "trunk_health": "trunks",
        "net_rx_mbps": "Mbps", "net_tx_mbps": "Mbps", "media_throughput_mbps": "Mbps",
        "jitter_avg_ms": "ms", "rtt_avg_ms": "ms", "mos_avg": "score",
        "threads_connected": "connections", "threads_running": "threads",
        "packets_in": "packets", "packets_out": "packets",
        "bytes_in": "bytes", "bytes_out": "bytes",
        "load_avg_1m": "load", "active_transactions": "transactions"
    }
    return units.get(metric, "units")