import { useState } from 'react'
import { useMutation, useQueryClient } from '@tanstack/react-query'
import {
  Area,
  CartesianGrid,
  ComposedChart,
  Line,
  ReferenceArea,
  ReferenceLine,
  ResponsiveContainer,
  Tooltip,
  XAxis,
  YAxis,
} from 'recharts'
import { Table2, LineChart as LineChartIcon, Plus, X, Flag } from 'lucide-react'
import { createNocEvent, deleteNocEvent } from '../../api/client'
import type { NocCompare, NocEvent, NocEventKind, NocSlot, NocSlotValues } from '../../types'
import { EVENT_KIND_LABELS, compareLabel, fmtCompact, fmtInt, pctChange } from './format'

type Metric = 'attempts' | 'answered' | 'concurrent'

const METRICS: Array<{ id: Metric; label: string; help: string }> = [
  { id: 'attempts', label: 'Intentos', help: 'Llamadas iniciadas por intervalo de 15 min' },
  { id: 'answered', label: 'Contestadas', help: 'Llamadas contestadas por intervalo de 15 min' },
  { id: 'concurrent', label: 'Simultáneas', help: 'Pico de llamadas en curso dentro de cada intervalo de 15 min' },
]

interface Point {
  label: string
  today: number | null
  compare: number
  /** Titles of the marks covering this slot */
  marks: string[]
}

const SLOT_MS = 15 * 60 * 1000

/** A mark placed on the time-of-day axis, for today or the comparison day */
interface PlacedMark {
  event: NocEvent
  onCompareDay: boolean
  from: number
  to: number
}

function placeMarks(events: NocEvent[], dayStart: string, compareDayStart: string, slots: number): PlacedMark[] {
  const today = new Date(dayStart).getTime()
  const cmp = new Date(compareDayStart).getTime()
  const slotOf = (t: number, base: number) => Math.floor((t - base) / SLOT_MS)
  return events.flatMap(event => {
    const start = new Date(event.starts_at).getTime()
    const end = event.ends_at ? new Date(event.ends_at).getTime() : start
    return [
      { base: today, onCompareDay: false },
      { base: cmp, onCompareDay: true },
    ]
      .map(({ base, onCompareDay }) => ({
        event,
        onCompareDay,
        from: slotOf(start, base),
        to: slotOf(end, base),
      }))
      .filter(m => m.to >= 0 && m.from < slots)
      .map(m => ({ ...m, from: Math.max(m.from, 0), to: Math.min(m.to, slots - 1) }))
  })
}

const fmtHm = (iso: string) =>
  new Date(iso).toLocaleTimeString('es-PE', { hour: '2-digit', minute: '2-digit', hour12: false })

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
      {p.marks.map(m => (
        <div key={m} className="mt-1 flex items-center gap-1.5 text-xs" style={{ color: 'var(--viz-event)' }}>
          <Flag className="w-3 h-3 shrink-0" aria-hidden="true" />
          <span className="truncate max-w-[220px]">{m}</span>
        </div>
      ))}
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

