// Quick time ranges for the CDR screen, always in Lima time (UTC-5, no DST),
// whatever the operator's PC clock zone is. Ranges are resolved when the query
// runs, so "Hoy" or "Última hora" keep moving on every refresh.

const LIMA_OFFSET_H = 5 // Lima = UTC-5 all year
const HOUR = 3_600_000
const DAY = 24 * HOUR

export type RangeKey =
  | 'last15m'
  | 'last1h'
  | 'last4h'
  | 'last24h'
  | 'today'
  | 'yesterday'
  | 'thisWeek'
  | 'lastWeek'
  | 'last7d'
  | 'thisMonth'
  | 'lastMonth'
  | 'last30d'
  | 'custom'

export interface RangeOption {
  key: Exclude<RangeKey, 'custom'>
  label: string
  /** Keyboard shortcut (live ranges only) */
  hotkey?: string
}

export const RANGE_GROUPS: { title: string; options: RangeOption[] }[] = [
  {
    title: 'En vivo',
    options: [
      { key: 'last15m', label: '15 min', hotkey: '1' },
      { key: 'last1h', label: '1 h', hotkey: '2' },
      { key: 'last4h', label: '4 h', hotkey: '3' },
      { key: 'last24h', label: '24 h', hotkey: '4' },
    ],
  },
  {
    title: 'Calendario',
    options: [
      { key: 'today', label: 'Hoy' },
      { key: 'yesterday', label: 'Ayer' },
      { key: 'thisWeek', label: 'Semana actual' },
      { key: 'lastWeek', label: 'Semana pasada' },
      { key: 'last7d', label: '7 días' },
      { key: 'thisMonth', label: 'Mes actual' },
      { key: 'lastMonth', label: 'Mes anterior' },
      { key: 'last30d', label: '30 días' },
    ],
  },
]

export const DEFAULT_RANGE: RangeKey = 'today'

const RANGE_KEYS = new Set<string>([
  ...RANGE_GROUPS.flatMap((g) => g.options.map((o) => o.key)),
  'custom',
])

export const isRangeKey = (v: string | null): v is RangeKey => v !== null && RANGE_KEYS.has(v)

/** `end: null` means "until now" (no end_date sent, new CDRs keep appearing). */
export interface ResolvedRange {
  start: Date | null
  end: Date | null
}

/** Lima calendar fields of an instant. */
const limaFields = (ms: number) => {
  const d = new Date(ms - LIMA_OFFSET_H * HOUR)
  return {
    y: d.getUTCFullYear(),
    m: d.getUTCMonth(),
    d: d.getUTCDate(),
    h: d.getUTCHours(),
    mi: d.getUTCMinutes(),
    s: d.getUTCSeconds(),
    ms: d.getUTCMilliseconds(),
    // Monday = 0 … Sunday = 6
    dow: (d.getUTCDay() + 6) % 7,
  }
}

/** 00:00 Lima of a calendar day; day/month overflow is normalised by Date.UTC. */
const limaMidnight = (y: number, m: number, d: number) =>
  new Date(Date.UTC(y, m, d, LIMA_OFFSET_H))

/** Closed range [start, next boundary): the API's end is inclusive, so stop 1 ms before. */
const closed = (start: Date, nextStart: Date): ResolvedRange => ({
  start,
  end: new Date(nextStart.getTime() - 1),
})

export function resolveRange(
  key: RangeKey,
  nowMs: number,
  custom?: { from?: string; to?: string }
): ResolvedRange {
  const t = limaFields(nowMs)
  const today = limaMidnight(t.y, t.m, t.d)
  const ago = (ms: number): ResolvedRange => ({ start: new Date(nowMs - ms), end: null })

  switch (key) {
    case 'last15m':
      return ago(15 * 60_000)
    case 'last1h':
      return ago(HOUR)
    case 'last4h':
      return ago(4 * HOUR)
    case 'last24h':
      return ago(DAY)
    case 'today':
      return { start: today, end: null }
    case 'yesterday':
      return closed(limaMidnight(t.y, t.m, t.d - 1), today)
    case 'thisWeek':
      return { start: limaMidnight(t.y, t.m, t.d - t.dow), end: null }
    case 'lastWeek': {
      const monday = limaMidnight(t.y, t.m, t.d - t.dow)
      return closed(limaMidnight(t.y, t.m, t.d - t.dow - 7), monday)
    }
    case 'last7d':
      return { start: limaMidnight(t.y, t.m, t.d - 6), end: null }
    case 'thisMonth':
      return { start: limaMidnight(t.y, t.m, 1), end: null }
    case 'lastMonth':
      return closed(limaMidnight(t.y, t.m - 1, 1), limaMidnight(t.y, t.m, 1))
    case 'last30d':
      return { start: limaMidnight(t.y, t.m, t.d - 29), end: null }
    case 'custom':
      return {
        start: parseLimaLocal(custom?.from),
        end: parseLimaLocal(custom?.to),
      }
  }
}

/** `YYYY-MM-DDTHH:MM` (a datetime-local value, read as Lima time) → instant. */
export function parseLimaLocal(value?: string | null): Date | null {
  const m = value?.match(/^(\d{4})-(\d{2})-(\d{2})T(\d{2}):(\d{2})$/)
  if (!m) return null
  const [, y, mo, d, h, mi] = m.map(Number)
  return new Date(Date.UTC(y, mo - 1, d, h + LIMA_OFFSET_H, mi))
}

const pad = (n: number, w = 2) => n.toString().padStart(w, '0')

/** Instant → `YYYY-MM-DDTHH:MM` in Lima, for datetime-local inputs. */
export function toLimaLocal(date: Date): string {
  const t = limaFields(date.getTime())
  return `${t.y}-${pad(t.m + 1)}-${pad(t.d)}T${pad(t.h)}:${pad(t.mi)}`
}

/** Instant → RFC 3339 in Lima (`2026-10-25T00:00:00.000-05:00`), what the API expects. */
export function toApiTimestamp(date: Date): string {
  const t = limaFields(date.getTime())
  return (
    `${t.y}-${pad(t.m + 1)}-${pad(t.d)}T${pad(t.h)}:${pad(t.mi)}:${pad(t.s)}.${pad(t.ms, 3)}` +
    `-${pad(LIMA_OFFSET_H)}:00`
  )
}

/** Instant → `25/10/2026 00:00` in Lima. */
export function formatLima(date: Date): string {
  const t = limaFields(date.getTime())
  return `${pad(t.d)}/${pad(t.m + 1)}/${t.y} ${pad(t.h)}:${pad(t.mi)}`
}

/** `19/10/2026 00:00 → ahora · GMT-5` */
export function describeRange(r: ResolvedRange): string {
  const from = r.start ? formatLima(r.start) : 'el inicio'
  // A closed range ends 1 ms before its boundary: show the boundary minute
  const to = r.end ? formatLima(new Date(r.end.getTime() + 1)) : 'ahora'
  return `${from} → ${to} · GMT-5`
}
