#!/usr/bin/env python3
from sqlalchemy import create_engine, text
import sys

# Intentar conectar a apolobilling con las mismas credenciales
DATABASE_URL = "postgresql://tarificador_user:fr4v4t3l@localhost/apolobilling"

try:
    engine = create_engine(DATABASE_URL)
    with engine.connect() as conn:
        print("✅ Conexión exitosa a 'apolobilling'")
        
        # Listar tablas
        result = conn.execute(text("SELECT tablename FROM pg_catalog.pg_tables WHERE schemaname = 'public'"))
        tables = [r[0] for r in result.fetchall()]
        print(f"📊 Tablas encontradas: {tables}")
        
        # Verificar zones
        if 'zones' in tables:
            count = conn.execute(text("SELECT COUNT(*) FROM zones")).scalar()
            print(f"🌍 Registros en 'zones': {count}")
            
except Exception as e:
    print(f"❌ Error conectando a 'apolobilling': {e}")
