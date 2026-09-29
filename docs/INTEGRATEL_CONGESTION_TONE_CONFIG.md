# Configuración de Tono de Congestión para INTEGRATEL

## Resumen Ejecutivo

Este documento describe la configuración implementada para reproducir un tono de congestión de 7 segundos en llamadas entrantes desde INTEGRATEL cuando el usuario no responde, antes de enviar la señalización de finalización de llamada.

**Fecha de implementación:** 4 de Junio de 2026
**Carrier:** INTEGRATEL (IP: 10.35.13.100)
**Resultado:** Señalización correcta con BYE + Reason: Q.850;cause=18;text="NO_USER_RESPONSE"

---

## Arquitectura del Sistema

```
INTEGRATEL (10.35.13.100)
        │
        │ SIP (UDP:5060)
        ▼
┌───────────────────────────────────────────────────────────────┐
│                    KAMAILIO / dSIPRouter                       │
│                         :5060 (SBC)                            │
│                                                                │
│   - Recibe INVITE de INTEGRATEL                               │
│   - Maneja 100rel/PRACK para reliable provisionals            │
│   - Reenvía respuestas provisionales y finales                │
│   - RTPEngine para media relay                                 │
└───────────────────────────────────────────────────────────────┘
        │
        │ SIP (UDP:5082)
        ▼
┌───────────────────────────────────────────────────────────────┐
│                        FREESWITCH                              │
│                    Profile: inbound (:5082)                    │
│                                                                │
│   - Context: to-kamailio                                       │
│   - Genera early media con 183 + SDP                          │
│   - Reproduce ringback durante early media                     │
│   - Timeout de 55 segundos                                     │
│   - Reproduce tono de congestión si no hay respuesta          │
└───────────────────────────────────────────────────────────────┘
        │
        │ RTP
        ▼
┌───────────────────────────────────────────────────────────────┐
│                        RTPENGINE                               │
│          Internal: 10.10.22.4 / External: 10.124.194.142      │
│                                                                │
│   - Media relay bidireccional                                  │
│   - Symmetric RTP para NAT traversal                          │
│   - Codec passthrough (PCMA/G711)                             │
└───────────────────────────────────────────────────────────────┘
```

---

## Flujo de Llamada

### Flujo Exitoso (Usuario Responde)

```
INTEGRATEL          Kamailio           FreeSWITCH         Usuario
    │                  │                   │                 │
    │──INVITE─────────>│                   │                 │
    │<─100 Trying──────│                   │                 │
    │                  │──INVITE──────────>│                 │
    │                  │<─183 Progress─────│ (early media)   │
    │<─183 Progress────│                   │                 │
    │──PRACK──────────>│                   │                 │
    │                  │──PRACK───────────>│                 │
    │                  │<─200 OK (PRACK)───│                 │
    │<─200 OK (PRACK)──│                   │                 │
    │                  │                   │──INVITE────────>│
    │                  │                   │<─200 OK─────────│
    │                  │<─200 OK───────────│                 │
    │<─200 OK──────────│                   │                 │
    │──ACK────────────>│                   │                 │
    │                  │──ACK─────────────>│                 │
    │<═══════════════════════ RTP ═══════════════════════════>│
```

### Flujo con Timeout (Usuario No Responde) - IMPLEMENTADO

```
INTEGRATEL          Kamailio           FreeSWITCH         Usuario
    │                  │                   │                 │
    │──INVITE─────────>│                   │                 │
    │<─100 Trying──────│                   │                 │
    │                  │──INVITE──────────>│                 │
    │                  │<─183 Progress─────│ (ringback)      │
    │<─183 Progress────│                   │                 │
    │──PRACK──────────>│                   │                 │
    │                  │──PRACK───────────>│                 │
    │                  │<─200 OK (PRACK)───│                 │
    │<─200 OK (PRACK)──│                   │                 │
    │                  │                   │                 │
    │<══════ RINGBACK (early media) ══════>│  (55 segundos) │
    │                  │                   │                 │
    │                  │                   │ [TIMEOUT 55s]   │
    │                  │                   │                 │
    │                  │<─200 OK───────────│ (answer)        │
    │<─200 OK──────────│                   │                 │
    │──ACK────────────>│                   │                 │
    │                  │──ACK─────────────>│                 │
    │                  │                   │                 │
    │<══════ TONO CONGESTIÓN (7 seg) ═════>│                 │
    │                  │                   │                 │
    │                  │<─BYE──────────────│                 │
    │<─BYE─────────────│  Reason: Q.850    │                 │
    │  cause=18        │  NO_USER_RESPONSE │                 │
    │──200 OK─────────>│                   │                 │
    │                  │──200 OK──────────>│                 │
```

