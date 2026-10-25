import { ArrowDownRight, ArrowUpRight, Minus } from 'lucide-react'

export interface KpiDelta {
  /** Formatted signed change, e.g. "+4,2%" or "−1,3 pp" */
  text: string
  /** Raw change, used for direction */
  value: number
  /** Whether a rise is good news (null = neutral metric, never colored) */
  upIsGood: boolean | null
  /** Changes smaller than this are shown as flat */
  flatBelow: number
}

interface KpiTileProps {
  label: string
  value: string
  unit?: string
  delta?: KpiDelta | null
  /** e.g. "vs ayer a esta hora" */
  deltaContext: string
  detail?: string
  /** Optional status chip; color never carries it alone (icon + label) */
  status?: { level: 'good' | 'warning' | 'critical'; label: string } | null
  /** Sparkline values; nulls are gaps */
  trend?: Array<number | null>
  title?: string
}

const STATUS_COLOR = {
  good: 'var(--viz-status-good)',
  warning: 'var(--viz-status-warning)',
  critical: 'var(--viz-status-critical)',
}

export function Sparkline({ values, w = 120, h = 32 }: { values: Array<number | null>; w?: number; h?: number }) {
  const pts = values.map((v, i) => [i, v] as const).filter(([, v]) => v != null) as Array<[number, number]>
  if (pts.length < 2) return <div style={{ width: w, height: h }} />
  const max = Math.max(...pts.map(([, v]) => v), 1)
  const x = (i: number) => (i / Math.max(values.length - 1, 1)) * (w - 4) + 2
  const y = (v: number) => h - 3 - (v / max) * (h - 6)
  const d = pts.map(([i, v], k) => `${k ? 'L' : 'M'}${x(i).toFixed(1)} ${y(v).toFixed(1)}`).join(' ')
  const [li, lv] = pts[pts.length - 1]
  return (
    <svg viewBox={`0 0 ${w} ${h}`} width={w} height={h} aria-hidden="true">
      <path d={d} fill="none" stroke="var(--viz-compare)" strokeWidth={1.5} strokeLinejoin="round" strokeLinecap="round" />
      <circle cx={x(li)} cy={y(lv)} r={3} fill="var(--viz-today)" stroke="var(--color-bg-card)" strokeWidth={2} />
    </svg>
  )
}

export default function KpiTile({ label, value, unit, delta, deltaContext, detail, status, trend, title }: KpiTileProps) {
  let deltaColor = 'var(--color-text-tertiary)'
  let DeltaIcon = Minus
  if (delta && Math.abs(delta.value) >= delta.flatBelow) {
    DeltaIcon = delta.value > 0 ? ArrowUpRight : ArrowDownRight
    if (delta.upIsGood != null) {
      const good = delta.value > 0 === delta.upIsGood
      deltaColor = good ? 'var(--viz-delta-good)' : 'var(--viz-delta-bad)'
    }
  }

  return (
    <div
      className="bg-[var(--color-bg-card)] rounded-xl border border-[var(--color-border-primary)] p-4 flex flex-col gap-2 min-w-0"
      // Crossing an operator threshold tints the whole tile, not just the chip
      style={status && status.level !== 'good' ? { borderColor: STATUS_COLOR[status.level], boxShadow: `inset 3px 0 0 ${STATUS_COLOR[status.level]}` } : undefined}
      title={title}
    >
      <div className="flex items-center justify-between gap-2">
        <span className="text-xs font-medium text-[var(--color-text-tertiary)] truncate">{label}</span>
        {status && (
          <span className="flex items-center gap-1 text-[11px] font-medium text-[var(--color-text-secondary)]">
            <span className="w-2 h-2 rounded-full" style={{ background: STATUS_COLOR[status.level] }} aria-hidden="true" />
            {status.label}
          </span>
        )}
      </div>
      <div className="flex items-end justify-between gap-2">
        <div className="min-w-0">
          <div className="text-[28px] leading-8 font-semibold text-[var(--color-text-primary)]">
            {value}
            {unit && <span className="text-sm font-medium text-[var(--color-text-tertiary)] ml-1">{unit}</span>}
          </div>
        </div>
        {trend && <Sparkline values={trend} />}
      </div>
      <div
        className="flex items-center gap-1 text-xs min-h-4"
        title={delta ? `Verde = mejora, rojo = empeora. En gris si la variación es menor a ±${delta.flatBelow}${delta.text.endsWith('pp') ? ' pp' : '%'}.` : undefined}
      >
        {delta ? (
          <>
            <DeltaIcon className="w-3.5 h-3.5 shrink-0" style={{ color: deltaColor }} aria-hidden="true" />
            <span className="font-semibold tabular-nums" style={{ color: deltaColor }}>{delta.text}</span>
            <span className="text-[var(--color-text-tertiary)] truncate">{deltaContext}</span>
          </>
        ) : (
          <span className="text-[var(--color-text-tertiary)]">Sin base de comparación</span>
        )}
      </div>
      {detail && <div className="text-xs text-[var(--color-text-secondary)] truncate">{detail}</div>}
    </div>
  )
}
