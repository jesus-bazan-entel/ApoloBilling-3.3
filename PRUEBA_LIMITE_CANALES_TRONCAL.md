# Prueba de Límite de Canales de Troncal PBX → SBC

**Fecha:** 2026-10-21  
**Nodo Asterisk:** 10.10.22.8  
**SBC (Kamailio/dSIPRouter):** 10.10.22.4:5060  
**Autor:** Jesus Bazan

---

## 1. Objetivo

Validar el límite de canales concurrentes configurado en el SBC para la troncal PBX, generando llamadas de prueba automatizadas desde el nodo Asterisk hacia el SBC y detectando el punto en que éste comienza a rechazar llamadas adicionales.

---

## 2. Topología del Escenario

```
┌─────────────────────────────────────────────────────────────┐
│              NODO ASTERISK (10.10.22.8:5060)                │
│                                                             │
│  Anexo 1001 ──► [internal] ──► PJSIP/EXTEN@sbc-trunk       │
│                                                             │
│  Script de prueba:                                          │
│  call file → Local/EXTEN@test-trunk-limit                   │
│              └──► Dial(PJSIP/EXTEN@sbc-trunk)              │
└──────────────────────────┬──────────────────────────────────┘
                           │ SIP/UDP :5060
                           ▼
┌─────────────────────────────────────────────────────────────┐
│              SBC / KAMAILIO (10.10.22.4:5060)               │
│              dSIPRouter 0.78                                │
│                                                             │
│  drouting module → dr_gateways (type=8: carriers)          │
│                 → dr_gateways (type=9: FreeSWITCH local)   │
│                                                             │
│  dsip_call_settings → límite concurrente por gwgroup       │
└──────────────────────────┬──────────────────────────────────┘
                           │ SIP/UDP
                           ▼
┌─────────────────────────────────────────────────────────────┐
│         CARRIER EXTERNO (VOIPSWITCH → 172.16.1.25)          │
│                    hop: 10.124.194.14                       │
│                    IVR destino: 10.35.13.100:5060           │
└─────────────────────────────────────────────────────────────┘
```

### Componentes del nodo

| Servicio | Versión | Puerto | Estado |
|----------|---------|--------|--------|
| Asterisk | 22.11.0 | 5060/UDP | Activo |
| PJSIP trunk `sbc-trunk` | — | → 10.10.22.4:5060 | Avail |
| Anexo SIP 1001 | — | registro digest | Avail |

---

## 3. Implementación

### 3.1 Contextos de Dialplan

Archivo: `/etc/asterisk/extensions_trunk_test.conf`

```ini
;==============================================================================
; extensions_trunk_test.conf
; Contextos auxiliares para prueba de límite de canales de troncal PBX.
;==============================================================================

[test-trunk-limit]
; Origina llamada saliente hacia el SBC y mantiene el canal local vivo.
; Answer() adelantado para que el call file considere la llamada respondida.
exten => _X.,1,NoOp(TEST-TRUNK: ${EXTEN} via sbc-trunk [timeout=${TEST_DIAL_TIMEOUT:-45}s])
 same => n,Answer()
 same => n,Dial(PJSIP/${EXTEN}@sbc-trunk,${TEST_DIAL_TIMEOUT:-45})
 same => n,Hangup()

[test-trunk-hold]
; Contexto de retorno para loopback: el SBC debe enrutar el DID 174021XX
; de regreso a este Asterisk para que las llamadas queden sostenidas.
exten => _174021XX,1,NoOp(TEST-HOLD: DID ${EXTEN} respondido para prueba de canal)
 same => n,Answer()
 same => n,Wait(${TEST_HOLD_DURATION:-120})
 same => n,Hangup()

exten => _X.,1,Hangup(21)
```

Incluido en `/etc/asterisk/extensions.conf`:
```ini
#include extensions_trunk_test.conf
```

### 3.2 Dialplan de Producción — Contexto [internal]

Archivo: `/etc/asterisk/extensions.conf`

Patrones permitidos para salida por troncal SBC (Perú):

```ini
[internal]
; 9 dígitos empezando con 9 (móviles Perú: ej. 987654321)
exten => _9XXXXXXXX,1,NoOp(Saliente ${EXTEN} via troncal SBC)
 same => n,Set(CALLERID(num)=1001)
 same => n,Dial(PJSIP/${EXTEN}@sbc-trunk,60)
 same => n,Hangup()

; 7 dígitos (fijos Lima Perú: ej. 3119898)
exten => _XXXXXXX,1,NoOp(Saliente ${EXTEN} via troncal SBC)
 same => n,Set(CALLERID(num)=1001)
 same => n,Dial(PJSIP/${EXTEN}@sbc-trunk,60)
 same => n,Hangup()

; 9 dígitos con 0 inicial (fijos provincia Perú con código de área: ej. 014567890)
exten => _0XXXXXXXX,1,NoOp(Saliente ${EXTEN} via troncal SBC)
 same => n,Set(CALLERID(num)=1001)
 same => n,Dial(PJSIP/${EXTEN}@sbc-trunk,60)
 same => n,Hangup()

; Todo lo demas: rechazado
exten => _X.,1,NoOp(Bloqueada ${EXTEN})
 same => n,Answer()
 same => n,Playback(pbx-invalid)
 same => n,Hangup()
```

