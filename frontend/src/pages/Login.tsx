import { useState, useEffect, useCallback } from 'react'
import { useNavigate } from 'react-router-dom'
import { useMutation, useQueryClient } from '@tanstack/react-query'
import { login } from '../api/client'
import { Eye, EyeOff, AlertCircle, Loader2, Wifi, Tv, Phone, Zap, ChevronLeft, ChevronRight } from 'lucide-react'

// Paleta copiada de https://win.pe/css/main-26.min.css
const WIN = {
  orange: '#ff5a0a',
  dorange: '#d8450b',
  lorange: '#fff3ed',
  brown: '#301d19',
  yellow: '#ffc200',
  lgray: '#f6f6f6',
  gray: '#4b4b4b',
}

// Carrusel idéntico al hero (#banners .slick-banners) de https://win.pe/
// Slides, imágenes, CTAs y estilos copiados de la página principal.
const SLIDES = [
  {
    id: 'cyberwin',
    bgDesktop: '/win-banners/gamer/banner.jpg',
    bgMobile: '/win-banners/gamer/banner-xs.jpg',
    bgAlt: 'CyberWin',
    titleImg: '/win-banners/gamer/text-plan.png',
    titleAlt: 'Planes gamer WIN',
    titleClass: 'w-[240px] sm:w-[300px] lg:w-[340px]',
    ctaHref: 'https://wa.me/51940061937?text=[001]-¡Hola! Quiero saber si estoy en cobertura',
    ctaText: 'CÁMBIATE A WIN AQUÍ',
  },
  {
    id: 'satisfaccion',
    bgDesktop: '/win-banners/satisfaccion/banner.jpg',
    bgMobile: '/win-banners/satisfaccion/banner-mobile.jpg',
    bgAlt: 'Fondo: Banner de fibra óptica',
    titleImg: '/win-banners/satisfaccion/title.png',
    titleAlt: 'Internet #1 en satisfacción al cliente',
    titleClass: 'w-[280px] sm:w-[340px] lg:w-[380px]',
    ctaHref: 'https://wa.me/51940061937?text=[001]-ban-¡Hola! Quiero más información sobre WIN 100% Fibra óptica.',
    ctaText: 'CÁMBIATE A WIN AQUÍ',
  },
]

const AUTOPLAY_MS = 6000

