#!/usr/bin/env python3
"""
Standalone Monitoring Service for ApoloBilling SBC
Runs as a separate service on port 8080
"""
import os
import sys
import asyncio
import logging
from contextlib import asynccontextmanager

# Add project root to path
sys.path.insert(0, '/opt/ApoloBilling')

# The unit's Environment= wins; the literal is this site's original value.
os.environ.setdefault('DATABASE_URL', "postgresql://apolo_user:apolo_password_2024@127.0.0.1:5432/apolo_billing")

from fastapi import FastAPI, Depends, HTTPException, Query
from fastapi.middleware.cors import CORSMiddleware
from sqlalchemy.orm import Session
from sqlalchemy import desc, func, and_, text
from datetime import datetime, timedelta
from typing import List, Optional, Dict, Any

# Local imports
from app.db.session import SessionLocal, Base, engine
from app.models.monitoring import (
    KamailioMetrics, RtpengineMetrics, MariaDBMetrics, 
    SystemMetrics, MonitoringAlert, MonitoringConfig
)
from app.monitoring.service import MonitoringService, CollectorResult
from app.monitoring.rates import mariadb_rates
from app.api.routers import monitoring

# Configure logging
logging.basicConfig(
    level=logging.INFO,
    format='%(asctime)s - %(name)s - %(levelname)s - %(message)s'
)
logger = logging.getLogger(__name__)

# Monitoring service instance
monitoring_service = MonitoringService()


@asynccontextmanager
async def lifespan(app: FastAPI):
    """Application lifespan manager"""
    # Startup
    logger.info("Starting Monitoring Service...")
    asyncio.create_task(monitoring_service.run_collection_loop())
    yield
    # Shutdown
    logger.info("Stopping Monitoring Service...")
    monitoring_service.stop()


app = FastAPI(
    title="ApoloBilling SBC Monitoring",
    description="Carrier-class monitoring API for Kamailio, RTPengine, MariaDB, and System metrics",
    version="1.0.0",
    lifespan=lifespan
)

# CORS
app.add_middleware(
    CORSMiddleware,
    allow_origins=["http://localhost:3000", "http://127.0.0.1:3000", "https://winproxy:5000"],
    allow_credentials=True,
    allow_methods=["*"],
    allow_headers=["*"],
)

# Include monitoring router
app.include_router(monitoring.router, prefix="/api/v1/monitoring")

def get_db():
    db = SessionLocal()
    try:
        yield db
    finally:
        db.close()


# ==================== HEALTH ====================

@app.get("/api/v1/monitoring/health")
async def health_check():
    return {"status": "healthy", "service": "apolo-sbc-monitoring", "version": "1.0.0"}


# ==================== OVERVIEW ====================

