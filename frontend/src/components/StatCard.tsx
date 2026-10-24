import type { LucideIcon } from 'lucide-react'
import { cn } from '../lib/utils'
import { TrendingUp, TrendingDown, Minus } from 'lucide-react'

interface StatCardProps {
  title: string
  value: string | number
  subtitle?: string
  icon: LucideIcon
  color?: 'blue' | 'green' | 'yellow' | 'red' | 'purple' | 'cyan' | 'indigo'
  trend?: {
    value: number
    isPositive: boolean
  }
  sparklineData?: number[]
  className?: string
}

const colorConfig = {
  blue: {
    text: 'text-[#d8450b]',
    bg: 'bg-[#fff3ed]',
    iconBg: 'bg-gradient-to-br from-[#ff5a0a] to-[#d8450b]',
    sparkline: '#ff5a0a',
    border: 'border-[#ffdac8]',
    ring: 'ring-[#ff5a0a]/20',
  },
  green: {
    text: 'text-emerald-600',
    bg: 'bg-emerald-50',
    iconBg: 'bg-gradient-to-br from-emerald-500 to-emerald-600',
    sparkline: '#10b981',
    border: 'border-emerald-100',
    ring: 'ring-emerald-500/20',
  },
  yellow: {
    text: 'text-amber-600',
    bg: 'bg-amber-50',
    iconBg: 'bg-gradient-to-br from-amber-500 to-amber-600',
    sparkline: '#f59e0b',
    border: 'border-amber-100',
    ring: 'ring-amber-500/20',
  },
  red: {
    text: 'text-red-600',
    bg: 'bg-red-50',
    iconBg: 'bg-gradient-to-br from-red-500 to-red-600',
    sparkline: '#ef4444',
    border: 'border-red-100',
    ring: 'ring-red-500/20',
  },
  purple: {
    text: 'text-[#d8450b]',
    bg: 'bg-[#fff3ed]',
    iconBg: 'bg-gradient-to-br from-[#ff7736] to-[#ff5a0a]',
    sparkline: '#ff7736',
    border: 'border-[#ffdac8]',
    ring: 'ring-[#ff5a0a]/20',
  },
  cyan: {
    text: 'text-[#d8450b]',
    bg: 'bg-[#fff3ed]',
    iconBg: 'bg-gradient-to-br from-[#ff5a0a] to-[#d8450b]',
    sparkline: '#ff5a0a',
    border: 'border-[#ffdac8]',
    ring: 'ring-[#ff5a0a]/20',
  },
  indigo: {
    text: 'text-[#301d19]',
    bg: 'bg-[#fff3ed]',
    iconBg: 'bg-gradient-to-br from-[#4b4b4b] to-[#301d19]',
    sparkline: '#301d19',
    border: 'border-[#ffdac8]',
    ring: 'ring-[#301d19]/20',
  },
}

function MiniSparkline({ data, color }: { data: number[]; color: string }) {
  if (!data || data.length < 2) return null

  const max = Math.max(...data)
  const min = Math.min(...data)
  const range = max - min || 1
  const width = 80
  const height = 32
  const padding = 2

  const points = data.map((value, index) => {
    const x = (index / (data.length - 1)) * (width - padding * 2) + padding
    const y = height - padding - ((value - min) / range) * (height - padding * 2)
    return `${x},${y}`
  }).join(' ')

  return (
    <svg width={width} height={height} className="opacity-60 group-hover:opacity-100 transition-opacity">
      <defs>
        <linearGradient id={`sparkGradient-${color.replace('#', '')}`} x1="0%" y1="0%" x2="0%" y2="100%">
          <stop offset="0%" stopColor={color} stopOpacity="0.3" />
          <stop offset="100%" stopColor={color} stopOpacity="0" />
        </linearGradient>
      </defs>
      <polyline
        fill="none"
        stroke={color}
        strokeWidth="2"
        strokeLinecap="round"
        strokeLinejoin="round"
        points={points}
      />
      <polygon
        fill={`url(#sparkGradient-${color.replace('#', '')})`}
        points={`${padding},${height - padding} ${points} ${width - padding},${height - padding}`}
      />
    </svg>
  )
}

export default function StatCard({
  title,
  value,
  subtitle,
  icon: Icon,
  color = 'blue',
  trend,
  sparklineData,
  className,
}: StatCardProps) {
  const config = colorConfig[color]

  return (
    <div
      className={cn(
        'group relative bg-[var(--color-bg-card)] rounded-2xl shadow-sm border border-[var(--color-border-primary)]',
        'p-6 transition-all duration-300 ease-out',
        'hover:shadow-lg hover:border-[var(--color-border-secondary)]',
        'hover:-translate-y-0.5',
        className
      )}
    >
      {/* Background accent */}
      <div
        className={cn(
          'absolute inset-0 rounded-2xl opacity-0 group-hover:opacity-100 transition-opacity duration-300',
          config.bg
        )}
        style={{ opacity: 0.03 }}
      />

      <div className="relative flex items-start justify-between">
        <div className="flex-1 min-w-0">
          <div className="flex items-center gap-2 mb-1">
            <p className="text-sm font-medium text-[var(--color-text-tertiary)] truncate">{title}</p>
          </div>

          <div className="flex items-baseline gap-3">
            <p className={cn(
              'text-3xl font-bold tracking-tight transition-colors',
              config.text
            )}>
              {value}
            </p>

            {trend && (
              <div
                className={cn(
                  'inline-flex items-center gap-1 text-xs font-semibold px-2 py-0.5 rounded-full',
                  trend.isPositive
                    ? 'bg-emerald-100 text-emerald-700'
                    : trend.value === 0
                    ? 'bg-[var(--color-bg-tertiary)] text-[var(--color-text-secondary)]'
                    : 'bg-red-100 text-red-700'
                )}
              >
                {trend.isPositive ? (
                  <TrendingUp className="w-3 h-3" />
                ) : trend.value === 0 ? (
                  <Minus className="w-3 h-3" />
                ) : (
                  <TrendingDown className="w-3 h-3" />
                )}
                <span>{Math.abs(trend.value)}%</span>
              </div>
            )}
          </div>

          {subtitle && (
            <p className="text-sm text-[var(--color-text-tertiary)] mt-2 truncate">{subtitle}</p>
          )}

          {sparklineData && sparklineData.length > 1 && (
            <div className="mt-3">
              <MiniSparkline data={sparklineData} color={config.sparkline} />
            </div>
          )}
        </div>

        <div className={cn(
          'flex-shrink-0 p-3 rounded-xl shadow-sm',
          'transition-all duration-300 ease-out',
          'group-hover:scale-110 group-hover:shadow-md',
          config.iconBg
        )}>
          <Icon className="w-6 h-6 text-white" />
        </div>
      </div>
    </div>
  )
}
