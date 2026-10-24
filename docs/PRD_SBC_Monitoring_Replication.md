# PRD — Réplica del Dashboard de Monitoreo SBC Carrier Monitoring (Vodafone Grade) en otro SBC

**Versión:** 1.0 — 2026-10-03
**Origen:** Implementación probada en ApoloBilling (`/opt/ApoloBilling`)
**Objetivo:** Replicar el dashboard **SBC Carrier Monitoring** (KPIs, series temporales, tablas de detalle SIP/media/DB/sistema, alertas e histórico por niveles) en otro servidor SBC con Kamailio + RTPengine + MariaDB/MySQL.

---

## 1. Resumen ejecutivo

El dashboard es una SPA React que consume una API de monitoreo (`/api/v1/monitoring/*`) servida por un microservicio Python (FastAPI, puerto 8080). Un bucle colector (cada 60 s) muestrea 4 fuentes, persiste filas raw de 1 minuto en PostgreSQL y genera rollups horarios/diarios para histórico carrier-class (raw 30 d → hourly 93 d → daily 400 d). Nginx expone todo por HTTPS con proxy a backend, API y servicio de monitoreo.

**Componentes a replicar (8 piezas):**

| # | Pieza | Origen en ApoloBilling |
|---|-------|------------------------|
| 1 | Tablas PostgreSQL de monitoreo | `backend/app/models/monitoring.py` + `backend/migrations/add_monitoring_tables.py` |
| 2 | Colectores (Kamailio / RTPengine / MariaDB / Sistema) | `backend/app/monitoring/service.py` |
| 3 | API REST `/api/v1/monitoring/*` | `backend/app/api/routers/monitoring.py` (+ `app/schemas/monitoring.py`) |
| 4 | Microservicio puerto 8080 | `monitoring_service.py` (raíz) + `apolo-monitoring.service` (systemd) |
| 5 | Frontend dashboard | `frontend/src/pages/Monitoring/MonitoringDashboard.tsx`, `frontend/src/components/Monitoring/KPICard.tsx`, `frontend/src/types/monitoring.ts`, ruta `/monitoring` en `App.tsx` |
| 6 | Nginx: proxy + HTTPS | `/etc/nginx/sites-available/apolobilling.conf` (plantilla en `deploy/nginx/`) |
| 7 | Requisitos Kamailio | módulos + `ctl` socket + dispatcher con probing (ver §4) |
| 8 | Requisitos RTPengine/MariaDB | NG `127.0.0.1:7722`, usuario MySQL de lectura (ver §4) |

---

## 2. Arquitectura

```
┌─────────────┐   kamcmd (ctl socket)    ┌──────────────┐
│  Kamailio   │◄─────────────────────────│              │
└─────────────┘   NG UDP 127.0.0.1:7722  │  Colector    │
┌─────────────┐◄─────────────────────────│  (cada 60s)  │
│ RTPengine   │   SHOW GLOBAL STATUS     │  4 fuentes   │
└─────────────┘◄─────────────────────────│              │
┌─────────────┐   psutil                 └──────┬───────┘
│MariaDB/Sist.│◄───────────────────────────────┘
└─────────────┘
                        │ escribe raw 1-min + rollups hourly/daily
                        ▼
               ┌──────────────────┐   :8080 /api/v1/monitoring/*
               │   PostgreSQL     │◄── FastAPI (monitoring_service)
               │ monitoring_*     │
               └──────────────────┘
                        ▲
┌──────────┐  :443 HTTPS ┌───────┐  /api/→:8000  ┌────────┐
│ Navegador│◄──────────►│ Nginx │──/api/v1/monitoring/→:8080─┤ React  │
└──────────┘  301 80→443└───────┘  /ws,/→front               │ dist   │
                                                             └────────┘
```

**Puertos:** 443 (HTTPS público) · 80 (solo redirect 301) · 8080 (monitoreo, solo loopback tras Nginx) · 7722/UDP (NG rtpengine, loopback) · socket `kamcmd` vía `ctl`.

---

## 3. Requisitos previos del SBC destino

### 3.1 Software

