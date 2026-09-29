# PRD Tecnico: Modulos SBC Simplificados

**Version:** 2.0
**Fecha:** 2026-08-16
**Autor:** ApoloBilling Team
**Estado:** Implementado y Verificado

---

## Indice

1. [Resumen Ejecutivo](#1-resumen-ejecutivo)
2. [Arquitectura General](#2-arquitectura-general)
3. [Dispositivos SIP](#3-dispositivos-sip)
4. [Rutas Entrantes](#4-rutas-entrantes)
5. [Rutas Salientes](#5-rutas-salientes)
6. [Carriers](#6-carriers)
7. [Carrier Groups](#7-carrier-groups)
8. [Endpoints](#8-endpoints)
9. [Monitor de Endpoints](#9-monitor-de-endpoints)
10. [Migraciones de Base de Datos](#10-migraciones-de-base-de-datos)
11. [Guia de Implementacion](#11-guia-de-implementacion)

---

## 1. Resumen Ejecutivo

Este documento describe la implementacion simplificada de los modulos de enrutamiento SBC de ApoloBilling. El sistema ha sido rediseñado para eliminar complejidad innecesaria mediante:

- **Auto-prioridad**: Patrones/prefijos mas largos ganan automaticamente (sin prioridad manual)
- **Modelo unificado**: Una sola tabla `inbound_routes` reemplaza `routing_carriers` + `routing_did_overrides`
- **Sincronizacion automatica**: Cambios se propagan a FreeSWITCH/Kamailio automaticamente
- **Monitoreo integrado**: SIP OPTIONS para verificar conectividad de carriers

### Stack Tecnologico

| Componente | Tecnologia |
|------------|------------|
| Backend API | Rust + Actix-web 4 |
| Base de Datos | PostgreSQL 15 (principal) + MySQL (Kamailio) |
| Frontend | React 19 + TypeScript + TanStack Query |
| PBX | FreeSWITCH 1.10 |
| SBC | Kamailio 5.8 |
| Media | RTPEngine 11.5 |

---

## 2. Arquitectura General

```
┌─────────────────────────────────────────────────────────────────────┐
│                        React Frontend (:3000)                        │
│   InboundRouting | OutboundRouting | Carriers | EndpointMonitor      │
└──────────────────────────────┬──────────────────────────────────────┘
                               │ REST API
                               ▼
┌─────────────────────────────────────────────────────────────────────┐
│                      Rust Backend (:8000)                            │
│                                                                      │
│  ┌──────────────┐  ┌──────────────┐  ┌──────────────┐               │
│  │   Handlers   │  │    DTOs      │  │   Services   │               │
│  │              │  │              │  │              │               │
│  │ inbound_     │  │ inbound.rs   │  │ sip_device   │               │
│  │ routing.rs   │  │ outbound.rs  │  │ _service.rs  │               │
│  │              │  │ carriers.rs  │  │              │               │
│  │ outbound_    │  │ endpoints.rs │  │              │               │
│  │ routing.rs   │  │              │  │              │               │
│  │              │  │              │  │              │               │
│  │ carriers.rs  │  │              │  │              │               │
│  │ endpoints.rs │  │              │  │              │               │
│  └──────┬───────┘  └──────────────┘  └──────────────┘               │
│         │                                                            │
│         ▼                                                            │
│  ┌──────────────────────────────────────────────────────────┐       │
│  │                     SQLx Queries                          │       │
│  │   PostgreSQL (apolo_billing)  │  MySQL (kamailio)         │       │
│  └──────────────────────────────────────────────────────────┘       │
└─────────────────────────────────────────────────────────────────────┘
          │                                      │
          ▼                                      ▼
┌──────────────────┐                   ┌──────────────────┐
│   FreeSWITCH     │                   │    Kamailio      │
│                  │                   │                  │
│ to-kamailio.xml  │◄─── XML Gen ─────│  dr_gateways     │
│ (dialplan)       │                   │  dr_gw_lists     │
│                  │                   │  dr_rules        │
└──────────────────┘                   └──────────────────┘
```

### Flujo de Sincronizacion

1. **Inbound Routes → FreeSWITCH**
   - Backend genera `/etc/freeswitch/dialplan/to-kamailio.xml`
   - Ejecuta `fs_cli -x "reloadxml"`

2. **Carriers/Outbound → Kamailio**
   - Backend escribe en tablas MySQL (dr_gateways, dr_gw_lists, dr_rules)
   - Ejecuta `kamcmd drouting.reload`

---

## 3. Dispositivos SIP

### 3.1 Modelo de Datos

```sql
CREATE TABLE sip_devices (
    id SERIAL PRIMARY KEY,
    sip_username VARCHAR(64) NOT NULL,
    sip_domain VARCHAR(128) NOT NULL DEFAULT 'apolo.local',
    display_name VARCHAR(100),
    account_id INT REFERENCES accounts(id),
    context VARCHAR(50) DEFAULT 'from-pbx',
    accountcode VARCHAR(50),
    codecs VARCHAR(255) DEFAULT 'PCMA,PCMU,G729',
    max_registrations INT DEFAULT 3,

    -- Seguridad (password encriptado AES-256)
    password_hash BYTEA NOT NULL,
    password_nonce BYTEA NOT NULL,
    a1_hash VARCHAR(64) NOT NULL,  -- MD5(username:realm:password)

    enabled BOOLEAN DEFAULT true,
    description TEXT,
    created_at TIMESTAMPTZ DEFAULT NOW(),
    updated_at TIMESTAMPTZ DEFAULT NOW(),

    UNIQUE(sip_username, sip_domain)
);

CREATE TABLE freeswitch_allowed_ips (
    id SERIAL PRIMARY KEY,
    ip_address VARCHAR(45) NOT NULL UNIQUE,
    description TEXT,
    created_at TIMESTAMPTZ DEFAULT NOW()
);
```

### 3.2 API Endpoints

| Metodo | Endpoint | Descripcion |
|--------|----------|-------------|
| GET | `/api/v1/sip-devices` | Listar dispositivos |
| POST | `/api/v1/sip-devices` | Crear dispositivo |
| GET | `/api/v1/sip-devices/{id}` | Obtener dispositivo |
| PUT | `/api/v1/sip-devices/{id}` | Actualizar dispositivo |
| DELETE | `/api/v1/sip-devices/{id}` | Eliminar dispositivo |
| GET | `/api/v1/sip-devices/{id}/password` | Ver password (admin) |
| POST | `/api/v1/sip-devices/{id}/regenerate-password` | Regenerar password |
| GET | `/api/v1/sip-devices/{id}/registration-status` | Estado de registro |
| GET | `/api/v1/sip-devices/allowed-ips` | Listar IPs permitidas |
| POST | `/api/v1/sip-devices/allowed-ips` | Agregar IP |
| DELETE | `/api/v1/sip-devices/allowed-ips/{id}` | Eliminar IP |

### 3.3 Flujo de Autenticacion

```
┌──────────────┐      REGISTER       ┌──────────────┐
│  SIP Device  │ ──────────────────► │  FreeSWITCH  │
│  (Softphone) │                     │  :5080       │
└──────────────┘                     └──────┬───────┘
                                            │ POST /api/v1/freeswitch/directory
                                            ▼
                                     ┌──────────────┐
                                     │ Rust Backend │
                                     │    :8000     │
                                     └──────┬───────┘
                                            │ 1. Validar IP whitelist
                                            │ 2. Buscar sip_device
                                            │ 3. Retornar XML con A1 hash
                                            ▼
                                     ┌──────────────────────────────┐
                                     │ <document type="freeswitch"> │
                                     │   <section name="directory"> │
                                     │     <domain name="...">      │
                                     │       <user id="...">        │
                                     │         <params>             │
                                     │           <param name="a1-   │
                                     │             hash" .../>      │
                                     │         </params>            │
                                     │       </user>                │
                                     │     </domain>                │
                                     │   </section>                 │
                                     │ </document>                  │
                                     └──────────────────────────────┘
```

### 3.4 Calculo de A1 Hash

```rust
fn calculate_a1_hash(username: &str, realm: &str, password: &str) -> String {
    let input = format!("{}:{}:{}", username, realm, password);
    format!("{:x}", md5::compute(input))
}
// Ejemplo: MD5("1001:apolo.local:secretpass") = "a7b3c4d5e6f7..."
```

---

## 4. Rutas Entrantes

### 4.1 Concepto de Auto-Prioridad

El sistema simplificado elimina la necesidad de configurar prioridades manualmente. La prioridad se determina automaticamente por la longitud del patron:

```
Patron mas largo = Mayor prioridad

Ejemplos ordenados por prioridad:
1. "672017402000"  (12 digitos) - Match exacto, maxima prioridad
2. "6720174*"      (7 digitos)  - Wildcard mas especifico
3. "6720*"         (4 digitos)  - Wildcard general
```

### 4.2 Modelo de Datos

```sql
CREATE TABLE inbound_routes (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    name VARCHAR(100) NOT NULL,
    description TEXT,

    -- Patron de matching (UNICO para auto-prioridad)
    pattern VARCHAR(30) NOT NULL UNIQUE,  -- "672017402000" o "6720*"
    source_ip VARCHAR(255),               -- Filtro IP opcional

    -- Destino
    destination_type VARCHAR(20) DEFAULT 'extension'
        CHECK (destination_type IN ('extension', 'gateway')),
    gateway_id UUID REFERENCES routing_trunks(id) ON DELETE SET NULL,
    destination_host VARCHAR(255) DEFAULT '10.10.22.4',

    -- Transformacion de numero
    strip_digits INT DEFAULT 4,           -- Digitos a quitar
    prefix_to_add VARCHAR(20) DEFAULT '', -- Prefijo a agregar

    -- Opciones de señalizacion
    call_timeout INT DEFAULT 55,          -- Timeout en segundos
    ims_mode BOOLEAN DEFAULT false,       -- 100rel/PRACK
    early_media BOOLEAN DEFAULT false,    -- 183 Session Progress

    enabled BOOLEAN DEFAULT true,
    sync_status VARCHAR(20) DEFAULT 'pending',
    sync_error TEXT,
    created_at TIMESTAMPTZ DEFAULT NOW(),
    updated_at TIMESTAMPTZ DEFAULT NOW()
);

-- Indice para auto-prioridad
CREATE INDEX idx_inbound_routes_pattern_len
    ON inbound_routes(LENGTH(pattern) DESC, pattern);
```

### 4.3 API Endpoints

| Metodo | Endpoint | Descripcion |
|--------|----------|-------------|
| GET | `/api/v1/inbound-routing` | Listar rutas (ordenadas por prioridad) |
| POST | `/api/v1/inbound-routing` | Crear ruta + sync FS |
| GET | `/api/v1/inbound-routing/{id}` | Obtener ruta |
| PUT | `/api/v1/inbound-routing/{id}` | Actualizar ruta + sync FS |
| DELETE | `/api/v1/inbound-routing/{id}` | Eliminar ruta + sync FS |
| POST | `/api/v1/inbound-routing/sync` | Sincronizar todas a FS |

### 4.4 DTOs

```rust
// Request para crear ruta entrante
pub struct CreateInboundRouteRequest {
    pub name: String,
    pub description: Option<String>,
    pub pattern: String,                    // "6720*" o "672017402000"
    pub source_ip: Option<String>,
    pub destination_type: String,           // "extension" | "gateway"
    pub gateway_id: Option<Uuid>,
    pub destination_host: String,           // Default: "10.10.22.4"
    pub strip_digits: i32,                  // Default: 4
    pub prefix_to_add: String,              // Default: ""
    pub call_timeout: i32,                  // Default: 55
    pub ims_mode: bool,                     // Default: false
    pub early_media: bool,                  // Default: false
    pub enabled: bool,                      // Default: true
}

// Response de ruta entrante
pub struct InboundRouteResponse {
    pub id: Uuid,
    pub name: String,
    pub description: Option<String>,
    pub pattern: String,
    pub is_wildcard: bool,                  // Calculado: pattern.ends_with('*')
    pub source_ip: Option<String>,
    pub destination_type: String,
    pub gateway_id: Option<Uuid>,
    pub gateway_name: Option<String>,       // JOIN con routing_trunks
    pub destination_host: String,
    pub strip_digits: i32,
    pub prefix_to_add: String,
    pub call_timeout: i32,
    pub ims_mode: bool,
    pub early_media: bool,
    pub enabled: bool,
    pub sync_status: String,                // "pending" | "synced" | "error"
    pub sync_error: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}
```

### 4.5 Generacion de Dialplan FreeSWITCH

El handler genera automaticamente `/etc/freeswitch/dialplan/to-kamailio.xml`:

```xml
<?xml version="1.0" encoding="utf-8"?>
<!-- Auto-generated by ApoloBilling Inbound Routing System -->
<include>
  <context name="to-kamailio">

    <!-- Inbound: DID Especifico (exact) -->
    <extension name="did-672017402000">
      <condition field="destination_number" expression="^(672017402000)$">
        <action application="log" data="INFO Inbound DID-672017402000: ${caller_id_number} -> ${destination_number}"/>
        <action application="set" data="original_destination=${destination_number}"/>
        <action application="set" data="transformed_number=${destination_number:4}"/>
        <action application="set" data="call_timeout=55"/>
        <action application="set" data="originate_timeout=55"/>
        <action application="set" data="inherit_codec=true"/>
        <action application="bridge" data="user/${transformed_number}@10.10.22.4"/>
      </condition>
    </extension>

    <!-- Inbound: ENTEL Principal (wildcard) -->
    <extension name="inbound-entel-principal">
      <condition field="destination_number" expression="^(6720.+)$">
        <action application="log" data="INFO Inbound ENTEL: ${caller_id_number} -> ${destination_number}"/>
        <action application="set" data="original_destination=${destination_number}"/>
        <action application="set" data="transformed_number=${destination_number:4}"/>
        <action application="set" data="call_timeout=55"/>
        <action application="set" data="originate_timeout=55"/>
        <!-- IMS Mode -->
        <action application="set" data="progress_timeout=55"/>
        <action application="set" data="sip_enable_100rel=true"/>
        <action application="export" data="sip_enable_100rel=true"/>
        <action application="set" data="bypass_media=false"/>
        <action application="set" data="hangup_after_bridge=false"/>
        <action application="set" data="continue_on_fail=true"/>
        <action application="set" data="inherit_codec=true"/>
        <action application="bridge" data="user/${transformed_number}@10.10.22.4"/>
        <action application="transfer" data="_handle_bridge_failure_"/>
      </condition>
    </extension>

    <!-- IMS Failure Handlers -->
    <extension name="hangup-timeout">
      <condition field="destination_number" expression="^_hangup_timeout_$">
        <action application="playback" data="tone_stream://%(500,500,480,620);loops=7"/>
        <action application="respond" data="486 Busy Here"/>
      </condition>
    </extension>

    <!-- Fallback -->
    <extension name="fallback-reject">
      <condition field="destination_number" expression="^(.+)$">
        <action application="log" data="WARNING Rejecting unknown: ${destination_number}"/>
        <action application="respond" data="404 Not Found"/>
      </condition>
    </extension>

  </context>
</include>
```

### 4.6 Transformacion de Numero

```
Entrada:        672017402002
Pattern:        6720*
Strip Digits:   4
Prefix to Add:  +51

Proceso:
1. Match: 672017402002 coincide con ^(6720.+)$
2. Strip:  672017402002 → 17402002 (quita 4 digitos)
3. Prefix: 17402002 → +5117402002 (agrega prefijo)

Resultado:      +5117402002
```

---

## 5. Rutas Salientes

### 5.1 Modelo de Datos

```sql
CREATE TABLE outbound_routes (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    name VARCHAR(100) NOT NULL,
    description TEXT,

    -- Prefijo de matching (UNICO)
    prefix VARCHAR(30) NOT NULL UNIQUE,

    -- Destino: grupo O carrier individual (mutuamente excluyentes)
    carrier_group_id UUID REFERENCES carrier_groups(id) ON DELETE SET NULL,
    carrier_id UUID REFERENCES carriers(id) ON DELETE SET NULL,

    -- Kamailio sync
    kamailio_ruleid INT,                    -- dr_rules.ruleid
    sync_status VARCHAR(20) DEFAULT 'pending',
    sync_error TEXT,

    enabled BOOLEAN DEFAULT true,
    created_at TIMESTAMPTZ DEFAULT NOW(),
    updated_at TIMESTAMPTZ DEFAULT NOW(),

    CONSTRAINT outbound_routes_dest_check CHECK (
        (carrier_group_id IS NOT NULL AND carrier_id IS NULL) OR
        (carrier_group_id IS NULL AND carrier_id IS NOT NULL)
    )
);

-- Auto-prioridad por longitud de prefijo
CREATE INDEX idx_outbound_routes_prefix_len
    ON outbound_routes(LENGTH(prefix) DESC, prefix);
```

### 5.2 API Endpoints

| Metodo | Endpoint | Descripcion |
|--------|----------|-------------|
| GET | `/api/v1/outbound-routing` | Listar rutas con stats |
| POST | `/api/v1/outbound-routing` | Crear ruta + sync Kamailio |
| GET | `/api/v1/outbound-routing/{id}` | Obtener ruta |
| PUT | `/api/v1/outbound-routing/{id}` | Actualizar ruta |
| DELETE | `/api/v1/outbound-routing/{id}` | Eliminar ruta |
| POST | `/api/v1/outbound-routing/sync` | Sincronizar todas |
| POST | `/api/v1/kamailio/sync-all` | Sync carriers+groups+routes |
| POST | `/api/v1/kamailio/reload` | Recargar drouting |

### 5.3 DTOs

```rust
pub struct OutboundRoute {
    pub id: Uuid,
    pub name: String,
    pub description: Option<String>,
    pub prefix: String,
    pub carrier_group_id: Option<Uuid>,
    pub carrier_id: Option<Uuid>,
    pub destination_name: Option<String>,    // Nombre del carrier/grupo
    pub destination_type: String,            // "group" | "carrier"
    pub kamailio_ruleid: Option<i32>,
    pub sync_status: SyncStatus,
    pub sync_error: Option<String>,
    pub enabled: bool,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

pub struct OutboundRouteListResponse {
    pub routes: Vec<OutboundRoute>,
    pub total: i64,
    pub enabled_count: i64,
    pub synced_count: i64,
}
```

### 5.4 Sincronizacion a Kamailio

```rust
// Logica de sync a dr_rules
async fn sync_outbound_route_to_kamailio(pool: &PgPool, route_id: Uuid) {
    // 1. Obtener ruta con carrier/grupo
    let route = fetch_route(route_id);

    // 2. Determinar gwlist
    let gwlist = if let Some(group_id) = route.kamailio_group_id {
        format!("#{}", group_id)  // Referencia a dr_gw_lists
    } else if let Some(gwid) = route.kamailio_gwid {
        gwid.to_string()          // Gateway directo
    };

    // 3. Calcular prioridad (prefijo mas largo = menor numero = mayor prioridad)
    let priority = 100 - route.prefix.len() as i32;

    // 4. Insertar/actualizar en MySQL
    if route.kamailio_ruleid.is_some() {
        mysql_query("UPDATE dr_rules SET prefix=?, gwlist=?, priority=? WHERE ruleid=?");
    } else {
        mysql_query("INSERT INTO dr_rules (groupid, prefix, gwlist, priority) VALUES (8000, ?, ?, ?)");
    }

    // 5. Actualizar PostgreSQL con ruleid
    pg_query("UPDATE outbound_routes SET kamailio_ruleid=?, sync_status='synced'");
}
```

---

## 6. Carriers

### 6.1 Modelo de Datos

```sql
CREATE TABLE carriers (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    name VARCHAR(100) NOT NULL UNIQUE,
    description TEXT,

    -- Conexion SIP
    host VARCHAR(255) NOT NULL,
    port INT DEFAULT 5060,
    transport VARCHAR(10) DEFAULT 'udp'
        CHECK (transport IN ('udp', 'tcp', 'tls')),

    -- Autenticacion (opcional)
    auth_username VARCHAR(100),
    auth_password VARCHAR(255),            -- Plaintext en DB

    -- Transformacion
    strip_digits INT DEFAULT 0,
    prefix_to_add VARCHAR(20) DEFAULT '',

    -- Kamailio sync
    kamailio_gwid INT,                     -- dr_gateways.gwid
    sync_status VARCHAR(20) DEFAULT 'pending',
    sync_error TEXT,

    -- Monitoreo SIP OPTIONS
    sip_status VARCHAR(20) DEFAULT 'unknown'
        CHECK (sip_status IN ('unknown', 'reachable', 'unreachable', 'checking')),
    last_options_check TIMESTAMPTZ,
    last_options_latency_ms INT,

    enabled BOOLEAN DEFAULT true,
    created_at TIMESTAMPTZ DEFAULT NOW(),
    updated_at TIMESTAMPTZ DEFAULT NOW()
);
```

### 6.2 API Endpoints

| Metodo | Endpoint | Descripcion |
|--------|----------|-------------|
| GET | `/api/v1/carriers` | Listar carriers con status |
| POST | `/api/v1/carriers` | Crear carrier + sync Kamailio |
| GET | `/api/v1/carriers/{id}` | Obtener carrier |
| PUT | `/api/v1/carriers/{id}` | Actualizar carrier |
| DELETE | `/api/v1/carriers/{id}` | Eliminar carrier |
| POST | `/api/v1/carriers/{id}/check-status` | Verificar SIP OPTIONS |
| POST | `/api/v1/carriers/sync` | Sincronizar todos a Kamailio |

### 6.3 Verificacion SIP OPTIONS

```rust
async fn check_sip_options(host: &str, port: i32) -> SipOptionsResult {
    let target = format!("sip:{}:{}", host, port);
    let start = Instant::now();

    // Ejecutar sipsak
    let output = Command::new("sipsak")
        .args(["-s", &target, "-v", "-W", "3"])
        .output()
        .await;

    let elapsed = start.elapsed().as_millis() as i32;

    match output {
        Ok(out) if out.status.success() => {
            // Parsear codigo de respuesta
            let response_code = parse_sip_response(&out.stdout);
            SipOptionsResult {
                status: SipStatus::Reachable,
                latency_ms: Some(elapsed),
                response_code,
                error: None,
            }
        }
        _ => SipOptionsResult {
            status: SipStatus::Unreachable,
            latency_ms: Some(elapsed),
            response_code: None,
            error: Some("Timeout or unreachable"),
        }
    }
}
```

### 6.4 Sincronizacion a Kamailio

Los carriers se sincronizan a `dr_gateways` con `type=8`:

```sql
-- Kamailio MySQL
INSERT INTO dr_gateways (type, address, strip, attrs, description)
VALUES (8, 'sip:10.35.13.100:5060', 0, 'auth_user', 'INTEGRATEL');
```

---

## 7. Carrier Groups

### 7.1 Modelo de Datos

```sql
CREATE TABLE carrier_groups (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    name VARCHAR(100) NOT NULL UNIQUE,
    description TEXT,

    -- Estrategia de failover
    strategy VARCHAR(20) DEFAULT 'sequential'
        CHECK (strategy IN ('sequential', 'round_robin', 'weighted')),

    -- Kamailio sync
    kamailio_group_id INT,                 -- dr_gw_lists.id
    sync_status VARCHAR(20) DEFAULT 'pending',
    sync_error TEXT,

    created_at TIMESTAMPTZ DEFAULT NOW(),
    updated_at TIMESTAMPTZ DEFAULT NOW()
);

CREATE TABLE carrier_group_members (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    group_id UUID NOT NULL REFERENCES carrier_groups(id) ON DELETE CASCADE,
    carrier_id UUID NOT NULL REFERENCES carriers(id) ON DELETE CASCADE,
    priority INT DEFAULT 1,                -- 1 = mas alta (se intenta primero)
    weight INT DEFAULT 100,                -- Para round_robin/weighted
    created_at TIMESTAMPTZ DEFAULT NOW(),

    UNIQUE(group_id, carrier_id)
);
```

### 7.2 Estrategias de Failover

| Estrategia | Descripcion |
|------------|-------------|
| `sequential` | Intenta carriers en orden de prioridad (1, 2, 3...) |
| `round_robin` | Distribuye llamadas equitativamente |
| `weighted` | Distribuye segun peso (weight) |

### 7.3 API Endpoints

| Metodo | Endpoint | Descripcion |
|--------|----------|-------------|
| GET | `/api/v1/carrier-groups` | Listar grupos con miembros |
| POST | `/api/v1/carrier-groups` | Crear grupo |
| GET | `/api/v1/carrier-groups/{id}` | Obtener grupo |
| PUT | `/api/v1/carrier-groups/{id}` | Actualizar grupo |
| DELETE | `/api/v1/carrier-groups/{id}` | Eliminar grupo |
| POST | `/api/v1/carrier-groups/{id}/members` | Agregar miembro |
| PUT | `/api/v1/carrier-groups/{gid}/members/{mid}` | Actualizar prioridad/peso |
| DELETE | `/api/v1/carrier-groups/{gid}/members/{mid}` | Quitar miembro |

### 7.4 Sincronizacion a Kamailio

```sql
-- Ejemplo: Grupo con 2 carriers (gwid 60 y 61)
-- PostgreSQL carrier_group_members:
--   carrier A: priority=1, kamailio_gwid=60
--   carrier B: priority=2, kamailio_gwid=61

-- Kamailio dr_gw_lists:
INSERT INTO dr_gw_lists (gwlist, description)
VALUES ('60,61', 'Grupo INTEGRATEL');  -- id=15

-- Kamailio dr_rules referencia al grupo:
INSERT INTO dr_rules (groupid, prefix, gwlist, priority)
VALUES (8000, '2267', '#15', 96);  -- #15 = referencia al grupo
```

---

## 8. Endpoints

### 8.1 Modelo de Datos

```sql
CREATE TABLE endpoints (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    name VARCHAR(100) NOT NULL UNIQUE,
    description TEXT,

    host VARCHAR(255) NOT NULL,            -- IP del PBX
    port INT DEFAULT 5060,
    transport VARCHAR(10) DEFAULT 'udp',

    -- FreeSWITCH profile
    fs_profile VARCHAR(50) DEFAULT 'internal',

    -- Kamailio sync (type=9 para PBX)
    kamailio_gwid INT,
    sync_status VARCHAR(20) DEFAULT 'pending',
    sync_error TEXT,

    enabled BOOLEAN DEFAULT true,
    is_default BOOLEAN DEFAULT false,      -- Solo 1 puede ser default
    created_at TIMESTAMPTZ DEFAULT NOW(),
    updated_at TIMESTAMPTZ DEFAULT NOW()
);

-- Constraint: solo 1 endpoint default
CREATE UNIQUE INDEX idx_endpoints_single_default
    ON endpoints (is_default) WHERE is_default = true;
```

### 8.2 Diferencia con Carriers

| Aspecto | Carriers (type=8) | Endpoints (type=9) |
|---------|-------------------|-------------------|
| Uso | Proveedores externos | PBX internos |
| Direccion | Hacia PSTN | Hacia extensiones |
| Monitoreo | SIP OPTIONS activo | Generalmente no |
| Autenticacion | Opcional | No requerida |

### 8.3 API Endpoints

| Metodo | Endpoint | Descripcion |
|--------|----------|-------------|
| GET | `/api/v1/sip-endpoints` | Listar endpoints |
| POST | `/api/v1/sip-endpoints` | Crear endpoint |
| GET | `/api/v1/sip-endpoints/{id}` | Obtener endpoint |
| PUT | `/api/v1/sip-endpoints/{id}` | Actualizar endpoint |
| DELETE | `/api/v1/sip-endpoints/{id}` | Eliminar endpoint |
| POST | `/api/v1/sip-endpoints/sync` | Sincronizar a Kamailio |

---

## 9. Monitor de Endpoints

### 9.1 Modelo de Datos

```sql
CREATE TABLE endpoint_monitor_probes (
    id BIGSERIAL PRIMARY KEY,
    ts BIGINT NOT NULL,                    -- Unix epoch UTC
    trunk_id UUID NOT NULL REFERENCES routing_trunks(id) ON DELETE CASCADE,
    trunk_name VARCHAR(128) NOT NULL DEFAULT '',
    trunk_type VARCHAR(20) NOT NULL DEFAULT 'public',
    ip VARCHAR(64) NOT NULL DEFAULT '',
    port SMALLINT NOT NULL DEFAULT 5060,
    status VARCHAR(10) NOT NULL DEFAULT 'down'
        CHECK (status IN ('up', 'down')),
    resp_code SMALLINT NOT NULL DEFAULT 0, -- 200/404/503/0
    rtt_ms DECIMAL(8,3),                   -- Round-trip time
    evidence TEXT                          -- Raw sipsak trace
);

-- Indices para consultas
CREATE INDEX idx_endpoint_monitor_ts ON endpoint_monitor_probes(ts);
CREATE INDEX idx_endpoint_monitor_trunk_ts ON endpoint_monitor_probes(trunk_id, ts);
CREATE INDEX idx_endpoint_monitor_ip_ts ON endpoint_monitor_probes(ip, ts);
CREATE INDEX idx_endpoint_monitor_status_ts ON endpoint_monitor_probes(status, ts);
```

### 9.2 API Endpoints

| Metodo | Endpoint | Descripcion |
|--------|----------|-------------|
| GET | `/api/v1/endpoint-status` | Dashboard con estado actual |
| POST | `/api/v1/endpoint-monitor/probe/{id}` | Ejecutar probe manual |
| GET | `/api/v1/endpoint-monitor/probe-results` | Historial de probes |
| GET | `/api/v1/endpoint-monitor/evidence/{id}` | Descargar evidencia |
| GET | `/api/v1/endpoint-monitor/report` | Descargar reporte |

### 9.3 Query Parameters para probe-results

| Parametro | Tipo | Descripcion |
|-----------|------|-------------|
| `from` | timestamp | Inicio del rango |
| `to` | timestamp | Fin del rango |
| `endpoint_name` | string | Filtrar por nombre |
| `ip` | string | Filtrar por IP |
| `status` | string | Filtrar por estado (up/down) |
| `limit` | int | Limite de resultados |

### 9.4 Servicio de Monitoreo

El servicio se ejecuta en background cada N segundos (configurable via `system_settings`):

```rust
// Configuracion en system_settings
// sip_options_enabled: true
// sip_options_interval: 30  // segundos

async fn probe_all_endpoints(pool: &PgPool) {
    let trunks = fetch_enabled_trunks();
    let ts = Utc::now().timestamp();

    for trunk in trunks {
        let result = check_sip_options(&trunk.host, trunk.port).await;

        insert_probe(
            trunk_id: trunk.id,
            trunk_name: trunk.name,
            ip: trunk.host,
            port: trunk.port,
            status: result.status,
            resp_code: result.response_code,
            rtt_ms: result.latency_ms,
            evidence: result.raw_output,
            ts: ts,
        ).await;
    }
}
```

---

## 10. Migraciones de Base de Datos

### 10.1 Orden de Ejecucion

```bash
# Ejecutar en orden
cd /opt/ApoloBilling/database/migrations

psql -U apolo_user -d apolo_billing -f 003_add_sip_devices.sql
psql -U apolo_user -d apolo_billing -f 004_unified_routing.sql
psql -U apolo_user -d apolo_billing -f 011_carrier_routing.sql
psql -U apolo_user -d apolo_billing -f 012_endpoint_monitor.sql
psql -U apolo_user -d apolo_billing -f 013_simplify_inbound_routing.sql
psql -U apolo_user -d apolo_billing -f 014_simplify_outbound_routing.sql
```

### 10.2 Rollback

Las tablas antiguas se mantienen con comentario DEPRECATED:

```sql
-- Para rollback, las tablas originales aun existen:
-- routing_carriers, routing_did_overrides, routing_inbound_routes
-- routing_trunks, routing_trunk_groups, routing_outbound_routes
```

---

## 11. Guia de Implementacion

### 11.1 Variables de Entorno Requeridas

```bash
# PostgreSQL (obligatorio)
DATABASE_URL=postgresql://apolo_user:password@localhost:5432/apolo_billing

# MySQL Kamailio (opcional, para sync)
KAMAILIO_DATABASE_URL=mysql://kamailio:password@localhost:3306/kamailio

# Redis (para cache)
REDIS_URL=redis://localhost:6379

# Backend
RUST_SERVER_PORT=8000
JWT_SECRET=your-secret-key

# Logging
RUST_LOG=apolo_api=info,actix_web=info
```

### 11.2 Checklist de Implementacion

- [ ] PostgreSQL configurado con `apolo_billing` database
- [ ] Migraciones ejecutadas en orden
- [ ] MySQL Kamailio accesible (si se usa sync)
- [ ] FreeSWITCH con mod_xml_curl configurado
- [ ] IP del backend en `freeswitch_allowed_ips`
- [ ] Kamailio con drouting habilitado
- [ ] `sipsak` instalado para SIP OPTIONS
- [ ] `kamcmd` accesible para recargar rutas
- [ ] `fs_cli` accesible para reloadxml
- [ ] Frontend configurado con proxy a :8000

### 11.3 Verificacion Post-Instalacion

```bash
# 1. Verificar API
curl http://localhost:8000/api/v1/health

# 2. Verificar FreeSWITCH
fs_cli -x "sofia status"

# 3. Verificar Kamailio
kamcmd drouting.dump

# 4. Crear primer carrier de prueba
curl -X POST http://localhost:8000/api/v1/carriers \
  -H "Content-Type: application/json" \
  -d '{"name": "Test", "host": "10.0.0.1", "port": 5060}'

# 5. Verificar sync
curl http://localhost:8000/api/v1/carriers
# Debe mostrar sync_status: "synced"
```

### 11.4 Comandos de Mantenimiento

```bash
# Recargar rutas FreeSWITCH
fs_cli -x "reloadxml"

# Recargar Kamailio drouting
kamcmd drouting.reload

# Ver gateways Kamailio
mysql -u kamailio -p kamailio -e "SELECT gwid, type, address, description FROM dr_gateways"

# Ver reglas Kamailio
mysql -u kamailio -p kamailio -e "SELECT ruleid, prefix, gwlist, priority FROM dr_rules WHERE groupid=8000"

# Ver grupos Kamailio
mysql -u kamailio -p kamailio -e "SELECT id, gwlist, description FROM dr_gw_lists"
```

---

## Apendice A: Diagramas de Flujo

### A.1 Llamada Entrante

```
PSTN → CARRIER (10.35.13.100)
         │
         │ INVITE sip:672017402002@SBC
         ▼
    KAMAILIO (:5060)
         │ Route: dr_rules groupid=9000
         │ Gateway: type=9 (FreeSWITCH)
         ▼
    FREESWITCH (:5082)
         │ Context: to-kamailio
         │ Match: ^(6720.+)$
         │ Transform: strip=4 → 17402002
         ▼
    EXTENSION (user/17402002@10.10.22.4)
```

### A.2 Llamada Saliente

```
EXTENSION → FREESWITCH (:5080)
                │ Context: from-pbx
                │ Dialplan: bridge → Kamailio
                ▼
           KAMAILIO (:5060)
                │ Route: dr_rules groupid=8000
                │ Match prefix: 2267
                │ Gateway: type=8 (INTEGRATEL)
                ▼
           CARRIER (10.35.13.100) → PSTN
```

---

## Apendice B: Troubleshooting

### B.1 Sync Status "error"

```bash
# Ver error especifico
curl http://localhost:8000/api/v1/inbound-routing | jq '.data[] | select(.sync_status=="error")'

# Verificar permisos de archivo
ls -la /etc/freeswitch/dialplan/to-kamailio.xml

# Verificar conexion MySQL
mysql -u kamailio -p -h localhost kamailio -e "SELECT 1"
```

### B.2 FreeSWITCH no aplica cambios

```bash
# Verificar XML generado
cat /etc/freeswitch/dialplan/to-kamailio.xml

# Forzar recarga
fs_cli -x "reloadxml"
fs_cli -x "show dialplan"

# Verificar log
tail -f /var/log/freeswitch/freeswitch.log
```

### B.3 Kamailio no enruta

```bash
# Verificar reglas en memoria
kamcmd drouting.dump

# Forzar recarga
kamcmd drouting.reload

# Verificar log
journalctl -u kamailio -f
```

---

**Fin del Documento PRD**
