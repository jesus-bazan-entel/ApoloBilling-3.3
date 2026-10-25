import { useEffect } from 'react'
import { CalendarClock, RefreshCw } from 'lucide-react'
import { cn } from '../lib/utils'
import { RANGE_GROUPS, type RangeKey } from '../lib/cdrTimeRanges'

interface QuickRangeBarProps {
  value: RangeKey
  onSelect: (key: Exclude<RangeKey, 'custom'>) => void
  onCustom: () => void
  /** Resolved range of the data on screen, e.g. "19/10/2026 00:00 → ahora · GMT-5" */
  description?: string
  /** Auto-refresh only makes sense for ranges that end "now" */
  openEnded: boolean
  autoRefresh: boolean
  onToggleAutoRefresh: () => void
  refreshing?: boolean
}

const HOTKEYS = Object.fromEntries(
  RANGE_GROUPS.flatMap((g) => g.options)
    .filter((o) => o.hotkey)
    .map((o) => [o.hotkey as string, o.key])
)

const isTyping = (target: EventTarget | null) =>
  target instanceof HTMLElement &&
  (target.isContentEditable || ['INPUT', 'TEXTAREA', 'SELECT'].includes(target.tagName))

export default function QuickRangeBar({
  value,
  onSelect,
  onCustom,
  description,
  openEnded,
  autoRefresh,
  onToggleAutoRefresh,
  refreshing,
}: QuickRangeBarProps) {
  // 1-4 jump to the live ranges, unless the operator is typing in a field
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (e.ctrlKey || e.metaKey || e.altKey || isTyping(e.target)) return
      const key = HOTKEYS[e.key]
      if (key) onSelect(key)
    }
    window.addEventListener('keydown', onKey)
    return () => window.removeEventListener('keydown', onKey)
  }, [onSelect])

  const button = (active: boolean) =>
    cn(
      'px-3 py-1.5 rounded-lg text-sm font-medium border transition-colors whitespace-nowrap',
      active
        ? 'bg-[#ff5a0a] border-[#ff5a0a] text-white shadow-sm'
        : 'border-[var(--color-border-primary)] text-[var(--color-text-secondary)] hover:bg-[var(--color-bg-secondary)] hover:text-[var(--color-text-primary)]'
    )

  return (
    <div className="bg-[var(--color-bg-card)] rounded-xl shadow-sm border border-[var(--color-border-primary)] p-4 space-y-3">
      <div className="flex flex-wrap items-start gap-x-6 gap-y-3">
        {RANGE_GROUPS.map((group) => (
          <div key={group.title} className="space-y-1.5">
            <div className="text-xs font-semibold uppercase tracking-wide text-[var(--color-text-tertiary)]">
              {group.title}
            </div>
            <div className="flex flex-wrap gap-1.5">
              {group.options.map((o) => (
                <button
                  key={o.key}
                  type="button"
                  onClick={() => onSelect(o.key)}
                  className={button(value === o.key)}
                  title={o.hotkey ? `Atajo: ${o.hotkey}` : undefined}
                >
                  {o.label}
                </button>
              ))}
            </div>
          </div>
        ))}
        <div className="space-y-1.5">
          <div className="text-xs font-semibold uppercase tracking-wide text-[var(--color-text-tertiary)]">
            Rango
          </div>
          <button
            type="button"
            onClick={onCustom}
            className={cn(button(value === 'custom'), 'flex items-center')}
          >
            <CalendarClock className="w-4 h-4 mr-1.5" />
            Personalizado…
          </button>
        </div>
      </div>

      <div className="flex flex-wrap items-center justify-between gap-2 pt-3 border-t border-[var(--color-border-primary)]">
        <span className="text-sm font-mono text-[var(--color-text-secondary)]">
          {description ?? 'Calculando rango…'}
        </span>
        <button
          type="button"
          onClick={onToggleAutoRefresh}
          disabled={!openEnded}
          className={cn(
            'flex items-center text-xs font-medium px-2.5 py-1 rounded-full border transition-colors',
            !openEnded
              ? 'opacity-50 cursor-not-allowed border-[var(--color-border-primary)] text-[var(--color-text-tertiary)]'
              : autoRefresh
              ? 'bg-[#fff3ed] border-[#ffbb9a] text-[#d8450b]'
              : 'border-[var(--color-border-primary)] text-[var(--color-text-secondary)] hover:bg-[var(--color-bg-secondary)]'
          )}
          title={openEnded ? 'Volver a consultar cada 30 s' : 'El rango ya terminó: no hay CDRs nuevos'}
        >
          <RefreshCw className={cn('w-3.5 h-3.5 mr-1.5', refreshing && openEnded && autoRefresh && 'animate-spin')} />
          {openEnded && autoRefresh ? 'Actualiza cada 30 s' : 'Auto-actualizar'}
        </button>
      </div>
    </div>
  )
}