| Paquete | Versión probada | Notas |
|---------|-----------------|-------|
| Kamailio | 5.x con `kamcmd` en `/usr/sbin/kamcmd` | Módulos obligatorios: `ctl`, `cfg_rpc`, `tm`, `tmx`, `sl`, `dialog` (con `enable_stats=1`), `dispatcher`, `drouting` |
| RTPengine | con `listen-ng = 127.0.0.1:7722` | Habilitar estadísticas RTCP/MOS si se quiere MOS/jitter reales |
| MariaDB/MySQL | 10.x/11.x | Usuario solo-lectura global: `SHOW GLOBAL STATUS`, `SHOW GLOBAL VARIABLES`, `SHOW ENGINE INNODB STATUS`, `SHOW PROCESSLIST` |
| PostgreSQL | 13+ | BD del billing (`apolo_billing`); el monitoreo escribe 8 tablas propias |
| Python | 3.11+ | `fastapi==0.104.1`, `uvicorn[standard]==0.24.0`, `sqlalchemy==2.0.23`, `psycopg2-binary`, `psutil`, `mysql-connector-python` (estos dos últimos **no están** en `backend/requirements.txt`: instalarlos aparte) |
| Node | 18+ | Solo para compilar frontend (`@tanstack/react-query@5`, `recharts@3`, `@radix-ui/*`) |
| Nginx | 1.22+ | Termina TLS, proxy inverso |

### 3.2 Kamailio — configuración mínima requerida

```cfg
loadmodule "ctl.so"
loadmodule "cfg_rpc.so"
loadmodule "tm.so"
loadmodule "tmx.so"
loadmodule "sl.so"
loadmodule "dialog.so"
loadmodule "dispatcher.so"

modparam("ctl", "binrpc", "unix:/var/run/kamailio/kamailio_ctl")
modparam("ctl", "mode", 0660)   # + usuario colector en grupo kamailio

modparam("dialog", "enable_stats", 1)
modparam("dialog", "hash_size", 4096)
modparam("dialog", "db_mode", 0)

modparam("dispatcher", "flags", 2)
modparam("dispatcher", "ds_probing_mode", 3)      # probing continuo
modparam("dispatcher", "ds_ping_method", "OPTIONS")
modparam("dispatcher", "ds_ping_interval", 60)
modparam("dispatcher", "ds_probing_threshold", 1)
modparam("dispatcher", "ds_inactive_threshold", 1)
```

Verificación obligatoria (como root) antes de seguir:

```bash
kamcmd stats.get_statistics all | head          # debe listar core:rcv_requests_invite, etc.
kamcmd dlg.stats_active                         # { ongoing / all }
kamcmd tm.stats                                  # { current / waiting / total }
kamcmd dispatcher.list | head                    # DEST con FLAGS A / DX
kamcmd shm.stats                                 # { total / free / real_used }
```

> **Lección aprendida (2026-10-03):** `kamcmd` exige cada token como argumento separado: `kamcmd stats.get_statistics all`. Pasar `"stats.get_statistics all"` como un solo argumento devuelve `error: 500 - command not found` con exit 0. El colector ya implementa `shlex.split()` (§6.1). Y transacciones se leen de **`tm.stats`** (`current`), no de `stats.get_statistics tmx` (no existe).

### 3.3 RTPengine / MariaDB

- `rtpengine.conf`: `listen-ng = 127.0.0.1:7722` (el colector habla bencode/UDP con cookie).
- MySQL: crear usuario de monitoreo con `PROCESS` + `SELECT` suficiente para los 4 `SHOW` globales; ajustar `host/port/user/password/database` en `MariaDBCollector.config` (`service.py:594`).

---

## 4. Base de datos (PostgreSQL)

### 4.1 Tablas

Copiar `backend/app/models/monitoring.py` (8 modelos):

