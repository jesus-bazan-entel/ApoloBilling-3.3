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

// Cause labels live in lib/callStatus so the CDR screen uses the same catalog
export { causeLabel } from '../../lib/callStatus'

export const CATEGORY_LABELS: Record<string, string> = {
  network: 'Red',
  user: 'Usuario',
  cancel: 'Cancelación',
  other: 'Otra',
}

// SIP final behind each cause, as FreeSWITCH (sofia sip2cause) and
// kamailio_cdr_sync map them, so both CDR paths read the same. 408 and 504
// both become RECOVERY_ON_TIMER_EXPIRE (network); 480 is NO_USER_RESPONSE.
const CAUSE_SIP: Record<string, string> = {
  ORIGINATOR_CANCEL: '487',
  USER_BUSY: '486',
  NO_ANSWER: '480',
  NO_USER_RESPONSE: '480',
  NORMAL_CLEARING: '486',
  CALL_REJECTED: '603',
  UNALLOCATED_NUMBER: '404',
  NO_ROUTE_DESTINATION: '404',
  NUMBER_CHANGED: '410',
  SUBSCRIBER_ABSENT: '480',
  INVALID_NUMBER_FORMAT: '484',
  RECOVERY_ON_TIMER_EXPIRE: '408/504',
  NORMAL_TEMPORARY_FAILURE: '503',
  NETWORK_OUT_OF_ORDER: '502',
  DESTINATION_OUT_OF_ORDER: '502',
  SWITCH_CONGESTION: '503',
  NORMAL_CIRCUIT_CONGESTION: '503',
  REQUESTED_CHAN_UNAVAIL: '503',
  SERVICE_UNAVAILABLE: '503',
  INCOMPATIBLE_DESTINATION: '488',
  BEARERCAPABILITY_NOTAVAIL: '488',
  INTERWORKING: '500',
  NORMAL_UNSPECIFIED: '480',
  // No explicit case in mod_sofia's hangup_cause_to_sip: falls to its 480 default
  USER_NOT_REGISTERED: '480',
}

export const causeSipCode = (cause: string): string | null => CAUSE_SIP[cause] ?? null

/** Why a cause is classed the way it is, for the tooltip */
export const CAUSE_NOTES: Record<string, string> = {
  RECOVERY_ON_TIMER_EXPIRE:
    'El siguiente salto respondió 408/504 o no respondió y venció el timer de transacción: falla de red, resta al NER.',
  NO_USER_RESPONSE: 'El destino respondió 480 (no disponible): decisión/estado del usuario, no resta al NER.',
  NO_ANSWER: 'Venció el tiempo de timbrado sin que contestaran: usuario, no resta al NER.',
  USER_NOT_REGISTERED:
    'El destino (o el ramal paralelo hacia la app) no tenía registro SIP vigente. FreeSWITCH no tiene un código propio para esta causa y responde 480: usuario, no resta al NER.',
  NORMAL_CLEARING:
    'Ring paralelo (Legado + app): Legado respondió 486 y la app no estaba registrada. FreeSWITCH contesta 486 al carrier pero guarda la causa Q.850 16.',
}

export type StatusLevel = 'good' | 'warning' | 'critical'

/** Level of a "higher is better" ratio against operator thresholds */
export const thresholdStatus = (
  value: number | null,
  warn: number,
  crit: number,
): { level: StatusLevel; label: string } | null =>
  value == null
    ? null
    : value >= warn
      ? { level: 'good', label: 'OK' }
      : value >= crit
        ? { level: 'warning', label: 'Vigilar' }
        : { level: 'critical', label: 'Crítico' }

/** 917012190 → 9170•••190; short extensions keep only their first digits */
export const maskNumber = (n: string): string => {
  if (!n) return n
  if (n.length >= 7) return `${n.slice(0, 4)}•••${n.slice(-3)}`
  if (n.length >= 4) return `${n.slice(0, 2)}•••`
  return n
}

export const EVENT_KIND_LABELS: Record<string, string> = {
  incident: 'Incidente',
  campaign: 'Campaña',
  maintenance: 'Mantenimiento',
  migration: 'Migración de ruta',
  other: 'Otro',
}
