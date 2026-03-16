#!/usr/bin/env python3
"""
Script STANDALONE para sincronizar rate_cards
No depende de main.py ni weasyprint
"""
from sqlalchemy import create_engine, text, Column, Integer, String, Numeric, DateTime
from sqlalchemy.orm import declarative_base, sessionmaker
from datetime import datetime

DATABASE_URL = "postgresql://tarificador_user:fr4v4t3l@localhost/tarificador"

engine = create_engine(DATABASE_URL)
SessionLocal = sessionmaker(autocommit=False, autoflush=False, bind=engine)
Base = declarative_base()

# Definir modelos necesarios
class RateCard(Base):
    __tablename__ = "rate_cards"
    id = Column(Integer, primary_key=True, index=True)
    destination_prefix = Column(String, index=True)
    destination_name = Column(String)
    rate_per_minute = Column(Numeric(10, 5))
    billing_increment = Column(Integer, default=1)
    connection_fee = Column(Numeric(10, 5), default=0)
    effective_start = Column(DateTime, default=datetime.utcnow)
    effective_end = Column(DateTime, nullable=True)
    priority = Column(Integer, default=1)

def sync_rate_cards_standalone():
    print("🔧 Iniciando sincronización de rate_cards (Standalone)...")
    
    # Crear tabla si no existe
    Base.metadata.create_all(bind=engine)
    print("✅ Tabla rate_cards verificada/creada")
    
    db = SessionLocal()
    try:
        # 1. Limpiar rate cards
        print("🧹 Limpiando tabla rate_cards...")
        db.execute(text("TRUNCATE TABLE rate_cards RESTART IDENTITY"))
        
        # 2. Insertar datos
        print("📥 Insertando datos desde zonas/prefijos/tarifas...")
        query = text("""
            INSERT INTO rate_cards (
                destination_prefix, destination_name, rate_per_minute,
                billing_increment, connection_fee, effective_start, priority
            )
            SELECT 
                p.prefijo, 
                z.nombre, 
                t.tarifa_segundo * 60,
                1, 
                0, 
                t.fecha_inicio,
                LENGTH(p.prefijo)
            FROM prefijos p
            JOIN zonas z ON p.zona_id = z.id
            JOIN tarifas t ON z.id = t.zona_id
            WHERE t.activa = TRUE
        """)
        result = db.execute(query)
        rows = result.rowcount
        db.commit()
        
        print(f"✅ Sincronización completada. Se insertaron {rows} registros.")
        
        # Verificar contenido
        count = db.execute(text("SELECT COUNT(*) FROM rate_cards")).scalar()
        print(f"📊 Total en rate_cards: {count}")
        
    except Exception as e:
        print(f"❌ Error: {e}")
        db.rollback()
    finally:
        db.close()

if __name__ == "__main__":
    sync_rate_cards_standalone()
