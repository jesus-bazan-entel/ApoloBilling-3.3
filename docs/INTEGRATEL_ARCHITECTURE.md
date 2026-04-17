# Arquitectura de Integración: FreeSWITCH + Kamailio + RTPEngine

## Diagrama General de Red

```
                                    INTERNET / PSTN
                                          │
                                          │ SIP (UDP/TCP)
                                          ▼
┌─────────────────────────────────────────────────────────────────────────────────────────┐
│                              CARRIER: INTEGRATEL                                         │
│                           IP: 190.105.250.x (Inbound)                                   │
│                           IP: 172.16.1.25 (Outbound)                                    │
└─────────────────────────────────────────────────────────────────────────────────────────┘
                                          │
                                          │ SIP :5060
                                          ▼
┌─────────────────────────────────────────────────────────────────────────────────────────┐
│                                                                                         │
│   ┌───────────────────────────────────────────────────────────────────────────────┐     │
│   │                         KAMAILIO / dSIPRouter                                 │     │
│   │                              :5060 (SBC)                                      │     │
│   │                                                                               │     │
│   │   ┌─────────────────┐    ┌─────────────────┐    ┌─────────────────┐          │     │
│   │   │   dr_gateways   │    │   dr_gw_lists   │    │    dr_rules     │          │     │
│   │   │    (type=8)     │    │   (failover)    │    │  (prefix match) │          │     │
│   │   │                 │    │                 │    │                 │          │     │
│   │   │ INTEGRATEL_IN   │    │ setid=13 (IN)   │    │ 54xx → setid=14 │          │     │
│   │   │ INTEGRATEL_OUT  │    │ setid=14 (OUT)  │    │ 011x → setid=14 │          │     │
│   │   └─────────────────┘    └─────────────────┘    └─────────────────┘          │     │
│   │                                                                               │     │
│   │   ┌─────────────────┐                                                        │     │
│   │   │   dr_gateways   │◄─── PBX Interno (FreeSWITCH)                           │     │
│   │   │    (type=9)     │     gwid=100 → 127.0.0.1:5080                          │     │
│   │   │   gwid=100      │                                                        │     │
│   │   └─────────────────┘                                                        │     │
│   │                                                                               │     │
│   │   Módulos: drouting, dispatcher, rtpengine, nathelper, permissions           │     │
│   │                                                                               │     │
│   └───────────────────────────────────────────────────────────────────────────────┘     │
│                          │                              │                               │
│                          │ SIP :5080                    │ RTP Control                   │
│                          ▼                              ▼                               │
│   ┌───────────────────────────────────────┐    ┌──────────────────────────────────┐    │
│   │           FREESWITCH                  │    │          RTPENGINE               │    │
│   │                                       │    │                                  │    │
│   │  ┌─────────────┐  ┌─────────────┐    │    │   Control: 127.0.0.1:7722        │    │
│   │  │  Internal   │  │  External   │    │    │   CLI:     127.0.0.1:9900        │    │
│   │  │   :5080     │  │   :5062     │    │    │                                  │    │
│   │  │             │  │             │    │    │   ┌────────────────────────┐     │    │
│   │  │ context:    │  │ context:    │    │    │   │     Interfaces         │     │    │
│   │  │ from-pbx    │  │ public      │    │    │   │                        │     │    │
│   │  │             │  │             │    │    │   │ internal/10.10.22.4    │     │    │
│   │  │ Recibe de   │  │ Gateway     │    │    │   │ external/10.124.194.142│     │    │
│   │  │ Kamailio    │  │ kamailio-   │    │    │   │                        │     │    │
│   │  │             │  │ internal    │    │    │   │ Ports: 10000-30000     │     │    │
│   │  └─────────────┘  └─────────────┘    │    │   └────────────────────────┘     │    │
│   │                                       │    │                                  │    │
│   │  mod_xml_curl → API :8000/directory  │    │   Threads: 8                     │    │
│   │  mod_sofia, mod_dptools              │    │   Timeout: 60s                   │    │
│   │                                       │    │                                  │    │
│   └───────────────────────────────────────┘    └──────────────────────────────────┘    │
│                          │                                      │                       │
│                          │                                      │                       │
│                          ▼                                      ▼                       │
│   ┌───────────────────────────────────────────────────────────────────────────────┐    │
│   │                              RUST BACKEND API                                  │    │
│   │                                  :8000                                         │    │
│   │                                                                               │    │
│   │   /api/v1/freeswitch/directory  ←── Autenticación dinámica SIP               │    │
│   │   /api/v1/routing/*             ←── Gestión de trunks/rutas                  │    │
│   │   /api/v1/sip-devices/*         ←── Gestión de dispositivos                  │    │
│   │                                                                               │    │
│   └───────────────────────────────────────────────────────────────────────────────┘    │
│                                                                                         │
│                                   SERVIDOR: 10.10.22.4                                  │
│                              (IP Externa: 10.124.194.142)                               │
└─────────────────────────────────────────────────────────────────────────────────────────┘
```