---

## Configuración de FreeSWITCH

### 1. Profile SIP Inbound

**Archivo:** `/etc/freeswitch/sip_profiles/inbound.xml`

```xml
<profile name="inbound">
  <settings>
    <param name="sip-ip" value="10.10.22.4"/>
    <param name="sip-port" value="5082"/>
    <param name="rtp-ip" value="10.10.22.4"/>
    <param name="ext-rtp-ip" value="auto-nat"/>
    <param name="ext-sip-ip" value="auto-nat"/>
    <param name="context" value="to-kamailio"/>
    <param name="dialplan" value="XML"/>

    <!-- CRÍTICO: Habilitar 100rel/PRACK para IMS -->
    <param name="enable-100rel" value="true"/>

    <param name="disable-register" value="true"/>
    <param name="inbound-bypass-media" value="false"/>
    <param name="rtp-timeout-sec" value="300"/>
    <param name="rtp-hold-timeout-sec" value="1800"/>
  </settings>
</profile>
```

**Puntos clave:**
- `enable-100rel=true`: Requerido para soportar PRACK con INTEGRATEL
- `inbound-bypass-media=false`: FreeSWITCH debe manejar el media para generar tones
- Puerto 5082 dedicado para tráfico de Kamailio

### 2. Dialplan para INTEGRATEL

**Archivo:** `/etc/freeswitch/dialplan/to-kamailio.xml`

```xml
<?xml version="1.0" encoding="utf-8"?>
<include>
  <context name="to-kamailio">

    <!-- ============================================================ -->
    <!-- Llamadas entrantes desde INTEGRATEL (RN 22)                   -->
    <!-- IMS Interworking: Early Media con 183 SDP + 100rel/PRACK      -->
    <!-- FreeSWITCH genera el ringback y envía 183 con SDP             -->
    <!-- ============================================================ -->
    <extension name="from-integratel-rn-22">
      <condition field="destination_number" expression="^(6722.+)$">
        <action application="log" data="INFO Inbound From Integratel RN 22: ${caller_id_number} -> ${destination_number}"/>
        <action application="set" data="original_destination=${destination_number}"/>
        <action application="set" data="transformed_number=${destination_number:4}"/>
        <action application="log" data="INFO Transformed: ${original_destination} -> ${transformed_number}"/>

        <!-- ============================================================ -->
        <!-- TIMEOUTS                                                      -->
        <!-- 55 segundos para tener 7s de tono antes del CANCEL de        -->
        <!-- INTEGRATEL (que ocurre aproximadamente a los 62 segundos)    -->
        <!-- ============================================================ -->
        <action application="set" data="call_timeout=55"/>
        <action application="set" data="originate_timeout=55"/>
        <action application="set" data="progress_timeout=55"/>

        <!-- ============================================================ -->
        <!-- IMS 100rel/PRACK support - REQUIRED for INTEGRATEL           -->
        <!-- Permite respuestas provisionales confiables (183 con PRACK)  -->
        <!-- ============================================================ -->
        <action application="set" data="sip_enable_100rel=true"/>
        <action application="export" data="sip_enable_100rel=true"/>

        <!-- Heredar codec del INVITE original -->
        <action application="set" data="inherit_codec=true"/>

        <!-- ============================================================ -->
        <!-- IMS EARLY MEDIA MODE                                          -->
        <!-- - bypass_media=false: FreeSWITCH maneja el media             -->
        <!-- - pre_answer: Envía 183 Session Progress CON SDP             -->
        <!-- - ringback: Genera tono de ringback durante early media      -->
        <!-- ============================================================ -->
        <action application="set" data="bypass_media=false"/>
        <action application="set" data="ringback=${us-ring}"/>
        <action application="set" data="instant_ringback=true"/>

        <!-- pre_answer envía 183 Session Progress CON SDP -->
        <action application="pre_answer"/>

        <!-- Continuar después del bridge para manejar timeout -->
        <action application="set" data="hangup_after_bridge=false"/>
        <action application="set" data="continue_on_fail=true"/>

        <!-- Bridge al usuario destino -->
        <action application="bridge" data="user/${transformed_number}@10.10.22.4"/>

        <!-- ============================================================ -->
        <!-- MANEJO DE TIMEOUT / NO RESPUESTA                              -->
        <!-- Si el bridge falló por timeout o no respuesta:               -->
        <!-- 1. Contestar la llamada (200 OK)                             -->
        <!-- 2. Reproducir tono de congestión (7 segundos)                -->
        <!-- 3. Colgar con causa NO_USER_RESPONSE                         -->
        <!--                                                               -->
        <!-- IMPORTANTE: No usar transfer para mantener SDP consistente   -->
        <!-- ============================================================ -->
        <action application="log" data="WARNING FS: Bridge terminó - cause=${bridge_hangup_cause}, reproduciendo tono de congestión"/>

        <!-- Contestar la llamada (envía 200 OK) -->
        <action application="answer"/>

        <!-- Pequeña pausa para estabilizar el canal -->
        <action application="sleep" data="200"/>

        <!-- Reproducir tono de congestión (7 segundos) -->
        <!-- Tono: 480Hz + 620Hz, 250ms ON, 250ms OFF, 14 ciclos = 7s -->
        <action application="playback" data="tone_stream://%(250,250,480,620);loops=14"/>

        <!-- Colgar con causa NO_USER_RESPONSE (Q.850 cause 18) -->
        <action application="hangup" data="NO_USER_RESPONSE"/>
      </condition>
    </extension>

    <!-- Fallback para rechazar otras llamadas -->
    <extension name="fallback-reject">
      <condition field="destination_number" expression="^(.+)$">
        <action application="log" data="WARNING Rejecting call from unknown source: ${network_addr}"/>
        <action application="respond" data="403 Forbidden"/>
      </condition>
    </extension>

  </context>
</include>
```

