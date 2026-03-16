#!/usr/bin/env python3
"""
Apolo Billing - Clean Database Initialization
No legacy tables (zones, prefixes, rate_zones)
Rate Cards is the single source of truth
"""

from sqlalchemy import create_engine, text
import sys
import os

# Add parent directory to path
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))

try:
    from app.core.config import settings
    engine = create_engine(settings.DATABASE_URL)
except Exception as e:
    print(f"❌ Error loading config: {e}")
    print("Using default DATABASE_URL")
    engine = create_engine("postgresql://apolo_user:apolo_password_2024@localhost/apolo_billing")

# SQL para crear solo las tablas necesarias (sin legacy)
sql_commands = """
-- Tabla de usuarios
CREATE TABLE IF NOT EXISTS users (
    id SERIAL PRIMARY KEY,
    username VARCHAR(50) UNIQUE NOT NULL,
    email VARCHAR(100) UNIQUE NOT NULL,
    hashed_password VARCHAR(255) NOT NULL,
    role VARCHAR(20) DEFAULT 'user',
    is_active BOOLEAN DEFAULT true,
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);

-- Tabla de cuentas (accounts)
CREATE TABLE IF NOT EXISTS accounts (
    id SERIAL PRIMARY KEY,
    account_number VARCHAR(50) UNIQUE NOT NULL,
    account_name VARCHAR(100),
    balance NUMERIC(10, 2) DEFAULT 0.00,
    account_type VARCHAR(20) DEFAULT 'PREPAID',
    status VARCHAR(20) DEFAULT 'ACTIVE',
    max_concurrent_calls INTEGER DEFAULT 5,
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);

-- Tabla de rate_cards (ÚNICA fuente de verdad para tarifas)
CREATE TABLE IF NOT EXISTS rate_cards (
    id SERIAL PRIMARY KEY,
    destination_prefix VARCHAR(20) NOT NULL,
    destination_name VARCHAR(100) NOT NULL,
    rate_per_minute NUMERIC(10, 4) NOT NULL,
    billing_increment INTEGER DEFAULT 60,
    connection_fee NUMERIC(10, 4) DEFAULT 0.0000,
    effective_start TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    effective_end TIMESTAMP,
    priority INTEGER DEFAULT 100,
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);

-- Índices para búsqueda rápida
CREATE INDEX IF NOT EXISTS idx_rate_cards_prefix ON rate_cards(destination_prefix);
CREATE INDEX IF NOT EXISTS idx_rate_cards_priority ON rate_cards(priority DESC);
CREATE INDEX IF NOT EXISTS idx_rate_cards_effective ON rate_cards(effective_start, effective_end);

-- Tabla de reservaciones de balance
CREATE TABLE IF NOT EXISTS balance_reservations (
    id SERIAL PRIMARY KEY,
    call_uuid UUID UNIQUE NOT NULL,
    account_id INTEGER REFERENCES accounts(id),
    reserved_amount NUMERIC(10, 4) NOT NULL,
    status VARCHAR(20) DEFAULT 'ACTIVE',
    expires_at TIMESTAMP NOT NULL,
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);

-- Tabla de transacciones de balance
CREATE TABLE IF NOT EXISTS balance_transactions (
    id SERIAL PRIMARY KEY,
    account_id INTEGER REFERENCES accounts(id),
    amount NUMERIC(10, 4) NOT NULL,
    transaction_type VARCHAR(20) NOT NULL,
    reason TEXT,
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);

-- Tabla de CDRs
CREATE TABLE IF NOT EXISTS cdrs (
    id SERIAL PRIMARY KEY,
    call_uuid UUID UNIQUE NOT NULL,
    account_id INTEGER REFERENCES accounts(id),
    caller_number VARCHAR(50),
    called_number VARCHAR(50),
    destination_prefix VARCHAR(20),
    start_time TIMESTAMP,
    answer_time TIMESTAMP,
    end_time TIMESTAMP,
    duration INTEGER,
    billsec INTEGER,
    rate_per_minute NUMERIC(10, 4),
    billing_increment INTEGER,
    cost NUMERIC(10, 4),
    hangup_cause VARCHAR(50),
    created_at TIMESTAMP DEFAULT CURRENT_TIMESTAMP
);

-- Índices para CDRs
CREATE INDEX IF NOT EXISTS idx_cdrs_account ON cdrs(account_id);
CREATE INDEX IF NOT EXISTS idx_cdrs_call_uuid ON cdrs(call_uuid);
CREATE INDEX IF NOT EXISTS idx_cdrs_start_time ON cdrs(start_time);

-- Insertar usuario admin
INSERT INTO users (username, email, hashed_password, role, is_active)
VALUES ('admin', 'admin@apolobilling.com', 
        '$2b$12$LQv3c1yqBWVHxkd0LHAkCOYz6TtxMQJqhN8/LewY5GyYfQw8Q8oSW',
        'superadmin', true)
ON CONFLICT (username) DO NOTHING;

-- Insertar rate cards de ejemplo (Perú)
INSERT INTO rate_cards (destination_prefix, destination_name, rate_per_minute, billing_increment, priority)
VALUES 
    ('51983', 'Perú Móvil Claro', 0.0850, 6, 150),
    ('51982', 'Perú Móvil Claro', 0.0850, 6, 150),
    ('51981', 'Perú Móvil Claro', 0.0850, 6, 150),
    ('51980', 'Perú Móvil Claro', 0.0850, 6, 150),
    ('51999', 'Perú Móvil Movistar', 0.0800, 6, 150),
    ('51998', 'Perú Móvil Movistar', 0.0800, 6, 150),
    ('51997', 'Perú Móvil Movistar', 0.0800, 6, 150),
    ('51996', 'Perú Móvil Movistar', 0.0800, 6, 150),
    ('511', 'Perú Lima Fijo', 0.0200, 6, 100),
    ('51', 'Perú Nacional', 0.0500, 6, 50),
    ('1', 'USA/Canada', 0.0100, 60, 100),
    ('52', 'México', 0.0300, 60, 100),
    ('34', 'España', 0.0250, 60, 100)
ON CONFLICT DO NOTHING;

-- Insertar cuenta de ejemplo
INSERT INTO accounts (account_number, account_name, balance, account_type, status)
VALUES ('100001', 'Cuenta Demo', 100.00, 'PREPAID', 'ACTIVE')
ON CONFLICT (account_number) DO NOTHING;

COMMIT;
"""

