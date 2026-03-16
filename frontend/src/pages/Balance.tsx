import { useState } from 'react'
import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query'
import { fetchAccounts, topupAccount } from '../api/client'
import DataTable from '../components/DataTable'
import Badge from '../components/Badge'
import {
  DollarSign,
  Plus,
  X,
  Search,
  TrendingUp,
  Users,
} from 'lucide-react'
import type { Account } from '../types'

export default function BalancePage() {
  const [showRechargeModal, setShowRechargeModal] = useState(false)
  const [selectedAccount, setSelectedAccount] = useState<Account | null>(null)
  const [searchTerm, setSearchTerm] = useState('')
  const queryClient = useQueryClient()

  const { data: accounts = [], isLoading } = useQuery({
    queryKey: ['accounts'],
    queryFn: fetchAccounts,
    refetchInterval: 10000,
  })

  const rechargeMutation = useMutation({
    mutationFn: ({ id, amount, reason }: { id: number; amount: number; reason?: string }) =>
      topupAccount(id, amount, reason),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['accounts'] })
      setShowRechargeModal(false)
      setSelectedAccount(null)
    },
  })

  const handleRecharge = (account: Account) => {
    setSelectedAccount(account)
    setShowRechargeModal(true)
  }

  // Filter accounts by search term
  const filteredAccounts = accounts.filter(
    (account: Account) =>
      account.account_number.toLowerCase().includes(searchTerm.toLowerCase()) ||
      (account.customer_phone?.toLowerCase().includes(searchTerm.toLowerCase()) ?? false)
  )

  const columns = [
    {
      key: 'account_number',
      header: 'Número de Cuenta',
      render: (account: Account) => (
        <span className="font-mono font-medium text-slate-900">
          {account.account_number}
        </span>
      ),
    },
    {
      key: 'customer_phone',
      header: 'Teléfono',
      render: (account: Account) => (
        <span className="text-slate-900">{account.customer_phone || '-'}</span>
      ),
    },
    {
      key: 'account_type',
      header: 'Tipo',
      render: (account: Account) => (
        <Badge variant={account.account_type.toLowerCase() === 'prepaid' ? 'info' : 'warning'}>
          {account.account_type}
        </Badge>
      ),
    },
    {
      key: 'balance',
      header: 'Saldo Actual',
      render: (account: Account) => {
        const balance = Number(account.balance) || 0
        return (
          <span
            className={`font-mono font-bold text-lg ${
              balance > 10
                ? 'text-green-600'
                : balance > 5
                ? 'text-yellow-600'
                : 'text-red-600'
            }`}
          >
            S/{balance.toFixed(2)}
          </span>
        )
      },
      className: 'text-right',
    },
    {
      key: 'status',
      header: 'Estado',
      render: (account: Account) => (
        <Badge variant={account.status.toLowerCase() === 'active' ? 'success' : 'error'}>
          {account.status}
        </Badge>
      ),
    },
    {
      key: 'actions',
      header: 'Acciones',
      render: (account: Account) => (
        <button
          onClick={() => handleRecharge(account)}
          className="flex items-center text-blue-600 hover:text-blue-700 font-medium"
          disabled={account.status.toLowerCase() !== 'active'}
        >
          <Plus className="w-4 h-4 mr-1" />
          Recargar
        </button>
      ),
    },
  ]

  // Summary stats
  const totalBalance = accounts.reduce((sum: number, a: Account) => sum + (Number(a.balance) || 0), 0)
  const activeAccounts = accounts.filter((a: Account) => a.status?.toLowerCase() === 'active').length
  const lowBalanceAccounts = accounts.filter(
    (a: Account) => (Number(a.balance) || 0) < 5 && a.status?.toLowerCase() === 'active'
  ).length

  return (
    <div className="space-y-6">
      <div className="flex items-center justify-between">
        <div>
          <h1 className="text-2xl font-bold text-slate-900">
            Gestión de Saldos
          </h1>
          <p className="text-slate-500">
            Administra saldos de cuentas y realiza recargas
          </p>
        </div>
      </div>

      {/* Summary Cards */}
      <div className="grid grid-cols-1 md:grid-cols-4 gap-4">
        <div className="bg-white rounded-lg shadow-sm p-4 border border-slate-200">
          <div className="flex items-center justify-between">
            <div>
              <p className="text-sm text-slate-500">Total Cuentas</p>
              <p className="text-2xl font-bold text-slate-900">{accounts.length}</p>
            </div>
            <Users className="w-8 h-8 text-blue-500" />
          </div>
        </div>
        <div className="bg-white rounded-lg shadow-sm p-4 border border-slate-200">
          <div className="flex items-center justify-between">
            <div>
              <p className="text-sm text-slate-500">Activas</p>
              <p className="text-2xl font-bold text-green-600">{activeAccounts}</p>
            </div>
            <Users className="w-8 h-8 text-green-500" />
          </div>
        </div>
        <div className="bg-white rounded-lg shadow-sm p-4 border border-slate-200">
          <div className="flex items-center justify-between">
            <div>
              <p className="text-sm text-slate-500">Saldo Bajo (&lt; S/5)</p>
              <p className="text-2xl font-bold text-red-600">
                {lowBalanceAccounts}
              </p>
            </div>
            <TrendingUp className="w-8 h-8 text-red-500" />
          </div>
        </div>
        <div className="bg-white rounded-lg shadow-sm p-4 border border-slate-200">
          <div className="flex items-center justify-between">
            <div>
              <p className="text-sm text-slate-500">Saldo Total</p>
              <p className="text-2xl font-bold text-green-600">
                S/{totalBalance.toFixed(2)}
              </p>
            </div>
            <DollarSign className="w-8 h-8 text-green-500" />
          </div>
        </div>
      </div>

      {/* Search Bar */}
      <div className="bg-white rounded-lg shadow-sm border border-slate-200 p-4">
        <div className="relative">
          <Search className="absolute left-3 top-1/2 transform -translate-y-1/2 w-5 h-5 text-slate-400" />
          <input
            type="text"
            value={searchTerm}
            onChange={(e) => setSearchTerm(e.target.value)}
            placeholder="Buscar por número de cuenta o teléfono..."
            className="w-full pl-10 pr-4 py-2 border border-slate-300 rounded-lg focus:ring-2 focus:ring-blue-500 focus:border-blue-500"
          />
        </div>
      </div>

      {/* Low Balance Alert */}
      {lowBalanceAccounts > 0 && (
        <div className="bg-red-50 border border-red-200 rounded-lg p-4">
          <div className="flex items-center">
            <TrendingUp className="w-5 h-5 text-red-600 mr-2" />
            <span className="text-red-800 font-medium">
              Atención: {lowBalanceAccounts} cuenta{lowBalanceAccounts > 1 ? 's' : ''}{' '}
              con saldo bajo (menos de S/5.00)
            </span>
          </div>
        </div>
      )}

      {/* Accounts Table */}
      <DataTable
        columns={columns}
        data={filteredAccounts}
        loading={isLoading}
        emptyMessage={
          searchTerm
            ? 'No se encontraron cuentas con ese criterio de búsqueda'
            : 'No hay cuentas registradas'
        }
      />

      {/* Recharge Modal */}
      {showRechargeModal && selectedAccount && (
        <RechargeModal
          account={selectedAccount}
          onClose={() => {
            setShowRechargeModal(false)
            setSelectedAccount(null)
          }}
          onSubmit={(amount, reason) => {
            rechargeMutation.mutate({ id: selectedAccount.id, amount, reason })
          }}
          isLoading={rechargeMutation.isPending}
        />
      )}
    </div>
  )
}

