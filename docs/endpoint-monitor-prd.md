# PRD — Monitoreo de Conectividad SIP por Endpoint (port a otro Kamailio)

**Documento:** especificación técnica de implementación
**Origen de verdad:** `/opt/dsiprouter` (SBC `sbc01`), módulo `endpoint-monitor` +
página `/apolo-monitor`, en producción.
**Destino:** un segundo servidor Kamailio **con GUI propia/distinta** (no la GUI
de dsiprouter).
**Alcance:** stack completo — colector (servicio de sondeo) + tabla + API REST +
página web.
**Fecha:** 2026-08-16

---

## 1. Objetivo y contexto

### 1.1 Problema
Los PBX Asterisk de los clientes hacen `qualify` (SIP `OPTIONS`) contra el SBC
como keepalive del trunk. Cuando un microcorte de red tumba ese keepalive, el
cliente ve el trunk **UNREACHABLE** y culpa al SBC — sin evidencia para
distinguir "cayó el SBC" de "se perdió la red del lado del cliente".

### 1.2 Solución
Un servicio que reproduce **la misma prueba en sentido inverso** (SBC →
endpoint, `OPTIONS` vía `sipsak`, 1/min a *cada* gateway), guarda el resultado
con historial, y lo expone en una página con tablero en vivo, gráfica de RTT,
tabla de sondas y **evidencia descargable** (traza cruda de la sonda) para
defender un reclamo.

### 1.3 Qué NO es
- No es un reemplazo del `dispatcher` de Kamailio (que ya hace probing para
  enrutar). Es una **capa de observabilidad/histórico** que *se cruza* con el
  dispatcher para mayor exactitud.
- No mide calidad de audio (jitter/MOS) — eso vive en el pipeline de RTP, otro
  módulo.

---

## 2. Arquitectura de referencia (implementación actual)

Cuatro componentes desacoplados. La única superficie compartida entre el colector
y la GUI es **la tabla** — no hay acoplamiento de proceso.

```
  ┌─────────────────────┐   sipsak OPTIONS 1/min      ┌──────────────┐
  │ endpoint-monitor.svc │ ──────────────────────────▶ │  cada gateway │
  │  (monitor.py, Python)│ ◀──── respuesta SIP ─────── │  (dr_gateways)│
  │                      │                             └──────────────┘
  │  + cruce autoritativo│  kamcmd dispatcher.list
  │    con el dispatcher │ ◀───────────────────────────  Kamailio RPC
  └──────────┬───────────┘
             │ INSERT (cliente mysql, socket local)
             ▼
     ┌───────────────────────┐        SELECT         ┌──────────────────┐
     │ dsip_endpoint_monitor │ ◀──────────────────── │  API REST (4 rutas)│
     │  (MariaDB, retención   │                       │  /monitor/*        │
     │   7 días)             │                       └─────────┬────────┘
     └───────────────────────┘                                 │ JSON
                                                               ▼
                                                     ┌──────────────────┐
                                                     │  Página web       │
                                                     │  tablero+gráfica+ │
                                                     │  tabla+evidencia  │
                                                     └──────────────────┘
```

| # | Componente | Archivo de referencia | Lenguaje/stack |
|---|---|---|---|
| C1 | Esquema de datos | `tools/endpoint-monitor/schema.sql` | SQL (MariaDB/InnoDB) |
| C2 | Colector (servicio) | `tools/endpoint-monitor/monitor.py` + `.service` | Python 3 + systemd |
| C3 | API REST | `gui/modules/local_api/routes.py` (marker `dashboard_endpoint_monitor`, 4 rutas) | Flask (a reescribir en tu GUI) |
| C4 | Página web | `gui/templates/apolo_monitor.html` | HTML + Chart.js (a reescribir en tu GUI) |

---

## 3. Dependencias y supuestos del entorno destino