## Flujo de Llamada Entrante (PSTN → Extensión)

```
┌──────────────┐      ┌──────────────┐      ┌──────────────┐      ┌──────────────┐
│  INTEGRATEL  │      │   KAMAILIO   │      │  FREESWITCH  │      │  EXTENSIÓN   │
│   Carrier    │      │    :5060     │      │    :5080     │      │   (1001)     │
└──────┬───────┘      └──────┬───────┘      └──────┬───────┘      └──────┬───────┘
       │                     │                     │                     │
       │ 1. INVITE           │                     │                     │
       │ To: +5411XXXX       │                     │                     │
       │────────────────────>│                     │                     │
       │                     │                     │                     │
       │                     │ 2. Lookup           │                     │
       │                     │    dr_gateways      │                     │
       │                     │    type=9 (PBX)     │                     │
       │                     │                     │                     │
       │                     │ 3. INVITE           │                     │
       │                     │ (con RTPEngine)     │                     │
       │                     │────────────────────>│                     │
       │                     │                     │                     │
       │                     │                     │ 4. Lookup           │
       │                     │                     │    dialplan         │
       │                     │                     │    from-pbx         │
       │                     │                     │                     │
       │                     │                     │ 5. INVITE           │
       │                     │                     │────────────────────>│
       │                     │                     │                     │
       │                     │                     │ 6. 200 OK           │
       │                     │                     │<────────────────────│
       │                     │                     │                     │
       │                     │ 7. 200 OK           │                     │
       │                     │<────────────────────│                     │
       │                     │                     │                     │
       │ 8. 200 OK           │                     │                     │
       │<────────────────────│                     │                     │
       │                     │                     │                     │
       │ ════════════════════╪═════════════════════╪═════════════════════│
       │                     │      RTP MEDIA      │                     │
       │                     │   (via RTPEngine)   │                     │
       │ ════════════════════╪═════════════════════╪═════════════════════│
       │                     │                     │                     │
```

## Flujo de Llamada Saliente (Extensión → PSTN)

```
┌──────────────┐      ┌──────────────┐      ┌──────────────┐      ┌──────────────┐
│  EXTENSIÓN   │      │  FREESWITCH  │      │   KAMAILIO   │      │  INTEGRATEL  │
│   (1001)     │      │    :5080     │      │    :5060     │      │   Carrier    │
└──────┬───────┘      └──────┬───────┘      └──────┬───────┘      └──────┬───────┘
       │                     │                     │                     │
       │ 1. INVITE           │                     │                     │
       │ To: 541156789012    │                     │                     │
       │────────────────────>│                     │                     │
       │                     │                     │                     │
       │                     │ 2. Dialplan         │                     │
       │                     │    from-pbx         │                     │
       │                     │    bypass_media=true│                     │
       │                     │                     │                     │
       │                     │ 3. Bridge via       │                     │
       │                     │    gateway          │                     │
       │                     │    kamailio-internal│                     │
       │                     │────────────────────>│                     │
       │                     │                     │                     │
       │                     │                     │ 4. drouting         │
       │                     │                     │    prefix match     │
       │                     │                     │    5411 → setid=14  │
       │                     │                     │                     │
       │                     │                     │ 5. RTPEngine        │
       │                     │                     │    offer/answer     │
       │                     │                     │                     │
       │                     │                     │ 6. INVITE           │
       │                     │                     │────────────────────>│
       │                     │                     │                     │
       │                     │                     │ 7. 183 Progress     │
       │                     │                     │<────────────────────│
       │                     │                     │                     │
       │                     │ 8. 183 Progress     │                     │
       │                     │<────────────────────│                     │
       │                     │                     │                     │
       │ 9. 183 Progress     │                     │                     │
       │<────────────────────│                     │                     │
       │                     │                     │                     │
       │                     │                     │ 10. 200 OK          │
       │                     │                     │<────────────────────│
       │                     │                     │                     │
       │ ════════════════════╪═════════════════════╪═════════════════════│
       │                     │      RTP MEDIA      │                     │
       │                     │   (via RTPEngine)   │                     │
       │ ════════════════════╪═════════════════════╪═════════════════════│
       │                     │                     │                     │
```