interface RechargeModalProps {
  account: Account
  onClose: () => void
  onSubmit: (amount: number, reason?: string) => void
  isLoading: boolean
}

function RechargeModal({
  account,
  onClose,
  onSubmit,
  isLoading,
}: RechargeModalProps) {
  const [amount, setAmount] = useState<number>(0)
  const [reason, setReason] = useState('')
  const [selectedAmount, setSelectedAmount] = useState<number | null>(null)

  const quickAmounts = [10, 20, 50, 100, 200, 500]

  const handleSubmit = (e: React.FormEvent) => {
    e.preventDefault()
    if (amount > 0) {
      onSubmit(amount, reason || undefined)
    }
  }

  const handleQuickAmount = (value: number) => {
    setSelectedAmount(value)
    setAmount(value)
  }

  const currentBalance = Number(account.balance) || 0
  const newBalance = currentBalance + amount

  return (
    <div className="fixed inset-0 bg-black bg-opacity-50 flex items-center justify-center z-50">
      <div className="bg-white rounded-xl shadow-xl max-w-lg w-full mx-4">
        <div className="flex items-center justify-between p-6 border-b border-slate-200">
          <h2 className="text-xl font-bold text-slate-900">Recargar Saldo</h2>
          <button
            onClick={onClose}
            className="text-slate-400 hover:text-slate-600"
          >
            <X className="w-6 h-6" />
          </button>
        </div>

        <form onSubmit={handleSubmit} className="p-6 space-y-4">
          {/* Account Info */}
          <div className="bg-slate-50 rounded-lg p-4">
            <div className="grid grid-cols-2 gap-4">
              <div>
                <p className="text-sm text-slate-500">Número de Cuenta</p>
                <p className="font-mono font-medium text-slate-900">
                  {account.account_number}
                </p>
              </div>
              <div>
                <p className="text-sm text-slate-500">Tipo</p>
                <p className="font-medium text-slate-900">{account.account_type}</p>
              </div>
              <div>
                <p className="text-sm text-slate-500">Saldo Actual</p>
                <p className="text-xl font-bold text-slate-900">
                  S/{currentBalance.toFixed(2)}
                </p>
              </div>
              {amount > 0 && (
                <div>
                  <p className="text-sm text-slate-500">Nuevo Saldo</p>
                  <p className="text-xl font-bold text-green-600">
                    S/{newBalance.toFixed(2)}
                  </p>
                </div>
              )}
            </div>
          </div>

          {/* Quick Amount Buttons */}
          <div>
            <label className="block text-sm font-medium text-slate-700 mb-2">
              Montos Rápidos
            </label>
            <div className="grid grid-cols-3 gap-2">
              {quickAmounts.map((value) => (
                <button
                  key={value}
                  type="button"
                  onClick={() => handleQuickAmount(value)}
                  className={`px-4 py-2 border rounded-lg font-medium transition-colors ${
                    selectedAmount === value
                      ? 'bg-blue-600 text-white border-blue-600'
                      : 'border-slate-300 text-slate-700 hover:bg-slate-50'
                  }`}
                >
                  S/{value}
                </button>
              ))}
            </div>
          </div>

          {/* Custom Amount */}
          <div>
            <label className="block text-sm font-medium text-slate-700 mb-1">
              Monto Personalizado
            </label>
            <div className="relative">
              <span className="absolute left-3 top-1/2 transform -translate-y-1/2 text-slate-500 font-medium">
                S/
              </span>
              <input
                type="number"
                step="0.01"
                min="0.01"
                required
                value={amount || ''}
                onChange={(e) => {
                  setAmount(parseFloat(e.target.value) || 0)
                  setSelectedAmount(null)
                }}
                className="w-full pl-8 pr-3 py-2 border border-slate-300 rounded-lg focus:ring-2 focus:ring-blue-500 focus:border-blue-500"
                placeholder="0.00"
              />
            </div>
          </div>

          {/* Reason */}
          <div>
            <label className="block text-sm font-medium text-slate-700 mb-1">
              Motivo (opcional)
            </label>
            <input
              type="text"
              value={reason}
              onChange={(e) => setReason(e.target.value)}
              className="w-full px-3 py-2 border border-slate-300 rounded-lg focus:ring-2 focus:ring-blue-500 focus:border-blue-500"
              placeholder="Ej: Recarga mensual"
            />
          </div>

          {/* Action Buttons */}
          <div className="flex justify-end space-x-3 pt-4 border-t border-slate-200">
            <button
              type="button"
              onClick={onClose}
              className="px-4 py-2 border border-slate-300 text-slate-700 rounded-lg hover:bg-slate-50"
            >
              Cancelar
            </button>
            <button
              type="submit"
              disabled={isLoading || amount <= 0}
              className="px-4 py-2 bg-green-600 text-white rounded-lg hover:bg-green-700 disabled:opacity-50 flex items-center"
            >
              <Plus className="w-4 h-4 mr-2" />
              {isLoading ? 'Procesando...' : 'Recargar'}
            </button>
          </div>
        </form>
      </div>
    </div>
  )
}