| Requisito | Detalle | Bloqueante |
|---|---|---|
| Kamailio con `dispatcher` activo | Para el cruce autoritativo; `kamcmd dispatcher.list` debe responder. Sin él, el colector sigue funcionando pero pierde la corrección del falso-`down`. | No (degradable) |
| MariaDB/MySQL local | El colector escribe por **socket local** (`mysql <db>`); la API lee. DB por defecto `kamailio`. | Sí |
| `sipsak` instalado | La sonda. `apt install sipsak`. | Sí |
| `python3` (stdlib solamente) | `monitor.py` no usa librerías externas: `argparse`, `re`, `subprocess`, `concurrent.futures`. | Sí |
| `ip` (iproute2) | Para `ip route get <dst>` → IP de egress por destino. | Sí |
| **Fuente de endpoints** | El colector necesita una lista `(gwid, gwgroup, name, ip, port)`. **Aquí está el mayor punto de adaptación** — ver §4.1. | Sí |
| GUI propia con backend | Tu capa web debe poder ejecutar SQL de solo-lectura contra `dsip_endpoint_monitor` y servir JSON + archivos `.txt`. | Sí |

---

## 4. Portabilidad: qué cambia respecto al original

El colector y la tabla son casi 1:1. **La API y la página se reescriben** contra
tu GUI (por eso este PRD las especifica como *contratos*, no como código a copiar).

### 4.1 ⚠️ Punto crítico: la fuente de endpoints

El `monitor.py` original obtiene los endpoints parseando la columna
`dr_gateways.description` con el **formato propio de dsiprouter**:

```
description = "name:CLIENTE_X,gwgroup:11,addr_id:42"
```

En un Kamailio con otra GUI, `dr_gateways.description` **puede tener otro
formato, estar vacía, o no usarse `drouting` en absoluto**. Debes elegir una de
estas estrategias de origen y adaptar `load_endpoints()`:

| Opción | Fuente | Cuándo | Ajuste |
|---|---|---|---|
| **A. `dr_gateways` (drouting)** | `SELECT gwid, address, description` | Si tu GUI usa `drouting` con description parseable | Ajustar el parseo de `name`/`gwgroup` al formato real |
| **B. `dispatcher`** | `SELECT id, setid, destination, description FROM dispatcher` | Si el enrutamiento es por `dispatcher` (lo más común en Kamailio puro) | `destination` = `sip:IP:PORT`; `setid` cumple el rol de `gwgroup`; `name` del `description` o `attrs` |
| **C. Tabla/vista propia** | Una vista que tu GUI ya tenga de "endpoints/trunks" | Si tu modelo de datos es distinto | Mapear columnas a `(gwid, gwgroup, name, ip, port)` |
| **D. Config estática** | Un `endpoints.yaml`/`.conf` | Pocos endpoints fijos, sin BD de routing | Leer archivo en vez de SQL |

> **Regla de contrato:** `load_endpoints()` debe devolver una lista de tuplas
> `(gwid:int, gwgroup:int, name:str, ip:str, port:int)`. Todo lo demás del
> colector permanece igual. Aísla el cambio a esa única función.

### 4.2 Cruce con el dispatcher
`dispatcher_status()` parsea `kamcmd dispatcher.list` y devuelve `{ip: (activo,
est_ms)}`. Depende de que exista el binario `kamcmd` y de que el dispatcher esté
poblado. Si el destino **no** usa dispatcher, deja que devuelva `{}` — el colector
degrada a "solo sipsak" sin romperse. La lógica de corrección del falso-`down`
(§5.2.4) simplemente no se activará.

### 4.3 Acceso a BD
- **Colector:** cliente `mysql` por socket local (no TCP, no password en línea de
  comando). Si tu MariaDB exige credenciales, usa un `~/.my.cnf` del usuario del
  servicio o un `--defaults-file`.
- **API:** tu GUI ya tiene su propia capa de acceso; solo necesita `SELECT` sobre
  `dsip_endpoint_monitor`. Concede el `GRANT SELECT` correspondiente.

### 4.4 Autenticación de la API
El original usa `@token_required` (sesión-cookie de la GUI dsiprouter). En tu GUI,
protege las 4 rutas con **tu propio middleware de sesión/auth**. No expongas
`/monitor/*` sin autenticación: la evidencia contiene IPs y trazas SIP internas.

---

## 5. Especificación por componente

### 5.1 C1 — Esquema de datos

Tabla **aditiva** (no toca ninguna tabla upstream de Kamailio). Reversible con
`DROP TABLE`.

