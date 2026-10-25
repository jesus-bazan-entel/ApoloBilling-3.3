import { useMemo, useRef, useState } from 'react'
import { useMutation } from '@tanstack/react-query'
import { unzipSync, strFromU8 } from 'fflate'
import { provisionAccount, provisionBulk } from '../api/client'
import type {
  ProvisionRequest,
  ProvisionResult,
  BulkProvisionResponse,
  BulkRowResult,
} from '../types'
import {
  UserPlus,
  Upload,
  Download,
  FileSpreadsheet,
  CheckCircle2,
  XCircle,
  MinusCircle,
  Loader2,
  KeyRound,
} from 'lucide-react'

const DEFAULT_DOMAIN = 'app.fonowin.pe'
// Simultaneous registrations per SIP device (what 3219 of 3225 devices use)
const DEFAULT_REGISTRATIONS = 2

// Template columns (order matters for the downloadable template).
const TEMPLATE_COLUMNS = [
  'CLIENTE',
  'Username',
  'Password',
  'Dominio',
  'Registros',
  'COD_PED',
  'Departamento',
  'Provincia',
  'Distrito',
] as const

// Normalize a header/string: lowercase, strip accents and surrounding spaces.
function normalize(s: string): string {
  return s
    .normalize('NFD')
    .replace(/[\u0300-\u036f]/g, '')
    .trim()
    .toLowerCase()
}

/** Registrations from the file: 1–10, or the default when empty/invalid */
function parseRegistrations(raw: string | undefined, fallback: number): number {
  const n = Number.parseInt((raw ?? '').trim(), 10)
  return Number.isFinite(n) && n >= 1 && n <= 10 ? n : fallback
}

// Decode the handful of XML entities that appear in spreadsheet text.
function decodeXml(s: string): string {
  return s
    .replace(/&lt;/g, '<')
    .replace(/&gt;/g, '>')
    .replace(/&quot;/g, '"')
    .replace(/&apos;/g, "'")
    .replace(/&#39;/g, "'")
    .replace(/&amp;/g, '&')
}

// Convert an A1-style column reference ("A", "AB") to a 0-based column index.
function colToIdx(ref: string): number {
  let n = 0
  for (const ch of ref) n = n * 26 + (ch.charCodeAt(0) - 64)
  return n - 1
}

// Parse a .xlsx workbook (a ZIP of XML parts) into rows of string cells, without
// the vulnerable `xlsx` npm package. Reads the first worksheet + shared strings.
// Empty cells are preserved via the cell's column reference so columns stay aligned.
function parseXlsx(buf: ArrayBuffer): string[][] {
  const files = unzipSync(new Uint8Array(buf))

  // Shared strings table (cells of type "s" reference entries here by index).
  const sharedStrings: string[] = []
  const ssFile = files['xl/sharedStrings.xml']
  if (ssFile) {
    const xml = strFromU8(ssFile)
    const siRegex = /<si>([\s\S]*?)<\/si>/g
    let si: RegExpExecArray | null
    while ((si = siRegex.exec(xml))) {
      const tRegex = /<t[^>]*>([\s\S]*?)<\/t>/g
      let t: RegExpExecArray | null
      let str = ''
      while ((t = tRegex.exec(si[1]))) str += decodeXml(t[1])
      sharedStrings.push(str)
    }
  }

  // First worksheet (sheet1.xml, or the lowest-numbered sheet present).
  const sheetName = Object.keys(files)
    .filter((f) => /^xl\/worksheets\/sheet\d+\.xml$/.test(f))
    .sort()[0]
  if (!sheetName) return []

  const xml = strFromU8(files[sheetName])
  const rows: string[][] = []
  const rowRegex = /<row[^>]*>([\s\S]*?)<\/row>/g
  const cellRegex = /<c\b([^>]*?)(?:\/>|>([\s\S]*?)<\/c>)/g
  let rm: RegExpExecArray | null
  while ((rm = rowRegex.exec(xml))) {
    const cells: string[] = []
    let cm: RegExpExecArray | null
    while ((cm = cellRegex.exec(rm[1]))) {
      const attrs = cm[1] ?? ''
      const inner = cm[2] ?? ''
      const refMatch = /r="([A-Z]+)\d+"/.exec(attrs)
      const idx = refMatch ? colToIdx(refMatch[1]) : cells.length
      const typeMatch = /t="([^"]+)"/.exec(attrs)
      const type = typeMatch ? typeMatch[1] : ''
      let val = ''
      if (type === 's') {
        const v = /<v>([\s\S]*?)<\/v>/.exec(inner)
        if (v) val = sharedStrings[parseInt(v[1], 10)] ?? ''
      } else if (type === 'inlineStr') {
        const t = /<t[^>]*>([\s\S]*?)<\/t>/.exec(inner)
        if (t) val = decodeXml(t[1])
      } else {
        const v = /<v>([\s\S]*?)<\/v>/.exec(inner)
        if (v) val = decodeXml(v[1])
      }
      cells[idx] = val
    }
    for (let i = 0; i < cells.length; i++) if (cells[i] === undefined) cells[i] = ''
    rows.push(cells)
  }
  return rows.filter((r) => r.some((c) => c.trim() !== ''))
}

