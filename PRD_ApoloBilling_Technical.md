# ApoloBilling - Product Requirements Document (PRD) Técnico

## Tabla de Contenidos
1. [Análisis del Sistema](#1-análisis-del-sistema)
2. [Funcionalidades Principales](#2-funcionalidades-principales)
3. [Arquitectura Técnica](#3-arquitectura-técnica)
4. [Modelos de Datos](#4-modelos-de-datos)
5. [Flujos de Trabajo](#5-flujos-de-trabajo)
6. [Integración con FreeSWITCH](#6-integración-con-freeswitch)
7. [API Endpoints](#7-api-endpoints)
8. [Casos de Uso](#8-casos-de-uso)
9. [Configuración y Despliegue](#9-configuración-y-despliegue)
10. [Consideraciones de Rendimiento](#10-consideraciones-de-rendimiento)

---

## 1. Análisis del Sistema

### 1.1 Descripción General

ApoloBilling es un sistema de facturación telefónica (billing) en tiempo real diseñado para entornos empresariales que requieren control granular sobre llamadas VoIP. El sistema integra múltiples plataformas (FreeSWITCH, CUCM) y proporciona funcionalidades avanzadas de rating, accounting y reporting.

### 1.2 Arquitectura Multi-Componente

El sistema está compuesto por **5 componentes principales**:

```mermaid
graph TB
    A[Frontend Web Dashboard] --> B[Backend Python FastAPI]
    B --> C[Motor Rust Billing Engine]
    B --> D[Java CUCM Listener]
    B --> E[ESL FreeSWITCH Connector]
    
    C --> F[(PostgreSQL Database)]
    D --> F
    E --> F
    
    G[FreeSWITCH PBX] --> E
    H[CUCM Server] --> D
```

#### Componentes del Sistema:

1. **Frontend Web Dashboard** - Interfaz administrativa con dashboards HTML
2. **Backend Python FastAPI** - API REST principal y lógica de negocio
3. **Motor Rust Billing Engine** - Procesamiento de rating en tiempo real
4. **Java CUCM Listener** - Integración con Cisco Unified Communications Manager
5. **ESL FreeSWITCH Connector** - Comunicación con FreeSWITCH PBX

---

## 2. Funcionalidades Principales

### 2.1 Gestión de Llamadas y CDR (Call Detail Records)

El sistema procesa llamadas en tiempo real y genera registros detallados:

```python
# Ejemplo de modelo CDR en Python
class CDR(Base):
    __tablename__ = "cdr"
    id = Column(Integer, primary_key=True, index=True)
    calling_number = Column(String)
    called_number = Column(String)
    start_time = Column(DateTime)
    end_time = Column(DateTime)
    duration_seconds = Column(Integer)
    duration_billable = Column(Integer)
    cost = Column(Numeric(10,2))
    status = Column(String)
    direction = Column(String)
    zone_id = Column(Integer)
```

**Funcionalidades:**
- Procesamiento automático de CDR desde PBX
- Detección automática de dirección de llamada (inbound/outbound/internal)
- Cálculo de costos basado en zonas geográficas
- Alertas de saldo bajo
- Estados de llamada: dialing, answered, disconnected, failed

### 2.2 Sistema de Zonas y Tarifas

El sistema implementa un sistema jerárquico de zonas geográficas con tarifas diferenciadas:

```python
class Zona(Base):
    __tablename__ = "zones"
    id = Column(Integer, primary_key=True, index=True)
    nombre = Column("zone_name", String, unique=True)
    descripcion = Column("description", String)
    
class Tarifa(Base):
    __tablename__ = "rate_zones"
    _rate_per_minute = Column("rate_per_minute", Numeric(10, 5))
    
    @property
    def tarifa_segundo(self):
        return self._rate_per_minute / 60 if self._rate_per_minute is not None else 0
```

**Algoritmo de Rating:**
- **Longest Prefix Match (LPM)**: Determina la zona basada en el prefijo más específico
- **Rating en Tiempo Real**: Cálculo de costos durante la llamada
- **Billing Increment**: Configuración granular de incrementos de facturación

### 2.3 Gestión de Anexos y Saldos

Sistema completo de gestión de usuarios con control de saldos:

```python
class Anexo(Base):
    __tablename__ = "anexos"
    numero = Column(String, unique=True, index=True)
    usuario = Column(String)
    saldo_actual = Column(Numeric(10, 2), default=0)
    pin = Column(String)
    activo = Column(Boolean, default=True)

class SaldoAnexo(Base):
    __tablename__ = "saldo_anexos"
    calling_number = Column(String, unique=True)
    saldo = Column(Numeric(10,2))
```

**Características:**
- Recarga individual y masiva de saldos
- Auditoría completa de transacciones
- PINs seguros con hash bcrypt
- Control de saldos por extensión

### 2.4 Dashboard de Monitoreo

Interfaz web con dashboards en tiempo real:

```javascript
// WebSocket para actualizaciones en tiempo real
const socket = new WebSocket('ws://localhost:8000/ws');

socket.onmessage = function(event) {
    const data = JSON.parse(event.data);
    if (data.type === 'update') {
        updateActiveCalls(data.active_calls);
    }
};
```

**Dashboards Disponibles:**
- Monitoreo de llamadas activas
- Saldos por anexo
- Reportes CDR con filtros avanzados
- Estadísticas financieras
- Auditoría de transacciones
- Gestión de zonas y tarifas

### 2.5 Sistema de Autorización FAC (Forced Authorization Codes)

Gestión avanzada de códigos de autorización:

```java
// Java listener para CUCM integration
public class TarificadorService {
    public void startMonitoring() {
        JtapiPeer peer = JtapiPeerFactory.getJtapiPeer(null);
        provider = peer.getProvider(providerString);
        provider.addCallObserver(new CallListener());
    }
}
```

**Características:**
- Sincronización bidireccional con CUCM
- Códigos de autorización por extensión
- Niveles de autorización granulares
- Auditoría de uso de códigos

---

## 3. Arquitectura Técnica

### 3.1 Backend Python (FastAPI)

El backend principal implementado en FastAPI con arquitectura modular:

```python
# main.py - Configuración principal
app = FastAPI()
app.mount("/static", StaticFiles(directory="static"), name="static")
templates = Jinja2Templates(directory="templates")

# WebSocket manager para tiempo real
class ConnectionManager:
    def __init__(self):
        self.active_connections: List[WebSocket] = []
        self._lock = asyncio.Lock()
```

**Características Técnicas:**
- **Framework**: FastAPI con async/await
- **WebSockets**: Comunicación en tiempo real
- **SQLAlchemy ORM**: Mapeo objeto-relacional
- **Pydantic**: Validación de datos
- **Jinja2**: Motor de templates
- **Passlib**: Hash seguro de PINs

### 3.2 Motor Rust Billing Engine

Motor de alta performance para procesamiento de llamadas:

```rust
// src/main.rs - Configuración del motor Rust
#[actix_web::main]
async fn main() -> std::io::Result<()> {
    let reservation_mgr = Arc::new(ReservationManager::new(
        db_pool.clone(),
        redis_client.clone()
    ));
    
    let auth_service = Arc::new(AuthorizationService::new(
        db_pool.clone(),
        redis_client.clone(),
        reservation_mgr.clone()
    ));
}
```

**Componentes del Motor Rust:**
- **ReservationManager**: Gestión de reservas de saldo
- **AuthorizationService**: Autorización de llamadas
- **RealtimeBiller**: Facturación en tiempo real
- **CdrGenerator**: Generación de CDR
- **FreeSwitchCluster**: Cluster ESL para FreeSWITCH

**Librerías Principales:**
- **Actix Web**: Framework web async
- **Tokio**: Runtime async
- **Diesel**: ORM para Rust
- **Redis**: Cache distribuido
- **Serde**: Serialización/deserialización

### 3.3 Java CUCM Listener

Servicio Java para integración con Cisco CUCM:

```java
// TarificadorService.java - Configuración JTAPI
public class TarificadorService {
    private static Provider provider;
    
    public static void startMonitoring() {
        String providerString = host + ";login=" + user + ";passwd=" + password;
        JtapiPeer peer = JtapiPeerFactory.getJtapiPeer(null);
        provider = peer.getProvider(providerString);
    }
}
```

**Características:**
- **JTAPI**: Java Telephony API para CUCM
- **Event Observers**: Monitoreo de eventos de llamadas
- **Auto-reconnection**: Reconexión automática
- **Thread Pool**: Ejecutor de tareas programado

### 3.4 ESL FreeSWITCH Connector

Conector para comunicación con FreeSWITCH:

```python
# connectors/freeswitch/esl_listener.py
class ESLClient:
    async def connect(self):
        self.reader, self.writer = await asyncio.open_connection(ESL_HOST, ESL_PORT)
        await self.authenticate()
        
    async def subscribe(self):
        cmd = "events plain CHANNEL_CREATE CHANNEL_ANSWER CHANNEL_HANGUP\n\n"
        self.writer.write(cmd.encode())
```

**Funcionalidades:**
- **Event Subscription**: Suscripción a eventos FreeSWITCH
- **AsyncIO**: Comunicación asíncrona
- **Event Parsing**: Procesamiento de eventos de llamadas
- **Backend Integration**: Integración con API principal

---

## 4. Modelos de Datos

### 4.1 Esquema de Base de Datos

El sistema utiliza PostgreSQL con un esquema optimizado para billing:

```sql
-- Tabla principal de CDR
CREATE TABLE call_detail_records (
    id BIGSERIAL PRIMARY KEY,
    call_uuid VARCHAR(100) UNIQUE NOT NULL,
    caller_number VARCHAR(50) NOT NULL,
    callee_number VARCHAR(50) NOT NULL,
    start_time TIMESTAMP NOT NULL,
    answer_time TIMESTAMP,
    end_time TIMESTAMP NOT NULL,
    duration INTEGER NOT NULL DEFAULT 0,
    billsec INTEGER NOT NULL DEFAULT 0,
    cost DECIMAL(12, 4) DEFAULT 0.0000,
    hangup_cause VARCHAR(50)
);

-- Tabla de cuentas prepago/postpago
CREATE TABLE accounts (
    id SERIAL PRIMARY KEY,
    account_code VARCHAR(50) UNIQUE NOT NULL,
    account_type account_type NOT NULL DEFAULT 'PREPAID',
    balance DECIMAL(12, 4) NOT NULL DEFAULT 0.0000,
    credit_limit DECIMAL(12, 4) DEFAULT 0.0000
);

-- Rate cards con LPM support
CREATE TABLE rate_cards (
    id SERIAL PRIMARY KEY,
    destination_prefix VARCHAR(20) NOT NULL,
    rate_per_minute DECIMAL(10, 6) NOT NULL,
    priority INTEGER NOT NULL DEFAULT 100
);
```

### 4.2 Modelos Rust

```rust
// src/models/rate.rs
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RateCard {
    pub id: i32,
    pub destination_prefix: String,
    pub rate_per_minute: Decimal,
    pub billing_increment: i32,
    pub connection_fee: Decimal,
}

impl RateCard {
    pub fn calculate_cost(&self, billsec: i32) -> Decimal {
        let rounded_billsec = ((billsec + self.billing_increment - 1) / self.billing_increment) 
            * self.billing_increment;
        let minutes = Decimal::from(rounded_billsec) / Decimal::from(60);
        minutes * self.rate_per_minute + self.connection_fee
    }
}
```

### 4.3 Algoritmo de Longest Prefix Match

```python
def determinar_zona_y_tarifa(numero_marcado: str, db):
    """
    Algoritmo LPM para determinar zona y tarifa
    """
    numero_limpio = ''.join(filter(str.isdigit, numero_marcado))
    
    # Generar todos los posibles prefijos
    posibles_prefijos = [numero_limpio[:i] for i in range(1, len(numero_limpio) + 1)]
    
    # Buscar el prefijo más largo que coincida
    matches = db.query(RateCard).filter(
        RateCard.destination_prefix.in_(posibles_prefijos)
    ).order_by(func.length(RateCard.destination_prefix).desc()).all()
    
    return matches[0] if matches else None
```

---

## 5. Flujos de Trabajo

### 5.1 Flujo de Procesamiento de Llamada

```mermaid
sequenceDiagram
    participant FS as FreeSWITCH
    participant ESL as ESL Connector
    participant API as FastAPI Backend
    participant Rust as Rust Billing Engine
    participant DB as PostgreSQL

    FS->>ESL: CHANNEL_CREATE Event
    ESL->>API: POST /active-calls
    API->>DB: Insert Active Call
    API-->>ESL: Success Response
    
    FS->>ESL: CHANNEL_ANSWER Event
    ESL->>API: Update Active Call
    
    FS->>ESL: CHANNEL_HANGUP Event
    ESL->>API: POST /cdr
    API->>Rust: Process Rating
    Rust->>DB: Update Account Balance
    API->>DB: Insert CDR
    API-->>ESL: Success Response
```

### 5.2 Flujo de Verificación de Saldo

```python
@app.get("/check_balance_for_call/{calling_number}/{called_number}")
def check_balance_for_call(calling_number: str, called_number: str):
    """
    Verifica si hay saldo suficiente para una llamada específica
    """
    # 1. Obtener saldo actual
    saldo_anexo = db.query(SaldoAnexo).filter(
        SaldoAnexo.calling_number == calling_number
    ).first()
    
    # 2. Determinar zona y tarifa
    zona_info = determinar_zona_y_tarifa(called_number, db)
    
    # 3. Verificar saldo disponible
    can_call = saldo_actual >= tarifa_segundo
    
    # 4. Calcular tiempo disponible
    tiempo_disponible_segundos = int(saldo_actual / tarifa_segundo)
    
    return {
        "can_call": can_call,
        "tiempo_disponible_segundos": tiempo_disponible_segundos,
        "zona": zona_info['zona_nombre']
    }
```

### 5.3 Flujo de Recarga de Saldo

```python
@app.post("/recargar/{calling_number}/{amount}")
async def recargar_saldo(calling_number: str, amount: float, user=Depends(admin_only)):
    """
    Proceso completo de recarga con auditoría
    """
    db = SessionLocal()
    
    # 1. Obtener saldo anterior
    saldo_anterior = db.execute(
        text("SELECT saldo FROM saldo_anexos WHERE calling_number = :calling_number"),
        {"calling_number": calling_number}
    ).fetchone()
    
    # 2. Actualizar saldo
    db.execute(
        text("UPDATE saldo_anexos SET saldo = saldo + :amount WHERE calling_number = :calling_number"),
        {"amount": amount, "calling_number": calling_number}
    )
    
    # 3. Registrar auditoría
    db.execute(
        text("""
            INSERT INTO saldo_auditoria (calling_number, saldo_anterior, saldo_nuevo, tipo_accion)
            VALUES (:calling_number, :saldo_anterior, :saldo_nuevo, 'recarga')
        """),
        {
            "calling_number": calling_number,
            "saldo_anterior": saldo_anterior[0] if saldo_anterior else 0,
            "saldo_nuevo": saldo_anterior[0] + amount if saldo_anterior else amount
        }
    )
    
    # 4. Registrar recarga
    db.execute(
        text("INSERT INTO recargas (calling_number, monto) VALUES (:calling_number, :amount)"),
        {"calling_number": calling_number, "amount": amount}
    )
    
    db.commit()
```

---

## 6. Integración con FreeSWITCH

### 6.1 Event Socket Library (ESL)

El sistema se conecta a FreeSWITCH usando ESL para recibir eventos en tiempo real:

```python
# ESL Client Implementation
class ESLClient:
    async def authenticate(self):
        # Read header hasta encontrar auth request
        while True:
            line = await self.reader.readline()
            if b"Content-Type: auth/request" in line:
                self.writer.write(f"auth {ESL_PASSWORD}\n\n".encode())
                break
                
    async def subscribe(self):
        # Suscribirse a eventos específicos
        cmd = "events plain CHANNEL_CREATE CHANNEL_ANSWER CHANNEL_HANGUP\n\n"
        self.writer.write(cmd.encode())
```

### 6.2 Eventos Procesados

**Eventos FreeSWITCH Monitoreados:**
- `CHANNEL_CREATE`: Inicio de llamada
- `CHANNEL_ANSWER`: Llamada contestada
- `CHANNEL_HANGUP`: Finalización de llamada

### 6.3 Procesamiento de Eventos

```python
async def process_event(self, event):
    event_name = event.get("Event-Name")
    uuid = event.get("Unique-ID")
    
    if event_name == "CHANNEL_CREATE":
        await self.report_call_start(event)
    elif event_name == "CHANNEL_ANSWER":
        await self.report_call_answer(event)
    elif event_name == "CHANNEL_HANGUP":
        await self.report_call_end(event)
```

---

## 7. API Endpoints

### 7.1 Endpoints de CDR

```python
# Procesamiento de CDR
@app.post("/cdr")
def create_cdr(event: CallEvent):
    """
    Endpoint principal para procesar CDR desde PBX
    """
    # 1. Determinar zona automáticamente
    zona_id = get_zone_by_prefix(db, event.called_number)
    
    # 2. Obtener tarifa específica
    rate_per_minute = get_rate_by_zone(db, zona_id)
    
    # 3. Calcular costo
    cost = (event.duration_billable / 60) * rate_per_minute
    
    # 4. Guardar CDR y actualizar saldo
    cdr = CDR(**cdr_data)
    db.add(cdr)
    
    # Actualizar saldo del anexo
    db.execute(
        text("UPDATE saldo_anexos SET saldo = saldo - :cost WHERE calling_number = :calling_number"),
        {"cost": cost, "calling_number": event.calling_number}
    )
```

### 7.2 Endpoints de Tiempo Real

```python
# WebSocket para llamadas activas
@app.websocket("/ws")
async def websocket_endpoint(websocket: WebSocket):
    await ws_manager.connect(websocket)
    
    # Enviar llamadas activas al cliente
    active_calls = get_active_calls()
    await websocket.send_json({
        "type": "update",
        "active_calls": active_calls
    })
```

### 7.3 Endpoints de Gestión

```python
# Gestión de anexos
@app.get("/dashboard/anexos")
async def dashboard_anexos(request: Request, user=Depends(admin_only)):
    """
    Dashboard de gestión de anexos con paginación
    """
    # Query con filtros
    query = text("""
        SELECT a.id, a.numero, a.usuario, a.area_nivel1, a.saldo_actual, s.saldo
        FROM anexos a
        LEFT JOIN saldo_anexos s ON a.numero = s.calling_number
        WHERE 1=1
    """)
    
    return templates.TemplateResponse("dashboard_anexos.html", {
        "request": request,
        "rows": query_result,
        "user": user
    })
```

### 7.4 Endpoints de Configuración

```python
# Configuración de zonas y tarifas
@app.post("/api/zonas")
async def crear_zona(zona: ZonaCreate, user=Depends(admin_only)):
    """
    Crear nueva zona geográfica
    """
    # Insertar zona
    zona_id = db.execute(insert_query).fetchone()[0]
    
    # Crear tarifa por defecto
    db.execute(text("""
        INSERT INTO rate_zones (zone_id, rate_per_minute, enabled)
        VALUES (:zona_id, 0.03, TRUE)
    """), {"zona_id": zona_id})
```

---

## 8. Casos de Uso

### 8.1 Caso de Uso 1: Llamada Saliente Facturada

**Escenario**: Un anexo realiza una llamada a número móvil

**Flujo:**
1. Usuario marca número destino en teléfono IP
2. FreeSWITCH inicia llamada y envía evento CHANNEL_CREATE
3. ESL Connector reporta llamada activa al sistema
4. Sistema verifica saldo del anexo
5. Si saldo es suficiente, permite llamada
6. Al contestar destino, FreeSWITCH envía CHANNEL_ANSWER
7. Al finalizar, FreeSWITCH envía CHANNEL_HANGUP
8. Sistema calcula costo basado en zona destino
9. Deduce costo del saldo del anexo
10. Genera CDR completo

**Código Relevante:**
```python
# Verificación previa
@app.get("/check_balance_for_call/{calling_number}/{called_number}")
def check_balance_for_call(calling_number: str, called_number: str):
    # Determinar zona y tarifa
    zona_info = determinar_zona_y_tarifa(called_number, db)
    tarifa_segundo = zona_info['tarifa_segundo']
    
    # Verificar si puede realizar al menos 1 segundo
    can_call = saldo_actual >= tarifa_segundo
    
    return {"can_call": can_call}
```

### 8.2 Caso de Uso 2: Recarga Masiva de Saldos

**Escenario**: Administrador recarga saldos de múltiples anexos desde archivo Excel

**Flujo:**
1. Administrador sube archivo Excel con anexos y montos
2. Sistema valida formato y estructura del archivo
3. Para cada fila válida:
   - Busca anexo en base de datos
   - Calcula nuevo saldo
   - Registra transacción de auditoría
4. Genera reporte de proceso con errores
5. Envía confirmación por email

**Código Relevante:**
```python
@app.post("/dashboard/recarga_masiva")
async def recarga_masiva(request: Request, file: UploadFile = File(...)):
    # Procesar archivo Excel
    wb = openpyxl.load_workbook(io.BytesIO(content))
    ws = wb.active
    
    for row in ws.rows:
        anexo = row[0].value
        monto = float(row[1].value)
        
        # Actualizar saldo
        db.execute(text("""
            UPDATE saldo_anexos SET saldo = saldo + :monto 
            WHERE calling_number = :anexo
        """), {"monto": monto, "anexo": anexo})
        
        # Registrar auditoría
        db.execute(audit_query)
```

### 8.3 Caso de Uso 3: Configuración de Nueva Zona Tarifaria

**Escenario**: Administrador agrega nueva zona con tarifa diferenciada

**Flujo:**
1. Administrador accede a dashboard de zonas
2. Crea nueva zona "Internacional Premium"
3. Define prefijos para la zona (ej: 001, 002)
4. Establece tarifa específica (ej: $0.50/minuto)
5. Sistema actualiza rate_cards para motor Rust
6. Nueva zona disponible para futuras llamadas

**Código Relevante:**
```python
@app.post("/api/tarifas")
async def crear_tarifa(tarifa: TarifaCreate):
    # Desactivar tarifas anteriores
    db.execute(text("UPDATE rate_zones SET enabled = FALSE WHERE zone_id = :zona_id"),
              {"zona_id": tarifa.zona_id})
    
    # Crear nueva tarifa
    db.execute(text("""
        INSERT INTO rate_zones (zone_id, rate_per_minute, enabled)
        VALUES (:zona_id, :tarifa_segundo * 60, TRUE)
    """), {"zona_id": tarifa.zona_id, "tarifa_segundo": tarifa.tarifa_segundo})
    
    # Sincronizar con motor Rust
    sync_rate_cards(db)
```

### 8.4 Caso de Uso 4: Monitoreo de Llamadas en Tiempo Real

**Escenario**: Supervisor monitorea llamadas activas en dashboard

**Flujo:**
1. Supervisor abre dashboard de monitoreo
2. WebSocket conecta automáticamente
3. Sistema envía lista de llamadas activas
4. Dashboard actualiza UI en tiempo real
5. Cuando ocurre nueva llamada, se actualiza automáticamente
6. Supervisor puede filtrar por anexo, zona, duración

**Código Relevante:**
```javascript
// Frontend JavaScript
const socket = new WebSocket('ws://localhost:8000/ws');

socket.onmessage = function(event) {
    const data = JSON.parse(event.data);
    if (data.type === 'update') {
        updateActiveCallsTable(data.active_calls);
        updateStatistics(data.stats);
    }
};

function updateActiveCallsTable(calls) {
    const tbody = document.getElementById('active-calls-body');
    tbody.innerHTML = '';
    
    calls.forEach(call => {
        const row = createCallRow(call);
        tbody.appendChild(row);
    });
}
```

---

## 9. Configuración y Despliegue

### 9.1 Requisitos del Sistema

**Hardware Mínimo:**
- **CPU**: 4 cores x86_64
- **RAM**: 8GB DDR4
- **Storage**: 100GB SSD
- **Network**: 1Gbps Ethernet

**Software:**
- **OS**: Ubuntu 20.04+ / CentOS 8+
- **Database**: PostgreSQL 13+
- **Runtime**: Python 3.9+, Java 11+, Rust 1.56+
- **Cache**: Redis 6+

### 9.2 Variables de Entorno

```bash
# Database Configuration
DATABASE_URL="postgresql://apolo:apolo123@localhost/apolobilling"

# Redis Configuration  
REDIS_URL="redis://localhost:6379/0"

# FreeSWITCH ESL
ESL_HOST="127.0.0.1"
ESL_PORT="8021"
ESL_PASSWORD="ClueCon"

# CUCM Configuration
CUCM_ADDRESS="190.105.250.127"
CUCM_USERNAME="jtapiuser"
CUCM_PASSWORD="fr4v4t3l"

# Security
SECRET="secreto-super-importante"
```

### 9.3 Instalación y Configuración

**1. Base de Datos PostgreSQL:**
```bash
# Crear base de datos
createdb apolobilling

# Ejecutar esquema
psql apolobilling < schema.sql

# Configurar usuario
psql apolobilling
CREATE USER apolo WITH PASSWORD 'apolo123';
GRANT ALL PRIVILEGES ON DATABASE apolobilling TO apolo;
```

**2. Motor Rust:**
```bash
# Compilar
cd rust-billing-engine
cargo build --release

# Ejecutar
./target/release/apolo-billing-engine
```

**3. Backend Python:**
```bash
# Instalar dependencias
pip install -r requirements.txt

# Ejecutar
uvicorn main:app --host 0.0.0.0 --port 8000
```

**4. Java Listener:**
```bash
# Compilar
cd java_listener
mvn clean package

# Ejecutar
java -jar target/tarificador-cucm-1.0.jar
```

### 9.4 Configuración de Servicios

**Systemd Services:**

```ini
# /etc/systemd/system/apolo-billing.service
[Unit]
Description=Apolo Billing Engine (Rust)
After=network.target

[Service]
Type=simple
User=apolo
WorkingDirectory=/opt/apolo-billing
ExecStart=/opt/apolo-billing/rust-billing-engine/target/release/apolo-billing-engine
Restart=always

[Install]
WantedBy=multi-user.target
```

```ini
# /etc/systemd/system/apolo-backend.service
[Unit]
Description=Apolo Backend API
After=network.target postgresql.service

[Service]
Type=simple
User=apolo
WorkingDirectory=/opt/apolo-billing
ExecStart=/usr/bin/python3 -m uvicorn main:app --host 0.0.0.0 --port 8000
Restart=always

[Install]
WantedBy=multi-user.target
```

### 9.5 Configuración de FreeSWITCH

```xml
<!-- /usr/local/freeswitch/conf/autoload_configs/xml_curl.conf.xml -->
<document type="text/xml">
    <X-Post-URL>http://localhost:8000/xml_cdr</X-Post-URL>
    <X-Post-URL-SSL>https://localhost:8443/xml_cdr</X-Post-URL-SSL>
</document>
```

---

## 10. Consideraciones de Rendimiento

### 10.1 Optimizaciones de Base de Datos

**Índices Estratégicos:**
```sql
-- Índices para optimización de consultas CDR
CREATE INDEX idx_cdr_start_time ON call_detail_records(start_time);
CREATE INDEX idx_cdr_account_start ON call_detail_records(account_id, start_time);
CREATE INDEX idx_cdr_caller ON call_detail_records(caller_number);
CREATE INDEX idx_cdr_callee ON call_detail_records(callee_number);

-- Índices para LPM en rate_cards
CREATE INDEX idx_rate_cards_prefix_priority ON rate_cards(destination_prefix, priority DESC);

-- Índices para auditoría
CREATE INDEX idx_audit_timestamp ON saldo_auditoria(fecha DESC);
CREATE INDEX idx_audit_calling_number ON saldo_auditoria(calling_number);
```

**Particionado de Tablas:**
```sql
-- Particionado por fecha para tabla CDR
CREATE TABLE call_detail_records_2024 PARTITION OF call_detail_records
FOR VALUES FROM ('2024-01-01') TO ('2025-01-01');

CREATE TABLE call_detail_records_2025 PARTITION OF call_detail_records
FOR VALUES FROM ('2025-01-01') TO ('2026-01-01');
```

### 10.2 Optimizaciones en Rust

**Pool de Conexiones:**
```rust
// src/database/pool.rs
pub async fn create_pool(database_url: &str) -> Result<PgPool, sqlx::Error> {
    PgPoolOptions::new()
        .max_connections(20)          // Máximo 20 conexiones
        .min_connections(5)           // Mínimo 5 conexiones
        .connect_timeout(Duration::from_secs(10))
        .idle_timeout(Duration::from_secs(300))
        .max_lifetime(Duration::from_secs(1800))  // Reciclar conexiones cada 30min
        .connect(database_url).await
}
```

**Cache con Redis:**
```rust
// src/cache/redis_client.rs
pub struct RedisClient {
    client: redis::Client,
    connection_pool: Pool<ConnectionManager>,
}

impl RedisClient {
    pub async fn cache_rate_lookup(&self, prefix: &str, rate: &RateCard) -> Result<(), RedisError> {
        let key = format!("rate:{}", prefix);
        let serialized = serde_json::to_string(rate)?;
        
        redis::cmd("SETEX")
            .arg(&key)
            .arg(3600) // TTL 1 hora
            .arg(&serialized)
            .query_async(&mut self.connection_pool)
            .await
    }
}
```

### 10.3 Optimizaciones en Python

**Connection Pooling:**
```python
# main.py
engine = create_engine(
    DATABASE_URL,
    pool_size=20,           # Tamaño del pool
    max_overflow=30,         # Conexiones adicionales
    pool_timeout=30,         # Timeout para obtener conexión
    pool_recycle=3600,       # Reciclar cada hora
    pool_pre_ping=True       # Verificar conexión antes de usar
)
```

**Async WebSockets:**
```python
# WebSocket con connection manager
class ConnectionManager:
    def __init__(self):
        self.active_connections: List[WebSocket] = []
        self._lock = asyncio.Lock()  # Thread-safe

    async def broadcast(self, message: dict) -> None:
        async with self._lock:
            closed_connections = []
            
            for connection in self.active_connections:
                try:
                    await connection.send_json(message)
                except Exception:
                    closed_connections.append(connection)
                    
            # Limpiar conexiones cerradas
            for conn in closed_connections:
                self.disconnect(conn)
```

### 10.4 Métricas de Rendimiento

**Objetivos de Performance:**
- **Latencia CDR**: < 100ms desde hangup hasta CDR procesado
- **Throughput**: > 1000 llamadas/minuto procesadas
- **WebSocket Updates**: < 50ms tiempo real
- **Database Queries**: < 10ms para consultas CDR
- **Rating Engine**: < 5ms para cálculo de costos

**Monitoreo:**
```python
# Métricas de performance
@app.middleware("http")
async def performance_middleware(request: Request, call_next):
    start_time = time.time()
    
    response = await call_next(request)
    
    process_time = time.time() - start_time
    logger.info(f"Request {request.url.path} took {process_time:.3f}s")
    
    response.headers["X-Process-Time"] = str(process_time)
    return response
```

### 10.5 Escalabilidad

**Horizontal Scaling:**
- **Load Balancer**: Nginx/HAProxy para balancear tráfico
- **Database Replication**: Master-Slave PostgreSQL
- **Redis Cluster**: Para cache distribuido
- **Message Queue**: RabbitMQ/Kafka para procesamiento asíncrono

**Arquitectura Escalable:**
```mermaid
graph LR
    LB[Nginx Load Balancer] --> API1[API Server 1]
    LB --> API2[API Server 2]
    LB --> API3[API Server N]
    
    API1 --> DB_M[(PostgreSQL Master)]
    API2 --> DB_S[(PostgreSQL Slave)]
    API3 --> DB_S
    
    API1 --> Redis1[(Redis Cluster)]
    API2 --> Redis2[(Redis Cluster)]
    API3 --> Redis3[(Redis Cluster)]
```

---

## Conclusión

ApoloBilling es un sistema de facturación telefónica robusto y escalable que combina tecnologías modernas (FastAPI, Rust, Java) para proporcionar:

- **Facturación en tiempo real** con latencia ultra-baja
- **Arquitectura distribuida** para alta disponibilidad
- **Integración nativa** con FreeSWITCH y CUCM
- **Dashboard web** intuitivo para administración
- **APIs RESTful** para integraciones externas
- **Auditoría completa** de transacciones
- **Escalabilidad horizontal** para crecimiento

El sistema está diseñado para entornos empresariales que requieren control granular sobre costos de telecomunicaciones y reporting detallado de usage patterns.

---

**Versión del Documento**: 1.0  
**Fecha**: Enero 2026  
**Autor**: Equipo Técnico ApoloBilling  
**Clasificación**: Documentación Técnica Interna