```sql
CREATE TABLE IF NOT EXISTS dsip_endpoint_monitor (
    id          BIGINT UNSIGNED NOT NULL AUTO_INCREMENT,
    ts          INT UNSIGNED    NOT NULL,          -- epoch UTC de la sonda
    gwid        INT             NOT NULL,          -- id del gateway origen
    gwgroupid   INT             NOT NULL DEFAULT 0,-- grupo/setid
    name        VARCHAR(128)    NOT NULL DEFAULT '',
    ip          VARCHAR(64)     NOT NULL DEFAULT '',
    port        SMALLINT UNSIGNED NOT NULL DEFAULT 5060,
    status      ENUM('up','down') NOT NULL DEFAULT 'down',
    resp_code   SMALLINT UNSIGNED NOT NULL DEFAULT 0,  -- código SIP final (200/404/503…) o 0
    rtt_ms      DECIMAL(8,3)    NULL,                  -- NULL si down
    evidence    MEDIUMTEXT      NULL,                  -- traza cruda sipsak (off-page)
    PRIMARY KEY (id),
    KEY idx_ts        (ts),
    KEY idx_gw_ts     (gwid, ts),
    KEY idx_ip_ts     (ip, ts)
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb4;
```

**Decisiones de diseño:**
- `evidence` es `MEDIUMTEXT` (off-page en InnoDB): las consultas del dashboard
  **no la SELECTean** (solo el endpoint de descarga), así no infla el I/O de las
  gráficas.
- `status='up'` = se recibió **cualquier** respuesta SIP (2xx–6xx). Un 404/403
  prueba conectividad. Mismo criterio que `ds_ping_reply_codes` del dispatcher.
- Índices por `ts`, `(gwid,ts)`, `(ip,ts)` cubren las tres consultas: último
  ciclo, serie por endpoint, y filtro por IP+rango.
- **Retención 7 días** por purga dentro del servicio (no evento externo).
  Volumen: `N_endpoints × 1440 sondas/día × 7`. Con evidencia ~2–8 KB/fila,
  dimensiona el disco: p.ej. 100 endpoints ≈ 1M filas ≈ 2–8 GB con evidencia.
  Si es mucho, ver §8 (opción de truncar `evidence` solo para `down`).

### 5.2 C2 — Colector (servicio de sondeo)

Ejecutable Python 3, un proceso, loop infinito con `sleep` alineado a `--interval`.

#### 5.2.1 Parámetros (CLI)
```
--interval        segundos entre ciclos           (default 60)
--timeout         segundos por sonda sipsak        (default 2)
--workers         sondas en paralelo (pool)        (default 8)
--retention-days  días de historial                (default 7)
--once            un ciclo y salir (para test)
```

#### 5.2.2 Ciclo (`run_cycle`)
1. `load_endpoints()` → lista `(gwid, gwgroup, name, ip, port)` (§4.1).
2. `ts = epoch actual`.
3. `dispatcher_status()` → `{ip: (activo, est_ms)}` (una sola llamada `kamcmd`).
4. Sondear **en paralelo** (`ThreadPoolExecutor(max_workers=workers)`): un ciclo
   con endpoints caídos dura `~timeout`, no `N×timeout`.
5. Por cada resultado, aplicar el **cruce con el dispatcher** (§5.2.4).
6. `INSERT` masivo de todas las filas del ciclo en una sentencia, **por stdin**
   (no `-e`): las trazas son multilínea y de varios KB → evita `ARG_MAX` y
   problemas de escaping de shell.
7. Cada ~1 h (`3600 // interval` ciclos), purgar `ts < now - retention*86400`.

#### 5.2.3 La sonda (`probe`) — gotchas de `sipsak` (críticos)
```
sipsak -vv -s sip:<IP>:<PORT> -H <src_ip>
```
- **`sipsak` devuelve `rc=1` aun en éxito** y **sin `-vv` no imprime nada**. El
  éxito se detecta **parseando la salida**, NUNCA por exit code:
  - `SIP/2.0 (\d{3})` → código de respuesta.
  - `reply received after ([\d.]+) ms` → RTT.
  - Si aparece cualquiera de los dos ⇒ `up`. Si no ⇒ `down`.
- **`-H <src>` es obligatorio**: sin él, sipsak falla resolviendo su propio
  hostname. `src` = IP de egress real hacia ese destino, obtenida con
  `ip route get <IP>` (regex `\bsrc\s+([0-9.]+)`) y **cacheada por destino**.
