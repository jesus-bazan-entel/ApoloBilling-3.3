import { useState } from 'react'
import { ArrowDown, ArrowUp } from 'lucide-react'
import type { NocCompare, NocRange } from '../../types'
import { acd, asr, compareLabel, fmtInt, fmtMmss, fmtPct, pctChange } from './format'
import { Sparkline } from './KpiTile'

const ROUTE_LABEL = { direct: 'Directo', freeswitch: 'Vía FreeSWITCH' }

type SortKey = 'prefix' | 'attempts' | 'change' | 'asr' | 'acd' | 'minutes'

const COLUMNS: Array<{ key: SortKey; label: string; align: 'left' | 'right' }> = [
  { key: 'prefix', label: 'Rango', align: 'left' },
  { key: 'attempts', label: 'Intentos', align: 'right' },
  { key: 'change', label: 'Var.', align: 'right' },
  { key: 'asr', label: 'ASR', align: 'right' },
  { key: 'acd', label: 'ACD', align: 'right' },
  { key: 'minutes', label: 'Minutos', align: 'right' },
]

/**
 * A prefix whose FreeSWITCH traffic collapsed while its direct traffic grew
 * was moved to direct routing (migration 017), not lost.
 */
function migratedPrefixes(ranges: NocRange[]): Set<string> {
  const out = new Set<string>()
  for (const fs of ranges.filter(r => r.route === 'freeswitch')) {
    const direct = ranges.find(r => r.route === 'direct' && r.prefix === fs.prefix)
    const ch = pctChange(fs.attempts, fs.compare_attempts)
    if (direct && ch != null && ch <= -50 && direct.attempts > direct.compare_attempts) out.add(fs.prefix)
  }
  return out
}

const sortValue = (r: NocRange, key: SortKey): number | string => {
  switch (key) {
    case 'prefix':
      return r.prefix
    case 'attempts':
      return r.attempts
    case 'change':
      return pctChange(r.attempts, r.compare_attempts) ?? Number.POSITIVE_INFINITY
    case 'asr':
      return asr(r) ?? -1
    case 'acd':
      return acd(r) ?? -1
    case 'minutes':
      return r.billsec
  }
}

