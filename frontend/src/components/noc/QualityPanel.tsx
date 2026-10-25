import type { NocCapacity, NocQuality } from '../../types'
import type { StatusLevel } from './format'
import { fmtInt } from './format'

const STATUS_COLOR: Record<StatusLevel, string> = {
  good: 'var(--viz-status-good)',
  warning: 'var(--viz-status-warning)',
  critical: 'var(--viz-status-critical)',
}

const STATUS_LABEL: Record<StatusLevel, string> = { good: 'OK', warning: 'Vigilar', critical: 'Crítico' }

/** Level for a value where lower is better (jitter, loss) or higher is better (MOS) */
const level = (v: number | null, good: number, warn: number, higherIsBetter = false): StatusLevel | null => {
  if (v == null) return null
  if (higherIsBetter) return v >= good ? 'good' : v >= warn ? 'warning' : 'critical'
  return v <= good ? 'good' : v <= warn ? 'warning' : 'critical'
}

function Metric({
  label,
  value,
  unit,
  status,
  hint,
}: {
  label: string
  value: string
  unit?: string
  status?: StatusLevel | null
  hint: string
}) {
  return (
    <div className="min-w-0" title={hint}>
      <div className="flex items-center justify-between gap-2">
        <span className="text-xs text-[var(--color-text-tertiary)] truncate">{label}</span>
        {status && (
          <span className="flex items-center gap-1 text-[11px] text-[var(--color-text-secondary)]">
            <span className="w-2 h-2 rounded-full" style={{ background: STATUS_COLOR[status] }} aria-hidden="true" />
            {STATUS_LABEL[status]}
          </span>
        )}
      </div>
      <div className="text-xl font-semibold text-[var(--color-text-primary)] tabular-nums mt-0.5">
        {value}
        {unit && <span className="text-xs font-medium text-[var(--color-text-tertiary)] ml-1">{unit}</span>}
      </div>
    </div>
  )
}

/** Media quality (RTPEngine) and signalling headroom (CPS) */
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

  return (
    <div className="bg-[var(--color-bg-card)] rounded-xl border border-[var(--color-border-primary)] p-5">
      <div className="flex flex-wrap items-baseline justify-between gap-2">
        <h2 className="text-base font-semibold text-[var(--color-text-primary)]">Calidad de voz y capacidad</h2>
        <span className="text-xs text-[var(--color-text-tertiary)]">{source}</span>
      </div>

      <div className="grid grid-cols-2 sm:grid-cols-3 lg:grid-cols-6 gap-4 mt-4">
        <Metric
          label="MOS"
          value={quality.mos == null ? '—' : quality.mos.toFixed(2)}
          status={level(quality.mos, 4.0, 3.6, true)}
          hint="Mean Opinion Score estimado por RTPEngine (1–5). ≥4,0 bueno; <3,6 los usuarios lo notan."
        />
        <Metric
          label="Jitter"
          value={quality.jitter_ms == null ? '—' : quality.jitter_ms.toFixed(0)}
          unit="ms"
          status={level(quality.jitter_ms, 30, 50)}
          hint="Variación del retardo de los paquetes RTP. ≤30 ms bueno; >50 ms causa cortes."
        />
        <Metric
          label="Pérdida de paquetes"
          value={quality.packet_loss_pct == null ? '—' : `${quality.packet_loss_pct.toFixed(2)}%`}
          status={level(quality.packet_loss_pct, 1, 3)}
          hint="Paquetes RTP perdidos / retransmitidos por RTPEngine durante la hora. ≤1% bueno; >3% audio entrecortado."
        />
        <Metric
          label="Audio en un sentido"
          value={oneSidedPct == null ? '—' : `${oneSidedPct.toFixed(1)}%`}
          unit={quality.one_sided_sessions == null ? undefined : `${fmtInt(quality.one_sided_sessions)} ses.`}
          status={level(oneSidedPct, 2, 5)}
          hint="Llamadas con más de 90 s (ya pasado el timbrado) a las que les falta media en uno o ambos sentidos: audio unidireccional, media muerta o llamada en espera. Se revisan todas las sesiones cada minuto. ≤2% OK; >5% crítico (revisar NAT, SDP o RTPEngine)."
        />
        <Metric
          label="CPS pico (última hora)"
          value={fmtInt(capacity.peak_cps_last_hour)}
          unit={capacity.max_cps ? `de ${fmtInt(capacity.max_cps)}` : undefined}
          status={cpsUse == null ? null : cpsUse >= 95 ? 'critical' : cpsUse >= 80 ? 'warning' : 'good'}
          hint={`Máximo de llamadas iniciadas en un mismo segundo, según los CDR. Promedio: ${capacity.avg_cps_last_hour.toFixed(2)} CPS.${
            capacity.max_cps ? '' : ' Define el límite del SBC/troncal en system_settings.noc_max_cps.'
          }`}
        />
        <Metric
          label="PDD (mediana)"
          value={quality.pdd_p50_ms == null ? '—' : (quality.pdd_p50_ms / 1000).toFixed(1)}
          unit={quality.pdd_p50_ms == null ? undefined : `s · p95 ${((quality.pdd_p95_ms ?? 0) / 1000).toFixed(1)} s`}
          status={level(quality.pdd_p95_ms == null ? null : quality.pdd_p95_ms / 1000, 4, 8)}
          hint={`Post-Dial Delay: del INVITE que envía este SBC al primer 180/183 del siguiente salto, en las llamadas de la última hora vía FreeSWITCH (${fmtInt(quality.pdd_samples)} muestras). FreeSWITCH Legado responde 180/183 con su propio ringback en ~20 ms, así que este valor no incluye el tiempo hasta que suena el terminal detrás de Legado. Estado sobre el p95: ≤4 s OK, >8 s crítico. Las llamadas directas Kamailio → troncal no traen este dato.`}
        />
      </div>
      <p className="text-[11px] text-[var(--color-text-tertiary)] mt-4">
        CPS promedio última hora: <span className="tabular-nums">{capacity.avg_cps_last_hour.toFixed(2)}</span>
        {!capacity.max_cps && ' · sin límite de CPS configurado'} · pasa el cursor sobre cada métrica para ver los umbrales.
      </p>
    </div>
  )
}