- La sonda sale del **puerto efímero** de sipsak, no de `:5060` → algunos
  endpoints que solo aceptan tráfico desde `:5060` darían falso `down`. Eso lo
  corrige el cruce con el dispatcher (§5.2.4).

#### 5.2.4 Cruce autoritativo con el dispatcher
El dispatcher sondea desde `:5060` (la MISMA ruta que el `qualify` del cliente),
así que es autoritativo sobre "¿el endpoint responde a *nosotros*?".

```
si probe==down  Y  dispatcher[ip].activo == True:
    status = "up"                       # corregir falso-down
    si rtt is None: rtt = dispatcher.est_ms   # RTT de respaldo = latencia estimada
    prepend nota explicativa a la evidencia
```

`FLAGS[0]` del dispatcher: `A`=activo, `I`=inactivo(caído), `T`=probando,
`D`=deshabilitado. Solo `A` cuenta como activo. (Ver memoria
`reference_kamailio_dispatcher_flags` para el matiz de `AX`.)

#### 5.2.5 Evidencia
Cada fila guarda una traza legible (cap **8192 bytes**): cabecera con origen,
destino, UTC, status/code/rtt, el comando exacto, y la salida cruda de sipsak (o
"sin salida — timeout"). Es el documento defendible ante un reclamo.

#### 5.2.6 Unit systemd (referencia)
```ini
[Unit]
Description=SIP endpoint connectivity monitor -> dsip_endpoint_monitor
After=network.target mariadb.service
Wants=mariadb.service

[Service]
Type=simple
User=root
ExecStart=/usr/bin/python3 /opt/<ruta>/monitor.py --interval 60 --timeout 2 --retention-days 7
Restart=on-failure
RestartSec=5
Nice=10
IOSchedulingClass=idle        # el sondeo no compite con el plano de llamadas
NoNewPrivileges=true

[Install]
WantedBy=multi-user.target
```
> `User=root` es lo mínimo si `kamcmd`/socket mysql lo requieren en tu caja. Si
> puedes, crea un usuario de servicio con acceso al socket mysql y a `kamcmd`
> (grupo `kamailio`) y baja privilegios.

### 5.3 C3 — Contrato de la API REST

Cuatro rutas de **solo lectura** bajo un prefijo (original:
`/api/local/v1/monitor`). Reimpleméntalas en tu GUI con tu auth. Envoltura JSON:
el original usa `{"data": {...}}` (payload directo bajo `data`). Adáptalo a la
convención de tu GUI, pero **mantén los nombres de campo** para poder reusar el
frontend.

#### 5.3.1 `GET /monitor/endpoints`
Estado del **último ciclo** — alimenta el dropdown y el tablero de tarjetas.
```
Query: (ninguno)
Resp:  data.endpoints[] = {
         name, ip, gwgroupid, port,
         status ('up'|'down'), resp_code, rtt_ms|null, last_ts
       }
       data.generated_at (epoch)
```
SQL núcleo:
```sql
SELECT name, ip, gwgroupid, port, status, resp_code, rtt_ms, ts
FROM dsip_endpoint_monitor
WHERE ts = (SELECT MAX(ts) FROM dsip_endpoint_monitor)
ORDER BY status ASC, name ASC;
```

#### 5.3.2 `GET /monitor/results`
Series para la gráfica (**downsampleada**) + filas crudas para la tabla.
```
Query: name, ip   (subcadena, LIKE en cada campo por separado)
       frm, to    (epoch s; default: últimas 3 h)
       limit      (filas de la tabla; clamp 1..5000, default 500)
Resp:  data.frm, data.to, data.bucket_seconds, data.generated_at
       data.series[] = { name, ip, points[]={ t, rtt|null, status, downs, n } }
       data.rows[]   = { id, ts, name, ip, gwgroupid, port, status, resp_code, rtt_ms|null }
```
**Downsampling** (clave para que 7 días no devuelvan 10k puntos/endpoint):
```
bucket = max(60, ((span/360)/60 + 1)*60)   # ~≤360 puntos, múltiplo de 60 s
```
SQL de la serie (bucket AVG; el bucket cuenta como `down` si *alguna* sonda falló):
```sql
SELECT name, ip, FLOOR(ts/:bucket)*:bucket AS tb,
       ROUND(AVG(rtt_ms),3) AS rtt, MIN(status) AS status,
       SUM(status='down') AS downs, COUNT(*) AS n
FROM dsip_endpoint_monitor
WHERE ts>=:frm AND ts<=:to [AND name LIKE :name] [AND ip LIKE :ip]
GROUP BY name, ip, tb ORDER BY name, ip, tb;
```
> `MIN(status)` sobre el ENUM devuelve `'down'` si cualquiera del bucket fue down
> (`'down' < 'up'` alfabéticamente) — de ahí el punto rojo en la gráfica.

