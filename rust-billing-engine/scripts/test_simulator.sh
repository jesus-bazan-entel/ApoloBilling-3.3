#!/bin/bash
#
# Script de prueba del simulador de llamadas
# Este script demuestra el flujo completo de llamadas entrantes y salientes
#

set -e

# Colores para output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

BILLING_ENGINE_URL="${BILLING_ENGINE_URL:-http://localhost:9000}"
BACKEND_URL="${BACKEND_URL:-http://localhost:8000}"

echo -e "${BLUE}========================================${NC}"
echo -e "${BLUE}  ApoloBilling - Simulador de Llamadas ${NC}"
echo -e "${BLUE}========================================${NC}"
echo ""

# Verificar que el billing engine esté corriendo
echo -e "${YELLOW}[1/7] Verificando servicios...${NC}"
if ! curl -s "${BILLING_ENGINE_URL}/api/v1/health" > /dev/null 2>&1; then
    echo -e "${RED}ERROR: Billing Engine no responde en ${BILLING_ENGINE_URL}${NC}"
    echo "Inicia el billing engine con: cd /opt/ApoloBilling/rust-billing-engine && cargo run"
    exit 1
fi
echo -e "${GREEN}✓ Billing Engine OK${NC}"

# ==========================================
# ESCENARIO 1: Llamada saliente exitosa
# ==========================================
echo ""
echo -e "${BLUE}========================================${NC}"
echo -e "${BLUE}  ESCENARIO 1: Llamada OUTBOUND exitosa ${NC}"
echo -e "${BLUE}========================================${NC}"

echo -e "${YELLOW}[2/7] Simulando llamada OUTBOUND (30 segundos)...${NC}"
echo "  Caller: 51999888777 (cuenta válida)"
echo "  Callee: 5491155551234 (Argentina Móvil)"

RESULT=$(curl -s -X POST "${BILLING_ENGINE_URL}/api/v1/simulate/call" \
    -H "Content-Type: application/json" \
    -d '{
        "caller": "51999888777",
        "callee": "5491155551234",
        "direction": "outbound",
        "duration_seconds": 30,
        "ring_seconds": 2,
        "hangup_cause": "NORMAL_CLEARING"
    }')

echo -e "${GREEN}Respuesta:${NC}"
echo "$RESULT" | jq '.' 2>/dev/null || echo "$RESULT"

CALL_UUID=$(echo "$RESULT" | jq -r '.call_uuid' 2>/dev/null)
if [ "$CALL_UUID" != "null" ] && [ -n "$CALL_UUID" ]; then
    echo -e "${GREEN}✓ Llamada iniciada: ${CALL_UUID}${NC}"
else
    echo -e "${RED}✗ Error al iniciar llamada${NC}"
fi

# ==========================================
# ESCENARIO 2: Llamada entrante (sin billing)
# ==========================================
echo ""
echo -e "${BLUE}========================================${NC}"
echo -e "${BLUE}  ESCENARIO 2: Llamada INBOUND (sin billing)${NC}"
echo -e "${BLUE}========================================${NC}"

echo -e "${YELLOW}[3/7] Simulando llamada INBOUND (20 segundos)...${NC}"
echo "  Caller: 5491100000000 (externo)"
echo "  Callee: 51999888777 (cuenta local)"

RESULT=$(curl -s -X POST "${BILLING_ENGINE_URL}/api/v1/simulate/call" \
    -H "Content-Type: application/json" \
    -d '{
        "caller": "5491100000000",
        "callee": "51999888777",
        "direction": "inbound",
        "duration_seconds": 20,
        "ring_seconds": 3,
        "hangup_cause": "NORMAL_CLEARING"
    }')

echo -e "${GREEN}Respuesta:${NC}"
echo "$RESULT" | jq '.' 2>/dev/null || echo "$RESULT"

# ==========================================
# ESCENARIO 3: Llamada rechazada (sin saldo)
# ==========================================
echo ""
echo -e "${BLUE}========================================${NC}"
echo -e "${BLUE}  ESCENARIO 3: Llamada rechazada (cuenta inexistente)${NC}"
echo -e "${BLUE}========================================${NC}"

echo -e "${YELLOW}[4/7] Simulando llamada con cuenta inexistente...${NC}"
echo "  Caller: 0000000000 (no existe)"
echo "  Callee: 5491155551234"

RESULT=$(curl -s -X POST "${BILLING_ENGINE_URL}/api/v1/simulate/call" \
    -H "Content-Type: application/json" \
    -d '{
        "caller": "0000000000",
        "callee": "5491155551234",
        "direction": "outbound",
        "duration_seconds": 60
    }')

