#!/usr/bin/env bash
# =============================================================================
# reporte_trafico_integratel.sh — Informe de tráfico con INTEGRATEL (ambos sentidos)
# =============================================================================
# Genera un informe Markdown en DOS PARTES:
#   PARTE I  — Tráfico SALIENTE (outbound) que, según el ruteo configurado en
#              `routing_outbound_routes` / `routing_trunks`, se envía por la
#              interconexión con el trunk INTEGRATEL.
#   PARTE II — Tráfico ENTRANTE (inbound) que INTEGRATEL nos envía, según el
#              ruteo configurado en `routing_inbound_routes` / `routing_trunks`.
#
# A diferencia del ruteo en Kamailio/dr_rules, la tabla `cdrs` de ApoloBilling
# NO guarda qué trunk/gateway cursó cada llamada. Este script reconstruye esa
# clasificación aplicando el mismo criterio que usa el motor de ruteo:
#
#   - SALIENTE: para cada `called_number` busca, entre las rutas salientes
#     habilitadas, el `prefix_pattern` más largo que matchea (LPM), y se queda
#     con las llamadas cuyo trunk (o algún miembro del trunk group) resuelto
#     es INTEGRATEL.
#   - ENTRANTE: para cada `called_number` (nuestro DID) busca, entre las rutas
#     entrantes habilitadas, el `did_pattern` (regex) que matchea, ordenando
#     por `priority` ASC (igual convención que el LPM saliente: la regla más
#     específica tiene prioridad numérica menor). El trunk de origen se
#     resuelve haciendo match del `source_ip_pattern` de la ruta contra el
#     `host` configurado en `routing_trunks` — así se sabe si el DID matcheado
#     corresponde a una ruta cuyo origen esperado es la IP de INTEGRATEL.
#
# Ambas son CLASIFICACIONES POR CONFIGURACIÓN, no una confirmación por-CDR de
# qué trunk cursó realmente cada llamada (si Kamailio hizo failover a otro
# trunk, o si el origen real difiere del `source_ip_pattern` configurado, este
# informe no puede detectarlo — ver secciones 2 y 14 para el detalle exacto de
# las reglas usadas en cada clasificación).
#
# Uso:
#   ./reporte_trafico_integratel.sh                              # mes en curso
#   ./reporte_trafico_integratel.sh --mes-anterior                # mes calendario anterior
#   ./reporte_trafico_integratel.sh --dias 30                     # últimos 30 días
#   ./reporte_trafico_integratel.sh --desde 2026-07-01 --hasta 2026-07-31
#   ./reporte_trafico_integratel.sh --trunk "Integratel"           # override nombre trunk (default)
#   ./reporte_trafico_integratel.sh --salida /ruta/custom.md
#
# Salida automática en: <dir del script>/../informes/trafico_integratel_<periodo>.md
# =============================================================================

set -uo pipefail

# ── Conexión BD ───────────────────────────────────────────────────────────────
SCRIPT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
APOLO_DIR="$(cd "$SCRIPT_DIR/.." && pwd)"
ENV_FILE="$APOLO_DIR/rust-backend/.env"

PG_USER="apolo"
PG_PASS=""
PG_HOST="localhost"
PG_DB="apolo_billing"

if [[ -f "$ENV_FILE" ]]; then
    PG_URL=$(grep -E '^DATABASE_URL=' "$ENV_FILE" | head -1 | cut -d'=' -f2-)
    if [[ -n "${PG_URL:-}" ]]; then
        PG_USER=$(echo "$PG_URL" | sed -n 's|.*://\([^:]*\):.*|\1|p')
        PG_PASS=$(echo "$PG_URL" | sed -n 's|.*://[^:]*:\([^@]*\)@.*|\1|p')
        PG_HOST_PARSED=$(echo "$PG_URL" | sed -n 's|.*@\([^:/]*\).*|\1|p')
        PG_DB_PARSED=$(echo "$PG_URL" | sed -n 's|.*/\([^/?]*\)\(\?.*\)\?$|\1|p')
        [[ -n "$PG_HOST_PARSED" ]] && PG_HOST="$PG_HOST_PARSED"
        [[ -n "$PG_DB_PARSED" ]] && PG_DB="$PG_DB_PARSED"
    fi
fi

# ── Directorios ───────────────────────────────────────────────────────────────
INFORMES_DIR="${APOLO_DIR}/informes"
TMPDIR_PY="$(mktemp -d)"
trap 'rm -rf "$TMPDIR_PY"' EXIT

# ── Parámetros ────────────────────────────────────────────────────────────────
MODO="mes"
DIAS=30
DESDE=""
HASTA=""
DESDE_ORIG=""
HASTA_ORIG=""
SALIDA_OVERRIDE=""
TRUNK_NAME="Integratel"

while [[ $# -gt 0 ]]; do
    case "$1" in
        --mes-anterior) MODO="mes_anterior"; shift ;;
        --dias)         MODO="dias";  DIAS="$2";  shift 2 ;;
        --desde)        MODO="rango"; DESDE="$2"; DESDE_ORIG="$2"; shift 2 ;;
        --hasta)                      HASTA="$2"; HASTA_ORIG="$2"; shift 2 ;;
        --salida)       SALIDA_OVERRIDE="$2";      shift 2 ;;
        --trunk)        TRUNK_NAME="$2";            shift 2 ;;
        *) echo "Opción desconocida: $1"; exit 1 ;;
    esac
done

case "$MODO" in
    mes)
        DESDE=$(date '+%Y-%m-01')
        DESDE_ORIG="$DESDE"
        HASTA=$(date -d "$(date '+%Y-%m-01') +1 month" '+%Y-%m-%d')
        HASTA_ORIG=$(date -d "$HASTA -1 day" '+%Y-%m-%d')
        LABEL="Mes en curso — $(date '+%B %Y')"
        FILE_PERIOD="$(date '+%Y-%m')"
        ;;
    mes_anterior)
        DESDE=$(date -d "$(date '+%Y-%m-01') -1 month" '+%Y-%m-%d')
        DESDE_ORIG="$DESDE"
        HASTA=$(date '+%Y-%m-01')
        HASTA_ORIG=$(date -d "$HASTA -1 day" '+%Y-%m-%d')
        LABEL="Mes anterior — $(date -d "$DESDE" '+%B %Y')"
        FILE_PERIOD="$(date -d "$DESDE" '+%Y-%m')"
        ;;
    dias)
        DESDE=$(date -d "-${DIAS} days" '+%Y-%m-%d')
        DESDE_ORIG="$DESDE"
        HASTA=$(date -d "+1 day" '+%Y-%m-%d')
        HASTA_ORIG=$(date '+%Y-%m-%d')
        LABEL="Últimos ${DIAS} días (${DESDE_ORIG} → ${HASTA_ORIG})"
        FILE_PERIOD="ultimos${DIAS}d_$(date '+%Y-%m-%d')"
        ;;
    rango)
        [[ -z "$DESDE" || -z "$HASTA" ]] && { echo "ERROR: --desde y --hasta son requeridos juntos."; exit 1; }
        HASTA_ORIG="$HASTA"
        HASTA=$(date -d "${HASTA} +1 day" '+%Y-%m-%d')
        LABEL="Período ${DESDE_ORIG} → ${HASTA_ORIG}"
        FILE_PERIOD="${DESDE_ORIG}_${HASTA_ORIG}"
        ;;
esac

# ── Archivo de salida ─────────────────────────────────────────────────────────
mkdir -p "$INFORMES_DIR"
TRUNK_SLUG=$(printf '%s' "$TRUNK_NAME" | tr '[:upper:]' '[:lower:]' | tr -cs 'a-z0-9' '_')
TRUNK_SLUG="${TRUNK_SLUG#_}"
TRUNK_SLUG="${TRUNK_SLUG%_}"
if [[ -n "$SALIDA_OVERRIDE" ]]; then
    OUTPATH="$SALIDA_OVERRIDE"
