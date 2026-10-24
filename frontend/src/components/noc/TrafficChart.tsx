import { useState } from 'react'
import {
  Area,
  CartesianGrid,
  ComposedChart,
  Line,
  ReferenceLine,
  ResponsiveContainer,
  Tooltip,
  XAxis,
  YAxis,
} from 'recharts'
import { Table2, LineChart as LineChartIcon } from 'lucide-react'
import type { NocCompare, NocSlot, NocSlotValues } from '../../types'
import { compareLabel, fmtCompact, fmtInt, pctChange } from './format'

type Metric = 'attempts' | 'answered' | 'concurrent'

const METRICS: Array<{ id: Metric; label: string; help: string }> = [
  { id: 'attempts', label: 'Intentos', help: 'Llamadas iniciadas por intervalo de 15 min' },
  { id: 'answered', label: 'Contestadas', help: 'Llamadas contestadas por intervalo de 15 min' },
  { id: 'concurrent', label: 'Simultáneas', help: 'Llamadas en curso a mitad de cada intervalo de 15 min' },
]

interface Point {
  label: string
  today: number | null
  compare: number
}

// Every 2 hours on the axis
const TICKS = Array.from({ length: 12 }, (_, i) => `${String(i * 2).padStart(2, '0')}:00`)

function ChartTooltip({
  active,
  payload,
  label,
  compareName,
}: {
  active?: boolean
  payload?: Array<{ payload: Point }>
  label?: string
  compareName: string
}) {
  if (!active || !payload?.length) return null
  const p = payload[0].payload
  const change = p.today != null ? pctChange(p.today, p.compare) : null
  return (
    <div className="bg-[var(--color-bg-card)] border border-[var(--color-border-primary)] rounded-lg shadow-lg px-3 py-2 min-w-[170px]">
      <div className="text-xs text-[var(--color-text-tertiary)] mb-1.5">{label} – intervalo de 15 min</div>
      <div className="flex items-center justify-between gap-4">
        <span className="flex items-center gap-2 text-xs text-[var(--color-text-secondary)]">
          <svg width="14" height="4" aria-hidden="true"><line x1="0" y1="2" x2="14" y2="2" stroke="var(--viz-today)" strokeWidth="2" /></svg>
          Hoy
        </span>
        <span className="text-sm font-semibold tabular-nums text-[var(--color-text-primary)]">
          {p.today == null ? '—' : fmtInt(p.today)}
        </span>
      </div>
      <div className="flex items-center justify-between gap-4">
        <span className="flex items-center gap-2 text-xs text-[var(--color-text-secondary)]">
          <svg width="14" height="4" aria-hidden="true"><line x1="0" y1="2" x2="14" y2="2" stroke="var(--viz-compare)" strokeWidth="2" strokeDasharray="4 3" /></svg>
          {compareName}
        </span>
        <span className="text-sm font-semibold tabular-nums text-[var(--color-text-primary)]">{fmtInt(p.compare)}</span>
      </div>
      {change != null && (
        <div className="mt-1.5 pt-1.5 border-t border-[var(--viz-grid)] text-xs text-[var(--color-text-tertiary)] tabular-nums">
          Variación: <span className="font-semibold text-[var(--color-text-secondary)]">{change > 0 ? '+' : ''}{change.toFixed(1)}%</span>
        </div>
      )}
    </div>
  )
}

/** Hourly rollup for the table view: sums for counts, max for concurrency */
function hourlyRows(series: NocSlot[], metric: Metric) {
  const agg = (vals: number[]) => (metric === 'concurrent' ? Math.max(...vals) : vals.reduce((a, b) => a + b, 0))
  return Array.from({ length: 24 }, (_, h) => {
    const slots = series.slice(h * 4, h * 4 + 4)
    const done = slots.filter(s => s.today)
    return {
      hour: `${String(h).padStart(2, '0')}:00`,
      today: done.length ? agg(done.map(s => (s.today as NocSlotValues)[metric])) : null,
      partial: done.length > 0 && done.length < slots.length,
      compare: agg(slots.map(s => s.compare[metric])),
    }
  })
}