def main():
    print("🗄️  Inicializando base de datos limpia (sin legacy)...")
    print("")
    
    try:
        with engine.connect() as conn:
            # Ejecutar cada comando SQL individualmente
            commands = [cmd.strip() for cmd in sql_commands.split(';') if cmd.strip()]
            
            for command in commands:
                # Saltar comentarios
                if command.startswith('--') or not command:
                    continue
                
                try:
                    conn.execute(text(command))
                    conn.commit()
                except Exception as e:
                    # Si el error es que la tabla/índice ya existe, continuar
                    if "already exists" in str(e):
                        continue
                    else:
                        raise
        
        print("✅ Base de datos inicializada correctamente")
        print("")
        print("📊 Tablas creadas:")
        print("   ✅ users")
        print("   ✅ accounts")
        print("   ✅ rate_cards (Principal)")
        print("   ✅ balance_reservations")
        print("   ✅ balance_transactions")
        print("   ✅ cdrs")
        print("")
        print("❌ Tablas legacy NO creadas:")
        print("   🗑️  zones")
        print("   🗑️  prefixes")
        print("   🗑️  rate_zones")
        print("   🗑️  countries")
        print("")
        print("👤 Usuario admin:")
        print("   Username: admin")
        print("   Password: admin123")
        print("")
        print("💳 13 Rate cards de ejemplo insertadas")
        print("   • Perú (móvil, fijo, nacional)")
        print("   • USA/Canada")
        print("   • México")
        print("   • España")
        print("")
        print("✨ Base de datos lista para usar")
        
    except Exception as e:
        print(f"❌ Error: {e}")
        import traceback
        traceback.print_exc()
        return 1
    
    return 0

if __name__ == "__main__":
    sys.exit(main())