else
    OUTPATH="${INFORMES_DIR}/trafico_${TRUNK_SLUG}_${FILE_PERIOD}.md"
fi

exec > >(tee "$OUTPATH") 2>&1
GENERADO=$(date '+%Y-%m-%d %H:%M:%S')

# ── Helper Python: CSV → tabla Markdown con barras en columnas % ──────────────
cat > "$TMPDIR_PY/csv2md.py" << 'PYEOF'
import sys, csv, re

BAR_WIDTH = 22
PCT_RE = re.compile(r'%|ASR|NER', re.IGNORECASE)

def make_bar(val_str):
    try:
        v = float(re.sub(r'[^\d.]', '', val_str.replace(',', '.')))
        v = min(max(v, 0), 100)
        f = round(v / 100 * BAR_WIDTH)
        return '█' * f + '░' * (BAR_WIDTH - f)
    except Exception:
        return ''

raw = [l for l in sys.stdin if not re.match(r'^\(\d+ rows?\)\s*$', l.strip())]
rows = list(csv.reader(raw))
if not rows:
    print("_(sin datos en el período)_\n")
    sys.exit()

header = list(rows[0])
data   = [list(r) for r in rows[1:] if r]

if not data:
    print("_(sin datos en el período)_\n")
    sys.exit()

pct_idx = [i for i, h in enumerate(header) if PCT_RE.search(h)]

offset = 0
for i in sorted(pct_idx):
    pos = i + 1 + offset
    header.insert(pos, 'Gráfica')
    for row in data:
        src = i + offset
        row.insert(pos, make_bar(row[src] if src < len(row) else ''))
    offset += 1

widths = [len(h) for h in header]
for row in data:
    for j, cell in enumerate(row):
        if j < len(widths):
            widths[j] = max(widths[j], len(str(cell or '')))

def fmt(r):
    return '| ' + ' | '.join(str(c or '').ljust(w) for c, w in zip(r, widths)) + ' |'

def sep():
    parts = []
    for j, w in enumerate(widths):
        if PCT_RE.search(header[j]) or header[j] == 'Gráfica':
            parts.append(':' + '-' * w + ':')
        else:
            parts.append('-' * (w + 2))
    return '|' + '|'.join(parts) + '|'

print(fmt(header))
print(sep())
for row in data:
    padded = row + [''] * max(0, len(header) - len(row))
    print(fmt(padded))
print()
PYEOF

# ── Helper Python: gráfica de barras horizontal ───────────────────────────────
cat > "$TMPDIR_PY/barchart.py" << 'PYEOF'
import sys, csv, re

BAR_WIDTH = 30

def main():
    label_col = sys.argv[1] if len(sys.argv) > 1 else ''
    value_col = sys.argv[2] if len(sys.argv) > 2 else ''

    raw  = [l for l in sys.stdin if not re.match(r'^\(\d+ rows?\)\s*$', l.strip())]
    rows = list(csv.reader(raw))
    if len(rows) < 2:
        print("_(sin datos en el período)_\n")
        return

    header = rows[0]
    data   = [r for r in rows[1:] if r]

    li = header.index(label_col) if label_col in header else 0
    vi = header.index(value_col) if value_col in header else 1

    values = []
    for row in data:
        lbl = row[li] if li < len(row) else '?'
        try:
            v = float(re.sub(r'[^\d.]', '', row[vi].replace(',', '.'))) if vi < len(row) else 0
        except Exception:
            v = 0
        values.append((lbl, v))

    if not values:
        print("_(sin datos en el período)_\n")
        return

    max_v   = max(v for _, v in values) or 1
    max_lbl = max(len(l) for l, _ in values)

    print('```')
    for lbl, v in values:
        f   = round(v / max_v * BAR_WIDTH)
        bar = '█' * f + '░' * (BAR_WIDTH - f)
        print(f'{lbl:<{max_lbl}}  {bar}  {v:,.1f}')
    print('```')
    print()

main()
PYEOF

# ── Funciones de conexión ──────────────────────────────────────────────────────
run_psql_raw() {
    # $1 = SQL. Intenta con credenciales de .env; si falla, cae a sudo -u postgres.
    local out
    out=$(PGPASSWORD="$PG_PASS" psql -U "$PG_USER" -h "$PG_HOST" -d "$PG_DB" \
            --csv -P footer=off -q -c "$1" 2>/tmp/reporte_integratel_psql_err.$$)
    if [[ $? -ne 0 ]]; then
        out=$(sudo -u postgres psql -d "$PG_DB" --csv -P footer=off -q -c "$1" 2>/tmp/reporte_integratel_psql_err.$$)
    fi
    rm -f /tmp/reporte_integratel_psql_err.$$
    printf '%s\n' "$out"
}

psql_md() {
    run_psql_raw "$1" | python3 "$TMPDIR_PY/csv2md.py"
}

bar_chart() {
    # $1=label_col  $2=value_col  $3=query
    run_psql_raw "$3" | python3 "$TMPDIR_PY/barchart.py" "$1" "$2"
}

# ── CTE base: clasificación de tráfico Integratel por LPM sobre el ruteo ──────
# `routes`: para cada regla de ruteo saliente habilitada, resuelve el/los
#           nombre(s) de trunk (directo, o todos los miembros si es grupo).
# `ic`    : CDRs outbound del período + el trunk resuelto por LPM (prefijo
#           más largo que matchea called_number, desempate por priority ASC).
read -r -d '' BASE_CTE << SQLEOF
WITH routes AS (
    SELECT
        r.prefix_pattern,
        r.priority,
        r.name AS route_name,
        COALESCE(
            t1.name,
            (SELECT string_agg(t.name, ', ' ORDER BY m.priority)
             FROM routing_trunk_group_members m
             JOIN routing_trunks t ON t.id = m.trunk_id
             WHERE m.group_id = r.trunk_group_id)
        ) AS trunk_names
    FROM routing_outbound_routes r
    LEFT JOIN routing_trunks t1 ON t1.id = r.trunk_id
    WHERE r.enabled = true
),
ic AS (
    SELECT
        c.*,
        (SELECT rt.trunk_names FROM routes rt
         WHERE c.called_number LIKE rt.prefix_pattern || '%'
         ORDER BY length(rt.prefix_pattern) DESC, rt.priority ASC
         LIMIT 1) AS matched_trunk,
        (SELECT rt.prefix_pattern FROM routes rt
         WHERE c.called_number LIKE rt.prefix_pattern || '%'
         ORDER BY length(rt.prefix_pattern) DESC, rt.priority ASC
         LIMIT 1) AS matched_prefix
    FROM cdrs c
    WHERE c.direction = 'outbound'
      AND c.start_time >= '${DESDE}' AND c.start_time < '${HASTA}'
)
SQLEOF
export BASE_CTE

# Encadena una query al WITH base. Si la query trae su propio WITH (CTEs
# adicionales, p.ej. secciones 5 y 11), no se puede pegar un segundo "WITH"
# después del BASE_CTE (sintaxis inválida) — se convierte en una coma para
# que quede como parte de la misma cláusula WITH.
_chain_cte() {
    # Trim de espacios/saltos de línea que hace bash puro (respeta saltos de
    # línea internos, a diferencia de un sed por-línea con ^).
    local q="$1" trimmed
    trimmed="${q#"${q%%[![:space:]]*}"}"
    if [[ "$trimmed" == WITH* || "$trimmed" == with* ]]; then
        trimmed="${trimmed#????}"                                   # quita "WITH"
        trimmed="${trimmed#"${trimmed%%[![:space:]]*}"}"            # quita el espacio/salto que sigue
        printf ',%s' "$trimmed"
    else
        printf '%s' "$q"
    fi
}

run_q() {
    psql_md "${BASE_CTE}
$(_chain_cte "$1")"
}

