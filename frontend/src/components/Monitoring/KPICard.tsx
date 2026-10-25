import React, { useId, useState } from 'react';
import {
  CheckCircle, AlertTriangle, XCircle, Info,
  TrendingUp, TrendingDown, ChevronDown, ChevronUp,
  Calculator, Database, Bell
} from 'lucide-react';
import type { KPICardData, KPIStatus } from '../../types/monitoring';

interface KPICardProps {
  data: KPICardData;
  compact?: boolean;
}

// Backend sends naive-UTC ISO strings with no "Z"/offset; JS would otherwise parse them
// as local time and shift the instant by the viewer's UTC offset. See MonitoringDashboard.tsx.
const parseUtc = (t: string | number | Date): Date => {
  if (typeof t === 'string' && !/Z$|[+-]\d{2}:?\d{2}$/.test(t)) {
    return new Date(t + 'Z');
  }
  return new Date(t);
};

const statusConfig: Record<KPIStatus, { label: string; classes: string; dot: string }> = {
  healthy: { label: 'Healthy', classes: 'bg-green-100 text-green-800 border-green-200', dot: 'bg-green-500' },
  degraded: { label: 'Degraded', classes: 'bg-orange-100 text-orange-800 border-orange-200', dot: 'bg-orange-500' },
  warning: { label: 'Warning', classes: 'bg-yellow-100 text-yellow-800 border-yellow-200', dot: 'bg-yellow-500' },
  critical: { label: 'Critical', classes: 'bg-red-100 text-red-800 border-red-200', dot: 'bg-red-500' },
  unknown: { label: 'N/A', classes: 'bg-gray-100 text-gray-600 border-gray-200', dot: 'bg-gray-400' },
};

function StatusBadge({ status }: { status: KPIStatus }) {
  const cfg = statusConfig[status] || statusConfig.unknown;
  const Icon = status === 'healthy' ? CheckCircle : status === 'critical' ? XCircle : status === 'unknown' ? Info : AlertTriangle;
  return (
    <span className={`inline-flex items-center gap-1 px-2 py-0.5 rounded-full text-xs font-semibold border ${cfg.classes}`}>
      <Icon className="w-3 h-3" />
      {cfg.label}
    </span>
  );
}

function Sparkline({ data, color }: { data: Array<{ timestamp: string; value: number }>; color: string }) {
  // useId's punctuation isn't valid inside url(#…)
  const gid = `sg-${useId().replace(/[^a-zA-Z0-9_-]/g, '')}`;
  if (!data || data.length < 2) return null;
  const values = data.map(d => d.value);
  const min = Math.min(...values);
  const max = Math.max(...values);
  const range = max - min || 1;
  const W = 120;
  const H = 32;
  const points = data.map((d, i) => {
    const x = (i / (data.length - 1)) * W;
    const y = H - 4 - ((d.value - min) / range) * (H - 8);
    return `${x.toFixed(1)},${y.toFixed(1)}`;
  }).join(' ');
  const area = `0,${H} ${points} ${W},${H}`;
  return (
    <svg width={W} height={H} className="overflow-visible">
      <defs>
        <linearGradient id={gid} x1="0" y1="0" x2="0" y2="1">
          <stop offset="0%" stopColor={color} stopOpacity={0.35} />
          <stop offset="100%" stopColor={color} stopOpacity={0} />
        </linearGradient>
      </defs>
      <polygon points={area} fill={`url(#${gid})`} />
      <polyline points={points} fill="none" stroke={color} strokeWidth={2} strokeLinecap="round" strokeLinejoin="round" />
    </svg>
  );
}

