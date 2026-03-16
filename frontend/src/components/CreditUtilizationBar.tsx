// Credit Utilization Bar Component
//
// Visual progress bar showing percentage of credit used for postpaid accounts.
// Color-coded based on utilization level.

import { getUtilizationColor, formatUtilization } from '../lib/accountHelpers'

interface CreditUtilizationBarProps {
  utilizationPercent: number
  showLabel?: boolean
}

export default function CreditUtilizationBar({
  utilizationPercent,
  showLabel = true,
}: CreditUtilizationBarProps) {
  const percent = Math.min(utilizationPercent, 100)
  const bgColor = getUtilizationColor(percent)

  return (
    <div className="w-full">
      <div className="h-2 bg-slate-200 rounded-full overflow-hidden">
        <div
          className={`h-full ${bgColor} transition-all duration-300`}
          style={{ width: `${percent}%` }}
        />
      </div>
      {showLabel && (
        <div className="text-xs text-slate-600 mt-1 text-right">
          {formatUtilization(percent)} usado
        </div>
      )}
    </div>
  )
}
