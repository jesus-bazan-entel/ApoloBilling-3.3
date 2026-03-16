# 🧪 Testing Tools - Apolo Billing Engine

Herramientas para testing del motor de billing Rust.

## 📁 Contenido

### `esl_simulator.py`
Simulador de eventos ESL de FreeSWITCH para testing del motor de billing sin necesidad de FreeSWITCH real.

**Uso:**
```bash
# Prueba básica
./tools/esl_simulator.py --duration 30

# Prueba completa
./tools/esl_simulator.py --duration 60 --calls 5 --delay 10

# Personalizada
./tools/esl_simulator.py \
    --caller 100001 \
    --callee 51987654321 \
    --duration 120 \
    --account 100001
```

### `test_billing_engine.sh`
Script automatizado que:
1. Verifica prerequisitos (PostgreSQL, Redis)
2. Prepara base de datos de prueba
3. Compila motor Rust si es necesario
4. Ejecuta pruebas del simulador

**Uso:**
```bash
./tools/test_billing_engine.sh
```

## 📖 Documentación Completa

Ver: `TESTING_BILLING_ENGINE.md`

## 🚀 Quick Start

```bash
# Terminal 1: Iniciar motor Rust
cd rust-billing-engine
RUST_LOG=info cargo run

# Terminal 2: Ejecutar prueba
./tools/test_billing_engine.sh
```
