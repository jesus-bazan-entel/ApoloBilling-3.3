"""
Monitoring Models - Carrier-class SBC Monitoring
Stores time-series metrics for Kamailio, RTPengine, MariaDB, System resources
"""
from sqlalchemy import Column, Integer, BigInteger, Float, String, DateTime, Index, Text, Boolean
from sqlalchemy.dialects.postgresql import JSONB
from app.db.session import Base
from datetime import datetime


class KamailioMetrics(Base):
    """Kamailio SIP proxy metrics collected every minute"""
    __tablename__ = "monitoring_kamailio_metrics"
    
    id = Column(BigInteger, primary_key=True, autoincrement=True)
    timestamp = Column(DateTime, default=datetime.utcnow, nullable=False, index=True)
    
    # Dialog/Call stats
    active_dialogs = Column(Integer, default=0)
    active_calls = Column(Integer, default=0)
    early_dialogs = Column(Integer, default=0)
    confirmed_dialogs = Column(Integer, default=0)
    
    # SIP method counters (per minute)
    invite_received = Column(Integer, default=0)
    invite_sent = Column(Integer, default=0)
    invite_200ok = Column(Integer, default=0)
    invite_4xx = Column(Integer, default=0)
    invite_5xx = Column(Integer, default=0)
    bye_received = Column(Integer, default=0)
    cancel_received = Column(Integer, default=0)
    ack_received = Column(Integer, default=0)
    options_received = Column(Integer, default=0)
    register_received = Column(Integer, default=0)
    register_200ok = Column(Integer, default=0)
    
    # Dispatcher/Load balancer stats
    dispatcher_active_destinations = Column(Integer, default=0)
    dispatcher_inactive_destinations = Column(Integer, default=0)

    # Trunk health: SIP OPTIONS to enabled routing_trunks via Kamailio
    trunks_online = Column(Integer, default=0)
    trunks_total = Column(Integer, default=0)

    # Transaction stats
    active_transactions = Column(Integer, default=0)
    transaction_timeouts = Column(Integer, default=0)
    
    # Shared memory
    shm_used_mb = Column(Float, default=0.0)
    shm_free_mb = Column(Float, default=0.0)
    shm_fragments = Column(Integer, default=0)
    pkg_used_mb = Column(Float, default=0.0)
    
    # Error counters
    parse_errors = Column(Integer, default=0)
    script_errors = Column(Integer, default=0)
    db_errors = Column(Integer, default=0)
    
    # Raw JSON for extensibility
    raw_data = Column(JSONB, nullable=True)
    
    __table_args__ = (
        Index('ix_kamailio_metrics_timestamp', 'timestamp'),
    )


class RtpengineMetrics(Base):
    """RTPengine media proxy metrics collected every minute"""
    __tablename__ = "monitoring_rtpengine_metrics"
    
    id = Column(BigInteger, primary_key=True, autoincrement=True)
    timestamp = Column(DateTime, default=datetime.utcnow, nullable=False, index=True)
    
    # Session stats
    active_sessions = Column(Integer, default=0)
    active_calls = Column(Integer, default=0)  # sessions with both legs
    one_sided_sessions = Column(Integer, default=0)
    deleted_sessions = Column(Integer, default=0)
    
    # Port usage
    ports_used = Column(Integer, default=0)
    ports_free = Column(Integer, default=0)
    port_usage_percent = Column(Float, default=0.0)
    
    # Traffic
    packets_in = Column(BigInteger, default=0)
    packets_out = Column(BigInteger, default=0)
    bytes_in = Column(BigInteger, default=0)
    bytes_out = Column(BigInteger, default=0)
    
    # Packet loss / quality
    packets_lost_in = Column(BigInteger, default=0)
    packets_lost_out = Column(BigInteger, default=0)
    jitter_avg_ms = Column(Float, default=0.0)
    rtt_avg_ms = Column(Float, default=0.0)
    mos_avg = Column(Float, default=0.0)
    
    # Errors
    offer_errors = Column(Integer, default=0)
    answer_errors = Column(Integer, default=0)
    delete_errors = Column(Integer, default=0)
    timeout_errors = Column(Integer, default=0)
    
    # CPU/Memory
    cpu_percent = Column(Float, default=0.0)
    memory_mb = Column(Float, default=0.0)
    threads = Column(Integer, default=0)
    
    raw_data = Column(JSONB, nullable=True)
    
    __table_args__ = (
        Index('ix_rtpengine_metrics_timestamp', 'timestamp'),
    )