## Flujo de RTP Media

```
                          SIN RTPEngine (bypass_media=true interno)
┌─────────────────────────────────────────────────────────────────────────────────────┐
│                                                                                     │
│   ┌────────────┐                                              ┌────────────┐        │
│   │ Extensión  │◄════════════════ RTP DIRECTO ═══════════════►│ FreeSWITCH │        │
│   │   1001     │              (mismo servidor)                │            │        │
│   └────────────┘                                              └────────────┘        │
│                                                                                     │
└─────────────────────────────────────────────────────────────────────────────────────┘


                          CON RTPEngine (llamadas externas)
┌─────────────────────────────────────────────────────────────────────────────────────┐
│                                                                                     │
│   ┌────────────┐         ┌────────────┐         ┌────────────┐         ┌─────────┐ │
│   │ INTEGRATEL │◄═══════►│  RTPEngine │◄═══════►│  Kamailio  │◄═══════►│   FS    │ │
│   │  Carrier   │   RTP   │            │   RTP   │   :5060    │   SIP   │  :5080  │ │
│   │            │         │ 10000-30000│         │            │         │         │ │
│   └────────────┘         └────────────┘         └────────────┘         └─────────┘ │
│                                                                                     │
│   Interfaz externa:      Interfaz interna:                                          │
│   10.124.194.142         10.10.22.4                                                 │
│                                                                                     │
└─────────────────────────────────────────────────────────────────────────────────────┘
```

## Configuración de Puertos

```
┌─────────────────────────────────────────────────────────────────────────────────────┐
│                              MAPA DE PUERTOS                                         │
├─────────────────────────────────────────────────────────────────────────────────────┤
│                                                                                     │
│   KAMAILIO                                                                          │
│   ├── :5060/UDP,TCP ─── SIP Signaling (SBC principal)                              │
│   ├── :5061/TLS ─────── SIP sobre TLS                                              │
│   ├── :4443/WSS ─────── WebSocket Secure (WebRTC)                                  │
│   └── :5090/UDP ─────── DMQ (clustering)                                           │
│                                                                                     │
│   FREESWITCH                                                                        │
│   ├── :5080/UDP,TCP ─── Internal Profile (desde Kamailio/extensiones)              │
│   ├── :5062/UDP,TCP ─── External Profile (hacia Kamailio)                          │
│   └── :8021/TCP ─────── Event Socket Layer (ESL)                                   │
│                                                                                     │
│   RTPENGINE                                                                         │
│   ├── :7722/UDP ─────── Control Protocol (ng)                                      │
│   ├── :9900/TCP ─────── CLI                                                        │
│   └── :10000-30000/UDP ─ RTP Media Ports                                           │
│                                                                                     │
│   BACKEND API                                                                       │
│   └── :8000/TCP ─────── REST API + WebSocket                                       │
│                                                                                     │
│   BILLING ENGINE                                                                    │
│   └── :9000/TCP ─────── Real-time billing API                                      │
│                                                                                     │
└─────────────────────────────────────────────────────────────────────────────────────┘
```

## Tablas de Kamailio (dr_*)

```
┌─────────────────────────────────────────────────────────────────────────────────────┐
│                              dr_gateways                                             │
├──────┬─────────────────┬──────────────────┬──────┬───────────────────────────────────┤
│ gwid │      name       │     address      │ type │           description             │
├──────┼─────────────────┼──────────────────┼──────┼───────────────────────────────────┤
│  1   │ INTEGRATEL_IN   │ 190.105.250.x    │  8   │ Carrier inbound                   │
│  2   │ INTEGRATEL_OUT  │ 172.16.1.25      │  8   │ Carrier outbound                  │
│ 100  │ FreeSWITCH      │ 127.0.0.1:5080   │  9   │ PBX interno                       │
└──────┴─────────────────┴──────────────────┴──────┴───────────────────────────────────┘

┌─────────────────────────────────────────────────────────────────────────────────────┐
│                              dr_gw_lists                                             │
├───────┬─────────────────┬────────────────────────────────────────────────────────────┤
│ setid │      name       │                    gateways                                │
├───────┼─────────────────┼────────────────────────────────────────────────────────────┤
│  13   │ INTEGRATEL_IN   │ 1                                                          │
│  14   │ INTEGRATEL_OUT  │ 2                                                          │
│ 100   │ PBX_FREESWITCH  │ 100                                                        │
└───────┴─────────────────┴────────────────────────────────────────────────────────────┘

┌─────────────────────────────────────────────────────────────────────────────────────┐
│                              dr_rules                                                │
├─────────┬──────────┬─────────┬───────────────────────────────────────────────────────┤
│ ruleid  │  prefix  │ gwlist  │                    description                        │
├─────────┼──────────┼─────────┼───────────────────────────────────────────────────────┤
│    1    │    54    │   14    │ Argentina (todos los números)                         │
│    2    │   011    │   14    │ Buenos Aires (código de área)                         │
│    3    │    0     │   14    │ Llamadas nacionales                                   │
└─────────┴──────────┴─────────┴───────────────────────────────────────────────────────┘
```

