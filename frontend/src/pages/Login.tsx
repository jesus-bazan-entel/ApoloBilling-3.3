import { useState } from 'react'
import { useNavigate } from 'react-router-dom'
import { useMutation, useQueryClient } from '@tanstack/react-query'
import { login } from '../api/client'
import { Zap, Eye, EyeOff, AlertCircle, Loader2 } from 'lucide-react'

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
    <div className="min-h-screen bg-[#0a0f1a] flex flex-col items-center justify-center p-4 relative overflow-hidden">
      {/* Background effects */}
      <div className="absolute inset-0 pointer-events-none">
        {/* Grid pattern */}
        <div
          className="absolute inset-0 opacity-[0.03]"
          style={{
            backgroundImage: `linear-gradient(rgba(6, 182, 212, 0.5) 1px, transparent 1px),
                              linear-gradient(90deg, rgba(6, 182, 212, 0.5) 1px, transparent 1px)`,
            backgroundSize: '50px 50px',
          }}
        />
        {/* Radial glow */}
        <div className="absolute top-1/4 left-1/2 -translate-x-1/2 w-[600px] h-[600px] bg-cyan-500/5 rounded-full blur-3xl" />
        <div className="absolute bottom-0 left-1/2 -translate-x-1/2 w-[800px] h-[400px] bg-amber-500/5 rounded-full blur-3xl" />
      </div>

      {/* Login card */}
      <div className="relative w-full max-w-md">
        {/* Top accent line */}
        <div className="absolute -top-px left-8 right-8 h-px bg-gradient-to-r from-transparent via-cyan-500 to-transparent" />

        <div className="bg-[#0d1321] border border-slate-800/50 rounded-xl shadow-2xl shadow-black/50 overflow-hidden">
          {/* Header */}
          <div className="px-8 pt-8 pb-6 text-center border-b border-slate-800/50">
            {/* Logo */}
            <div className="inline-flex items-center justify-center w-16 h-16 rounded-xl bg-gradient-to-br from-cyan-500 to-blue-600 mb-4 shadow-lg shadow-cyan-500/25">
              <Zap className="w-8 h-8 text-white" />
            </div>

            <h1 className="text-2xl font-bold text-white tracking-tight">
              APOLO<span className="text-cyan-500">BILLING</span>
            </h1>
            <p className="text-slate-500 text-sm mt-2">
              Sistema de Facturación en Tiempo Real
            </p>
          </div>

          {/* Form */}
          <form onSubmit={handleSubmit} className="p-8 space-y-5">
            {/* Error message */}
            {error && (
              <div className="flex items-center gap-3 px-4 py-3 rounded-lg bg-red-500/10 border border-red-500/20 text-red-400">
                <AlertCircle className="w-5 h-5 flex-shrink-0" />
                <span className="text-sm">{error}</span>
              </div>
            )}

            {/* Username field */}
            <div className="space-y-2">
              <label htmlFor="username" className="block text-xs font-medium text-slate-400 uppercase tracking-wider">
                Usuario
              </label>
              <input
                id="username"
                type="text"
                value={username}
                onChange={(e) => setUsername(e.target.value)}
                placeholder="admin"
                autoComplete="username"
                autoFocus
                className="w-full px-4 py-3 rounded-lg bg-slate-900/50 border border-slate-700/50 text-white placeholder-slate-600 focus:outline-none focus:border-cyan-500/50 focus:ring-1 focus:ring-cyan-500/25 transition-colors"
              />
            </div>

            {/* Password field */}
            <div className="space-y-2">
              <label htmlFor="password" className="block text-xs font-medium text-slate-400 uppercase tracking-wider">
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
                  className="w-full px-4 py-3 pr-12 rounded-lg bg-slate-900/50 border border-slate-700/50 text-white placeholder-slate-600 focus:outline-none focus:border-cyan-500/50 focus:ring-1 focus:ring-cyan-500/25 transition-colors"
                />
                <button
                  type="button"
                  onClick={() => setShowPassword(!showPassword)}
                  className="absolute right-3 top-1/2 -translate-y-1/2 p-1 text-slate-500 hover:text-slate-300 transition-colors"
                >
                  {showPassword ? <EyeOff className="w-5 h-5" /> : <Eye className="w-5 h-5" />}
                </button>
              </div>
            </div>

            {/* Submit button */}
            <button
              type="submit"
              disabled={loginMutation.isPending}
              className="w-full py-3.5 px-4 rounded-lg bg-gradient-to-r from-cyan-600 to-blue-600 hover:from-cyan-500 hover:to-blue-500 text-white font-semibold shadow-lg shadow-cyan-500/25 hover:shadow-cyan-500/40 transition-all duration-200 disabled:opacity-50 disabled:cursor-not-allowed flex items-center justify-center gap-2"
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
          <div className="px-8 py-4 bg-slate-900/30 border-t border-slate-800/50">
            <p className="text-center text-xs text-slate-600">
              Sistema protegido. Acceso solo para usuarios autorizados.
            </p>
          </div>
        </div>

        {/* Bottom accent */}
        <div className="absolute -bottom-px left-8 right-8 h-px bg-gradient-to-r from-transparent via-amber-500/50 to-transparent" />
      </div>

      {/* Version badge */}
      <div className="mt-8 text-xs text-slate-600 font-mono">
        v1.0.0 • Rust Backend
      </div>
    </div>
  )
}