function MarkForm({
  dayStart,
  compareDayStart,
  compareName,
  onDone,
}: {
  dayStart: string
  compareDayStart: string
  compareName: string
  onDone: () => void
}) {
  const queryClient = useQueryClient()
  const [day, setDay] = useState<'today' | 'compare'>('today')
  const [from, setFrom] = useState('')
  const [to, setTo] = useState('')
  const [kind, setKind] = useState<NocEventKind>('incident')
  const [title, setTitle] = useState('')
  const create = useMutation({
    mutationFn: createNocEvent,
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['nocOverview'] })
      onDone()
    },
  })
  // Times are local to the operation: offset from the day start the API sent
  const at = (hm: string) => {
    const [h, m] = hm.split(':').map(Number)
    return new Date(new Date(day === 'today' ? dayStart : compareDayStart).getTime() + (h * 60 + m) * 60000).toISOString()
  }
  const valid = title.trim() && from && (!to || to >= from)
  const field = 'px-2 py-1 text-xs rounded-md border border-[var(--color-border-primary)] bg-[var(--color-bg-card)] text-[var(--color-text-primary)]'

  return (
    <form
      className="mt-3 flex flex-wrap items-end gap-2 p-3 rounded-lg border border-[var(--color-border-primary)]"
      onSubmit={e => {
        e.preventDefault()
        if (valid) create.mutate({ kind, title: title.trim(), starts_at: at(from), ends_at: to ? at(to) : null })
      }}
    >
      <label className="flex flex-col gap-1 text-[11px] text-[var(--color-text-tertiary)]">
        Día
        <select className={field} value={day} onChange={e => setDay(e.target.value as 'today' | 'compare')}>
          <option value="today">Hoy</option>
          <option value="compare">{compareName}</option>
        </select>
      </label>
      <label className="flex flex-col gap-1 text-[11px] text-[var(--color-text-tertiary)]">
        Desde
        <input type="time" required className={field} value={from} onChange={e => setFrom(e.target.value)} />
      </label>
      <label className="flex flex-col gap-1 text-[11px] text-[var(--color-text-tertiary)]">
        Hasta (opcional)
        <input type="time" className={field} value={to} min={from} onChange={e => setTo(e.target.value)} />
      </label>
      <label className="flex flex-col gap-1 text-[11px] text-[var(--color-text-tertiary)]">
        Tipo
        <select className={field} value={kind} onChange={e => setKind(e.target.value as NocEventKind)}>
          {Object.entries(EVENT_KIND_LABELS).map(([k, v]) => (
            <option key={k} value={k}>{v}</option>
          ))}
        </select>
      </label>
      <label className="flex flex-col gap-1 text-[11px] text-[var(--color-text-tertiary)] flex-1 min-w-[180px]">
        Descripción
        <input
          className={field}
          maxLength={120}
          required
          placeholder="p. ej. Campaña SMS cliente X"
          value={title}
          onChange={e => setTitle(e.target.value)}
        />
      </label>
      <button
        type="submit"
        disabled={!valid || create.isPending}
        className="px-3 py-1.5 text-xs font-medium rounded-md bg-[var(--viz-today)] text-white disabled:opacity-50 cursor-pointer"
      >
        Guardar marca
      </button>
      <button type="button" onClick={onDone} className="px-2 py-1.5 text-xs text-[var(--color-text-tertiary)] cursor-pointer">
        Cancelar
      </button>
      {create.isError && <p className="w-full text-xs" style={{ color: 'var(--viz-status-critical)' }}>No se pudo guardar la marca.</p>}
    </form>
  )
}

