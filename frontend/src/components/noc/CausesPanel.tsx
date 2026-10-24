import { AlertTriangle } from 'lucide-react'
import type { NocCause, NocCompare } from '../../types'
import { CATEGORY_LABELS, causeLabel, compareLabel, fmtInt, pctChange } from './format'

/** Unanswered calls by hangup cause; network causes are the ones that cost NER. */
export default function CausesPanel({ causes, compare }: { causes: NocCause[]; compare: NocCompare }) {
  const total = causes.reduce((a, c) => a + c.today, 0)
  const max = Math.max(...causes.map(c => c.today), 1)

  return (
    <div className="bg-[var(--color-bg-card)] rounded-xl border border-[var(--color-border-primary)] p-5">
      <h2 className="text-base font-semibold text-[var(--color-text-primary)]">Llamadas no completadas por causa</h2>
      <p className="text-xs text-[var(--color-text-tertiary)] mt-0.5 mb-4">
        Hoy · {fmtInt(total)} sin contestar · variación vs {compareLabel(compare)} a la misma hora
      </p>

      {causes.length === 0 ? (
        <p className="text-sm text-[var(--color-text-tertiary)] py-8 text-center">Sin llamadas no completadas hoy</p>
      ) : (
        <ul className="space-y-3">
          {causes.map(c => {
            const ch = pctChange(c.today, c.compare)
            const network = c.category === 'network'
            return (
              <li key={c.cause} title={c.cause}>
                <div className="flex items-center justify-between gap-3 text-sm">
                  <span className="flex items-center gap-1.5 min-w-0 text-[var(--color-text-primary)]">
                    {network && (
                      <AlertTriangle className="w-3.5 h-3.5 shrink-0" style={{ color: 'var(--viz-status-critical)' }} aria-label="Falla de red" />
                    )}
                    <span className="truncate">{causeLabel(c.cause)}</span>
                    <span className="text-[11px] text-[var(--color-text-tertiary)] shrink-0">· {CATEGORY_LABELS[c.category]}</span>
                  </span>
                  <span className="flex items-baseline gap-2 shrink-0 tabular-nums">
                    <span className="font-semibold text-[var(--color-text-primary)]">{fmtInt(c.today)}</span>
                    <span className="text-xs text-[var(--color-text-tertiary)] w-12 text-right">
                      {total ? `${((100 * c.today) / total).toFixed(1)}%` : '—'}
                    </span>
                    <span className="text-xs text-[var(--color-text-tertiary)] w-14 text-right">
                      {ch == null ? '—' : `${ch > 0 ? '+' : ''}${ch.toFixed(0)}%`}
                    </span>
                  </span>
                </div>
                <div className="mt-1 h-1.5 rounded-full bg-[var(--viz-grid)]" aria-hidden="true">
                  <div
                    className="h-1.5 rounded-full"
                    style={{
                      width: `${Math.max((100 * c.today) / max, c.today ? 1 : 0)}%`,
                      background: network ? 'var(--viz-status-critical)' : 'var(--viz-compare)',
                    }}
                  />
                </div>
              </li>
            )
          })}
        </ul>
      )}
      <p className="text-[11px] text-[var(--color-text-tertiary)] mt-4">
        Las causas marcadas como «Red» son fallas de la red (timeouts, congestión, destino caído) y son las únicas que restan al NER.
      </p>
    </div>
  )
}
