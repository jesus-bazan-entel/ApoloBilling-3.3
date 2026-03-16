# 🧪 Guía de Testing del Motor de Billing Rust

## 📋 Descripción

Esta guía explica cómo probar el **motor de billing en Rust** usando un simulador de eventos ESL de FreeSWITCH, sin necesidad de tener FreeSWITCH instalado.

---

## 🎯 ¿Qué se va a probar?

El motor de billing Rust procesa eventos ESL y realiza:

1. **Autorización de llamadas** (CHANNEL_CREATE)
   - Verifica si la cuenta tiene balance
   - Calcula tarifa usando LPM (Longest Prefix Match)
   - Reserva balance para la llamada
   - Rechaza llamadas sin saldo

2. **Facturación en tiempo real** (CHANNEL_ANSWER)
   - Inicia billing cada X segundos
   - Extiende reservación automáticamente
   - Cuelga llamada si se agota saldo

3. **Generación de CDRs** (CHANNEL_HANGUP_COMPLETE)
   - Calcula costo total de la llamada
   - Genera registro CDR en PostgreSQL
   - Libera reservación de balance
   - Actualiza balance final de cuenta

---

## 🛠️ Prerequisitos

### 1. Servicios Requeridos

✅ **PostgreSQL** (base de datos)
```bash
sudo service postgresql start
sudo service postgresql status
```

✅ **Redis** (cache)
```bash
sudo service redis-server start
sudo service redis-server status
```

### 2. Base de Datos Inicializada

```bash
cd /home/jbazan/ApoloBilling/backend
source venv/bin/activate
python init_db_clean.py
```

Debe crear estas tablas:
- `accounts` (cuentas de clientes)
- `rate_cards` (tarifas)
- `balance_reservations` (reservas de balance)
- `cdrs` (call detail records)

### 3. Rust Instalado

```bash
# Verificar si Rust está instalado
rustc --version

# Si no está instalado:
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
source $HOME/.cargo/env
```

### 4. Python 3

```bash
python3 --version  # Debe ser 3.7+
```

---

## 🚀 Cómo Usar el Simulador

### Opción 1: Script Automático (Recomendado)

```bash
cd /home/jbazan/ApoloBilling
./tools/test_billing_engine.sh
```

Este script:
1. ✅ Verifica que PostgreSQL y Redis están corriendo
2. ✅ Verifica que la base de datos existe
3. ✅ Crea cuenta de prueba (100001) con $10.00
4. ✅ Verifica que existen rate cards
5. ✅ Compila el motor Rust si es necesario
6. ✅ Muestra instrucciones de uso

### Opción 2: Manual Paso a Paso

#### Terminal 1: Iniciar Motor Rust

```bash
cd /home/jbazan/ApoloBilling/rust-billing-engine

# Modo debug (con logs detallados)
RUST_LOG=info cargo run

# O modo release (más rápido)
RUST_LOG=info cargo run --release
```

**Salida esperada:**
```
🚀 Starting Apolo Billing Engine
📊 PostgreSQL pool: OK
📦 Redis connection: OK
🎧 ESL Server listening on 0.0.0.0:8021
✅ Billing Engine ready
```

#### Terminal 2: Ejecutar Simulador ESL

**Prueba básica (1 llamada de 30 segundos):**
```bash
cd /home/jbazan/ApoloBilling
python3 tools/esl_simulator.py --duration 30
```

**Prueba completa (5 llamadas de 60 segundos):**
```bash
python3 tools/esl_simulator.py \
    --duration 60 \
    --calls 5 \
    --delay 10
```

**Prueba personalizada:**
```bash
python3 tools/esl_simulator.py \
    --caller 100001 \
    --callee 51987654321 \
    --duration 120 \
    --account 100001 \
    --calls 3 \
    --delay 15
```

---

## 📊 Parámetros del Simulador

| Parámetro | Descripción | Default |
|-----------|-------------|---------|
| `--host` | Host del ESL | 127.0.0.1 |
| `--port` | Puerto ESL | 8021 |
| `--password` | Password ESL | ClueCon |
| `--caller` | Número del caller | 100001 |
| `--callee` | Número destino | 51987654321 |
| `--duration` | Duración en segundos | 60 |
| `--account` | Account ID | 100001 |
| `--calls` | Número de llamadas | 1 |
| `--delay` | Delay entre llamadas | 5 |

---

## 🔍 Monitoreo de Resultados

### 1. Logs del Motor Rust (Terminal 1)

Debes ver estos eventos:

```
📞 CHANNEL_CREATE: <uuid> - 100001 → 51987654321
   ↓ Buscando tarifa para 51987654321...
   ✅ Tarifa encontrada: 519 (Perú Móvil) - $0.0180/min
   ✅ Balance reservado: $2.40 para 120s
   ✅ Call AUTHORIZED: <uuid>

✅ CHANNEL_ANSWER: <uuid>
   ↓ Iniciando billing en tiempo real...
   💰 Billing tick: <uuid> - Billed: $0.18 (10s)
   💰 Billing tick: <uuid> - Billed: $0.36 (20s)
   ...

📴 CHANNEL_HANGUP: <uuid> - Duration: 120s, Billsec: 120s
   ↓ Generando CDR...
   ✅ CDR generado - Costo total: $3.60
   ✅ Balance actualizado: $6.40 restante
```