> **Nota:** El patrón original solo permitía `_9XXXXXXXX`. Se agregaron `_XXXXXXX` y `_0XXXXXXXX` para soportar fijos Lima (7 dígitos) y provincia (9 dígitos con 0).

### 3.3 Configuración PJSIP — Troncal sbc-trunk

Archivo: `/etc/asterisk/pjsip.conf`

```ini
[sbc-trunk]
type=endpoint
context=from-sbc
disallow=all
allow=ulaw
allow=alaw
allow=g722
direct_media=no
force_rport=yes
rewrite_contact=yes
rtp_symmetric=yes
dtmf_mode=rfc4733
send_rpid=no
allow_subscribe=no
aors=sbc-trunk-aor
; Autenticación por IP de origen, sin credenciales digest

[sbc-trunk-aor]
type=aor
contact=sip:10.10.22.4:5060
qualify_frequency=30
qualify_timeout=3

[sbc-trunk-identify]
type=identify
endpoint=sbc-trunk
match=10.10.22.4
```

### 3.4 Script de Prueba Automatizado

Archivo: `/usr/local/bin/test-trunk-channels.sh`

El script genera llamadas concurrentes mediante call files en el spool de Asterisk y monitorea las respuestas SIP para detectar el límite del SBC.

#### Modos de operación

| Modo | Descripción | Uso recomendado |
|------|-------------|-----------------|
| `burst` | Envía todas las llamadas en ráfaga rápida | Detectar límite con llamadas que fallan rápido |
| `ramp` | +1 llamada por segundo | Monitoreo gradual con IVR que responde |
| `sustain` | Mantiene N canales activos reemplazando terminadas | Prueba de carga sostenida |

#### Opciones principales

```
-m, --mode         burst|ramp|sustain       (default: burst)
-n, --max-channels N                        (default: 30)
-D, --fixed-dest   NUMERO                  todas las llamadas al mismo número
-t, --dial-timeout SEGUNDOS                (default: 45)
-i, --interval     MS entre llamadas burst (default: 100)
-s, --sustain-time SEGUNDOS modo sustain   (default: 120)
-v, --verbose      detalle por llamada
    --dry-run      muestra config sin ejecutar
```

#### Mecanismo de call file (atomic write)

```bash
# Escribe en /tmp primero, luego mv atómico al spool
tmpfile=$(mktemp /tmp/trunk_call_XXXXXX.tmp)
printf 'Channel: Local/%s@test-trunk-limit/n\n' "$dest" > "$tmpfile"
printf 'MaxRetries: 0\n'     >> "$tmpfile"
printf 'WaitTime: 5\n'       >> "$tmpfile"
printf 'Application: Wait\n' >> "$tmpfile"
printf 'Data: %s\n' "$DIAL_TIMEOUT" >> "$tmpfile"
printf 'Setvar: TEST_DIAL_TIMEOUT=%s\n' "$DIAL_TIMEOUT" >> "$tmpfile"
mv "$tmpfile" /var/spool/asterisk/outgoing/trunk_test_XXXX.call
```

> El canal `Local/EXTEN@test-trunk-limit/n` (sufijo `/n`) desactiva la optimización early-bridge, manteniendo el A-leg vivo mientras el B-leg (PJSIP) está en curso. Esto produce 3 canales por llamada: Local A-leg + Local B-leg + PJSIP outbound.

#### Señales de límite detectadas

El script detecta dos tipos de respuesta de límite del SBC:

| Código SIP | Significado | Origen típico |
|-----------|-------------|---------------|
| `503 Service Unavailable` | Límite de canales genérico | Kamailio directo |
| `486 Busy Here` | Límite por `dsip_call_settings` | dSIPRouter call limit |

---

## 4. Pruebas Realizadas

### Test 1 — Burst 15 llamadas (destinos únicos)

```bash
test-trunk-channels.sh -m burst -n 15 -i 50
```

| Resultado | Valor |
|-----------|-------|
| Llamadas enviadas | 15 |
| Máx canales Asterisk | ~30 |
| 503 / 486 detectados | 0 |
| Conclusión | Límite > 15 |

### Test 2 — Sustain 30 canales × 60 segundos

```bash
test-trunk-channels.sh -m sustain -n 30 -s 60
```

