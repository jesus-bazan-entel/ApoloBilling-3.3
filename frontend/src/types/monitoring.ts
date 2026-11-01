export interface KPIReason {
  explanation: string;
  impact: 'high' | 'medium' | 'low';
  recommendation?: string;
  relatedMetrics?: string[];
}

export interface KPICalculationStep {
  step: number;
  title: string;
  description: string;
}

export interface KPICalculationMethod {
  description: string;
  formula?: string;
  steps?: KPICalculationStep[];
  dataSources?: string[];
  frequency: string;
  latency: string;
}

export interface KPIThreshold {
  name: string;
  metric: string;
  operator: '>' | '<' | '>=' | '<=' | '==';
  warning: number;
  critical: number;
  unit: string;
  description: string;
  enabled: boolean;
}

export type KPIStatus = 'healthy' | 'degraded' | 'critical' | 'warning' | 'unknown';

export interface KPICardData {
  key: string;
  title: string;
  value: string | number;
  unit?: string;
  trend?: number;
  sparklineData?: Array<{ timestamp: string; value: number | null }>;
  color: string;
  status: KPIStatus;
  icon: React.ReactNode;
  subtitle: string;
  component: string;
  metric: string;
  reason?: KPIReason;
  thresholds?: KPIThreshold[];
  calculationMethod?: KPICalculationMethod;
  lastUpdate?: string;
}

export interface DashboardOverview {
  timestamp: string;
  kamailio: KamailioSummary;
  rtpengine: RtpengineSummary;
  mariadb: MariaDBSummary;
  system: SystemSummary;
  active_alerts: number;
  critical_alerts: number;
  warning_alerts: number;
}

export interface KamailioSummary {
  active_calls: number;
  calls_per_second: number;
  success_rate: number;
  shm_usage_percent: number;
  active_transactions: number;
  dispatcher_health: string;
  dispatcher_active_destinations: number;
  dispatcher_inactive_destinations: number;
  trunk_health: string;
  trunks_online: number;
  trunks_total: number;
  trunks: TrunkStatus[];
  last_update: string;
}

export interface TrunkStatus {
  name: string;
  trunk_type: string;
  host: string;
  port: number;
  sip_status: string; // reachable | unreachable | unknown
  message: string | null;
  response_code: number | null;
  latency_ms: number | null;
  last_check: string | null;
}

export interface RtpengineSummary {
  active_sessions: number;
  port_usage_percent: number;
  packet_loss_percent: number;
  avg_mos: number;
  avg_jitter_ms: number;
  avg_rtt_ms: number;
  media_throughput_mbps: number;
  cpu_percent: number;
  memory_mb: number;
  status: string;
  last_update: string;
}

export interface MariaDBSummary {
  connections_used: number;
  connections_max: number;
  connection_usage_percent: number;
  threads_connected: number;
  threads_running: number;
  queries_per_second: number;
  slow_queries_per_minute: number;
  buffer_pool_usage_percent: number;
  replication_lag_seconds: number | null;
  deadlocks_per_minute: number;
  status: string;
  last_update: string;
}

export interface SystemSummary {
  cpu_percent: number;
  load_avg_1m: number;
  memory_percent: number;
  disk_percent: number;
  net_throughput_mbps: number;
  kamailio_status: string;
  rtpengine_status: string;
  freeswitch_status: string;
  postgres_status: string;
  last_update: string;
}

export interface Alert {
  id: number;
  timestamp: string;
  alert_name: string;
  severity: 'critical' | 'warning' | 'info';
  component: string;
  metric_name: string;
  threshold_value: number;
  current_value: number;
  operator: string;
  status: string;
  message: string;
}

export interface TimeSeriesResponse {
  metric: string;
  component: string;
  unit: string;
  /** value per `agg`; avg/min/max of the bucket on gauges (absent on rate/ratio KPIs).
   *  value is null on an interval with no data (e.g. success rate with no attempts). */
  data: TimeSeriesPoint[];
}

export interface TimeSeriesPoint {
  timestamp: string;
  value: number | null;
  avg?: number | null;
  min?: number | null;
  max?: number | null;
}
