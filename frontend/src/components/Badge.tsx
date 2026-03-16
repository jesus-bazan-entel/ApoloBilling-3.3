import { cn } from '../lib/utils'

interface BadgeProps {
  children: React.ReactNode
  variant?: 'success' | 'warning' | 'error' | 'info' | 'default'
  size?: 'sm' | 'md'
  className?: string
}

const variantClasses = {
  success: 'bg-green-100 text-green-800 ring-1 ring-green-600/20',
  warning: 'bg-yellow-100 text-yellow-800 ring-1 ring-yellow-600/20',
  error: 'bg-red-100 text-red-800 ring-1 ring-red-600/20',
  info: 'bg-blue-100 text-blue-800 ring-1 ring-blue-600/20',
  default: 'bg-slate-100 text-slate-800 ring-1 ring-slate-600/20',
}

const sizeClasses = {
  sm: 'px-2 py-0.5 text-xs',
  md: 'px-2.5 py-1 text-sm',
}

export default function Badge({
  children,
  variant = 'default',
  size = 'sm',
  className,
}: BadgeProps) {
  return (
    <span
      className={cn(
        'inline-flex items-center font-medium rounded-full transition-colors',
        variantClasses[variant],
        sizeClasses[size],
        className
      )}
    >
      {children}
    </span>
  )
}
