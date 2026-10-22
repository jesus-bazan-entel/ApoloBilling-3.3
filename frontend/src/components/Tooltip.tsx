import type { ReactNode } from 'react'
import * as RadixTooltip from '@radix-ui/react-tooltip'

interface TooltipProps {
  content: ReactNode
  children: ReactNode
  side?: 'top' | 'right' | 'bottom' | 'left'
}

/** Hover/focus help bubble. Wrap any trigger element (usually an info icon). */
export default function Tooltip({ content, children, side = 'top' }: TooltipProps) {
  return (
    <RadixTooltip.Provider delayDuration={150}>
      <RadixTooltip.Root>
        <RadixTooltip.Trigger asChild>
          <span tabIndex={0} className="inline-flex cursor-help outline-none">
            {children}
          </span>
        </RadixTooltip.Trigger>
        <RadixTooltip.Portal>
          <RadixTooltip.Content
            side={side}
            sideOffset={6}
            className="z-50 max-w-sm rounded-lg border px-3 py-2 text-xs shadow-lg"
            style={{
              backgroundColor: 'var(--color-bg-secondary)',
              borderColor: 'var(--color-border-primary)',
              color: 'var(--color-text-primary)',
            }}
          >
            {content}
            <RadixTooltip.Arrow style={{ fill: 'var(--color-bg-secondary)' }} />
          </RadixTooltip.Content>
        </RadixTooltip.Portal>
      </RadixTooltip.Root>
    </RadixTooltip.Provider>
  )
}