export default function TrafficChart({
  series,
  currentSlot,
  compare,
  dayStart,
  compareDayStart,
  events,
  canEdit,
}: {
  series: NocSlot[]
  currentSlot: number
  compare: NocCompare
  dayStart: string
  compareDayStart: string
  events: NocEvent[]
  /** Admins can add and remove marks */
  canEdit: boolean
}) {
  const queryClient = useQueryClient()
  const [metric, setMetric] = useState<Metric>('attempts')
  const [asTable, setAsTable] = useState(false)
  const [adding, setAdding] = useState(false)
  const compareName = compareLabel(compare).replace(/^./, c => c.toUpperCase())
  const remove = useMutation({
    mutationFn: deleteNocEvent,
    onSuccess: () => queryClient.invalidateQueries({ queryKey: ['nocOverview'] }),
  })

  const marks = placeMarks(events, dayStart, compareDayStart, series.length)
  const markName = (m: PlacedMark) => `${m.onCompareDay ? `${compareName}: ` : ''}${m.event.title}`
  const data: Point[] = series.map((s, i) => ({
    label: s.label,
    today: s.today ? s.today[metric] : null,
    compare: s.compare[metric],
    marks: marks.filter(m => i >= m.from && i <= m.to).map(markName),
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
        {marks.length > 0 && (
          <span className="flex items-center gap-2">
            <svg width="10" height="12" aria-hidden="true"><line x1="5" y1="0" x2="5" y2="12" stroke="var(--viz-event)" strokeWidth="2" /></svg>
            Marcas
          </span>
        )}
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
            {/* Hover cursor is a soft band so it never reads as a second "Ahora" */}
            <Tooltip
              content={<ChartTooltip compareName={compareName} />}
              cursor={{ stroke: 'var(--viz-grid)', strokeWidth: 10, strokeOpacity: 0.7 }}
              isAnimationActive={false}
            />
            {marks.map(m =>
              m.to > m.from ? (
                <ReferenceArea
                  key={`${m.event.id}-${m.onCompareDay}`}
                  x1={series[m.from].label}
                  x2={series[m.to].label}
                  fill="var(--viz-event)"
                  fillOpacity={m.onCompareDay ? 0.06 : 0.12}
                  stroke="var(--viz-event)"
                  strokeOpacity={0.5}
                  strokeDasharray={m.onCompareDay ? '3 3' : undefined}
                  ifOverflow="extendDomain"
                />
              ) : (
                <ReferenceLine
                  key={`${m.event.id}-${m.onCompareDay}`}
                  x={series[m.from].label}
                  stroke="var(--viz-event)"
                  strokeWidth={1.5}
                  strokeDasharray={m.onCompareDay ? '3 3' : undefined}
                />
              ),
            )}
            {nowLabel && (
              <ReferenceLine
                x={nowLabel}
                stroke="var(--viz-today)"
                strokeWidth={1.5}
                label={{ value: 'Ahora', position: 'top', fontSize: 11, fontWeight: 600, fill: 'var(--viz-today)' }}
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

      {/* Marks: context for spikes and drops (incidents, campaigns, migrations) */}
      {(marks.length > 0 || canEdit) && (
        <div className="mt-3 flex flex-wrap items-center gap-2">
          {marks.map(({ event: e, onCompareDay }) => (
            <span
              key={`${e.id}-${onCompareDay}`}
              className="inline-flex items-center gap-1.5 pl-2 pr-1 py-0.5 rounded-md border text-xs text-[var(--color-text-secondary)]"
              style={{ borderColor: 'var(--viz-event)' }}
              title={e.created_by ? `Creada por ${e.created_by}` : undefined}
            >
              <Flag className="w-3 h-3" style={{ color: 'var(--viz-event)' }} aria-hidden="true" />
              <span className="font-medium">{EVENT_KIND_LABELS[e.kind] ?? e.kind}</span>
              <span className="tabular-nums text-[var(--color-text-tertiary)]">
                {onCompareDay ? `${compareName} ` : ''}
                {fmtHm(e.starts_at)}
                {e.ends_at ? `–${fmtHm(e.ends_at)}` : ''}
              </span>
              <span className="truncate max-w-[240px]">{e.title}</span>
              {canEdit && (
                <button
                  onClick={() => remove.mutate(e.id)}
                  className="p-0.5 rounded text-[var(--color-text-tertiary)] hover:text-[var(--color-text-primary)] cursor-pointer"
                  aria-label={`Quitar marca ${e.title}`}
                >
                  <X className="w-3 h-3" />
                </button>
              )}
            </span>
          ))}
          {canEdit && !adding && (
            <button
              onClick={() => setAdding(true)}
              className="inline-flex items-center gap-1 px-2 py-0.5 rounded-md border border-dashed border-[var(--color-border-primary)] text-xs text-[var(--color-text-tertiary)] hover:text-[var(--color-text-primary)] cursor-pointer"
            >
              <Plus className="w-3 h-3" aria-hidden="true" />
              Marcar incidente o campaña
            </button>
          )}
        </div>
      )}
      {adding && (
        <MarkForm dayStart={dayStart} compareDayStart={compareDayStart} compareName={compareName} onDone={() => setAdding(false)} />
      )}
    </div>
  )
}