## Contextos de FreeSWITCH

```
┌─────────────────────────────────────────────────────────────────────────────────────┐
│                           DIALPLAN CONTEXTS                                          │
├─────────────────────────────────────────────────────────────────────────────────────┤
│                                                                                     │
│   from-pbx (Internal Profile :5080)                                                 │
│   ├── Recibe llamadas de Kamailio                                                  │
│   ├── Recibe llamadas de extensiones registradas                                   │
│   │                                                                                 │
│   ├── Extension: inbound-fibertelperu                                              │
│   │   └── Carrier → bridge a Kamailio (bypass_media=true)                          │
│   │                                                                                 │
│   ├── Extension: local-extensions                                                  │
│   │   └── Si usuario registrado → bridge directo                                   │
│   │                                                                                 │
│   └── Extension: outbound-to-kamailio                                              │
│       └── Default → bridge a Kamailio:5060 (bypass_media=true)                     │
│                                                                                     │
│   to-kamailio (rutas hacia Kamailio)                                               │
│   └── Gateway: kamailio-internal                                                   │
│       └── 10.10.22.4:5080 (sin autenticación)                                      │
│                                                                                     │
│   public (External Profile :5062)                                                   │
│   └── Llamadas entrantes directas (no usadas con Kamailio)                         │
│                                                                                     │
└─────────────────────────────────────────────────────────────────────────────────────┘
```

## Flags y Configuraciones Importantes

```
┌─────────────────────────────────────────────────────────────────────────────────────┐
│                           CONFIGURACIONES CLAVE                                      │
├─────────────────────────────────────────────────────────────────────────────────────┤
│                                                                                     │
│   FreeSWITCH (from-pbx.xml)                                                         │
│   ├── bypass_media=true ──────── RTP no pasa por FreeSWITCH                        │
│   ├── inherit_codec=true ─────── Preserva codec original                           │
│   ├── sip_enable_100rel=true ─── Soporte PRACK (early media confiable)             │
│   └── call_timeout=30 ────────── Timeout de ring                                   │
│                                                                                     │
│   Kamailio (kamailio.cfg)                                                           │
│   ├── WITH_RTPENGINE ─────────── RTPEngine habilitado                              │
│   ├── WITH_NAT ───────────────── NAT traversal                                     │
│   ├── WITH_DROUTE ────────────── Dynamic routing                                   │
│   ├── WITH_TLS ───────────────── TLS habilitado                                    │
│   └── WITH_WEBSOCKETS ────────── WebRTC support                                    │
│                                                                                     │
│   RTPEngine (rtpengine.conf)                                                        │
│   ├── interface = internal/10.10.22.4;external/10.124.194.142                      │
│   ├── port-min = 10000                                                             │
│   ├── port-max = 30000                                                             │
│   ├── timeout = 60 ───────────── RTP timeout                                       │
│   └── silent-timeout = 3600 ──── Timeout sin actividad                             │
│                                                                                     │
└─────────────────────────────────────────────────────────────────────────────────────┘
```

## Comandos de Diagnóstico

```bash
# Kamailio
kamcmd drouting.reload              # Recargar rutas
kamcmd dispatcher.list              # Ver dispatchers
kamctl fifo ds_list                 # Estado de gateways
kamcmd rtpengine.show all           # Estado de RTPEngine

# FreeSWITCH
fs_cli -x "sofia status"            # Estado de perfiles SIP
fs_cli -x "sofia status profile internal"
fs_cli -x "show calls"              # Llamadas activas
fs_cli -x "sofia global siptrace on" # Debug SIP

# RTPEngine
rtpengine-ctl list sessions         # Sesiones activas
rtpengine-ctl list totals           # Estadísticas
```