#### 5.3.3 `GET /monitor/evidence`
Descarga la traza sipsak de **una** sonda como `.txt`.
```
Query: id  (dsip_endpoint_monitor.id)
Resp:  text/plain; Content-Disposition: attachment; filename="evidencia_<slug>_<fecha>.txt"
```

#### 5.3.4 `GET /monitor/evidence_report`
**Reporte consolidado** (.txt) por endpoint + rango — el documento entregable
ante un reclamo.
```
Query: name, ip (LIKE), frm, to (epoch; default 24 h), full (1=incluye trazas crudas)
Resp:  text/plain adjunto. Cabecera con host, rango (local+UTC), totales
       (up/down, %disponibilidad, RTT min/avg/max) y, si full=1, las trazas.
```

**Seguridad de las 4 rutas:** usa consultas **parametrizadas** (el original arma
el `WHERE` con binds `:name/:ip/:frm/:to`, nunca concatena valores). Protégelas
con tu auth de sesión.

### 5.4 C4 — Página web

Página autocontenida (una sola vista) con 4 zonas. Consume `/endpoints` y
`/results`; auto-refresco cada **30 s** solo en modo "live" (rango relativo, no
personalizado).

**Zonas:**
1. **Filtros:** dropdown de endpoint (`name·ip`), texto libre (nombre/IP; si
   parece IP filtra por `ip`, si no por `name`), botones de rango
   `1h/6h/24h/7d/Personalizado` (con `datetime-local` desde/hasta), botón
   Aplicar y botón **⤓ Descargar evidencia** (llama a `/evidence_report` con el
   filtro actual).
2. **Tablero** (grid de tarjetas, una por endpoint del último ciclo): nombre, IP
   `· grupo N`, estado (Activo/Caído con borde verde/rojo), RTT en ms.
   Click en tarjeta = seleccionar/deseleccionar ese endpoint como filtro.
3. **Gráfica de RTT** (línea, Chart.js): un dataset por endpoint; eje X sobre
   **todo** el rango `[frm,to]` (no solo donde hay datos) usando `bucket_seconds`;
   los puntos donde `downs>0` se pintan **rojos y más grandes**; `spanGaps:false`
   (hueco = tramo caído); tooltip "— (caído)" cuando `rtt==null`.
4. **Tabla** "Sondas recientes": Hora, Endpoint, IP, Grupo, Estado (badge),
   Código, RTT (coloreado: >150 ms ámbar), y **⤓ traza** → `/evidence?id=`.

**Indicador live:** un "latido" (dot animado) activo solo cuando el rango es
relativo; "Actualizado HH:MM:SS" tras cada carga.

**Dependencia de front:** solo Chart.js (self-hosted/vendored). Sin CDN externo.

**Reescritura en tu GUI:** el HTML/CSS/JS del original es un `<div class="mon-scope">`
con tema oscuro embebido y ~200 líneas de JS vanilla. Puedes (a) portarlo casi
tal cual dentro de tu layout, o (b) reimplementar las 4 zonas en tu framework
consumiendo el mismo contrato JSON. El contrato de datos es lo que debe
preservarse; el look es libre.

---

## 6. Plan de implementación (fases)

| Fase | Entregable | Verificación |
|---|---|---|
| **0. Prep** | `sipsak`, `python3`, `ip` presentes; `kamcmd dispatcher.list` responde; DB accesible por socket | `sipsak -V`; `kamcmd dispatcher.list \| head` |
| **1. Datos** | `schema.sql` aplicado; tabla `dsip_endpoint_monitor` creada | `SHOW CREATE TABLE dsip_endpoint_monitor` |
| **2. Origen de endpoints** | `load_endpoints()` adaptado a tu fuente (§4.1) | `python3 monitor.py --once` imprime "ciclo ok: N endpoints" |
| **3. Colector** | Servicio corriendo `--interval 60`; cruce con dispatcher OK | `journalctl -u <svc> -f`; filas nuevas cada minuto |
| **4. API** | 4 rutas en tu GUI, con auth y consultas parametrizadas | `curl` autenticado a `/monitor/endpoints` devuelve el último ciclo |
| **5. Página** | Vista con tablero+gráfica+tabla+evidencia | Tablero pinta N tarjetas; gráfica con puntos rojos en caídas; descarga de traza funciona |
| **6. Retención** | Purga a 7 días verificada | Tras >1 h, no hay filas con `ts < now-7d` |

