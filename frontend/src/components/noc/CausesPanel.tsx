import { AlertTriangle } from 'lucide-react'
import type { NocCause, NocCompare } from '../../types'
import { CATEGORY_LABELS, CAUSE_NOTES, causeLabel, causeSipCode, compareLabel, fmtInt, pctChange } from './format'

const dayTime = (iso: string) =>
  new Date(iso).toLocaleString('es-PE', { day: '2-digit', month: '2-digit', hour: '2-digit', minute: '2-digit', hour12: false })

/**
 * Unanswered calls by hangup cause; network causes are the ones that cost NER.
 * `directSince` / `compareEnd`: when the comparison window predates the first
 * direct-routing CDR, causes only that source reports (it maps the real SIP
 * code; FreeSWITCH gives Q.850) have no base to compare with.
 */
export default function CausesPanel({
  causes,
  compare,
  directSince,
  compareEnd,
}: {
  causes: NocCause[]
  compare: NocCompare
  directSince: string | null
  compareEnd: Date
}) {
  const total = causes.reduce((a, c) => a + c.today, 0)
  const max = Math.max(...causes.map(c => c.today), 1)
  const directIsNew = directSince != null && new Date(directSince) > compareEnd

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
            const sip = causeSipCode(c.cause)
            // Only reported by a CDR source the comparison day did not have yet
            const noBase = ch == null && c.today > 0 && directIsNew && c.today_direct === c.today
            const lines = [
              `${c.cause}${sip ? ` · SIP ${sip}` : ''}`,
              CAUSE_NOTES[c.cause],
              c.today_direct > 0 && `${fmtInt(c.today_direct)} de ${fmtInt(c.today)} vienen de CDR directos de Kamailio (código SIP real).`,
              noBase &&
                `Sin base de comparación: los CDR directos de Kamailio existen desde el ${dayTime(directSince!)}. Por FreeSWITCH estos casos llegaban como «Liberada sin contestar» o «Usuario no registrado».`,
              ch == null && c.today > 0 && !noBase && `Sin casos ${compareLabel(compare)} a la misma hora.`,
            ].filter(Boolean)
            return (
              <li key={c.cause} title={lines.join('\n')}>
                <div className="flex items-center justify-between gap-3 text-sm">
                  <span className="flex items-center gap-1.5 min-w-0 text-[var(--color-text-primary)]">
                    {network && (
                      <AlertTriangle className="w-3.5 h-3.5 shrink-0" style={{ color: 'var(--viz-status-critical)' }} aria-label="Falla de red" />
                    )}
                    {sip && (
                      <span className="font-mono text-[11px] px-1 rounded bg-[var(--color-bg-tertiary)] text-[var(--color-text-secondary)] shrink-0 tabular-nums">
                        {sip}
                      </span>
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
                      {ch == null ? (c.today > 0 ? (noBase ? 'sin base' : 'nuevo') : '—') : `${ch > 0 ? '+' : ''}${ch.toFixed(0)}%`}
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
        El número junto a cada causa es la respuesta SIP que la produce (mismo mapeo que FreeSWITCH); pasa el cursor para ver la causa Q.850 y el criterio de clasificación.
        {directIsNew && (
          <>
            {' '}
            La comparación no es homogénea: desde el {dayTime(directSince!)} parte del tráfico entrante va directo Kamailio → troncal y su
            causa sale del código SIP real (486, 408, 480…), así que esas causas no tienen base {compareLabel(compare)}. Se normaliza sola
            cuando el día de comparación sea posterior a esa fecha.
          </>
        )}
      </p>
    </div>
  )
}
