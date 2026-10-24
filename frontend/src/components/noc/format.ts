// Shared math and formatting for the NOC dashboard. Ratios are always derived
// from raw counters here so tiles, chart, tooltips and tables agree.
import type { NocCompare, NocTotals } from '../../types'

export const asr = (t: Pick<NocTotals, 'attempts' | 'answered'>): number | null =>
  t.attempts > 0 ? (100 * t.answered) / t.attempts : null

/** Average billed seconds per answered call */
export const acd = (t: Pick<NocTotals, 'answered' | 'billsec'>): number | null =>
  t.answered > 0 ? t.billsec / t.answered : null

/** Network Effectiveness Ratio (ITU-T E.425): share of attempts the network did not fail */
export const ner = (t: Pick<NocTotals, 'attempts' | 'network_failures'>): number | null =>
  t.attempts > 0 ? 100 - (100 * t.network_failures) / t.attempts : null

export const minutes = (t: Pick<NocTotals, 'billsec'>): number => t.billsec / 60

const nf = new Intl.NumberFormat('es-PE')
const nf1 = new Intl.NumberFormat('es-PE', { minimumFractionDigits: 1, maximumFractionDigits: 1 })

export const fmtInt = (n: number): string => nf.format(Math.round(n))
export const fmtPct = (n: number | null): string => (n == null ? '—' : `${nf1.format(n)}%`)

export const fmtCompact = (n: number): string =>
  n >= 1_000_000 ? `${nf1.format(n / 1_000_000)}M` : n >= 1_000 ? `${nf1.format(n / 1_000)}K` : fmtInt(n)

export const fmtMmss = (seconds: number | null): string => {
  if (seconds == null) return '—'
  const s = Math.round(seconds)
  return `${Math.floor(s / 60)}:${String(s % 60).padStart(2, '0')}`
}

export const compareLabel = (c: NocCompare): string => (c === 'week' ? 'sem. pasada' : 'ayer')
export const compareLongLabel = (c: NocCompare): string =>
  c === 'week' ? 'el mismo día de la semana pasada' : 'ayer'

/** Relative change in percent; null when there is no base */
export const pctChange = (now: number, before: number): number | null =>
  before > 0 ? (100 * (now - before)) / before : null

// Hangup causes in operator language. Unknown causes fall back to the raw name.
const CAUSE_LABELS: Record<string, string> = {
  ORIGINATOR_CANCEL: 'Cancelada por quien llama',
  NORMAL_CLEARING: 'Liberada sin contestar',
  CALL_REJECTED: 'Rechazada',
  USER_BUSY: 'Ocupado',
  NO_ANSWER: 'Sin respuesta',
  NO_USER_RESPONSE: 'Sin respuesta del terminal',
  UNALLOCATED_NUMBER: 'Número no asignado',
  RECOVERY_ON_TIMER_EXPIRE: 'Timeout de señalización',
  NORMAL_TEMPORARY_FAILURE: 'Falla temporal',
  NETWORK_OUT_OF_ORDER: 'Red fuera de servicio',
  DESTINATION_OUT_OF_ORDER: 'Destino fuera de servicio',
  NORMAL_UNSPECIFIED: 'Normal, sin especificar',
  SWITCH_CONGESTION: 'Congestión',
  NORMAL_CIRCUIT_CONGESTION: 'Congestión de circuitos',
  NO_ROUTE_DESTINATION: 'Sin ruta al destino',
  INCOMPATIBLE_DESTINATION: 'Destino incompatible',
  MEDIA_TIMEOUT: 'Timeout de media (RTP)',
  SERVICE_UNAVAILABLE: 'Servicio no disponible',
  USER_NOT_REGISTERED: 'Usuario no registrado',
}

export const causeLabel = (cause: string): string => CAUSE_LABELS[cause] ?? cause

export const CATEGORY_LABELS: Record<string, string> = {
  network: 'Red',
  user: 'Usuario',
  cancel: 'Cancelación',
  other: 'Otra',
}