// Minimal RFC4180-ish parser supporting comma / semicolon / tab delimiters and
// quoted fields. Returns an array of string arrays (rows of cells).
function parseDelimited(text: string): string[][] {
  const clean = text.replace(/^\ufeff/, '') // strip BOM
  // Auto-detect delimiter from the first non-empty line.
  const firstLine = clean.split(/\r?\n/).find((l) => l.trim() !== '') ?? ''
  const counts: Record<string, number> = {
    '\t': (firstLine.match(/\t/g) || []).length,
    ';': (firstLine.match(/;/g) || []).length,
    ',': (firstLine.match(/,/g) || []).length,
  }
  const delim = (Object.entries(counts).sort((a, b) => b[1] - a[1])[0]?.[0] ?? ',') as string

  const rows: string[][] = []
  let field = ''
  let row: string[] = []
  let inQuotes = false

  for (let i = 0; i < clean.length; i++) {
    const c = clean[i]
    if (inQuotes) {
      if (c === '"') {
        if (clean[i + 1] === '"') {
          field += '"'
          i++
        } else {
          inQuotes = false
        }
      } else {
        field += c
      }
    } else if (c === '"') {
      inQuotes = true
    } else if (c === delim) {
      row.push(field)
      field = ''
    } else if (c === '\n') {
      row.push(field)
      rows.push(row)
      row = []
      field = ''
    } else if (c === '\r') {
      // ignore; handled by \n
    } else {
      field += c
    }
  }
  // last field/row
  if (field !== '' || row.length > 0) {
    row.push(field)
    rows.push(row)
  }
  return rows.filter((r) => r.some((c) => c.trim() !== ''))
}

interface ParsedRow extends ProvisionRequest {
  /** The file set the value (otherwise the default applies) */
  _fromFile: boolean
  _valid: boolean
  _error?: string
}

