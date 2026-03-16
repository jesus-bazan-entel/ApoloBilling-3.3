import type { Account, AccountDisplayInfo } from '../types'

/**
 * Transform account data into display-friendly format
 * IMPORTANT: NEVER shows negative values to the user
 */
export function getAccountDisplayInfo(account: Account): AccountDisplayInfo {
  const isPrepaid = account.account_type?.toLowerCase() === 'prepaid'
  const balance = Number(account.balance) || 0
  const creditLimit = Number(account.credit_limit) || 0
  // availableBalance del backend se usa solo para prepago
  // Para postpago calculamos localmente: creditLimit - consumedCredit

  if (isPrepaid) {
    // PREPAGO: solo mostrar saldo disponible (siempre positivo)
    const currentBalance = Math.max(0, balance)

    return {
      isPrepaid: true,
      currentBalance,
      displayValue: `S/${currentBalance.toFixed(2)}`,
      isLowBalance: currentBalance < 10,
      balanceColor: currentBalance > 50 ? 'success' : currentBalance > 10 ? 'warning' : 'error',
    }
  } else {
    // POSTPAGO: mostrar consumido/límite/disponible
    // El balance empieza en credit_limit y se va restando con cada llamada
    // consumido = lo que se ha gastado = credit_limit - balance
    // disponible = lo que queda por gastar = balance

    // Si backend ya envió consumed_credit, usar ese valor
    // Sino, calcular: consumido = credit_limit - balance (si balance <= credit_limit)
    const consumedCredit = account.consumed_credit !== undefined
      ? Number(account.consumed_credit)
      : (balance <= creditLimit ? Math.max(0, creditLimit - balance) : 0)

    // Disponible = el balance actual (lo que queda)
    const availableCredit = Math.max(0, Math.min(balance, creditLimit))

    const utilization = account.utilization_percent !== undefined
      ? Number(account.utilization_percent)
      : (creditLimit > 0 ? (consumedCredit / creditLimit) * 100 : 0)

    return {
      isPrepaid: false,
      consumedCredit,
      creditLimit,
      availableCredit,
      utilizationPercent: Math.min(100, utilization),
      displayValue: `Consumido: S/${consumedCredit.toFixed(2)} | Disponible: S/${availableCredit.toFixed(2)}`,
      isLowBalance: availableCredit < creditLimit * 0.2,  // Menos del 20%
      balanceColor: utilization < 70 ? 'success' : utilization < 90 ? 'warning' : 'error',
    }
  }
}

/**
 * Get color variant for balance display
 * Maps to Badge component variants
 */
export function getBalanceColorVariant(account: Account): 'success' | 'warning' | 'error' {
  const info = getAccountDisplayInfo(account)
  return info.balanceColor
}

/**
 * Get utilization bar color (for CreditUtilizationBar component)
 */
export function getUtilizationColor(utilizationPercent: number): string {
  if (utilizationPercent >= 90) return 'bg-red-500'
  if (utilizationPercent >= 70) return 'bg-yellow-500'
  return 'bg-green-500'
}

/**
 * Format balance for display (always positive)
 */
export function formatBalance(amount: number): string {
  return `S/${Math.max(0, amount).toFixed(2)}`
}

/**
 * Check if account needs attention (low balance or high utilization)
 */
export function needsAttention(account: Account): boolean {
  const info = getAccountDisplayInfo(account)
  return info.isLowBalance || info.balanceColor === 'error'
}

/**
 * Format utilization percentage for display
 */
export function formatUtilization(percent: number): string {
  return `${Math.min(100, Math.round(percent))}%`
}