**Puntos clave del dialplan:**

| Configuración | Valor | Propósito |
|---------------|-------|-----------|
| `call_timeout` | 55s | Tiempo antes de considerar no respuesta |
| `sip_enable_100rel` | true | Soporte para PRACK (IMS) |
| `bypass_media` | false | FreeSWITCH maneja el media |
| `ringback` | ${us-ring} | Tono de ringback US durante early media |
| `pre_answer` | - | Envía 183 con SDP |
| `hangup_after_bridge` | false | Continuar procesando después del bridge |
| `continue_on_fail` | true | Ejecutar acciones posteriores si falla |

### 3. Tono de Congestión

El tono de congestión se genera con `tone_stream`:

```
tone_stream://%(250,250,480,620);loops=14
```

- **Frecuencias:** 480Hz + 620Hz (tono de congestión estándar)
- **Timing:** 250ms ON, 250ms OFF
- **Duración:** 14 ciclos × 500ms = 7 segundos

---

## Configuración de Kamailio

### 1. Definiciones de Flags

**Archivo:** `/etc/kamailio/kamailio.cfg`

```kamailio
# Flag para identificar llamadas de INTEGRATEL que requieren tono de congestión
#!define FLT_INTEGRATEL_CONGESTION 24

# Branch flags para identificar origen/destino
#!define FLB_SRC_CARRIER 5
#!define FLB_DST_CARRIER 6
```

### 2. Configuración de RTPEngine

```kamailio
#!define RTPENGINE_URI "udp:localhost:7722"

modparam("rtpengine", "rtpengine_sock", RTPENGINE_URI)
```

### 3. Manejo de P-Early-Media (RFC 5009)

En `onreply_route[MANAGE_REPLY]`:

```kamailio
# P-Early-Media: RFC 5009 - Autorización de Early Media
# Responder con sendrecv si el peer anunció supported en INVITE
# Necesario para interoperabilidad con carriers IMS (ej: INTEGRATEL)
if (t_check_status("183")) {
    if ($dlg_var(pem_supported) == "1") {
        if (!is_present_hf("P-Early-Media")) {
            append_hf("P-Early-Media: sendrecv\r\n");
            xlog("L_INFO", "P-Early-Media: sendrecv added to 183 for $ci\n");
        }
    }
}
```

### 4. RTPEngine Offer/Answer

**OFFER (INVITE entrante):**
```kamailio
route[RTPENGINEOFFER] {
    # Flags para llamadas SIP a SIP
    $var(reflags) = "trust-address replace-origin replace-session-connection rtcp-mux-demux ICE=remove symmetric RTP/AVP";

    # Dirección: external (INTEGRATEL) -> internal (FreeSWITCH)
    $var(reflags) = $var(reflags) + " direction=external direction=internal port-latching";

    rtpengine_offer("$var(reflags)");
}
```

**ANSWER (Respuestas provisionales y finales):**
```kamailio
route[RTPENGINEANSWER] {
    # Flags para respuestas
    $var(reflags) = "trust-address replace-origin replace-session-connection rtcp-mux-demux ICE=remove symmetric";

    # Dirección invertida: internal (FreeSWITCH) -> external (INTEGRATEL)
    $var(reflags) = $var(reflags) + " direction=internal direction=external port-latching";

    rtpengine_answer("$var(reflags)");
}
```

