#!/usr/bin/env python3
from sqlalchemy import create_engine, text, inspect
import sys

try:
    engine = create_engine('sqlite:///./tarificador.db')
    inspector = inspect(engine)
    
    # Listar todas las tablas
    tables = inspector.get_table_names()
    
    print(f"📊 Total de tablas en la base de datos: {len(tables)}")
    print()
    
    if tables:
        print("📋 Tablas encontradas:")
        for table in tables:
            print(f"  • {table}")
            
            # Contar registros en cada tabla
            with engine.connect() as conn:
                try:
                    result = conn.execute(text(f'SELECT COUNT(*) FROM {table}'))
                    count = result.scalar()
                    print(f"    └─ Registros: {count}")
                except Exception as e:
                    print(f"    └─ Error al contar: {e}")
        print()
    else:
        print("⚠️  No se encontraron tablas en la base de datos")
        print()
    
    # Verificar si existen las tablas necesarias
    required_tables = ['zonas', 'prefijos', 'tarifas', 'cdr', 'users', 'accounts']
    print("🔍 Verificando tablas requeridas:")
    for table in required_tables:
        exists = table in tables
        status = "✅" if exists else "❌"
        print(f"  {status} {table}")
    
except Exception as e:
    print(f"❌ Error: {e}")
    sys.exit(1)