function buildTemplateCsv(): string {
  const header = TEMPLATE_COLUMNS.join(',')
  const samples = [
    ['VASTHI TUESTA CRUZ', '17349597', 'W1nn3t*2022#Fonowin1#', 'app.fonowin.pe', '2', '456336', 'Lima', 'Lima', 'Puente Piedra'],
    ['DIEGO ARMANDO JACHO ZUNIGA', '19032672', 'W1nn3t*2022#Fonowin1#', 'app.fonowin.pe', '2', '717566', 'Lima', 'Lima', 'Villa Maria del Triunfo'],
  ]
  const lines = samples.map((r) => r.map((c) => (/[",\n]/.test(c) ? `"${c.replace(/"/g, '""')}"` : c)).join(','))
  return [header, ...lines].join('\n')
}

export default function ProvisioningPage() {
  const [tab, setTab] = useState<'individual' | 'bulk'>('individual')

  return (
    <div className="space-y-6">
      <div>
        <h1 className="text-3xl font-bold gradient-text tracking-tight">Aprovisionamiento</h1>
        <p className="text-sm text-[var(--color-text-secondary)] mt-1 font-medium">
          Crea el cliente y su dispositivo SIP en un solo paso — individual o masivo
        </p>
      </div>

      {/* Tabs */}
      <div className="flex gap-2 border-b border-[var(--color-border-primary)]">
        <button
          onClick={() => setTab('individual')}
          className={`px-4 py-2 text-sm font-medium border-b-2 transition-colors ${
            tab === 'individual'
              ? 'border-[#ff5a0a] text-[#ff5a0a]'
              : 'border-transparent text-[var(--color-text-tertiary)] hover:text-[var(--color-text-primary)]'
          }`}
        >
          <UserPlus className="w-4 h-4 inline mr-2" />
          Individual
        </button>
        <button
          onClick={() => setTab('bulk')}
          className={`px-4 py-2 text-sm font-medium border-b-2 transition-colors ${
            tab === 'bulk'
              ? 'border-[#ff5a0a] text-[#ff5a0a]'
              : 'border-transparent text-[var(--color-text-tertiary)] hover:text-[var(--color-text-primary)]'
          }`}
        >
          <FileSpreadsheet className="w-4 h-4 inline mr-2" />
          Carga Masiva
        </button>
      </div>

      {tab === 'individual' ? (
        <IndividualForm />
      ) : (
        <BulkForm />
      )}
    </div>
  )
}

// ----------------------------- Individual -----------------------------

function IndividualForm() {
  const [form, setForm] = useState<ProvisionRequest>({
    customer_name: '',
    username: '',
    password: '',
    domain: DEFAULT_DOMAIN,
    max_registrations: DEFAULT_REGISTRATIONS,
    cod_ped: '',
    departamento: '',
    provincia: '',
    distrito: '',
  })
  const [result, setResult] = useState<ProvisionResult | null>(null)

  const mutation = useMutation({
    mutationFn: provisionAccount,
    onSuccess: (r) => setResult(r),
  })

  const set = (k: keyof ProvisionRequest, v: string | number) => setForm((f) => ({ ...f, [k]: v }))

  const errorMsg = mutation.isError
    ? ((mutation.error as { response?: { data?: { error?: string; message?: string } } })?.response?.data?.error ??
        (mutation.error as { response?: { data?: { message?: string } } })?.response?.data?.message ??
        'Error al aprovisionar')
    : null

  const submit = (e: React.FormEvent) => {
    e.preventDefault()
    setResult(null)
    mutation.mutate({
      ...form,
      customer_name: form.customer_name?.trim() || undefined,
      password: form.password?.trim() || undefined,
      domain: form.domain?.trim() || DEFAULT_DOMAIN,
    })
  }

  const inputClass =
    'w-full px-3 py-2 border border-[var(--color-border-primary)] rounded-lg focus:ring-2 focus:ring-[#ff5a0a] focus:border-[#ff5a0a] bg-[var(--color-bg-card)] text-[var(--color-text-primary)]'

  return (
    <div className="max-w-3xl">
      <form
        onSubmit={submit}
        className="bg-[var(--color-bg-card)] rounded-xl shadow-sm border border-[var(--color-border-primary)] p-6 space-y-4"
      >
        <div className="grid grid-cols-1 md:grid-cols-2 gap-4">
          <div className="md:col-span-2">
            <label className="block text-sm font-medium text-[var(--color-text-secondary)] mb-1">Cliente</label>
            <input className={inputClass} value={form.customer_name} onChange={(e) => set('customer_name', e.target.value)} placeholder="Nombre del cliente" />
          </div>
          <div>
            <label className="block text-sm font-medium text-[var(--color-text-secondary)] mb-1">
              Username <span className="text-red-500">*</span>
            </label>
            <input className={inputClass} required value={form.username} onChange={(e) => set('username', e.target.value)} placeholder="Ej. 19007232 (cliente = usuario SIP)" />
          </div>
          <div>
            <label className="block text-sm font-medium text-[var(--color-text-secondary)] mb-1">Password</label>
            <input className={inputClass} value={form.password} onChange={(e) => set('password', e.target.value)} placeholder="Vacío = se genera automáticamente" />
          </div>
          <div>
            <label className="block text-sm font-medium text-[var(--color-text-secondary)] mb-1">Dominio</label>
            <input className={inputClass} value={form.domain} onChange={(e) => set('domain', e.target.value)} placeholder={DEFAULT_DOMAIN} />
          </div>
          <div>
            <label className="block text-sm font-medium text-[var(--color-text-secondary)] mb-1">Registros simultáneos</label>
            <input
              type="number"
              min={1}
              max={10}
              className={inputClass}
              value={form.max_registrations ?? DEFAULT_REGISTRATIONS}
              onChange={(e) => set('max_registrations', Number(e.target.value))}
            />
            <p className="text-xs text-[var(--color-text-tertiary)] mt-1">Dispositivos que pueden registrarse a la vez con este usuario (1–10).</p>
          </div>
          <div>
            <label className="block text-sm font-medium text-[var(--color-text-secondary)] mb-1">COD_PED</label>
            <input className={inputClass} value={form.cod_ped} onChange={(e) => set('cod_ped', e.target.value)} placeholder="Código de pedido (display name del SIP)" />
          </div>
          <div>
            <label className="block text-sm font-medium text-[var(--color-text-secondary)] mb-1">Departamento</label>
            <input className={inputClass} value={form.departamento} onChange={(e) => set('departamento', e.target.value)} />
          </div>
          <div>
            <label className="block text-sm font-medium text-[var(--color-text-secondary)] mb-1">Provincia</label>
            <input className={inputClass} value={form.provincia} onChange={(e) => set('provincia', e.target.value)} />
          </div>
          <div>
            <label className="block text-sm font-medium text-[var(--color-text-secondary)] mb-1">Distrito</label>
            <input className={inputClass} value={form.distrito} onChange={(e) => set('distrito', e.target.value)} />
          </div>
        </div>

        {errorMsg && (
          <div className="flex items-center gap-2 text-sm text-red-700 bg-red-50 border border-red-200 rounded-lg px-3 py-2">
            <XCircle className="w-4 h-4 shrink-0" />
            {errorMsg}
          </div>
        )}

        <div className="flex justify-end">
          <button
            type="submit"
            disabled={mutation.isPending || !form.username}
            className="flex items-center px-4 py-2 bg-[#ff5a0a] text-white rounded-lg hover:bg-[#d8450b] transition-colors disabled:opacity-50"
          >
            {mutation.isPending ? <Loader2 className="w-5 h-5 mr-2 animate-spin" /> : <UserPlus className="w-5 h-5 mr-2" />}
            Aprovisionar
          </button>
        </div>
      </form>

      {result && (
        <div className="mt-4 bg-green-50 border border-green-200 rounded-xl p-5">
          <div className="flex items-center gap-2 text-green-800 font-semibold mb-3">
            <CheckCircle2 className="w-5 h-5" />
            Cliente aprovisionado correctamente
          </div>
          <div className="grid grid-cols-2 gap-x-6 gap-y-1 text-sm text-green-900">
            <span>Cliente: <strong>{result.account_number}</strong> (id {result.account_id})</span>
            <span>Dispositivo SIP: <strong>{result.sip_username}@{result.sip_domain}</strong></span>
            <span className="col-span-2 flex items-center gap-2">
              <KeyRound className="w-4 h-4" /> Password: <code className="bg-white px-2 py-0.5 rounded border border-green-300">{result.password}</code>
            </span>
          </div>
        </div>
      )}
    </div>
  )
}

// ----------------------------- Bulk -----------------------------

function BulkForm() {
  const [defaultRegistrations, setDefaultRegistrations] = useState<number>(DEFAULT_REGISTRATIONS)
  const [rows, setRows] = useState<ParsedRow[]>([])
  const [fileName, setFileName] = useState('')
  const [parseError, setParseError] = useState<string | null>(null)
  const [report, setReport] = useState<BulkProvisionResponse | null>(null)
  const fileRef = useRef<HTMLInputElement>(null)

  const mutation = useMutation({
    mutationFn: provisionBulk,
    onSuccess: (r) => setReport(r),
  })

  const validRows = useMemo(() => rows.filter((r) => r._valid), [rows])

  const processMatrix = (matrix: string[][], registrations: number) => {
    setReport(null)
    try {
      if (matrix.length < 2) {
        setParseError('El archivo no tiene filas de datos (se esperaba una cabecera + filas).')
        setRows([])
        return
      }
      const header = matrix[0].map(normalize)
      const col = (name: string) => header.indexOf(normalize(name))
      const idx = {
        cliente: col('CLIENTE'),
        username: col('Username'),
        password: col('Password'),
        dominio: col('Dominio'),
        registros: col('Registros'),
        cod: col('COD_PED'),
        depto: col('Departamento'),
        prov: col('Provincia'),
        dist: col('Distrito'),
      }
      if (idx.username < 0) {
        setParseError('No se encontró la columna "Username" en la cabecera.')
        setRows([])
        return
      }
      const get = (r: string[], i: number) => (i >= 0 ? (r[i] ?? '').trim() : '')
      const parsed: ParsedRow[] = matrix.slice(1).map((r) => {
        const username = get(r, idx.username)
        const rawRegistrations = get(r, idx.registros)
        return {
          customer_name: get(r, idx.cliente) || undefined,
          username,
          password: get(r, idx.password) || undefined,
          domain: get(r, idx.dominio) || undefined,
          max_registrations: parseRegistrations(rawRegistrations, registrations),
          cod_ped: get(r, idx.cod) || undefined,
          departamento: get(r, idx.depto) || undefined,
          provincia: get(r, idx.prov) || undefined,
          distrito: get(r, idx.dist) || undefined,
          _fromFile: parseRegistrations(rawRegistrations, 0) > 0,
          _valid: username.length > 0,
          _error: !username ? 'Username vacío' : undefined,
        }
      })
      setRows(parsed)
      setParseError(null)
    } catch {
      setParseError('No se pudo interpretar el contenido del archivo.')
      setRows([])
    }
  }

  const onFile = async (file: File) => {
    setFileName(file.name)
    setReport(null)
    const isXlsx =
      /\.xlsx$/i.test(file.name) ||
      file.type === 'application/vnd.openxmlformats-officedocument.spreadsheetml.sheet'
    try {
      if (isXlsx) {
        const buf = await file.arrayBuffer()
        processMatrix(parseXlsx(buf), defaultRegistrations)
      } else if (/\.xls$/i.test(file.name)) {
        setParseError('Los archivos .xls antiguos no son compatibles. Guárdalo como .xlsx o CSV.')
        setRows([])
      } else {
        const text = await file.text()
        processMatrix(parseDelimited(text), defaultRegistrations)
      }
    } catch {
      setParseError('No se pudo leer el archivo. Verifica que sea un .xlsx o CSV válido.')
      setRows([])
    }
  }

  const onDefaultRegistrationsChange = (n: number) => {
    setDefaultRegistrations(n)
    // Rows whose file left the column empty follow the default
    setRows((prev) => prev.map((r) => (r._fromFile ? r : { ...r, max_registrations: n })))
  }

  const downloadTemplate = () => {
    const blob = new Blob([buildTemplateCsv()], { type: 'text/csv;charset=utf-8;' })
    const url = URL.createObjectURL(blob)
    const a = document.createElement('a')
    a.href = url
    a.download = 'plantilla_aprovisionamiento.csv'
    a.click()
    URL.revokeObjectURL(url)
  }

  const submit = () => {
    const payload: ProvisionRequest[] = validRows.map((r) => ({
      customer_name: r.customer_name,
      username: r.username,
      password: r.password,
      domain: r.domain,
      max_registrations: r.max_registrations,
      cod_ped: r.cod_ped,
      departamento: r.departamento,
      provincia: r.provincia,
      distrito: r.distrito,
    }))
    mutation.mutate(payload)
  }

  const inputClass =
    'px-3 py-2 border border-[var(--color-border-primary)] rounded-lg focus:ring-2 focus:ring-[#ff5a0a] bg-[var(--color-bg-card)] text-[var(--color-text-primary)]'

  return (
    <div className="space-y-4">
      {/* Controls */}
      <div className="bg-[var(--color-bg-card)] rounded-xl shadow-sm border border-[var(--color-border-primary)] p-5 space-y-4">
        <div className="flex flex-wrap items-end gap-4">
          <div>
            <label className="block text-sm font-medium text-[var(--color-text-secondary)] mb-1">Registros simultáneos por defecto</label>
            <input
              type="number"
              min={1}
              max={10}
              className={inputClass}
              value={defaultRegistrations}
              onChange={(e) => onDefaultRegistrationsChange(Math.min(10, Math.max(1, Number(e.target.value) || DEFAULT_REGISTRATIONS)))}
            />
            <p className="text-xs text-[var(--color-text-tertiary)] mt-1">Se usa cuando la columna "Registros" del archivo está vacía.</p>
          </div>

          <button onClick={() => fileRef.current?.click()} className="flex items-center px-4 py-2 bg-[#ff5a0a] text-white rounded-lg hover:bg-[#d8450b] transition-colors">
            <Upload className="w-5 h-5 mr-2" />
            Subir archivo (Excel o CSV)
          </button>
          <input
            ref={fileRef}
            type="file"
            accept=".xlsx,.csv,.tsv,.txt,application/vnd.openxmlformats-officedocument.spreadsheetml.sheet,text/csv,text/plain"
            className="hidden"
            onChange={(e) => {
              const f = e.target.files?.[0]
              if (f) onFile(f)
              e.target.value = ''
            }}
          />

          <button onClick={downloadTemplate} className="flex items-center px-4 py-2 border border-[var(--color-border-primary)] text-[var(--color-text-secondary)] rounded-lg hover:bg-[var(--color-bg-hover)] transition-colors">
            <Download className="w-5 h-5 mr-2" />
            Descargar plantilla
          </button>

          {fileName && <span className="text-sm text-[var(--color-text-tertiary)] self-center">{fileName}</span>}
        </div>

        <p className="text-xs text-[var(--color-text-tertiary)]">
          Formatos aceptados: <code>.xlsx</code> (Excel) y <code>.csv</code>. Columnas esperadas:{' '}
          <code>{TEMPLATE_COLUMNS.join(', ')}</code>. El cliente y el usuario SIP toman el valor de <code>Username</code>;{' '}
          <code>COD_PED</code> se usa como display name del SIP.
        </p>

        {parseError && (
          <div className="flex items-center gap-2 text-sm text-red-700 bg-red-50 border border-red-200 rounded-lg px-3 py-2">
            <XCircle className="w-4 h-4 shrink-0" />
            {parseError}
          </div>
        )}
      </div>

      {/* Preview */}
      {rows.length > 0 && (
        <div className="bg-[var(--color-bg-card)] rounded-xl shadow-sm border border-[var(--color-border-primary)] overflow-hidden">
          <div className="px-5 py-3 border-b border-[var(--color-border-primary)] flex items-center justify-between">
            <span className="text-sm font-medium text-[var(--color-text-secondary)]">
              {rows.length} fila(s) — {validRows.length} válida(s), {rows.length - validRows.length} con error
            </span>
            <button
              onClick={submit}
              disabled={mutation.isPending || validRows.length === 0}
              className="flex items-center px-4 py-2 bg-green-600 text-white rounded-lg hover:bg-green-700 transition-colors disabled:opacity-50"
            >
              {mutation.isPending ? <Loader2 className="w-5 h-5 mr-2 animate-spin" /> : <CheckCircle2 className="w-5 h-5 mr-2" />}
              Procesar {validRows.length} fila(s)
            </button>
          </div>
          <div className="overflow-x-auto max-h-96">
            <table className="min-w-full text-sm">
              <thead className="bg-[var(--color-bg-tertiary)] sticky top-0">
                <tr className="text-left text-xs uppercase text-[var(--color-text-tertiary)]">
                  <th className="px-3 py-2">#</th>
                  <th className="px-3 py-2">Cliente</th>
                  <th className="px-3 py-2">Username</th>
                  <th className="px-3 py-2">Dominio</th>
                  <th className="px-3 py-2">Registros</th>
                  <th className="px-3 py-2">COD_PED</th>
                  <th className="px-3 py-2">Ubicación</th>
                  <th className="px-3 py-2">Estado</th>
                </tr>
              </thead>
              <tbody className="divide-y divide-[var(--color-border-primary)]">
                {rows.map((r, i) => (
                  <tr key={i} className={r._valid ? '' : 'bg-red-50'}>
                    <td className="px-3 py-2 text-[var(--color-text-tertiary)]">{i + 1}</td>
                    <td className="px-3 py-2">{r.customer_name ?? '—'}</td>
                    <td className="px-3 py-2 font-medium">{r.username || '—'}</td>
                    <td className="px-3 py-2">{r.domain ?? DEFAULT_DOMAIN}</td>
                    <td className="px-3 py-2 tabular-nums">
                      {r.max_registrations}
                      {!r._fromFile && <span className="text-[var(--color-text-tertiary)]"> (por defecto)</span>}
                    </td>
                    <td className="px-3 py-2">{r.cod_ped ?? '—'}</td>
                    <td className="px-3 py-2 text-[var(--color-text-tertiary)]">{[r.distrito, r.provincia, r.departamento].filter(Boolean).join(', ') || '—'}</td>
                    <td className="px-3 py-2">
                      {r._valid ? (
                        <span className="text-green-600 text-xs">OK</span>
                      ) : (
                        <span className="text-red-600 text-xs">{r._error}</span>
                      )}
                    </td>
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
        </div>
      )}

      {/* Report */}
      {report && <BulkReport report={report} />}
    </div>
  )
}

function BulkReport({ report }: { report: BulkProvisionResponse }) {
  const badge = (s: BulkRowResult['status']) => {
    if (s === 'created') return <span className="inline-flex items-center gap-1 text-green-700 text-xs"><CheckCircle2 className="w-3.5 h-3.5" /> Creada</span>
    if (s === 'skipped') return <span className="inline-flex items-center gap-1 text-amber-600 text-xs"><MinusCircle className="w-3.5 h-3.5" /> Omitida</span>
    return <span className="inline-flex items-center gap-1 text-red-600 text-xs"><XCircle className="w-3.5 h-3.5" /> Error</span>
  }

  return (
    <div className="bg-[var(--color-bg-card)] rounded-xl shadow-sm border border-[var(--color-border-primary)] overflow-hidden">
      <div className="px-5 py-3 border-b border-[var(--color-border-primary)] flex flex-wrap gap-4 text-sm">
        <span className="font-semibold text-[var(--color-text-primary)]">Resultado:</span>
        <span className="text-[var(--color-text-secondary)]">Total {report.total}</span>
        <span className="text-green-700">Creadas {report.created}</span>
        <span className="text-amber-600">Omitidas {report.skipped}</span>
        <span className="text-red-600">Errores {report.errors}</span>
      </div>
      <div className="overflow-x-auto max-h-96">
        <table className="min-w-full text-sm">
          <thead className="bg-[var(--color-bg-tertiary)] sticky top-0">
            <tr className="text-left text-xs uppercase text-[var(--color-text-tertiary)]">
              <th className="px-3 py-2">#</th>
              <th className="px-3 py-2">Username</th>
              <th className="px-3 py-2">Estado</th>
              <th className="px-3 py-2">Password</th>
              <th className="px-3 py-2">Detalle</th>
            </tr>
          </thead>
          <tbody className="divide-y divide-[var(--color-border-primary)]">
            {report.results.map((r) => (
              <tr key={r.row}>
                <td className="px-3 py-2 text-[var(--color-text-tertiary)]">{r.row}</td>
                <td className="px-3 py-2 font-medium">{r.username}</td>
                <td className="px-3 py-2">{badge(r.status)}</td>
                <td className="px-3 py-2">
                  {r.password ? <code className="bg-[var(--color-bg-tertiary)] px-2 py-0.5 rounded">{r.password}</code> : '—'}
                </td>
                <td className="px-3 py-2 text-[var(--color-text-tertiary)]">{r.message ?? '—'}</td>
              </tr>
            ))}
          </tbody>
        </table>
      </div>
    </div>
  )
}
