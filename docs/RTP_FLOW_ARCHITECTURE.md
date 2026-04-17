# Arquitectura de Flujo RTP - ApoloBilling SBC

> **Documento de Referencia**: Análisis completo del flujo RTP en la arquitectura FreeSWITCH + Kamailio + RTPEngine
> **Fecha de creación**: 2026-04-12
> **Última actualización**: 2026-04-12

---

## Tabla de Contenidos

1. [Arquitectura General](#1-arquitectura-general)
2. [Configuración Actual de RTPEngine](#2-configuración-actual-de-rtpengine)
3. [Flujo RTP - Llamadas Entrantes](#3-flujo-rtp---llamadas-entrantes)
4. [Flujo RTP - Llamadas Salientes](#4-flujo-rtp---llamadas-salientes)
5. [Modos de Operación del Media](#5-modos-de-operación-del-media)
6. [Problema de Hold y Solución send_early_media](#6-problema-de-hold-y-solución-send_early_media)
7. [Análisis: Bypass de FreeSWITCH en RTP](#7-análisis-bypass-de-freeswitch-en-rtp)
8. [Configuraciones para Bypass](#8-configuraciones-para-bypass)

---

## 1. Arquitectura General

### Diagrama de Componentes

```
┌─────────────────────────────────────────────────────────────────────────────────┐
│                              INTERNET / PSTN                                    │
│                                                                                 │
│                     Carrier (ej: 190.105.250.x / 172.16.1.25)                  │
│                                    │                                            │
│                           RTP ◄────┼────► SIP                                   │
│                                    │                                            │
└────────────────────────────────────┼────────────────────────────────────────────┘
                                     │
┌────────────────────────────────────┼────────────────────────────────────────────┐
│                          SERVIDOR SBC                                           │
│                                    │                                            │
│  ┌─────────────────────────────────▼─────────────────────────────────────────┐  │
│  │                         KAMAILIO :5060                                    │  │
│  │                                                                           │  │
│  │   SIP Signaling ◄─────────────────────────────────────► SIP Signaling    │  │
│  │                                                                           │  │
│  │                    ┌─────────────────────────┐                            │  │
│  │                    │      RTPEngine          │                            │  │
│  │                    │    :10000-30000 UDP     │                            │  │
│  │                    │                         │                            │  │
│  │                    │  ┌─────┐     ┌─────┐   │                            │  │
│  │    Carrier RTP ◄───┼──│ Leg │◄───►│ Leg │───┼───► FreeSWITCH RTP        │  │
│  │                    │  │  A  │     │  B  │   │                            │  │
│  │                    │  └─────┘     └─────┘   │                            │  │
│  │                    └─────────────────────────┘                            │  │
│  └───────────────────────────────────────────────────────────────────────────┘  │
│                                    │                                            │
│                               SIP (UDP)                                         │
│                                    │                                            │
│  ┌─────────────────────────────────▼─────────────────────────────────────────┐  │
│  │                         FREESWITCH                                        │  │
│  │                                                                           │  │
│  │   Internal :5080                              External :5062              │  │
│  │   (from-pbx, to-kamailio)                     (hacia carriers)            │  │
│  │                                                                           │  │
│  │         ▲                                                                 │  │
│  │         │ RTP                                                             │  │
│  │         ▼                                                                 │  │
│  │   ┌───────────┐                                                           │  │
│  │   │ Softphone │                                                           │  │
│  │   │ Extension │                                                           │  │
│  │   └───────────┘                                                           │  │
│  └───────────────────────────────────────────────────────────────────────────┘  │
└─────────────────────────────────────────────────────────────────────────────────┘
```

### Puertos del Sistema

| Componente | Puerto SIP | Puertos RTP | Rol |
|------------|------------|-------------|-----|
| Kamailio | 5060 | - | Solo señalización |
| RTPEngine | - | 10000-30000 | Relay/transcodificación de media |
| FreeSWITCH Internal | 5080 | 16384-32767 | Media hacia extensiones |
| FreeSWITCH External | 5062 | 16384-32767 | Media hacia carriers (si no bypass) |
| Softphones | variable | variable | Endpoints finales |

---

## 2. Configuración Actual de RTPEngine

### Archivo: `/etc/rtpengine/rtpengine.conf`

```ini
[rtpengine]
table = -1                    # No usar kernel module (userspace only)
no-fallback = false

# INTERFACES DUALES (clave para la arquitectura)
interface = internal/10.10.22.4;external/10.124.194.142
#           ^^^^^^^^          ^^^^^^^^
#           LAN/PBX           WAN/Internet

listen-ng = 127.0.0.1:7722    # Control socket (Kamailio se conecta aquí)
listen-cli = 127.0.0.1:9900   # CLI para debug

port-min = 10000              # Rango de puertos RTP
port-max = 30000
timeout = 60                  # Timeout de sesión
silent-timeout = 3600         # Timeout sin audio (1 hora)
```

### Interfaces Duales - Concepto

```
                    INTERNET                              LAN
                 (Carriers/PSTN)                    (FreeSWITCH/PBX)
                       │                                  │
                       │                                  │
            ┌──────────▼──────────┐          ┌───────────▼───────────┐
            │  external interface │          │  internal interface   │
            │   10.124.194.142    │◄════════►│     10.10.22.4        │
            │                     │   RTP    │                       │
            │   (puertos 10000-   │  bridge  │   (puertos 10000-     │
            │    30000)           │          │    30000)             │
            └─────────────────────┴──────────┴───────────────────────┘
                                   RTPENGINE
```

### Configuración en Kamailio

```
#!define WITH_RTPENGINE
loadmodule "rtpengine.so"

modparam("rtpengine", "rtpengine_sock", "udp:localhost:7722")
modparam("rtpengine", "rtpengine_retr", 5)
modparam("rtpengine", "hash_table_tout", 60)
```

---

## 3. Flujo RTP - Llamadas Entrantes

### Diagrama de Señalización y Media

```
   CARRIER                KAMAILIO              RTPENGINE            FREESWITCH           SOFTPHONE
     │                       │                      │                     │                    │
     │ INVITE (SDP-A)        │                      │                     │                    │
     │──────────────────────►│                      │                     │                    │
     │                       │                      │                     │                    │
     │                       │ rtpengine_offer()    │                     │                    │
     │                       │─────────────────────►│                     │                    │
     │                       │ SDP-A' (RTPEngine IP)│                     │                    │
     │                       │◄─────────────────────│                     │                    │
     │                       │                      │                     │                    │
     │                       │ INVITE (SDP-A')      │                     │                    │
     │                       │─────────────────────────────────────────►│                    │
     │                       │                      │                     │                    │
     │                       │                      │                     │ INVITE (SDP-B)     │
     │                       │                      │                     │───────────────────►│
     │                       │                      │                     │                    │
     │ 100 Trying            │                      │                     │                    │
     │◄──────────────────────│                      │                     │                    │
     │                       │                      │                     │                    │
     │                       │                      │                     │ 180 Ringing        │
     │                       │                      │                     │◄───────────────────│
     │                       │                      │                     │                    │
     │                       │ 180 (SDP-B')         │                     │                    │
     │                       │◄─────────────────────────────────────────│                    │
     │                       │                      │                     │                    │
     │                       │ rtpengine_answer()   │                     │                    │
     │                       │─────────────────────►│                     │                    │
     │                       │ SDP final            │                     │                    │
     │                       │◄─────────────────────│                     │                    │
     │                       │                      │                     │                    │
     │ 183/180 (SDP-RTPEng)  │                      │                     │                    │
     │◄──────────────────────│                      │                     │                    │
     │                       │                      │                     │                    │
     │                       │                      │                     │ 200 OK (SDP-B)     │
     │                       │                      │                     │◄───────────────────│
     │                       │                      │                     │                    │
     │                       │ 200 OK               │                     │                    │
     │                       │◄─────────────────────────────────────────│                    │
     │                       │                      │                     │                    │
     │ 200 OK (SDP-RTPEng)   │                      │                     │                    │
     │◄──────────────────────│                      │                     │                    │
     │                       │                      │                     │                    │
     │ ACK                   │                      │                     │                    │
     │──────────────────────►│ ACK                  │                     │ ACK               │
     │                       │─────────────────────────────────────────►│───────────────────►│
     │                       │                      │                     │                    │
     ════════════════════════════════════════════════════════════════════════════════════════════
                                         FLUJO RTP ESTABLECIDO
     ════════════════════════════════════════════════════════════════════════════════════════════
     │                       │                      │                     │                    │
     │      RTP ◄────────────────────────────────► │ ◄─────────────────► │ ◄─────────────────►│
     │   (media carrier)     │                   RTPEngine            FreeSWITCH           Softphone
     │                       │                 (relay/transcode)      (si no bypass)        (media)
```

### Flujo RTP Entrante - Detalle de IPs

```
┌────────────┐         ┌─────────────────┐         ┌─────────────┐         ┌──────────┐
│  CARRIER   │         │   RTPENGINE     │         │ FREESWITCH  │         │ SOFTPHONE│
│190.105.x.x │         │  10.10.22.4     │         │ 10.10.22.4  │         │192.168.x │
└─────┬──────┘         └────────┬────────┘         └──────┬──────┘         └────┬─────┘
      │                         │                         │                      │
      │ RTP :10000-30000        │                         │                      │
      │ ◄─────────────────────► │                         │                      │
      │  (Leg A - External)     │                         │                      │
      │                         │                         │                      │
      │                         │ RTP :16384-32767        │                      │
      │                         │ ◄─────────────────────► │                      │
      │                         │  (Leg B - Internal)     │                      │
      │                         │                         │                      │
      │                         │                         │ RTP :16384-32767     │
      │                         │                         │ ◄──────────────────► │
      │                         │                         │  (FS ↔ Softphone)    │
```

---

## 4. Flujo RTP - Llamadas Salientes

### Diagrama de Señalización

```
   SOFTPHONE            FREESWITCH            KAMAILIO              RTPENGINE              CARRIER
     │                      │                     │                      │                     │
     │ INVITE (SDP-A)       │                     │                      │                     │
     │─────────────────────►│                     │                      │                     │
     │                      │                     │                      │                     │
     │                      │ INVITE (SDP-A')     │                      │                     │
     │                      │────────────────────►│                      │                     │
     │                      │                     │                      │                     │
     │                      │                     │ rtpengine_offer()    │                     │
     │                      │                     │─────────────────────►│                     │
     │                      │                     │ SDP-RTPEng           │                     │
     │                      │                     │◄─────────────────────│                     │
     │                      │                     │                      │                     │
     │                      │                     │ INVITE (SDP-RTPEng)  │                     │
     │                      │                     │─────────────────────────────────────────►│
     │                      │                     │                      │                     │
     │ 100 Trying           │                     │                      │                     │
     │◄─────────────────────│                     │                      │                     │
     │                      │                     │                      │                     │
     │                      │                     │                      │ 183 (SDP-B)        │
     │                      │                     │◄─────────────────────────────────────────│
     │                      │                     │                      │                     │
     │                      │                     │ rtpengine_answer()   │                     │
     │                      │                     │─────────────────────►│                     │
     │                      │                     │ SDP final            │                     │
     │                      │                     │◄─────────────────────│                     │
     │                      │                     │                      │                     │
     │                      │ 183 (SDP processed) │                      │                     │
     │                      │◄────────────────────│                      │                     │
     │ 183 (ringback)       │                     │                      │                     │
     │◄─────────────────────│                     │                      │                     │
     │                      │                     │                      │                     │
     │                      │                     │                      │ 200 OK (SDP-B)     │
     │                      │                     │◄─────────────────────────────────────────│
     │                      │ 200 OK              │                      │                     │
     │                      │◄────────────────────│                      │                     │
     │ 200 OK               │                     │                      │                     │
     │◄─────────────────────│                     │                      │                     │
     │                      │                     │                      │                     │
     │ ACK                  │ ACK                 │ ACK                  │                     │
     │─────────────────────►│────────────────────►│─────────────────────────────────────────►│
```

---

## 5. Modos de Operación del Media

### Modo 1: RTPEngine como Relay (FreeSWITCH EN el path)

```
┌─────────┐      RTP       ┌───────────┐      RTP       ┌─────────┐      RTP       ┌─────────┐
│ Carrier │◄──────────────►│ RTPEngine │◄──────────────►│   FS    │◄──────────────►│Softphone│
└─────────┘                └───────────┘                └─────────┘                └─────────┘
   Leg A                                                   Leg B
   (External IP)                                        (Internal IP)
```

**Ventajas:**
- NAT traversal automático
- Oculta IPs internas
- Permite transcodificación
- MOH, DTMF, grabación funcionan

### Modo 2: bypass_media en FreeSWITCH

Cuando `bypass_media=true` en el dialplan:

```
┌─────────┐      RTP       ┌───────────┐      RTP       ┌─────────┐
│ Carrier │◄──────────────►│ RTPEngine │◄──────────────►│Softphone│
└─────────┘                └───────────┘                └─────────┘
                                │
                          (FS no toca RTP)
                                │
                          ┌─────────┐
                          │   FS    │ (solo señalización)
                          └─────────┘
```

### Comparativa

| Aspecto | Sin bypass (FS en RTP) | Con bypass (FS fuera) |
|---------|------------------------|----------------------|
| **Latencia RTP** | +5-20ms (FS procesa) | Mínima (relay directo) |
| **CPU FreeSWITCH** | Alta (procesa media) | Baja (solo SIP) |
| **Música en hold** | ✅ Funciona | ❌ No funciona* |
| **Grabación en FS** | ✅ Funciona | ❌ No funciona |
| **DTMF inband** | ✅ Detecta | ❌ No detecta |
| **Transcodificación** | ✅ Disponible | ✅ RTPEngine lo hace |
| **Escalabilidad** | Limitada por CPU de FS | Alta |

---

## 6. Problema de Hold y Solución send_early_media

### El Problema

Cuando un carrier envía una llamada entrante y la extensión interna la pone en hold, el carrier no recibía audio (música de espera) porque el canal de media no estaba establecido antes del ANSWER.

### La Solución: send_early_media

Se agregó la opción `send_early_media` en `routing_inbound_routes` que genera el siguiente dialplan en FreeSWITCH:

```xml
<!-- Cuando send_early_media = true -->
<action application="set" data="ringback=${us-ring}"/>
<action application="set" data="instant_ringback=true"/>
<action application="pre_answer"/>
<action application="bridge" data="..."/>
```

### Cómo funciona:

1. **`ringback=${us-ring}`** - Configura el tono de timbre US estándar
2. **`instant_ringback=true`** - Envía el tono inmediatamente sin esperar al destino
3. **`pre_answer`** - Envía un **183 Session Progress** al carrier con SDP, estableciendo el canal de media temprano

### Por qué resuelve el problema de hold:

- **Sin early media**: El flujo de media solo se establece después del 200 OK (ANSWER)
- **Con early media**: El 183 Session Progress establece el canal de media bidireccional **antes** del answer

### Configuración

Migración: `database/migrations/009_inbound_early_media.sql`

```sql
ALTER TABLE routing_inbound_routes
ADD COLUMN send_early_media BOOLEAN NOT NULL DEFAULT false;
```

Código en `rust-backend/crates/apolo-api/src/handlers/unified_routing.rs:3226-3230`:

```rust
// Early media: send 183 Session Progress with ringback tone
if route.send_early_media {
    xml.push_str("        <action application=\"set\" data=\"ringback=${us-ring}\"/>\n");
    xml.push_str("        <action application=\"set\" data=\"instant_ringback=true\"/>\n");
    xml.push_str("        <action application=\"pre_answer\"/>\n");
}
```

---

## 7. Análisis: Bypass de FreeSWITCH en RTP

### Arquitectura Propuesta

```
┌──────────┐      ┌───────────────────────────────┐      ┌──────────┐
│ CARRIER  │◄════►│          RTPENGINE            │◄════►│ SOFTPHONE│
└──────────┘ RTP  │                               │ RTP  └──────────┘
                  │  external ◄═══════► internal  │
                  └───────────────────────────────┘
                              ▲
                              │ (RTP directo)
                              │
                  ┌───────────┴───────────┐
                  │      FREESWITCH       │
                  │    (solo SIP/control) │
                  └───────────────────────┘
```

### Problemas a Resolver

#### Problema #1: HOLD (Música de Espera)

**Sin bypass (actual):**
```
SOFTPHONE pone HOLD → FreeSWITCH detecta → FS inyecta MOH → Carrier escucha música ✅
```

**Con bypass (propuesto):**
```
SOFTPHONE pone HOLD → FreeSWITCH detecta → ❌ FS NO puede inyectar MOH
```

**Solución:** MOH en RTPEngine con `play-media`

#### Problema #2: DTMF

| Tipo | Sin Bypass | Con Bypass |
|------|------------|------------|
| RFC 2833 | ✅ FS detecta | ✅ RTPEngine relay |
| SIP INFO | ✅ Funciona | ✅ Funciona (es SIP) |
| Inband | ✅ FS detecta | ⚠️ Requiere RTPEngine DSP |

#### Problema #3: Early Media

**Solución:** RTPEngine genera ringback con `play-media`

### Tabla Comparativa de Impacto

| Funcionalidad | Actual (FS en RTP) | Propuesto (FS bypass) |
|---------------|--------------------|-----------------------|
| Latencia RTP | +5-20ms | Mínima |
| CPU FreeSWITCH | Alta | Baja |
| MOH (Hold) | ✅ FS inyecta | ⚠️ Requiere RTPEngine |
| DTMF RFC 2833 | ✅ | ✅ RTPEngine relay |
| DTMF SIP INFO | ✅ | ✅ (es SIP) |
| DTMF Inband | ✅ | ⚠️ RTPEngine DSP |
| Early Media | ✅ FS pre_answer | ⚠️ RTPEngine |
| Grabación | ✅ FS | ⚠️ RTPEngine/SIPREC |
| Escalabilidad | Limitada | Alta |

---

## 8. Configuraciones para Bypass

### 8.1 Configuración de RTPEngine

#### MOH (Music on Hold)

```ini
# /etc/rtpengine/rtpengine.conf
[rtpengine]
# ... configuración existente ...
media-dir = /var/lib/rtpengine/media
num-threads = 8
media-buffer = 50
```

#### Estructura de archivos de audio

```
/var/lib/rtpengine/media/
├── moh/
│   ├── default.wav          # MOH por defecto (8kHz mono 16-bit, loop)
│   └── jazz.wav             # MOH alternativo
├── ringback/
│   ├── us-ring.wav          # Ringback tono US
│   ├── uk-ring.wav          # Ringback tono UK
│   └── ar-ring.wav          # Ringback tono Argentina
└── announcements/
    └── please-wait.wav      # "Por favor espere"
```

#### Conversión de archivos

```bash
# Convertir a formato compatible
ffmpeg -i input.mp3 -ar 8000 -ac 1 -acodec pcm_s16le output.wav

# Crear loop sin cortes
sox input.wav output.wav fade 0.5 0 0.5 repeat 10
```

### 8.2 Configuración de Kamailio

#### Defines necesarios

```c
#!define FLT_CALL_ONHOLD 30
#!define FLT_EARLY_MEDIA 31
#!substdef "!MOH_FILE!/var/lib/rtpengine/media/moh/default.wav!g"
#!substdef "!RINGBACK_AR!/var/lib/rtpengine/media/ringback/ar-ring.wav!g"
```

#### Ruta: Detección de HOLD

```c
route[DETECT_HOLD] {
    if (!is_method("INVITE") || !has_totag()) {
        return;
    }

    if (!has_body("application/sdp")) {
        return;
    }

    # Detectar a=sendonly o a=inactive (HOLD)
    if (sdp_with_media("audio") && (
        sdp_content("a=sendonly") ||
        sdp_content("a=inactive")
    )) {
        xlog("L_INFO", "HOLD DETECTADO: CallID=$ci\n");
        setflag(FLT_CALL_ONHOLD);
        $dlg_var(call_state) = "hold";
        return;
    }

    # Detectar c=IN IP4 0.0.0.0 (HOLD legacy)
    if (sdp_content("c=IN IP4 0.0.0.0")) {
        xlog("L_INFO", "HOLD DETECTADO (legacy): CallID=$ci\n");
        setflag(FLT_CALL_ONHOLD);
        $dlg_var(call_state) = "hold";
        return;
    }

    # Detectar a=sendrecv (UNHOLD)
    if (sdp_with_media("audio") && sdp_content("a=sendrecv")) {
        if ($dlg_var(call_state) == "hold") {
            xlog("L_INFO", "UNHOLD DETECTADO: CallID=$ci\n");
            resetflag(FLT_CALL_ONHOLD);
            $dlg_var(call_state) = "active";
        }
    }
}
```

#### Ruta: Iniciar MOH

```c
route[RTPE_MOH_START] {
    xlog("L_INFO", "MOH_START: Activando música de espera - CallID: $ci\n");

    $var(moh_flags) = "replace-origin replace-session-connection ";
    $var(moh_flags) = $var(moh_flags) + "play-media=MOH_FILE ";
    $var(moh_flags) = $var(moh_flags) + "direction=" + $dlg_var(dst_direction) + " ";
    $var(moh_flags) = $var(moh_flags) + "media-handover ";

    if (!rtpengine_offer("$var(moh_flags)")) {
        xlog("L_ERR", "MOH_START: Error activando MOH\n");
        return;
    }

    $dlg_var(moh_active) = "yes";
}
```

#### Ruta: Detener MOH

```c
route[RTPE_MOH_STOP] {
    if ($dlg_var(moh_active) != "yes") {
        return;
    }

    xlog("L_INFO", "MOH_STOP: Desactivando música de espera - CallID: $ci\n");

    $var(stop_flags) = "replace-origin replace-session-connection ";
    $var(stop_flags) = $var(stop_flags) + "stop-media ";
    $var(stop_flags) = $var(stop_flags) + "direction=" + $dlg_var(src_direction) + " ";
    $var(stop_flags) = $var(stop_flags) + "direction=" + $dlg_var(dst_direction) + " ";

    if (!rtpengine_offer("$var(stop_flags)")) {
        xlog("L_ERR", "MOH_STOP: Error desactivando MOH\n");
        return;
    }

    $dlg_var(moh_active) = "no";
}
```

#### Ruta: Manejo de re-INVITEs

```c
route[HANDLE_REINVITE] {
    if (!is_method("INVITE") || !has_totag()) {
        return;
    }

    route(DETECT_HOLD);

    if (isflagset(FLT_CALL_ONHOLD) && $dlg_var(moh_active) != "yes") {
        xlog("L_INFO", "HOLD detectado, activando MOH\n");
        route(RTPE_MOH_START);
    }
    else if (!isflagset(FLT_CALL_ONHOLD) && $dlg_var(moh_active) == "yes") {
        xlog("L_INFO", "UNHOLD detectado, desactivando MOH\n");
        route(RTPE_MOH_STOP);
    }
    else {
        route(RTPENGINEOFFER);
    }
}
```

### 8.3 Flags de RTPEngine

| Flag | Descripción |
|------|-------------|
| `play-media=FILE` | Reproduce archivo de audio |
| `stop-media` | Detiene reproducción |
| `direction=external/internal` | Hacia qué leg enviar |
| `media-handover` | Transfiere control del media |
| `generate-DTMF` | Transcoding inband → RFC 2833 |
| `DTMF-PT=101` | Payload type para telephone-event |
| `DTMF-log` | Log de eventos DTMF |

---

## Resumen de Requisitos para Implementar Bypass

### RTPEngine
- [ ] Compilar con soporte de play-media (ffmpeg/libavcodec)
- [ ] Configurar archivos MOH en `/var/lib/rtpengine/media/`
- [ ] Habilitar DTMF transcoding si hay endpoints con inband

### Kamailio
- [ ] Agregar lógica de detección de HOLD (a=sendonly/inactive)
- [ ] Agregar lógica de detección de UNHOLD (a=sendrecv)
- [ ] Configurar rtpengine_offer() con play-media para MOH
- [ ] Configurar early-media para ringback (opcional)

### FreeSWITCH
- [ ] Configurar bypass_media=true en dialplan (to-kamailio.xml)
- [ ] Asegurar que SDP de Kamailio/RTPEngine pase sin modificación
- [ ] Ajustar CDR para que funcione sin eventos de media

### Testing
- [ ] Probar llamadas entrantes con hold/unhold
- [ ] Probar DTMF en ambas direcciones
- [ ] Probar early media / ringback
- [ ] Probar transferencias y conferencias
- [ ] Verificar calidad de audio

---

## Referencias

- Archivo de configuración RTPEngine: `/etc/rtpengine/rtpengine.conf`
- Archivo de configuración Kamailio: `/etc/kamailio/kamailio.cfg`
- Handler de unified routing: `rust-backend/crates/apolo-api/src/handlers/unified_routing.rs`
- Migración early_media: `database/migrations/009_inbound_early_media.sql`
