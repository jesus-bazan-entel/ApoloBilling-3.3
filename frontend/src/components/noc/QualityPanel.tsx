import type { NocCapacity, NocQuality } from '../../types'
import type { StatusLevel } from './format'
import { fmtInt } from './format'

const STATUS_COLOR: Record<StatusLevel, string> = {
  good: 'var(--viz-status-good)',
  warning: 'var(--viz-status-warning)',
  critical: 'var(--viz-status-critical)',
}

const STATUS_LABEL: Record<StatusLevel, string> = { good: 'OK', warning: 'Vigilar', critical: 'Crítico' }

/** FreeSWITCH (without timerfd) stamps channel times on its 20 ms tick */
const FS_CLOCK_MS = 20

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

/** Media quality (RTPEngine) and signalling headroom (CPS) */
const hm = (iso: string) => new Date(iso).toLocaleTimeString('es-PE', { hour: '2-digit', minute: '2-digit', hour12: false })

export default function QualityPanel({ quality, capacity }: { quality: NocQuality; capacity: NocCapacity }) {
  const fs = quality.media_source === 'freeswitch'
  const source = fs
    ? 'MOS y pérdida: FreeSWITCH, llamadas terminadas en los últimos 15 min · jitter y audio en un sentido: RTPEngine'
    : !quality.as_of
      ? 'Sin datos del colector de RTPEngine'
      : quality.window_minutes === 15
        ? `Media: RTPEngine, últimos 15 min (hasta las ${hm(quality.as_of)})`
        : `Media: RTPEngine, hora ${quality.window_start ? hm(quality.window_start) : ''}–${hm(quality.as_of)} (sin muestras recientes)`
  const unit = fs ? 'llamadas' : 'muestras RTCP'
  const oneSidedPct =
    quality.one_sided_sessions != null && quality.active_sessions
      ? (100 * quality.one_sided_sessions) / quality.active_sessions
      : null
  const cpsUse = capacity.max_cps ? (100 * capacity.peak_cps_last_hour) / capacity.max_cps : null
  const noDirection = !quality.sessions_checked
  const noRtcp =
    quality.mos_samples === 0
      ? ' Sin datos: RTPEngine reenvía el RTCP en el módulo kernel y no lo analiza (su contador no avanza), y aún no hay llamadas con medición de FreeSWITCH en la ventana.'
      : ''
  const pddText = (ms: number) => (ms < FS_CLOCK_MS ? `< ${FS_CLOCK_MS} ms` : ms < 1000 ? `${fmtInt(ms)} ms` : `${(ms / 1000).toFixed(1)} s`)

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
          noData={quality.mos == null}
          status={level(quality.mos, 4.0, 3.6, true)}
          sample={`n=${fmtInt(quality.mos_samples)} ${unit}${fs && quality.poor_calls ? ` · ${fmtInt(quality.poor_calls)} < 3,6` : ''}`}
          hint={
            fs
              ? 'Mean Opinion Score que FreeSWITCH estima (máx. 4,5) con el audio que recibe en cada tramo de la llamada; por llamada se toma el peor tramo y aquí se promedian las llamadas terminadas en los últimos 15 min. ≥4,0 bueno; <3,6 los usuarios lo notan. Las llamadas directas Kamailio → troncal no pasan por FreeSWITCH y no entran.'
              : `Mean Opinion Score estimado por RTPEngine (1–5) a partir de los informes RTCP recibidos en la ventana. ≥4,0 bueno; <3,6 los usuarios lo notan.${noRtcp}`
          }
        />
        <Metric
          label="Jitter"
          value={quality.jitter_ms == null ? '—' : quality.jitter_ms.toFixed(0)}
          unit={quality.jitter_ms == null ? undefined : 'ms'}
          noData={quality.jitter_ms == null}
          status={level(quality.jitter_ms, 30, 50)}
          sample={`n=${fmtInt(quality.jitter_samples)} muestras RTCP`}
          hint={`Variación del retardo de los paquetes RTP reportada por RTCP en la ventana. ≤30 ms bueno; >50 ms causa cortes.${
            quality.jitter_samples === 0
              ? ' Sin datos: RTPEngine reenvía el RTCP en el módulo kernel y no lo analiza, y FreeSWITCH no entrega un jitter en ms (lo incorpora en su MOS).'
              : ''
          }`}
        />
        <Metric
          label="Pérdida de paquetes"
          value={quality.packet_loss_pct == null ? '—' : `${quality.packet_loss_pct.toFixed(2)}%`}
          noData={quality.packet_loss_pct == null}
          status={level(quality.packet_loss_pct, 1, 3)}
          sample={`n=${fmtInt(quality.packet_loss_samples)} ${unit}`}
          hint={
            fs
              ? 'Paquetes RTP que faltaron en la secuencia recibida por FreeSWITCH (peor tramo de cada llamada), promedio de las llamadas terminadas en los últimos 15 min. ≤1% bueno; >3% audio entrecortado.'
              : `Pérdida de paquetes RTP reportada por RTCP, promedio de la ventana. ≤1% bueno; >3% audio entrecortado.${noRtcp}`
          }
        />
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
        <Metric
          label="PDD al siguiente salto"
          value={quality.pdd_p50_ms == null ? '—' : pddText(quality.pdd_p50_ms)}
          unit={quality.pdd_p95_ms == null ? undefined : `mediana · p95 ${pddText(quality.pdd_p95_ms)}`}
          noData={quality.pdd_samples === 0}
          sample={`n=${fmtInt(quality.pdd_samples)} llamadas · solo informativo`}
          hint={`Del INVITE que recibe FreeSWITCH al primer 180/183 que devuelve el siguiente salto, en las llamadas de la última hora vía FreeSWITCH. Hoy el siguiente salto es casi siempre FreeSWITCH Legado, que responde 180/183 al instante con su propio ringback: por eso da menos de ${FS_CLOCK_MS} ms (la resolución del reloj de FreeSWITCH). No es el PDD de la red de destino, que queda detrás de Legado y no se ve desde este SBC; por eso no lleva semáforo. Las llamadas directas Kamailio → troncal no traen este dato.`}
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
