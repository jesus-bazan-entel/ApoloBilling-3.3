import type { LucideIcon } from 'lucide-react'
import { cn } from '../lib/utils'

interface StatCardProps {
  title: string
  value: string | number
  subtitle?: string
  icon: LucideIcon
  color?: 'blue' | 'green' | 'yellow' | 'red' | 'purple'
  trend?: {
    value: number
    isPositive: boolean
  }
}

const textColorClasses = {
  blue: 'text-blue-600',
  green: 'text-green-600',
  yellow: 'text-yellow-600',
  red: 'text-red-600',
  purple: 'text-purple-600',
}

const bgGradientClasses = {
  blue: 'from-blue-500 to-blue-600',
  green: 'from-green-500 to-green-600',
  yellow: 'from-yellow-500 to-yellow-600',
  red: 'from-red-500 to-red-600',
  purple: 'from-purple-500 to-purple-600',
}

export default function StatCard({
  title,
  value,
  subtitle,
  icon: Icon,
  color = 'blue',
  trend,
}: StatCardProps) {
  return (
    <div className="group bg-white rounded-xl shadow-sm hover:shadow-md p-6 border border-slate-200 transition-all duration-200 hover:border-slate-300">
      <div className="flex items-start justify-between">
        <div className="flex-1">
          <p className="text-sm font-medium text-slate-500 mb-1">{title}</p>
          <p className={cn('text-3xl font-bold mt-2 transition-colors', textColorClasses[color])}>
            {value}
          </p>
          {subtitle && (
            <p className="text-sm text-slate-500 mt-2">{subtitle}</p>
          )}
          {trend && (
            <div
              className={cn(
                'inline-flex items-center mt-2 text-sm font-medium px-2 py-1 rounded-full',
                trend.isPositive ? 'bg-green-100 text-green-700' : 'bg-red-100 text-red-700'
              )}
            >
              <span className="mr-1">{trend.isPositive ? '↑' : '↓'}</span>
              <span>{Math.abs(trend.value)}%</span>
            </div>
          )}
        </div>
        <div className={cn(
          'bg-gradient-to-br p-3 rounded-xl shadow-sm transition-transform group-hover:scale-110 duration-200',
          `bg-gradient-to-br ${bgGradientClasses[color]}`
        )}>
          <Icon className="w-6 h-6 text-white" />
        </div>
      </div>
    </div>
  )
}