- `monitoring_kamailio_metrics` — diálogos (`active_dialogs/calls`, `early`, `confirmed`), contadores SIP acumulados (`invite_received/sent`, `invite_200ok/4xx/5xx`, `bye/cancel/ack/options/register_received`, `register_200ok`), dispatcher (`active/inactive_destinations`), transacciones, SHM (`shm_used/free_mb`, `fragments`), errores (`parse/script/db`), `raw_data JSONB`.
- `monitoring_rtpengine_metrics` — sesiones/puertos, paquetes/bytes acumulados, pérdida/jitter/RTT/MOS, errores offer/answer/delete/timeout, cpu/mem, `raw_data`.
- `monitoring_mariadb_metrics` — hilos/conexiones, queries, InnoDB, replicación, tmp tables, `raw_data`.
- `monitoring_system_metrics` — cpu/load/mem/swap/disco/red + cpu/mem por proceso (kamailio, rtpengine, freeswitch, postgres), `raw_data`.
- `monitoring_alerts` — `alert_name, severity, component, metric_name, threshold_value, current_value, operator, status (firing/resolved/acknowledged)`.
- `monitoring_config` — por componente: `thresholds JSONB, interval_seconds, retention_days (default 30), enabled`.
- `monitoring_kpi_hourly` / `monitoring_kpi_daily` — `(component, metric, bucket_start)` único + `avg/min/max/samples`.

### 4.2 Migración

```bash
cd /opt/ApoloBilling/backend
python3 migrations/add_monitoring_tables.py   # create_all + verificación
```

Política de retención (implementada en `MonitoringService._retention`, corre ≤1 vez/hora): raw según `monitoring_config.retention_days` (30 d), `raw_data` JSONB se nulifica tras 48 h, hourly 93 d, daily 400 d, alertas resueltas >30 d se purgan.

---

## 5. Colectores (`backend/app/monitoring/service.py`)

Bucle `MonitoringService.run_collection_loop()` cada 60 s → `collect_all()` (4 colectores en threads) → `store_metrics()` → `_check_alerts()` → `_rollup_hourly()` (últimas 3 h completadas, idempotente) → `_rollup_daily()` (ayer desde hourly) → `_retention_gated()`. **Importante:** `CollectorResult(success=True)` aun con datos vacíos; si un panel sale en 0 con tráfico real, sospechar del comando `kamcmd` antes que de la DB.

| Colector | Fuente | Comandos exactos | Métricas clave |
|----------|--------|------------------|----------------|
| `KamailioCollector` | `kamcmd` (`/usr/sbin/kamcmd`, timeout 5 s) | `stats.get_statistics all` (formato `k = v`) → contadores `core:*`; `dlg.stats_active` (formato `k: v`) → `all/connecting/answering/ongoing`; `dispatcher.list` (texto, cuenta `FLAGS: AP/A` vs `D/X/DX`) → destinos; `tm.stats` → `current` = transacciones activas; `shm.stats` → `real_used/free/fragments` (bytes→MB) | INVITE rx, 200/4xx/5xx a INVITE, BYE/CANCEL/ACK, OPTIONS/REGISTER, diálogos, SHM %, salud dispatcher |
| `RtpengineCollector` | NG UDP 7722 (bencode + cookie) + `kamcmd rtpengine.get_hash_total` + psutil | `list` → call-ids = sesiones activas (autoritativo); `statistics` → `currentstatistics/totalstatistics/mos/voip_metrics/controlstatistics`; `query` (muestra ≤8 calls) → puertos, one-sided; fallback hash Kamailio | sesiones, puertos usados/libres (rango 10000–20000 configurable), paquetes/bytes relayed, timeouts, MOS/jitter/RTT (0 = sin muestras RTCP-XR, no fallo) |
| `MariaDBCollector` | `mysql.connector` | `SHOW GLOBAL STATUS / VARIABLES`, `SHOW ENGINE INNODB STATUS`, `SHOW PROCESSLIST` | `threads_connected` vs `@@max_connections` (¡no `Max_used_connections`!), QPS = ΔQuestions/60, slow queries, deadlocks, buffer pool |
| `SystemCollector` | `psutil` | cpu/mem/disco/red + por proceso (nombre contiene kamailio/rtpengine/freeswitch/postgres) | CPU %, load, disco %, throughput red, RSS por servicio |

Reglas derivadas (overview): success rate = `200/(200+4xx+5xx)`; dispatcher `inactive==0→healthy, >50%→critical, else degraded, total==0→unknown`; SHM `>90 critical, >75 warning`; DB pool `>90/>70`; media sin sesiones ni MOS = `unknown`, no critical.

---

## 6. API (`backend/app/api/routers/monitoring.py`, prefijo `/api/v1/monitoring`)

