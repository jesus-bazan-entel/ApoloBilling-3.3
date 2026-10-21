import { useState, useEffect } from 'react'
import { useQuery, useMutation, useQueryClient } from '@tanstack/react-query'
import { Shield, Save, Loader2, AlertCircle, CheckCircle, Info } from 'lucide-react'
import { fetchSettings, updateSetting, type SystemSetting } from '../api/client'

const SETTING_KEYS = {
  require_outbound_authorization: {
    label: 'Requerir autorización para salientes',
    description: 'Si está habilitado, las llamadas salientes requieren autorización de cuenta y facturación. Deshabilitar permite llamadas salientes sin verificación.',
    type: 'boolean' as const,
  },
  require_account_for_outbound: {
    label: 'Requerir cuenta asociada para salientes',
    description: 'Si está habilitado, las llamadas salientes desde endpoints (troncas/extensiones) sin account_id asociado son rechazadas.',
    type: 'boolean' as const,
  },
  require_account_for_inbound: {
    label: 'Requerir cuenta asociada para entrantes',
    description: 'Si está habilitado, las llamadas entrantes desde endpoints (troncas/extensiones) sin account_id asociado son rechazadas. La trunca de origen debe tener cuenta asociada.',
    type: 'boolean' as const,
  },
} as const

type SettingKey = keyof typeof SETTING_KEYS