**Recomendación:** aísla el servicio en su propio directorio (`tools/endpoint-monitor/`),
no lo mezcles con el árbol servido por la GUI, y si tu despliegue usa un script
idempotente de patches, envuélvelo en un `apply_*()` con su propio marker (misma
convención que el original).

---

## 7. Criterios de aceptación

1. Cada endpoint de la fuente recibe una sonda por minuto; una fila por sonda.
2. Un endpoint que solo responde a `:5060` se registra `up` (corrección por
   dispatcher), no `down`.
3. Un endpoint realmente inalcanzable se registra `down` con evidencia de timeout.
4. La gráfica de 7 días responde en <2 s (gracias al bucketing) y marca las
   caídas en rojo.
5. `/evidence?id=` descarga la traza exacta de esa sonda.
6. `/evidence_report` produce un `.txt` con totales de disponibilidad y (opcional)
   trazas — presentable a un cliente.
7. La purga mantiene la tabla acotada a 7 días.
8. Un fallo del colector (o de `kamcmd`) **no** tumba la GUI ni Kamailio; el
   servicio reintenta (`Restart=on-failure`).

---

## 8. Riesgos y decisiones abiertas

| Riesgo / decisión | Nota |
|---|---|
| **Formato de la fuente de endpoints** (§4.1) | El único cambio de código real. Confírmalo antes de la Fase 2. |
| **Volumen de `evidence`** | Con muchos endpoints, la evidencia domina el tamaño. Opción: guardar `evidence` solo cuando `status='down'` (los `up` casi nunca se auditan). Reduce el disco ~10×. |
| **Sin dispatcher en el destino** | El cruce se desactiva; sube la tasa de falsos-`down` para endpoints que filtran por puerto de origen. Mitigación: sondear desde `:5060` con opciones de sipsak, o aceptar el ruido. |
| **`User=root` en el servicio** | Reducir a usuario de servicio con acceso a socket mysql + grupo `kamailio` si la política lo exige. |
| **Carga de sondeo** | `workers=8`, `Nice=10`, `IOSchedulingClass=idle` → impacto despreciable en el plano de llamadas. Subir workers solo si `N_endpoints` es grande y el ciclo no cierra en `interval`. |
| **Zona horaria** | `ts` se guarda en **epoch UTC**; la GUI formatea a local. No guardar horas locales en la tabla. |

---

## 9. Rollback

Totalmente reversible, sin tocar nada de Kamailio upstream:
```bash
systemctl disable --now endpoint-monitor.service
rm /etc/systemd/system/endpoint-monitor.service && systemctl daemon-reload
mysql kamailio -e "DROP TABLE dsip_endpoint_monitor;"
# retirar las 4 rutas /monitor/* y la vista de la GUI
```

---

## Apéndice A — Checklist de archivos a portar

| Del origen | Acción |
|---|---|
| `tools/endpoint-monitor/schema.sql` | Aplicar tal cual |
| `tools/endpoint-monitor/monitor.py` | Copiar; adaptar **solo** `load_endpoints()` (§4.1) |
| `tools/endpoint-monitor/endpoint-monitor.service` | Copiar; ajustar rutas/usuario |
| `gui/modules/local_api/routes.py` (marker `dashboard_endpoint_monitor`, 4 rutas) | **Reescribir** como contrato §5.3 en tu backend |
| `gui/templates/apolo_monitor.html` | **Reescribir/portar** como §5.4 en tu frontend |

## Apéndice B — Referencias del origen
- Memoria del proyecto: `project_tcontakto_qualify_monitor` (nació del incidente TCONTAKTO).
- Flags del dispatcher: `reference_kamailio_dispatcher_flags`.
- README original: `tools/endpoint-monitor/README.md`.