export const KPICard: React.FC<KPICardProps> = ({ data, compact = false }) => {
  const [expanded, setExpanded] = useState(false);
  const st = statusConfig[data.status] || statusConfig.unknown;

  return (
    <div
      role="button"
      tabIndex={0}
      onClick={() => setExpanded(v => !v)}
      onKeyDown={(e) => { if (e.key === 'Enter' || e.key === ' ') { e.preventDefault(); setExpanded(v => !v); } }}
      className="relative bg-[var(--color-bg-card)] rounded-[20px] border border-[var(--color-border-primary)] shadow-sm hover:shadow-lg transition-all hover:-translate-y-1 cursor-pointer overflow-hidden outline-none focus:ring-2 focus:ring-[#ff5a0a]"
      style={{ borderLeft: `4px solid ${data.color}` }}
      title={`Click para ver Reason y método de cálculo de ${data.title}`}
    >
      <div className="absolute top-0 left-0 right-0 h-[3px]" style={{ backgroundColor: data.color, opacity: 0.85 }} />
      <div className={compact ? 'p-4' : 'p-5'}>
        <div className="flex justify-between items-start mb-2">
          <div>
            <div className="text-[11px] font-semibold uppercase tracking-wider text-gray-500">{data.subtitle}</div>
            <div className="font-semibold text-gray-900 dark:text-white">{data.title}</div>
          </div>
          <div className="flex items-center gap-2">
            <span className="w-8 h-8 rounded-full flex items-center justify-center" style={{ backgroundColor: `${data.color}1a`, color: data.color }}>
              {data.icon}
            </span>
            <StatusBadge status={data.status} />
          </div>
        </div>

        <div className="flex items-baseline gap-1 mb-2">
          <span className={compact ? 'text-2xl font-bold' : 'text-3xl font-bold'}>{typeof data.value === 'number' ? data.value.toLocaleString() : data.value}</span>
          {data.unit && <span className="text-sm text-gray-500">{data.unit}</span>}
        </div>

        <div className="flex items-center gap-3 mb-1 min-h-[34px]">
          {data.sparklineData && data.sparklineData.length > 1
            ? <Sparkline data={data.sparklineData} color={data.color} />
            : <span className="text-xs text-gray-400">sin histórico</span>}
          {data.trend !== undefined && data.trend !== 0 && (
            <span className={`flex items-center gap-1 text-xs font-semibold ${data.trend >= 0 ? 'text-green-600' : 'text-red-600'}`}>
              {data.trend >= 0 ? <TrendingUp className="w-3 h-3" /> : <TrendingDown className="w-3 h-3" />}
              {Math.abs(data.trend).toFixed(1)}%
            </span>
          )}
        </div>

        {/* Reason resumido siempre visible estilo Vodafone */}
        {data.reason && (
          <div className="mt-2 text-xs leading-relaxed rounded-lg p-2 border" style={{ backgroundColor: `${st.dot === 'bg-red-500' ? '#fef2f2' : st.dot === 'bg-yellow-500' || st.dot === 'bg-orange-500' ? '#fffbeb' : st.dot === 'bg-gray-400' ? '#f9fafb' : '#f0fdf4'}`, borderColor: '#e5e7eb' }}>
            <span className="font-semibold">Reason: </span>
            <span className="text-gray-700 dark:text-gray-300">{data.reason.explanation}</span>
          </div>
        )}

        <button
          className="mt-2 w-full flex items-center justify-center gap-1 text-xs font-medium text-[#ff5a0a] hover:text-[#d8450b] py-1"
          onClick={(e) => { e.stopPropagation(); setExpanded(v => !v); }}
        >
          {expanded ? <><ChevronUp className="w-3 h-3" /> Ocultar detalle</> : <><ChevronDown className="w-3 h-3" /> Ver Reason + cálculo</>}
        </button>

        {expanded && (
          <div className="mt-2 pt-3 border-t border-gray-200 space-y-3" onClick={(e) => e.stopPropagation()}>
            {data.reason?.recommendation && (
              <div className="rounded-lg p-3 bg-green-50 border border-green-200 text-xs">
                <div className="font-semibold text-green-800 flex items-center gap-1 mb-1"><Bell className="w-3 h-3" /> Acción recomendada</div>
                <div className="text-gray-700">{data.reason.recommendation}</div>
                {data.reason.relatedMetrics && (
                  <div className="mt-1 text-gray-500">Métricas relacionadas: {data.reason.relatedMetrics.join(', ')}</div>
                )}
              </div>
            )}
            {data.thresholds && data.thresholds.length > 0 && (
              <div>
                <div className="text-[11px] font-bold uppercase tracking-wider text-gray-500 mb-1">Thresholds</div>
                {data.thresholds.filter(t => t.enabled).map((t, i) => (
                  <div key={i} className="text-xs flex justify-between py-1 border-b border-gray-100 last:border-0">
                    <span className="font-medium">{t.name}</span>
                    <span className="font-mono text-gray-600">
                      {t.warning !== undefined ? `warn ${t.operator} ${t.warning}${t.unit}` : ''}
                      {t.warning !== undefined && t.critical !== undefined ? ' | ' : ''}
                      {t.critical !== undefined ? `crit ${t.operator} ${t.critical}${t.unit}` : ''}
                    </span>
                  </div>
                ))}
                <div className="text-[11px] text-gray-500 mt-1">{data.thresholds[0]?.description}</div>
              </div>
            )}
            {data.calculationMethod && (
              <div className="rounded-lg p-3 bg-slate-50 border border-slate-200">
                <div className="text-[11px] font-bold uppercase tracking-wider text-gray-500 mb-1 flex items-center gap-1">
                  <Calculator className="w-3 h-3" /> Método de cálculo
                </div>
                <div className="text-xs text-gray-700 leading-relaxed">{data.calculationMethod.description}</div>
                {data.calculationMethod.formula && (
                  <div className="mt-2 font-mono text-[11px] bg-white border rounded p-2 overflow-auto">
                    <span className="font-bold">Fórmula: </span>{data.calculationMethod.formula}
                  </div>
                )}
                {data.calculationMethod.steps && (
                  <ol className="mt-2 space-y-1">
                    {data.calculationMethod.steps.map(s => (
                      <li key={s.step} className="text-xs flex gap-2">
                        <span className="w-5 h-5 rounded-full bg-[#ff5a0a] text-white text-[10px] font-bold flex items-center justify-center shrink-0">{s.step}</span>
                        <span><b>{s.title}:</b> {s.description}</span>
                      </li>
                    ))}
                  </ol>
                )}
                <div className="mt-2 flex flex-wrap gap-1">
                  <span className="text-[10px] px-2 py-0.5 rounded border bg-white flex items-center gap-1"><Database className="w-3 h-3" />{data.calculationMethod.dataSources?.join(', ')}</span>
                  <span className="text-[10px] px-2 py-0.5 rounded border bg-white">Freq: {data.calculationMethod.frequency}</span>
                  <span className="text-[10px] px-2 py-0.5 rounded border bg-white">Lat: {data.calculationMethod.latency}</span>
                </div>
              </div>
            )}
            {data.lastUpdate && <div className="text-[10px] text-gray-400">Actualizado: {parseUtc(data.lastUpdate).toLocaleTimeString('es-PE', { timeZone: 'America/Lima' })}</div>}
          </div>
        )}
      </div>
    </div>
  );
};

export default KPICard;