@app.get("/api/v1/monitoring/overview")
async def get_overview(db: Session = Depends(get_db)):
    """Get complete dashboard overview"""
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
    
    # Alert counts
    active_alerts = db.query(MonitoringAlert).filter(
        MonitoringAlert.status == "firing"
    ).count()
    critical_alerts = db.query(MonitoringAlert).filter(
        MonitoringAlert.status == "firing", MonitoringAlert.severity == "critical"
    ).count()
    warning_alerts = db.query(MonitoringAlert).filter(
        MonitoringAlert.status == "firing", MonitoringAlert.severity == "warning"
    ).count()
    
    # Build response
    def k_summary():
        if not kamailio_latest:
            return {"active_calls": 0, "calls_per_second": 0, "success_rate": 0, "shm_usage_percent": 0, "active_transactions": 0, "dispatcher_health": "unknown", "last_update": now.isoformat()}
        k = kamailio_latest
        total_inv = k.invite_200ok + k.invite_4xx + k.invite_5xx
        sr = (k.invite_200ok / total_inv * 100) if total_inv > 0 else 100
        shm_t = k.shm_used_mb + k.shm_free_mb
        shm_p = (k.shm_used_mb / shm_t * 100) if shm_t > 0 else 0
        td = k.dispatcher_active_destinations + k.dispatcher_inactive_destinations
        if td == 0: dh = "unknown"
        elif k.dispatcher_inactive_destinations == 0: dh = "healthy"
        elif k.dispatcher_inactive_destinations / td > 0.5: dh = "critical"
        else: dh = "degraded"
        return {
            "active_calls": k.active_calls, "calls_per_second": 0, "success_rate": round(sr, 2),
            "shm_usage_percent": round(shm_p, 2), "active_transactions": k.active_transactions,
            "dispatcher_health": dh, "last_update": k.timestamp.isoformat()
        }
    
    def r_summary():
        if not rtpengine_latest:
            return {"active_sessions": 0, "port_usage_percent": 0, "packet_loss_percent": 0, "avg_mos": 0, "avg_jitter_ms": 0, "avg_rtt_ms": 0, "media_throughput_mbps": 0, "cpu_percent": 0, "memory_mb": 0, "status": "unknown", "last_update": now.isoformat()}
        r = rtpengine_latest
        tp = r.packets_in + r.packets_out
        lp = r.packets_lost_in + r.packets_lost_out
        lp_p = (lp / tp * 100) if tp > 0 else 0
        th = (r.bytes_in + r.bytes_out) / (1024*1024) / 60
        st = "healthy"
        if r.cpu_percent > 80 or lp_p > 5 or r.mos_avg < 3.5: st = "critical"
        elif r.cpu_percent > 60 or lp_p > 1 or r.mos_avg < 4.0: st = "degraded"
        return {
            "active_sessions": r.active_sessions, "port_usage_percent": round(r.port_usage_percent, 2),
            "packet_loss_percent": round(lp_p, 2), "avg_mos": round(r.mos_avg, 2),
            "avg_jitter_ms": round(r.jitter_avg_ms, 2), "avg_rtt_ms": round(r.rtt_avg_ms, 2),
            "media_throughput_mbps": round(th, 2), "cpu_percent": round(r.cpu_percent, 2),
            "memory_mb": round(r.memory_mb, 2), "status": st, "last_update": r.timestamp.isoformat()
        }
    
    def m_summary():
        if not mariadb_latest:
            return {"connections_used": 0, "connections_max": 0, "connection_usage_percent": 0, "queries_per_second": 0, "slow_queries_per_minute": 0, "buffer_pool_usage_percent": 0, "replication_lag_seconds": None, "deadlocks_per_minute": 0, "status": "unknown", "last_update": now.isoformat()}
        m = mariadb_latest
        slow_pm, deadlocks_pm = mariadb_rates(db, m)
        mc = m.max_used_connections or 151
        cp = (m.threads_connected / mc * 100) if mc > 0 else 0
        bp = m.innodb_buffer_pool_used_mb + m.innodb_buffer_pool_free_mb
        bp_p = (m.innodb_buffer_pool_used_mb / bp * 100) if bp > 0 else 0
        st = "healthy"
        if cp > 90 or slow_pm > 10 or deadlocks_pm > 5: st = "critical"
        elif cp > 70 or slow_pm > 5 or deadlocks_pm > 1: st = "degraded"
        return {
            "connections_used": m.threads_connected, "connections_max": mc,
            "connection_usage_percent": round(cp, 2), "queries_per_second": round(m.queries_per_second or 0, 2),
            "slow_queries_per_minute": slow_pm, "buffer_pool_usage_percent": round(bp_p, 2),
            "replication_lag_seconds": m.seconds_behind_master, "deadlocks_per_minute": deadlocks_pm,
            "status": st, "last_update": m.timestamp.isoformat()
        }
    
    def s_summary():
        if not system_latest:
            return {"cpu_percent": 0, "load_avg_1m": 0, "memory_percent": 0, "disk_percent": 0, "net_throughput_mbps": 0, "kamailio_status": "unknown", "rtpengine_status": "unknown", "freeswitch_status": "unknown", "postgres_status": "unknown", "last_update": now.isoformat()}
        s = system_latest
        nt = s.net_rx_mbps + s.net_tx_mbps
        def ps(cpu, mem):
            if cpu > 80 or mem > 2000: return "critical"
            elif cpu > 50 or mem > 1000: return "degraded"
            return "healthy"
        return {
            "cpu_percent": round(s.cpu_percent, 2), "load_avg_1m": round(s.load_avg_1m, 2),
            "memory_percent": round(s.memory_percent, 2), "disk_percent": round(s.disk_percent, 2),
            "net_throughput_mbps": round(nt, 2),
            "kamailio_status": ps(s.kamailio_cpu_percent, s.kamailio_memory_mb),
            "rtpengine_status": ps(s.rtpengine_cpu_percent, s.rtpengine_memory_mb),
            "freeswitch_status": ps(s.freeswitch_cpu_percent, s.freeswitch_memory_mb),
            "postgres_status": ps(s.postgres_cpu_percent, s.postgres_memory_mb),
            "last_update": s.timestamp.isoformat()
        }
    
    return {
        "timestamp": now.isoformat(),
        "kamailio": k_summary(),
        "rtpengine": r_summary(),
        "mariadb": m_summary(),
        "system": s_summary(),
        "active_alerts": active_alerts,
        "critical_alerts": critical_alerts,
        "warning_alerts": warning_alerts
    }


