# Call Simulator - ApoloBilling

El simulador de llamadas permite probar el flujo completo de billing sin necesidad de FreeSWITCH. Simula los tres eventos ESL principales:

1. **CHANNEL_CREATE** - Autorización y reserva de balance
2. **CHANNEL_ANSWER** - Llamada contestada
3. **CHANNEL_HANGUP_COMPLETE** - Generación de CDR y consumo de reserva

## Endpoints API

Base URL: `http://localhost:9000/api/v1`

### 1. Iniciar una llamada simulada

```bash
POST /simulate/call
```

**Request:**
```json
{
    "caller": "51999888777",
    "callee": "5491155551234",
    "direction": "outbound",
    "duration_seconds": 60,
    "ring_seconds": 3,
    "hangup_cause": "NORMAL_CLEARING"
}
```

**Campos:**
| Campo | Tipo | Requerido | Descripción |
|-------|------|-----------|-------------|
| caller | string | Sí | Número llamante (ANI) - identifica la cuenta |
| callee | string | Sí | Número llamado (DNIS) - para búsqueda de tarifa |
| direction | string | No | "outbound" (default) o "inbound" |
| duration_seconds | int | No | Duración simulada. Si no se especifica, la llamada permanece activa |
| ring_seconds | int | No | Segundos de ring antes de contestar (default: 2) |
| hangup_cause | string | No | Causa de hangup (default: "NORMAL_CLEARING") |

**Response (éxito):**
```json
{
    "success": true,
    "call_uuid": "f47ac10b-58cc-4372-a567-0e02b2c3d479",
    "message": "Call started successfully",
    "authorization": {
        "authorized": true,
        "reason": "authorized",
        "account_id": 1,
        "reservation_id": "550e8400-e29b-41d4-a716-446655440000",
        "reserved_amount": 0.81,
        "rate_per_minute": 0.15,
        "max_duration_seconds": 324
    }
}
```

**Response (llamada rechazada):**
```json
{
    "success": false,
    "call_uuid": "f47ac10b-58cc-4372-a567-0e02b2c3d479",
    "message": "Call denied: account_not_found",
    "authorization": {
        "authorized": false,
        "reason": "account_not_found",
        "account_id": null,
        "reservation_id": null,
        "reserved_amount": null,
        "rate_per_minute": null,
        "max_duration_seconds": null
    }
}
```

### 2. Colgar una llamada manualmente

```bash
POST /simulate/hangup/{call_uuid}?cause=NORMAL_CLEARING
```

**Response:**
```json
{
    "success": true,
    "message": "Call f47ac10b-58cc-4372-a567-0e02b2c3d479 hung up"
}
```

### 3. Listar llamadas activas

```bash
GET /simulate/calls
```

**Response:**
```json
{
    "count": 2,
    "calls": [
        {
            "call_uuid": "f47ac10b-58cc-4372-a567-0e02b2c3d479",
            "caller": "51999888777",
            "callee": "5491155551234",
            "direction": "outbound",
            "start_time": "2026-01-21T15:30:00Z",
            "answer_time": "2026-01-21T15:30:02Z",
            "end_time": null,
            "status": "answered",
            "account_id": 1,
            "rate_per_minute": "0.15",
            "max_duration_seconds": 324,
            "hangup_cause": null
        }
    ]
}
```

### 4. Obtener detalles de una llamada

```bash
GET /simulate/call/{call_uuid}
```

### 5. Ejecutar un escenario de prueba

```bash
POST /simulate/scenario
```

**Request:**
```json
{
    "name": "Test Mix Calls",
    "calls": [
        {
            "caller": "51999888777",
            "callee": "5491155550001",
            "direction": "outbound",
            "duration_seconds": 30,
            "delay_before_ms": 0
        },
        {
            "caller": "5491100000000",
            "callee": "51999888777",
            "direction": "inbound",
            "duration_seconds": 20,
            "delay_before_ms": 500
        },
        {
            "caller": "51999888777",
            "callee": "5491155550002",
            "direction": "outbound",
            "duration_seconds": 45,
            "delay_before_ms": 1000
        }
    ]
}
```

**Response:**
```json
{
    "scenario_completed": true,
    "total_calls": 3,
    "successful": 3,
    "failed": 0,
    "results": [...]
}
```

### 6. Limpiar simulaciones completadas

```bash
POST /simulate/cleanup
```

## Ejemplos de Uso con curl

### Llamada saliente (OUTBOUND) - Con billing

```bash
# Llamada de 60 segundos
curl -X POST http://localhost:9000/api/v1/simulate/call \
    -H "Content-Type: application/json" \
    -d '{
        "caller": "51999888777",
        "callee": "5491155551234",
        "direction": "outbound",
        "duration_seconds": 60
    }'
```

### Llamada entrante (INBOUND) - Sin billing

```bash
# Las llamadas inbound NO se facturan
curl -X POST http://localhost:9000/api/v1/simulate/call \
    -H "Content-Type: application/json" \
    -d '{
        "caller": "5491100000000",
        "callee": "51999888777",
        "direction": "inbound",
        "duration_seconds": 30
    }'
```

