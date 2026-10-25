import React, { useState, useEffect } from 'react';
import { useQuery } from '@tanstack/react-query';
import { Tabs, TabsContent, TabsList, TabsTrigger } from '@radix-ui/react-tabs';
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from '@radix-ui/react-select';
import { Tooltip, TooltipTrigger, TooltipContent, TooltipProvider } from '@radix-ui/react-tooltip';
import {
  RefreshCw, AlertTriangle, CheckCircle, XCircle, Info,
  Activity, Cpu, MemoryStick, Phone, Volume2, Database, Server,
  Loader2, AlertCircle, ChevronDown, MousePointerClick
} from 'lucide-react';
import { XAxis, YAxis, CartesianGrid, Tooltip as RechartsTooltip, ResponsiveContainer, AreaChart, Area } from 'recharts';
import apiClient from '../../api/client';
import KPICard from '../../components/Monitoring/KPICard';
import type {
  TimeSeriesResponse, DashboardOverview, Alert, KPICardData, KPIStatus,
} from '../../types/monitoring';

const StatusBadge = ({ status, label }: { status: string; label: string }) => {
  if (status === 'unknown') {
    return (
      <span className="inline-flex items-center gap-1 px-2 py-0.5 rounded-full text-xs font-medium border bg-gray-100 text-gray-600 border-gray-200">
        <Info className="w-3 h-3" /> N/A
      </span>
    );
  }
  const map: Record<string, string> = {
    healthy: 'bg-green-100 text-green-800 border-green-200',
    degraded: 'bg-orange-100 text-orange-800 border-orange-200',
    warning: 'bg-yellow-100 text-yellow-800 border-yellow-200',
    critical: 'bg-red-100 text-red-800 border-red-200',
  };
  const Icon = status === 'healthy' ? CheckCircle : status === 'critical' ? XCircle : AlertTriangle;
  return (
    <span className={`inline-flex items-center gap-1 px-2 py-0.5 rounded-full text-xs font-semibold border ${map[status] || map.unknown}`}>
      <Icon className="w-3 h-3" /> {label || status}
    </span>
  );
};

const DetailTable = ({ title, subtitle, rows }: { title: string; subtitle?: string; rows: Array<[string, React.ReactNode]> }) => (
  <div className="rounded-xl border border-gray-200 bg-white p-5">
    <h3 className="font-semibold text-gray-900">{title}</h3>
    {subtitle && <p className="text-xs text-gray-500 mt-0.5 mb-3">{subtitle}</p>}
    <dl className="divide-y divide-gray-100">
      {rows.map(([k, v], i) => (
        <div key={i} className="flex justify-between gap-4 py-1.5 text-sm">
          <dt className="text-gray-500">{k}</dt>
          <dd className="font-mono font-medium text-gray-900 text-right">{v}</dd>
        </div>
      ))}
    </dl>
  </div>
);