export default function TrafficChart({
  series,
  currentSlot,
  compare,
}: {
  series: NocSlot[]
  currentSlot: number
  compare: NocCompare
}) {
  const [metric, setMetric] = useState<Metric>('attempts')
  const [asTable, setAsTable] = useState(false)
  const compareName = compareLabel(compare).replace(/^./, c => c.toUpperCase())

  const data: Point[] = series.map(s => ({
    label: s.label,
    today: s.today ? s.today[metric] : null,
    compare: s.compare[metric],
  }))
  const nowLabel = series[Math.min(currentSlot, series.length - 1)]?.label
  const lastToday = [...data].reverse().find(d => d.today != null)

  return (
    <div className="bg-[var(--color-bg-card)] rounded-xl border border-[var(--color-border-primary)] p-5">
      <div className="flex flex-wrap items-start justify-between gap-3 mb-4">
        <div>
          <h2 className="text-base font-semibold text-[var(--color-text-primary)]">
            Tráfico de hoy vs {compareLabel(compare)}
          </h2>
          <p className="text-xs text-[var(--color-text-tertiary)] mt-0.5">
            {METRICS.find(m => m.id === metric)?.help}
            {lastToday?.today != null && (
              <> · último intervalo cerrado ({lastToday.label}): <span className="font-semibold text-[var(--color-text-secondary)]">{fmtInt(lastToday.today)}</span></>
            )}
          </p>
        </div>
        <div className="flex items-center gap-2">
          <div role="tablist" aria-label="Métrica" className="flex rounded-lg border border-[var(--color-border-primary)] p-0.5">
            {METRICS.map(m => (
              <button
                key={m.id}
                role="tab"
                aria-selected={metric === m.id}
                onClick={() => setMetric(m.id)}
                className={`px-2.5 py-1 text-xs font-medium rounded-md cursor-pointer transition-colors ${
                  metric === m.id
                    ? 'bg-[var(--color-bg-tertiary)] text-[var(--color-text-primary)]'
                    : 'text-[var(--color-text-tertiary)] hover:text-[var(--color-text-primary)]'
                }`}
              >
                {m.label}
              </button>
            ))}
          </div>
          <button
            onClick={() => setAsTable(v => !v)}
            className="p-1.5 rounded-md border border-[var(--color-border-primary)] text-[var(--color-text-tertiary)] hover:text-[var(--color-text-primary)] cursor-pointer"
            title={asTable ? 'Ver gráfica' : 'Ver tabla por hora'}
            aria-label={asTable ? 'Ver gráfica' : 'Ver tabla por hora'}
          >
            {asTable ? <LineChartIcon className="w-4 h-4" /> : <Table2 className="w-4 h-4" />}
          </button>
        </div>
      </div>

      {/* Legend: line keys mirror the marks */}
      <div className="flex items-center gap-5 mb-2 text-xs text-[var(--color-text-secondary)]">
        <span className="flex items-center gap-2">
          <svg width="18" height="4" aria-hidden="true"><line x1="0" y1="2" x2="18" y2="2" stroke="var(--viz-today)" strokeWidth="2" /></svg>
          Hoy
        </span>
        <span className="flex items-center gap-2">
          <svg width="18" height="4" aria-hidden="true"><line x1="0" y1="2" x2="18" y2="2" stroke="var(--viz-compare)" strokeWidth="2" strokeDasharray="4 3" /></svg>
          {compareName} (día completo)
        </span>
      </div>

      {asTable ? (
        <div className="max-h-[300px] overflow-auto">
          <table className="w-full text-sm">
            <thead className="sticky top-0 bg-[var(--color-bg-card)]">
              <tr className="text-xs text-[var(--color-text-tertiary)] border-b border-[var(--viz-grid)]">
                <th className="text-left font-medium py-1.5">Hora</th>
                <th className="text-right font-medium py-1.5">Hoy</th>
                <th className="text-right font-medium py-1.5">{compareName}</th>
                <th className="text-right font-medium py-1.5">Variación</th>
              </tr>
            </thead>
            <tbody className="tabular-nums">
              {hourlyRows(series, metric).map(r => {
                const ch = r.today != null && !r.partial ? pctChange(r.today, r.compare) : null
                return (
                  <tr key={r.hour} className="border-b border-[var(--viz-grid)] last:border-0">
                    <td className="py-1 text-[var(--color-text-secondary)]">{r.hour}</td>
                    <td className="py-1 text-right text-[var(--color-text-primary)]">
                      {r.today == null ? '—' : fmtInt(r.today)}
                      {r.partial && <span className="text-[var(--color-text-tertiary)]"> (parcial)</span>}
                    </td>
                    <td className="py-1 text-right text-[var(--color-text-secondary)]">{fmtInt(r.compare)}</td>
                    <td className="py-1 text-right text-[var(--color-text-secondary)]">
                      {ch == null ? '—' : `${ch > 0 ? '+' : ''}${ch.toFixed(1)}%`}
                    </td>
                  </tr>
                )
              })}
            </tbody>
          </table>
        </div>
      ) : (
        <ResponsiveContainer width="100%" height={300}>
          <ComposedChart data={data} margin={{ top: 16, right: 12, bottom: 0, left: 0 }}>
            <CartesianGrid stroke="var(--viz-grid)" vertical={false} />
            <XAxis
              dataKey="label"
              ticks={TICKS}
              interval={0}
              tick={{ fontSize: 11, fill: 'var(--color-text-tertiary)' }}
              axisLine={{ stroke: 'var(--viz-axis)' }}
              tickLine={false}
            />
            <YAxis
              tickFormatter={fmtCompact}
              tick={{ fontSize: 11, fill: 'var(--color-text-tertiary)' }}
              axisLine={false}
              tickLine={false}
              width={44}
              allowDecimals={false}
            />
            <Tooltip
              content={<ChartTooltip compareName={compareName} />}
              cursor={{ stroke: 'var(--viz-axis)', strokeWidth: 1 }}
              isAnimationActive={false}
            />
            {nowLabel && (
              <ReferenceLine
                x={nowLabel}
                stroke="var(--viz-axis)"
                label={{ value: 'Ahora', position: 'top', fontSize: 11, fill: 'var(--color-text-tertiary)' }}
              />
            )}
            <Line
              type="monotone"
              dataKey="compare"
              name={compareName}
              stroke="var(--viz-compare)"
              strokeWidth={2}
              strokeDasharray="5 4"
              dot={false}
              activeDot={false}
              isAnimationActive={false}
            />
            <Area
              type="monotone"
              dataKey="today"
              stroke="none"
              fill="var(--viz-today)"
              fillOpacity={0.1}
              connectNulls={false}
              isAnimationActive={false}
              activeDot={false}
            />
            <Line
              type="monotone"
              dataKey="today"
              name="Hoy"
              stroke="var(--viz-today)"
              strokeWidth={2}
              strokeLinecap="round"
              strokeLinejoin="round"
              connectNulls={false}
              dot={false}
              activeDot={{ r: 4, fill: 'var(--viz-today)', stroke: 'var(--color-bg-card)', strokeWidth: 2 }}
              isAnimationActive={false}
            />
          </ComposedChart>
        </ResponsiveContainer>
      )}
    </div>
  )
}
