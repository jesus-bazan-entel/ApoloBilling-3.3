"""
Monitoring Schemas - Pydantic models for API serialization
"""
from pydantic import BaseModel
from typing import Optional, List, Dict, Any
from datetime import datetime


# ==================== KAMAILIO SCHEMAS ====================

class KamailioMetricsBase(BaseModel):
    timestamp: datetime
    active_dialogs: int = 0
    active_calls: int = 0
    early_dialogs: int = 0
    confirmed_dialogs: int = 0
    invite_received: int = 0
    invite_sent: int = 0
    invite_200ok: int = 0
    invite_4xx: int = 0
    invite_5xx: int = 0
    bye_received: int = 0
    cancel_received: int = 0
    ack_received: int = 0
    options_received: int = 0
    register_received: int = 0
    register_200ok: int = 0
    dispatcher_active_destinations: int = 0
    dispatcher_inactive_destinations: int = 0
    trunks_online: int = 0
    trunks_total: int = 0
    active_transactions: int = 0
    transaction_timeouts: int = 0
    shm_used_mb: float = 0.0
    shm_free_mb: float = 0.0
    shm_fragments: int = 0
    pkg_used_mb: float = 0.0
    parse_errors: int = 0
    script_errors: int = 0
    db_errors: int = 0
    raw_data: Optional[Dict[str, Any]] = None


class KamailioMetricsResponse(KamailioMetricsBase):
    id: int
    
    class Config:
        from_attributes = True


class TrunkStatus(BaseModel):
    """Last SIP OPTIONS result for one enabled routing trunk"""
    name: str
    trunk_type: str
    host: str
    port: int
    sip_status: str  # "reachable", "unreachable", "unknown"
    message: Optional[str] = None
    response_code: Optional[int] = None
    latency_ms: Optional[int] = None
    last_check: Optional[datetime] = None


class KamailioSummary(BaseModel):
    """Summary for dashboard cards"""
    active_calls: int
    calls_per_second: float
    success_rate: float  # 200ok / (200ok + 4xx + 5xx)
    shm_usage_percent: float
    active_transactions: int
    dispatcher_health: str  # "healthy", "degraded", "critical"
    dispatcher_active_destinations: int = 0
    dispatcher_inactive_destinations: int = 0
    trunk_health: str = "unknown"  # "healthy", "degraded", "critical", "unknown"
    trunks_online: int = 0
    trunks_total: int = 0
    trunks: List[TrunkStatus] = []
    last_update: datetime


# ==================== RTPENGINE SCHEMAS ====================

class RtpengineMetricsBase(BaseModel):
    timestamp: datetime
    active_sessions: int = 0
    active_calls: int = 0
    one_sided_sessions: int = 0
    deleted_sessions: int = 0
    ports_used: int = 0
    ports_free: int = 0
    port_usage_percent: float = 0.0
    packets_in: int = 0
    packets_out: int = 0
    bytes_in: int = 0
    bytes_out: int = 0
    packets_lost_in: int = 0
    packets_lost_out: int = 0
    jitter_avg_ms: float = 0.0
    rtt_avg_ms: float = 0.0
    mos_avg: float = 0.0
    offer_errors: int = 0
    answer_errors: int = 0
    delete_errors: int = 0
    timeout_errors: int = 0
    cpu_percent: float = 0.0
    memory_mb: float = 0.0
    threads: int = 0
    raw_data: Optional[Dict[str, Any]] = None


class RtpengineMetricsResponse(RtpengineMetricsBase):
    id: int
    
    class Config:
        from_attributes = True


class RtpengineSummary(BaseModel):
    active_sessions: int
    port_usage_percent: float
    packet_loss_percent: float
    avg_mos: float
    avg_jitter_ms: float
    avg_rtt_ms: float
    media_throughput_mbps: float
    cpu_percent: float
    memory_mb: float
    status: str  # "healthy", "degraded", "critical"
    last_update: datetime


# ==================== MARIADB SCHEMAS ====================

class MariaDBMetricsBase(BaseModel):
    timestamp: datetime
    threads_connected: int = 0
    threads_running: int = 0
    max_used_connections: int = 0
    aborted_connects: int = 0
    connection_errors: int = 0
    questions: int = 0
    slow_queries: int = 0
    queries_per_second: Optional[float] = None
    innodb_buffer_pool_used_mb: float = 0.0
    innodb_buffer_pool_free_mb: float = 0.0
    innodb_buffer_pool_pages_dirty: int = 0
    innodb_row_lock_waits: int = 0
    innodb_deadlocks: int = 0
    slave_sql_running: Optional[bool] = None
    slave_io_running: Optional[bool] = None
    seconds_behind_master: Optional[int] = None
    created_tmp_disk_tables: int = 0
    created_tmp_tables: int = 0
    raw_data: Optional[Dict[str, Any]] = None


