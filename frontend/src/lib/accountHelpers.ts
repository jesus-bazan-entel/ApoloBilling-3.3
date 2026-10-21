/**
 * Get utilization bar color (for CreditUtilizationBar component)
 */
export function getUtilizationColor(utilizationPercent: number): string {
  if (utilizationPercent >= 90) return 'bg-red-500'
  if (utilizationPercent >= 70) return 'bg-yellow-500'
  return 'bg-green-500'
}

/**
 * Format utilization percentage for display
 */
export function formatUtilization(percent: number): string {
  return `${Math.min(100, Math.round(percent))}%`
}