/** Traffic per called-number prefix and signalling path (direct vs through FreeSWITCH) */
export default function RangesTable({
  ranges,
  compare,
  minAttempts,
  currentSlot,
}: {
  ranges: NocRange[]
  compare: NocCompare
  minAttempts: number
  /** 15-min slot in progress (operation timezone); cuts the trend at the current hour */
  currentSlot: number
}) {
  const currentHour = Math.floor(currentSlot / 4)
  const [sort, setSort] = useState<{ key: SortKey; desc: boolean }>({ key: 'attempts', desc: true })
  const [showAll, setShowAll] = useState(false)

  const total = ranges.reduce((a, r) => a + r.attempts, 0)
  const direct = ranges.filter(r => r.route === 'direct').reduce((a, r) => a + r.attempts, 0)
  const directPct = total ? (100 * direct) / total : 0
  const migrated = migratedPrefixes(ranges)

  const hidden = ranges.filter(r => r.attempts < minAttempts).length
  const rows = (showAll ? ranges : ranges.filter(r => r.attempts >= minAttempts)).slice().sort((a, b) => {
    const va = sortValue(a, sort.key)
    const vb = sortValue(b, sort.key)
    const cmp = typeof va === 'string' ? va.localeCompare(vb as string) : va - (vb as number)
    return sort.desc ? -cmp : cmp
  })

  const toggleSort = (key: SortKey) =>
    setSort(s => (s.key === key ? { key, desc: !s.desc } : { key, desc: key !== 'prefix' }))

  return (
    <div className="bg-[var(--color-bg-card)] rounded-xl border border-[var(--color-border-primary)] p-5">
      <h2 className="text-base font-semibold text-[var(--color-text-primary)]">Tráfico por rango de DID y camino</h2>
      <p className="text-xs text-[var(--color-text-tertiary)] mt-0.5 mb-4">
        Hoy · primeros 4 dígitos del número llamado · variación vs {compareLabel(compare)} a la misma hora
      </p>

      {/* Share of traffic that already bypasses FreeSWITCH */}
      <div className="mb-4">
        <div className="flex justify-between text-xs mb-1">
          <span className="text-[var(--color-text-secondary)]">
            Directo Kamailio → troncal: <span className="font-semibold text-[var(--color-text-primary)] tabular-nums">{directPct.toFixed(1)}%</span>
          </span>
          <span className="text-[var(--color-text-secondary)]">
            Vía FreeSWITCH: <span className="font-semibold text-[var(--color-text-primary)] tabular-nums">{(100 - directPct).toFixed(1)}%</span>
          </span>
        </div>
        <div className="flex h-2 rounded-full overflow-hidden bg-[var(--viz-grid)]" role="img" aria-label={`${directPct.toFixed(1)}% del tráfico va directo`}>
          <div style={{ width: `${directPct}%`, background: 'var(--viz-today)' }} />
          <div style={{ width: `${100 - directPct}%`, background: 'var(--viz-compare)', marginLeft: directPct > 0 && directPct < 100 ? 2 : 0 }} />
        </div>
      </div>

      {ranges.length === 0 ? (
        <p className="text-sm text-[var(--color-text-tertiary)] py-8 text-center">Sin tráfico hoy</p>
      ) : (
        <>
          <div className="overflow-x-auto">
            <table className="w-full text-sm">
              <thead>
                <tr className="text-xs text-[var(--color-text-tertiary)] border-b border-[var(--viz-grid)]">
                  {COLUMNS.map(c => (
                    <th
                      key={c.key}
                      className={`font-medium py-1.5 ${c.align === 'right' ? 'text-right' : 'text-left'}`}
                      aria-sort={sort.key === c.key ? (sort.desc ? 'descending' : 'ascending') : 'none'}
                    >
                      <button
                        onClick={() => toggleSort(c.key)}
                        className={`inline-flex items-center gap-0.5 cursor-pointer hover:text-[var(--color-text-primary)] ${
                          sort.key === c.key ? 'text-[var(--color-text-primary)]' : ''
                        }`}
                      >
                        {c.label}
                        {sort.key === c.key &&
                          (sort.desc ? <ArrowDown className="w-3 h-3" aria-hidden="true" /> : <ArrowUp className="w-3 h-3" aria-hidden="true" />)}
                      </button>
                    </th>
                  ))}
                  <th className="text-right font-medium py-1.5 pl-3">Hoy por hora</th>
                </tr>
              </thead>
              <tbody className="tabular-nums">
                {rows.map(r => {
                  const ch = pctChange(r.attempts, r.compare_attempts)
                  const movedAway = r.route === 'freeswitch' && migrated.has(r.prefix)
                  const movedIn = r.route === 'direct' && migrated.has(r.prefix)
                  return (
                    <tr key={`${r.prefix}-${r.route}`} className="border-b border-[var(--viz-grid)] last:border-0">
                      <td className="py-1.5">
                        <div className="font-mono text-[var(--color-text-primary)]">{r.prefix}…</div>
                        <span className="flex items-center gap-1.5 text-[11px] text-[var(--color-text-tertiary)]">
                          <span
                            className="w-2 h-2 rounded-full"
                            style={{ background: r.route === 'direct' ? 'var(--viz-today)' : 'var(--viz-compare)' }}
                            aria-hidden="true"
                          />
                          {ROUTE_LABEL[r.route]}
                        </span>
                      </td>
                      <td className="py-1.5 text-right text-[var(--color-text-primary)]">{fmtInt(r.attempts)}</td>
                      <td className="py-1.5 text-right text-xs text-[var(--color-text-tertiary)]">
                        {movedAway ? (
                          <span title={`${ch?.toFixed(0)}% vs ${compareLabel(compare)}: el tráfico pasó al camino Directo`}>migrado a Directo</span>
                        ) : movedIn ? (
                          <span title={ch == null ? 'Sin tráfico directo en la comparación' : `${ch > 0 ? '+' : ''}${ch.toFixed(0)}%`}>desde FreeSWITCH</span>
                        ) : ch == null ? (
                          'nuevo'
                        ) : (
                          `${ch > 0 ? '+' : ''}${ch.toFixed(0)}%`
                        )}
                      </td>
                      <td className="py-1.5 text-right text-[var(--color-text-secondary)]">{fmtPct(asr(r))}</td>
                      <td className="py-1.5 text-right text-[var(--color-text-secondary)]">{fmtMmss(acd(r))}</td>
                      <td className="py-1.5 text-right text-[var(--color-text-secondary)]">{fmtInt(r.billsec / 60)}</td>
                      <td className="py-1.5 pl-3">
                        <div className="flex justify-end">
                          <Sparkline values={r.hourly_attempts.map((v, h) => (h <= currentHour ? v : null))} w={72} h={20} />
                        </div>
                      </td>
                    </tr>
                  )
                })}
              </tbody>
            </table>
          </div>
          {hidden > 0 && (
            <button
              onClick={() => setShowAll(v => !v)}
              className="mt-3 text-xs text-[var(--color-text-tertiary)] hover:text-[var(--color-text-primary)] cursor-pointer"
            >
              {showAll
                ? `Ocultar rangos con menos de ${fmtInt(minAttempts)} intentos`
                : `Mostrar ${fmtInt(hidden)} rango${hidden === 1 ? '' : 's'} con menos de ${fmtInt(minAttempts)} intentos (ASR poco fiable)`}
            </button>
          )}
        </>
      )}
    </div>
  )
}