### 2. Base de Datos PostgreSQL

**Ver CDRs generados:**
```bash
sudo -u postgres psql -d apolo_billing -c \
    "SELECT call_uuid, caller_number, called_number, duration, billsec, cost, hangup_cause, created_at 
     FROM cdrs 
     ORDER BY created_at DESC 
     LIMIT 5;"
```

**Ver balance de cuenta:**
```bash
sudo -u postgres psql -d apolo_billing -c \
    "SELECT account_number, account_name, balance, account_type, status 
     FROM accounts 
     WHERE account_number = '100001';"
```

**Ver reservaciones activas (durante llamada):**
```bash
sudo -u postgres psql -d apolo_billing -c \
    "SELECT call_uuid, reserved_amount, status, expires_at 
     FROM balance_reservations 
     WHERE status = 'ACTIVE';"
```

**Ver transacciones de balance:**
```bash
sudo -u postgres psql -d apolo_billing -c \
    "SELECT amount, transaction_type, reason, created_at 
     FROM balance_transactions 
     WHERE account_id = (SELECT id FROM accounts WHERE account_number = '100001')
     ORDER BY created_at DESC 
     LIMIT 10;"
```

### 3. Redis Cache

**Ver reservaciones en Redis:**
```bash
redis-cli KEYS "reservation:*"
redis-cli GET "reservation:<uuid>"
```

**Ver estado de llamadas:**
```bash
redis-cli KEYS "call_state:*"
redis-cli GET "call_state:<uuid>"
```

### 4. Dashboard Web

Si tienes el servidor FastAPI corriendo:

```bash
# Terminal 3
cd /home/jbazan/ApoloBilling/backend
source venv/bin/activate
uvicorn main:app --host 0.0.0.0 --port 8000 --reload
```

Accede a:
- **CDRs**: http://localhost:8000/dashboard/cdr
- **Cuentas**: http://localhost:8000/dashboard/saldo
- **Rate Cards**: http://localhost:8000/dashboard/rate-cards

---

## 🧪 Casos de Prueba Sugeridos

### Caso 1: Llamada Normal (Suficiente Balance)

```bash
# Cuenta con $10.00
# Llamada de 60s a Perú Móvil ($0.0180/min = $1.08 total)
python3 tools/esl_simulator.py --duration 60 --callee 51987654321
```

**Resultado esperado:**
- ✅ Autorizada
- ✅ CDR generado
- ✅ Costo: ~$1.08
- ✅ Balance final: ~$8.92

### Caso 2: Llamada Larga (Extensión de Reservación)

```bash
# Llamada de 300s (5 minutos)
python3 tools/esl_simulator.py --duration 300 --callee 51987654321
```

**Resultado esperado:**
- ✅ Reservación extendida automáticamente cada 120s
- ✅ CDR generado
- ✅ Costo: ~$5.40 (300s * $0.0180/min)

### Caso 3: Sin Balance (Rechazo)

```bash
# Reducir balance a $0.10
sudo -u postgres psql -d apolo_billing -c \
    "UPDATE accounts SET balance = 0.10 WHERE account_number = '100001';"

# Intentar llamada de 60s (necesita $1.08)
python3 tools/esl_simulator.py --duration 60 --callee 51987654321
```

**Resultado esperado:**
- ❌ Llamada RECHAZADA
- ❌ Razón: "Insufficient balance"
- ❌ NO se genera CDR

### Caso 4: Múltiples Llamadas Concurrentes

```bash
# 10 llamadas de 30s con delay de 2s entre ellas
python3 tools/esl_simulator.py --duration 30 --calls 10 --delay 2
```

**Resultado esperado:**
- ✅ 10 CDRs generados
- ✅ Balance decrementado correctamente
- ✅ Sin conflictos de reservación

### Caso 5: Diferentes Destinos (LPM)

```bash
# Perú Lima (511) - $0.0200/min
python3 tools/esl_simulator.py --duration 60 --callee 5111234567

# Perú Nacional (51) - $0.0500/min
python3 tools/esl_simulator.py --duration 60 --callee 5171234567

# USA (1) - $0.0100/min
python3 tools/esl_simulator.py --duration 60 --callee 12025551234
```

**Resultado esperado:**
- ✅ Tarifa correcta por destino (LPM)
- ✅ Costos diferentes según rate card

---

## 🐛 Troubleshooting

### Error: "No se pudo conectar al servidor ESL"

**Causa:** Motor Rust no está corriendo

**Solución:**
```bash
# Terminal 1: Iniciar motor
cd /home/jbazan/ApoloBilling/rust-billing-engine
RUST_LOG=info cargo run
```

### Error: "Base de datos no existe"

**Causa:** Base de datos no inicializada

**Solución:**
```bash
cd /home/jbazan/ApoloBilling/backend
source venv/bin/activate
python init_db_clean.py
```

### Error: "Cuenta no encontrada"