const MonitoringDashboard: React.FC = () => {
  const [timeRange, setTimeRange] = useState('1h');
  const [historyData, setHistoryData] = useState<Record<string, TimeSeriesResponse>>({});
  const [selectedMetrics, setSelectedMetrics] = useState<string[]>([
    'kamailio.active_calls', 'rtpengine.active_sessions',
    'mariadb.threads_connected', 'system.cpu_percent',
  ]);

  const { data: overview, isLoading, error, refetch } = useQuery({
    queryKey: ['monitoringOverview'],
    queryFn: async () => (await apiClient.get('/monitoring/overview')).data as DashboardOverview,
    refetchInterval: 30000,
  });

  const { data: alertsData } = useQuery({
    queryKey: ['monitoringAlerts'],
    queryFn: async () => (await apiClient.get('/monitoring/alerts?status=firing&limit=20')).data as Alert[],
    refetchInterval: 30000,
  });

  const { data: histStatus } = useQuery({
    queryKey: ['monitoringHistoryStatus'],
    queryFn: async () => (await apiClient.get('/monitoring/history/status')).data,
    refetchInterval: 300000,
  });

  const LIMA_TZ = 'America/Lima';

  // Backend (FastAPI monitoring service) stores naive UTC timestamps and serializes them
  // with plain .isoformat() — no "Z"/offset suffix. Per the ES spec, a date-time string
  // with no timezone marker parses as LOCAL time, not UTC, so `new Date(t)` silently
  // shifts the instant by the viewer's UTC offset. Force UTC interpretation here, then
  // let toLocale*String's `timeZone: LIMA_TZ` do the actual GMT-5 conversion for display.
  const parseUtc = (t: string | number | Date): Date => {
    if (typeof t === 'string' && !/Z$|[+-]\d{2}:?\d{2}$/.test(t)) {
      return new Date(t + 'Z');
    }
    return new Date(t);
  };

  const fmtTick = (t: string) => {
    const d = parseUtc(t);
    if (timeRange === '7d' || timeRange === '30d') {
      return d.toLocaleDateString('es-PE', { timeZone: LIMA_TZ }) + ' ' + d.toLocaleTimeString('es-PE', { timeZone: LIMA_TZ, hour: '2-digit', minute: '2-digit' });
    }
    return d.toLocaleTimeString('es-PE', { timeZone: LIMA_TZ });
  };

  const fmtDateTime = (t: string | number | Date) => parseUtc(t).toLocaleString('es-PE', { timeZone: LIMA_TZ });

  // Detalle por componente (tablas inferiores): NO repiten los KPI cards superiores.
  // Cada tab muestra el desglose que el overview resumido no trae.
  const { data: kamDetail } = useQuery({
    queryKey: ['monitoringKamDetail'],
    queryFn: async () => (await apiClient.get('/monitoring/kamailio/latest')).data,
    refetchInterval: 30000,
  });
  const { data: rtpDetail } = useQuery({
    queryKey: ['monitoringRtpDetail'],
    queryFn: async () => (await apiClient.get('/monitoring/rtpengine/latest')).data,
    refetchInterval: 30000,
  });
  const { data: dbDetail } = useQuery({
    queryKey: ['monitoringDbDetail'],
    queryFn: async () => (await apiClient.get('/monitoring/mariadb/latest')).data,
    refetchInterval: 30000,
  });
  const { data: sysDetail } = useQuery({
    queryKey: ['monitoringSysDetail'],
    queryFn: async () => (await apiClient.get('/monitoring/system/latest')).data,
    refetchInterval: 30000,
  });

  const alerts: Alert[] = alertsData ?? [];

  useEffect(() => {
    if (selectedMetrics.length === 0 || !overview) return;
    const fetchTS = async () => {
      const results: Record<string, TimeSeriesResponse> = {};
      // Carrier-class ranges: 1h/6h/24h raw-hourly, 7d/30d hourly-daily (auto tier)
      const hours = timeRange === '7d' ? 168 : timeRange === '30d' ? 720 : parseInt(timeRange.replace('h', '')) || 1;
      for (const key of selectedMetrics) {
        const [component, metric] = key.split('.');
        try {
          const r = await apiClient.get(`/monitoring/${component}/timeseries`, { params: { metric, hours } });
          results[key] = r.data;
        } catch (e) { console.error(e); }
      }
      setHistoryData(results);
    };
    fetchTS();
  }, [selectedMetrics, overview, timeRange]);

  if (isLoading) return <div className="flex items-center justify-center min-h-[400px]"><Loader2 className="w-10 h-10 animate-spin text-[#ff5a0a]" /></div>;
  if (error) return (
    <div className="bg-red-50 border border-red-200 rounded-xl p-6">
      <div className="flex items-center gap-3"><AlertCircle className="w-6 h-6 text-red-600" />
        <div><h3 className="font-semibold text-red-800">Error loading monitoring data</h3>
        <p className="text-sm text-red-600">{error instanceof Error ? error.message : 'Unknown error'}</p></div>
      </div>
      <button onClick={() => refetch()} className="mt-4 px-4 py-2 bg-red-600 text-white rounded-lg"><RefreshCw className="w-4 h-4 mr-2 inline" /> Retry</button>
    </div>
  );
  if (!overview) return null;

  const { kamailio, rtpengine, mariadb, system } = overview;
  const trunks = kamailio.trunks ?? [];
  const trunksOnline = kamailio.trunks_online ?? 0;
  const trunksTotal = kamailio.trunks_total ?? trunks.length;
  const trunksDown = trunks.filter(t => t.sip_status !== 'reachable');
  const trunkHealth = kamailio.trunk_health ?? 'unknown';

  const buildKPIData = (): KPICardData[] => [
    {
      key: 'kamailio.active_calls', title: 'Active Calls', value: kamailio.active_calls, unit: 'calls',
      sparklineData: historyData['kamailio.active_calls']?.data || [], color: '#ff5a0a',
      status: kamailio.active_calls > 0 ? 'healthy' : 'unknown',
      icon: <Phone className="w-5 h-5" />, subtitle: 'Kamailio · SIP', component: 'kamailio', metric: 'active_calls',
      lastUpdate: kamailio.last_update,
      reason: {
        explanation: kamailio.active_calls > 0
          ? `${kamailio.active_calls} diálogos SIP activos en Kamailio. El SBC está cursando tráfico correctamente. Este KPI es independiente de Dispatcher Health.`
          : 'Sin diálogos activos. Puede ser horario valle o corte de tráfico entrante.',
        impact: 'high',
        recommendation: 'Si cae a 0 en horario pico, revisar carriers, drouting y logs de Kamailio (acc/auth).',
        relatedMetrics: ['calls_per_second', 'success_rate', 'trunk_health'],
      },
      thresholds: [{ name: 'Sin tráfico', metric: 'active_calls', operator: '==', warning: 0, critical: 0, unit: 'calls', description: '0 llamadas en horario pico = posible corte', enabled: true }],
      calculationMethod: {
        description: 'Conteo de diálogos SIP en estados confirmados reportados por el módulo dialog de Kamailio vía kamcmd.',
        formula: 'dlg.stats_active → ongoing',
        steps: [
          { step: 1, title: 'Kamailio dialog', description: 'El módulo dialog trackea cada INVITE hasta BYE/timeout.' },
          { step: 2, title: 'kamcmd', description: 'El colector ejecuta `kamcmd dlg.stats_active` cada 60s.' },
          { step: 3, title: 'Persistencia', description: 'Se guarda confirmed_dialogs → active_calls en monitoring_kamailio_metrics.' },
        ],
        dataSources: ['kamcmd dlg.stats_active', 'PostgreSQL monitoring_kamailio_metrics'],
        frequency: 'Cada 60s', latency: '< 1 min',
      },
    },
    {
      key: 'kamailio.trunk_health', title: 'Trunk Health', value: `${trunksOnline}/${trunksTotal}`, unit: 'online',
      color: trunkHealth === 'healthy' ? '#22c55e' : trunkHealth === 'degraded' ? '#f97316' : trunkHealth === 'unknown' ? '#6b7280' : '#ef4444',
      status: trunkHealth as KPIStatus,
      icon: <Server className="w-5 h-5" />, subtitle: 'Kamailio · Troncales', component: 'kamailio', metric: 'trunk_health',
      lastUpdate: kamailio.last_update,
      reason: {
        explanation: trunksTotal === 0
          ? 'No hay troncales habilitados en Enrutamiento.'
          : trunkHealth === 'unknown'
            ? 'Sin resultados recientes de la sonda SIP OPTIONS (más de 5 min): el colector de monitoreo está detenido o Kamailio no responde a kamcmd.'
            : trunksDown.length === 0
              ? `Los ${trunksTotal} troncales responden a SIP OPTIONS: ` +
                trunks.map(t => `${t.name} (${t.response_code}, ${t.latency_ms} ms)`).join(', ') + '.'
              : `${trunksDown.length} de ${trunksTotal} troncales no responden: ` +
                trunksDown.map(t => `${t.name} (${t.message ?? 'sin respuesta'})`).join('; ') + '. ' +
                (trunksDown.length / Math.max(1, trunksTotal) > 0.5
                  ? 'Más de la mitad caídos = CRITICAL: la mayoría de destinos de llamada no está disponible.'
                  : 'Las llamadas hacia ese troncal fallarán o saldrán por el failover de su grupo.'),
        impact: 'high',
        recommendation: 'Ver el troncal en Enrutamiento → Troncales (estado y último OPTIONS). Comprobar conectividad y firewall hacia host:puerto, y que el peer acepte OPTIONS desde la IP del SBC.',
        relatedMetrics: ['active_calls', 'success_rate', 'invite_5xx'],
      },
      thresholds: [
        { name: 'Troncales caídos', metric: 'trunks_offline', operator: '>', warning: 0, critical: Math.ceil(trunksTotal / 2), unit: 'troncales', description: '>50% sin respuesta = critical', enabled: true },
      ],
      calculationMethod: {
        description: 'Cada 60s el colector pide a Kamailio un SIP OPTIONS a cada troncal habilitado de routing_trunks, enviado desde el mismo socket que el tráfico real: público (carrier) para troncales public, LAN para private.',
        formula: 'responde = código < 500 y ≠ 408; online==total→healthy; caídos/total>0.5→critical; resto→degraded; sin datos en 5 min→unknown',
        steps: [
          { step: 1, title: 'Troncales', description: 'Lee los troncales habilitados de routing_trunks (Enrutamiento).' },
          { step: 2, title: 'OPTIONS vía Kamailio', description: '`kamcmd tm.t_uac_wait_block OPTIONS sip:host:puerto` con timeout de 4s. La latencia incluye el sondeo de kamcmd (~100 ms).' },
          { step: 3, title: 'Persistencia', description: 'Actualiza routing_trunks.sip_status, registra en routing_sip_status_log (7 días) y guarda trunks_online/total en monitoring_kamailio_metrics.' },
        ],
        dataSources: ['kamcmd tm.t_uac_wait_block', 'routing_trunks', 'routing_sip_status_log', 'monitoring_kamailio_metrics.trunks_*'],
        frequency: 'Cada 60s', latency: '< 1 min',
      },
    },
    {
      key: 'kamailio.success_rate', title: 'Success Rate', value: (kamailio.success_rate ?? 100).toFixed(1), unit: '%',
      sparklineData: [], color: '#22c55e',
      status: (kamailio.success_rate ?? 100) < 95 ? 'critical' : (kamailio.success_rate ?? 100) < 98 ? 'warning' : 'healthy',
      icon: <CheckCircle className="w-5 h-5" />, subtitle: 'Kamailio · Calidad', component: 'kamailio', metric: 'success_rate',
      reason: {
        explanation: `De cada 100 INVITEs, ${(kamailio.success_rate ?? 100).toFixed(1)} terminan en 2xx. 100% = señalización sana extremo a extremo.`,
        impact: 'high', recommendation: 'Si baja de 98%, revisar 4xx (auth/rutas) y 5xx (carriers/PBX) en acc y Trunk Health.',
        relatedMetrics: ['invite_4xx', 'invite_5xx', 'trunk_health'],
      },
      thresholds: [{ name: 'ASR', metric: 'success_rate', operator: '<', warning: 98, critical: 95, unit: '%', description: 'Answer-Seizure Ratio bajo', enabled: true }],
      calculationMethod: { description: '200 OK / (200+4xx+5xx) de INVITE desde stats de Kamailio en ventana de 5 min.', formula: '100 * invite_200ok / (invite_200ok+invite_4xx+invite_5xx)', dataSources: ['kamcmd stats.get_statistics', 'acc table'], frequency: 'Cada 60s', latency: '< 2 min' },
    },
    {
      key: 'kamailio.shm', title: 'SHM Usage', value: kamailio.shm_usage_percent.toFixed(1), unit: '%',
      color: '#f97316',
      status: kamailio.shm_usage_percent > 90 ? 'critical' : kamailio.shm_usage_percent > 75 ? 'warning' : 'healthy',
      icon: <MemoryStick className="w-5 h-5" />, subtitle: 'Kamailio · Memoria', component: 'kamailio', metric: 'shm_usage_percent',
      reason: {
        explanation: `Memoria compartida de Kamailio al ${kamailio.shm_usage_percent.toFixed(1)}%. Por debajo de 75% es operación normal.`,
        impact: 'medium', recommendation: 'Si supera 90% sostenido, reiniciar planificado o aumentar -m SHM y revisar dialogs/transacciones colgadas.',
        relatedMetrics: ['active_dialogs', 'active_transactions', 'shm_fragments'],
      },
      thresholds: [{ name: 'SHM', metric: 'shm_usage_percent', operator: '>', warning: 75, critical: 90, unit: '%', description: 'Riesgo de OOM en Kamailio', enabled: true }],
      calculationMethod: { description: 'real_used / total desde kamcmd shm.stats.', formula: '100 * real_used / total_size', dataSources: ['kamcmd shm.stats'], frequency: 'Cada 60s', latency: '< 1 min' },
    },
    {
      key: 'mariadb.connections', title: 'DB Connections', value: mariadb.connections_used, unit: `(${mariadb.connection_usage_percent.toFixed(1)}% de ${mariadb.connections_max})`,
      sparklineData: historyData['mariadb.threads_connected']?.data || [], color: '#64748b',
      status: mariadb.connection_usage_percent > 90 ? 'critical' : mariadb.connection_usage_percent > 75 ? 'warning' : 'healthy',
      icon: <Database className="w-5 h-5" />, subtitle: 'MariaDB · Pool', component: 'mariadb', metric: 'threads_connected',
      lastUpdate: mariadb.last_update,
      reason: {
        explanation: `${mariadb.connections_used} conexiones activas de ${mariadb.connections_max} máximas (${mariadb.connection_usage_percent.toFixed(1)}%). Tras ampliar max_connections a 1000 el pool está holgado. El falso 98% anterior era porque se dividía entre Max_used_connections (48) en vez de max_connections.`,
        impact: mariadb.connection_usage_percent > 75 ? 'high' : 'low',
        recommendation: 'Mantener. Si supera 75%, revisar pooling de Kamailio/Apolo y slow queries.',
        relatedMetrics: ['queries_per_second', 'slow_queries_per_minute', 'threads_running'],
      },
      thresholds: [{ name: 'Pool', metric: 'connection_usage_percent', operator: '>', warning: 75, critical: 90, unit: '%', description: 'Pool cerca de agotarse', enabled: true }],
      calculationMethod: { description: 'Threads_connected / max_connections (variable global, no Max_used).', formula: '100 * Threads_connected / @@max_connections', dataSources: ['SHOW GLOBAL STATUS', 'SHOW GLOBAL VARIABLES'], frequency: 'Cada 60s', latency: '< 5s' },
    },
    {
      key: 'mariadb.qps', title: 'Queries/sec', value: mariadb.queries_per_second.toFixed(1), unit: 'qps',
      sparklineData: historyData['mariadb.queries_per_second']?.data || [], color: '#78716c',
      status: 'healthy', icon: <Activity className="w-5 h-5" />, subtitle: 'MariaDB · Throughput', component: 'mariadb', metric: 'queries_per_second',
      reason: { explanation: 'Throughput de consultas SQL/s. QPS alto con 0 slow queries = DB sana.', impact: 'low', relatedMetrics: ['slow_queries_per_minute', 'threads_running'] },
      thresholds: [{ name: 'QPS alto', metric: 'queries_per_second', operator: '>', warning: 5000, critical: 10000, unit: 'qps', description: 'Picos anómalos', enabled: true }],
      calculationMethod: { description: 'Delta de Questions en 60s / 60.', formula: 'ΔQuestions / 60', dataSources: ['SHOW GLOBAL STATUS'], frequency: 'Cada 60s', latency: '< 5s' },
    },
    {
      key: 'rtpengine.sessions', title: 'RTP Sessions', value: rtpengine.active_sessions, unit: 'sess',
      color: '#f97316', status: rtpengine.active_sessions > 0 ? 'healthy' : 'unknown',
      icon: <Volume2 className="w-5 h-5" />, subtitle: 'RTPengine · Media', component: 'rtpengine', metric: 'active_sessions',
      reason: {
        explanation: `Sesiones RTP reales vía protocolo NG (comando list sobre UDP 7722): ${rtpengine.active_sessions} call-ids activos. Correlaciona con ${kamailio.active_calls} diálogos SIP y ${rtpengine.port_usage_percent.toFixed(1)}% de puertos en uso. 0 = sin media (N/A), no fallo.`,
        impact: 'high', recommendation: 'Si Active Calls > 0 pero RTP Sessions = 0, revisar rtpengine_offer/answer en Kamailio y firewall UDP del rango port-min/port-max de rtpengine.',
        relatedMetrics: ['port_usage_percent', 'one_sided_sessions'],
      },
      thresholds: [{ name: 'Sin sesiones', metric: 'active_sessions', operator: '==', warning: 0, critical: 0, unit: 'sess', description: 'Solo informativo cuando hay llamadas sin media', enabled: false }],
      calculationMethod: { description: 'Comando NG `list` a 127.0.0.1:7722 (bencode con cookie): cuenta call-ids. Fallback: statistics.sessionstotal y kamcmd rtpengine.get_hash_total.', formula: 'len(NG list.calls)', dataSources: ['rtpengine NG UDP 7722', 'kamcmd rtpengine.get_hash_total'], frequency: 'Cada 60s', latency: '< 5s' },
    },
    {
      key: 'system.cpu', title: 'CPU', value: system.cpu_percent.toFixed(1), unit: '%',
      sparklineData: historyData['system.cpu_percent']?.data || [], color: '#ef4444',
      status: system.cpu_percent > 85 ? 'critical' : system.cpu_percent > 70 ? 'warning' : 'healthy',
      icon: <Cpu className="w-5 h-5" />, subtitle: 'System', component: 'system', metric: 'cpu_percent',
      reason: { explanation: `CPU del host al ${system.cpu_percent.toFixed(1)}% con load ${system.load_avg_1m}.`, impact: 'medium', relatedMetrics: ['load_avg_1m', 'memory_percent'] },
      thresholds: [{ name: 'CPU', metric: 'cpu_percent', operator: '>', warning: 70, critical: 85, unit: '%', description: 'CPU alta sostenida', enabled: true }],
      calculationMethod: { description: 'psutil.cpu_percent + loadavg.', formula: '100 - idle%', dataSources: ['psutil', '/proc/stat'], frequency: 'Cada 60s', latency: '< 10s' },
    },
  ];

  const kpis = buildKPIData();
  const toggleMetric = (key: string) => setSelectedMetrics(p =>
    p.includes(key) ? p.filter(k => k !== key) : [...p, key]);

  return (
    <div className="p-6 space-y-6">
      <div className="flex flex-col sm:flex-row sm:items-center sm:justify-between gap-4">
        <div>
          <h1 className="text-2xl font-bold text-gray-900 dark:text-white flex items-center gap-2">SBC Carrier Monitoring <span className="text-xs font-normal px-2 py-1 rounded bg-[#ffede3] text-[#d8450b]">Vodafone-grade</span></h1>
          <p className="text-sm text-gray-500 mt-1 flex items-center gap-1"><MousePointerClick className="w-4 h-4" /> Click en cualquier card para ver <b>Reason</b> + método de cálculo • Auto-refresh 30s • {parseUtc(overview.timestamp).toLocaleTimeString('es-PE', { timeZone: LIMA_TZ })} (Lima)</p>
        </div>
        <div className="flex items-center gap-3">
          <TooltipProvider><Tooltip><TooltipTrigger asChild>
            <Select value={timeRange} onValueChange={setTimeRange}>
              <SelectTrigger className="w-[160px]"><SelectValue placeholder="Time Range" /><ChevronDown className="w-4 h-4 opacity-50" /></SelectTrigger>
              <SelectContent>
                <SelectItem value="1h">Last hour (1-min)</SelectItem>
                <SelectItem value="6h">Last 6 hours (1-min)</SelectItem>
                <SelectItem value="24h">Last 24 hours (hourly)</SelectItem>
                <SelectItem value="7d">Last 7 days (hourly)</SelectItem>
                <SelectItem value="30d">Last 30 days (daily)</SelectItem>
              </SelectContent>
            </Select>
          </TooltipTrigger><TooltipContent><p className="text-sm">Rango para gráficas</p></TooltipContent></Tooltip></TooltipProvider>
          <button onClick={() => refetch()} className="p-2 rounded-lg bg-gray-100 text-gray-600 hover:bg-gray-200"><RefreshCw className="w-5 h-5" /></button>
        </div>
      </div>

      {(overview.critical_alerts > 0 || overview.warning_alerts > 0) && (
        <div className={`rounded-xl p-4 border-l-4 ${overview.critical_alerts > 0 ? 'bg-red-50 border-red-500' : 'bg-yellow-50 border-yellow-500'}`}>
          <h3 className="font-semibold">{overview.critical_alerts > 0 ? `${overview.critical_alerts} Critical` : `${overview.warning_alerts} Warning`} Alerts</h3>
          <div className="text-xs font-mono text-gray-600 mt-1">{alerts.slice(0, 3).map(a => <div key={a.id}>[{a.component}] {a.alert_name}: {a.current_value}</div>)}</div>
        </div>
      )}
      {overview.critical_alerts === 0 && overview.warning_alerts === 0 && (
        <div className="rounded-xl p-3 border border-green-200 bg-green-50 text-sm text-green-800 flex items-center gap-2"><CheckCircle className="w-4 h-4" /> 0 alertas disparadas. Los estados “critical” en cards son salud por threshold, no alertas activas.</div>
      )}

      <div className="grid grid-cols-1 sm:grid-cols-2 lg:grid-cols-4 gap-4">
        {kpis.map(k => <KPICard key={k.key} data={{ ...k, sparklineData: historyData[k.key]?.data || k.sparklineData }} />)}
      </div>

      <div className="rounded-xl border bg-white dark:bg-gray-800 p-5">
        <div className="flex flex-col sm:flex-row sm:items-center sm:justify-between gap-3 mb-1">
          <h2 className="font-semibold">Evolución temporal ({timeRange})</h2>
          <div className="flex items-center gap-1 bg-gray-100 dark:bg-gray-700 rounded-lg p-1">
            {['1h', '6h', '24h', '7d', '30d'].map(r => (
              <button
                key={r}
                onClick={() => setTimeRange(r)}
                className={`px-3 py-1 text-xs font-semibold rounded-md transition-colors ${timeRange === r ? 'bg-[#ff5a0a] text-white shadow' : 'text-gray-600 hover:text-gray-900'}`}
              >
                {r}
              </button>
            ))}
          </div>
        </div>
        <p className="text-xs text-gray-500 mb-1">Click en las chips para añadir/quitar métricas. Resolución automática carrier-class: ≤12h raw 1-min · ≤93d hourly (avg) · mayor daily (avg).</p>
        <p className="text-[11px] text-gray-400 mb-4">
          Histórico: raw 30 días · hourly 93 días · daily 400 días.
          {histStatus?.tiers?.raw?.tables?.kamailio?.oldest
            ? ` Datos desde ${fmtDateTime(histStatus.tiers.raw.tables.kamailio.oldest)} · hourly ${histStatus.tiers.hourly.rows} filas · daily ${histStatus.tiers.daily.rows} filas.`
            : ' Cargando cobertura…'}
        </p>
        <div className="flex flex-wrap gap-2 mb-4">
          {kpis.map(k => (
            <button key={k.key} onClick={() => toggleMetric(k.key)}
              className={`text-xs px-2 py-1 rounded-full border ${selectedMetrics.includes(k.key) ? 'bg-[#ff5a0a] text-white border-[#ff5a0a]' : 'bg-white text-gray-600'}`}>
              {k.title}
            </button>
          ))}
        </div>
        <div className="grid grid-cols-1 lg:grid-cols-2 gap-6">
          {selectedMetrics.map(key => {
            const h = historyData[key]?.data || [];
            const color = kpis.find(k => k.key === key)?.color || '#ff5a0a';
            return (
              <div key={key} className="bg-gray-50 rounded-lg p-4">
                <h3 className="text-sm font-medium mb-2">{key} <span className="text-xs font-normal text-gray-400">({h.length} pts)</span></h3>
                {h.length === 0 ? (
                  <div className="h-56 flex items-center justify-center text-sm text-gray-500">Sin datos en este rango (histórico en llenado).</div>
                ) : (
                <div className="h-56">
                  <ResponsiveContainer width="100%" height="100%">
                    <AreaChart data={h} margin={{ top: 10, right: 10, left: 0, bottom: 0 }}>
                      <defs><linearGradient id={`g-${key.replace(/\./g, '-')}`} x1="0" y1="0" x2="0" y2="1"><stop offset="5%" stopColor={color} stopOpacity={0.3} /><stop offset="95%" stopColor={color} stopOpacity={0} /></linearGradient></defs>
                      <CartesianGrid strokeDasharray="3 3" stroke="#e5e7eb" />
                      <XAxis dataKey="timestamp" tickFormatter={fmtTick} fontSize={11} />
                      <YAxis fontSize={11} />
                      <RechartsTooltip contentStyle={{ backgroundColor: '#fff', border: '1px solid #e5e7eb', borderRadius: 8 }} labelFormatter={(t) => fmtDateTime(t)} />
                      <Area type="monotone" dataKey="value" stroke={color} strokeWidth={2} fill={`url(#g-${key.replace(/\./g, '-')})`} />
                    </AreaChart>
                  </ResponsiveContainer>
                </div>
                )}
              </div>
            );
          })}
          {selectedMetrics.length === 0 && <div className="text-sm text-gray-500">Selecciona una métrica arriba.</div>}
        </div>
      </div>

      <div className="rounded-xl border bg-white dark:bg-gray-800 overflow-hidden">
        <Tabs defaultValue="kamailio" className="w-full">
          <TabsList className="grid w-full grid-cols-5 bg-gray-50 p-1">
            <TabsTrigger value="kamailio" className="flex items-center justify-center gap-2 py-2 text-sm font-medium"><Phone className="w-4 h-4" />Kamailio</TabsTrigger>
            <TabsTrigger value="rtpengine" className="flex items-center justify-center gap-2 py-2 text-sm font-medium"><Volume2 className="w-5 h-5" />RTPengine</TabsTrigger>
            <TabsTrigger value="mariadb" className="flex items-center justify-center gap-2 py-2 text-sm font-medium"><Database className="w-4 h-4" />MariaDB</TabsTrigger>
            <TabsTrigger value="system" className="flex items-center justify-center gap-2 py-2 text-sm font-medium"><Server className="w-4 h-4" />System</TabsTrigger>
            <TabsTrigger value="alerts" className="flex items-center justify-center gap-2 py-2 text-sm font-medium"><AlertTriangle className="w-4 h-4" />Alerts ({overview.active_alerts})</TabsTrigger>
          </TabsList>
          <TabsContent value="kamailio" className="p-5">
            <p className="text-xs text-gray-500 mb-4">Desglose SIP (los KPIs ejecutivos están arriba — aquí solo detalle complementario, sin repetir cards).</p>
            <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 gap-4">
              <DetailTable title="Diálogos y transacciones" subtitle="Kamailio dialog + TM" rows={[
                ['Diálogos activos', kamDetail?.active_dialogs ?? '—'],
                ['Confirmados', kamDetail?.confirmed_dialogs ?? '—'],
                ['Early', kamDetail?.early_dialogs ?? '—'],
                ['Transacciones activas', kamailio.active_transactions],
              ]} />
              <DetailTable title="Métodos SIP (acumulado)" subtitle="Desde último reinicio Kamailio" rows={[
                ['INVITE rx', kamDetail?.invite_received ?? '—'],
                ['200 OK a INVITE', kamDetail?.invite_200ok ?? '—'],
                ['4xx a INVITE', kamDetail?.invite_4xx ?? '—'],
                ['5xx a INVITE', kamDetail?.invite_5xx ?? '—'],
                ['BYE / CANCEL / ACK', `${kamDetail?.bye_received ?? '—'} / ${kamDetail?.cancel_received ?? '—'} / ${kamDetail?.ack_received ?? '—'}`],
                ['OPTIONS / REGISTER', `${kamDetail?.options_received ?? '—'} / ${kamDetail?.register_received ?? '—'}`],
              ]} />
              <DetailTable title="Troncales" subtitle="SIP OPTIONS vía Kamailio · cada 60s" rows={[
                ...trunks.map((t): [string, React.ReactNode] => [
                  t.name,
                  <span className={t.sip_status === 'reachable' ? 'text-green-600' : 'text-red-600'}>
                    {t.response_code ?? '—'}{t.latency_ms != null ? ` · ${t.latency_ms} ms` : ''}
                  </span>,
                ]),
                ['Salud', trunkHealth],
              ]} />
              <DetailTable title="Memoria SHM" subtitle="kamcmd shm.stats" rows={[
                ['Uso', `${kamailio.shm_usage_percent.toFixed(1)}%`],
                ['En uso', `${kamDetail?.shm_used_mb?.toFixed?.(0) ?? '—'} MB`],
                ['Libre', `${kamDetail?.shm_free_mb?.toFixed?.(0) ?? '—'} MB`],
                ['Fragmentos', kamDetail?.shm_fragments ?? '—'],
              ]} />
              <DetailTable title="Errores" subtitle="Script / parse / DB" rows={[
                ['Parse errors', kamDetail?.parse_errors ?? '—'],
                ['Script errors', kamDetail?.script_errors ?? '—'],
                ['DB errors', kamDetail?.db_errors ?? '—'],
                ['Timeouts tx', kamDetail?.transaction_timeouts ?? '—'],
              ]} />
            </div>
          </TabsContent>
          <TabsContent value="rtpengine" className="p-5">
            <p className="text-xs text-gray-500 mb-4">Desglose media vía protocolo NG (UDP 7722: comandos <span className="font-mono">list</span>, <span className="font-mono">statistics</span>, <span className="font-mono">query</span>). Sin MOS, jitter, RTT ni pérdida: en modo kernel rtpengine reenvía el RTCP sin analizarlo.</p>
            <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 gap-4">
              <DetailTable title="Sesiones y puertos" subtitle="NG list + statistics" rows={[
                ['Sesiones activas (NG list)', rtpengine.active_sessions],
                ['One-sided (estim.)', rtpDetail?.one_sided_sessions ?? '—'],
                ['Puertos usados', rtpDetail?.ports_used ?? '—'],
                ['Puertos libres', rtpDetail?.ports_free ?? '—'],
                ['Uso puertos', `${rtpengine.port_usage_percent.toFixed(2)}%`],
              ]} />
              <DetailTable title="Tráfico acumulado" subtitle="Contadores desde arranque rtpengine" rows={[
                ['Paquetes relayed', (rtpDetail?.packets_in ?? 0).toLocaleString()],
                ['Bytes relayed', (rtpDetail?.bytes_in ?? 0).toLocaleString()],
                ['Throughput instant.', `${rtpengine.media_throughput_mbps.toFixed(2)} Mbps`],
                ['Timeouts sesión', rtpDetail?.timeout_errors ?? '—'],
              ]} />
              <DetailTable title="Señalización NG" subtitle="Contadores controlstatistics" rows={[
                ['Offers', rtpDetail?.raw_data?.ng_control?.offercount ?? '—'],
                ['Answers', rtpDetail?.raw_data?.ng_control?.answercount ?? '—'],
                ['Deletes', rtpDetail?.raw_data?.ng_control?.deletecount ?? '—'],
                ['CPU / Mem proc.', `${rtpengine.cpu_percent.toFixed(1)}% / ${rtpengine.memory_mb.toFixed(0)} MB`],
              ]} />
            </div>
          </TabsContent>
          <TabsContent value="mariadb" className="p-5">
            <p className="text-xs text-gray-500 mb-4">Desglose InnoDB y conexiones (los KPIs ejecutivos están arriba).</p>
            <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 gap-4">
              <DetailTable title="Conexiones" subtitle="SHOW GLOBAL STATUS" rows={[
                ['Conectados', dbDetail?.threads_connected ?? mariadb.connections_used],
                ['En ejecución', dbDetail?.threads_running ?? '—'],
                ['Máx. configurado', mariadb.connections_max],
                ['Abortados', dbDetail?.aborted_connects ?? '—'],
              ]} />
              <DetailTable title="Rendimiento" subtitle="InnoDB + queries" rows={[
                ['Slow queries (desde arranque)', dbDetail?.slow_queries ?? '—'],
                ['Deadlocks (desde arranque)', dbDetail?.innodb_deadlocks ?? '—'],
                ['Buffer pool usado', `${dbDetail?.innodb_buffer_pool_used_mb?.toFixed?.(0) ?? '—'} MB`],
                ['Buffer pool libre', `${dbDetail?.innodb_buffer_pool_free_mb?.toFixed?.(0) ?? '—'} MB`],
                ['Páginas dirty', dbDetail?.innodb_buffer_pool_pages_dirty ?? '—'],
              ]} />
              <DetailTable title="Temp tables" subtitle="Síntoma de queries pesadas" rows={[
                ['Tmp en disco', dbDetail?.created_tmp_disk_tables ?? '—'],
                ['Tmp total', dbDetail?.created_tmp_tables ?? '—'],
              ]} />
            </div>
          </TabsContent>
          <TabsContent value="system" className="p-5">
            <p className="text-xs text-gray-500 mb-4">Desglose host y procesos (CPU/Mem ejecutivos arriba).</p>
            <div className="grid grid-cols-1 md:grid-cols-2 lg:grid-cols-3 gap-4">
              <DetailTable title="Carga y disco" subtitle="psutil" rows={[
                ['Load 1/5/15m', `${sysDetail?.load_avg_1m?.toFixed?.(2) ?? '—'} / ${sysDetail?.load_avg_5m?.toFixed?.(2) ?? '—'} / ${sysDetail?.load_avg_15m?.toFixed?.(2) ?? '—'}`],
                ['Disco', `${system.disk_percent.toFixed(1)}%`],
                ['Red', `${system.net_throughput_mbps.toFixed(1)} Mbps (${sysDetail?.net_interface ?? '—'})`],
                ['Drops/errores', `${sysDetail?.net_drops ?? '—'} / ${sysDetail?.net_errors ?? '—'}`],
              ]} />
              <DetailTable title="Procesos" subtitle="CPU / RSS por servicio" rows={[
                ['Kamailio', `${sysDetail?.kamailio_cpu_percent?.toFixed?.(1) ?? '—'}% / ${sysDetail?.kamailio_memory_mb?.toFixed?.(0) ?? '—'} MB`],
                ['RTPengine', `${sysDetail?.rtpengine_cpu_percent?.toFixed?.(1) ?? '—'}% / ${sysDetail?.rtpengine_memory_mb?.toFixed?.(0) ?? '—'} MB`],
                ['FreeSWITCH', `${sysDetail?.freeswitch_cpu_percent?.toFixed?.(1) ?? '—'}% / ${sysDetail?.freeswitch_memory_mb?.toFixed?.(0) ?? '—'} MB`],
                ['PostgreSQL', `${sysDetail?.postgres_cpu_percent?.toFixed?.(1) ?? '—'}% / ${sysDetail?.postgres_memory_mb?.toFixed?.(0) ?? '—'} MB`],
              ]} />
            </div>
          </TabsContent>
          <TabsContent value="alerts" className="p-5">
            {alerts.length === 0 ? (
              <div className="text-center py-12"><CheckCircle className="w-12 h-12 text-green-500 mx-auto mb-4" /><h3 className="text-lg font-medium">No Active Alerts</h3><p className="text-gray-500 mt-1">All systems operating normally</p></div>
            ) : (
              <div className="space-y-3">{alerts.map(a => (
                <div key={a.id} className="flex gap-3 p-4 rounded-lg border bg-red-50 border-red-200">
                  <div className="w-2 h-2 rounded-full mt-2 bg-red-500" />
                  <div><div className="flex gap-2 items-center"><h4 className="font-medium">{a.alert_name}</h4><StatusBadge status={a.severity} label={a.severity} /></div>
                  <p className="text-sm font-mono">[{a.component}] {a.metric_name} {a.operator} {a.threshold_value} (current: {a.current_value})</p>
                  <p className="text-xs text-gray-500">{fmtDateTime(a.timestamp)}</p></div>
                </div>))}
              </div>
            )}
          </TabsContent>
        </Tabs>
      </div>
    </div>
  );
};

export default MonitoringDashboard;
