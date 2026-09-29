# Curso NOC: SBC y Motor de Billing - ApoloBilling

> **Nivel:** Básico-Intermedio
> **Duración estimada:** 40-50 horas
> **Audiencia:** Personal técnico del NOC
> **Versión:** 1.0 (Agosto 2026)

---

## Tabla de Contenidos

1. [Módulo 1: Fundamentos de VoIP y SIP](#módulo-1-fundamentos-de-voip-y-sip)
2. [Módulo 2: Arquitectura del Sistema ApoloBilling](#módulo-2-arquitectura-del-sistema-apolobilling)
3. [Módulo 3: Kamailio - Session Border Controller](#módulo-3-kamailio---session-border-controller)
4. [Módulo 4: FreeSWITCH - Media Server y PBX](#módulo-4-freeswitch---media-server-y-pbx)
5. [Módulo 5: RTPEngine - Media Relay](#módulo-5-rtpengine---media-relay)
6. [Módulo 6: Motor de Billing en Rust](#módulo-6-motor-de-billing-en-rust)
7. [Módulo 7: PostgreSQL - Base de Datos](#módulo-7-postgresql---base-de-datos)
8. [Módulo 8: Flujos de Señalización](#módulo-8-flujos-de-señalización)
9. [Módulo 9: Flujos de Media (RTP)](#módulo-9-flujos-de-media-rtp)
10. [Módulo 10: Troubleshooting y Diagnóstico](#módulo-10-troubleshooting-y-diagnóstico)
11. [Anexos: Comandos de Referencia Rápida](#anexos-comandos-de-referencia-rápida)

---

# Módulo 1: Fundamentos de VoIP y SIP

## 1.1 Conceptos Básicos de VoIP

### ¿Qué es VoIP?
Voice over IP (VoIP) es la tecnología que permite transmitir voz sobre redes de datos IP en lugar de la red telefónica tradicional (PSTN).

### Componentes Clave

| Componente | Función | Ejemplo en ApoloBilling |
|------------|---------|-------------------------|
| **Softswitch/SBC** | Control de señalización | Kamailio |
| **Media Server** | Procesamiento de audio | FreeSWITCH |
| **Media Relay** | NAT traversal, relay RTP | RTPEngine |
| **Endpoint** | Terminal de usuario | Softphone, IP Phone |
| **Trunk/Carrier** | Conexión a PSTN | INTEGRATEL |

### Planos de Comunicación

```
┌─────────────────────────────────────────────────────────────┐
│                    PLANO DE SEÑALIZACIÓN                     │
│              (SIP - Puerto 5060 UDP/TCP/TLS)                 │
│                                                              │
│    INVITE → 100 Trying → 180 Ringing → 200 OK → ACK         │
└─────────────────────────────────────────────────────────────┘
                              ↕
┌─────────────────────────────────────────────────────────────┐
│                      PLANO DE MEDIA                          │
│               (RTP - Puertos 10000-30000 UDP)                │
│                                                              │
│              Audio bidireccional en tiempo real              │
└─────────────────────────────────────────────────────────────┘
```

## 1.2 Protocolo SIP (Session Initiation Protocol)

### Métodos SIP Principales

| Método | Propósito | Ejemplo |
|--------|-----------|---------|
| **INVITE** | Iniciar llamada | Establecer sesión |
| **ACK** | Confirmar 200 OK | Completar handshake |
| **BYE** | Terminar llamada | Colgar |
| **CANCEL** | Cancelar INVITE pendiente | Cancelar antes de contestar |
| **REGISTER** | Registrar ubicación | Softphone se registra |
| **OPTIONS** | Verificar disponibilidad | Health check |
| **PRACK** | ACK provisional (100rel) | Confirmar 183 |
| **INFO** | Información mid-dialog | DTMF vía SIP |

### Códigos de Respuesta SIP

**Clase 1xx - Provisional (informativas)**
```
100 Trying         → Procesando solicitud
180 Ringing        → Destino está timbrando
183 Session Progress → Early media disponible (con SDP)
```

**Clase 2xx - Éxito**
```
200 OK             → Solicitud exitosa
202 Accepted       → Aceptado para procesamiento
```

**Clase 3xx - Redirección**
```
301 Moved Permanently → Usar nueva ubicación
302 Moved Temporarily → Redirección temporal
```

**Clase 4xx - Error del Cliente**
```
400 Bad Request    → Sintaxis inválida
401 Unauthorized   → Requiere autenticación
403 Forbidden      → Servidor rechaza la solicitud
404 Not Found      → Usuario no encontrado
408 Request Timeout→ Timeout
480 Unavailable    → Usuario temporalmente no disponible
486 Busy Here      → Usuario ocupado
488 Not Acceptable → SDP incompatible (codecs, etc.)
```

**Clase 5xx - Error del Servidor**
```
500 Internal Error → Error interno del servidor
502 Bad Gateway    → Gateway inválido
503 Service Unavail→ Servicio no disponible
504 Timeout        → Timeout de gateway
```

**Clase 6xx - Error Global**
```
600 Busy Everywhere → Ocupado en todas partes
603 Decline         → Usuario rechaza la llamada
```

### Estructura de un Mensaje SIP

```
INVITE sip:17402002@10.10.22.4:5080 SIP/2.0
Via: SIP/2.0/UDP 10.124.194.14:5060;branch=z9hG4bK1234
Max-Forwards: 70
From: <sip:938391839@carrier.com>;tag=abc123
To: <sip:17402002@10.10.22.4>
Call-ID: 123456789@10.124.194.14
CSeq: 1 INVITE
Contact: <sip:938391839@10.124.194.14:5060>
Content-Type: application/sdp
Content-Length: 200

v=0
o=- 123456 123456 IN IP4 10.124.194.142
s=SIP Call
c=IN IP4 10.124.194.142
t=0 0
m=audio 10000 RTP/AVP 8 0 101
a=rtpmap:8 PCMA/8000
a=rtpmap:0 PCMU/8000
a=rtpmap:101 telephone-event/8000
a=fmtp:101 0-16
```

### Headers SIP Importantes

| Header | Función |
|--------|---------|
| **Via** | Ruta de la solicitud (cada hop agrega uno) |
| **From** | Originador de la solicitud |
| **To** | Destino de la solicitud |
| **Call-ID** | Identificador único de la sesión |
| **CSeq** | Número de secuencia del comando |
| **Contact** | Dónde enviar futuras solicitudes |
| **Route/Record-Route** | Ruta forzada a través de proxies |
| **P-Asserted-Identity** | Identidad verificada del caller |
| **P-Early-Media** | Control de early media (RFC 5009) |
| **Supported** | Features soportados (100rel, timer, etc.) |

## 1.3 Protocolo SDP (Session Description Protocol)

### Estructura de SDP

```
v=0                                    ← Versión del protocolo
o=- 123456 123456 IN IP4 10.10.22.4   ← Origen (sesión ID, versión, IP)
s=SIP Call                             ← Nombre de la sesión
c=IN IP4 10.10.22.4                    ← Conexión (IP donde recibir RTP)
t=0 0                                  ← Tiempo (0 0 = permanente)
m=audio 16384 RTP/AVP 8 0 101          ← Media: tipo, puerto, perfil, codecs
a=rtpmap:8 PCMA/8000                   ← Codec 8 = G.711 A-law
a=rtpmap:0 PCMU/8000                   ← Codec 0 = G.711 μ-law
a=rtpmap:101 telephone-event/8000      ← DTMF RFC 2833
a=fmtp:101 0-16                        ← DTMF eventos 0-16 (dígitos + flash)
a=sendrecv                             ← Dirección del media
```

### Atributos de Dirección (HOLD)

| Atributo | Significado | Uso |
|----------|-------------|-----|
| `a=sendrecv` | Envía y recibe | Llamada normal |
| `a=sendonly` | Solo envía | Hold (el que pone en hold) |
| `a=recvonly` | Solo recibe | Hold (el que está en hold) |
| `a=inactive` | Nada | Hold completo |

### Codecs Comunes

| Payload | Codec | Bitrate | Uso |
|---------|-------|---------|-----|
| 0 | PCMU (G.711 μ-law) | 64 kbps | US/Japón |
| 8 | PCMA (G.711 A-law) | 64 kbps | Europa/Latam |
| 9 | G.722 | 64 kbps | HD Voice |
| 18 | G.729 | 8 kbps | Bajo ancho de banda |
| 101 | telephone-event | variable | DTMF RFC 2833 |

## 1.4 Protocolo RTP (Real-time Transport Protocol)

### Estructura del Paquete RTP

```
 0                   1                   2                   3
 0 1 2 3 4 5 6 7 8 9 0 1 2 3 4 5 6 7 8 9 0 1 2 3 4 5 6 7 8 9 0 1
+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
|V=2|P|X|  CC   |M|     PT      |       Sequence Number          |
+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
|                           Timestamp                            |
+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
|                              SSRC                              |
+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+
|                            Payload                             |
+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+-+

V   = Versión (2)
P   = Padding
X   = Extension
CC  = CSRC count
M   = Marker (inicio de talkspurt)
PT  = Payload Type (codec)
```

### Conceptos Clave de RTP

| Término | Definición |
|---------|------------|
| **SSRC** | Synchronization Source - identifica el stream |
| **Timestamp** | Tiempo de muestreo (8000 Hz para G.711) |
| **Sequence** | Número de secuencia para detectar pérdida |
| **Jitter** | Variación en el tiempo de llegada |
| **Latency** | Retraso end-to-end |
| **Packet Loss** | Paquetes perdidos en tránsito |

---

# Módulo 2: Arquitectura del Sistema ApoloBilling

## 2.1 Diagrama de Arquitectura General

```
┌─────────────────────────────────────────────────────────────────────────────────┐
│                              INTERNET / PSTN                                    │
│                                                                                 │
│                     Carrier INTEGRATEL (10.35.13.100)                          │
│                                    │                                            │
│                           SIP/RTP ◄┼────►                                       │
│                                    │                                            │
└────────────────────────────────────┼────────────────────────────────────────────┘
                                     │
┌────────────────────────────────────┼────────────────────────────────────────────┐
│                          SERVIDOR SBC (10.10.22.4)                              │
│                                    │                                            │
│  ┌─────────────────────────────────▼─────────────────────────────────────────┐  │
│  │                         KAMAILIO :5060                                    │  │
│  │                    (Session Border Controller)                            │  │
│  │                                                                           │  │
│  │   • Señalización SIP            • drouting (enrutamiento dinámico)       │  │
│  │   • Autenticación               • Topology Hiding                         │  │
│  │   • ACL por IP                  • 100rel/PRACK                           │  │
│  │   • TLS/WebSocket               • Integración RTPEngine                   │  │
│  └───────────────────────────────────────────────────────────────────────────┘  │
│                                    │                                            │
│                          UDP:7722 (control)                                     │
│                                    │                                            │
│  ┌─────────────────────────────────▼─────────────────────────────────────────┐  │
│  │                         RTPENGINE                                         │  │
│  │                                                                           │  │
│  │   internal/10.10.22.4    ◄═══════════════►    external/10.124.194.142    │  │
│  │   (hacia FreeSWITCH)              RTP         (hacia carriers/Internet)   │  │
│  │                                                                           │  │
│  │   • NAT traversal               • Codec passthrough                       │  │
│  │   • Topology hiding             • Symmetric RTP                           │  │
│  └───────────────────────────────────────────────────────────────────────────┘  │
│                                    │                                            │
│                               SIP (UDP)                                         │
│                                    │                                            │
│  ┌─────────────────────────────────▼─────────────────────────────────────────┐  │
│  │                         FREESWITCH                                        │  │
│  │                                                                           │  │
│  │   Internal :5080                 External :5062              Inbound :5082│  │
│  │   (from-pbx)                     (to-kamailio)              (de carriers) │  │
│  │                                                                           │  │
│  │   • IVR/Dialplan                • Billing ESL                             │  │
│  │   • Early media                 • mod_xml_curl                            │  │
│  └───────────────────────────────────────────────────────────────────────────┘  │
│                                    │                                            │
│                               ESL :8021                                         │
│                                    │                                            │
│  ┌─────────────────────────────────▼─────────────────────────────────────────┐  │
│  │                    RUST BILLING ENGINE :9000                              │  │
│  │                                                                           │  │
│  │   • Autorización de llamadas    • Reservas de balance                     │  │
│  │   • Billing en tiempo real      • Generación de CDRs                      │  │
│  │   • LPM (Longest Prefix Match)  • Extensión de reservas                   │  │
│  └───────────────────────────────────────────────────────────────────────────┘  │
│                                    │                                            │
│  ┌────────────────┬────────────────┴────────────────┬────────────────────────┐  │
│  │                │                                 │                        │  │
│  ▼                ▼                                 ▼                        │  │
│ PostgreSQL    Redis :6379                     MySQL :3306                    │  │
│  :5432        (cache)                         (Kamailio)                     │  │
│ (billing)                                                                    │  │
└─────────────────────────────────────────────────────────────────────────────────┘
```

## 2.2 Interfaces de Red

| Interface | IP | Función |
|-----------|-----|---------|
| **ens18** | 10.10.22.4/28 | Internal - LAN, señalización local |
| **ens19** | 10.124.194.14/30 | External SIP - señalización hacia carriers |
| **ens20** | 10.124.194.142/30 | External RTP - media hacia carriers |

## 2.3 Puertos del Sistema

| Puerto | Protocolo | Servicio | Descripción |
|--------|-----------|----------|-------------|
| 5060 | UDP/TCP | Kamailio | SIP signaling (SBC) |
| 5080 | UDP/TCP | FreeSWITCH | Internal profile (softphones) |
| 5062 | UDP/TCP | FreeSWITCH | External profile (hacia Kamailio) |
| 5082 | UDP/TCP | FreeSWITCH | Inbound profile (de carriers) |
| 7722 | UDP | RTPEngine | Control socket (Kamailio → RTPEngine) |
| 9900 | TCP | RTPEngine | CLI para diagnóstico |
| 8021 | TCP | FreeSWITCH | Event Socket Layer (ESL) |
| 8000 | TCP | Rust Backend | API REST |
| 9000 | TCP | Billing Engine | API de billing |
| 10000-30000 | UDP | RTPEngine | Puertos de media RTP |
| 5432 | TCP | PostgreSQL | Base de datos billing |
| 3306 | TCP | MySQL | Base de datos Kamailio |
| 6379 | TCP | Redis | Cache |

## 2.4 Servicios Systemd

```bash
# Ver estado de todos los servicios
systemctl status kamailio freeswitch rtpengine \
    apolo-backend apolo-billing-engine \
    postgresql redis mysql

# Comandos comunes
systemctl start|stop|restart|status <servicio>
journalctl -u <servicio> -f    # Logs en tiempo real
```

---

# Módulo 3: Kamailio - Session Border Controller

## 3.1 ¿Qué es Kamailio?

Kamailio es un servidor SIP de clase carrier que actúa como:
- **SBC (Session Border Controller)**: Control de acceso entre redes
- **Proxy SIP**: Enrutamiento de mensajes SIP
- **Registrar**: Almacena ubicación de usuarios
- **Load Balancer**: Distribución de carga

### Ubicación en la Arquitectura

```
INTERNET ─────► KAMAILIO:5060 ─────► FREESWITCH:5080/5082
                    │
                    ▼
               RTPENGINE:7722
```

## 3.2 Configuración Principal

**Archivo:** `/etc/kamailio/kamailio.cfg`

### Defines (Features Habilitados)

```c
#!define WITH_MYSQL          // Base de datos MySQL
#!define WITH_AUTH           // Autenticación SIP
#!define WITH_DROUTE         // Dynamic Routing
#!define WITH_RTPENGINE      // Media relay
#!define WITH_TLS            // Encriptación
#!define WITH_WEBSOCKETS     // WebRTC
```

### Flags del Sistema

Los flags son bits que marcan el estado de una transacción/diálogo:

```c
// Transaction Flags (por transacción)
#!define FLT_ACC           1   // Accounting habilitado
#!define FLT_ACCFAILED     2   // Log de fallos
#!define FLT_NATS          5   // NAT detectado
#!define FLT_PBX_AUTH      20  // Autenticado como PBX
#!define FLT_INTEGRATEL_CONGESTION 24  // Tono de congestión

// Branch Flags (por rama/destino)
#!define FLB_SRC_CARRIER   5   // Origen es carrier
#!define FLB_DST_CARRIER   6   // Destino es carrier
#!define FLB_SRC_PBX       7   // Origen es PBX

// Dialog Flags (por diálogo/llamada)
#!define FLD_TOPO_HIDING   30  // Topology hiding activo
```

### Interfaces de Escucha

```c
listen=udp:10.10.22.4:5060         // LAN
listen=udp:10.124.194.14:5060      // WAN
```

## 3.3 Módulo drouting (Dynamic Routing)

drouting permite enrutar llamadas basándose en prefijos configurados en base de datos.

### Tablas MySQL

**dr_gateways** - Gateways/Carriers
```sql
SELECT gwid, type, address, strip, description FROM dr_gateways;
+------+------+---------------------+-------+------------------------+
| gwid | type | address             | strip | description            |
+------+------+---------------------+-------+------------------------+
|   60 |    8 | 10.35.13.100:5060   |     0 | INTEGRATEL Principal   |
|   59 |    8 | sip.fibertel.com    |     0 | FIBERTEL               |
|  100 |    9 | 10.10.22.4:5082     |     0 | FreeSWITCH PBX Local   |
+------+------+---------------------+-------+------------------------+

-- type=8: Carrier externo
-- type=9: PBX interno
```

**dr_gw_lists** - Grupos de gateways (failover)
```sql
SELECT id, gwlist, description FROM dr_gw_lists;
+-----+--------+--------------------+
| id  | gwlist | description        |
+-----+--------+--------------------+
|  11 | 60     | Integratel         |
|  12 | 59     | FIBERTEL           |
| 100 | 100    | FreeSWITCH Local   |
+-----+--------+--------------------+
```

**dr_rules** - Reglas de enrutamiento
```sql
SELECT ruleid, groupid, prefix, gwlist FROM dr_rules;
+---------+---------+--------+--------+
| ruleid  | groupid | prefix | gwlist |
+---------+---------+--------+--------+
|       1 |    8000 | 2067   | 60     |  -- ENTEL → Integratel
|       2 |    8000 | 2167   | 60     |  -- CLARO → Integratel
|       3 |    8000 | 2267   | 60     |  -- MOVISTAR → Integratel
|       4 |    9000 |        | #100   |  -- Inbound → FreeSWITCH
+---------+---------+--------+--------+

-- groupid=8000: Llamadas salientes
-- groupid=9000: Llamadas entrantes
```

### Comandos de Recarga

```bash
# Recargar reglas de drouting
kamcmd drouting.reload

# Ver dump de reglas en memoria
kamcmd drouting.dump

# Recargar tablas de permisos (ACL)
kamcmd permissions.addressReload

# Recargar htable
kamcmd htable.reload gw2gwgroup
```

## 3.4 Integración con RTPEngine

### Configuración

```c
loadmodule "rtpengine.so"
modparam("rtpengine", "rtpengine_sock", "udp:localhost:7722")
```

### Flags de RTPEngine

| Flag | Descripción |
|------|-------------|
| `replace-origin` | Reemplaza IP de origen en SDP |
| `replace-session-connection` | Reemplaza c= en SDP |
| `direction=X` | Interfaz a usar (internal/external) |
| `symmetric` | RTP simétrico (NAT) |
| `port-latching` | Latching de puerto |
| `ICE=remove` | Eliminar candidatos ICE |

### Rutas de RTPEngine

```c
route[RTPENGINEOFFER] {
    // Llamada entrante: external → internal
    $var(flags) = "replace-origin replace-session-connection ";
    $var(flags) = $var(flags) + "direction=external direction=internal ";
    $var(flags) = $var(flags) + "symmetric port-latching ICE=remove";

    rtpengine_offer("$var(flags)");
}

route[RTPENGINEANSWER] {
    // Respuesta: internal → external
    $var(flags) = "replace-origin replace-session-connection ";
    $var(flags) = $var(flags) + "direction=internal direction=external ";
    $var(flags) = $var(flags) + "symmetric port-latching";

    rtpengine_answer("$var(flags)");
}
```

## 3.5 Topology Hiding

Topology hiding oculta información interna (IPs, hostnames) de la red externa.

### Mecanismos

1. **Via header stripping**: Eliminar Vias internas
2. **Record-Route encryption**: Encriptar rutas
3. **Contact rewriting**: Reescribir Contact hacia IP pública
4. **SDP IP replacement**: RTPEngine reemplaza IPs en SDP

### Implementación

```c
# En respuestas hacia carriers
if (isbflagset(FLB_DST_CARRIER)) {
    # Reescribir Contact
    subst_uri("/^sip:(.*)@10\.10\.22\./sip:\1@10.124.194.14/");

    # RTPEngine ya reescribió las IPs del SDP
}
```

## 3.6 100rel y PRACK (RFC 3262)

### ¿Qué es 100rel?

100rel permite respuestas provisionales confiables (183 Session Progress con SDP).

### Flujo Normal vs 100rel

**Sin 100rel:**
```
INVITE → 180 Ringing → 200 OK → ACK
         (no confirmado)
```

**Con 100rel:**
```
INVITE → 183 Session Progress → PRACK → 200 OK (PRACK) → 200 OK → ACK
         (SDP + RSeq)           (RAck)
```

### Por qué lo necesitamos

INTEGRATEL es un carrier IMS que requiere:
1. Early media con SDP en el 183
2. Confirmación vía PRACK
3. P-Early-Media header (RFC 5009)

### Configuración en Kamailio

```c
// En INVITE saliente hacia carrier
if (isbflagset(FLB_DST_CARRIER)) {
    // Agregar soporte 100rel
    if (!is_present_hf("Supported")) {
        append_hf("Supported: 100rel\r\n");
    }

    // Agregar PRACK al Allow
    if (!search("^Allow:.*PRACK")) {
        remove_hf("Allow");
        append_hf("Allow: INVITE, ACK, PRACK, OPTIONS, BYE, CANCEL\r\n");
    }

    // P-Early-Media
    if (!is_present_hf("P-Early-Media")) {
        append_hf("P-Early-Media: supported\r\n");
    }
}
```

## 3.7 Ejercicios Prácticos

### Ejercicio 3.1: Ver reglas de drouting
```bash
# Conectar a MySQL
mysql -u kamailio -p kamailio

# Ver gateways
SELECT gwid, type, address, description FROM dr_gateways;

# Ver reglas activas
SELECT ruleid, groupid, prefix, gwlist FROM dr_rules ORDER BY groupid, prefix;
```

### Ejercicio 3.2: Agregar una nueva ruta
```sql
-- Agregar ruta para prefijo 51 (Perú) al carrier FIBERTEL
INSERT INTO dr_rules (groupid, prefix, gwlist, description)
VALUES (8000, '51', '59', 'Peru via FIBERTEL');

-- Recargar
-- En bash: kamcmd drouting.reload
```

### Ejercicio 3.3: Debug de Kamailio
```bash
# Subir nivel de debug
kamcmd debug.level 3

# Ver logs en tiempo real
journalctl -u kamailio -f

# Bajar debug al terminar
kamcmd debug.level 1
```

---

# Módulo 4: FreeSWITCH - Media Server y PBX

## 4.1 ¿Qué es FreeSWITCH?

FreeSWITCH es un softswitch/PBX que maneja:
- **B2BUA**: Back-to-Back User Agent (une dos llamadas)
- **Media processing**: IVR, conferencias, grabación
- **Dialplan**: Lógica de enrutamiento de llamadas
- **ESL**: Event Socket Layer para integración externa

## 4.2 Perfiles SIP

### Internal Profile (:5080)

**Archivo:** `/etc/freeswitch/sip_profiles/internal.xml`

```xml
<profile name="internal">
  <settings>
    <param name="sip-ip" value="10.10.22.4"/>
    <param name="sip-port" value="5080"/>
    <param name="context" value="from-pbx"/>

    <!-- Autenticación requerida -->
    <param name="auth-calls" value="true"/>

    <!-- Media -->
    <param name="inbound-bypass-media" value="false"/>
    <param name="rtp-ip" value="10.10.22.4"/>
  </settings>
</profile>
```

**Uso:** Recibe llamadas de softphones/extensiones.

### External Profile (:5062)

**Archivo:** `/etc/freeswitch/sip_profiles/external.xml`

```xml
<profile name="external">
  <settings>
    <param name="sip-ip" value="10.10.22.4"/>
    <param name="sip-port" value="5062"/>
    <param name="context" value="to-kamailio"/>

    <!-- 100rel para carriers IMS -->
    <param name="enable-100rel" value="true"/>
    <param name="auto-answer-100rel" value="true"/>

    <!-- Sin autenticación (Kamailio ya autenticó) -->
    <param name="auth-calls" value="false"/>
  </settings>
</profile>
```

**Uso:** Comunicación con Kamailio para llamadas salientes.

### Inbound Profile (:5082)

**Archivo:** `/etc/freeswitch/sip_profiles/inbound.xml`

```xml
<profile name="inbound">
  <settings>
    <param name="sip-ip" value="10.10.22.4"/>
    <param name="sip-port" value="5082"/>
    <param name="context" value="to-kamailio"/>

    <!-- 100rel para INTEGRATEL (IMS) -->
    <param name="enable-100rel" value="true"/>

    <!-- FreeSWITCH genera early media -->
    <param name="inbound-bypass-media" value="false"/>
  </settings>
</profile>
```

**Uso:** Recibe llamadas entrantes desde Kamailio/carriers.

## 4.3 Dialplan XML

### Estructura del Dialplan

```
/etc/freeswitch/dialplan/
├── default.xml          # Contexto por defecto
├── from-pbx.xml         # Llamadas desde softphones
├── to-kamailio.xml      # Llamadas desde Kamailio
└── public.xml           # Contexto público
```

### Contexto from-pbx (Llamadas Salientes)

**Archivo:** `/etc/freeswitch/dialplan/from-pbx.xml`

```xml
<context name="from-pbx">
  <!-- Llamadas salientes a través de Kamailio -->
  <extension name="outbound-to-kamailio">
    <condition field="destination_number" expression="^(\d{10,})$">
      <action application="log" data="INFO Outbound: ${caller_id_number} → $1"/>

      <!-- Billing: heredar codec, bypass media para RTPEngine -->
      <action application="set" data="inherit_codec=true"/>
      <action application="set" data="bypass_media=true"/>

      <!-- Bridge a Kamailio -->
      <action application="bridge"
              data="sofia/external/$1@10.10.22.4:5060"/>
    </condition>
  </extension>

  <!-- Extensiones locales -->
  <extension name="local-extension">
    <condition field="destination_number" expression="^(1\d{3})$">
      <action application="bridge" data="user/$1@10.10.22.4"/>
    </condition>
  </extension>
</context>
```

### Contexto to-kamailio (Llamadas Entrantes)

**Archivo:** `/etc/freeswitch/dialplan/to-kamailio.xml`

```xml
<context name="to-kamailio">
  <!-- Llamadas entrantes de INTEGRATEL (prefijo 6722) -->
  <extension name="from-integratel-rn-22">
    <condition field="destination_number" expression="^(6722.+)$">
      <action application="log" data="INFO Inbound INTEGRATEL: $1"/>

      <!-- Transformar número: 672217402002 → 17402002 -->
      <action application="set" data="transformed_number=${destination_number:4}"/>

      <!-- Timeouts -->
      <action application="set" data="call_timeout=55"/>
      <action application="set" data="originate_timeout=55"/>

      <!-- 100rel para IMS -->
      <action application="set" data="sip_enable_100rel=true"/>
      <action application="export" data="sip_enable_100rel=true"/>

      <!-- Early media: FreeSWITCH genera ringback -->
      <action application="set" data="bypass_media=false"/>
      <action application="set" data="ringback=${us-ring}"/>
      <action application="pre_answer"/>

      <!-- Continuar después del bridge para manejar timeout -->
      <action application="set" data="hangup_after_bridge=false"/>
      <action application="set" data="continue_on_fail=true"/>

      <!-- Bridge al usuario -->
      <action application="bridge" data="user/${transformed_number}@10.10.22.4"/>

      <!-- Si timeout/no respuesta: tono de congestión -->
      <action application="log" data="WARNING Bridge failed: ${originate_disposition}"/>
      <action application="answer"/>
      <action application="sleep" data="200"/>
      <!-- 7 segundos de tono de congestión -->
      <action application="playback" data="tone_stream://%(250,250,480,620);loops=14"/>
      <action application="hangup" data="NO_USER_RESPONSE"/>
    </condition>
  </extension>
</context>
```

## 4.4 Aplicaciones del Dialplan

### Aplicaciones Comunes

| Aplicación | Función | Ejemplo |
|------------|---------|---------|
| `bridge` | Conectar a otro endpoint | `bridge data="user/1001"` |
| `answer` | Contestar la llamada | Envía 200 OK |
| `pre_answer` | Early media (183) | Envía 183 con SDP |
| `playback` | Reproducir audio | `playback data="ivr/menu.wav"` |
| `hangup` | Colgar | `hangup data="NORMAL_CLEARING"` |
| `set` | Establecer variable | `set data="var=value"` |
| `log` | Escribir al log | `log data="INFO mensaje"` |
| `sleep` | Esperar ms | `sleep data="1000"` |
| `respond` | Enviar respuesta SIP | `respond data="486 Busy"` |
| `transfer` | Transferir a extensión | `transfer data="1001 XML default"` |

### Variables Importantes

| Variable | Descripción |
|----------|-------------|
| `${caller_id_number}` | Número del caller |
| `${destination_number}` | Número destino |
| `${originate_disposition}` | Resultado del bridge |
| `${bridge_hangup_cause}` | Causa de colgado |
| `${sip_contact_uri}` | Contact del registro |
| `${network_addr}` | IP de origen |

### Causas de Hangup (Q.850)

| Código | Nombre | Descripción |
|--------|--------|-------------|
| 16 | NORMAL_CLEARING | Colgado normal |
| 17 | USER_BUSY | Usuario ocupado |
| 18 | NO_USER_RESPONSE | Sin respuesta (timeout) |
| 19 | NO_ANSWER | No contestó |
| 20 | SUBSCRIBER_ABSENT | Usuario no disponible |
| 21 | CALL_REJECTED | Llamada rechazada |
| 31 | NORMAL_UNSPECIFIED | Normal no especificado |
| 34 | NO_CIRCUIT_AVAILABLE | Sin circuitos |
| 42 | SWITCHING_CONGESTION | Congestión |

## 4.5 mod_xml_curl (Directorio Dinámico)

mod_xml_curl permite autenticar usuarios consultando un endpoint HTTP.

### Flujo

```
REGISTER ──► FreeSWITCH ──► HTTP POST /api/v1/freeswitch/directory
                                         │
                                         ▼
                                   Rust Backend
                                         │
                                         ▼
                                   PostgreSQL
                                   (sip_devices)
                                         │
                                         ▼
                             XML Response con credenciales
```

### Configuración

**Archivo:** `/etc/freeswitch/autoload_configs/xml_curl.conf.xml`

```xml
<configuration name="xml_curl.conf" description="xml_curl">
  <bindings>
    <binding name="directory">
      <param name="gateway-url"
             value="http://127.0.0.1:8000/api/v1/freeswitch/directory"/>
      <param name="method" value="POST"/>
    </binding>
  </bindings>
</configuration>
```

### Respuesta del Backend

```xml
<?xml version="1.0" encoding="UTF-8"?>
<document type="freeswitch/xml">
  <section name="directory">
    <domain name="10.10.22.4">
      <user id="1001">
        <params>
          <param name="a1-hash" value="abc123def456..."/>
          <param name="dial-string"
                 value="{^^:sip_invite_domain=${dialed_domain}:
                        presence_id=${dialed_user}@${dialed_domain}}
                        ${sofia_contact(*/1001@10.10.22.4)}"/>
        </params>
        <variables>
          <variable name="toll_allow" value="domestic,international"/>
          <variable name="effective_caller_id_name" value="Usuario 1001"/>
          <variable name="effective_caller_id_number" value="1001"/>
        </variables>
      </user>
    </domain>
  </section>
</document>
```

## 4.6 Comandos FreeSWITCH

### fs_cli - Consola Interactiva

```bash
# Conectar a la consola
fs_cli

# Ver estado de perfiles SIP
fs_cli -x "sofia status"
fs_cli -x "sofia status profile internal"

# Ver llamadas activas
fs_cli -x "show calls"
fs_cli -x "show channels"

# Ver registros
fs_cli -x "sofia status profile internal reg"

# Debug SIP (cuidado: muy verboso)
fs_cli -x "sofia global siptrace on"
fs_cli -x "sofia global siptrace off"

# Originar llamada de prueba
fs_cli -x "originate user/1001 &echo"

# Colgar todas las llamadas
fs_cli -x "hupall"

# Recargar configuración
fs_cli -x "reloadxml"
```

### Logs de FreeSWITCH

```bash
# Ver log en tiempo real
tail -f /var/log/freeswitch/freeswitch.log

# Filtrar por INTEGRATEL
tail -f /var/log/freeswitch/freeswitch.log | grep -i integratel

# Ver errores
grep -i error /var/log/freeswitch/freeswitch.log | tail -50
```

## 4.7 Ejercicios Prácticos

### Ejercicio 4.1: Ver estado de FreeSWITCH
```bash
# Estado general
fs_cli -x "sofia status"

# Registros en el perfil internal
fs_cli -x "sofia status profile internal reg"

# Llamadas activas
fs_cli -x "show calls"
```

### Ejercicio 4.2: Simular una llamada
```bash
# Originar llamada de test (reproduce echo)
fs_cli -x "originate user/1001 &echo"

# Originar llamada a extensión
fs_cli -x "originate user/1001 &bridge(user/1002)"
```

### Ejercicio 4.3: Debug de SIP
```bash
# Activar trace SIP
fs_cli -x "sofia global siptrace on"

# Hacer una llamada de prueba
# Ver logs
tail -f /var/log/freeswitch/freeswitch.log | grep -E "INVITE|200|ACK"

# Desactivar trace
fs_cli -x "sofia global siptrace off"
```

---

# Módulo 5: RTPEngine - Media Relay

## 5.1 ¿Qué es RTPEngine?

RTPEngine es un proxy de media RTP que:
- Relaya paquetes RTP entre endpoints
- Permite NAT traversal
- Oculta topología (topology hiding)
- Puede transcodificar codecs
- Soporta DTLS-SRTP para WebRTC

## 5.2 Arquitectura de Interfaces Duales

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

### Por qué interfaces duales

1. **Seguridad**: La IP interna nunca se expone a Internet
2. **NAT traversal**: RTPEngine maneja la traducción
3. **Topology hiding**: Los carriers solo ven la IP externa
4. **Symmetric RTP**: Permite atravesar firewalls

## 5.3 Configuración

**Archivo:** `/etc/rtpengine/rtpengine.conf`

```ini
[rtpengine]
# Sin kernel module (userspace only)
table = -1

# Interfaces duales
interface = internal/10.10.22.4;external/10.124.194.142

# Socket de control (Kamailio se conecta aquí)
listen-ng = 127.0.0.1:7722

# CLI para diagnóstico
listen-cli = 127.0.0.1:9900

# Rango de puertos RTP
port-min = 10000
port-max = 30000

# Threads
num-threads = 8
media-num-threads = 4

# Timeouts
timeout = 60           # Sesión inactiva
silent-timeout = 3600  # Sin audio (1 hora)
offer-timeout = 300    # SDP sin answer

# Logging
log-level = 6
```

## 5.4 Flujo de Media

### Llamada Entrante (Carrier → Extensión)

```
┌────────────┐         ┌─────────────────┐         ┌─────────────┐         ┌──────────┐
│  CARRIER   │         │   RTPENGINE     │         │ FREESWITCH  │         │ SOFTPHONE│
│10.35.13.100│         │  10.10.22.4     │         │ 10.10.22.4  │         │192.168.x │
└─────┬──────┘         └────────┬────────┘         └──────┬──────┘         └────┬─────┘
      │                         │                         │                      │
      │ RTP :10000-30000        │                         │                      │
      │ ◄─────────────────────► │                         │                      │
      │  (Leg A - External)     │                         │                      │
      │  IP: 10.124.194.142     │                         │                      │
      │                         │                         │                      │
      │                         │ RTP :16384-32767        │                      │
      │                         │ ◄─────────────────────► │                      │
      │                         │  (Leg B - Internal)     │                      │
      │                         │  IP: 10.10.22.4         │                      │
      │                         │                         │                      │
      │                         │                         │ RTP                  │
      │                         │                         │ ◄──────────────────► │
      │                         │                         │  (FS ↔ Softphone)    │
```

### SDP Transformation

**SDP Original (del carrier):**
```
v=0
o=- 123 123 IN IP4 10.35.13.100
c=IN IP4 10.35.13.100
m=audio 20000 RTP/AVP 8 0
```

**SDP después de rtpengine_offer() (hacia FreeSWITCH):**
```
v=0
o=- 123 123 IN IP4 10.10.22.4
c=IN IP4 10.10.22.4
m=audio 15000 RTP/AVP 8 0
```

**SDP de respuesta (de FreeSWITCH):**
```
v=0
o=- 456 456 IN IP4 10.10.22.4
c=IN IP4 10.10.22.4
m=audio 16384 RTP/AVP 8 0
```

**SDP después de rtpengine_answer() (hacia carrier):**
```
v=0
o=- 456 456 IN IP4 10.124.194.142
c=IN IP4 10.124.194.142
m=audio 12000 RTP/AVP 8 0
```

## 5.5 Comandos de Diagnóstico

### rtpengine-ctl

```bash
# Ver sesiones activas
rtpengine-ctl list sessions

# Ver estadísticas
rtpengine-ctl list totals

# Ver detalles de una sesión (por Call-ID)
rtpengine-ctl query <call-id>

# Ver todos los comandos
rtpengine-ctl help
```

### Ejemplo de Salida

```bash
$ rtpengine-ctl list sessions

local 10.10.22.4:15432, remote 192.168.1.50:16000, relay 10.10.22.4:10500
	packets: 1234 bytes: 123400 errors: 0
	last packet: 0.5s ago
	codec: PCMA

local 10.124.194.142:10500, remote 10.35.13.100:20000
	packets: 1234 bytes: 123400 errors: 0
	last packet: 0.5s ago
	codec: PCMA
```

### Logs de RTPEngine

```bash
# Ver logs
journalctl -u rtpengine -f

# O si usa syslog
tail -f /var/log/syslog | grep rtpengine
```

## 5.6 Flags Comunes

| Flag | Descripción |
|------|-------------|
| `replace-origin` | Reemplaza o= en SDP |
| `replace-session-connection` | Reemplaza c= en SDP |
| `direction=X` | Usa interfaz X (internal/external) |
| `symmetric` | RTP simétrico |
| `port-latching` | Detecta puerto del remote |
| `ICE=remove` | Elimina candidatos ICE |
| `DTLS=off` | Deshabilita DTLS |
| `SDES=off` | Deshabilita SDES |
| `codec-strip=X` | Elimina codec X |
| `codec-mask=X` | Solo permite codec X |
| `transcode=X` | Transcodifica a codec X |

## 5.7 Ejercicios Prácticos

### Ejercicio 5.1: Ver sesiones RTP
```bash
# Durante una llamada activa
rtpengine-ctl list sessions

# Ver estadísticas totales
rtpengine-ctl list totals
```

### Ejercicio 5.2: Debug de RTPEngine
```bash
# Ver logs en tiempo real
journalctl -u rtpengine -f

# Hacer una llamada y observar:
# - Creación de sesión
# - Puertos asignados
# - Estadísticas de paquetes
```

### Ejercicio 5.3: Reiniciar RTPEngine
```bash
# Ver estado
systemctl status rtpengine

# Reiniciar (CUIDADO: corta llamadas activas)
systemctl restart rtpengine

# Verificar
rtpengine-ctl list totals
```

---

# Módulo 6: Motor de Billing en Rust

## 6.1 Arquitectura del Billing Engine

```
┌─────────────────────────────────────────────────────────────────┐
│                   RUST BILLING ENGINE (:9000)                    │
│                                                                  │
│  ┌────────────────┐  ┌────────────────┐  ┌────────────────────┐ │
│  │ Authorization  │  │ RealtimeBiller │  │   CDR Generator    │ │
│  │    Service     │  │                │  │                    │ │
│  │                │  │                │  │                    │ │
│  │ - LPM lookup   │  │ - Monitor cada │  │ - Calcula costo   │ │
│  │ - Reserva      │  │   180 segundos │  │ - Crea CDR        │ │
│  │   balance      │  │ - Extiende     │  │ - Libera reserva  │ │
│  │ - Valida       │  │   reservas     │  │ - Registra        │ │
│  │   cuenta       │  │                │  │   transacción     │ │
│  └───────┬────────┘  └───────┬────────┘  └──────────┬─────────┘ │
│          │                   │                      │           │
│          ▼                   ▼                      ▼           │
│  ┌──────────────────────────────────────────────────────────┐  │
│  │                   ESL Event Handler                       │  │
│  │                                                           │  │
│  │  CHANNEL_CREATE    CHANNEL_ANSWER    CHANNEL_HANGUP       │  │
│  │       │                  │                 │              │  │
│  │       ▼                  ▼                 ▼              │  │
│  │   Authorize()      StartBilling()     GenerateCDR()       │  │
│  └──────────────────────────────────────────────────────────┘  │
│                              │                                  │
│                              ▼                                  │
│                    FreeSWITCH ESL :8021                        │
└─────────────────────────────────────────────────────────────────┘
```

## 6.2 Flujo de Billing

### Evento CHANNEL_CREATE

```rust
// 1. Recibe evento CHANNEL_CREATE
// 2. Determina si es outbound/inbound
// 3. Busca cuenta por ANI (caller) o DNIS (toll-free)
// 4. Busca tarifa por LPM
// 5. Calcula reserva inicial
// 6. Crea reserva en DB
// 7. Retorna max_duration al canal

pub async fn authorize(&self, req: &AuthRequest) -> Result<AuthResponse, BillingError> {
    // Verificar si autorización está habilitada
    if !self.is_outbound_auth_required().await {
        return Ok(AuthResponse::authorized_no_billing());
    }

    // Buscar cuenta
    let account = self.find_account_by_ani(&req.caller).await?;

    // Buscar tarifa (LPM)
    let rate = self.get_rate(&req.callee).await?;

    // Crear reserva
    let reservation = self.reservation_mgr
        .create_reservation(account.id, &call_uuid, &req.callee, rate)
        .await?;

    Ok(AuthResponse {
        authorized: true,
        reservation_id: Some(reservation.id),
        max_duration_seconds: Some(reservation.max_duration),
        rate_per_minute: Some(rate.rate_per_minute),
    })
}
```

### Longest Prefix Match (LPM)

```rust
// Para destino: 541156789012
// Genera prefijos: ["5", "54", "541", "5411", "54115", ...]
// Busca en rate_cards el prefijo más largo que matchea

async fn get_rate(&self, destination: &str) -> Result<Option<RateCard>> {
    let normalized = destination.replace('+', "");

    // Generar todos los prefijos posibles
    let mut prefixes: Vec<String> = Vec::new();
    for i in (1..=normalized.len()).rev() {
        prefixes.push(normalized[..i].to_string());
    }

    // Query: el más largo que matchea
    let row = client.query_opt(
        "SELECT * FROM rate_cards
         WHERE destination_prefix = ANY($1)
         AND effective_start <= NOW()
         AND (effective_end IS NULL OR effective_end >= NOW())
         ORDER BY LENGTH(destination_prefix) DESC, priority DESC
         LIMIT 1",
        &[&prefixes],
    ).await?;

    // ...
}
```

### Cálculo de Reserva

```rust
// Fórmula:
// base = rate_per_minute × 5 minutos
// buffer = base × 8%
// total = clamp(base + buffer, $0.30, $30.00)
// max_duration = (total / rate_per_minute) × 60 segundos

fn calculate_reservation(rate: Decimal) -> (Decimal, u32) {
    let base = rate * Decimal::from(5);           // 5 minutos
    let buffer = base * Decimal::new(8, 2);       // 8%
    let mut total = base + buffer;

    // Clamp entre $0.30 y $30.00
    total = total.max(Decimal::new(30, 2));       // min $0.30
    total = total.min(Decimal::from(30));         // max $30.00

    // Calcular duración máxima
    let max_minutes = total / rate;
    let max_seconds = (max_minutes * Decimal::from(60)).to_u32().unwrap_or(300);

    (total, max_seconds)
}
```

## 6.3 Evento CHANNEL_ANSWER

```rust
// RealtimeBiller monitorea llamadas activas
// Cada 180 segundos verifica si necesita extender reserva

pub async fn monitor_call(&self, call_uuid: &str) -> Result<()> {
    loop {
        tokio::time::sleep(Duration::from_secs(180)).await;

        // Verificar tiempo restante
        let remaining = self.get_remaining_time(call_uuid).await?;

        if remaining < Duration::from_secs(240) {
            // Extender reserva por 3 minutos más
            self.extend_reservation(call_uuid, Duration::from_secs(180)).await?;
        }
    }
}
```

## 6.4 Evento CHANNEL_HANGUP_COMPLETE

```rust
pub async fn generate_cdr(&self, event: &ChannelHangup) -> Result<Cdr> {
    // 1. Obtener reserva
    let reservation = self.get_reservation(&event.uuid).await?;

    // 2. Calcular costo real
    let duration_seconds = event.billsec;
    let duration_minutes = Decimal::from(duration_seconds) / Decimal::from(60);
    let cost = duration_minutes * reservation.rate_per_minute;

    // 3. Crear CDR
    let cdr = Cdr {
        call_uuid: event.uuid.clone(),
        account_id: reservation.account_id,
        caller_number: event.caller_id_number.clone(),
        called_number: event.destination_number.clone(),
        start_time: event.created_time,
        answer_time: event.answered_time,
        end_time: event.hangup_time,
        duration: duration_seconds,
        billsec: duration_seconds,
        cost,
        rate_per_minute: reservation.rate_per_minute,
        hangup_cause: event.hangup_cause.clone(),
        direction: event.direction.clone(),
    };

    // 4. Insertar CDR
    self.insert_cdr(&cdr).await?;

    // 5. Consumir balance
    self.consume_balance(reservation.account_id, cost).await?;

    // 6. Liberar exceso de reserva
    let excess = reservation.amount - cost;
    if excess > Decimal::ZERO {
        self.release_reservation(&event.uuid, excess).await?;
    }

    Ok(cdr)
}
```

## 6.5 ESL (Event Socket Layer)

### Modos de Operación

**Modo Testing (ESL Server):**
```rust
// Si FREESWITCH_SERVERS está vacío
// El billing engine inicia un servidor ESL en :8021
// Útil para testing con simulador de llamadas
```

**Modo Producción (ESL Client):**
```rust
// Si FREESWITCH_SERVERS = "host:port:password"
// El billing engine se conecta a FreeSWITCH como cliente ESL
// Suscribe a eventos: CHANNEL_CREATE, CHANNEL_ANSWER, CHANNEL_HANGUP_COMPLETE
```

### Eventos Suscritos

| Evento | Trigger | Acción |
|--------|---------|--------|
| `CHANNEL_CREATE` | Inicio de llamada | Autorizar |
| `CHANNEL_ANSWER` | Llamada contestada | Iniciar billing |
| `CHANNEL_HANGUP_COMPLETE` | Llamada terminada | Generar CDR |

## 6.6 Base de Datos

### Tabla accounts

```sql
CREATE TABLE accounts (
    id SERIAL PRIMARY KEY,
    account_number VARCHAR(50) UNIQUE NOT NULL,
    account_type VARCHAR(20) NOT NULL,  -- 'prepaid' o 'postpaid'
    balance DECIMAL(15,4) NOT NULL DEFAULT 0,
    credit_limit DECIMAL(15,4) DEFAULT 0,
    status VARCHAR(20) NOT NULL DEFAULT 'active',
    max_concurrent_calls INTEGER DEFAULT 1,
    created_at TIMESTAMP WITH TIME ZONE DEFAULT NOW()
);
```

### Tabla rate_cards

```sql
CREATE TABLE rate_cards (
    id SERIAL PRIMARY KEY,
    destination_prefix VARCHAR(20) NOT NULL,
    destination_name VARCHAR(100),
    rate_per_minute DECIMAL(10,6) NOT NULL,
    billing_increment INTEGER DEFAULT 60,  -- segundos
    connection_fee DECIMAL(10,4) DEFAULT 0,
    effective_start TIMESTAMP WITH TIME ZONE NOT NULL,
    effective_end TIMESTAMP WITH TIME ZONE,
    priority INTEGER DEFAULT 0
);

-- Índice para LPM
CREATE INDEX idx_rate_cards_prefix ON rate_cards(destination_prefix);
```

### Tabla balance_reservations

```sql
CREATE TABLE balance_reservations (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    call_uuid VARCHAR(100) UNIQUE NOT NULL,
    account_id INTEGER NOT NULL REFERENCES accounts(id),
    reserved_amount DECIMAL(15,4) NOT NULL,
    rate_per_minute DECIMAL(10,6) NOT NULL,
    status VARCHAR(20) NOT NULL DEFAULT 'active',
    max_duration_seconds INTEGER,
    created_at TIMESTAMP WITH TIME ZONE DEFAULT NOW()
);
```

### Tabla cdrs

```sql
CREATE TABLE cdrs (
    id SERIAL PRIMARY KEY,
    call_uuid VARCHAR(100) UNIQUE NOT NULL,
    account_id INTEGER REFERENCES accounts(id),
    caller_number VARCHAR(50) NOT NULL,
    called_number VARCHAR(50) NOT NULL,
    start_time TIMESTAMP WITH TIME ZONE NOT NULL,
    answer_time TIMESTAMP WITH TIME ZONE,
    end_time TIMESTAMP WITH TIME ZONE NOT NULL,
    duration INTEGER NOT NULL,
    billsec INTEGER NOT NULL,
    cost DECIMAL(15,4) NOT NULL,
    rate_per_minute DECIMAL(10,6),
    hangup_cause VARCHAR(50),
    direction VARCHAR(20),
    created_at TIMESTAMP WITH TIME ZONE DEFAULT NOW()
);

-- Índices para queries comunes
CREATE INDEX idx_cdrs_account_id ON cdrs(account_id);
CREATE INDEX idx_cdrs_start_time ON cdrs(start_time);
CREATE INDEX idx_cdrs_caller ON cdrs(caller_number);
```

## 6.7 API del Billing Engine

| Endpoint | Método | Descripción |
|----------|--------|-------------|
| `/health` | GET | Health check |
| `/authorize` | POST | Autorizar llamada |
| `/hangup` | POST | Notificar colgado |
| `/stats` | GET | Estadísticas |

## 6.8 Ejercicios Prácticos

### Ejercicio 6.1: Ver logs del billing engine
```bash
# Logs del servicio
journalctl -u apolo-billing-engine -f

# Buscar autorizaciones
journalctl -u apolo-billing-engine | grep "AUTHORIZED"

# Buscar errores
journalctl -u apolo-billing-engine | grep -E "ERROR|DENIED"
```

### Ejercicio 6.2: Consultar reservas activas
```sql
-- Conectar a PostgreSQL
psql -U apolo_user -d apolo_billing

-- Ver reservas activas
SELECT id, call_uuid, account_id, reserved_amount,
       rate_per_minute, max_duration_seconds, created_at
FROM balance_reservations
WHERE status = 'active'
ORDER BY created_at DESC;
```

### Ejercicio 6.3: Ver CDRs recientes
```sql
-- CDRs de las últimas 24 horas
SELECT call_uuid, caller_number, called_number,
       duration, cost, hangup_cause, direction
FROM cdrs
WHERE start_time > NOW() - INTERVAL '24 hours'
ORDER BY start_time DESC
LIMIT 20;
```

---

# Módulo 7: PostgreSQL - Base de Datos

## 7.1 Esquema de Base de Datos

### Diagrama ER Simplificado

```
┌─────────────┐     ┌─────────────────┐     ┌─────────────────┐
│  accounts   │     │ rate_cards      │     │     cdrs        │
├─────────────┤     ├─────────────────┤     ├─────────────────┤
│ id (PK)     │     │ id (PK)         │     │ id (PK)         │
│ account_num │◄────│                 │     │ account_id (FK) │
│ balance     │     │ dest_prefix     │     │ call_uuid       │
│ status      │     │ rate_per_minute │     │ caller_number   │
│ plan_id     │     │ effective_start │     │ cost            │
└─────────────┘     └─────────────────┘     │ hangup_cause    │
       │                                     └─────────────────┘
       │
       ▼
┌─────────────────────┐     ┌──────────────────────┐
│ balance_reservations│     │ balance_transactions │
├─────────────────────┤     ├──────────────────────┤
│ id (PK)             │     │ id (PK)              │
│ call_uuid           │     │ account_id (FK)      │
│ account_id (FK)     │     │ amount               │
│ reserved_amount     │     │ transaction_type     │
│ status              │     │ reference            │
└─────────────────────┘     └──────────────────────┘
```

## 7.2 Queries Comunes

### Ver cuentas
```sql
-- Listar cuentas activas
SELECT id, account_number, account_type, balance, status
FROM accounts
WHERE status = 'active'
ORDER BY account_number;

-- Buscar cuenta por número
SELECT * FROM accounts WHERE account_number = '17402002';
```

### Ver tarifas
```sql
-- Buscar tarifa para un destino (LPM manual)
SELECT destination_prefix, destination_name, rate_per_minute
FROM rate_cards
WHERE '541155550000' LIKE destination_prefix || '%'
  AND effective_start <= NOW()
  AND (effective_end IS NULL OR effective_end >= NOW())
ORDER BY LENGTH(destination_prefix) DESC
LIMIT 1;

-- Ver todas las tarifas para Argentina
SELECT destination_prefix, destination_name, rate_per_minute
FROM rate_cards
WHERE destination_prefix LIKE '54%'
ORDER BY destination_prefix;
```

### Ver CDRs
```sql
-- CDRs de hoy
SELECT date_trunc('hour', start_time) as hora,
       COUNT(*) as llamadas,
       SUM(duration)/60 as minutos,
       SUM(cost) as costo
FROM cdrs
WHERE start_time >= CURRENT_DATE
GROUP BY date_trunc('hour', start_time)
ORDER BY hora;

-- Top 10 destinos por costo
SELECT called_number, COUNT(*) as llamadas, SUM(cost) as total
FROM cdrs
WHERE start_time > NOW() - INTERVAL '7 days'
GROUP BY called_number
ORDER BY total DESC
LIMIT 10;

-- CDRs fallidos
SELECT call_uuid, caller_number, called_number, hangup_cause, start_time
FROM cdrs
WHERE hangup_cause NOT IN ('NORMAL_CLEARING', 'SUCCESS')
  AND start_time > NOW() - INTERVAL '24 hours'
ORDER BY start_time DESC;
```

### Ver reservas
```sql
-- Reservas activas
SELECT br.id, br.call_uuid, a.account_number,
       br.reserved_amount, br.max_duration_seconds,
       br.created_at
FROM balance_reservations br
JOIN accounts a ON br.account_id = a.id
WHERE br.status = 'active';

-- Reservas huérfanas (más de 2 horas)
SELECT * FROM balance_reservations
WHERE status = 'active'
  AND created_at < NOW() - INTERVAL '2 hours';
```

## 7.3 Mantenimiento

### Limpieza de reservas huérfanas
```sql
-- Liberar reservas viejas
UPDATE balance_reservations
SET status = 'expired'
WHERE status = 'active'
  AND created_at < NOW() - INTERVAL '4 hours';
```

### Vacuuming
```bash
# Vacuum de tablas grandes
psql -U apolo_user -d apolo_billing -c "VACUUM ANALYZE cdrs;"
psql -U apolo_user -d apolo_billing -c "VACUUM ANALYZE balance_reservations;"
```

### Backup
```bash
# Backup completo
pg_dump -U apolo_user apolo_billing > backup_$(date +%Y%m%d).sql

# Backup solo de CDRs
pg_dump -U apolo_user -t cdrs apolo_billing > cdrs_backup.sql
```

## 7.4 Ejercicios Prácticos

### Ejercicio 7.1: Análisis de tráfico
```sql
-- Conectar
psql -U apolo_user -d apolo_billing

-- Tráfico por hora hoy
SELECT date_trunc('hour', start_time) as hora,
       direction,
       COUNT(*) as llamadas,
       ROUND(AVG(duration)) as dur_promedio,
       ROUND(SUM(cost)::numeric, 2) as costo_total
FROM cdrs
WHERE start_time >= CURRENT_DATE
GROUP BY date_trunc('hour', start_time), direction
ORDER BY hora, direction;
```

### Ejercicio 7.2: Buscar problemas
```sql
-- Llamadas con alto ASR de fallo
SELECT called_number,
       COUNT(*) as total,
       SUM(CASE WHEN hangup_cause = 'NORMAL_CLEARING' THEN 1 ELSE 0 END) as exitosas,
       ROUND(100.0 * SUM(CASE WHEN hangup_cause = 'NORMAL_CLEARING' THEN 1 ELSE 0 END) / COUNT(*), 2) as asr
FROM cdrs
WHERE start_time > NOW() - INTERVAL '24 hours'
GROUP BY called_number
HAVING COUNT(*) >= 10
ORDER BY asr ASC
LIMIT 20;
```

---

# Módulo 8: Flujos de Señalización

## 8.1 Llamada Entrante (Carrier → Extensión)

### Diagrama de Secuencia

```
INTEGRATEL        Kamailio         RTPEngine        FreeSWITCH        Softphone
(10.35.13.100)   (10.10.22.4:5060) (7722)          (5082)           (192.168.x)
    │                  │                │                │                │
    │──INVITE─────────>│                │                │                │
    │                  │                │                │                │
    │<─100 Trying──────│                │                │                │
    │                  │                │                │                │
    │                  │──rtpe_offer───>│                │                │
    │                  │<─SDP rewritten─│                │                │
    │                  │                │                │                │
    │                  │──INVITE───────────────────────>│                │
    │                  │                │                │                │
    │                  │                │                │──INVITE──────>│
    │                  │                │                │                │
    │                  │                │                │<─180 Ringing──│
    │                  │                │                │                │
    │                  │<─183 Progress───────────────────│                │
    │                  │   (SDP + early media)           │                │
    │                  │                │                │                │
    │                  │──rtpe_answer──>│                │                │
    │                  │<─SDP rewritten─│                │                │
    │                  │                │                │                │
    │<─183 Progress────│ (P-Early-Media)│                │                │
    │                  │                │                │                │
    │──PRACK──────────>│                │                │                │
    │                  │──PRACK────────────────────────>│                │
    │                  │<─200 OK (PRACK)─────────────────│                │
    │<─200 OK (PRACK)──│                │                │                │
    │                  │                │                │                │
    │<════════════════════════ RTP (ringback) ═══════════════════════════>│
    │                  │                │                │                │
    │                  │                │                │<─200 OK───────│
    │                  │<─200 OK─────────────────────────│                │
    │                  │──rtpe_answer──>│                │                │
    │<─200 OK──────────│                │                │                │
    │                  │                │                │                │
    │──ACK────────────>│──ACK──────────────────────────>│──ACK─────────>│
    │                  │                │                │                │
    │<═══════════════════════════ RTP (voice) ═══════════════════════════>│
```

### Headers Clave

**INVITE inicial (INTEGRATEL → Kamailio):**
```
INVITE sip:672217402002@10.124.194.14 SIP/2.0
Via: SIP/2.0/UDP 10.35.13.100:5060
From: <sip:938391839@integratel.com>;tag=abc123
To: <sip:672217402002@10.124.194.14>
Supported: 100rel
P-Early-Media: supported
```

**183 Session Progress (FreeSWITCH → Kamailio → INTEGRATEL):**
```
SIP/2.0 183 Session Progress
From: <sip:938391839@integratel.com>;tag=abc123
To: <sip:672217402002@10.124.194.14>;tag=def456
Require: 100rel
RSeq: 1
P-Early-Media: sendrecv
Content-Type: application/sdp

v=0
c=IN IP4 10.124.194.142   ← IP de RTPEngine (external)
m=audio 15000 RTP/AVP 8 0
```

**PRACK (INTEGRATEL → FreeSWITCH):**
```
PRACK sip:672217402002@10.10.22.4:5082 SIP/2.0
RAck: 1 1 INVITE
```

## 8.2 Llamada Saliente (Extensión → Carrier)

### Diagrama de Secuencia

```
Softphone         FreeSWITCH         Kamailio          RTPEngine        INTEGRATEL
(192.168.x)      (5080)             (5060)            (7722)           (10.35.13.100)
    │                │                  │                │                  │
    │──INVITE──────>│                  │                │                  │
    │                │                  │                │                  │
    │<─100 Trying────│                  │                │                  │
    │                │                  │                │                  │
    │                │──INVITE─────────>│                │                  │
    │                │                  │                │                  │
    │                │                  │──rtpe_offer───>│                  │
    │                │                  │<─SDP rewritten─│                  │
    │                │                  │                │                  │
    │                │                  │──INVITE──────────────────────────>│
    │                │                  │   (+100rel, +P-Early-Media)       │
    │                │                  │                │                  │
    │                │                  │<─100 Trying─────────────────────────│
    │                │<─100 Trying──────│                │                  │
    │                │                  │                │                  │
    │                │                  │<─183 Progress────────────────────│
    │                │                  │   (SDP + Require:100rel)          │
    │                │                  │                │                  │
    │                │                  │──rtpe_answer──>│                  │
    │                │                  │<─SDP rewritten─│                  │
    │                │                  │                │                  │
    │                │<─183 Progress────│                │                  │
    │                │                  │                │                  │
    │                │──PRACK──────────>│                │                  │
    │                │                  │──PRACK───────────────────────────>│
    │                │                  │<─200 OK (PRACK)────────────────────│
    │                │<─200 OK (PRACK)──│                │                  │
    │                │                  │                │                  │
    │<═══════════════════════════ RTP (ringback) ═══════════════════════════>│
    │                │                  │                │                  │
    │                │                  │<─200 OK──────────────────────────│
    │                │<─200 OK──────────│                │                  │
    │<─200 OK────────│                  │                │                  │
    │                │                  │                │                  │
    │──ACK─────────>│──ACK────────────>│──ACK─────────────────────────────>│
    │                │                  │                │                  │
    │<═══════════════════════════ RTP (voice) ═════════════════════════════>│
```

## 8.3 Timeout y Tono de Congestión

### Flujo de Timeout (Usuario No Responde)

```
INTEGRATEL        Kamailio         FreeSWITCH        Softphone
    │                │                │                │
    │══════════ INVITE + early media setup ══════════►│
    │                │                │                │
    │<═══════════ RTP RINGBACK (55 segundos) ═════════│
    │                │                │                │
    │                │                │ [TIMEOUT 55s]  │
    │                │                │ bridge falla   │
    │                │                │                │
    │                │                │──200 OK (answer)
    │<─200 OK────────│                │   (para poder enviar tono)
    │                │                │                │
    │<═════════ RTP TONO CONGESTIÓN (7 segundos) ══════│
    │                │                │                │
    │                │<─BYE───────────│                │
    │                │  Reason: Q.850;cause=18         │
    │<─BYE───────────│                │                │
    │                │                │                │
    │──200 OK───────>│──200 OK───────>│                │
```

### Tono de Congestión

```
tone_stream://%(250,250,480,620);loops=14

Frecuencias: 480Hz + 620Hz (estándar de congestión)
Cadencia: 250ms ON, 250ms OFF = 500ms por ciclo
14 ciclos × 500ms = 7 segundos
```

---

# Módulo 9: Flujos de Media (RTP)

## 9.1 Arquitectura de Media

```
┌────────────────────────────────────────────────────────────────────────────┐
│                           SERVIDOR SBC                                      │
│                                                                             │
│   ┌─────────────────────────────────────────────────────────────────────┐  │
│   │                          RTPENGINE                                   │  │
│   │                                                                      │  │
│   │   ┌─────────────────┐              ┌─────────────────┐              │  │
│   │   │ External Leg    │              │ Internal Leg    │              │  │
│   │   │ 10.124.194.142  │◄════════════►│ 10.10.22.4      │              │  │
│   │   │ :10000-30000    │    RELAY     │ :10000-30000    │              │  │
│   │   └────────▲────────┘              └────────▲────────┘              │  │
│   │            │                                │                        │  │
│   └────────────┼────────────────────────────────┼────────────────────────┘  │
│                │                                │                           │
│                │                                │                           │
│   ┌────────────┼────────────────────────────────┼────────────────────────┐  │
│   │            │                                │                        │  │
│   │    RTP     │                         RTP    │                        │  │
│   │            ▼                                ▼                        │  │
│   │   ┌─────────────┐                  ┌─────────────┐                   │  │
│   │   │   CARRIER   │                  │ FREESWITCH  │                   │  │
│   │   │ 10.35.13.100│                  │ 10.10.22.4  │                   │  │
│   │   └─────────────┘                  │   :5080     │                   │  │
│   │                                    └──────▲──────┘                   │  │
│   │                                           │                          │  │
│   │                                     RTP   │                          │  │
│   │                                           ▼                          │  │
│   │                                    ┌─────────────┐                   │  │
│   │                                    │  SOFTPHONE  │                   │  │
│   │                                    │ 192.168.x.x │                   │  │
│   │                                    └─────────────┘                   │  │
│   │                                                                      │  │
│   │                        LAN (10.10.22.0/28)                           │  │
│   └──────────────────────────────────────────────────────────────────────┘  │
│                                                                             │
└─────────────────────────────────────────────────────────────────────────────┘
                                   │
                                   │ INTERNET
                                   ▼
                        ┌─────────────────┐
                        │ CARRIER NETWORK │
                        │  10.35.13.0/24  │
                        └─────────────────┘
```

## 9.2 NAT Traversal

### Problema de NAT

```
Sin RTPEngine:
                                    NAT/Firewall
                                         │
Softphone (192.168.1.50:16000) ──────────┼─────────► Carrier (10.35.13.100)
     │                                   │                     │
     │  SDP dice: c=192.168.1.50         │                     │
     │                                   │                     │
     └───────────────────────────────────┼─────────────────────┘
                                         │
            Carrier no puede alcanzar IP privada!
```

### Solución con RTPEngine

```
Con RTPEngine:
                                    NAT/Firewall
                                         │
Softphone ──► RTPEngine ─────────────────┼─────────► Carrier
192.168.1.50   10.10.22.4               │          10.35.13.100
                  │                      │
                  │ SDP reescrito:       │
                  │ c=10.124.194.142     │
                  └──────────────────────┘
                                    IP pública alcanzable!
```

## 9.3 Transformación de SDP

### Llamada Entrante

**1. INVITE del Carrier:**
```
c=IN IP4 10.35.13.100
m=audio 20000 RTP/AVP 8 0
```

**2. Después de rtpengine_offer() → hacia FreeSWITCH:**
```
c=IN IP4 10.10.22.4        ← IP del leg interno de RTPEngine
m=audio 15432 RTP/AVP 8 0  ← Puerto del leg interno
```

**3. Respuesta de FreeSWITCH:**
```
c=IN IP4 10.10.22.4
m=audio 16384 RTP/AVP 8 0
```

**4. Después de rtpengine_answer() → hacia Carrier:**
```
c=IN IP4 10.124.194.142     ← IP del leg externo
m=audio 12000 RTP/AVP 8 0   ← Puerto del leg externo
```

## 9.4 Symmetric RTP y Port Latching

### Symmetric RTP

```
Problema sin symmetric:
    Carrier envía desde: 10.35.13.100:20001
    Pero en SDP dice:     10.35.13.100:20000 ← puerto diferente!

Con symmetric RTP:
    RTPEngine detecta de dónde vienen los paquetes
    y responde a esa IP:puerto (no al del SDP)
```

### Port Latching

```
1. RTPEngine escucha en puerto 10000
2. Primer paquete RTP llega de 10.35.13.100:25000
3. RTPEngine "latches" a ese origen
4. Todos los paquetes de respuesta van a 10.35.13.100:25000
```

## 9.5 Early Media

### ¿Qué es Early Media?

Media que fluye **antes** del 200 OK (durante el 183 Session Progress).

### Uso en ApoloBilling

```
┌──────────┐   INVITE   ┌───────────┐            ┌──────────┐
│ Carrier  │───────────>│    SBC    │───INVITE──>│Extension │
│          │            │           │            │          │
│          │<──183──────│           │<──180──────│          │
│          │  (SDP)     │           │            │          │
│          │            │           │            │          │
│          │<══════════════ RTP RINGBACK ════════│          │
│          │            │  (durante 0-55 seg)    │          │
│          │            │           │            │          │
│          │<──200 OK───│           │<──200 OK───│          │
│          │            │           │            │          │
│          │<═══════════════ RTP VOICE ══════════│          │
└──────────┘            └───────────┘            └──────────┘
```

## 9.6 Ejercicios Prácticos

### Ejercicio 9.1: Ver sesiones RTP durante llamada
```bash
# Terminal 1: Hacer una llamada de prueba

# Terminal 2: Ver sesiones RTPEngine
watch -n 1 'rtpengine-ctl list sessions'

# Ver detalles de una sesión específica
rtpengine-ctl query <call-id>
```

### Ejercicio 9.2: Captura de RTP
```bash
# Capturar RTP en interface externa
tcpdump -i ens20 -s 0 -w /tmp/rtp_capture.pcap "udp portrange 10000-30000"

# Analizar con tshark
tshark -r /tmp/rtp_capture.pcap -Y rtp -T fields \
    -e frame.time_relative -e ip.src -e ip.dst \
    -e rtp.ssrc -e rtp.seq -e rtp.timestamp
```

---

# Módulo 10: Troubleshooting y Diagnóstico

## 10.1 Herramientas de Diagnóstico

### sngrep - Captura SIP

```bash
# Captura básica
sngrep

# Filtrar por IP
sngrep host 10.35.13.100

# Filtrar por puerto
sngrep port 5060

# Guardar a archivo
sngrep -O /tmp/sip_trace.pcap

# Cargar archivo existente
sngrep -I /tmp/sip_trace.pcap
```

### tcpdump - Captura de Paquetes

```bash
# SIP (señalización)
tcpdump -i any -s 0 -w /tmp/sip.pcap "port 5060 or port 5080 or port 5082"

# RTP (media)
tcpdump -i ens20 -s 0 -w /tmp/rtp.pcap "udp portrange 10000-30000"

# Todo el tráfico de un carrier
tcpdump -i any -s 0 -w /tmp/carrier.pcap "host 10.35.13.100"
```

### sipgrep - Grep para SIP

```bash
# Filtrar INVITEs
sipgrep -i any "INVITE"

# Buscar Call-ID específico
sipgrep -i any "Call-ID: abc123"

# Filtrar por número
sipgrep -i any "672217402002"
```

## 10.2 Diagnóstico por Componente

### Kamailio

```bash
# Estado del servicio
systemctl status kamailio

# Logs en tiempo real
journalctl -u kamailio -f

# Subir nivel de debug
kamcmd debug.level 3

# Ver estadísticas
kamcmd stats.fetch all

# Verificar drouting
kamcmd drouting.dump

# Recargar configuración
kamcmd drouting.reload
```

### FreeSWITCH

```bash
# Estado del servicio
systemctl status freeswitch

# Consola interactiva
fs_cli

# Comandos útiles en fs_cli
sofia status                    # Estado de perfiles
sofia status profile internal   # Detalle de perfil
sofia status profile internal reg  # Registros
show calls                      # Llamadas activas
show channels                   # Canales
sofia global siptrace on        # Debug SIP
reloadxml                       # Recargar configuración
```

### RTPEngine

```bash
# Estado del servicio
systemctl status rtpengine

# Ver sesiones
rtpengine-ctl list sessions

# Ver estadísticas
rtpengine-ctl list totals

# Logs
journalctl -u rtpengine -f
```

### PostgreSQL

```bash
# Conectar
psql -U apolo_user -d apolo_billing

# Verificar conexiones
SELECT * FROM pg_stat_activity WHERE datname = 'apolo_billing';

# Ver tamaño de tablas
SELECT relname, pg_size_pretty(pg_total_relation_size(relid))
FROM pg_stat_user_tables
ORDER BY pg_total_relation_size(relid) DESC;
```

## 10.3 Problemas Comunes

### Problema: Llamada no conecta (sin respuesta del carrier)

**Síntomas:**
- INVITE enviado pero no hay respuesta
- Timeout después de 32 segundos

**Diagnóstico:**
```bash
# 1. Verificar que INVITE llega a Kamailio
sngrep host 10.35.13.100

# 2. Verificar routing
kamcmd drouting.dump

# 3. Verificar conectividad
ping 10.35.13.100
```

**Solución:**
- Verificar IP del carrier en dr_gateways
- Verificar reglas de drouting
- Verificar firewall

### Problema: 488 Not Acceptable Here

**Síntomas:**
- Carrier rechaza con 488
- No hay media establecido

**Diagnóstico:**
```bash
# Ver SDP en el INVITE
sngrep host 10.35.13.100
# Buscar el INVITE y revisar los codecs ofrecidos

# Ver codecs en FreeSWITCH
fs_cli -x "sofia status profile internal"
```

**Solución:**
- Verificar codecs compatibles (PCMA/G.711A es el más común)
- Agregar codecs al perfil de FreeSWITCH
- Usar codec-mask en RTPEngine si es necesario

### Problema: Sin audio (one-way audio)

**Síntomas:**
- Llamada conecta pero no hay audio
- Audio en una sola dirección

**Diagnóstico:**
```bash
# 1. Ver sesiones RTPEngine
rtpengine-ctl list sessions

# 2. Verificar que hay paquetes en ambas direcciones
# Mirar "packets" en cada leg

# 3. Capturar RTP
tcpdump -i any "udp portrange 10000-30000" -c 100
```

**Solución:**
- Verificar que symmetric está habilitado en RTPEngine
- Verificar interfaces de RTPEngine
- Verificar firewall para puertos RTP

### Problema: PRACK no enviado (carrier IMS)

**Síntomas:**
- 183 Session Progress llega con `Require: 100rel`
- No se envía PRACK
- Carrier retransmite 183 y eventualmente cancela

**Diagnóstico:**
```bash
# Ver en sngrep:
# 1. INVITE tiene "Supported: 100rel"?
# 2. 183 tiene "RSeq" header?
# 3. Se envía PRACK con "RAck"?
sngrep host 10.35.13.100
```

**Solución:**
- Verificar `enable-100rel=true` en perfil FreeSWITCH
- Verificar que Kamailio agrega `Supported: 100rel`
- Verificar `auto-answer-100rel=true`

### Problema: Registro fallido (SIP device)

**Síntomas:**
- 401/403 en REGISTER
- Softphone no puede registrarse

**Diagnóstico:**
```bash
# 1. Ver logs de FreeSWITCH
tail -f /var/log/freeswitch/freeswitch.log | grep -i register

# 2. Verificar que el dispositivo existe
psql -U apolo_user -d apolo_billing -c \
  "SELECT username, enabled FROM sip_devices WHERE username = '1001'"

# 3. Verificar IP whitelist para mod_xml_curl
psql -U apolo_user -d apolo_billing -c \
  "SELECT * FROM freeswitch_allowed_ips"
```

**Solución:**
- Verificar credenciales del dispositivo
- Verificar que el endpoint de directory responde
- Verificar IP whitelist

## 10.4 Checklist de Troubleshooting

### Antes de Escalar

- [ ] ¿El servicio está corriendo? (`systemctl status <service>`)
- [ ] ¿Hay errores en los logs? (`journalctl -u <service> -n 100`)
- [ ] ¿Hay conectividad? (`ping`, `telnet <ip> <port>`)
- [ ] ¿La configuración es correcta? (verificar archivos)
- [ ] ¿Hubo cambios recientes? (git log, deployments)

### Durante una Incidencia

1. **Capturar evidencia** antes de hacer cambios
2. **Documentar** timestamp, síntomas, acciones
3. **Comunicar** al equipo si el impacto es alto
4. **Escalar** si no se resuelve en 15 minutos

---

# Anexos: Comandos de Referencia Rápida

## A.1 Kamailio

```bash
# Servicio
systemctl start|stop|restart|status kamailio

# Logs
journalctl -u kamailio -f
journalctl -u kamailio --since "1 hour ago"

# kamcmd
kamcmd drouting.reload     # Recargar routes
kamcmd drouting.dump       # Ver routes
kamcmd debug.level 3       # Subir debug
kamcmd debug.level 1       # Bajar debug
kamcmd stats.fetch all     # Estadísticas
kamcmd permissions.addressReload  # Recargar ACL

# MySQL (Kamailio DB)
mysql -u kamailio -p kamailio
SELECT * FROM dr_gateways;
SELECT * FROM dr_rules WHERE groupid = 8000;
```

## A.2 FreeSWITCH

```bash
# Servicio
systemctl start|stop|restart|status freeswitch

# Logs
tail -f /var/log/freeswitch/freeswitch.log
tail -f /var/log/freeswitch/freeswitch.log | grep ERROR

# fs_cli
fs_cli                            # Consola interactiva
fs_cli -x "comando"              # Ejecutar comando

# Comandos comunes
sofia status                      # Estado de perfiles
sofia status profile internal     # Detalle perfil
sofia status profile internal reg # Registros
show calls                        # Llamadas activas
show channels                     # Canales
hupall                           # Colgar todas
reloadxml                        # Recargar config
sofia global siptrace on          # Debug SIP on
sofia global siptrace off         # Debug SIP off
originate user/1001 &echo        # Test call
```

## A.3 RTPEngine

```bash
# Servicio
systemctl start|stop|restart|status rtpengine

# Logs
journalctl -u rtpengine -f

# rtpengine-ctl
rtpengine-ctl list sessions      # Ver sesiones
rtpengine-ctl list totals        # Estadísticas
rtpengine-ctl query <call-id>    # Detalles de sesión
rtpengine-ctl help               # Ayuda
```

## A.4 PostgreSQL

```bash
# Conectar
psql -U apolo_user -d apolo_billing

# Queries útiles
\dt                              # Listar tablas
\d+ accounts                     # Describir tabla

# CDRs recientes
SELECT * FROM cdrs ORDER BY start_time DESC LIMIT 10;

# Reservas activas
SELECT * FROM balance_reservations WHERE status = 'active';

# Cuentas
SELECT account_number, balance, status FROM accounts;

# Rate cards
SELECT destination_prefix, rate_per_minute FROM rate_cards;
```

## A.5 Captura de Tráfico

```bash
# SIP con sngrep
sngrep                           # Interactivo
sngrep host 10.35.13.100         # Filtrar por IP
sngrep -O /tmp/trace.pcap        # Guardar

# SIP con tcpdump
tcpdump -i any -s 0 -w /tmp/sip.pcap "port 5060"

# RTP con tcpdump
tcpdump -i ens20 -s 0 -w /tmp/rtp.pcap "udp portrange 10000-30000"

# Todo de un carrier
tcpdump -i any -s 0 -w /tmp/all.pcap "host 10.35.13.100"

# Analizar con tshark
tshark -r capture.pcap -Y sip
tshark -r capture.pcap -Y rtp
```

## A.6 Systemd

```bash
# Ver todos los servicios del sistema
systemctl list-units --type=service | grep -E "kamailio|freeswitch|rtpengine|apolo|postgres|mysql|redis"

# Ver logs de múltiples servicios
journalctl -u kamailio -u freeswitch -u rtpengine -f

# Reiniciar todos los servicios de voz
systemctl restart kamailio freeswitch rtpengine

# Ver desde cuándo está corriendo
systemctl show kamailio --property=ActiveEnterTimestamp
```

---

## Glosario

| Término | Definición |
|---------|------------|
| **ANI** | Automatic Number Identification - número del caller |
| **ASR** | Answer Seizure Ratio - porcentaje de llamadas contestadas |
| **B2BUA** | Back-to-Back User Agent - une dos llamadas |
| **CDR** | Call Detail Record - registro de llamada |
| **DTMF** | Dual-Tone Multi-Frequency - tonos de teclado |
| **ESL** | Event Socket Layer - interfaz de FreeSWITCH |
| **IMS** | IP Multimedia Subsystem - arquitectura de carrier |
| **LPM** | Longest Prefix Match - búsqueda de tarifa |
| **NAT** | Network Address Translation |
| **PRACK** | Provisional Response Acknowledgment |
| **RTP** | Real-time Transport Protocol - media |
| **SBC** | Session Border Controller |
| **SDP** | Session Description Protocol |
| **SIP** | Session Initiation Protocol |
| **SRTP** | Secure RTP - media encriptado |

---

## Recursos Adicionales

### Documentación Interna
- `/opt/ApoloBilling/docs/RTP_FLOW_ARCHITECTURE.md` - Flujos RTP detallados
- `/opt/ApoloBilling/docs/INTEGRATEL_CONGESTION_TONE_CONFIG.md` - Configuración INTEGRATEL
- `/opt/ApoloBilling/CLAUDE.md` - Documentación técnica completa
- `/opt/ApoloBilling/INSTALL.md` - Guía de instalación

### RFCs Relevantes
- RFC 3261 - SIP: Session Initiation Protocol
- RFC 3262 - Reliability of Provisional Responses (100rel/PRACK)
- RFC 3264 - SDP Offer/Answer Model
- RFC 3550 - RTP: Real-time Transport Protocol
- RFC 5009 - P-Early-Media Header

### Herramientas
- [sngrep](https://github.com/irontec/sngrep) - Captura SIP
- [Wireshark](https://www.wireshark.org/) - Análisis de paquetes
- [SIPVicious](https://github.com/EnableSecurity/sipvicious) - Testing SIP

---

*Documento creado para el equipo NOC de ApoloBilling*
*Versión 1.0 - Agosto 2026*