- `GET /overview` → `DashboardOverview` (resúmenes + conteo alertas; ventana último minuto).
- `GET /{kamailio,rtpengine,mariadb,system}/latest` → última fila.
- `GET /{component}/history?start&end&limit` → filas raw.
- `GET /{component}/timeseries?metric&hours&resolution=auto|raw|hourly|daily&max_points=500` → **niveles:** `hours≤12→raw`, `≤2232 (93 d)→hourly`, resto `daily`; con fallback a raw si no hay rollup; downsampling por stride. Soporta **alias** (`avg_mos→mos_avg`) y **derivadas** (`calls_per_second` = tasa de `invite_received`; `success_rate`; `shm_usage_percent`; `packet_loss_percent`; `media/net_throughput_mbps`).
- `GET /alerts?status&severity&component&limit` · `POST /alerts/{id}/acknowledge?acknowledged_by=` · `GET /history/status` (cobertura por nivel, mostrado en la UI) · `GET /config` · `GET /health`.

### 6.1 Microservicio puerto 8080

`monitoring_service.py` (raíz): FastAPI con `lifespan` que lanza `run_collection_loop()`, monta el router en `/api/v1/monitoring`, `DATABASE_URL` del billing. Systemd (`/etc/systemd/system/apolo-monitoring.service`):

```ini
[Service]
WorkingDirectory=/opt/ApoloBilling
Environment=PYTHONPATH=/opt/ApoloBilling/backend
Environment=DATABASE_URL=postgresql://apolo_user:apolo_password_2024@127.0.0.1:5432/apolo_billing
ExecStart=/usr/bin/python3 -m monitoring_service
Restart=always
```

---

## 7. Frontend

Archivos a copiar: `src/pages/Monitoring/MonitoringDashboard.tsx` (7 KPI cards + gráficas `AreaChart` 1h/6h/24h/7d/30d con auto-refresh 30 s + 5 tabs: Kamailio/RTPengine/MariaDB/System/Alerts), `src/components/Monitoring/KPICard.tsx` (card con reason + método de cálculo), `src/types/monitoring.ts`, ruta `path="/monitoring"` en `App.tsx`. Cliente `src/api/client.ts` usa `baseURL: '/api/v1'` (rutas relativas: funciona igual en HTTP y HTTPS) y WebSocket `wss:` automático si `location.protocol === 'https:'`.

Dependencias npm: `@tanstack/react-query@5`, `recharts@3`, `@radix-ui/{tabs,select,tooltip}`, `lucide-react`. Build (`vite`) → `dist/` servido por Nginx.

Contenido de cada tab (detalle, sin repetir KPIs): Kamailio = diálogos/transacciones + **Métodos SIP acumulado desde reinicio** + dispatcher + SHM + errores; RTPengine = sesiones/puertos + tráfico acumulado + calidad RTCP-XR + señalización NG; MariaDB = conexiones/rendimiento/tmp tables; System = carga/disco/procesos.

---

## 8. Nginx + HTTPS

Bloque HTTPS (plantilla viva: `deploy/nginx/apolobilling-https.conf`):

```nginx
server {  # HTTP solo redirige
    listen 80; listen [::]:80; server_name _;
    return 301 https://$host$request_uri;
}
server {  # Dashboard https://<IP>/
    listen 443 ssl; listen [::]:443 ssl; server_name _;
    ssl_certificate /etc/nginx/ssl/apolo.crt;
    ssl_certificate_key /etc/nginx/ssl/apolo.key;
    root /opt/ApoloBilling/frontend/dist; index index.html;

    location /api/v1/monitoring/ { proxy_pass http://127.0.0.1:8080; ... }  # ¡antes que /api/!
    location /api/ { proxy_pass http://127.0.0.1:8000; ... }
    location /ws  { proxy_pass http://127.0.0.1:8000; upgrade... }            # wss automático
    location / { try_files $uri $uri/ /index.html; }
}
```

Certificado autofirmado multi-IP (825 días):

```bash
openssl req -x509 -nodes -days 825 -newkey rsa:2048 \
 -keyout /etc/nginx/ssl/apolo.key -out /etc/nginx/ssl/apolo.crt \
 -subj "/CN=<IP_PRINCIPAL>" \
 -addext "subjectAltName=IP:<IP1>,IP:<IP2>,IP:127.0.0.1,DNS:localhost"
nginx -t && systemctl reload nginx
```