run_bar() {
    bar_chart "$1" "$2" "${BASE_CTE}
$(_chain_cte "$3")"
}

# ── CTE base ENTRANTE: clasificación de tráfico Integratel por DID/origen ────
# `routes_in`: para cada regla de ruteo entrante habilitada con
#              `source_ip_pattern` configurado, resuelve el trunk cuyo `host`
#              matchea ese patrón (el origen esperado de la llamada).
# `icc`      : CDRs inbound del período + el trunk resuelto vía LATERAL JOIN,
#              matcheando `called_number` (nuestro DID) contra el `did_pattern`
#              (regex) de la ruta, priorizando por `priority` ASC (la regla
#              más específica tiene prioridad numérica menor — misma
#              convención que el LPM saliente). LATERAL en vez de subquery
#              correlacionada por volumen: inbound puede ser 2 órdenes de
#              magnitud mayor que outbound.
read -r -d '' BASE_CTE_IN << SQLEOF
WITH routes_in AS (
    SELECT
        r.did_pattern,
        r.priority,
        r.name AS route_name,
        t.name AS trunk_name
    FROM routing_inbound_routes r
    JOIN routing_trunks t ON t.host ~ r.source_ip_pattern
    WHERE r.enabled = true
      AND r.source_ip_pattern IS NOT NULL AND r.source_ip_pattern <> ''
),
icc AS (
    SELECT
        c.*,
        m.trunk_name  AS matched_trunk,
        m.route_name  AS matched_route,
        m.did_pattern AS matched_pattern
    FROM cdrs c
    LEFT JOIN LATERAL (
        SELECT rt.trunk_name, rt.route_name, rt.did_pattern
        FROM routes_in rt
        WHERE c.called_number ~ rt.did_pattern
        ORDER BY rt.priority ASC
        LIMIT 1
    ) m ON true
    WHERE c.direction = 'inbound'
      AND c.start_time >= '${DESDE}' AND c.start_time < '${HASTA}'
)
SQLEOF
export BASE_CTE_IN

run_q_in() {
    psql_md "${BASE_CTE_IN}
$(_chain_cte "$1")"
}

run_bar_in() {
    bar_chart "$1" "$2" "${BASE_CTE_IN}
$(_chain_cte "$3")"
}

