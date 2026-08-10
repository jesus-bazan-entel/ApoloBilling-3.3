# Incidentes y mejoras — 2026-08-09

Documento de sesión de soporte sobre `winproxy` (Kamailio + FreeSWITCH + ApoloBilling).
Cubre 10 hallazgos/cambios, en el orden en que se investigaron. Cada sección tiene:
problema, evidencia de la falla, causa raíz, fix aplicado (comandos/código) y verificación.

**Alcance de los cambios:** MariaDB (Kamailio), PostgreSQL (`apolo_billing`), Kamailio
(`kamailio.cfg`), `rust-billing-engine`, `frontend`, FreeSWITCH (systemd), y config del
sistema (NTP/systemd).

---

## Índice

1. [MariaDB saturada por `WITH_CDRS`](#1-mariadb-saturada-por-with_cdrs)
2. [Tabla `kamailio.acc` sin retención](#2-tabla-kamailioacc-sin-retención)
3. [Endpoint 953: `dr_gateways.attrs` incompleto](#3-endpoint-953-dr_gatewaysattrs-incompleto)
4. [PostgreSQL: housekeeping y particionamiento de `cdrs`](#4-postgresql-housekeeping-y-particionamiento-de-cdrs)
5. [Race condition en reservas de saldo](#5-race-condition-en-reservas-de-saldo)
6. [Dirección de CDRs mal clasificada](#6-dirección-de-cdrs-mal-clasificada)
7. [Reloj de FreeSWITCH desincronizado (~5h)](#7-reloj-de-freeswitch-desincronizado-5h)
8. [Corrección masiva de `start_time`/`end_time`](#8-corrección-masiva-de-start_timeend_time)
9. [Dashboard de CDRs sin auto-refresh](#9-dashboard-de-cdrs-sin-auto-refresh)
10. [Branding del login](#10-branding-del-login)
11. [Commits de git](#11-commits-de-git)
12. [Tablas de auditoría / cómo revertir](#12-tablas-de-auditoría--cómo-revertir)

---

## 1. MariaDB saturada por `WITH_CDRS`

### Problema
MariaDB reportaba decenas de `Aborted connection ... Got an error writing communication
packets` cada ~5 minutos, y Kamailio logueaba errores reales de DB:
```
ERROR: db_mysql [km_dbase.c:129]: db_mysql_submit_query_impl(): driver error on query:
Lost connection to server during query (2013)
```

### Evidencia
- `kamailio.cfg` tiene un `rtimer` que corre cada 300s (`modparam("rtimer","timer","name=cdr;interval=300;mode=1;")`)
  y ejecuta `route[CDRS]` → `sql_query("kam","call kamailio_cdrs()","rb")`.
- El slow query log de MariaDB mostró cada ejecución tardando **82-90 segundos**, examinando
  **~2.65M filas** por corrida (full scan, sin índice adecuado sobre `acc.method`/`acc.cdr_id`).
- `kamailio.acc` tenía **1.34M filas**, de las cuales **1.29M INVITEs** seguían pendientes de
  convertir a CDR (backlog nunca procesado, el `net_write_timeout=60s` cortaba la conexión
  a mitad de query).
- El propio script de purga (`scripts/purge_kamailio_acc.sh`, agregado el mismo día) documenta:
  *"Nothing in ApoloBilling reads kamailio.acc/kamailio.cdrs — it's Kamailio's internal audit
  log"* → el pipeline real de facturación es PostgreSQL vía `rust-billing-engine`, este
  procesamiento en MySQL no beneficiaba a nadie.

### Causa raíz
Procedimiento almacenado ineficiente (cursor row-by-row, sin índice adecuado) corriendo cada
5 minutos sobre una tabla de 1.3M+ filas que crecía sin retención, para alimentar una tabla
(`kamailio.cdrs`) que ningún sistema consume.

### Fix
Deshabilitado en `/etc/kamailio/kamailio.cfg` (línea 13):
```diff
-#!define WITH_CDRS
+##!define WITH_CDRS  (disabled 2026-08-09: kamailio_cdrs() rtimer full-scans acc every 5min,
+##                    82-90s/run, causing "Lost connection to server during query" + MariaDB
+##                    aborted-connection warnings. kamailio.acc/cdrs is not consumed by
+##                    ApoloBilling - real CDR pipeline is PostgreSQL via rust-billing-engine.)
```

### Comandos ejecutados
```bash
cp -p /etc/kamailio/kamailio.cfg /etc/kamailio/kamailio.cfg.bak-20260809142146
kamailio -c -f /etc/kamailio/kamailio.cfg   # validar sintaxis, exit 0
systemctl restart kamailio
```

### Verificación
Monitor de 400s (>1 ciclo del rtimer viejo) sin ningún `Aborted connection` nuevo en MariaDB.
Servicio estable, tráfico SIP normal durante y después del restart.

---

## 2. Tabla `kamailio.acc` sin retención

### Problema
`kamailio.acc` en 1.34M filas / 367MB, sin política de retención, alimentando el problema
del punto 1.

### Fix
Backup + `TRUNCATE`, confirmado explícitamente por el usuario antes de ejecutar (acción
irreversible sobre datos de producción).

```bash
mkdir -p /var/backups/kamailio-acc
mysqldump kamailio acc | gzip > /var/backups/kamailio-acc/acc-backup-20260809171415.sql.gz   # 66.6MB
mysql -e "TRUNCATE TABLE kamailio.acc;"
```

### Resultado
1,362,137 filas → 0. Tamaño 367.53MB → 0.03MB. Kamailio siguió insertando con normalidad
(1 fila nueva confirmada segundos después). Los CDRs reales de facturación (PostgreSQL,
`apolo_billing.cdrs`) no se tocaron — verificado con conteo antes/después.

Ya existía un cron de purga con retención de 90 días (`purge_kamailio_acc.sh`, corre
diario 3:30am) que evita que esto vuelva a acumularse sin control.

---

## 3. Endpoint 953: `dr_gateways.attrs` incompleto

### Problema
Warnings repetidos en Kamailio:
```
WARNING: invalid signalling configuration requested for endpoint 953
WARNING: invalid media configuration for 953. Defaulting to RTP/AVP.
```
en 11 gateways (`8, 946, 947, 951, 953, 954-959`).

### Causa raíz
`dr_gateways.attrs` para esos gateways solo tenía `gwid,type` (ej. `"953,9"`), sin los campos
`msteams_domain,signalling,media`. El GUI de dSIPRouter (`gui/database/__init__.py`,
`Gateways.attrsToDict()`) ya asume default `'proxy'` cuando faltan esos campos — pero
`kamailio.cfg` (`route[SET_CALLDST_INFO]`) no aplicaba ese mismo default, cayendo en el
`default:` de los `switch` de `SET_DST_SIGNALLING`/`SET_DST_MEDIA` y logueando el warning.
**Impacto funcional: ninguno** — verificado que el fallback ya producía el mismo resultado
que el caso explícito `"proxy"`.

### Fix
En `route[SET_CALLDST_INFO]` (dos ramas: `dr_attrs` AVP y `domain_pbx_list`/SQL), agregado:
```kamailio
if (strempty($var(dst_signalling))) { $var(dst_signalling) = "proxy"; }
if (strempty($var(dst_media))) { $var(dst_media) = "proxy"; }
```
justo después de extraer esos valores, replicando el default de `attrsToDict()`.

### Verificación
264 eventos reales hacia `app.fonowin.pe` (endpoint 953) post-restart, incluyendo llamadas
contestadas con `200 OK`, **0 recurrencias** del warning.

---

## 4. PostgreSQL: housekeeping y particionamiento de `cdrs`

### Problema
- `cdrs` (4.88M filas) **nunca había tenido un VACUUM/ANALYZE** desde su creación
  (`vacuum_count=0, autovacuum_count=0, analyze_count=0`). `pg_stat_user_tables` reportaba
  320K filas en vez de las 4.88M reales (15x de error) → planes de query malos.
- Confirmado con `EXPLAIN`: 42 sequential scans acumulados leyendo 67M tuplas.
- Sin `log_lock_waits`, sin `statement_timeout`, sin `idle_in_transaction_session_timeout`.
- `cdrs` monolítica, sin particionar, creciendo ~43K filas/día.

### Fix

**a) VACUUM ANALYZE:**
```sql
VACUUM (ANALYZE, VERBOSE) cdrs;   -- 1.2s, sin bloqueo (0 dead tuples, insert-only)
```

**b) Config de la base `apolo_billing`:**
```sql
ALTER SYSTEM SET log_lock_waits = on;              -- (como superusuario postgres)
SELECT pg_reload_conf();
ALTER DATABASE apolo_billing SET statement_timeout = '15s';
ALTER DATABASE apolo_billing SET idle_in_transaction_session_timeout = '30s';
```

**c) Particionamiento por mes (migración online, sin downtime del listado, ~15s de pausa
del billing engine para el corte final):**

1. Backup completo: `pg_dump "$DBURL" -t cdrs -Fc -f /var/backups/postgres-cdrs/cdrs-backup-20260809182709.dump` (229MB)
2. Crear `cdrs_new` particionada `PARTITION BY RANGE (start_time)`, con `PRIMARY KEY (id, start_time)`
   y `UNIQUE (call_uuid, start_time)` (Postgres exige que la clave de partición esté en el PK/UNIQUE).
3. Particiones mensuales `cdrs_y2026m04` … `cdrs_y2026m10` + `cdrs_default` (catch-all).
4. Backfill en lotes de 100K filas (script bash + psql, corrido en background).
5. Corte final: detener `apolo-billing-engine`, backfill delta, `ALTER TABLE ... RENAME`
   (swap `cdrs` ↔ `cdrs_new`), reiniciar el servicio.
6. Renombrar constraints/índices para quitar el sufijo `_new` (la tabla vieja se rebautizó
   con sufijo `_bak` para liberar los nombres canónicos).
7. Migración `database/migrations/016_partition_cdrs.sql` agregada al repo — idempotente
   (detecta si `cdrs` ya está particionada y no hace nada; en una instalación nueva crea el
   esquema particionado desde cero).

### Verificación
- Conteo y `MAX(id)` idénticos entre `cdrs` y `cdrs_new` antes del swap (0 pérdida de datos).
- `EXPLAIN`: partition pruning confirmado (`Merge Append` + `Index Scan Backward` para el
  caso común "últimos CDRs sin filtro de fecha", 5.2ms; poda directa a una sola partición
  cuando hay filtro de fecha, 0.26ms).
- Tabla vieja retenida como `cdrs_monolith_backup` (no se borró).

---

## 5. Race condition en reservas de saldo

### Problema
Riesgo de sobregiro: dos llamadas concurrentes en la misma cuenta (`accounts.max_concurrent_calls`
permite varias) podían leer el mismo saldo disponible y ambas ser autorizadas.

### Evidencia (código, antes del fix)
```rust
// create_reservation() — ANTES
let available_balance = self.get_available_balance(account_id).await?;  // SELECT simple, sin lock
if available_balance < total_reservation { /* rechaza */ }
// ... otra conexión, sin transacción que envuelva los pasos anteriores
client.execute("INSERT INTO balance_reservations ...").await?;
```
`get_available_balance()` abría su propia conexión (no una `Transaction`), a diferencia de
`consume_reservation()`/`consume_normal_tx()` que sí usaban `FOR UPDATE` correctamente.

### Fix — `rust-billing-engine/src/services/reservation_manager.rs`

`create_reservation()` y `extend_reservation()` ahora bloquean la fila de `accounts` con
`FOR UPDATE` dentro de una transacción, antes de leer/comparar saldo e insertar la reserva:

```rust
let mut client = self.db_pool.get().await
    .map_err(|e| BillingError::Internal(e.to_string()))?;
let transaction = client.transaction().await
    .map_err(|e| BillingError::Database(e))?;

let account_row = transaction
    .query_one(
        "SELECT balance, COALESCE(credit_limit, 0), account_type
         FROM accounts WHERE id = $1 FOR UPDATE",
        &[&account_id_i32],
    )
    .await
    .map_err(|e| BillingError::Database(e))?;
// ... calcula available_balance dentro de la misma transacción ...

if available_balance < total_reservation {
    transaction.rollback().await.ok();
    return Ok(ReservationResult { success: false, reason: "insufficient_balance".into(), .. });
}
// ... INSERT INTO balance_reservations dentro de la MISMA transacción ...
transaction.commit().await.map_err(|e| BillingError::Database(e))?;
```

`get_available_balance()` (la lectura sin lock, causa raíz) fue **eliminada** — quedaba sin
llamadas después del fix.

### Comandos de build/deploy
```bash
cd /opt/ApoloBilling/rust-billing-engine
cargo build --release          # limpio, sin warnings nuevos
systemctl restart apolo-billing-engine
```

### Verificación
Build limpio, servicio reiniciado sin errores, CDRs en vivo fluyendo con normalidad tras
el restart.

---

## 6. Dirección de CDRs mal clasificada

Este fue el hallazgo más profundo de la sesión — pasó por 3 iteraciones antes del fix final.

### 6.1 Síntoma reportado por el usuario
CDRs de `USER_NOT_REGISTERED` con origen claramente externo (móvil de 9 dígitos) marcados
como **"Saliente"** en vez de "Entrante". Ejemplo:
```
988482386 → 19006016   Saliente   USER_NOT_REGISTERED
```

### 6.2 Iteración 1 — el código original
```rust
// event_handler.rs — ANTES (ambas funciones CHANNEL_CREATE y CHANNEL_HANGUP)
let direction = if context == "from-pbx" || context.starts_with("from-pbx") {
    "outbound".to_string()          // asunción: from-pbx = usuario PBX llamando afuera
} else {
    event.get_header("Call-Direction")
        .or_else(|| event.get_header("Caller-Direction"))
        .map(|s| s.to_lowercase())
        .unwrap_or_else(|| "outbound".to_string())   // default también "outbound"
};
```
Primer fix: agregar caso `from-kamailio → inbound` y cambiar el default a `inbound`. **Insuficiente**,
porque el `context` real de estas llamadas SÍ era `"from-pbx"` (ver 6.3).

### 6.3 Iteración 2 — causa raíz real (con evidencia del log)
```
📞 Context: from-pbx → Direction: outbound
```
`context` (`Caller-Context`/`variable_context`) describe qué **perfil de FreeSWITCH está
entregando** la llamada en ese momento, no quién la originó. Al intentar timbrar una
extensión local no registrada, el intento corre en el perfil interno y reporta `from-pbx`
sin importar si el origen fue externo.

Fix quirúrgico aplicado (en el hangup handler): `hangup_cause == "USER_NOT_REGISTERED"`
fuerza `inbound` — estructuralmente esa causa **solo le puede pasar al destino**, nunca a
quien origina la llamada.

### 6.4 Iteración 3 — fix definitivo (headers nativos de FreeSWITCH)
Volcado completo de headers de un evento real (`fs_cli`/ESL) reveló que FreeSWITCH **ya
reporta la dirección correcta** en 5 headers distintos, que el código nunca leía:
```json
"Call-Direction": "inbound",
"Caller-Direction": "inbound",
"Caller-Logical-Direction": "inbound",
"variable_direction": "inbound",
"Presence-Call-Direction": "inbound",
"Caller-Context": "from-pbx"   // <- esto era lo único que el código miraba
```

**Código final** (idéntico en `handle_channel_create` y `handle_channel_hangup`):
```rust
let context = event.get_header("Caller-Context")
    .or_else(|| event.get_header("variable_context"))
    .map(|s| s.as_str())
    .unwrap_or("unknown");

let native_direction = event.get_header("Call-Direction")
    .or_else(|| event.get_header("Caller-Direction"))
    .or_else(|| event.get_header("Caller-Logical-Direction"))
    .or_else(|| event.get_header("variable_direction"))
    .map(|s| s.to_lowercase())
    .filter(|d| d == "inbound" || d == "outbound"); // descarta "leg_b" u otros valores

let direction = native_direction.unwrap_or_else(|| {
    if context == "from-pbx" || context.starts_with("from-pbx") {
        "outbound".to_string()
    } else if context == "from-kamailio" || context.starts_with("from-kamailio") {
        "inbound".to_string()
    } else {
        "inbound".to_string()  // tráfico predominante de esta plataforma es entrante
    }
});
```
En `handle_channel_hangup` además se mantiene, como red de seguridad adicional:
```rust
let direction = if hangup_cause == "USER_NOT_REGISTERED" {
    "inbound".to_string()
} else {
    native_direction.unwrap_or_else(|| { /* mismo fallback de arriba */ })
};
```

### 6.5 Corrección de histórico

**Paso 1 — regresión de `USER_NOT_REGISTERED` (1.46M filas):**
Se encontró el quiebre exacto **2026-06-26 19:00 hora Lima**: antes 100% correctamente
`inbound`, después ~100% mal etiquetado `outbound` (validado por hora, no por semana).
```sql
CREATE TABLE direction_fix_audit_20260809 (
    cdr_id bigint, start_time timestamptz, old_direction text, new_direction text,
    fixed_at timestamptz DEFAULT now()
);
INSERT INTO direction_fix_audit_20260809 (cdr_id, start_time, old_direction, new_direction)
SELECT id, start_time, direction, 'inbound' FROM cdrs
WHERE hangup_cause = 'USER_NOT_REGISTERED' AND direction = 'outbound'
  AND start_time >= '2026-06-26 19:00:00-05';   -- 1,460,562 filas

UPDATE cdrs c SET direction = 'inbound'
FROM direction_fix_audit_20260809 a
WHERE c.id = a.cdr_id AND c.start_time = a.start_time;
```
Corrido en lotes de 50,000 por `statement_timeout=15s` (ver §4b) — script bash con
`SET statement_timeout = '60s'` por lote.

**Paso 2 — residual tras aplicar el fix de código (362 filas)**, la ventana entre los dos
restarts del billing engine mientras se afinaba el fix — mismo patrón de auditoría.

**Validación cruzada (no solo intuición):** se cruzó contra `kamailio.acc.calltype`
(clasificación propia e independiente de Kamailio) — 468/470 pares emparejados coincidían
100% en `gwgroupid=35 (INTEGRATEL_SC_LAVICTORIA)` + `calltype=inbound` + `direction=inbound`.

**Resultado final:** `USER_NOT_REGISTERED` = 3,283,985 inbound / **0** outbound en todo
el histórico.

### 6.6 Lo que se investigó y se descartó (por falta de evidencia suficiente)
- `NORMAL_CLEARING`: mismo patrón de "ventana anómala" pero **polaridad invertida** — se
  descartó tras confirmar que el patrón de número que parecía "entrante" (`1`+7 dígitos) es
  en realidad formato válido de PSTN de Lima para tráfico **saliente** legítimo (gateways
  "Legado", `dr_rules` con prefijos 2-7/04-08/105).
- `USER_BUSY`, `NO_ROUTE_DESTINATION`: mismo patrón de ventana anómala, pero sin cobertura
  de validación cruzada suficiente contra `kamailio.acc` (solo 4 pares emparejados). **No
  corregidos** — quedan como hallazgo documentado, no como fix aplicado.
- `NORMAL_UNSPECIFIED`: picos de un solo día (ráfaga), no un patrón sostenido — no se
  investigó a fondo el origen.

---

## 7. Reloj de FreeSWITCH desincronizado (~5h)

### Síntoma reportado por el usuario
"En el frontend los registros de CDRs se ven con la hora en GMT."

### Evidencia (comparación dentro de la MISMA fila, sin cruces ambiguos)
```sql
SELECT call_uuid, start_time, created_at, created_at - start_time AS diferencia
FROM cdrs ORDER BY id DESC LIMIT 1;
--  start_time = 2026-08-10 01:55:58-05
--  created_at = 2026-08-09 20:49:32-05   (correcto, hora real del sistema)
--  diferencia = -05:06:25   (constante en 15 filas consecutivas)
```
`fs_cli -x 'eval ${strftime(%Y-%m-%d %H:%M:%S)}'` respondió **"2026-08-10 01:55:00"**
cuando la hora real de Lima era **20:48 PM** — FreeSWITCH mismo tenía el reloj mal.

### Causa raíz
```
Boot de la VM:               2026-07-31 11:50:28
systemd-timesyncd arrancó:   2026-07-31 16:56:57   ← 5h06m29s después
FreeSWITCH arrancó:          2026-07-31 16:57:21   ← 24s después de timesyncd
```
FreeSWITCH arrancó con el reloj del sistema aún desincronizado (NTP tardó 5h+ en converger
tras el boot de la VM KVM, típico de `kvm-clock` tras pausa/snapshot). Al no recalibrar
mientras el proceso vive, quedó con esa referencia fija durante los 9+ días de uptime.

### Fix 1 — corregir el reloj ahora
```bash
fs_cli -x "show channels count"   # 2 canales activos, impacto mínimo
systemctl restart freeswitch
fs_cli -x 'eval ${strftime(%Y-%m-%d %H:%M:%S)}'   # ahora coincide exacto con `date`
```

### Fix 2 — prevención (no requiere tocar el proceso en vivo)
```bash
systemctl enable systemd-time-wait-sync.service
mkdir -p /etc/systemd/system/freeswitch.service.d
cat > /etc/systemd/system/freeswitch.service.d/wait-for-time-sync.conf << 'EOF'
[Unit]
After=systemd-time-wait-sync.service
Wants=systemd-time-wait-sync.service
EOF
systemctl daemon-reload
```
FreeSWITCH ahora espera a que el reloj esté confirmado sincronizado antes de arrancar, en
cualquier reinicio/boot futuro.

### Fix 3 — detección (por si vuelve a pasar)
Script `/usr/local/bin/check-freeswitch-clock-drift.sh` — compara `fs_cli` vs `date`,
umbral 5 min, loguea a journald (`systemd-cat -t freeswitch-clock-check`, nivel `err` si
excede umbral). Corre cada 15 min vía `freeswitch-clock-check.timer`:
```ini
# /etc/systemd/system/freeswitch-clock-check.timer
[Timer]
OnBootSec=5min
OnUnitActiveSec=15min
AccuracySec=1min
```
```bash
systemctl enable --now freeswitch-clock-check.timer
journalctl -t freeswitch-clock-check   # para revisar el historial de chequeos
```

**Nota:** estos artefactos de FreeSWITCH/systemd viven en `/etc` y `/usr/local/bin`, fuera
de git — consistente con cómo ya se gestiona el resto de la config de sistema en este
proyecto (`kamailio.cfg`, dialplan de FreeSWITCH tampoco están en git en el servidor real).

---

## 8. Corrección masiva de `start_time`/`end_time`

### Alcance real (mucho mayor de lo que parecía al principio)
El bug del §7 no era solo de esa sesión — llevaba desde el **17 de mayo**, en dos episodios
ligados a reinicios previos de FreeSWITCH que perdieron la misma carrera contra NTP:

| Ventana | Filas afectadas | Desfase |
|---|---|---|
| 2026-05-17 → 2026-07-13 | ~2.4M | -307 min (~5h07m), constante |
| 2026-08-05 → 2026-08-09 | ~400K | -306 min (~5h06m), constante |
| **Total** | **2,954,245** | — |

### Método de detección
`created_at` (reloj del sistema/app, siempre confiable) vs `end_time` (epoch de FreeSWITCH):
```sql
SELECT COUNT(*) FROM cdrs WHERE ABS(EXTRACT(EPOCH FROM (created_at - end_time))) > 60;
```

### Fix — anclado en `created_at`, preservando la duración exacta
```sql
CREATE TABLE timestamp_fix_audit_20260809 (
    cdr_id bigint, old_start_time timestamptz, old_end_time timestamptz,
    new_start_time timestamptz, new_end_time timestamptz, fixed_at timestamptz DEFAULT now()
);
INSERT INTO timestamp_fix_audit_20260809 (cdr_id, old_start_time, old_end_time, new_start_time, new_end_time)
SELECT id, start_time, end_time, created_at - (end_time - start_time), created_at
FROM cdrs WHERE ABS(EXTRACT(EPOCH FROM (created_at - end_time))) > 60;

-- UPDATE en lotes de 50,000, statement_timeout=90s por lote (script bash en background)
UPDATE cdrs c
SET start_time = a.new_start_time, end_time = a.new_end_time
FROM timestamp_fix_audit_20260809 a
WHERE c.id = a.cdr_id AND c.start_time = a.old_start_time
  AND a.cdr_id > :last AND a.cdr_id <= :next;
```

### Error propio detectado y corregido durante la verificación
La primera corrección manual (52 filas de la ventana del restart, antes de escalar al fix
general) usó la fórmula al revés para las pocas filas con duración real (`new_start_time =
created_at` en vez de `new_end_time = created_at`), empujando `end_time` varios minutos
**después** de `created_at`. Se detectó al verificar (3 filas residuales tras la corrección
masiva) y se corrigió puntualmente con la fórmula correcta.

### Verificación final
```sql
SELECT COUNT(*) FROM cdrs WHERE ABS(EXTRACT(EPOCH FROM (created_at - end_time))) > 60;
-- 0
```
0 errores en el log del batch, todos los servicios (`apolo-billing-engine`, `freeswitch`,
`kamailio`, `mariadb`, `postgresql`) activos y sanos durante y después de la corrección.

---

## 9. Dashboard de CDRs sin auto-refresh

### Problema
El listado de CDRs no mostraba llamadas nuevas sin recargar la página manualmente.

### Causa raíz
`frontend/src/pages/CDR.tsx` usaba `useQuery` de React Query **sin `refetchInterval`** —
fetch único al entrar a la página. `ActiveCalls.tsx`, en cambio, sí tenía
`refetchInterval: 3000` + WebSocket.

### Fix
```diff
  const { data, isLoading } = useQuery({
    queryKey: ['cdrs', page, filters],
    queryFn: () => fetchCDRs(filters, page, 50),
+   // Refresca automáticamente para mostrar CDRs nuevos sin recargar la página
+   // (mismo patrón que ActiveCalls.tsx).
+   refetchInterval: 3000,
  })
```

### Deploy
```bash
cd /opt/ApoloBilling/frontend
npm run build
systemctl restart apolo-frontend
```

### Verificación
Confirmado en el bundle servido (`refetchInterval:3e3` — 2 ocurrencias, la de `ActiveCalls`
y la nueva de `CDR`), y confirmado visualmente por el usuario en el dashboard en vivo.

---

## 10. Branding del login

`frontend/src/pages/Login.tsx`:
```diff
- v1.0.0 • Powered by Fibertel
+ v1.0.0 • Powered by Apolo AI
```
Build + `systemctl restart apolo-frontend`, verificado en el bundle servido.

---

## 11. Commits de git

Repo `/opt/ApoloBilling`, rama `fix/cdrs-partitioning-and-reservation-race` (creada desde
`main`, sin push a `origin`):

```
abe16e42  fix: dirección de CDRs usa headers nativos de FreeSWITCH + branding login
fad289af  fix: race condition en reservas de saldo + particionar cdrs por mes
```

**Deliberadamente fuera de los commits** (no son cambios de esta sesión / no versionados
en este repo):
- `rust-billing-engine/.env`, `rust-backend/.env` — contienen secretos reales, su drift es
  previo a esta sesión (ver hallazgo de seguridad en la revisión de codebase de esta misma
  fecha: estos archivos están trackeados en git pese a `.gitignore`).
- WIP preexistente sin relación (handlers, migraciones 010-015, frontend) que ya estaba
  sin commitear antes de esta sesión.
- Config de Kamailio/FreeSWITCH/systemd (§1, §3, §7) — vive en `/etc`, no es un repo git.

---

## 12. Tablas de auditoría / cómo revertir

Todas las correcciones masivas de datos quedaron con auditoría reversible en PostgreSQL
(`apolo_billing`):

| Tabla | Qué guarda | Filas |
|---|---|---|
| `direction_fix_audit_20260809` | `direction` anterior por CDR (§6.5) | 1,460,924 |
| `timestamp_fix_audit_20260809` | `start_time`/`end_time` anteriores por CDR (§8) | 2,954,245 |

Para revertir una corrección puntual:
```sql
UPDATE cdrs c SET direction = a.old_direction
FROM direction_fix_audit_20260809 a WHERE c.id = a.cdr_id;

UPDATE cdrs c SET start_time = a.old_start_time, end_time = a.old_end_time
FROM timestamp_fix_audit_20260809 a WHERE c.id = a.cdr_id;
```

Backups adicionales en disco:
- `/var/backups/kamailio-acc/acc-backup-20260809171415.sql.gz` (MariaDB `kamailio.acc`, 66.6MB)
- `/var/backups/postgres-cdrs/cdrs-backup-20260809182709.dump` (Postgres `cdrs` pre-partición, 229MB)
- `cdrs_monolith_backup` (tabla completa, dentro de la misma DB `apolo_billing`)
- `/etc/kamailio/kamailio.cfg.bak-*` (varios, cronológicos, en el mismo servidor)

---

*Documento generado el 2026-08-09 como cierre de la sesión de soporte. Autor: Claude
(Anthropic), en conjunto con el equipo de ApoloBilling.*
