// Single catalog of call outcomes for every screen (CDR table, NOC panels).
//
// A call is answered when it has an answer time (the backend's ASR uses the
// same rule: COUNT(answer_time)); the hangup cause only explains why an
// unanswered call failed. NORMAL_CLEARING alone does not mean "completed":
// unanswered NORMAL_CLEARING is a parallel-ring bridge whose answering leg was
// busy (FreeSWITCH answers 486 but keeps Q.850 16).

// Causes of unanswered calls in operator language. Unknown causes fall back to the raw name.
const CAUSE_LABELS: Record<string, string> = {
  ORIGINATOR_CANCEL: 'Cancelada por quien llama',
  NORMAL_CLEARING: 'Ocupado en ring paralelo',
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

export type OutcomeVariant = 'success' | 'warning' | 'error' | 'default'

/** Unanswered causes that are the called party's decision or state, not a failure */
const USER_SIDE = new Set([
  'NORMAL_CLEARING',
  'ORIGINATOR_CANCEL',
  'USER_BUSY',
  'NO_ANSWER',
  'NO_USER_RESPONSE',
  'CALL_REJECTED',
  'USER_NOT_REGISTERED',
])

export function callOutcome(cdr: {
  answer_time?: string | null
  hangup_cause?: string | null
}): { label: string; variant: OutcomeVariant } {
  if (cdr.answer_time) return { label: 'Contestada', variant: 'success' }
  const cause = cdr.hangup_cause || ''
  if (!cause) return { label: 'Desconocido', variant: 'default' }
  return { label: causeLabel(cause), variant: USER_SIDE.has(cause) ? 'warning' : 'error' }
}