**Causa:** Cuenta de prueba no existe

**Solución:**
```bash
sudo -u postgres psql -d apolo_billing << 'EOF'
INSERT INTO accounts (account_number, account_name, balance, account_type, status)
VALUES ('100001', 'Cuenta Demo', 10.00, 'PREPAID', 'ACTIVE')
ON CONFLICT (account_number) DO NOTHING;
EOF
```

### Error: "Rate card no encontrada"

**Causa:** No hay rate cards

**Solución:**
```bash
cd /home/jbazan/ApoloBilling/backend
source venv/bin/activate
python init_db_clean.py  # Inserta 13 rate cards de ejemplo
```

### Error: Compilación Rust falla

**Causa:** Dependencias faltantes

**Solución:**
```bash
sudo apt update
sudo apt install -y build-essential libssl-dev pkg-config
cd /home/jbazan/ApoloBilling/rust-billing-engine
cargo clean
cargo build
```

### Llamadas no se procesan

**Verificar:**
1. Motor Rust está corriendo y escuchando en puerto 8021
2. PostgreSQL está corriendo
3. Redis está corriendo
4. Cuenta tiene balance suficiente
5. Rate cards existen para el destino

---

## 📊 Estructura del Flujo de Llamada

```
┌─────────────────────────────────────────────────────────────────┐
│  1. CHANNEL_CREATE (Autorización)                               │
├─────────────────────────────────────────────────────────────────┤
│  ├─ Simulador envía evento con: caller, callee, uuid            │
│  ├─ Motor Rust recibe evento                                    │
│  ├─ Busca cuenta por caller (account_number)                    │
│  ├─ Busca tarifa con LPM (Longest Prefix Match)                │
│  ├─ Calcula costo estimado (120s * rate_per_minute)            │
│  ├─ Verifica balance disponible                                 │
│  ├─ Crea reservación en balance_reservations                    │
│  ├─ Guarda en Redis: reservation:uuid                           │
│  └─ Responde: AUTHORIZED o DENIED                               │
└─────────────────────────────────────────────────────────────────┘
                              ↓
┌─────────────────────────────────────────────────────────────────┐
│  2. CHANNEL_ANSWER (Inicio de Billing)                          │
├─────────────────────────────────────────────────────────────────┤
│  ├─ Simulador envía evento ANSWER                               │
│  ├─ Motor Rust inicia billing en tiempo real                    │
│  ├─ Cada 10s: descuenta balance y actualiza reservación         │
│  ├─ Si balance < 0: envía uuid_kill a FreeSWITCH               │
│  └─ Si reservación expira: extiende automáticamente +120s       │
└─────────────────────────────────────────────────────────────────┘
                              ↓
                       [Llamada en curso]
                              ↓
┌─────────────────────────────────────────────────────────────────┐
│  3. CHANNEL_HANGUP_COMPLETE (Fin y CDR)                         │
├─────────────────────────────────────────────────────────────────┤
│  ├─ Simulador envía evento HANGUP con duration/billsec          │
│  ├─ Motor Rust detiene billing                                  │
│  ├─ Recupera reservación de DB                                  │
│  ├─ Calcula costo final: billsec * rate_per_minute             │
│  ├─ Genera CDR en tabla cdrs                                    │
│  ├─ Libera reservación (status = COMPLETED)                     │
│  ├─ Actualiza balance final de cuenta                           │
│  └─ Limpia Redis: DELETE reservation:uuid                       │
└─────────────────────────────────────────────────────────────────┘
```

---

## 📈 Métricas de Performance Esperadas

| Operación | Tiempo Esperado |
|-----------|-----------------|
| Autorización (CHANNEL_CREATE) | < 10ms |
| Inicio billing (CHANNEL_ANSWER) | < 5ms |
| Billing tick (cada 10s) | < 3ms |
| Generación CDR (CHANNEL_HANGUP) | < 15ms |
| Búsqueda LPM en rate_cards | < 2ms |
| Reservación de balance | < 5ms |

---

## 📖 Archivos Relacionados

- `rust-billing-engine/src/main.rs` - Entry point del motor
- `rust-billing-engine/src/esl/event_handler.rs` - Procesador de eventos
- `rust-billing-engine/src/services/authorization.rs` - Servicio de autorización
- `rust-billing-engine/src/services/realtime_biller.rs` - Billing en tiempo real
- `rust-billing-engine/src/services/cdr_generator.rs` - Generador de CDRs
- `tools/esl_simulator.py` - Simulador de eventos ESL
- `tools/test_billing_engine.sh` - Script de testing automatizado

---

## 🎯 Próximos Pasos

Después de probar el motor:

1. ✅ Verificar que los CDRs se generan correctamente
2. ✅ Revisar que el balance se actualiza
3. ✅ Comprobar que las reservaciones se liberan
4. 🚀 Conectar con FreeSWITCH real (producción)
5. 📊 Monitorear performance en producción
6. 📈 Escalar horizontalmente si es necesario

---

**Última actualización:** 2025-12-22  
**Versión:** 1.0  
**Autor:** GenSpark AI Developer
