# Configuration Files for INTEGRATEL Carrier Integration

Working configurations for FreeSWITCH, Kamailio, and RTPEngine interconnection.

## Structure

```
config/
├── kamailio/
│   └── kamailio.cfg          # Kamailio SBC configuration
├── freeswitch/
│   ├── dialplan/
│   │   ├── from-pbx.xml      # Outbound/inbound call routing
│   │   ├── to-kamailio.xml   # Routes to Kamailio
│   │   └── public/           # Public context dialplan
│   ├── sip_profiles/
│   │   ├── internal.xml      # Internal SIP profile (:5080)
│   │   ├── external.xml      # External SIP profile (:5062)
│   │   └── external/gateways/
│   │       └── kamailio-internal.xml  # FS <-> Kamailio trunk
│   └── autoload_configs/
│       ├── acl.conf.xml      # Access control lists
│       ├── sofia.conf.xml    # SIP stack configuration
│       └── event_socket.conf.xml  # ESL configuration
└── rtpengine/
    └── rtpengine.conf        # RTPEngine media relay config
```

## Placeholders to Replace

Before deploying, replace these placeholders with your actual values:

| Placeholder | File | Description |
|-------------|------|-------------|
| `YOUR_KAMAILIO_DB_PASSWORD` | kamailio.cfg | MySQL password for Kamailio |
| `YOUR_DSIP_ID` | kamailio.cfg | dSIPRouter unique identifier |
| `YOUR_GATEWAY_PASSWORD` | fibertel-interno.xml, fibertelperu.xml | SIP trunk password |

## IP Addresses

Current configuration uses:
- **Internal IP**: 10.10.22.4 (FreeSWITCH/Kamailio)
- **External IP**: 10.124.194.142 (Public-facing)
- **RTPEngine**: 127.0.0.1:7722

## Port Mapping

| Service | Port | Protocol | Purpose |
|---------|------|----------|---------|
| Kamailio SIP | 5060 | UDP/TCP | SBC signaling |
| FreeSWITCH Internal | 5080 | UDP/TCP | Internal SIP profile |
| FreeSWITCH External | 5062 | UDP/TCP | External SIP profile |
| RTPEngine | 7722 | UDP | Control protocol |
| RTP Media | 10000-30000 | UDP | Media relay |

## Key Features

- **Bypass Media**: Enabled for direct RTP between endpoints
- **100rel**: PRACK support enabled for reliable provisional responses
- **Codec Inheritance**: Preserves original codec negotiation
- **SIP OPTIONS**: Health monitoring for trunk availability

## Carrier: INTEGRATEL

Configuration tested and working with INTEGRATEL carrier interconnection.