class MariaDBMetricsResponse(MariaDBMetricsBase):
    id: int
    
    class Config:
        from_attributes = True


class MariaDBSummary(BaseModel):
    connections_used: int
    connections_max: int
    connection_usage_percent: float
    queries_per_second: float
    slow_queries_per_minute: int
    buffer_pool_usage_percent: float
    replication_lag_seconds: Optional[int]
    deadlocks_per_minute: int
    status: str
    last_update: datetime


# ==================== SYSTEM SCHEMAS ====================

class SystemMetricsBase(BaseModel):
    timestamp: datetime
    hostname: str = "winproxy"
    cpu_percent: float = 0.0
    cpu_user_percent: float = 0.0
    cpu_system_percent: float = 0.0
    cpu_iowait_percent: float = 0.0
    load_avg_1m: float = 0.0
    load_avg_5m: float = 0.0
    load_avg_15m: float = 0.0
    cpu_cores: int = 0
    memory_total_mb: float = 0.0
    memory_used_mb: float = 0.0
    memory_free_mb: float = 0.0
    memory_available_mb: float = 0.0
    memory_percent: float = 0.0
    swap_total_mb: float = 0.0
    swap_used_mb: float = 0.0
    swap_percent: float = 0.0
    disk_total_gb: float = 0.0
    disk_used_gb: float = 0.0
    disk_free_gb: float = 0.0
    disk_percent: float = 0.0
    disk_read_mbps: float = 0.0
    disk_write_mbps: float = 0.0
    disk_iops_read: int = 0
    disk_iops_write: int = 0
    net_interface: str = "eth0"
    net_rx_mbps: float = 0.0
    net_tx_mbps: float = 0.0
    net_rx_pps: int = 0
    net_tx_pps: int = 0
    net_errors: int = 0
    net_drops: int = 0
    kamailio_cpu_percent: float = 0.0
    kamailio_memory_mb: float = 0.0
    kamailio_threads: int = 0
    rtpengine_cpu_percent: float = 0.0
    rtpengine_memory_mb: float = 0.0
    freeswitch_cpu_percent: float = 0.0
    freeswitch_memory_mb: float = 0.0
    postgres_cpu_percent: float = 0.0
    postgres_memory_mb: float = 0.0
    raw_data: Optional[Dict[str, Any]] = None


class SystemMetricsResponse(SystemMetricsBase):
    id: int
    
    class Config:
        from_attributes = True


class SystemSummary(BaseModel):
    cpu_percent: float
    load_avg_1m: float
    memory_percent: float
    disk_percent: float
    net_throughput_mbps: float
    kamailio_status: str
    rtpengine_status: str
    freeswitch_status: str
    postgres_status: str
    last_update: datetime


# ==================== ALERT SCHEMAS ====================

class MonitoringAlertBase(BaseModel):
    alert_name: str
    severity: str  # critical, warning, info
    component: str
    metric_name: str
    threshold_value: float
    current_value: float
    operator: str
    status: str = "firing"
    message: Optional[str] = None


class MonitoringAlertResponse(MonitoringAlertBase):
    id: int
    timestamp: datetime
    acknowledged_by: Optional[str] = None
    acknowledged_at: Optional[datetime] = None
    resolved_at: Optional[datetime] = None
    notification_sent: bool = False
    
    class Config:
        from_attributes = True


class AlertRule(BaseModel):
    """Alert rule configuration"""
    component: str
    metric_name: str
    threshold_value: float
    operator: str  # >, <, >=, <=, ==
    severity: str  # critical, warning, info
    enabled: bool = True
    notification_channels: List[str] = []  # webhook, email, slack


# ==================== DASHBOARD AGGREGATE ====================

class DashboardOverview(BaseModel):
    """Complete dashboard overview for carrier-class monitoring"""
    timestamp: datetime
    kamailio: KamailioSummary
    rtpengine: RtpengineSummary
    mariadb: MariaDBSummary
    system: SystemSummary
    active_alerts: int
    critical_alerts: int
    warning_alerts: int


class MetricsTimeSeries(BaseModel):
    """Time series data for charts"""
    metric: str
    component: str
    unit: str
    data: List[Dict[str, Any]]  # [{timestamp, value}, ...]


# ==================== HEALTH CHECK ====================

class ComponentHealth(BaseModel):
    component: str
    status: str  # healthy, degraded, critical, unknown
    last_check: datetime
    message: Optional[str] = None
    metrics: Dict[str, Any] = {}


class HealthCheckResponse(BaseModel):
    overall_status: str
    components: List[ComponentHealth]
    timestamp: datetime