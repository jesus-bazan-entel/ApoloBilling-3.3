#!/usr/bin/env python3
"""
Script para inicializar la base de datos PostgreSQL
Crea todas las tablas necesarias e inicializa las zonas y prefijos
"""

from sqlalchemy import create_engine, Column, Integer, String, Numeric, DateTime, text, Boolean, ForeignKey
from sqlalchemy.orm import declarative_base, sessionmaker, relationship
from datetime import datetime

# Configuración de base de datos
DATABASE_URL = "postgresql://tarificador_user:fr4v4t3l@localhost/tarificador"

engine = create_engine(DATABASE_URL)
SessionLocal = sessionmaker(autocommit=False, autoflush=False, bind=engine)
Base = declarative_base()

# Definir modelos (copiados de main.py)
class Zona(Base):
    __tablename__ = "zonas"
    id = Column(Integer, primary_key=True, index=True)
    nombre = Column(String, unique=True)
    descripcion = Column(String)
    prefijos = relationship("Prefijo", back_populates="zona")
    tarifas = relationship("Tarifa", back_populates="zona")

class Prefijo(Base):
    __tablename__ = "prefijos"
    id = Column(Integer, primary_key=True, index=True)
    zona_id = Column(Integer, ForeignKey("zonas.id"))
    prefijo = Column(String)
    longitud_minima = Column(Integer)
    longitud_maxima = Column(Integer)
    zona = relationship("Zona", back_populates="prefijos")

class Tarifa(Base):
    __tablename__ = "tarifas"
    id = Column(Integer, primary_key=True, index=True)
    zona_id = Column(Integer, ForeignKey("zonas.id"))
    tarifa_segundo = Column(Numeric(10, 5))
    fecha_inicio = Column(DateTime, default=datetime.utcnow)
    activa = Column(Boolean, default=True)
    zona = relationship("Zona", back_populates="tarifas")

def inicializar_zonas_y_prefijos():
    db = SessionLocal()
    
    try:
        # Verificar si ya existen zonas
        check_query = text("SELECT COUNT(*) FROM zonas")
        count = db.execute(check_query).scalar()
        
        if count > 0:
            print(f"ℹ️  Ya existen {count} zonas en la base de datos. No se inicializará nuevamente.")
            db.close()
            return
        
        # Crear zonas iniciales
        zonas = [
            {"nombre": "Local", "descripcion": "Llamadas locales de 7 dígitos", "tarifa": 0.00015},
            {"nombre": "Movil", "descripcion": "Llamadas a celulares", "tarifa": 0.00030},
            {"nombre": "LDN", "descripcion": "Larga Distancia Nacional", "tarifa": 0.00050},
            {"nombre": "LDI", "descripcion": "Larga Distancia Internacional", "tarifa": 0.00120},
            {"nombre": "Emergencia", "descripcion": "Números de emergencia", "tarifa": 0.00000},
            {"nombre": "0800", "descripcion": "Números gratuitos", "tarifa": 0.00000}
        ]
        
        zonas_ids = {}
        
        for zona_data in zonas:
            # Insertar zona
            insert_zona_query = text("""
                INSERT INTO zonas (nombre, descripcion) 
                VALUES (:nombre, :descripcion)
                RETURNING id
            """)
            
            result = db.execute(insert_zona_query, {
                "nombre": zona_data["nombre"],
                "descripcion": zona_data["descripcion"]
            })
            
            zona_id = result.fetchone()[0]
            zonas_ids[zona_data["nombre"]] = zona_id
            
            # Insertar tarifa para la zona
            insert_tarifa_query = text("""
                INSERT INTO tarifas (zona_id, tarifa_segundo, fecha_inicio, activa)
                VALUES (:zona_id, :tarifa_segundo, CURRENT_TIMESTAMP, TRUE)
            """)
            
            db.execute(insert_tarifa_query, {
                "zona_id": zona_id,
                "tarifa_segundo": zona_data["tarifa"]
            })
        
        # Insertar prefijos
        prefijos = [
            # Local - números fijos 7 dígitos (2-9)XXXXXX
            {"zona_id": zonas_ids["Local"], "prefijo": "2", "longitud_minima": 7, "longitud_maxima": 7},
            {"zona_id": zonas_ids["Local"], "prefijo": "3", "longitud_minima": 7, "longitud_maxima": 7},
            {"zona_id": zonas_ids["Local"], "prefijo": "4", "longitud_minima": 7, "longitud_maxima": 7},
            {"zona_id": zonas_ids["Local"], "prefijo": "5", "longitud_minima": 7, "longitud_maxima": 7},
            {"zona_id": zonas_ids["Local"], "prefijo": "6", "longitud_minima": 7, "longitud_maxima": 7},
            {"zona_id": zonas_ids["Local"], "prefijo": "7", "longitud_minima": 7, "longitud_maxima": 7},
            {"zona_id": zonas_ids["Local"], "prefijo": "8", "longitud_minima": 7, "longitud_maxima": 7},
            {"zona_id": zonas_ids["Local"], "prefijo": "9", "longitud_minima": 7, "longitud_maxima": 7},
            
            # Móvil - 9 dígitos 9XXXXXXXX
            {"zona_id": zonas_ids["Movil"], "prefijo": "9", "longitud_minima": 9, "longitud_maxima": 9},
            
            # LDN - 0[4-8]XXXXXXX
            {"zona_id": zonas_ids["LDN"], "prefijo": "04", "longitud_minima": 9, "longitud_maxima": 10},
            {"zona_id": zonas_ids["LDN"], "prefijo": "05", "longitud_minima": 9, "longitud_maxima": 10},
            {"zona_id": zonas_ids["LDN"], "prefijo": "06", "longitud_minima": 9, "longitud_maxima": 10},
            {"zona_id": zonas_ids["LDN"], "prefijo": "07", "longitud_minima": 9, "longitud_maxima": 10},
            {"zona_id": zonas_ids["LDN"], "prefijo": "08", "longitud_minima": 9, "longitud_maxima": 10},
            
            # LDI - 00[1-9]XXXXXXX.... (10-15 dígitos)
            {"zona_id": zonas_ids["LDI"], "prefijo": "001", "longitud_minima": 10, "longitud_maxima": 15},
            {"zona_id": zonas_ids["LDI"], "prefijo": "002", "longitud_minima": 10, "longitud_maxima": 15},
            {"zona_id": zonas_ids["LDI"], "prefijo": "003", "longitud_minima": 10, "longitud_maxima": 15},
            {"zona_id": zonas_ids["LDI"], "prefijo": "004", "longitud_minima": 10, "longitud_maxima": 15},
            {"zona_id": zonas_ids["LDI"], "prefijo": "005", "longitud_minima": 10, "longitud_maxima": 15},
            {"zona_id": zonas_ids["LDI"], "prefijo": "006", "longitud_minima": 10, "longitud_maxima": 15},
            {"zona_id": zonas_ids["LDI"], "prefijo": "007", "longitud_minima": 10, "longitud_maxima": 15},
            {"zona_id": zonas_ids["LDI"], "prefijo": "008", "longitud_minima": 10, "longitud_maxima": 15},
            {"zona_id": zonas_ids["LDI"], "prefijo": "009", "longitud_minima": 10, "longitud_maxima": 15},
            
            # Emergencia 1XX
            {"zona_id": zonas_ids["Emergencia"], "prefijo": "1", "longitud_minima": 3, "longitud_maxima": 3},
            
            # 0800 - 0800XXXXX
            {"zona_id": zonas_ids["0800"], "prefijo": "0800", "longitud_minima": 9, "longitud_maxima": 9},
        ]
        
        for prefijo_data in prefijos:
            insert_prefijo_query = text("""
                INSERT INTO prefijos (zona_id, prefijo, longitud_minima, longitud_maxima)
                VALUES (:zona_id, :prefijo, :longitud_minima, :longitud_maxima)
            """)
            
            db.execute(insert_prefijo_query, {
                "zona_id": prefijo_data["zona_id"],
                "prefijo": prefijo_data["prefijo"],
                "longitud_minima": prefijo_data["longitud_minima"],
                "longitud_maxima": prefijo_data["longitud_maxima"]
            })
        
        db.commit()
        print("✅ Zonas y prefijos inicializados correctamente")
        
    except Exception as e:
        db.rollback()
        raise e
    finally:
        db.close()

