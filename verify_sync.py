#!/usr/bin/env python3
"""
Script para verificar y sincronizar rate_cards
"""
import sys
# Importar paths
sys.path.insert(0, '/home/jbazan/ApoloBilling/TarificadorFreeswitch')

from sqlalchemy import create_engine, text
from main import Base, engine, SessionLocal, RateCard, sync_rate_cards

def check_rate_cards():
    print("🔧 Verificando tabla rate_cards...")
    
    # Asegurar que la tabla existe
    Base.metadata.create_all(bind=engine)
    
    db = SessionLocal()
    try:
        # Sincronizar
        print("🔄 Ejecutando sincronización...")
        sync_rate_cards(db)
        
        # Verificar contenido
        count = db.execute(text("SELECT COUNT(*) FROM rate_cards")).scalar()
        print(f"✅ Total rate_cards: {count}")
        
        # Mostrar algunos ejemplos
        rows = db.execute(text("SELECT destination_prefix, destination_name, rate_per_minute FROM rate_cards LIMIT 5")).fetchall()
        print("📋 Ejemplos:")
        for r in rows:
            print(f"  • {r[0]} ({r[1]}): ${r[2]}/min")
            
    except Exception as e:
        print(f"❌ Error: {e}")
    finally:
        db.close()

if __name__ == "__main__":
    check_rate_cards()