function WinHeroCarousel() {
  const [index, setIndex] = useState(0)
  const [paused, setPaused] = useState(false)

  const goTo = useCallback((i: number) => {
    setIndex(((i % SLIDES.length) + SLIDES.length) % SLIDES.length)
  }, [])
  const prev = useCallback(() => goTo(index - 1), [goTo, index])
  const next = useCallback(() => goTo(index + 1), [goTo, index])

  useEffect(() => {
    if (paused) return
    const t = setInterval(() => {
      setIndex((i) => (i + 1) % SLIDES.length)
    }, AUTOPLAY_MS)
    return () => clearInterval(t)
  }, [paused])

  return (
    <div
      className="relative w-full overflow-hidden rounded-[30px] h-[360px] sm:h-[400px] lg:h-[430px] xl:h-[460px]"
      style={{ border: '1px solid #000', boxShadow: '5px 7px 0px -2px rgba(0,0,0,0.75)' }}
      onMouseEnter={() => setPaused(true)}
      onMouseLeave={() => setPaused(false)}
      role="region"
      aria-roledescription="carrusel"
      aria-label="Promociones WIN"
    >
      {SLIDES.map((s, i) => (
        <div
          key={s.id}
          className="absolute inset-0 transition-opacity duration-700"
          style={{ opacity: i === index ? 1 : 0, zIndex: i === index ? 1 : 0 }}
          aria-hidden={i !== index}
        >
          <picture className="absolute inset-0 w-full h-full">
            <source media="(min-width: 768px)" srcSet={s.bgDesktop} />
            <img
              src={s.bgMobile}
              alt={s.bgAlt}
              className="w-full h-full object-cover"
              loading={i === 0 ? 'eager' : 'lazy'}
            />
          </picture>
          <div className="absolute inset-0 flex flex-col items-center justify-end pb-8 sm:pb-10 px-6 text-center">
            <img src={s.titleImg} alt={s.titleAlt} className={`${s.titleClass} max-w-full mb-4 sm:mb-5`} />
            <a
              href={s.ctaHref}
              target="_blank"
              rel="noreferrer"
              tabIndex={i === index ? 0 : -1}
              className="relative inline-flex items-center justify-center font-extrabold text-base sm:text-lg px-8 h-[42px] sm:h-[50px] rounded-full transition-transform hover:scale-105"
              style={{
                backgroundColor: '#fff',
                color: WIN.orange,
                border: '1px solid #000',
                boxShadow: '5px 7px 0px -2px rgba(0,0,0,0.75)',
                minWidth: '240px',
              }}
            >
              {s.ctaText}
              <img
                src="/win-banners/click.png"
                alt=""
                aria-hidden
                className="absolute w-[30px] sm:w-[38px]"
                style={{ bottom: '50%', right: 0, transform: 'translate(70%, 100%)' }}
              />
            </a>
          </div>
        </div>
      ))}

      {/* Flechas estilo win.pe */}
      <button
        onClick={prev}
        aria-label="Banner anterior"
        className="hidden lg:flex absolute left-4 top-1/2 -translate-y-1/2 z-10 w-[50px] h-[50px] rounded-full items-center justify-center bg-white/90 hover:bg-white transition-transform hover:scale-105"
        style={{ border: '1px solid #000' }}
      >
        <ChevronLeft className="w-6 h-6" style={{ color: WIN.orange }} />
      </button>
      <button
        onClick={next}
        aria-label="Banner siguiente"
        className="hidden lg:flex absolute right-4 top-1/2 -translate-y-1/2 z-10 w-[50px] h-[50px] rounded-full items-center justify-center bg-white/90 hover:bg-white transition-transform hover:scale-105"
        style={{ border: '1px solid #000' }}
      >
        <ChevronRight className="w-6 h-6" style={{ color: WIN.orange }} />
      </button>

      {/* Dots estilo win.pe .slick-dots-banner */}
      <div className="absolute bottom-3 left-1/2 -translate-x-1/2 z-10 flex items-center gap-[4px]">
        {SLIDES.map((s, i) => (
          <button
            key={s.id}
            onClick={() => goTo(i)}
            aria-label={`Ir al banner ${i + 1}`}
            className="rounded-[3px] transition-all"
            style={
              i === index
                ? { backgroundColor: WIN.brown, width: 22, height: 8, border: '1px solid #fff' }
                : { backgroundColor: 'rgba(48,29,25,0.6)', width: 17, height: 7, border: '1px solid #fff' }
            }
          />
        ))}
      </div>
    </div>
  )
}

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
      queryClient.setQueryData(['currentUser'], data)
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
    <div
      className="min-h-screen flex flex-col lg:flex-row"
      style={{ fontFamily: "'Exo', 'MADETommySoft', 'Panton', system-ui, sans-serif" }}
    >
      {/* ===== PANEL IZQUIERDO estilo win.pe (bg-orange) ===== */}
      <div
        className="relative flex-1 flex flex-col px-5 py-6 sm:px-8 sm:py-8 lg:px-12 lg:py-10 overflow-hidden"
        style={{ backgroundColor: WIN.orange }}
      >
        {/* textura sutil */}
        <div
          className="absolute inset-0 pointer-events-none opacity-10"
          style={{
            backgroundImage:
              'radial-gradient(circle at 20% 20%, #fff 1.5px, transparent 1.5px), radial-gradient(circle at 80% 60%, #fff 1.5px, transparent 1.5px)',
            backgroundSize: '42px 42px',
          }}
        />

        {/* header: logo white win.pe */}
        <div className="relative flex items-center justify-between mb-6">
          <a href="https://win.pe" target="_blank" rel="noreferrer" className="flex items-center gap-3">
            <img
              src="/win-logo-white.png"
              alt="WIN internet"
              className="h-11 w-auto"
              style={{ filter: 'drop-shadow(0 2px 0 rgba(0,0,0,0.25))' }}
            />
          </a>
          <a
            href="tel:017073000"
            className="hidden sm:inline-flex items-center gap-2 text-white text-sm font-bold bg-black/25 hover:bg-black/40 transition-colors px-4 py-2 rounded-full border border-white/30"
          >
            <Phone className="w-4 h-4" />
            (01) 707-3000
          </a>
        </div>

        {/* Carrusel hero de win.pe */}
        <div className="relative">
          <WinHeroCarousel />
        </div>

        {/* beneficios estilo win.pe */}
        <div className="relative grid grid-cols-1 sm:grid-cols-3 gap-3 mt-6">
          {[
            { icon: Zap, title: 'Simétrico', desc: 'Sube y baja igual' },
            { icon: Wifi, title: 'FTTH real', desc: 'Fibra hasta tu hogar' },
            { icon: Tv, title: 'Dúos y tríos', desc: '+ TV digital' },
          ].map((b) => (
            <div
              key={b.title}
              className="rounded-[20px] p-4 bg-white"
              style={{ border: '1px solid #000', boxShadow: '5px 7px 0px -2px rgba(0,0,0,0.75)' }}
            >
              <b.icon className="w-6 h-6 mb-2" style={{ color: WIN.orange }} />
              <div className="font-extrabold text-sm" style={{ color: WIN.brown }}>{b.title}</div>
              <div className="text-xs font-medium" style={{ color: WIN.gray }}>{b.desc}</div>
            </div>
          ))}
        </div>

        <p className="relative mt-6 text-white/70 text-xs font-medium">
          Panel de gestión — Según OSIPTEL 2024 · Datum & CX Latam
        </p>
      </div>

      {/* ===== PANEL DERECHO login ===== */}
      <div className="flex-1 flex flex-col bg-white" style={{ backgroundColor: '#fff' }}>
        {/* topbar negra estilo win.pe */}
        <div className="bg-black text-white text-xs py-2.5 px-8 hidden lg:flex items-center justify-end gap-6 font-medium">
          <a href="https://win.pe/nosotros" target="_blank" rel="noreferrer" className="hover:underline">Nosotros</a>
          <a href="https://win.pe/blog/" target="_blank" rel="noreferrer" className="hover:underline">Blog</a>
          <a href="https://win.pe/preguntas-frecuentes/" target="_blank" rel="noreferrer" className="hover:underline">Preguntas frecuentes</a>
          <span className="px-3 py-1 rounded-full font-extrabold" style={{ backgroundColor: WIN.orange }}>
            MI PORTAL WIN
          </span>
        </div>

        <div className="flex-1 flex items-center justify-center p-6 sm:p-10" style={{ backgroundColor: WIN.lgray }}>
          <div className="w-full max-w-md">
            <div
              className="bg-white rounded-[30px] overflow-hidden"
              style={{ border: `1px solid ${WIN.orange}`, boxShadow: '0 20px 60px rgba(255,90,10,0.15)' }}
            >
              {/* header card */}
              <div className="px-8 pt-8 pb-6 text-center" style={{ backgroundColor: WIN.lorange }}>
                <img src="/win-logo-color.png" alt="WIN" className="h-12 w-auto mx-auto mb-4" />
                <h2 className="text-2xl font-black uppercase tracking-tight" style={{ color: WIN.brown }}>
                  Iniciar sesión
                </h2>
                <p className="text-sm mt-1 font-medium" style={{ color: WIN.gray }}>
                  Apolo SBC · Acceso al panel
                </p>
              </div>

              <form onSubmit={handleSubmit} className="p-8 space-y-5">
                {error && (
                  <div className="flex items-center gap-3 px-4 py-3 rounded-[15px] bg-red-50 border border-red-200 text-red-600">
                    <AlertCircle className="w-5 h-5 flex-shrink-0" />
                    <span className="text-sm font-semibold">{error}</span>
                  </div>
                )}

                <div className="space-y-2">
                  <label htmlFor="username" className="block text-sm font-extrabold uppercase tracking-wide" style={{ color: WIN.brown }}>
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
                    className="w-full px-5 py-3.5 rounded-[20px] bg-white text-[15px] font-medium focus:outline-none transition-all"
                    style={{ border: `1px solid #e5b89a`, color: WIN.brown }}
                    onFocus={(e) => { e.currentTarget.style.borderColor = WIN.orange; e.currentTarget.style.boxShadow = '0 0 0 3px rgba(255,90,10,0.15)' }}
                    onBlur={(e) => { e.currentTarget.style.boxShadow = 'none' }}
                  />
                </div>

                <div className="space-y-2">
                  <label htmlFor="password" className="block text-sm font-extrabold uppercase tracking-wide" style={{ color: WIN.brown }}>
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
                      className="w-full px-5 py-3.5 pr-12 rounded-[20px] bg-white text-[15px] font-medium focus:outline-none transition-all"
                      style={{ border: '1px solid #e5b89a', color: WIN.brown }}
                      onFocus={(e) => { e.currentTarget.style.borderColor = WIN.orange; e.currentTarget.style.boxShadow = '0 0 0 3px rgba(255,90,10,0.15)' }}
                      onBlur={(e) => { e.currentTarget.style.boxShadow = 'none' }}
                    />
                    <button
                      type="button"
                      onClick={() => setShowPassword(!showPassword)}
                      className="absolute right-3 top-1/2 -translate-y-1/2 p-2 rounded-full hover:bg-[#fff3ed] transition-colors"
                      style={{ color: WIN.gray }}
                      aria-label={showPassword ? 'Ocultar contraseña' : 'Mostrar contraseña'}
                    >
                      {showPassword ? <EyeOff className="w-5 h-5" /> : <Eye className="w-5 h-5" />}
                    </button>
                  </div>
                </div>

                <button
                  type="submit"
                  disabled={loginMutation.isPending}
                  className="w-full h-[50px] rounded-full font-black text-lg uppercase tracking-wide transition-transform hover:scale-[1.02] active:scale-[0.99] flex items-center justify-center gap-2 disabled:opacity-50 disabled:cursor-not-allowed"
                  style={{
                    backgroundColor: WIN.orange,
                    color: '#fff',
                    border: '1px solid #000',
                    boxShadow: '5px 7px 0px -2px rgba(0,0,0,0.75)',
                  }}
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

                <div className="flex items-center justify-between text-xs font-semibold" style={{ color: WIN.gray }}>
                  <span>Sistema protegido</span>
                  <a href="https://win.pe/contacto" target="_blank" rel="noreferrer" className="hover:underline" style={{ color: WIN.orange }}>
                    ¿Necesitas ayuda?
                  </a>
                </div>
              </form>

              <div className="px-8 py-4 border-t text-center text-xs font-medium" style={{ borderColor: '#f0d9c8', backgroundColor: '#fff', color: '#7c7e80' }}>
                Acceso solo para usuarios autorizados · © 2026 WIN
              </div>
            </div>

            <div className="mt-6 flex items-center justify-center gap-2 text-xs font-mono" style={{ color: '#7c7e80' }}>
              <span className="px-3 py-1.5 rounded-full bg-white border">v1.0.0 · Powered by Apolo AI</span>
            </div>
          </div>
        </div>
      </div>
    </div>
  )
}