Para producción con dominio sustituir por Let's Encrypt (`certbot --nginx`). El navegador pedirá excepción con cert autofirmado.

---

## 9. Plan de réplica paso a paso (orden)

1. **SBC base:** Kamailio (módulos §3.2) + RTPengine (`listen-ng 127.0.0.1:7722`) + MariaDB + PostgreSQL + Python + Nginx. Verificar los 5 `kamcmd` del §3.2.
2. **MySQL:** crear usuario monitoreo (lectura global) y anotar credenciales.
3. **Código:** copiar `backend/app/models/monitoring.py`, `backend/app/schemas/monitoring.py`, `backend/app/monitoring/service.py`, `backend/app/api/routers/monitoring.py`, `monitoring_service.py`, `migrations/add_monitoring_tables*.py`. Instalar `psutil`, `mysql-connector-python`.
4. **Configurar:** `DATABASE_URL`, `PYTHONPATH`, credenciales MySQL en `MariaDBCollector`, rango de puertos RTP y `kamcmd_path` si difieren.
5. **Migrar DB:** `python3 migrations/add_monitoring_tables.py` y confirmar 8 tablas `monitoring_%`.
6. **Smoke test colector:** `PYTHONPATH=backend python3 -c "from app.monitoring.service import KamailioCollector; print(KamailioCollector().collect().data)"` — todos los contadores SIP deben ser ≠ 0 con tráfico.
7. **Systemd:** instalar y habilitar `apolo-monitoring.service`; esperar 2 ciclos (2 min) y consultar `SELECT invite_received, invite_200ok FROM monitoring_kamailio_metrics ORDER BY timestamp DESC LIMIT 2;`.
8. **Nginx:** bloque 80→301 + 443 con los 3 `location` (§8); `nginx -t && systemctl reload nginx`. Probar `https://<IP>/`, `/api/v1/monitoring/health`, `/api/v1/monitoring/kamailio/latest`.
9. **Frontend:** copiar archivos §7, `npm i && npm run build`, `dist/` a la ruta de `root`, recargar. Abrir `/monitoring`: KPIs, gráficas y tabs con datos.
10. **Alertas:** sembrar `monitoring_config` (thresholds por componente) y verificar `/alerts`.

## 10. Verificación y troubleshooting

| Síntoma | Causa probable | Acción |
|---------|----------------|--------|
| Métodos SIP en 0 con tráfico real | `kamcmd` mal invocado (un solo arg) o grupo stats inexistente | Releer §3.2-§5; probar comandos a mano; revisar `raw_data` de la última fila |
| `transacciones activas` en 0 | Se usa `stats.get_statistics tmx` | Usar `tm.stats` → `current` (§5) |
| `dispatcher total = 0` / unknown | dispatcher sin destinos o sin probing | `kamcmd dispatcher.list`; revisar `ds_probing_mode=3`, flags DX vs A |
| DB pool ~98 % falso | Dividir entre `Max_used_connections` | Usar `@@max_connections` (ya corregido en colector) |
| MOS/jitter/RTT = N/A | Sin muestras RTCP-XR | Esperado sin media RTCP; no es fallo |
| `connect_unix_sock: Permission denied` | socket `ctl` 0600 | `modparam("ctl","mode",0660)` + usuario en grupo `kamailio` |
| Timeseries vacía en rangos largos | Sin rollups aún | Esperar ciclos horarios; `history/status` muestra cobertura |

## 11. Criterios de aceptación

- [ ] Los 5 `kamcmd` del §3.2 devuelven datos.
- [ ] Colector smoke test con contadores SIP ≠ 0.
- [ ] Tras 5 min, `monitoring_kamailio_metrics` crece 1 fila/min con valores crecientes.
- [ ] `GET /api/v1/monitoring/overview`, `/kamailio/latest`, `/history/status` responden 200 vía HTTPS.
- [ ] `/monitoring` muestra KPIs, gráficas en los 5 rangos y las 5 tabs sin errores de API.
- [ ] `http://<IP>/` redirige a `https://<IP>/`.