# ==================== TIMESERIES ====================

@app.get("/api/v1/monitoring/{component}/timeseries")
async def get_timeseries(
    component: str,
    metric: str = Query(...),
    hours: int = Query(1, ge=1, le=24),
    db: Session = Depends(get_db)
):
    start = datetime.utcnow() - timedelta(hours=hours)
    
    model_map = {
        "kamailio": KamailioMetrics,
        "rtpengine": RtpengineMetrics,
        "mariadb": MariaDBMetrics,
        "system": SystemMetrics,
    }
    
    if component not in model_map:
        raise HTTPException(400, f"Invalid component. Valid: {list(model_map.keys())}")
    
    Model = model_map[component]
    
    # Validate metric
    valid_metrics = [c.key for c in Model.__table__.columns if c.key not in ['id', 'timestamp', 'raw_data']]
    if metric not in valid_metrics:
        raise HTTPException(400, f"Invalid metric. Valid: {valid_metrics}")
    
    results = db.query(
        Model.timestamp,
        getattr(Model, metric)
    ).filter(Model.timestamp >= start).order_by(Model.timestamp).all()
    
    data = [{"timestamp": r[0].isoformat(), "value": r[1]} for r in results if r[1] is not None]
    
    units = {
        "active_calls": "calls", "active_sessions": "sessions", "active_dialogs": "dialogs",
        "shm_used_mb": "MB", "memory_mb": "MB", "memory_percent": "%",
        "cpu_percent": "%", "port_usage_percent": "%", "connection_usage_percent": "%",
        "queries_per_second": "qps", "calls_per_second": "cps",
        "net_rx_mbps": "Mbps", "net_tx_mbps": "Mbps", "media_throughput_mbps": "Mbps",
        "jitter_avg_ms": "ms", "rtt_avg_ms": "ms", "mos_avg": "score",
        "threads_connected": "connections", "packets_in": "packets",
        "load_avg_1m": "load"
    }
    
    return {
        "metric": metric, "component": component, "unit": units.get(metric, "units"),
        "data": data
    }


# ==================== ALERTS ====================

@app.get("/api/v1/monitoring/alerts")
async def get_alerts(
    status: Optional[str] = Query(None, pattern="^(firing|resolved|acknowledged)$"),
    severity: Optional[str] = Query(None, pattern="^(critical|warning|info)$"),
    component: Optional[str] = Query(None),
    limit: int = Query(100, le=1000),
    db: Session = Depends(get_db)
):
    query = db.query(MonitoringAlert)
    if status: query = query.filter(MonitoringAlert.status == status)
    if severity: query = query.filter(MonitoringAlert.severity == severity)
    if component: query = query.filter(MonitoringAlert.component == component)
    
    alerts = query.order_by(desc(MonitoringAlert.timestamp)).limit(limit).all()
    
    return [{
        "id": a.id, "timestamp": a.timestamp.isoformat(),
        "alert_name": a.alert_name, "severity": a.severity,
        "component": a.component, "metric_name": a.metric_name,
        "threshold_value": a.threshold_value, "current_value": a.current_value,
        "operator": a.operator, "status": a.status,
        "message": a.message, "acknowledged_by": a.acknowledged_by,
        "acknowledged_at": a.acknowledged_at.isoformat() if a.acknowledged_at else None,
        "resolved_at": a.resolved_at.isoformat() if a.resolved_at else None,
    } for a in alerts]


# ==================== METRICS LATEST ====================

@app.get("/api/v1/monitoring/{component}/latest")
async def get_latest(component: str, db: Session = Depends(get_db)):
    model_map = {
        "kamailio": KamailioMetrics, "rtpengine": RtpengineMetrics,
        "mariadb": MariaDBMetrics, "system": SystemMetrics,
    }
    if component not in model_map:
        raise HTTPException(400, "Invalid component")
    
    Model = model_map[component]
    latest = db.query(Model).order_by(desc(Model.timestamp)).first()
    if not latest:
        raise HTTPException(404, "No metrics found")
    return latest


# Run with: uvicorn monitoring_service:app --host 0.0.0.0 --port 8080
if __name__ == "__main__":
    import uvicorn
    uvicorn.run(app, host="0.0.0.0", port=8080, log_level="info")