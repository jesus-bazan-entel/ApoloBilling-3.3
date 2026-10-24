import type { NocCompare, NocRange } from '../../types'
import { acd, asr, compareLabel, fmtInt, fmtMmss, fmtPct, pctChange } from './format'

const ROUTE_LABEL = { direct: 'Directo', freeswitch: 'Vía FreeSWITCH' }

/** Traffic per called-number prefix and signalling path (direct vs through FreeSWITCH) */
export default function RangesTable({ ranges, compare }: { ranges: NocRange[]; compare: NocCompare }) {
  const total = ranges.reduce((a, r) => a + r.attempts, 0)
  const direct = ranges.filter(r => r.route === 'direct').reduce((a, r) => a + r.attempts, 0)
  const directPct = total ? (100 * direct) / total : 0

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
        <div className="overflow-x-auto">
          <table className="w-full text-sm">
            <thead>
              <tr className="text-xs text-[var(--color-text-tertiary)] border-b border-[var(--viz-grid)]">
                <th className="text-left font-medium py-1.5">Rango</th>
                <th className="text-left font-medium py-1.5">Camino</th>
                <th className="text-right font-medium py-1.5">Intentos</th>
                <th className="text-right font-medium py-1.5">Var.</th>
                <th className="text-right font-medium py-1.5">ASR</th>
                <th className="text-right font-medium py-1.5">ACD</th>
                <th className="text-right font-medium py-1.5">Minutos</th>
              </tr>
            </thead>
            <tbody className="tabular-nums">
              {ranges.map(r => {
                const ch = pctChange(r.attempts, r.compare_attempts)
                return (
                  <tr key={`${r.prefix}-${r.route}`} className="border-b border-[var(--viz-grid)] last:border-0">
                    <td className="py-1.5 font-mono text-[var(--color-text-primary)]">{r.prefix}…</td>
                    <td className="py-1.5">
                      <span className="flex items-center gap-1.5 text-xs text-[var(--color-text-secondary)]">
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
                      {ch == null ? 'nuevo' : `${ch > 0 ? '+' : ''}${ch.toFixed(0)}%`}
                    </td>
                    <td className="py-1.5 text-right text-[var(--color-text-secondary)]">{fmtPct(asr(r))}</td>
                    <td className="py-1.5 text-right text-[var(--color-text-secondary)]">{fmtMmss(acd(r))}</td>
                    <td className="py-1.5 text-right text-[var(--color-text-secondary)]">{fmtInt(r.billsec / 60)}</td>
                  </tr>
                )
              })}
            </tbody>
          </table>
        </div>
      )}
    </div>
  )
}