### Llamada sin duración fija (para hangup manual)

```bash
# Iniciar llamada sin duración
RESPONSE=$(curl -s -X POST http://localhost:9000/api/v1/simulate/call \
    -H "Content-Type: application/json" \
    -d '{
        "caller": "51999888777",
        "callee": "5491155551234",
        "direction": "outbound"
    }')

CALL_UUID=$(echo $RESPONSE | jq -r '.call_uuid')
echo "Call UUID: $CALL_UUID"

# Esperar un momento...
sleep 45

# Colgar manualmente
curl -X POST "http://localhost:9000/api/v1/simulate/hangup/${CALL_UUID}?cause=NORMAL_CLEARING"
```

### Ejecutar script de prueba completo

```bash
cd /opt/ApoloBilling/rust-billing-engine
./scripts/test_simulator.sh
```

## Flujo de una Llamada Simulada

```
┌─────────────────────────────────────────────────────────────┐
│  POST /simulate/call                                        │
│  {caller, callee, direction, duration_seconds}             │
└─────────────────────────────────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────┐
│  FASE 1: CHANNEL_CREATE (T+0)                               │
│  ├─ AuthorizationService.authorize()                        │
│  │   ├─ Buscar cuenta por ANI (caller)                     │
│  │   ├─ Verificar estado cuenta                            │
│  │   ├─ Si INBOUND: autorizar sin billing                  │
│  │   ├─ Si OUTBOUND: buscar tarifa LPM                     │
│  │   └─ Crear reserva de balance                           │
│  │                                                          │
│  └─ INSERT INTO active_calls                                │
└─────────────────────────────────────────────────────────────┘
                              │
                    (ring_seconds)
                              │
                              ▼
┌─────────────────────────────────────────────────────────────┐
│  FASE 2: CHANNEL_ANSWER (T+ring_seconds)                   │
│  ├─ Marcar llamada como "answered"                          │
│  └─ Iniciar timer de duración                               │
└─────────────────────────────────────────────────────────────┘
                              │
                    (duration_seconds)
                              │
                              ▼
┌─────────────────────────────────────────────────────────────┐
│  FASE 3: CHANNEL_HANGUP_COMPLETE (T+ring+duration)         │
│  ├─ DELETE FROM active_calls                                │
│  │                                                          │
│  ├─ CdrGenerator.generate_cdr()                             │
│  │   ├─ Calcular billsec redondeado                        │
│  │   ├─ Calcular costo = (billsec/60) × rate               │
│  │   └─ INSERT INTO cdrs                                    │
│  │                                                          │
│  └─ ReservationManager.consume_reservation()                │
│      ├─ Actualizar balance de cuenta                        │
│      └─ Liberar reserva no usada                           │
└─────────────────────────────────────────────────────────────┘
```

## Diferencias INBOUND vs OUTBOUND

| Aspecto | OUTBOUND | INBOUND |
|---------|----------|---------|
| Cuenta identificada por | Caller (ANI) | Callee (DNIS) |
| Búsqueda de tarifa | Sí (LPM por callee) | No |
| Reserva de balance | Sí | No |
| Costo calculado | Sí | No (costo = 0) |
| CDR generado | Sí (con costo) | Sí (costo = null) |
| Descuento de balance | Sí | No |

## Causas de Hangup Comunes

| Causa | Descripción |
|-------|-------------|
| NORMAL_CLEARING | Colgado normal |
| USER_BUSY | Usuario ocupado |
| NO_ANSWER | Sin respuesta |
| CALL_REJECTED | Rechazada |
| ORIGINATOR_CANCEL | Cancelada por origen |
| DESTINATION_OUT_OF_ORDER | Destino fuera de servicio |

## Verificación en Base de Datos

```sql
-- Ver llamadas activas
SELECT * FROM active_calls ORDER BY start_time DESC;

-- Ver CDRs recientes
SELECT id, call_uuid, caller_number, called_number,
       direction, duration, billsec, cost, hangup_cause
FROM cdrs
WHERE created_at > NOW() - INTERVAL '1 hour'
ORDER BY created_at DESC;

-- Ver reservas activas
SELECT * FROM balance_reservations
WHERE status = 'active';

-- Ver transacciones de balance
SELECT * FROM balance_transactions
WHERE created_at > NOW() - INTERVAL '1 hour'
ORDER BY created_at DESC;
```

## Troubleshooting

### Llamada rechazada con "account_not_found"
- Verificar que existe una cuenta con el `account_number` igual al `caller`
- La cuenta debe estar en estado `ACTIVE`

### Llamada rechazada con "no_rate_found"
- Verificar que existe un `rate_card` con un prefijo que coincida con el `callee`
- El rate_card debe tener `effective_start <= NOW()` y `effective_end >= NOW()` (o NULL)

### Llamada rechazada con "insufficient_balance"
- Verificar que la cuenta tiene balance suficiente
- La reserva inicial es: `rate_per_minute × 5 min × 1.08`

### CDR no generado
- Revisar logs: `journalctl -u apolo-billing-engine -f`
- Verificar que la llamada llegó a CHANNEL_HANGUP_COMPLETE