class MariaDBMetrics(Base):
    """MariaDB database metrics collected every minute"""
    __tablename__ = "monitoring_mariadb_metrics"
    
    id = Column(BigInteger, primary_key=True, autoincrement=True)
    timestamp = Column(DateTime, default=datetime.utcnow, nullable=False, index=True)
    
    # Connections
    threads_connected = Column(Integer, default=0)
    threads_running = Column(Integer, default=0)
    max_used_connections = Column(Integer, default=0)
    aborted_connects = Column(Integer, default=0)
    connection_errors = Column(Integer, default=0)
    
    # Query performance
    questions = Column(BigInteger, default=0)  # queries per minute
    slow_queries = Column(Integer, default=0)
    queries_per_second = Column(Float, default=0.0)
    
    # InnoDB
    innodb_buffer_pool_used_mb = Column(Float, default=0.0)
    innodb_buffer_pool_free_mb = Column(Float, default=0.0)
    innodb_buffer_pool_pages_dirty = Column(Integer, default=0)
    innodb_row_lock_waits = Column(Integer, default=0)
    innodb_deadlocks = Column(Integer, default=0)
    
    # Replication (if applicable)
    slave_sql_running = Column(Boolean, nullable=True)
    slave_io_running = Column(Boolean, nullable=True)
    seconds_behind_master = Column(Integer, nullable=True)
    
    # Temp tables / disk usage
    created_tmp_disk_tables = Column(Integer, default=0)
    created_tmp_tables = Column(Integer, default=0)
    
    raw_data = Column(JSONB, nullable=True)
    
    __table_args__ = (
        Index('ix_mariadb_metrics_timestamp', 'timestamp'),
    )


class SystemMetrics(Base):
    """System-level metrics (CPU, Memory, Disk, Network)"""
    __tablename__ = "monitoring_system_metrics"
    
    id = Column(BigInteger, primary_key=True, autoincrement=True)
    timestamp = Column(DateTime, default=datetime.utcnow, nullable=False, index=True)
    hostname = Column(String(255), default="winproxy")
    
    # CPU
    cpu_percent = Column(Float, default=0.0)
    cpu_user_percent = Column(Float, default=0.0)
    cpu_system_percent = Column(Float, default=0.0)
    cpu_iowait_percent = Column(Float, default=0.0)
    load_avg_1m = Column(Float, default=0.0)
    load_avg_5m = Column(Float, default=0.0)
    load_avg_15m = Column(Float, default=0.0)
    cpu_cores = Column(Integer, default=0)
    
    # Memory
    memory_total_mb = Column(Float, default=0.0)
    memory_used_mb = Column(Float, default=0.0)
    memory_free_mb = Column(Float, default=0.0)
    memory_available_mb = Column(Float, default=0.0)
    memory_percent = Column(Float, default=0.0)
    swap_total_mb = Column(Float, default=0.0)
    swap_used_mb = Column(Float, default=0.0)
    swap_percent = Column(Float, default=0.0)
    
    # Disk
    disk_total_gb = Column(Float, default=0.0)
    disk_used_gb = Column(Float, default=0.0)
    disk_free_gb = Column(Float, default=0.0)
    disk_percent = Column(Float, default=0.0)
    disk_read_mbps = Column(Float, default=0.0)
    disk_write_mbps = Column(Float, default=0.0)
    disk_iops_read = Column(Integer, default=0)
    disk_iops_write = Column(Integer, default=0)
    
    # Network (primary interface)
    net_interface = Column(String(64), default="eth0")
    net_rx_mbps = Column(Float, default=0.0)
    net_tx_mbps = Column(Float, default=0.0)
    net_rx_pps = Column(Integer, default=0)
    net_tx_pps = Column(Integer, default=0)
    net_errors = Column(Integer, default=0)
    net_drops = Column(Integer, default=0)
    
    # Process-specific
    kamailio_cpu_percent = Column(Float, default=0.0)
    kamailio_memory_mb = Column(Float, default=0.0)
    kamailio_threads = Column(Integer, default=0)
    rtpengine_cpu_percent = Column(Float, default=0.0)
    rtpengine_memory_mb = Column(Float, default=0.0)
    freeswitch_cpu_percent = Column(Float, default=0.0)
    freeswitch_memory_mb = Column(Float, default=0.0)
    postgres_cpu_percent = Column(Float, default=0.0)
    postgres_memory_mb = Column(Float, default=0.0)
    
    raw_data = Column(JSONB, nullable=True)
    
    __table_args__ = (
        Index('ix_system_metrics_timestamp', 'timestamp'),
    )