---

## Configuración de RTPEngine

**Archivo:** `/etc/rtpengine/rtpengine.conf`

```ini
[rtpengine]
table = -1
no-fallback = false

# Interfaces: internal=LAN, external=WAN
interface = internal/10.10.22.4;external/10.124.194.142

# Control socket para Kamailio
listen-ng = 127.0.0.1:7722
listen-cli = 127.0.0.1:9900

# Puertos RTP
port-min = 10000
port-max = 30000

# Threads
num-threads = 8
media-num-threads = 4

# Timeouts
timeout = 60
silent-timeout = 3600
offer-timeout = 300

# Logging
log-level = 7
log-stderr = false
log-facility = local1
log-facility-cdr = local1
log-facility-rtcp = local1
```

---

## Resultado Final

### Señalización BYE con Reason Header

```
BYE sip:10.35.13.100:5060;yop=00.00.55D4303B.0000.7016;transport=udp SIP/2.0
Via: SIP/2.0/UDP 10.124.194.14;branch=z9hG4bKa861.dee392c6c72552c1ba96ecd30a4d2c0d.0
Via: SIP/2.0/UDP 10.10.22.4:5082;received=10.10.22.4;rport=5082;branch=z9hG4bKZcD79NyBScFrj
Max-Forwards: 69
From: <sip:672217402002@10.124.194.14;user=phone>;tag=B1cgtZ2gpg79N
To: <sip:938391839@10.35.13.100;user=phone>;tag=Dib1VD4g5.e_5DW5
Call-ID: 4670F1C48AD5CD1EBDA57117@1670ffffffff
CSeq: 115724197 BYE
User-Agent: FreeSWITCH-mod_sofia/1.10.12
Allow: INVITE, ACK, BYE, CANCEL, OPTIONS, MESSAGE, INFO, UPDATE, REFER, PRACK, NOTIFY
Supported: 100rel, timer, path, replaces
Reason: Q.850;cause=18;text="NO_USER_RESPONSE"
Content-Length: 0
```

### Timeline de la Llamada

| Tiempo | Evento | Descripción |
|--------|--------|-------------|
| 0s | INVITE | INTEGRATEL envía INVITE |
| 0s | 100 Trying | Kamailio responde |
| 0s | 183 Progress | FreeSWITCH envía con SDP |
| 0s | PRACK | INTEGRATEL confirma 183 |
| 0-55s | Early Media | Ringback hacia INTEGRATEL |
| 55s | Timeout | Bridge falla por no respuesta |
| 55s | 200 OK | FreeSWITCH contesta |
| 55-62s | Audio | Tono de congestión (7s) |
| 62s | BYE | FreeSWITCH envía con Q.850 cause=18 |

---

## Troubleshooting

### Verificar Logs de FreeSWITCH

```bash
# Ver logs en tiempo real
tail -f /var/log/freeswitch/freeswitch.log | grep -E "INTEGRATEL|congestion|bridge"

# Ver llamadas activas
fs_cli -x "show calls"
```

### Verificar Logs de Kamailio

```bash
# Ver logs en tiempo real
journalctl -u kamailio -f | grep -E "RTPENGINE|MANAGE_REPLY|183|200"
```

### Verificar RTPEngine

```bash
# Ver sesiones activas
rtpengine-ctl list sessions

# Ver estadísticas
rtpengine-ctl statistics
```

### Captura SIP para Diagnóstico

```bash
# Captura con sngrep
sngrep -c

# Captura con tcpdump
tcpdump -i any -s 0 -w /tmp/sip_capture.pcap "port 5060 or port 5082"
```

---

## Referencias

- RFC 3261 - SIP: Session Initiation Protocol
- RFC 3262 - Reliability of Provisional Responses (100rel/PRACK)
- RFC 5009 - P-Early-Media Header
- ITU-T Q.850 - Cause Codes
- FreeSWITCH Dialplan Documentation
- Kamailio RTPEngine Module Documentation

---

## Historial de Cambios

| Fecha | Cambio | Autor |
|-------|--------|-------|
| 2026-06-04 | Implementación inicial con transfer a extensión de tono | - |
| 2026-06-04 | Diagnóstico: 200 OK sin SDP por 100rel/PRACK | - |
| 2026-06-04 | Solución: Reproducir tono inline sin transfer | - |
| 2026-06-04 | Verificación exitosa de señalización | - |