| Resultado | Valor |
|-----------|-------|
| Llamadas enviadas | 30+ |
| Máx canales Asterisk | ~45 |
| 503 / 486 detectados | 0 |
| Conclusión | Llamadas terminaban rápido, sin sostenerse |

### Test 3 — Burst 7 llamadas al IVR 3119898

```bash
test-trunk-channels.sh -m burst -n 7 -i 50 -D 3119898 -t 90
```

| Resultado | Valor |
|-----------|-------|
| Llamadas enviadas | 7 |
| Máx canales Asterisk | 21 (7 × 3) |
| Duración activa | ~60s (IVR respondió) |
| 503 / 486 detectados | 0 |
| Flujo SIP confirmado | INVITE → 183 → PRACK → 200 OK → ACK → BYE |

Traza SIP completa confirmada vía `pjsip set logger on`:
- INVITE enviado a `UDP:10.10.22.4:5060` ✓
- SBC procesa con intercambio PRACK (100rel) ✓
- `200 OK` recibido desde IVR en `sip:3119898@10.35.13.100:5060` ✓
- Record-Route: `10.124.194.14:5060` → `10.10.22.4:5060` ✓

### Test 4 — Ramp 10 llamadas al IVR 3119898 (**límite detectado**)

```bash
test-trunk-channels.sh -m ramp -n 10 -D 3119898 -t 90
```

| Resultado | Valor |
|-----------|-------|
| Llamadas enviadas | 10 |
| Máx canales Asterisk | **15** (techo en 5 llamadas) |
| 486 vistos en sngrep SBC | **5 BUSY** |
| 200 OK vistos en sngrep SBC | **5 COMPLETED** |
| **Límite confirmado** | **5 canales concurrentes** |

**Evidencia del sngrep en el SBC:**

```
Idx  Method  SIP From              SIP To               Msgs  State
866  INVITE  anonymous@anon.inv   3119898@10.10.22.4   22    COMPLETED
867  INVITE  anonymous@anon.inv   3119898@10.10.22.4   22    COMPLETED
871  INVITE  anonymous@anon.inv   3119898@10.10.22.4   22    COMPLETED
876  INVITE  anonymous@anon.inv   3119898@10.10.22.4   22    COMPLETED
880  INVITE  anonymous@anon.inv   3119898@10.10.22.4   22    COMPLETED
881  INVITE  anonymous@anon.inv   3119898@10.10.22.4    3    BUSY  ← límite
885  INVITE  anonymous@anon.inv   3119898@10.10.22.4    3    BUSY
887  INVITE  anonymous@anon.inv   3119898@10.10.22.4    3    BUSY
891  INVITE  anonymous@anon.inv   3119898@10.10.22.4    3    BUSY
894  INVITE  anonymous@anon.inv   3119898@10.10.22.4    3    BUSY
```

- **22 mensajes** = diálogo SIP completo (INVITE → 200 OK → BYE)
- **3 mensajes** = rechazo rápido (INVITE → 100 Trying → **486 Busy Here**)

El límite es aplicado por `dsip_call_settings` en dSIPRouter, no por el carrier.

### Test 5 — Repetición ramp 10 llamadas (verificación de recuperación)

```bash
test-trunk-channels.sh -m ramp -n 10 -D 3119898 -t 90
```

| Resultado | Valor |
|-----------|-------|
| Máx canales Asterisk | 15 (mismo techo) |
| Comportamiento | Idéntico al Test 4 |
| Conclusión | Límite 5 canales estable y consistente |

---

## 5. Hallazgos

### 5.1 Límite de canales confirmado

> **El SBC (dSIPRouter/Kamailio) está configurado con un límite de 5 canales concurrentes para esta troncal PBX.**

El rechazo se produce con `486 Busy Here` (no `503 Service Unavailable`), lo que indica que el límite está en `dsip_call_settings` y no en el drouting estándar.

### 5.2 Patrón de canales Asterisk como indicador

Con `Local/EXTEN@context/n`, cada llamada genera 3 canales en Asterisk:

```
Local/3119898@test-trunk-limit/n;1  (A-leg — pbx_spool Wait)
Local/3119898@test-trunk-limit/n;2  (B-leg — Dial)
PJSIP/sbc-trunk-XXXXXXXX            (canal SIP hacia el SBC)
```

Por lo tanto: **canales Asterisk ÷ 3 = llamadas concurrentes reales**.

Con techo en 15 canales → 15 ÷ 3 = **5 llamadas concurrentes** = límite SBC.

### 5.3 Comportamiento del drouting ante rechazos masivos

Al acumular múltiples rechazos `486 Busy Here` en ráfaga, Kamailio puede marcar el carrier/gateway como **degradado** y redirigir las siguientes llamadas al FreeSWITCH local (tipo=9, gwid=100, `127.0.0.1:5080`). FreeSWITCH entonces devuelve `603 Decline`.