class MonitoringAlert(Base):
    """Alert definitions and triggered alerts"""
    __tablename__ = "monitoring_alerts"
    
    id = Column(BigInteger, primary_key=True, autoincrement=True)
    timestamp = Column(DateTime, default=datetime.utcnow, nullable=False, index=True)
    
    # Alert identification
    alert_name = Column(String(128), nullable=False)
    severity = Column(String(16), nullable=False)  # critical, warning, info
    component = Column(String(64), nullable=False)  # kamailio, rtpengine, mariadb, system
    
    # Condition
    metric_name = Column(String(128), nullable=False)
    threshold_value = Column(Float, nullable=False)
    current_value = Column(Float, nullable=False)
    operator = Column(String(8), nullable=False)  # >, <, >=, <=, ==
    
    # Status
    status = Column(String(16), default="firing")  # firing, resolved, acknowledged
    acknowledged_by = Column(String(128), nullable=True)
    acknowledged_at = Column(DateTime, nullable=True)
    resolved_at = Column(DateTime, nullable=True)
    
    # Notification
    message = Column(Text, nullable=True)
    notification_sent = Column(Boolean, default=False)
    
    __table_args__ = (
        Index('ix_monitoring_alerts_timestamp', 'timestamp'),
        Index('ix_monitoring_alerts_component_status', 'component', 'status'),
    )


class MonitoringConfig(Base):
    """Monitoring configuration and thresholds"""
    __tablename__ = "monitoring_config"

    id = Column(BigInteger, primary_key=True, autoincrement=True)

    # Component
    component = Column(String(64), nullable=False, unique=True)

    # Thresholds (JSON for flexibility)
    thresholds = Column(JSONB, nullable=False)

    # Collection settings
    interval_seconds = Column(Integer, default=60)
    retention_days = Column(Integer, default=30)
    enabled = Column(Boolean, default=True)

    # Notification settings
    webhook_url = Column(String(512), nullable=True)
    email_recipients = Column(JSONB, nullable=True)

    updated_at = Column(DateTime, default=datetime.utcnow, onupdate=datetime.utcnow)
    updated_by = Column(String(128), nullable=True)


# ==================== CARRIER-CLASS HISTORY (rollups) ====================
# Raw 1-min tables above are the source of truth (retention: monitoring_config.retention_days,
# default 30d). Rollups below serve fast history queries without scanning raw:
#   hourly: 1 row per (component, metric, hour)   -> retention 93 days
#   daily:  1 row per (component, metric, day)    -> retention 400 days
# raw_data JSONB is stripped (NULL) after 48h to bound table size; numeric
# columns are kept for the full raw retention window.

class MonitoringKpiHourly(Base):
    """Hourly aggregates for any KPI (carrier-class history tier 1)"""
    __tablename__ = "monitoring_kpi_hourly"

    id = Column(BigInteger, primary_key=True, autoincrement=True)
    component = Column(String(32), nullable=False)
    metric = Column(String(64), nullable=False)
    bucket_start = Column(DateTime, nullable=False)
    avg_value = Column(Float, nullable=False)
    min_value = Column(Float, nullable=False)
    max_value = Column(Float, nullable=False)
    samples = Column(Integer, nullable=False, default=0)

    __table_args__ = (
        Index('ix_kpi_hourly_comp_metric_bucket', 'component', 'metric', 'bucket_start', unique=True),
        Index('ix_kpi_hourly_bucket', 'bucket_start'),
    )


class MonitoringKpiDaily(Base):
    """Daily aggregates for any KPI (carrier-class history tier 2)"""
    __tablename__ = "monitoring_kpi_daily"

    id = Column(BigInteger, primary_key=True, autoincrement=True)
    component = Column(String(32), nullable=False)
    metric = Column(String(64), nullable=False)
    bucket_start = Column(DateTime, nullable=False)
    avg_value = Column(Float, nullable=False)
    min_value = Column(Float, nullable=False)
    max_value = Column(Float, nullable=False)
    samples = Column(Integer, nullable=False, default=0)

    __table_args__ = (
        Index('ix_kpi_daily_comp_metric_bucket', 'component', 'metric', 'bucket_start', unique=True),
        Index('ix_kpi_daily_bucket', 'bucket_start'),
    )