echo -e "${GREEN}Respuesta:${NC}"
echo "$RESULT" | jq '.' 2>/dev/null || echo "$RESULT"

SUCCESS=$(echo "$RESULT" | jq -r '.success' 2>/dev/null)
if [ "$SUCCESS" == "false" ]; then
    echo -e "${GREEN}✓ Llamada rechazada correctamente${NC}"
else
    echo -e "${YELLOW}⚠ Comportamiento inesperado${NC}"
fi

# ==========================================
# Listar llamadas activas
# ==========================================
echo ""
echo -e "${BLUE}========================================${NC}"
echo -e "${BLUE}  Llamadas activas en simulador ${NC}"
echo -e "${BLUE}========================================${NC}"

echo -e "${YELLOW}[5/7] Consultando llamadas activas...${NC}"
sleep 2  # Esperar un poco para que las llamadas se procesen

ACTIVE_CALLS=$(curl -s "${BILLING_ENGINE_URL}/api/v1/simulate/calls")
echo "$ACTIVE_CALLS" | jq '.' 2>/dev/null || echo "$ACTIVE_CALLS"

# ==========================================
# ESCENARIO 4: Escenario batch
# ==========================================
echo ""
echo -e "${BLUE}========================================${NC}"
echo -e "${BLUE}  ESCENARIO 4: Batch de llamadas ${NC}"
echo -e "${BLUE}========================================${NC}"

echo -e "${YELLOW}[6/7] Ejecutando escenario con múltiples llamadas...${NC}"

RESULT=$(curl -s -X POST "${BILLING_ENGINE_URL}/api/v1/simulate/scenario" \
    -H "Content-Type: application/json" \
    -d '{
        "name": "Test Mix Inbound/Outbound",
        "calls": [
            {
                "caller": "51999888777",
                "callee": "5491155550001",
                "direction": "outbound",
                "duration_seconds": 15,
                "delay_before_ms": 0
            },
            {
                "caller": "5491155550002",
                "callee": "51999888777",
                "direction": "inbound",
                "duration_seconds": 10,
                "delay_before_ms": 500
            },
            {
                "caller": "51999888777",
                "callee": "5491155550003",
                "direction": "outbound",
                "duration_seconds": 25,
                "delay_before_ms": 1000
            }
        ]
    }')

echo -e "${GREEN}Resultado del escenario:${NC}"
echo "$RESULT" | jq '.' 2>/dev/null || echo "$RESULT"

# ==========================================
# Verificar CDRs generados
# ==========================================
echo ""
echo -e "${BLUE}========================================${NC}"
echo -e "${BLUE}  Verificando CDRs generados ${NC}"
echo -e "${BLUE}========================================${NC}"

echo -e "${YELLOW}[7/7] Esperando a que terminen las llamadas y se generen CDRs...${NC}"
sleep 35  # Esperar que terminen las llamadas más largas

# Consultar CDRs recientes en el backend si está disponible
if curl -s "${BACKEND_URL}/api/v1/health" > /dev/null 2>&1; then
    echo -e "${GREEN}Consultando CDRs recientes (últimos 5 minutos)...${NC}"

    # Primero hacer login para obtener cookie
    COOKIES=$(mktemp)
    curl -s -c "$COOKIES" -X POST "${BACKEND_URL}/api/v1/auth/login" \
        -H "Content-Type: application/json" \
        -d '{"username":"admin","password":"admin123"}' > /dev/null 2>&1

    # Consultar CDRs
    CDRS=$(curl -s -b "$COOKIES" "${BACKEND_URL}/api/v1/cdrs?per_page=10")

    echo -e "${GREEN}CDRs recientes:${NC}"
    echo "$CDRS" | jq '.data | .[] | {call_uuid, caller_number, called_number, direction, duration, billsec, cost, hangup_cause}' 2>/dev/null || echo "$CDRS"

    rm -f "$COOKIES"
else
    echo -e "${YELLOW}Backend no disponible - verificar CDRs manualmente${NC}"
fi

# Limpiar simulaciones completadas
echo ""
echo -e "${YELLOW}Limpiando simulaciones completadas...${NC}"
curl -s -X POST "${BILLING_ENGINE_URL}/api/v1/simulate/cleanup" > /dev/null

echo ""
echo -e "${GREEN}========================================${NC}"
echo -e "${GREEN}  Simulación completada ${NC}"
echo -e "${GREEN}========================================${NC}"
echo ""
echo "Para más detalles, consulte:"
echo "  - Llamadas activas: GET ${BILLING_ENGINE_URL}/api/v1/simulate/calls"
echo "  - CDRs: GET ${BACKEND_URL}/api/v1/cdrs"
echo "  - Logs: journalctl -u apolo-billing-engine -f"