export default function SettingsPage() {
  const queryClient = useQueryClient()
  const [settings, setSettings] = useState<Record<string, string>>({})
  const [saving, setSaving] = useState<string | null>(null)
  const [message, setMessage] = useState<{ type: 'success' | 'error', text: string } | null>(null)

  const { data, isLoading, error } = useQuery({
    queryKey: ['systemSettings'],
    queryFn: fetchSettings,
    staleTime: 30000,
  })

  useEffect(() => {
    if (data) {
      const mapped: Record<string, string> = {}
      data.forEach((s: SystemSetting) => {
        mapped[s.key] = s.value
      })
      setSettings(mapped)
    }
  }, [data])

  const updateSettingMutation = useMutation({
    mutationFn: ({ key, value }: { key: string; value: string }) => updateSetting(key, value),
    onSuccess: () => {
      queryClient.invalidateQueries({ queryKey: ['systemSettings'] })
      setSaving(null)
      setMessage({ type: 'success', text: 'Configuración guardada correctamente' })
      setTimeout(() => setMessage(null), 3000)
    },
    onError: (err: Error & { response?: { data?: { message?: string } } }) => {
      setSaving(null)
      setMessage({ type: 'error', text: err.response?.data?.message || err.message || 'Error al guardar' })
      setTimeout(() => setMessage(null), 5000)
    },
  })

  const handleChange = (key: SettingKey, value: string) => {
    setSettings(prev => ({ ...prev, [key]: value }))
  }

  const handleSave = (key: SettingKey) => {
    setSaving(key)
    updateSettingMutation.mutate({ key, value: settings[key] })
  }

  const getValue = (key: SettingKey) => settings[key] ?? 'false'

  if (isLoading) {
    return (
      <div className="flex items-center justify-center h-64">
        <Loader2 className="w-8 h-8 animate-spin text-[#0099D9]" />
      </div>
    )
  }

  if (error) {
    return (
      <div className="flex items-center justify-center h-64 text-red-400">
        <AlertCircle className="w-6 h-6 mr-2" />
        Error al cargar configuraciones
      </div>
    )
  }

  return (
    <div className="space-y-6">
      <div>
        <h1 className="text-3xl font-bold gradient-text tracking-tight">
          Configuración del Sistema
        </h1>
        <p className="text-sm text-[var(--color-text-secondary)] mt-1 font-medium">
          Configuraciones avanzadas de autorización y facturación
        </p>
      </div>

      {message && (
        <div className={`flex items-center gap-2 px-4 py-3 rounded-lg border ${
          message.type === 'success'
            ? 'bg-emerald-500/10 border-emerald-500/30 text-emerald-400'
            : 'bg-red-500/10 border-red-500/30 text-red-400'
        }`}>
          {message.type === 'success' ? <CheckCircle className="w-5 h-5 flex-shrink-0" /> : <AlertCircle className="w-5 h-5 flex-shrink-0" />}
          <span className="text-sm">{message.text}</span>
        </div>
      )}

      <div className="grid gap-4 md:grid-cols-2 lg:grid-cols-3">
        {Object.entries(SETTING_KEYS).map(([key, config]) => (
          <div
            key={key}
            className="bg-[var(--color-bg-card)] rounded-xl border border-[var(--color-border-primary)] p-6 transition-all hover:border-[#0099D9]/30"
          >
            <div className="flex items-start gap-3">
              <div className="w-10 h-10 rounded-lg bg-[#0099D9]/10 flex items-center justify-center flex-shrink-0">
                <Shield className="w-5 h-5 text-[#38BDF8]" />
              </div>
              <div className="flex-1">
                <h3 className="text-lg font-semibold text-[var(--color-text-primary)]">
                  {config.label}
                </h3>
                <p className="text-sm text-[var(--color-text-tertiary)] mt-1">
                  {config.description}
                </p>
              </div>
            </div>

            <div className="mt-4 flex items-center justify-between">
              <label className="relative inline-flex items-center cursor-pointer">
                <input
                  type="checkbox"
                  checked={getValue(key as SettingKey) === 'true'}
                  onChange={(e) => handleChange(key as SettingKey, e.target.checked ? 'true' : 'false')}
                  className="sr-only peer"
                />
                <div className="w-11 h-6 bg-[#334155] peer-focus:ring-2 peer-focus:ring-[#0099D9]/50 rounded-full peer peer-checked:after:translate-x-full peer-checked:after:border-white after:content-[''] after:absolute after:top-[2px] after:left-[2px] after:bg-white after:border-gray-300 after:border after:rounded-full after:h-5 after:w-5 after:transition-all peer-checked:bg-[#0099D9]"></div>
              </label>
              <button
                onClick={() => handleSave(key as SettingKey)}
                disabled={saving === key}
                className="px-4 py-2 text-sm font-medium bg-[#0099D9] hover:bg-[#007BB5] text-white rounded-lg transition-colors disabled:opacity-50 disabled:cursor-not-allowed flex items-center gap-2"
              >
                {saving === key ? (
                  <>
                    <Loader2 className="w-4 h-4 animate-spin" />
                    Guardando...
                  </>
                ) : (
                  <>
                    <Save className="w-4 h-4" />
                    Guardar
                  </>
                )}
              </button>
            </div>
          </div>
        ))}
      </div>

      <div className="bg-[var(--color-bg-card)] rounded-xl border border-[var(--color-border-primary)] p-6">
        <div className="flex items-start gap-3">
          <div className="w-10 h-10 rounded-lg bg-amber-500/10 flex items-center justify-center flex-shrink-0">
            <Info className="w-5 h-5 text-amber-400" />
          </div>
          <div>
            <h3 className="text-lg font-semibold text-[var(--color-text-primary)]">
              Notas importantes
            </h3>
            <ul className="text-sm text-[var(--color-text-tertiary)] mt-2 space-y-1 list-disc list-inside">
              <li>Los cambios toman efecto en ~30 segundos (cache Redis)</li>
              <li><strong>Requerir cuenta para entrantes</strong> bloquea llamadas si la trunca de origen no tiene <code>account_id</code> configurado</li>
              <li><strong>Requerir cuenta para salientes</strong> bloquea llamadas si la extensión/trunca que origina no tiene <code>account_id</code></li>
              <li>La autorización saliente (<code>require_outbound_authorization</code>) debe estar habilitada para que las otras dos funcionen en salientes</li>
              <li>Todas las configuraciones requieren rol <code>superadmin</code></li>
            </ul>
          </div>
        </div>
      </div>
    </div>
  )
}