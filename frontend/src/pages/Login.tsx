import { useState } from 'react'
import { useNavigate } from 'react-router-dom'
import { useMutation, useQueryClient } from '@tanstack/react-query'
import { login } from '../api/client'
import { Eye, EyeOff, AlertCircle, Loader2 } from 'lucide-react'

export default function Login() {
  const navigate = useNavigate()
  const queryClient = useQueryClient()
  const [username, setUsername] = useState('')
  const [password, setPassword] = useState('')
  const [showPassword, setShowPassword] = useState(false)
  const [error, setError] = useState('')

  const loginMutation = useMutation({
    mutationFn: login,
    onSuccess: (data) => {
      // Set user data directly in cache to avoid race condition
      queryClient.setQueryData(['currentUser'], data)
      // Small delay to ensure cookie is set
      setTimeout(() => {
        navigate('/', { replace: true })
      }, 100)
    },
    onError: () => {
      setError('Usuario o contraseña incorrectos')
    },
  })

  const handleSubmit = (e: React.FormEvent) => {
    e.preventDefault()
    setError('')

    if (!username.trim() || !password.trim()) {
      setError('Ingresa usuario y contraseña')
      return
    }

    loginMutation.mutate({ username: username.trim(), password })
  }

  return (
    <div className="min-h-screen bg-gradient-to-br from-[#0D1B2A] via-[#1B3A4B] to-[#0D1B2A] flex flex-col items-center justify-center p-4 relative overflow-hidden">
      {/* Background effects */}
      <div className="absolute inset-0 pointer-events-none">
        {/* Animated circles */}
        <div className="absolute top-1/4 left-1/4 w-96 h-96 bg-[#0099D9]/10 rounded-full blur-3xl animate-pulse-soft" />
        <div className="absolute bottom-1/4 right-1/4 w-80 h-80 bg-[#0099D9]/5 rounded-full blur-3xl animate-pulse-soft" style={{ animationDelay: '1s' }} />

        {/* Grid pattern */}
        <div
          className="absolute inset-0 opacity-[0.03]"
          style={{
            backgroundImage: `linear-gradient(rgba(0, 153, 217, 0.5) 1px, transparent 1px),
                              linear-gradient(90deg, rgba(0, 153, 217, 0.5) 1px, transparent 1px)`,
            backgroundSize: '60px 60px',
          }}
        />
      </div>

      {/* Login card */}
      <div className="relative w-full max-w-md">
        {/* Top accent line - Fibertel blue gradient */}
        <div className="absolute -top-px left-8 right-8 h-px bg-gradient-to-r from-transparent via-[#0099D9] to-transparent" />

        <div className="bg-white/95 dark:bg-[#0D1B2A]/95 backdrop-blur-xl border border-[#0099D9]/20 rounded-2xl shadow-2xl shadow-[#0099D9]/10 overflow-hidden">
          {/* Header with Logo */}
          <div className="px-8 pt-10 pb-8 text-center bg-gradient-to-b from-[#F0F9FF] to-white dark:from-[#1B3A4B]/50 dark:to-transparent">
            {/* Fibertel Logo */}
            <div className="flex justify-center mb-6">
              <img
                src="/logo.png"
                alt="Fibertel"
                className="h-14 w-auto"
              />
            </div>

            <h1 className="text-2xl font-bold text-[#1E293B] dark:text-white tracking-tight">
              Sistema de Facturación
            </h1>
            <p className="text-[#6B7280] dark:text-[#94A3B8] text-sm mt-2">
              Ingresa tus credenciales para continuar
            </p>
          </div>

          {/* Form */}
          <form onSubmit={handleSubmit} className="p-8 space-y-5">
            {/* Error message */}
            {error && (
              <div className="flex items-center gap-3 px-4 py-3 rounded-xl bg-red-50 dark:bg-red-500/10 border border-red-200 dark:border-red-500/20 text-red-600 dark:text-red-400">
                <AlertCircle className="w-5 h-5 flex-shrink-0" />
                <span className="text-sm font-medium">{error}</span>
              </div>
            )}

            {/* Username field */}
            <div className="space-y-2">
              <label htmlFor="username" className="block text-sm font-semibold text-[#1E293B] dark:text-[#CBD5E1]">
                Usuario
              </label>
              <input
                id="username"
                type="text"
                value={username}
                onChange={(e) => setUsername(e.target.value)}
                placeholder="Ingresa tu usuario"
                autoComplete="username"
                autoFocus
                className="w-full px-4 py-3 rounded-xl bg-[#F0F9FF] dark:bg-[#1E293B] border-2 border-[#BAE6FD] dark:border-[#334155] text-[#1E293B] dark:text-white placeholder-[#94A3B8] focus:outline-none focus:border-[#0099D9] focus:ring-4 focus:ring-[#0099D9]/10 transition-all"
              />
            </div>

            {/* Password field */}
            <div className="space-y-2">
              <label htmlFor="password" className="block text-sm font-semibold text-[#1E293B] dark:text-[#CBD5E1]">
                Contraseña
              </label>
              <div className="relative">
                <input
                  id="password"
                  type={showPassword ? 'text' : 'password'}
                  value={password}
                  onChange={(e) => setPassword(e.target.value)}
                  placeholder="••••••••"
                  autoComplete="current-password"
                  className="w-full px-4 py-3 pr-12 rounded-xl bg-[#F0F9FF] dark:bg-[#1E293B] border-2 border-[#BAE6FD] dark:border-[#334155] text-[#1E293B] dark:text-white placeholder-[#94A3B8] focus:outline-none focus:border-[#0099D9] focus:ring-4 focus:ring-[#0099D9]/10 transition-all"
                />
                <button
                  type="button"
                  onClick={() => setShowPassword(!showPassword)}
                  className="absolute right-3 top-1/2 -translate-y-1/2 p-1.5 text-[#6B7280] hover:text-[#0099D9] transition-colors rounded-lg hover:bg-[#0099D9]/10"
                >
                  {showPassword ? <EyeOff className="w-5 h-5" /> : <Eye className="w-5 h-5" />}
                </button>
              </div>
            </div>

            {/* Submit button */}
            <button
              type="submit"
              disabled={loginMutation.isPending}
              className="w-full py-3.5 px-4 rounded-xl bg-gradient-to-r from-[#0099D9] to-[#007BB5] hover:from-[#33ADDF] hover:to-[#0099D9] text-white font-semibold shadow-lg shadow-[#0099D9]/25 hover:shadow-[#0099D9]/40 transition-all duration-200 disabled:opacity-50 disabled:cursor-not-allowed flex items-center justify-center gap-2 hover:-translate-y-0.5 active:translate-y-0"
            >
              {loginMutation.isPending ? (
                <>
                  <Loader2 className="w-5 h-5 animate-spin" />
                  <span>Ingresando...</span>
                </>
              ) : (
                <span>Iniciar Sesión</span>
              )}
            </button>
          </form>

          {/* Footer */}
          <div className="px-8 py-4 bg-[#F0F9FF] dark:bg-[#1B3A4B]/30 border-t border-[#BAE6FD]/50 dark:border-[#334155]">
            <p className="text-center text-xs text-[#6B7280] dark:text-[#64748B]">
              Sistema protegido • Acceso solo para usuarios autorizados
            </p>
          </div>
        </div>

        {/* Bottom accent */}
        <div className="absolute -bottom-px left-8 right-8 h-px bg-gradient-to-r from-transparent via-[#6B7280]/30 to-transparent" />
      </div>

      {/* Version badge */}
      <div className="mt-8 px-4 py-2 rounded-full bg-white/10 backdrop-blur-sm border border-white/10">
        <span className="text-xs text-[#94A3B8] font-mono">
          v1.0.0 • Powered by Fibertel
        </span>
      </div>
    </div>
  )
}