if __name__ == "__main__":
    try:
        print("🔧 Iniciando creación de base de datos PostgreSQL...")
        print()
        
        # Crear todas las tablas
        print("📋 Creando tablas...")
        Base.metadata.create_all(bind=engine)
        print("✅ Tablas creadas exitosamente")
        print()
        
        # Verificar tablas creadas
        from sqlalchemy import inspect
        inspector = inspect(engine)
        tables = inspector.get_table_names()
        
        print(f"📊 Tablas en la base de datos ({len(tables)}):")
        for table in sorted(tables):
            print(f"  ✓ {table}")
        print()
        
        # Inicializar zonas y prefijos
        print("🌍 Inicializando zonas y prefijos...")
        inicializar_zonas_y_prefijos()
        print()
        
        # Verificar datos insertados
        db = SessionLocal()
        
        zonas_count = db.execute(text('SELECT COUNT(*) FROM zonas')).scalar()
        prefijos_count = db.execute(text('SELECT COUNT(*) FROM prefijos')).scalar()
        tarifas_count = db.execute(text('SELECT COUNT(*) FROM tarifas')).scalar()
        
        print("📈 Datos inicializados:")
        print(f"  • Zonas: {zonas_count}")
        print(f"  • Prefijos: {prefijos_count}")
        print(f"  • Tarifas: {tarifas_count}")
        print()
        
        # Listar zonas
        result = db.execute(text('SELECT id, nombre, descripcion FROM zonas ORDER BY id'))
        zonas = result.fetchall()
        
        print("🌍 Zonas creadas:")
        for zona in zonas:
            print(f"  • {zona[1]} (ID: {zona[0]}): {zona[2]}")
        
        db.close()
        
        print()
        print("✅ ¡Base de datos inicializada correctamente!")
        print()
        print("💡 Las zonas ahora deberían aparecer al agregar un nuevo prefijo")
        print("   Si la aplicación está corriendo, las zonas ya están disponibles.")
        
    except Exception as e:
        print(f"❌ Error durante la inicialización: {e}")
        import traceback
        traceback.print_exc()
        exit(1)
