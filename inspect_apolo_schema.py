#!/usr/bin/env python3
from sqlalchemy import create_engine, text, inspect
import sys

# Credenciales encontradas en rust-billing-engine/.env
DATABASE_URL = "postgresql://apolo:apolo123@localhost/apolobilling"

try:
    engine = create_engine(DATABASE_URL)
    inspector = inspect(engine)
    
    print("✅ Conexión exitosa a 'apolobilling'")
    
    tables = ['zones', 'prefixes', 'rate_cards', 'rate_zones']
    
    for table_name in tables:
        print(f"\n📋 Schema de '{table_name}':")
        try:
            columns = inspector.get_columns(table_name)
            for col in columns:
                print(f"  - {col['name']} ({col['type']})")
                
            # Muestra 1 fila de ejemplo
            with engine.connect() as conn:
                result = conn.execute(text(f"SELECT * FROM {table_name} LIMIT 1"))
                row = result.fetchone()
                if row:
                    print(f"  📝 Ejemplo: {row}")
                else:
                    print(f"  ⚠️ Tabla vacía")
        except Exception as e:
            print(f"  ❌ Error: {e}")

except Exception as e:
    print(f"❌ Error conectando a 'apolobilling': {e}")