# ── Validación: ¿existen rutas hacia el trunk pedido? ─────────────────────────
ROUTE_CHECK=$(run_psql_raw "
SELECT count(*) FROM routing_outbound_routes r
LEFT JOIN routing_trunks t1 ON t1.id = r.trunk_id
LEFT JOIN routing_trunk_group_members m ON m.group_id = r.trunk_group_id
LEFT JOIN routing_trunks t2 ON t2.id = m.trunk_id
WHERE r.enabled = true AND (t1.name ILIKE '%${TRUNK_NAME}%' OR t2.name ILIKE '%${TRUNK_NAME}%');
" | tail -n +2 | tr -d '[:space:]')

ROUTE_CHECK_IN=$(run_psql_raw "
SELECT count(*) FROM routing_inbound_routes r
JOIN routing_trunks t ON t.host ~ r.source_ip_pattern
WHERE r.enabled = true AND r.source_ip_pattern IS NOT NULL AND r.source_ip_pattern <> ''
  AND t.name ILIKE '%${TRUNK_NAME}%';
" | tail -n +2 | tr -d '[:space:]')

if [[ ( -z "$ROUTE_CHECK" || "$ROUTE_CHECK" == "0" ) && ( -z "$ROUTE_CHECK_IN" || "$ROUTE_CHECK_IN" == "0" ) ]]; then
    echo "ERROR: no hay reglas salientes ni entrantes habilitadas que apunten a un trunk que matchee '${TRUNK_NAME}'." >&2
    echo "Trunks disponibles:" >&2
    run_psql_raw "SELECT name, trunk_type, host FROM routing_trunks ORDER BY name;" >&2
    rm -f "$OUTPATH"
    exit 1
fi
if [[ -z "$ROUTE_CHECK" || "$ROUTE_CHECK" == "0" ]]; then
    echo "AVISO: sin reglas en routing_outbound_routes para '${TRUNK_NAME}' — se omite la Parte I (saliente)." >&2
fi
if [[ -z "$ROUTE_CHECK_IN" || "$ROUTE_CHECK_IN" == "0" ]]; then
    echo "AVISO: sin reglas en routing_inbound_routes (con source_ip_pattern) para '${TRUNK_NAME}' — se omite la Parte II (entrante)." >&2
fi

# =============================================================================
# INFORME
# =============================================================================
echo "# Informe de Tráfico — Interconexión ${TRUNK_NAME} (Saliente + Entrante)"
echo ""
echo "> **Período:** ${LABEL}  "
echo "> **Rango exacto:** \`${DESDE_ORIG}\` → \`${HASTA_ORIG}\`  "
echo "> **Generado:** ${GENERADO}  "
echo "> **BD:** \`${PG_DB}\` | **Tabla:** \`public.cdrs\` (particionada por mes) | **Direcciones:** outbound + inbound  "
echo "> **Trunk analizado:** \`${TRUNK_NAME}\`"
echo ""
echo "> ⚠️ **Metodología:** ApoloBilling no guarda en el CDR qué trunk/gateway"
echo "> cursó cada llamada. Este informe clasifica cada CDR aplicando el mismo"
echo "> criterio que usa el motor de ruteo:"
echo ">"
echo "> - **Parte I (saliente):** Longest-Prefix-Match — \`routing_outbound_routes.prefix_pattern\`"
echo ">   más largo que matchea \`called_number\`, contra las reglas habilitadas."
echo "> - **Parte II (entrante):** \`routing_inbound_routes.did_pattern\` (regex) que matchea"
echo ">   \`called_number\` (nuestro DID), ordenando por \`priority\` ASC, cuyo"
echo ">   \`source_ip_pattern\` matchea el \`host\` configurado en \`routing_trunks\`"
echo ">   para \`${TRUNK_NAME}\` — es decir, se asume que el origen real de la"
echo ">   llamada coincide con la IP que la ruta espera recibir."
echo ">"
echo "> Si Kamailio hace failover a otro trunk del mismo grupo (saliente), o si"
echo "> el origen real de una llamada entrante difiere del \`source_ip_pattern\`"
echo "> configurado, este informe no puede detectarlo — ver secciones 2 y 14"
echo "> para la lista exacta de reglas usadas en cada clasificación."
echo ""
echo "---"
echo ""
echo "# PARTE I — Tráfico Saliente"
echo ""

if [[ -n "$ROUTE_CHECK" && "$ROUTE_CHECK" != "0" ]]; then

# ── 1. Resumen Ejecutivo ──────────────────────────────────────────────────────
echo "## 1. Resumen Ejecutivo"
echo ""
echo "### Totales hacia ${TRUNK_NAME}"
echo ""
run_q "
SELECT
    COUNT(*)                                                          AS \"Intentos\",
    COUNT(*) FILTER (WHERE billsec > 0)                              AS \"Contestadas\",
    COUNT(*) FILTER (WHERE billsec = 0 OR billsec IS NULL)           AS \"No contest.\",
    ROUND(100.0 * COUNT(*) FILTER(WHERE billsec > 0)
          / NULLIF(COUNT(*),0), 2)                                   AS \"ASR(%)\",
    ROUND(100.0 * COUNT(*) FILTER(WHERE billsec > 0 OR hangup_cause = 'USER_BUSY')
          / NULLIF(COUNT(*),0), 2)                                   AS \"NER(%)\",
    ROUND(AVG(billsec) FILTER (WHERE billsec > 0), 1)                AS \"ACD(seg)\",
    ROUND(SUM(billsec) / 60.0, 1)                                    AS \"Minutos\",
    ROUND(SUM(cost), 2)                                               AS \"Costo total\"
FROM ic
WHERE matched_trunk ILIKE '%${TRUNK_NAME}%';
"

echo "### Participación de ${TRUNK_NAME} sobre el total saliente"
echo ""
run_q "
SELECT
    (SELECT COUNT(*) FROM ic WHERE matched_trunk ILIKE '%${TRUNK_NAME}%')      AS \"Intentos ${TRUNK_NAME}\",
    (SELECT COUNT(*) FROM ic)                                                  AS \"Intentos totales outbound\",
    ROUND(100.0 * (SELECT COUNT(*) FROM ic WHERE matched_trunk ILIKE '%${TRUNK_NAME}%')
          / NULLIF((SELECT COUNT(*) FROM ic), 0), 2)                           AS \"% intentos\",
    ROUND((SELECT SUM(billsec) FROM ic WHERE matched_trunk ILIKE '%${TRUNK_NAME}%') / 60.0, 1) AS \"Minutos ${TRUNK_NAME}\",
    ROUND((SELECT SUM(billsec) FROM ic) / 60.0, 1)                             AS \"Minutos totales outbound\",
    ROUND(100.0 * (SELECT SUM(billsec) FROM ic WHERE matched_trunk ILIKE '%${TRUNK_NAME}%')
          / NULLIF((SELECT SUM(billsec) FROM ic), 0), 2)                       AS \"% minutos\";
"

echo "### Monedas involucradas"
echo ""
run_q "
SELECT COALESCE(a.currency, 'sin cuenta')                            AS \"Moneda\",
       COUNT(*)                                                       AS \"CDRs\",
       ROUND(SUM(ic.cost), 2)                                        AS \"Costo\"
FROM ic
LEFT JOIN accounts a ON a.id = ic.account_id
WHERE ic.matched_trunk ILIKE '%${TRUNK_NAME}%'
GROUP BY a.currency ORDER BY COUNT(*) DESC;
"

# ── 2. Prefijos clasificados hacia el trunk ───────────────────────────────────
echo "---"
echo ""
echo "## 2. Prefijos de Ruteo Clasificados hacia ${TRUNK_NAME}"
echo ""
echo "> Reglas activas en \`routing_outbound_routes\` cuyo trunk resuelto matchea"
echo "> \`${TRUNK_NAME}\`. Éste es el criterio usado en todo el informe."
echo ""
run_q "
SELECT r.prefix_pattern                                               AS \"Prefijo\",
       r.priority                                                     AS \"Prioridad\",
       r.name                                                         AS \"Nombre ruta\",
       COALESCE(
           t1.name,
           (SELECT string_agg(t.name, ', ' ORDER BY m.priority)
            FROM routing_trunk_group_members m
            JOIN routing_trunks t ON t.id = m.trunk_id
            WHERE m.group_id = r.trunk_group_id)
       )                                                               AS \"Trunk(s)\"
FROM routing_outbound_routes r
LEFT JOIN routing_trunks t1 ON t1.id = r.trunk_id
WHERE r.enabled = true
  AND COALESCE(
        t1.name,
        (SELECT string_agg(t.name, ', ')
         FROM routing_trunk_group_members m
         JOIN routing_trunks t ON t.id = m.trunk_id
         WHERE m.group_id = r.trunk_group_id)
      ) ILIKE '%${TRUNK_NAME}%'
ORDER BY length(r.prefix_pattern) DESC, r.priority;
"

# ── 3. Desglose por prefijo destino ───────────────────────────────────────────
echo "---"
echo ""
echo "## 3. Tráfico por Prefijo de Destino"
echo ""
run_q "
SELECT COALESCE(matched_prefix, 'sin match')                          AS \"Prefijo\",
       COUNT(*)                                                       AS \"Intentos\",
       COUNT(*) FILTER (WHERE billsec > 0)                           AS \"Contest.\",
       ROUND(100.0 * COUNT(*) FILTER(WHERE billsec > 0)
             / NULLIF(COUNT(*),0), 2)                                AS \"ASR(%)\",
       ROUND(AVG(billsec) FILTER (WHERE billsec > 0), 1)             AS \"ACD(seg)\",
       ROUND(SUM(billsec) / 60.0, 1)                                 AS \"Minutos\",
       ROUND(SUM(cost), 2)                                           AS \"Costo\",
       ROUND(100.0 * SUM(cost)
             / NULLIF(SUM(SUM(cost)) OVER (), 0), 2)                  AS \"% costo\"
FROM ic
WHERE matched_trunk ILIKE '%${TRUNK_NAME}%'
GROUP BY matched_prefix ORDER BY SUM(billsec) DESC NULLS LAST;
"

echo "#### Minutos por prefijo"
echo ""
run_bar "Prefijo" "Minutos" "
SELECT COALESCE(matched_prefix,'sin match') AS \"Prefijo\",
       ROUND(SUM(billsec)/60.0,1)           AS \"Minutos\"
FROM ic
WHERE matched_trunk ILIKE '%${TRUNK_NAME}%'
GROUP BY matched_prefix ORDER BY SUM(billsec) DESC NULLS LAST;
"

# ── 4. Causas de liberación ───────────────────────────────────────────────────
echo "---"
echo ""
echo "## 4. Causas de Liberación — Top 15"
echo ""
echo "> ApoloBilling guarda la causa de FreeSWITCH como texto (no código Q.850)."
echo ""
run_q "
SELECT COALESCE(hangup_cause, 'DESCONOCIDO')                          AS \"Causa hangup\",
       COUNT(*)                                                       AS \"CDRs\",
       ROUND(100.0 * COUNT(*) / NULLIF(SUM(COUNT(*)) OVER(),0), 2)   AS \"% total\",
       COUNT(*) FILTER (WHERE billsec > 0)                           AS \"Contest.\",
       ROUND(AVG(billsec) FILTER (WHERE billsec > 0), 1)             AS \"ACD(seg)\",
       ROUND(SUM(billsec) / 60.0, 1)                                 AS \"Minutos\"
FROM ic
WHERE matched_trunk ILIKE '%${TRUNK_NAME}%'
GROUP BY hangup_cause ORDER BY COUNT(*) DESC LIMIT 15;
"

echo "#### Top causas de liberación"
echo ""
run_bar "Causa hangup" "CDRs" "
SELECT COALESCE(hangup_cause,'DESCONOCIDO') AS \"Causa hangup\", COUNT(*) AS \"CDRs\"
FROM ic
WHERE matched_trunk ILIKE '%${TRUNK_NAME}%'
GROUP BY hangup_cause ORDER BY COUNT(*) DESC LIMIT 10;
"

# ── 5. Distribución de duración ───────────────────────────────────────────────
echo "---"
echo ""
echo "## 5. Distribución de Duración"
echo ""
run_q "
WITH r AS (
    SELECT
        CASE
            WHEN billsec IS NULL OR billsec = 0   THEN 0
            WHEN billsec BETWEEN 1   AND 30        THEN 1
            WHEN billsec BETWEEN 31  AND 60        THEN 2
            WHEN billsec BETWEEN 61  AND 120       THEN 3
            WHEN billsec BETWEEN 121 AND 300       THEN 4
            WHEN billsec BETWEEN 301 AND 600       THEN 5
            WHEN billsec BETWEEN 601 AND 1800      THEN 6
            ELSE 7
        END AS ord,
        CASE
            WHEN billsec IS NULL OR billsec = 0   THEN '0 seg (no contest.)'
            WHEN billsec BETWEEN 1   AND 30        THEN '1-30 seg'
            WHEN billsec BETWEEN 31  AND 60        THEN '31-60 seg'
            WHEN billsec BETWEEN 61  AND 120       THEN '1-2 min'
            WHEN billsec BETWEEN 121 AND 300       THEN '2-5 min'
            WHEN billsec BETWEEN 301 AND 600       THEN '5-10 min'
            WHEN billsec BETWEEN 601 AND 1800      THEN '10-30 min'
            ELSE '> 30 min'
        END AS rango, billsec
    FROM ic
    WHERE matched_trunk ILIKE '%${TRUNK_NAME}%'
)
SELECT rango                                                          AS \"Rango duración\",
       COUNT(*)                                                       AS \"CDRs\",
       ROUND(100.0 * COUNT(*) / NULLIF(SUM(COUNT(*)) OVER(),0), 2)   AS \"% total\",
       ROUND(SUM(billsec) / 60.0, 1)                                 AS \"Min acum\"
FROM r GROUP BY ord, rango ORDER BY ord;
"

# ── 6. Distribución horaria ────────────────────────────────────────────────────
echo "---"
echo ""
echo "## 6. Distribución Horaria (hora Lima)"
echo ""
run_q "
SELECT
    LPAD(EXTRACT(HOUR FROM start_time AT TIME ZONE 'America/Lima')::text,2,'0') || ':00' AS \"Hora\",
    COUNT(*)                                                          AS \"Intentos\",
    COUNT(*) FILTER (WHERE billsec > 0)                              AS \"Contest.\",
    ROUND(100.0 * COUNT(*) FILTER(WHERE billsec > 0)
          / NULLIF(COUNT(*),0), 1)                                   AS \"ASR(%)\",
    ROUND(SUM(billsec) / 60.0, 1)                                    AS \"Minutos\",
    ROUND(SUM(cost), 2)                                                AS \"Costo\"
FROM ic
WHERE matched_trunk ILIKE '%${TRUNK_NAME}%'
GROUP BY 1 ORDER BY 1;
"

echo "#### Volumen de llamadas por hora del día"
echo ""
run_bar "Hora" "Intentos" "
SELECT LPAD(EXTRACT(HOUR FROM start_time AT TIME ZONE 'America/Lima')::text,2,'0')||':00' AS \"Hora\",
       COUNT(*) AS \"Intentos\"
FROM ic
WHERE matched_trunk ILIKE '%${TRUNK_NAME}%'
GROUP BY 1 ORDER BY 1;
"

# ── 7. Distribución diaria ────────────────────────────────────────────────────
echo "---"
echo ""
echo "## 7. Distribución Diaria"
echo ""
run_q "
SELECT
    (start_time AT TIME ZONE 'America/Lima')::date                   AS \"Fecha\",
    COUNT(*)                                                         AS \"Intentos\",
    COUNT(*) FILTER (WHERE billsec > 0)                             AS \"Contest.\",
    ROUND(100.0 * COUNT(*) FILTER(WHERE billsec > 0)
          / NULLIF(COUNT(*),0), 2)                                  AS \"ASR(%)\",
    ROUND(AVG(billsec) FILTER (WHERE billsec > 0), 1)                AS \"ACD(seg)\",
    ROUND(SUM(billsec) / 60.0, 1)                                   AS \"Minutos\",
    ROUND(SUM(cost), 2)                                                AS \"Costo\"
FROM ic
WHERE matched_trunk ILIKE '%${TRUNK_NAME}%'
GROUP BY 1 ORDER BY 1;
"

echo "#### Llamadas diarias"
echo ""
run_bar "Fecha" "Intentos" "
SELECT (start_time AT TIME ZONE 'America/Lima')::date::text AS \"Fecha\",
       COUNT(*) AS \"Intentos\"
FROM ic
WHERE matched_trunk ILIKE '%${TRUNK_NAME}%'
GROUP BY 1 ORDER BY 1;
"

# ── 8. Top destinos ────────────────────────────────────────────────────────────
echo "---"
echo ""
echo "## 8. Top 20 Destinos"
echo ""
run_q "
SELECT ic.called_number                                               AS \"Destino\",
       COALESCE(rc.destination_name, '-')                            AS \"Zona (tarifa)\",
       COUNT(*)                                                       AS \"Intentos\",
       COUNT(*) FILTER (WHERE ic.billsec > 0)                        AS \"Contest.\",
       ROUND(100.0 * COUNT(*) FILTER(WHERE ic.billsec > 0)
             / NULLIF(COUNT(*),0), 2)                                AS \"ASR(%)\",
       ROUND(AVG(ic.billsec) FILTER (WHERE ic.billsec > 0), 1)       AS \"ACD(seg)\",
       ROUND(SUM(ic.billsec) / 60.0, 1)                              AS \"Minutos\",
       ROUND(SUM(ic.cost), 2)                                        AS \"Costo\"
FROM ic
LEFT JOIN rate_cards rc ON rc.id = ic.rate_id
WHERE ic.matched_trunk ILIKE '%${TRUNK_NAME}%'
GROUP BY ic.called_number, rc.destination_name
ORDER BY COUNT(*) DESC LIMIT 20;
"

# ── 9. Top orígenes / cuentas ──────────────────────────────────────────────────
echo "---"
echo ""
echo "## 9. Top 20 Orígenes (CLI / Cuenta)"
echo ""
run_q "
SELECT ic.caller_number                                               AS \"Origen (CLI)\",
       COALESCE(a.account_number, 'sin cuenta')                      AS \"Cuenta\",
       COALESCE(a.account_name, '-')                                 AS \"Nombre cuenta\",
       COUNT(*)                                                       AS \"CDRs\",
       COUNT(*) FILTER (WHERE ic.billsec > 0)                        AS \"Contest.\",
       ROUND(SUM(ic.billsec) / 60.0, 1)                              AS \"Minutos\",
       ROUND(SUM(ic.cost), 2)                                        AS \"Costo\"
FROM ic
LEFT JOIN accounts a ON a.id = ic.account_id
WHERE ic.matched_trunk ILIKE '%${TRUNK_NAME}%'
GROUP BY ic.caller_number, a.account_number, a.account_name
ORDER BY COUNT(*) DESC LIMIT 20;
"

# ── 10. Costo por cuenta ───────────────────────────────────────────────────────
echo "---"
echo ""
echo "## 10. Top 15 Cuentas por Costo hacia ${TRUNK_NAME}"
echo ""
run_q "
SELECT COALESCE(a.account_number, 'sin cuenta')                      AS \"Cuenta\",
       COALESCE(a.account_name, '-')                                 AS \"Nombre\",
       COUNT(*)                                                       AS \"CDRs\",
       ROUND(SUM(ic.billsec) / 60.0, 1)                              AS \"Minutos\",
       ROUND(SUM(ic.cost), 2)                                        AS \"Costo\",
       ROUND(100.0 * SUM(ic.cost)
             / NULLIF(SUM(SUM(ic.cost)) OVER (), 0), 2)               AS \"% costo\"
FROM ic
LEFT JOIN accounts a ON a.id = ic.account_id
WHERE ic.matched_trunk ILIKE '%${TRUNK_NAME}%'
GROUP BY a.account_number, a.account_name
ORDER BY SUM(ic.cost) DESC NULLS LAST LIMIT 15;
"

# ── 11. PDD ───────────────────────────────────────────────────────────────────
echo "---"
echo ""
echo "## 11. PDD — Post-Dial Delay (contestadas)"
echo ""
echo "> PDD = \`answer_time - start_time\`."
echo ""
run_q "
WITH p AS (
    SELECT EXTRACT(EPOCH FROM (answer_time - start_time)) * 1000 AS pdd_ms
    FROM ic
    WHERE matched_trunk ILIKE '%${TRUNK_NAME}%' AND billsec > 0
      AND answer_time IS NOT NULL AND answer_time > start_time AND answer_time <= end_time
)
SELECT
    CASE WHEN pdd_ms < 500                 THEN '< 500 ms (excelente)'
         WHEN pdd_ms BETWEEN 500  AND 1000 THEN '500 ms - 1 s (bueno)'
         WHEN pdd_ms BETWEEN 1001 AND 2000 THEN '1-2 s (aceptable)'
         WHEN pdd_ms BETWEEN 2001 AND 4000 THEN '2-4 s (alto)'
         ELSE                                   '> 4 s (crítico)'
    END                                                              AS \"Rango PDD\",
    COUNT(*)                                                         AS \"CDRs\",
    ROUND(100.0 * COUNT(*) / NULLIF(SUM(COUNT(*)) OVER(),0), 2)     AS \"% total\"
FROM p GROUP BY 1 ORDER BY 2 DESC;
"

echo "### Estadísticas PDD"
echo ""
run_q "
SELECT
    ROUND(MIN(EXTRACT(EPOCH FROM (answer_time - start_time))*1000))  AS \"PDD mín (ms)\",
    ROUND(MAX(EXTRACT(EPOCH FROM (answer_time - start_time))*1000))  AS \"PDD máx (ms)\",
    ROUND(AVG(EXTRACT(EPOCH FROM (answer_time - start_time))*1000))  AS \"PDD prom (ms)\",
    ROUND(PERCENTILE_CONT(0.5) WITHIN GROUP (
          ORDER BY EXTRACT(EPOCH FROM (answer_time - start_time))*1000)::numeric) AS \"PDD med (ms)\",
    ROUND(PERCENTILE_CONT(0.95) WITHIN GROUP (
          ORDER BY EXTRACT(EPOCH FROM (answer_time - start_time))*1000)::numeric) AS \"PDD p95 (ms)\"
FROM ic
WHERE matched_trunk ILIKE '%${TRUNK_NAME}%' AND billsec > 0
  AND answer_time IS NOT NULL AND answer_time > start_time AND answer_time <= end_time;
"

echo "#### Distribución de PDD"
echo ""
run_bar "Rango PDD" "CDRs" "
WITH p AS (
    SELECT EXTRACT(EPOCH FROM (answer_time - start_time))*1000 AS pdd_ms
    FROM ic
    WHERE matched_trunk ILIKE '%${TRUNK_NAME}%' AND billsec > 0
      AND answer_time IS NOT NULL AND answer_time > start_time AND answer_time <= end_time
)
SELECT
    CASE WHEN pdd_ms < 500                 THEN '1. < 500ms'
         WHEN pdd_ms BETWEEN 500  AND 1000 THEN '2. 500ms-1s'
         WHEN pdd_ms BETWEEN 1001 AND 2000 THEN '3. 1-2s'
         WHEN pdd_ms BETWEEN 2001 AND 4000 THEN '4. 2-4s'
         ELSE                                   '5. > 4s'
    END AS \"Rango PDD\", COUNT(*) AS \"CDRs\"
FROM p GROUP BY 1 ORDER BY 1;
"

# ── 12. Tarifas aplicadas ──────────────────────────────────────────────────────
echo "---"
echo ""
echo "## 12. Tarifas Aplicadas (rate_per_minute usado en la tasación)"
echo ""
run_q "
SELECT COALESCE(rc.destination_name, 'sin match de tarifa')          AS \"Zona\",
       ic.rate_per_minute                                            AS \"Tarifa/min\",
       COUNT(*)                                                      AS \"CDRs\",
       ROUND(SUM(ic.billsec)/60.0, 1)                                AS \"Minutos\",
       ROUND(SUM(ic.cost), 2)                                        AS \"Costo\"
FROM ic
LEFT JOIN rate_cards rc ON rc.id = ic.rate_id
WHERE ic.matched_trunk ILIKE '%${TRUNK_NAME}%'
GROUP BY rc.destination_name, ic.rate_per_minute
ORDER BY SUM(ic.cost) DESC NULLS LAST LIMIT 20;
"

else
    echo "_(Sin reglas en \`routing_outbound_routes\` para \`${TRUNK_NAME}\` — Parte I omitida.)_"
    echo ""
fi

echo "---"
echo ""
echo "# PARTE II — Tráfico Entrante"
echo ""
echo "> Tráfico que \`${TRUNK_NAME}\` nos ENVÍA (dirección \`inbound\` en \`cdrs\`)."
echo "> \`caller_number\` = número externo que llama (PSTN) · \`called_number\` ="
echo "> nuestro DID marcado."
echo ""

if [[ -n "$ROUTE_CHECK_IN" && "$ROUTE_CHECK_IN" != "0" ]]; then

# ── 13. Resumen Ejecutivo (entrante) ──────────────────────────────────────────
echo "## 13. Resumen Ejecutivo"
echo ""
echo "### Totales recibidos desde ${TRUNK_NAME}"
echo ""
run_q_in "
SELECT
    COUNT(*)                                                          AS \"Intentos\",
    COUNT(*) FILTER (WHERE billsec > 0)                              AS \"Contestadas\",
    COUNT(*) FILTER (WHERE billsec = 0 OR billsec IS NULL)           AS \"No contest.\",
    ROUND(100.0 * COUNT(*) FILTER(WHERE billsec > 0)
          / NULLIF(COUNT(*),0), 2)                                   AS \"ASR(%)\",
    ROUND(100.0 * COUNT(*) FILTER(WHERE billsec > 0 OR hangup_cause = 'USER_BUSY')
          / NULLIF(COUNT(*),0), 2)                                   AS \"NER(%)\",
    ROUND(AVG(billsec) FILTER (WHERE billsec > 0), 1)                AS \"ACD(seg)\",
    ROUND(SUM(billsec) / 60.0, 1)                                    AS \"Minutos\",
    ROUND(SUM(cost), 2)                                               AS \"Costo total\"
FROM icc
WHERE matched_trunk ILIKE '%${TRUNK_NAME}%';
"

echo "### Participación de ${TRUNK_NAME} sobre el total entrante"
echo ""
run_q_in "
SELECT
    (SELECT COUNT(*) FROM icc WHERE matched_trunk ILIKE '%${TRUNK_NAME}%')      AS \"Intentos ${TRUNK_NAME}\",
    (SELECT COUNT(*) FROM icc)                                                  AS \"Intentos totales inbound\",
    ROUND(100.0 * (SELECT COUNT(*) FROM icc WHERE matched_trunk ILIKE '%${TRUNK_NAME}%')
          / NULLIF((SELECT COUNT(*) FROM icc), 0), 2)                           AS \"% intentos\",
    ROUND((SELECT SUM(billsec) FROM icc WHERE matched_trunk ILIKE '%${TRUNK_NAME}%') / 60.0, 1) AS \"Minutos ${TRUNK_NAME}\",
    ROUND((SELECT SUM(billsec) FROM icc) / 60.0, 1)                             AS \"Minutos totales inbound\",
    ROUND(100.0 * (SELECT SUM(billsec) FROM icc WHERE matched_trunk ILIKE '%${TRUNK_NAME}%')
          / NULLIF((SELECT SUM(billsec) FROM icc), 0), 2)                       AS \"% minutos\";
"

echo "### CDRs entrantes sin clasificar (ninguna ruta matcheó el DID)"
echo ""
echo "> Llamadas \`inbound\` cuyo \`called_number\` no matcheó ningún \`did_pattern\`"
echo "> habilitado (posible DID fuera de rango, o rutas mal configuradas)."
echo ""
run_q_in "
SELECT COUNT(*) AS \"Sin clasificar\", ROUND(SUM(billsec)/60.0,1) AS \"Minutos\"
FROM icc WHERE matched_trunk IS NULL;
"

# ── 14. Rutas entrantes clasificadas hacia el trunk ───────────────────────────
echo "---"
echo ""
echo "## 14. Rutas Entrantes Clasificadas como origen ${TRUNK_NAME}"
echo ""
echo "> Reglas activas en \`routing_inbound_routes\` cuyo \`source_ip_pattern\`"
echo "> matchea el \`host\` de \`${TRUNK_NAME}\` en \`routing_trunks\`. Éste es el"
echo "> criterio usado en toda la Parte II."
echo ""
run_q_in "
SELECT rt.did_pattern                                                 AS \"Patrón DID\",
       rt.priority                                                    AS \"Prioridad\",
       rt.route_name                                                  AS \"Nombre ruta\",
       rt.trunk_name                                                  AS \"Trunk (origen esperado)\"
FROM routes_in rt
WHERE rt.trunk_name ILIKE '%${TRUNK_NAME}%'
ORDER BY rt.priority ASC;
"

# ── 15. Desglose por patrón DID ────────────────────────────────────────────────
echo "---"
echo ""
echo "## 15. Tráfico por Patrón DID (ruta entrante matcheada)"
echo ""
run_q_in "
SELECT COALESCE(matched_route, 'sin match')                           AS \"Ruta\",
       COUNT(*)                                                       AS \"Intentos\",
       COUNT(*) FILTER (WHERE billsec > 0)                           AS \"Contest.\",
       ROUND(100.0 * COUNT(*) FILTER(WHERE billsec > 0)
             / NULLIF(COUNT(*),0), 2)                                AS \"ASR(%)\",
       ROUND(AVG(billsec) FILTER (WHERE billsec > 0), 1)             AS \"ACD(seg)\",
       ROUND(SUM(billsec) / 60.0, 1)                                 AS \"Minutos\",
       ROUND(SUM(cost), 2)                                           AS \"Costo\"
FROM icc
WHERE matched_trunk ILIKE '%${TRUNK_NAME}%'
GROUP BY matched_route ORDER BY SUM(billsec) DESC NULLS LAST;
"

echo "#### Minutos por ruta/DID"
echo ""
run_bar_in "Ruta" "Minutos" "
SELECT COALESCE(matched_route,'sin match') AS \"Ruta\",
       ROUND(SUM(billsec)/60.0,1)          AS \"Minutos\"
FROM icc
WHERE matched_trunk ILIKE '%${TRUNK_NAME}%'
GROUP BY matched_route ORDER BY SUM(billsec) DESC NULLS LAST;
"

# ── 16. Causas de liberación ───────────────────────────────────────────────────
echo "---"
echo ""
echo "## 16. Causas de Liberación — Top 15"
echo ""
run_q_in "
SELECT COALESCE(hangup_cause, 'DESCONOCIDO')                          AS \"Causa hangup\",
       COUNT(*)                                                       AS \"CDRs\",
       ROUND(100.0 * COUNT(*) / NULLIF(SUM(COUNT(*)) OVER(),0), 2)   AS \"% total\",
       COUNT(*) FILTER (WHERE billsec > 0)                           AS \"Contest.\",
       ROUND(AVG(billsec) FILTER (WHERE billsec > 0), 1)             AS \"ACD(seg)\",
       ROUND(SUM(billsec) / 60.0, 1)                                 AS \"Minutos\"
FROM icc
WHERE matched_trunk ILIKE '%${TRUNK_NAME}%'
GROUP BY hangup_cause ORDER BY COUNT(*) DESC LIMIT 15;
"

echo "#### Top causas de liberación"
echo ""
run_bar_in "Causa hangup" "CDRs" "
SELECT COALESCE(hangup_cause,'DESCONOCIDO') AS \"Causa hangup\", COUNT(*) AS \"CDRs\"
FROM icc
WHERE matched_trunk ILIKE '%${TRUNK_NAME}%'
GROUP BY hangup_cause ORDER BY COUNT(*) DESC LIMIT 10;
"

# ── 17. Distribución de duración ──────────────────────────────────────────────
echo "---"
echo ""
echo "## 17. Distribución de Duración"
echo ""
run_q_in "
WITH r AS (
    SELECT
        CASE
            WHEN billsec IS NULL OR billsec = 0   THEN 0
            WHEN billsec BETWEEN 1   AND 30        THEN 1
            WHEN billsec BETWEEN 31  AND 60        THEN 2
            WHEN billsec BETWEEN 61  AND 120       THEN 3
            WHEN billsec BETWEEN 121 AND 300       THEN 4
            WHEN billsec BETWEEN 301 AND 600       THEN 5
            WHEN billsec BETWEEN 601 AND 1800      THEN 6
            ELSE 7
        END AS ord,
        CASE
            WHEN billsec IS NULL OR billsec = 0   THEN '0 seg (no contest.)'
            WHEN billsec BETWEEN 1   AND 30        THEN '1-30 seg'
            WHEN billsec BETWEEN 31  AND 60        THEN '31-60 seg'
            WHEN billsec BETWEEN 61  AND 120       THEN '1-2 min'
            WHEN billsec BETWEEN 121 AND 300       THEN '2-5 min'
            WHEN billsec BETWEEN 301 AND 600       THEN '5-10 min'
            WHEN billsec BETWEEN 601 AND 1800      THEN '10-30 min'
            ELSE '> 30 min'
        END AS rango, billsec
    FROM icc
    WHERE matched_trunk ILIKE '%${TRUNK_NAME}%'
)
SELECT rango                                                          AS \"Rango duración\",
       COUNT(*)                                                       AS \"CDRs\",
       ROUND(100.0 * COUNT(*) / NULLIF(SUM(COUNT(*)) OVER(),0), 2)   AS \"% total\",
       ROUND(SUM(billsec) / 60.0, 1)                                 AS \"Min acum\"
FROM r GROUP BY ord, rango ORDER BY ord;
"

# ── 18. Distribución horaria ───────────────────────────────────────────────────
echo "---"
echo ""
echo "## 18. Distribución Horaria (hora Lima)"
echo ""
run_q_in "
SELECT
    LPAD(EXTRACT(HOUR FROM start_time AT TIME ZONE 'America/Lima')::text,2,'0') || ':00' AS \"Hora\",
    COUNT(*)                                                          AS \"Intentos\",
    COUNT(*) FILTER (WHERE billsec > 0)                              AS \"Contest.\",
    ROUND(100.0 * COUNT(*) FILTER(WHERE billsec > 0)
          / NULLIF(COUNT(*),0), 1)                                   AS \"ASR(%)\",
    ROUND(SUM(billsec) / 60.0, 1)                                    AS \"Minutos\",
    ROUND(SUM(cost), 2)                                                AS \"Costo\"
FROM icc
WHERE matched_trunk ILIKE '%${TRUNK_NAME}%'
GROUP BY 1 ORDER BY 1;
"

echo "#### Volumen de llamadas por hora del día"
echo ""
run_bar_in "Hora" "Intentos" "
SELECT LPAD(EXTRACT(HOUR FROM start_time AT TIME ZONE 'America/Lima')::text,2,'0')||':00' AS \"Hora\",
       COUNT(*) AS \"Intentos\"
FROM icc
WHERE matched_trunk ILIKE '%${TRUNK_NAME}%'
GROUP BY 1 ORDER BY 1;
"

# ── 19. Distribución diaria ────────────────────────────────────────────────────
echo "---"
echo ""
echo "## 19. Distribución Diaria"
echo ""
run_q_in "
SELECT
    (start_time AT TIME ZONE 'America/Lima')::date                   AS \"Fecha\",
    COUNT(*)                                                         AS \"Intentos\",
    COUNT(*) FILTER (WHERE billsec > 0)                             AS \"Contest.\",
    ROUND(100.0 * COUNT(*) FILTER(WHERE billsec > 0)
          / NULLIF(COUNT(*),0), 2)                                  AS \"ASR(%)\",
    ROUND(AVG(billsec) FILTER (WHERE billsec > 0), 1)                AS \"ACD(seg)\",
    ROUND(SUM(billsec) / 60.0, 1)                                   AS \"Minutos\",
    ROUND(SUM(cost), 2)                                                AS \"Costo\"
FROM icc
WHERE matched_trunk ILIKE '%${TRUNK_NAME}%'
GROUP BY 1 ORDER BY 1;
"

echo "#### Llamadas diarias"
echo ""
run_bar_in "Fecha" "Intentos" "
SELECT (start_time AT TIME ZONE 'America/Lima')::date::text AS \"Fecha\",
       COUNT(*) AS \"Intentos\"
FROM icc
WHERE matched_trunk ILIKE '%${TRUNK_NAME}%'
GROUP BY 1 ORDER BY 1;
"

# ── 20. Top DIDs propios receptores ────────────────────────────────────────────
echo "---"
echo ""
echo "## 20. Top 20 DIDs Propios (destino de la llamada entrante)"
echo ""
run_q_in "
SELECT icc.called_number                                              AS \"DID propio\",
       COALESCE(a.account_number, '-')                                AS \"Cuenta (si 0800/toll-free)\",
       COUNT(*)                                                       AS \"Intentos\",
       COUNT(*) FILTER (WHERE icc.billsec > 0)                       AS \"Contest.\",
       ROUND(100.0 * COUNT(*) FILTER(WHERE icc.billsec > 0)
             / NULLIF(COUNT(*),0), 2)                                AS \"ASR(%)\",
       ROUND(AVG(icc.billsec) FILTER (WHERE icc.billsec > 0), 1)     AS \"ACD(seg)\",
       ROUND(SUM(icc.billsec) / 60.0, 1)                             AS \"Minutos\",
       ROUND(SUM(icc.cost), 2)                                       AS \"Costo\"
FROM icc
LEFT JOIN accounts a ON a.id = icc.account_id
WHERE icc.matched_trunk ILIKE '%${TRUNK_NAME}%'
GROUP BY icc.called_number, a.account_number
ORDER BY COUNT(*) DESC LIMIT 20;
"

# ── 21. Top orígenes externos ──────────────────────────────────────────────────
echo "---"
echo ""
echo "## 21. Top 20 Orígenes Externos (CLI que llama)"
echo ""
run_q_in "
SELECT icc.caller_number                                              AS \"Origen (CLI externo)\",
       COUNT(*)                                                       AS \"CDRs\",
       COUNT(*) FILTER (WHERE icc.billsec > 0)                       AS \"Contest.\",
       ROUND(100.0 * COUNT(*) FILTER(WHERE icc.billsec > 0)
             / NULLIF(COUNT(*),0), 2)                                AS \"ASR(%)\",
       ROUND(SUM(icc.billsec) / 60.0, 1)                             AS \"Minutos\"
FROM icc
WHERE icc.matched_trunk ILIKE '%${TRUNK_NAME}%'
GROUP BY icc.caller_number
ORDER BY COUNT(*) DESC LIMIT 20;
"

# ── 22. Costo por cuenta (toll-free 0800/0801) ─────────────────────────────────
echo "---"
echo ""
echo "## 22. Costo por Cuenta (llamadas entrantes facturadas, ej. 0800/0801)"
echo ""
echo "> Solo aplica a rutas toll-free donde la cuenta CALLEE (dueña del 0800)"
echo "> paga la llamada — ver \"Inbound vs Outbound Calls\" en CLAUDE.md."
echo ""
run_q_in "
SELECT COALESCE(a.account_number, 'sin cuenta / no facturado')       AS \"Cuenta\",
       COALESCE(a.account_name, '-')                                 AS \"Nombre\",
       COUNT(*)                                                       AS \"CDRs\",
       ROUND(SUM(icc.billsec) / 60.0, 1)                             AS \"Minutos\",
       ROUND(SUM(icc.cost), 2)                                       AS \"Costo\"
FROM icc
LEFT JOIN accounts a ON a.id = icc.account_id
WHERE icc.matched_trunk ILIKE '%${TRUNK_NAME}%' AND icc.cost > 0
GROUP BY a.account_number, a.account_name
ORDER BY SUM(icc.cost) DESC NULLS LAST LIMIT 15;
"

# ── 23. PDD ─────────────────────────────────────────────────────────────────────
echo "---"
echo ""
echo "## 23. PDD — Post-Dial Delay (contestadas)"
echo ""
run_q_in "
WITH p AS (
    SELECT EXTRACT(EPOCH FROM (answer_time - start_time)) * 1000 AS pdd_ms
    FROM icc
    WHERE matched_trunk ILIKE '%${TRUNK_NAME}%' AND billsec > 0
      AND answer_time IS NOT NULL AND answer_time > start_time AND answer_time <= end_time
)
SELECT
    CASE WHEN pdd_ms < 500                 THEN '< 500 ms (excelente)'
         WHEN pdd_ms BETWEEN 500  AND 1000 THEN '500 ms - 1 s (bueno)'
         WHEN pdd_ms BETWEEN 1001 AND 2000 THEN '1-2 s (aceptable)'
         WHEN pdd_ms BETWEEN 2001 AND 4000 THEN '2-4 s (alto)'
         ELSE                                   '> 4 s (crítico)'
    END                                                              AS \"Rango PDD\",
    COUNT(*)                                                         AS \"CDRs\",
    ROUND(100.0 * COUNT(*) / NULLIF(SUM(COUNT(*)) OVER(),0), 2)     AS \"% total\"
FROM p GROUP BY 1 ORDER BY 2 DESC;
"

echo "### Estadísticas PDD"
echo ""
run_q_in "
SELECT
    ROUND(MIN(EXTRACT(EPOCH FROM (answer_time - start_time))*1000))  AS \"PDD mín (ms)\",
    ROUND(MAX(EXTRACT(EPOCH FROM (answer_time - start_time))*1000))  AS \"PDD máx (ms)\",
    ROUND(AVG(EXTRACT(EPOCH FROM (answer_time - start_time))*1000))  AS \"PDD prom (ms)\",
    ROUND(PERCENTILE_CONT(0.5) WITHIN GROUP (
          ORDER BY EXTRACT(EPOCH FROM (answer_time - start_time))*1000)::numeric) AS \"PDD med (ms)\",
    ROUND(PERCENTILE_CONT(0.95) WITHIN GROUP (
          ORDER BY EXTRACT(EPOCH FROM (answer_time - start_time))*1000)::numeric) AS \"PDD p95 (ms)\"
FROM icc
WHERE matched_trunk ILIKE '%${TRUNK_NAME}%' AND billsec > 0
  AND answer_time IS NOT NULL AND answer_time > start_time AND answer_time <= end_time;
"

echo "#### Distribución de PDD"
echo ""
run_bar_in "Rango PDD" "CDRs" "
WITH p AS (
    SELECT EXTRACT(EPOCH FROM (answer_time - start_time))*1000 AS pdd_ms
    FROM icc
    WHERE matched_trunk ILIKE '%${TRUNK_NAME}%' AND billsec > 0
      AND answer_time IS NOT NULL AND answer_time > start_time AND answer_time <= end_time
)
SELECT
    CASE WHEN pdd_ms < 500                 THEN '1. < 500ms'
         WHEN pdd_ms BETWEEN 500  AND 1000 THEN '2. 500ms-1s'
         WHEN pdd_ms BETWEEN 1001 AND 2000 THEN '3. 1-2s'
         WHEN pdd_ms BETWEEN 2001 AND 4000 THEN '4. 2-4s'
         ELSE                                   '5. > 4s'
    END AS \"Rango PDD\", COUNT(*) AS \"CDRs\"
FROM p GROUP BY 1 ORDER BY 1;
"

else
    echo "_(Sin reglas en \`routing_inbound_routes\` con \`source_ip_pattern\` para \`${TRUNK_NAME}\` — Parte II omitida.)_"
    echo ""
fi

# ── Footer ────────────────────────────────────────────────────────────────────
echo "---"
echo ""
echo "*Fin del informe — Período: \`${DESDE_ORIG}\` → \`${HASTA_ORIG}\`*  "
echo "*BD: \`${PG_DB}\` | Tabla: \`public.cdrs\` | Trunk: \`${TRUNK_NAME}\` | Generado: ${GENERADO}*"
echo ""
echo "> Guardado en: \`${OUTPATH}\`"