**Síntoma:** La troncal reporta `Avail` (OPTIONS keepalive OK) pero todas las llamadas reciben `603 Decline` con `User-Agent: FreeSWITCH`.

**Resolución:** El carrier se recupera automáticamente en ~5-10 minutos, o bien forzar recarga en el SBC:
```bash
kamcmd drouting.reload
```

---

## 6. Configuración del Límite en el SBC

### Consultar el límite actual

En el SBC (MySQL — base de datos `kamailio`):

```sql
SELECT gwgroupid, max_calls_inbound, max_calls_outbound
FROM dsip_call_settings;
```

### Modificar el límite

```sql
UPDATE dsip_call_settings
SET max_calls_outbound = 30
WHERE gwgroupid = <id_del_grupo_pbx>;
```

Recargar en Kamailio sin reiniciar:
```bash
kamcmd drouting.reload
kamcmd htable.reload gw2gwgroup
```

### Verificar reglas de enrutamiento para un prefijo

```sql
SELECT r.ruleid, r.prefix, r.gwlist, g.gwid, g.address, g.type, g.flags
FROM dr_rules r
JOIN dr_gw_lists l ON FIND_IN_SET(l.id, r.gwlist)
JOIN dr_gateways g ON FIND_IN_SET(g.gwid, l.gwlist)
WHERE '3119898' LIKE CONCAT(r.prefix, '%')
ORDER BY LENGTH(r.prefix) DESC
LIMIT 10;
```

---

## 7. Uso del Script de Prueba

### Instalación

El script está en `/usr/local/bin/test-trunk-channels.sh` y es ejecutable por root.

### Ejemplos de uso

```bash
# Prueba básica — burst 30 llamadas (detectar límite rápidamente)
test-trunk-channels.sh -m burst -n 30 -i 50 -D 3119898

# Ramp gradual — observar en qué canal empieza el rechazo
test-trunk-channels.sh -m ramp -n 20 -D 3119898 -t 90

# Carga sostenida — verificar estabilidad a N canales
test-trunk-channels.sh -m sustain -n 5 -s 120 -D 3119898

# Dry-run — ver configuración sin ejecutar
test-trunk-channels.sh -m burst -n 10 -D 3119898 --dry-run
```

### Interpretación de resultados

| Indicador | Significado |
|-----------|-------------|
| Canales Asterisk se estabilizan en N | Límite SBC = N ÷ 3 llamadas |
| `486` en contador del script | SBC rechaza por `dsip_call_settings` |
| `503` en contador del script | SBC rechaza por drouting/capacidad |
| `603 Decline` con User-Agent FreeSWITCH | Carrier degradado, Kamailio rutea a FS local |
| Canales suben y bajan en ~60s | IVR respondió y colgó por silencio |

### Monitoreo simultáneo en el SBC

Desde el SBC (10.10.22.4), ejecutar sngrep durante la prueba:

```bash
# Ver todas las llamadas desde este Asterisk
sngrep host 10.10.22.8

# Sin filtro (ver flujo completo)
sngrep port 5060
```

Los INVITEs con **22 mensajes** son llamadas completadas; los de **3 mensajes** son rechazos por límite de canal.

---

## 8. Problemas Conocidos y Soluciones Aplicadas

| Problema | Causa | Solución aplicada |
|----------|-------|------------------|
| Llamadas desde anexo 1001 a fijos Lima no progresan | `[internal]` solo permitía `_9XXXXXXXX` (móviles) | Agregados patrones `_XXXXXXX` y `_0XXXXXXXX` |
| Contador 486 siempre en 0 en el reporte | PJSIP logger escribe en `messages.log`, no en `full_trunk_test.log` | Pendiente de corrección (el canal count es el indicador real) |
| `603 Decline` tras ráfaga de tests | Kamailio marca carrier como degradado por rechazos masivos | Esperar ~5-10 min o `kamcmd drouting.reload` en SBC |
| `bc: command not found` | `bc` no instalado en el sistema | Reemplazado por `awk "BEGIN{printf..."` |
| sngrep abre TUI interactiva | `sngrep -q` aún abre curses | Ejecutado con `setsid ... < /dev/null > /dev/null &` |
| Call files fallan por permisos | `/var/spool/asterisk/outgoing/` es de asterisk:asterisk | Write en `/tmp` + `mv` atómico al spool |

---

## 9. Archivos Modificados / Creados

| Archivo | Tipo | Descripción |
|---------|------|-------------|
| `/usr/local/bin/test-trunk-channels.sh` | Script bash | Script principal de prueba automatizada |
| `/etc/asterisk/extensions_trunk_test.conf` | Dialplan Asterisk | Contextos `[test-trunk-limit]` y `[test-trunk-hold]` |
| `/etc/asterisk/extensions.conf` | Dialplan Asterisk | Patrones de salida actualizados + `#include` agregado |
