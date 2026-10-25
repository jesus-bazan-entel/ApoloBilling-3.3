import type { NocCapacity, NocQuality } from '../../types'
import type { StatusLevel } from './format'
import { fmtInt } from './format'

const STATUS_COLOR: Record<StatusLevel, string> = {
  good: 'var(--viz-status-good)',
  warning: 'var(--viz-status-warning)',
  critical: 'var(--viz-status-critical)',
}

const STATUS_LABEL: Record<StatusLevel, string> = { good: 'OK', warning: 'Vigilar', critical: 'Crítico' }

/** Level for a value where lower is better */
const level = (v: number | null, good: number, warn: number): StatusLevel | null => {
  if (v == null) return null
  return v <= good ? 'good' : v <= warn ? 'warning' : 'critical'
}

function Metric({
  label,
  value,
  unit,
  status,
  noData,
  sample,
  hint,
}: {
  label: string
  value: string
  unit?: string
  status?: StatusLevel | null
  /** No measurement behind the value: never show a status colour */
  noData?: boolean
  /** How many samples the value comes from ("n=312") */
  sample?: string
  hint: string
}) {
  return (
    <div className="min-w-0" title={hint}>
      <div className="flex items-center justify-between gap-2">
        <span className="text-xs text-[var(--color-text-tertiary)] truncate">{label}</span>
        {noData ? (
          <span className="flex items-center gap-1 text-[11px] text-[var(--color-text-tertiary)]">
            <span className="w-2 h-2 rounded-full border border-[var(--color-text-tertiary)]" aria-hidden="true" />
            Sin datos
          </span>
        ) : status && (
          <span className="flex items-center gap-1 text-[11px] text-[var(--color-text-secondary)]">
            <span className="w-2 h-2 rounded-full" style={{ background: STATUS_COLOR[status] }} aria-hidden="true" />
            {STATUS_LABEL[status]}
          </span>
        )}
      </div>
      <div
        className={`text-xl font-semibold tabular-nums mt-0.5 ${noData ? 'text-[var(--color-text-tertiary)]' : 'text-[var(--color-text-primary)]'}`}
      >
        {value}
        {unit && <span className="text-xs font-medium text-[var(--color-text-tertiary)] ml-1">{unit}</span>}
      </div>
      {sample && <div className="text-[11px] text-[var(--color-text-tertiary)] tabular-nums mt-0.5">{sample}</div>}
    </div>
  )
}

/**
 * Media-path health (RTPEngine packet counters) and signalling headroom (CPS).
 * MOS, jitter, loss and PDD were removed: none of them measured anything real
 * here (RTPEngine kernel mode never parses RTCP, FreeSWITCH's MOS sits at its
 * 4.5 ceiling, its skip count is not network loss, the next hop answers 18x
 * within one 20 ms clock tick).
 */
const hm = (iso: string) => new Date(iso).toLocaleTimeString('es-PE', { hour: '2-digit', minute: '2-digit', hour12: false })

export default function QualityPanel({ quality, capacity }: { quality: NocQuality; capacity: NocCapacity }) {
  const source = !quality.as_of
    ? 'Sin datos del colector de RTPEngine'
    : quality.window_minutes === 15
      ? `Media: RTPEngine, últimos 15 min (hasta las ${hm(quality.as_of)})`
      : `Media: RTPEngine, hora ${quality.window_start ? hm(quality.window_start) : ''}–${hm(quality.as_of)} (sin muestras recientes)`
  const oneSidedPct =
    quality.one_sided_sessions != null && quality.active_sessions
      ? (100 * quality.one_sided_sessions) / quality.active_sessions
      : null
  const cpsUse = capacity.max_cps ? (100 * capacity.peak_cps_last_hour) / capacity.max_cps : null
  const noDirection = !quality.sessions_checked

  return (
    <div className="bg-[var(--color-bg-card)] rounded-xl border border-[var(--color-border-primary)] p-5">
      <div className="flex flex-wrap items-baseline justify-between gap-2">
        <h2 className="text-base font-semibold text-[var(--color-text-primary)]">Media y capacidad</h2>
        <span className="text-xs text-[var(--color-text-tertiary)]">{source}</span>
      </div>

      <div className="grid grid-cols-1 sm:grid-cols-2 gap-4 mt-4">
        <Metric
          label="Audio en un sentido"
          value={oneSidedPct == null ? '—' : `${oneSidedPct.toFixed(1)}%`}
          unit={quality.one_sided_sessions == null || noDirection ? undefined : `${fmtInt(quality.one_sided_sessions)} ses.`}
          noData={noDirection}
          status={level(oneSidedPct, 2, 5)}
          sample={quality.sessions_checked == null ? undefined : `n=${fmtInt(quality.sessions_checked)} sesiones revisadas`}
          hint="Llamadas con más de 90 s (ya pasado el timbrado) a las que les falta media en uno o ambos sentidos: audio unidireccional, media muerta o llamada en espera. El colector revisa los contadores de paquetes de cada sesión de RTPEngine cada minuto (n = sesiones revisadas en la última muestra). ≤2% OK; >5% crítico (revisar NAT, SDP o RTPEngine)."
        />
        <Metric
          label="CPS pico (última hora)"
          value={fmtInt(capacity.peak_cps_last_hour)}
          unit={capacity.max_cps ? `de ${fmtInt(capacity.max_cps)}` : undefined}
          status={cpsUse == null ? null : cpsUse >= 95 ? 'critical' : cpsUse >= 80 ? 'warning' : 'good'}
          sample={capacity.max_cps ? undefined : 'sin límite configurado'}
          hint={`Máximo de llamadas iniciadas en un mismo segundo, según los CDR. Promedio: ${capacity.avg_cps_last_hour.toFixed(2)} CPS.${
            capacity.max_cps ? '' : ' Define el límite del SBC/troncal en system_settings.noc_max_cps.'
          }`}
        />
      </div>
      <p className="text-[11px] text-[var(--color-text-tertiary)] mt-4">
        CPS promedio última hora: <span className="tabular-nums">{capacity.avg_cps_last_hour.toFixed(2)}</span> · «n» es la
        cantidad de muestras detrás de cada valor; «Sin datos» significa que no hubo ninguna en la ventana · pasa el cursor sobre
        cada métrica para ver los umbrales.
      </p>
    </div>
  )
}
