from fastapi import FastAPI, Depends, Request, Form, Query, UploadFile, File, HTTPException
from fastapi.responses import RedirectResponse, StreamingResponse
from fastapi.templating import Jinja2Templates
from fastapi_login import LoginManager
from fastapi.staticfiles import StaticFiles
from passlib.context import CryptContext
from sqlalchemy import create_engine, Column, Integer, String, Numeric, DateTime, text, Boolean, ForeignKey, and_
from sqlalchemy.orm import declarative_base, sessionmaker, relationship
from datetime import datetime, timedelta
from collections import Counter
import io
import csv
import os
import asyncio
from typing import Optional, List, Dict, Union, Any
from pydantic import BaseModel, validator, Field, ValidationError
from decimal import Decimal
from fastapi import WebSocket, WebSocketDisconnect
import logging

# Configurar logging al inicio del archivo
logging.basicConfig(
    level=logging.INFO,
    format='%(asctime)s - %(name)s - %(levelname)s - %(message)s'
)
logger = logging.getLogger(__name__)


# Custom Jinja2 filters
def lt_filter(value, threshold):
    try:
        return float(value) < float(threshold)
    except (ValueError, TypeError):
        return False

def ternary_filter(value, true_val, false_val):
    return true_val if value else false_val

DATABASE_URL = "postgresql://apolo:apolo123@localhost/apolobilling"
SECRET = "secreto-super-importante"

engine = create_engine(DATABASE_URL)
SessionLocal = sessionmaker(autocommit=False, autoflush=False, bind=engine)
Base = declarative_base()
app = FastAPI()
# Montar archivos estáticos
app.mount("/static", StaticFiles(directory="static"), name="static")

templates = Jinja2Templates(directory="templates")
templates.env.filters['lt'] = lt_filter
templates.env.filters['ternary'] = ternary_filter
manager = LoginManager(SECRET, token_url="/auth/login", use_cookie=True)
manager.cookie_name = "auth_token"
pwd_context = CryptContext(schemes=["bcrypt"], deprecated="auto")

# Add custom filters to Jinja2 environment
templates.env.filters['lt'] = lt_filter
templates.env.filters['ternary'] = ternary_filter

# Modelo para validar llamadas activas
class ActiveCall(BaseModel):
    callId: str
    origin: str
    destination: str
    startTime: str
    duration: int
    estimatedCost: float
    zone: Optional[str] = "Desconocida"

# Gestor de conexiones WebSocket
class ConnectionManager:
    def __init__(self):
        self.active_connections: List[WebSocket] = []
        self._lock = asyncio.Lock()  # Añadir un lock para evitar condiciones de carrera

    async def connect(self, websocket: WebSocket) -> None:
        await websocket.accept()
        async with self._lock:
            self.active_connections.append(websocket)

    def disconnect(self, websocket: WebSocket) -> None:
        if websocket in self.active_connections:
            self.active_connections.remove(websocket)

    async def broadcast(self, message: dict) -> None:
        closed_connections = []
        
        for connection in self.active_connections:
            try:
                await connection.send_json(message)
            except Exception:
                # Marcar la conexión para eliminarla después
                closed_connections.append(connection)
                
        # Eliminar las conexiones cerradas
        for conn in closed_connections:
            self.disconnect(conn)

ws_manager = ConnectionManager()

# Almacenamiento de llamadas activas con timestamp
active_calls: Dict[str, Dict] = {}
last_update = datetime.now()

# Estadísticas de WebSocket
ws_stats = {
    "total_connections": 0,
    "messages_sent": 0,
    "messages_received": 0
}

@app.get("/api/active-calls")
async def get_active_calls():
    """Obtiene la lista de llamadas activas para la API"""
    try:
        db = SessionLocal()
        
        # ✅ Agregar direction a la consulta SELECT
        active_calls_rows = db.execute(
            text("""SELECT call_id, calling_number, called_number, direction, start_time, 
            current_duration, current_cost, zone
            FROM active_calls ORDER BY start_time DESC""")
        ).fetchall()
        
        active_calls = []
        for row in active_calls_rows:
            # ✅ Extraer direction (nuevo campo en posición 3)
            direction = row[3] if len(row) > 3 and row[3] else "unknown"
            
            # ✅ Crear display version con iconos
            direction_displays = {
                "inbound": "📱 Entrante",
                "outbound": "📞 Saliente", 
                "internal": "🏢 Interna",
                "transit": "🔄 Tránsito"
            }
            direction_display = direction_displays.get(direction, f"❓ {direction}")
            
            call = {
                "call_id": row[0],
                "calling_number": row[1],
                "called_number": row[2],
                "direction": direction,                    # ✅ Campo direction
                "direction_display": direction_display,   # ✅ Para mostrar en UI
                "start_time": row[4].isoformat() if row[4] else None,  # ✅ Ajustar índice
                "current_duration": row[5] if row[5] is not None else 0,  # ✅ Ajustar índice
                "current_cost": float(row[6]) if row[6] is not None else 0.0,  # ✅ Ajustar índice
                "zone": row[7] if len(row) > 7 else "Desconocida"  # ✅ Ajustar índice
            }
            active_calls.append(call)
        
        db.close()
        return active_calls
        
    except Exception as e:
        print(f"Error obteniendo llamadas activas: {str(e)}")
        return {"status": "error", "message": str(e)}
            
@app.get("/api/active-calls-list")
def get_active_calls_list():
    db = SessionLocal()
    try:
        active_calls_rows = db.execute(
            text("SELECT * FROM active_calls ORDER BY start_time DESC")
        ).fetchall()
        
        active_calls = []
        for row in active_calls_rows:
            call = {
                "call_id": row[1],
                "calling_number": row[2],
                "called_number": row[3],
                "start_time": row[4].isoformat() if row[4] else None,
                "current_duration": row[6] if row[6] is not None else 0,
                "current_cost": float(row[7]) if row[7] is not None else 0.0
            }
            active_calls.append(call)
        
        return active_calls
    finally:
        db.close()

# Endpoint WebSocket principal
@app.websocket("/ws")
async def websocket_endpoint(websocket: WebSocket):
    await ws_manager.connect(websocket)
    print(f"Nueva conexión WebSocket establecida. Total conexiones: {len(ws_manager.active_connections)}")
    
    try:
        # Envía todas las llamadas activas al cliente que se acaba de conectar
        db = SessionLocal()
        active_calls_rows = db.execute(
            text("""SELECT call_id, calling_number, called_number, start_time, 
            current_duration, current_cost, connection_id 
            FROM active_calls""")
        ).fetchall()
        db.close()
        
        # Convertir resultados a lista de diccionarios con exactamente los campos esperados
        active_calls = []
        for row in active_calls_rows:
            call = {
                "call_id": row[0],
                "calling_number": row[1],
                "called_number": row[2],
                "start_time": row[3].isoformat() if row[3] else None,
                "current_duration": row[4] if row[4] is not None else 0,
                "current_cost": float(row[5]) if row[5] is not None else 0.0
            }
            active_calls.append(call)
        
        print(f"Enviando {len(active_calls)} llamadas activas iniciales: {active_calls}")
        
        # Asegurarse de que el formato sea exactamente lo que el cliente espera
        await websocket.send_json({
            "type": "update",
            "active_calls": active_calls
        })
        
        # Bucle principal para recibir mensajes del cliente
        while True:
            data = await websocket.receive_text()
            print(f"Mensaje recibido del cliente: {data}")
            
            try:
                message = json.loads(data)
                action = message.get("action")
                
                if action == "get_active_calls":
                    # Actualización manual solicitada por el cliente
                    db = SessionLocal()
                    active_calls_rows = db.execute(
                        text("""SELECT call_id, calling_number, called_number, start_time, 
                        current_duration, current_cost, connection_id 
                        FROM active_calls ORDER BY start_time DESC""")
                    ).fetchall()
                    db.close()
                    
                    # Convertir resultados a lista de diccionarios
                    active_calls = []
                    for row in active_calls_rows:
                        call = {
                            "call_id": row[0],
                            "calling_number": row[1],
                            "called_number": row[2],
                            "start_time": row[3].isoformat() if row[3] else None,
                            "current_duration": row[4] if row[4] is not None else 0,
                            "current_cost": float(row[5]) if row[5] is not None else 0.0,
                            "connection_id": row[6]
                        }
                        active_calls.append(call)
                    
                    await websocket.send_json({
                        "type": "update",
                        "active_calls": active_calls
                    })
                
                elif action == "terminate_call" and "call_id" in message:
                    # Procesar solicitud para terminar una llamada
                    call_id = message["call_id"]
                    print(f"Solicitud para terminar llamada: {call_id}")
                    
                    # Obtener connection_id de la llamada
                    db = SessionLocal()
                    result = db.execute(
                        text(f"SELECT connection_id FROM active_calls WHERE call_id = :call_id"),
                        {"call_id": call_id}
                    ).fetchone()
                    db.close()
                    
                    if result and result[0]:
                        # Implementar la terminación de la llamada
                        await websocket.send_json({
                            "type": "terminate_result",
                            "call_id": call_id,
                            "success": True
                        })
                    else:
                        await websocket.send_json({
                            "type": "terminate_result",
                            "call_id": call_id,
                            "success": False,
                            "error": "Llamada no encontrada"
                        })
            
            except json.JSONDecodeError:
                print("Error al decodificar mensaje JSON")
            except Exception as e:
                print(f"Error procesando mensaje: {str(e)}")
    
    except WebSocketDisconnect:
        ws_manager.disconnect(websocket)
        print(f"Cliente WebSocket desconectado. Conexiones restantes: {len(ws_manager.active_connections)}")
    except Exception as e:
        print(f"Error en WebSocket: {str(e)}")
        ws_manager.disconnect(websocket)    


@app.post("/api/active-calls")
async def report_active_call(call_data: dict):
    print(f"Recibido reporte de llamada activa: {call_data}")
    
    try:
        # Extraer datos
        call_id = call_data.get("call_id")
        
        if not call_id:
            return {"status": "error", "message": "call_id es requerido"}
        
        # ✅ Mapear campos incluyendo direction y zone
        db_call = {
            "call_id": call_id,
            "calling_number": call_data.get("calling_number") or call_data.get("origin"),
            "called_number": call_data.get("called_number") or call_data.get("destination"),
            "direction": call_data.get("direction", "unknown"),  # ✅ Nuevo campo
            "zone": call_data.get("zone", "Desconocida"),        # ✅ Nuevo campo
            "start_time": datetime.fromisoformat(call_data.get("start_time").replace('Z', '+00:00')) 
                if call_data.get("start_time") else datetime.now(),
            "last_updated": datetime.now(),
            "current_duration": call_data.get("current_duration") or call_data.get("duration", 0),
            "current_cost": call_data.get("current_cost") or call_data.get("estimatedCost", 0),
            "connection_id": call_data.get("connection_id") or call_id
        }
        
        # ✅ Logging mejorado con iconos
        direction_icons = {
            "inbound": "📱",
            "outbound": "📞", 
            "internal": "🏢",
            "transit": "🔄"
        }
        icon = direction_icons.get(db_call["direction"], "❓")
        
        print(f"{icon} {db_call['calling_number']} → {db_call['called_number']} "
              f"[{db_call['direction'].upper()}] "
              f"(dur: {db_call['current_duration']}s, costo: ${db_call['current_cost']:.2f})")
        
        # Actualizar en la base de datos
        db = SessionLocal()
        
        try:
            # Verificar si ya existe
            existing = db.execute(
                text("SELECT id FROM active_calls WHERE call_id = :call_id"),
                {"call_id": call_id}
            ).fetchone()
            
            if existing:
                # Actualizar
                set_clause = ", ".join([f"{k} = :{k}" for k in db_call.keys()])
                query = text(f"UPDATE active_calls SET {set_clause} WHERE call_id = :call_id")
                db.execute(query, db_call)
            else:
                # Insertar nuevo
                columns = ", ".join(db_call.keys())
                placeholders = ", ".join([f":{k}" for k in db_call.keys()])
                query = text(f"INSERT INTO active_calls ({columns}) VALUES ({placeholders})")
                db.execute(query, db_call)
            
            db.commit()
            
            # ✅ Consulta actualizada para incluir direction y zone
            db = SessionLocal()
            active_calls_rows = db.execute(
                text("""SELECT id, call_id, calling_number, called_number, direction, 
                        start_time, current_duration, current_cost, zone 
                        FROM active_calls ORDER BY start_time DESC""")
            ).fetchall()
            
            # Convertir a formato que el cliente espera
            active_calls = []
            for row in active_calls_rows:
                # ✅ Mapear todos los campos incluyendo direction
                direction = row[4] if len(row) > 4 and row[4] else "unknown"
                zone = row[8] if len(row) > 8 and row[8] else "Desconocida"
                
                # ✅ Crear display version para el frontend
                direction_displays = {
                    "inbound": "📱 Entrante",
                    "outbound": "📞 Saliente", 
                    "internal": "🏢 Interna",
                    "transit": "🔄 Tránsito"
                }
                direction_display = direction_displays.get(direction, f"❓ {direction}")
                
                call = {
                    "call_id": row[1],
                    "calling_number": row[2],
                    "called_number": row[3],
                    "direction": direction,                    # ✅ Campo original
                    "direction_display": direction_display,   # ✅ Para mostrar en UI
                    "start_time": row[5].isoformat() if row[5] else None,
                    "current_duration": row[6] if row[6] is not None else 0,
                    "current_cost": float(row[7]) if row[7] is not None else 0.0,
                    "zone": zone                              # ✅ Zona
                }
                active_calls.append(call)
            
            # Broadcast con debug
            if ws_manager.active_connections:
                message = {
                    "type": "update",
                    "active_calls": active_calls
                }
                print(f"Enviando broadcast con {len(active_calls)} llamadas activas")
                # ✅ Solo mostrar detalles de la primera llamada para evitar spam
                if active_calls:
                    first_call = active_calls[0]
                    print(f"  Ejemplo: {first_call['calling_number']} → {first_call['called_number']} [{first_call['direction']}]")
                
                await ws_manager.broadcast(message)
                print(f"Broadcast enviado a {len(ws_manager.active_connections)} conexiones")
            
            return {"status": "ok", "active_calls_count": len(active_calls)}
                
        except Exception as e:
            db.rollback()
            print(f"Error en BD: {str(e)}")
            raise
        finally:
            db.close()
            
    except Exception as e:
        print(f"Error general: {str(e)}")
        return {"status": "error", "message": str(e)}
    

@app.delete("/api/active-calls/{call_id}")
async def remove_active_call(call_id: str):
    try:
        db = SessionLocal()
        
        # Buscar primero por call_id
        result = db.execute(
            text("SELECT id FROM active_calls WHERE call_id = :call_id"),
            {"call_id": call_id}
        ).fetchone()
        
        # Si no se encuentra, buscar por connection_id como alternativa
        if not result:
            result = db.execute(
                text("SELECT id FROM active_calls WHERE connection_id = :call_id"),
                {"call_id": call_id}
            ).fetchone()
        
        if result:
            # Eliminar la llamada
            db.execute(
                text("DELETE FROM active_calls WHERE id = :id"),
                {"id": result[0]}
            )
            db.commit()
            print(f"Llamada eliminada: {call_id}")
            
            # Obtener lista actualizada para broadcast
            active_calls_rows = db.execute(
                text("SELECT call_id, calling_number, called_number, start_time, current_duration, current_cost FROM active_calls")
            ).fetchall()
            
            # Broadcast a clientes WebSocket
            if ws_manager.active_connections:
                active_calls = []
                for row in active_calls_rows:
                    call = {
                        "call_id": row[0],
                        "calling_number": row[1],
                        "called_number": row[2],
                        "start_time": row[3].isoformat() if row[3] else None,
                        "current_duration": row[4] if row[4] is not None else 0,
                        "current_cost": float(row[5]) if row[5] is not None else 0.0
                    }
                    active_calls.append(call)
                
                await ws_manager.broadcast({
                    "type": "update",
                    "active_calls": active_calls
                })
            
            return {"status": "ok", "message": f"Llamada {call_id} eliminada correctamente"}
        else:
            print(f"No se encontró la llamada a eliminar: {call_id}")
            return {"status": "not_found", "message": f"Llamada {call_id} no encontrada"}
        
    except Exception as e:
        print(f"Error al eliminar llamada activa: {str(e)}")
        return {"status": "error", "message": str(e)}
        
# Endpoints para estadísticas y monitoreo
@app.get("/api/ws-stats")
async def get_ws_stats():
    return {
        **ws_stats,
        "active_connections": ws_manager.connection_count,
        "active_calls_count": len(active_calls),
        "timestamp": datetime.now().isoformat()
    }

class CDR(Base):
    __tablename__ = "cdr"
    id = Column(Integer, primary_key=True, index=True)
    calling_number = Column(String)
    called_number = Column(String)
    start_time = Column(DateTime)
    end_time = Column(DateTime)
    duration_seconds = Column(Integer)
    duration_billable = Column(Integer)  # Nuevo campo
    cost = Column(Numeric(10,2))
    status = Column(String)
    direction = Column(String)  # Ya existe pero no se está mapeando
    release_cause = Column(Integer)
    connect_time = Column(DateTime)  # Ya existe pero se llama answer_time en Java
    dialing_time = Column(DateTime)  # Nuevo campo
    network_reached_time = Column(DateTime)  # Nuevo campo
    network_alerting_time = Column(DateTime)  # Nuevo campo
    zona_id = Column(Integer)  # Ya existe

class SaldoAnexo(Base):
    __tablename__ = "saldo_anexos"
    id = Column(Integer, primary_key=True, index=True)
    calling_number = Column(String, unique=True)
    saldo = Column(Numeric(10,2))
    fecha_ultima_recarga = Column(DateTime, default=datetime.utcnow)

class Recarga(Base):
    __tablename__ = "recargas"
    id = Column(Integer, primary_key=True, index=True)
    calling_number = Column(String)
    monto = Column(Numeric(10,2))
    fecha = Column(DateTime, default=datetime.utcnow)

class Auditoria(Base):
    __tablename__ = "saldo_auditoria"
    id = Column(Integer, primary_key=True, index=True)
    calling_number = Column(String)
    saldo_anterior = Column(Numeric(10,2))
    saldo_nuevo = Column(Numeric(10,2))
    tipo_accion = Column(String)
    fecha = Column(DateTime, default=datetime.utcnow)

class Anexo(Base):
    __tablename__ = "anexos"
    id = Column(Integer, primary_key=True, index=True)
    numero = Column(String, unique=True, index=True)
    usuario = Column(String)
    area_nivel1 = Column(String)
    area_nivel2 = Column(String, nullable=True)
    area_nivel3 = Column(String, nullable=True)
    pin = Column(String)  # Almacenaremos un hash del PIN
    saldo_actual = Column(Numeric(10, 2), default=0)
    fecha_creacion = Column(DateTime, default=datetime.utcnow)
    activo = Column(Boolean, default=True)

class Configuracion(Base):
    __tablename__ = "configuracion"
    id = Column(Integer, primary_key=True, index=True)
    clave = Column(String, unique=True)
    valor = Column(String)
    descripcion = Column(String, nullable=True)

# Agregar al principio de main.py, junto con las otras definiciones de modelos
class CucmConfig(Base):
    __tablename__ = "cucm_config"
    id = Column(Integer, primary_key=True, index=True)
    server_ip = Column(String, nullable=False)
    server_port = Column(Integer, default=2748)
    username = Column(String, nullable=False)
    password = Column(String, nullable=False)
    app_info = Column(String, default="TarificadorApp")
    reconnect_delay = Column(Integer, default=30)
    check_interval = Column(Integer, default=60)
    enabled = Column(Boolean, default=True)
    last_updated = Column(DateTime, default=datetime.utcnow)
    last_status = Column(String, default="unknown")
    last_status_update = Column(DateTime, nullable=True)

class Usuario(Base):
    __tablename__ = "usuarios"
    id = Column(Integer, primary_key=True, index=True)
    username = Column(String, unique=True, index=True)
    password = Column(String)
    nombre = Column(String, nullable=True)
    apellido = Column(String, nullable=True)
    email = Column(String, nullable=True)
    role = Column(String)
    activo = Column(Boolean, default=True)
    ultimo_login = Column(DateTime, nullable=True)

# Nuevos modelos para gestión de zonas y tarifas
# Adaptación: zonas -> zones
class Zona(Base):
    __tablename__ = "zones"
    id = Column(Integer, primary_key=True, index=True)
    nombre = Column("zone_name", String, unique=True)
    descripcion = Column("description", String)
    prefijos = relationship("Prefijo", back_populates="zona")
    tarifas = relationship("Tarifa", back_populates="zona")

# Adaptación: prefijos -> prefixes
class Prefijo(Base):
    __tablename__ = "prefixes"
    id = Column(Integer, primary_key=True, index=True)
    zona_id = Column("zone_id", Integer, ForeignKey("zones.id"))
    prefijo = Column("prefix", String)
    longitud_minima = Column("prefix_length", Integer)
    zona = relationship("Zona", back_populates="prefijos")
    
    @property
    def longitud_maxima(self):
        """Alias for longitud_minima since prefixes table only has one length column"""
        return self.longitud_minima

# Adaptación: tarifas -> rate_zones
class Tarifa(Base):
    __tablename__ = "rate_zones"
    id = Column(Integer, primary_key=True, index=True)
    zona_id = Column("zone_id", Integer, ForeignKey("zones.id"))
    # Mapeo especial para tarifa
    _rate_per_minute = Column("rate_per_minute", Numeric(10, 5))
    
    fecha_inicio = Column("effective_from", DateTime, default=datetime.utcnow)
    activa = Column("enabled", Boolean, default=True)
    zona = relationship("Zona", back_populates="tarifas")
    
    @property
    def tarifa_segundo(self):
        # Convertir min -> seg
        return self._rate_per_minute / 60 if self._rate_per_minute is not None else 0
        
    @tarifa_segundo.setter
    def tarifa_segundo(self, value):
        self._rate_per_minute = value * 60

# Modelo para compatibilidad con motor de billing Rust
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

Base.metadata.create_all(bind=engine)

# Función para sincronizar prefijos con rate_cards (para el motor Rust)
def sync_rate_cards(db):
    """
    Sincroniza la tabla rate_cards basada en las tablas prefixes, zones y rate_zones.
    Esto permite que el motor Rust (que lee rate_cards) funcione con la configuración del dashboard.
    """
    try:
        # 1. Limpiar rate cards existentes para regenerar
        # Nota: En producción esto podría ser bloqueante. Mejor diff sync.
        # Pero para este fix rápido, truncate es seguro si la carga es baja.
        db.execute(text("TRUNCATE TABLE rate_cards RESTART IDENTITY"))
        
        # 2. Insertar datos combinados
        query = text("""
            INSERT INTO rate_cards (
                destination_prefix, destination_name, rate_per_minute,
                billing_increment, connection_fee, effective_start, priority, created_at
            )
            SELECT 
                p.prefix, 
                z.zone_name, 
                t.rate_per_minute,
                t.billing_increment, 
                0, 
                t.effective_from,
                t.priority,
                CURRENT_TIMESTAMP
            FROM prefixes p
            JOIN zones z ON p.zone_id = z.id
            JOIN rate_zones t ON z.id = t.zone_id
            WHERE t.enabled = TRUE AND p.enabled = TRUE
        """)
        db.execute(query)
        db.commit()
        print("✅ rate_cards sincronizado correctamente para el motor Rust (Apolobilling DB)")
    except Exception as e:
        print(f"❌ Error sincronizando rate_cards: {str(e)}")
        db.rollback()

# Función para inicializar zonas y prefijos
def inicializar_zonas_y_prefijos():
    db = SessionLocal()
    
    # Verificar si ya existen zonas
    try:
        check_query = text("SELECT COUNT(*) FROM zones")
        count = db.execute(check_query).scalar()
        
        if count > 0:
            db.close()
            return
            
        print("La tabla zones está vacía. Inicializando datos por defecto...")
        
        # Crear zonas iniciales (Adaptado para apolobilling)
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
            # zones: zone_name, description, country_id, zone_code, zone_type, region_name, enabled
            insert_zona_query = text("""
                INSERT INTO zones (zone_name, description, country_id, zone_code, zone_type, region_name, enabled, created_at, updated_at) 
                VALUES (:nombre, :descripcion, 4, :code, 'GEOGRAPHIC', 'Default', TRUE, CURRENT_TIMESTAMP, CURRENT_TIMESTAMP)
                RETURNING id
            """)
            
            code = zona_data["nombre"].upper().replace(" ", "-")[:20]
            
            result = db.execute(insert_zona_query, {
                "nombre": zona_data["nombre"],
                "descripcion": zona_data["descripcion"],
                "code": code
            })
            
            zona_id = result.fetchone()[0]
            zonas_ids[zona_data["nombre"]] = zona_id
            
            # Insertar tarifa para la zona en rate_zones
            # rate_per_minute = tarifa * 60
            insert_tarifa_query = text("""
                INSERT INTO rate_zones (
                    zone_id, rate_name, rate_per_minute, rate_per_call, billing_increment, 
                    min_duration, effective_from, currency, priority, enabled, created_at, updated_at
                )
                VALUES (
                    :zona_id, 'Default Rate', :rate_per_minute, 0, 60, 
                    0, CURRENT_TIMESTAMP, 'USD', 1, TRUE, CURRENT_TIMESTAMP, CURRENT_TIMESTAMP
                )
            """)
            
            db.execute(insert_tarifa_query, {
                "zona_id": zona_id,
                "rate_per_minute": zona_data["tarifa"] * 60
            })
        
        # Insertar prefijos
        prefijos = [
            # Local - números fijos 7 dígitos (2-9)XXXXXX
            {"zona_id": zonas_ids["Local"], "prefijo": "2", "longitud_minima": 7},
            {"zona_id": zonas_ids["Local"], "prefijo": "3", "longitud_minima": 7},
            {"zona_id": zonas_ids["Local"], "prefijo": "4", "longitud_minima": 7},
            {"zona_id": zonas_ids["Local"], "prefijo": "5", "longitud_minima": 7},
            {"zona_id": zonas_ids["Local"], "prefijo": "6", "longitud_minima": 7},
            {"zona_id": zonas_ids["Local"], "prefijo": "7", "longitud_minima": 7},
            {"zona_id": zonas_ids["Local"], "prefijo": "8", "longitud_minima": 7},
            {"zona_id": zonas_ids["Local"], "prefijo": "9", "longitud_minima": 7},
            
            # Móvil - 9 dígitos 9XXXXXXXX
            {"zona_id": zonas_ids["Movil"], "prefijo": "9", "longitud_minima": 9},
            
            # LDN - 0[4-8]XXXXXXX
            {"zona_id": zonas_ids["LDN"], "prefijo": "04", "longitud_minima": 9},
            {"zona_id": zonas_ids["LDN"], "prefijo": "05", "longitud_minima": 9},
            {"zona_id": zonas_ids["LDN"], "prefijo": "06", "longitud_minima": 9},
            {"zona_id": zonas_ids["LDN"], "prefijo": "07", "longitud_minima": 9},
            {"zona_id": zonas_ids["LDN"], "prefijo": "08", "longitud_minima": 9},
            
            # LDI - 00[1-9]XXXXXXX.... (10-15 dígitos)
            {"zona_id": zonas_ids["LDI"], "prefijo": "001", "longitud_minima": 10},
            
            # Emergencia 1XX
            {"zona_id": zonas_ids["Emergencia"], "prefijo": "1", "longitud_minima": 3},
            
            # 0800 - 0800XXXXX
            {"zona_id": zonas_ids["0800"], "prefijo": "0800", "longitud_minima": 9},
        ]
        
        for prefijo_data in prefijos:
            insert_prefijo_query = text("""
                INSERT INTO prefixes (zone_id, prefix, prefix_length, operator_name, network_type, enabled, created_at, updated_at)
                VALUES (:zona_id, :prefijo, :longitud_minima, 'Default', 'UNKNOWN', TRUE, CURRENT_TIMESTAMP, CURRENT_TIMESTAMP)
            """)
            
            db.execute(insert_prefijo_query, {
                "zona_id": prefijo_data["zona_id"],
                "prefijo": prefijo_data["prefijo"],
                "longitud_minima": prefijo_data["longitud_minima"]
            })
        
        db.commit()
    except Exception as e:
        print(f"Error inicializando zonas: {e}")
        db.rollback()
    finally:
        db.close()

def determinar_zona_y_tarifa(numero_marcado: str, db):
    """
    Determina la zona del número marcado y obtiene la tarifa correspondiente.
    Usa SQLAlchemy ORM con los modelos Zona, Prefijo y Tarifa.
    """
    # Limpiar el número (quitar caracteres especiales)
    numero_limpio = ''.join(filter(str.isdigit, numero_marcado))
    longitud_numero = len(numero_limpio)
    
    # Buscar prefijos que coincidan con la longitud del número
    prefijos_candidatos = db.query(Prefijo).filter(
        Prefijo.longitud_minima <= longitud_numero,
        Prefijo.longitud_maxima >= longitud_numero
    ).all()
    
    # Encontrar el prefijo más específico (más largo) que coincida
    mejor_prefijo = None
    mejor_longitud = 0
    
    for prefijo_obj in prefijos_candidatos:
        # Verificar si el número comienza con este prefijo
        if numero_limpio.startswith(prefijo_obj.prefijo):
            # Si este prefijo es más específico (más largo), usarlo
            if len(prefijo_obj.prefijo) > mejor_longitud:
                mejor_prefijo = prefijo_obj
                mejor_longitud = len(prefijo_obj.prefijo)
    
    if mejor_prefijo:
        # Obtener la tarifa activa para esta zona usando la relación
        tarifa_activa = db.query(Tarifa).filter(
            Tarifa.zona_id == mejor_prefijo.zona_id,
            Tarifa.activa == True
        ).order_by(Tarifa.fecha_inicio.desc()).first()
        
        if tarifa_activa:
            return {
                'prefijo_id': mejor_prefijo.id,
                'zona_id': mejor_prefijo.zona_id,
                'prefijo': mejor_prefijo.prefijo,
                'zona_nombre': mejor_prefijo.zona.nombre,
                'zona_descripcion': mejor_prefijo.zona.descripcion,
                'tarifa_segundo': float(tarifa_activa.tarifa_segundo),
                'tarifa_id': tarifa_activa.id,
                'numero_valido': True
            }
        else:
            # No hay tarifa activa para esta zona
            return {
                'prefijo_id': mejor_prefijo.id,
                'zona_id': mejor_prefijo.zona_id,
                'prefijo': mejor_prefijo.prefijo,
                'zona_nombre': mejor_prefijo.zona.nombre,
                'zona_descripcion': f"Sin tarifa activa: {mejor_prefijo.zona.descripcion}",
                'tarifa_segundo': 0.0,
                'tarifa_id': None,
                'numero_valido': False
            }
    
    # Si no se encuentra ningún prefijo que coincida
    return {
        'prefijo_id': None,
        'zona_id': None,
        'prefijo': 'UNKNOWN',
        'zona_nombre': 'Desconocida',
        'zona_descripcion': f'Número no reconocido: {numero_marcado} (longitud: {longitud_numero})',
        'tarifa_segundo': 0.0,
        'tarifa_id': None,
        'numero_valido': False
    }

@app.get("/check_balance_for_call/{calling_number}/{called_number}")
def check_balance_for_call(calling_number: str, called_number: str):
    """
    Verifica si hay saldo suficiente para realizar una llamada específica.
    Sistema basado completamente en segundos.
    """
    try:
        # Obtener saldo actual
        with SessionLocal() as db:
            saldo_anexo = db.query(SaldoAnexo).filter(
                SaldoAnexo.calling_number == calling_number
            ).first()
            
            if not saldo_anexo:
                return {
                    "has_balance": False, 
                    "balance": 0.0, 
                    "can_call": False, 
                    "reason": "No account found"
                }
            
            saldo_actual = float(saldo_anexo.saldo)
            
            # Determinar zona y tarifa para el número de destino
            zona_info = determinar_zona_y_tarifa(called_number, db)
            
            if not zona_info['numero_valido']:
                return {
                    "has_balance": saldo_actual > 0,
                    "balance": saldo_actual,
                    "can_call": False,
                    "reason": f"Invalid destination number: {called_number}"
                }
            
            # Todo basado en segundos
            tarifa_segundo = zona_info['tarifa_segundo']
            
            # Verificar si puede realizar al menos 1 segundo de llamada
            can_call = saldo_actual >= tarifa_segundo
            
            # Calcular tiempo disponible en segundos
            tiempo_disponible_segundos = int(saldo_actual / tarifa_segundo) if tarifa_segundo > 0 else 999999
            
            return {
                "has_balance": saldo_actual > 0,
                "balance": saldo_actual,
                "can_call": can_call,
                "zona": zona_info['zona_nombre'],
                "tarifa_segundo": tarifa_segundo,
                "tiempo_disponible_segundos": tiempo_disponible_segundos
            }
        
    except Exception as e:
        print(f"Error verificando saldo para llamada: {e}")
        import traceback
        traceback.print_exc()
        return {
            "has_balance": False, 
            "balance": 0.0, 
            "can_call": False, 
            "reason": str(e)
        }
    
# Llamar a la función de inicialización al arrancar la aplicación
@app.on_event("startup")
def startup_event():
    inicializar_zonas_y_prefijos()
    # Sincronizar tabla para el motor Rust
    db = SessionLocal()
    sync_rate_cards(db)
    db.close()

# Función para determinar la zona de un número
def determinar_zona(numero):
    db = SessionLocal()
    
    # Consultar todos los prefijos ordenados por longitud descendente
    query = text("""
        SELECT id, zone_id, prefix as prefijo, prefix_length as longitud_minima, prefix_length as longitud_maxima 
        FROM prefixes 
        ORDER BY LENGTH(prefix) DESC
    """)
    
    prefijos = db.execute(query).fetchall()
    db.close()
    
    for prefijo in prefijos:
        prefijo_str = prefijo[2]
        longitud_minima = prefijo[3]
        longitud_maxima = prefijo[4]
        
        if (numero.startswith(prefijo_str) and 
            longitud_minima <= len(numero) <= longitud_maxima):
            return prefijo[1]  # Retornar zona_id
    
    return None  # Si no se encuentra una zona

# Función para obtener la tarifa activa de una zona
def obtener_tarifa(zona_id):
    if not zona_id:
        return 0.0005  # Tarifa por defecto si no se encuentra zona
    
    db = SessionLocal()
    
    query = text("""
        SELECT rate_per_minute / 60 as tarifa_segundo 
        FROM rate_zones 
        WHERE zone_id = :zona_id AND enabled = TRUE 
        ORDER BY effective_from DESC 
        LIMIT 1
    """)
    
    result = db.execute(query, {"zona_id": zona_id}).fetchone()
    db.close()
    
    if result:
        return float(result[0])
    
    return 0.0005  # Tarifa por defecto si no hay tarifa activa

@manager.user_loader()
def load_user(username: str):
    try:
        # Usar SQLAlchemy para consultar el usuario en la tabla 'usuarios'
        db = SessionLocal()
        # Usamos text para ejecutar una consulta SQL directa
        query = text("SELECT username, password, role FROM usuarios WHERE username = :username")
        result = db.execute(query, {"username": username}).fetchone()
        
        if result:
            # Devolver un diccionario con los datos del usuario
            user_dict = {
                "username": result[0],
                "password": result[1],
                "role": result[2]
            }
            db.close()
            return user_dict
        else:
            db.close()
            return None
    except Exception as e:
        print(f"Error cargando usuario desde DB: {e}")
        return None
        
async def authenticated_user(request: Request):
    token = request.cookies.get(manager.cookie_name)
    if token is None:
        return RedirectResponse(url="/login")
    try:
        user = await manager.get_current_user(token)
    except Exception:
        return RedirectResponse(url="/login")
    return user

async def admin_only(request: Request):
    user = await authenticated_user(request)
    # Si authenticated_user devuelve un RedirectResponse, simplemente devuélvelo
    if isinstance(user, RedirectResponse):
        return user
    if user["role"] != "admin":
        return RedirectResponse(url="/login")
    return user

@app.post("/auth/login")
def login(request: Request, username: str = Form(...), password: str = Form(...)):
    user = load_user(username)
    if not user or not pwd_context.verify(password, user['password']):
        return templates.TemplateResponse("login.html", {"request": request, "error": "Credenciales inválidas"})

    # Actualizar último login
    try:
        db = SessionLocal()
        update_query = text("UPDATE usuarios SET ultimo_login = CURRENT_TIMESTAMP WHERE username = :username")
        db.execute(update_query, {"username": username})
        db.commit()
        db.close()
    except Exception as e:
        print(f"Error actualizando último login: {e}")

    access_token = manager.create_access_token(data={"sub": username})
    resp = RedirectResponse(url="/dashboard/saldo", status_code=302)
    manager.set_cookie(resp, access_token)
    return resp

@app.get("/login")
def login_page(request: Request):
    return templates.TemplateResponse("login.html", {"request": request})

# Modelos para entrada de datos
from pydantic import BaseModel, field_validator
from fastapi import BackgroundTasks
from fastapi.staticfiles import StaticFiles
from weasyprint import HTML
from jinja2 import Template
from typing import Optional, Any
from datetime import datetime
import json

class CallEvent(BaseModel):
    calling_number: str
    called_number: str
    start_time: Any  # Acepta cualquier tipo, lo convertimos después
    end_time: Any    # Acepta cualquier tipo, lo convertimos después
    duration_seconds: int
    duration_billable: int
    status: str
    direction: Optional[str] = "unknown"
    release_cause: Optional[int] = 0
    answer_time: Optional[Any] = None
    dialing_time: Optional[Any] = None
    network_reached_time: Optional[Any] = None
    network_alerting_time: Optional[Any] = None

    @field_validator('start_time', 'end_time', 'answer_time', 'dialing_time', 'network_reached_time', 'network_alerting_time', mode='before')
    @classmethod
    def convert_datetime_fields(cls, v):
        """Convierte cualquier formato de fecha a datetime"""
        if v is None or v == "null" or v == "":
            return None
        
        if isinstance(v, datetime):
            return v
        
        if isinstance(v, str):
            try:
                if 'T' in v and v.endswith('Z'):
                    return datetime.fromisoformat(v.replace('Z', '+00:00'))
                elif 'T' in v:
                    return datetime.fromisoformat(v)
                else:
                    return datetime.strptime(v, '%Y-%m-%d %H:%M:%S')
            except:
                return datetime.now()  # Fallback a tiempo actual
        
        return datetime.now()  # Fallback para otros tipos

    model_config = {
        "extra": "ignore",  # Ignorar campos adicionales
        "str_strip_whitespace": True  # Limpiar espacios en strings
    }


# Nuevos modelos para zonas y tarifas
class ZonaCreate(BaseModel):
    nombre: str
    descripcion: str

class PrefijoCreate(BaseModel):
    zona_id: int
    prefijo: str
    longitud_minima: int
    longitud_maxima: int

class TarifaCreate(BaseModel):
    zona_id: int
    tarifa_segundo: float


def get_rate_by_zone(db, zona_id: int) -> float:
    """
    Obtiene la tarifa por minuto para una zona específica
    CORREGIDA: Usa tarifa_segundo y la convierte a tarifa por minuto
    
    Args:
        db: Sesión de base de datos
        zona_id: ID de la zona
    
    Returns:
        float: Tarifa por minuto, o tarifa por defecto si no encuentra
    """
    try:
        query = text("""
            SELECT rate_per_minute / 60 as tarifa_segundo 
            FROM rate_zones 
            WHERE zone_id = :zona_id 
              AND enabled = true
            ORDER BY effective_from DESC
            LIMIT 1
        """)
        
        result = db.execute(query, {"zona_id": zona_id}).fetchone()
        
        if result and result[0] is not None:
            # Convertir de tarifa por segundo a tarifa por minuto
            tarifa_por_segundo = float(result[0])
            tarifa_por_minuto = tarifa_por_segundo * 60
            return tarifa_por_minuto
        else:
            print(f"⚠️  No se encontró tarifa activa para zona {zona_id}, usando tarifa por defecto")
            return 3.0  # Tarifa por defecto: 3.0 por minuto (0.05 por segundo * 60)
            
    except Exception as e:
        print(f"Error obteniendo tarifa para zona {zona_id}: {e}")
        return 3.0  # Tarifa por defecto en caso de error


# ===== FUNCIÓN CORREGIDA PARA DETERMINAR ZONA POR PREFIJO =====
def get_zone_by_prefix(db, called_number: str) -> int:
    """
    Determina la zona basándose en el prefijo del número marcado
    CORREGIDA: Mejor manejo de errores y casos edge
    
    Args:
        db: Sesión de base de datos
        called_number: Número de destino marcado
    
    Returns:
        int: ID de la zona correspondiente, o zona por defecto si no encuentra
    """
    if not called_number:
        return 1  # Zona por defecto para números vacíos
    
    # Limpiar el número (quitar espacios, guiones, etc.)
    clean_number = ''.join(filter(str.isdigit, str(called_number)))
    
    if not clean_number:
        return 1  # Zona por defecto si no hay dígitos
    
    try:
        # Obtener todos los prefijos ordenados por longitud descendente
        # Esto prioriza prefijos más específicos (más largos) sobre los generales
        query = text("""
            SELECT zone_id, prefix as prefijo, prefix_length as longitud_minima, prefix_length as longitud_maxima
            FROM prefixes 
            ORDER BY LENGTH(prefix) DESC, prefix
        """)
        
        prefijos = db.execute(query).fetchall()
        
        for prefijo_row in prefijos:
            zona_id, prefijo, long_min, long_max = prefijo_row
            
            # Verificar si el número empieza con este prefijo
            if clean_number.startswith(str(prefijo)):
                # Verificar longitud del número
                if long_min and len(clean_number) < long_min:
                    continue
                if long_max and len(clean_number) > long_max:
                    continue
                
                # ¡Encontramos la zona!
                return zona_id
        
        # Si no se encuentra ningún prefijo, usar zona por defecto
        print(f"⚠️  No se encontró zona para el número: {called_number} (limpio: {clean_number})")
        return 1  # Zona por defecto (puedes cambiar este valor)
        
    except Exception as e:
        print(f"Error determinando zona para {called_number}: {e}")
        return 1  # Zona por defecto en caso de error


# API Principal - Modificada para usar zonas y tarifas
@app.post("/cdr")
def create_cdr(event: CallEvent):
    db = SessionLocal()
    
    try:
        # 1. Determinar la zona basándose en el prefijo
        zona_id = get_zone_by_prefix(db, event.called_number)
        
        # 2. Obtener la tarifa específica para esa zona
        rate_per_minute = get_rate_by_zone(db, zona_id)
        
        # 3. Calcular el costo basado en duration_billable y tarifa de la zona
        cost = (event.duration_billable / 60) * rate_per_minute
        
        # 4. Crear el CDR con la zona determinada automáticamente
        cdr_data = {
            "calling_number": event.calling_number,
            "called_number": event.called_number,
            "start_time": event.start_time,
            "end_time": event.end_time,
            "duration_seconds": event.duration_seconds,
            "duration_billable": event.duration_billable,
            "cost": cost,
            "status": event.status,
            "direction": event.direction,
            "release_cause": event.release_cause,
            "connect_time": event.answer_time,  # Mapear answer_time a connect_time
            "dialing_time": event.dialing_time,
            "network_reached_time": event.network_reached_time,
            "network_alerting_time": event.network_alerting_time,
            "zona_id": zona_id  # ¡Ahora se calcula automáticamente!
        }
        
        # 5. Guardar el CDR
        cdr = CDR(**cdr_data)
        db.add(cdr)
        
        # 6. Actualizar el saldo del anexo
        update_result = db.execute(
            text("UPDATE saldo_anexos SET saldo = saldo - :cost WHERE calling_number = :calling_number"),
            {"cost": cost, "calling_number": event.calling_number}
        )
        
        # 7. Verificar si se actualizó el saldo (anexo existe)
        if update_result.rowcount == 0:
            print(f"⚠️  Anexo {event.calling_number} no encontrado en saldo_anexos")
            # Opcional: crear registro de saldo para el anexo
            # db.execute(
            #     text("INSERT INTO saldo_anexos (calling_number, saldo) VALUES (:calling_number, :saldo)"),
            #     {"calling_number": event.calling_number, "saldo": -cost}
            # )
        
        # 8. Verificar saldo bajo y generar alerta si es necesario
        nuevo_saldo_result = db.execute(
            text("SELECT saldo FROM saldo_anexos WHERE calling_number = :calling_number"),
            {"calling_number": event.calling_number}
        ).fetchone()
        
        if nuevo_saldo_result:
            nuevo_saldo = nuevo_saldo_result[0]
            if nuevo_saldo < 1.0:
                # TODO: Enviar alerta de saldo bajo
                print(f"🚨 ALERTA: Anexo {event.calling_number} con saldo bajo: ${nuevo_saldo:.2f}")
                # Aquí puedes programar enviar correo o alerta
        
        # 9. Confirmar transacción
        db.commit()
        
        # 10. Obtener información de la zona para logging/debugging
        zona_info = db.execute(
            text("SELECT zone_name as nombre FROM zones WHERE id = :zona_id"),
            {"zona_id": zona_id}
        ).fetchone()
        
        zona_nombre = zona_info[0] if zona_info else "Desconocida"
        
        return {
            "message": "CDR saved successfully",
            "cdr_id": cdr.id if hasattr(cdr, 'id') else None,
            "cost": round(cost, 4),
            "zona_id": zona_id,
            "zona_nombre": zona_nombre,
            "rate_per_minute": rate_per_minute,
            "duration_billed_minutes": round(event.duration_billable / 60, 4)
        }
        
    except Exception as e:
        db.rollback()
        print(f"Error creando CDR: {e}")
        raise HTTPException(status_code=500, detail=f"Error guardando CDR: {str(e)}")
    finally:
        db.close()


# Nuevo endpoint para verificar saldo con destino
@app.get("/check_balance/{calling_number}/{called_number}")
def check_balance_with_destination(calling_number: str, called_number: str):
    db = SessionLocal()
    
    # Verificar saldo
    query = text("SELECT saldo FROM saldo_anexos WHERE calling_number = :calling_number")
    saldo = db.execute(query, {"calling_number": calling_number}).fetchone()
    
    if not saldo or saldo[0] <= 0:
        db.close()
        return {"has_balance": False, "reason": "insufficient_balance"}
    
    # Determinar la zona y tarifa del destino
    zona_id = determinar_zona(called_number)
    if not zona_id:
        db.close()
        return {"has_balance": False, "reason": "invalid_destination"}
    
    # Verificar si el saldo es suficiente para al menos 1 minuto de llamada
    tarifa_segundo = obtener_tarifa(zona_id)
    costo_minuto = tarifa_segundo * 60
    
    if saldo[0] < costo_minuto:
        db.close()
        return {"has_balance": False, "reason": "low_balance_for_call"}
    
    db.close()
    return {"has_balance": True, "zona_id": zona_id, "tarifa_segundo": tarifa_segundo}

# Mantener compatibilidad con la versión anterior
@app.get("/check_balance/{calling_number}")
def check_balance(calling_number: str):
    with SessionLocal() as db:
        query = text("SELECT saldo FROM saldo_anexos WHERE calling_number = :calling_number")
        saldo = db.execute(query, {"calling_number": calling_number}).fetchone()
        return {"has_balance": True}
        #return {"has_balance": saldo and saldo[0] > 0}

@app.post("/recargar/{calling_number}/{amount}")
async def recargar_saldo(calling_number: str, amount: float, user=Depends(admin_only)):
    db = SessionLocal()
    
    # Check if the record exists
    query = text("SELECT saldo FROM saldo_anexos WHERE calling_number = :calling_number")
    saldo_actual = db.execute(query, {"calling_number": calling_number}).fetchone()

    if saldo_actual:
        update_query = text(
            "UPDATE saldo_anexos SET saldo = saldo + :amount, fecha_ultima_recarga = CURRENT_TIMESTAMP WHERE calling_number = :calling_number"
        )
        db.execute(update_query, {"amount": amount, "calling_number": calling_number})
    else:
        insert_query = text(
            "INSERT INTO saldo_anexos (calling_number, saldo, fecha_ultima_recarga) VALUES (:calling_number, :amount, CURRENT_TIMESTAMP)"
        )
        db.execute(insert_query, {"calling_number": calling_number, "amount": amount})

    # Logging the audit record  
    audit_query = text(
        "INSERT INTO saldo_auditoria (calling_number, saldo_anterior, saldo_nuevo, tipo_accion) "
        "VALUES (:calling_number, :saldo_anterior, :saldo_nuevo, 'recarga')"
    )
    db.execute(audit_query, {
        "calling_number": calling_number, 
        "saldo_anterior": saldo_actual[0] if saldo_actual else 0,
        "saldo_nuevo": saldo_actual[0] + Decimal(str(amount)) if saldo_actual else Decimal(str(amount))
        #"saldo_nuevo": saldo_actual[0] + amount if saldo_actual else amount
    })

    # Record the recharge
    recharge_query = text("INSERT INTO recargas (calling_number, monto) VALUES (:calling_number, :amount)")
    db.execute(recharge_query, {"calling_number": calling_number, "amount": amount})

    db.commit()
    db.close()
    return {"message": f"Recargado {amount} al número {calling_number}"}

# DASHBOARD: SALDO
@app.get("/dashboard/saldo")
async def dashboard_saldo(request: Request, user=Depends(authenticated_user)):
    db = SessionLocal()
    query = text("SELECT calling_number, saldo FROM saldo_anexos ORDER BY calling_number ASC")
    rows = db.execute(query).fetchall()
    saldos_bajos = [row for row in rows if float(row[1]) < 1.0]
    labels = [row[0] for row in rows]
    data = [float(row[1]) for row in rows]
    
    # Pre-calculate the background colors based on saldo values
    background_colors = ['rgba(220, 53, 69, 0.8)' if float(row[1]) < 1.0 else 'rgba(13, 110, 253, 0.8)' for row in rows]
    
    db.close()
    return templates.TemplateResponse("dashboard_saldo.html", {
        "request": request, 
        "rows": rows, 
        "labels": labels, 
        "data": data, 
        "saldos_bajos": saldos_bajos, 
        "user": user, 
        "background_colors": background_colors
    })

def process_cdr_status(status, duration_seconds, duration_billable):
    """
    Procesa el estado de la llamada basándose en el campo status de la BD
    PRIORIDAD: Campo 'status' > Duración > Otros factores
    """
    
    # ✅ PRIORIDAD 1: Usar el campo 'status' directamente
    if status == 'no_answer':
        return "📵 No contestada"
    elif status == 'failed':
        return "❌ Fallida"
    elif status == 'answered' or status == 'completed':
        return "✅ Completada"
    elif status == 'ringing' or status == 'alerting':
        return "🔔 Timbrando"
    elif status == 'dialing':
        return "📞 Marcando"
    elif status == 'in_progress':
        return "🔄 En progreso"
    
    # ✅ PRIORIDAD 2: Si status es 'disconnected', usar duración
    elif status == 'disconnected':
        if duration_billable and duration_billable > 0:
            return "✅ Completada"
        else:
            return "📵 No contestada"
    
    # ✅ PRIORIDAD 3: Fallback para estados desconocidos
    else:
        if duration_billable and duration_billable > 0:
            return f"✅ Completada ({status})"
        else:
            return f"❓ {status.title() if status else 'Desconocido'}"



# DASHBOARD: CDR - SINTAXIS SQLALCHEMY CORRECTA
def get_filtered_cdr_data(request: Request):
    """
    Función centralizada para obtener datos filtrados
    Compatible con SQLAlchemy
    """
    phone_number = request.query_params.get('phone_number', '').strip()
    start_date = request.query_params.get('start_date', '').strip()
    end_date = request.query_params.get('end_date', '').strip()
    status_filter = request.query_params.get('status', '').strip()
    direction_filter = request.query_params.get('direction', '').strip()
    
    db = SessionLocal()
    # Construir query
    params = []
    query = """
        SELECT calling_number, called_number, call_date, call_time, 
               duration, cost, hangup_cause, status, direction, cdr_id
        FROM call_logs 
        WHERE 1=1
    """
    
    if phone_number:
        query += " AND (calling_number LIKE ? OR called_number LIKE ?)"
        search_pattern = f"%{phone_number}%"
        params.extend([search_pattern, search_pattern])
    
    if start_date:
        query += " AND DATE(call_date) >= ?"
        params.append(start_date)
    
    if end_date:
        query += " AND DATE(call_date) <= ?"
        params.append(end_date)
    
    if status_filter:
        query += " AND status = ?"
        params.append(status_filter)
    
    if direction_filter:
        query += " AND direction = ?"
        params.append(direction_filter)
    
    query += " ORDER BY call_date DESC, call_time DESC"
    
    # ✅ USAR text() para SQLAlchemy
    result = db.execute(text(query), params)
    return result.fetchall()

@app.get("/dashboard/cdr")
def dashboard_cdr(request: Request, 
                  user=Depends(authenticated_user),
                  page: int = Query(1, ge=1),
                  per_page: int = Query(10, ge=1, le=100),
                  phone_number: str = Query(None),
                  start_date: str = Query(None),
                  end_date: str = Query(None),
                  min_duration: int = Query(0),
                  status: str = Query(None),
                  direction: str = Query(None)):

    db = SessionLocal()
    offset = (page - 1) * per_page
    
    # ✅ CONSULTA CORREGIDA con sintaxis SQLAlchemy :parameter
    query = """
        SELECT 
            calling_number, 
            called_number, 
            start_time, 
            end_time, 
            COALESCE(duration_seconds, 0) as duration_seconds,
            COALESCE(cost, 0) as cost, 
            COALESCE(duration_billable, 0) as duration_billable,
            COALESCE(status, 'unknown') as status,
            COALESCE(direction, 'unknown') as direction,
            COALESCE(release_cause, 0) as release_cause
        FROM cdr 
        WHERE 1=1
    """
    
    # Parámetros para la consulta parametrizada
    params = {}
    
    # ✅ Aplicar filtros con sintaxis :parameter
    #if phone_number:
    #    query += " AND calling_number = :calling_number"
    #    params['calling_number'] = calling_number
    if phone_number:
        query += " AND (calling_number LIKE :phone_pattern OR called_number LIKE :phone_pattern)"
        params['phone_pattern'] = f"%{phone_number}%"

    if start_date:
        query += " AND start_time >= :start_date"
        params['start_date'] = f"{start_date} 00:00:00"
    
    if end_date:
        query += " AND start_time <= :end_date"
        params['end_date'] = f"{end_date} 23:59:59"
    
    # ✅ IMPORTANTE: Solo filtrar por duración si se especifica explícitamente > 0
    if min_duration and min_duration > 0:
        query += " AND COALESCE(duration_seconds, 0) >= :min_duration"
        params['min_duration'] = min_duration
    
    # ✅ NUEVOS FILTROS
    if status and status != 'all':
        query += " AND COALESCE(status, 'unknown') = :status"
        params['status'] = status
        
    if direction and direction != 'all':
        query += " AND COALESCE(direction, 'unknown') = :direction"
        params['direction'] = direction
    
    # Ordenar y paginar - ✅ SINTAXIS CORREGIDA
    query += " ORDER BY start_time DESC LIMIT :limit OFFSET :offset"
    params['limit'] = per_page
    params['offset'] = offset
    
    try:
        # Ejecutar consulta
        rows = db.execute(text(query), params).fetchall()
        
        # ✅ Procesar filas con información adicional
        processed_rows = []
        for row in rows:
            # Determinar tipo de llamada para mostrar
            duration_seconds = row[4]
            duration_billable = row[9] if len(row) > 9 else 0
            call_type = process_cdr_status(status, duration_seconds, duration_billable)
            status = row[7]
            if status == 'no_answer':
                call_type = "📵 No contestada"
            elif status == 'disconnected':
                call_type = "✅ Contestada"
            else:
                call_type = "🔄 En progreso"
                
            """
            call_type = "📞 Completada"
            status_value = row[7] if len(row) > 7 else 'unknown'
            duration_value = row[4] if len(row) > 4 else 0
            
            if status_value == 'failed':
                call_type = "❌ Fallida"
            elif status_value == 'disconnected' and duration_value == 0:
                call_type = "📵 No contestada"
            elif status_value == 'disconnected' and duration_value > 0:
                call_type = "✅ Completada"
            elif status_value == 'initiated':
                call_type = "🔄 En progreso"
            """

            # Formatear dirección
            direction_value = row[8] if len(row) > 8 else 'unknown'
            direction_display = {
                'inbound': '📱 Entrante',
                'outbound': '📞 Saliente', 
                'internal': '🏢 Interna',
                'transit': '🔄 Tránsito',
                'unknown': '❓ Desconocida'
            }.get(direction_value, '❓ Desconocida')
            
            processed_row = list(row) + [call_type, direction_display]
            processed_rows.append(tuple(processed_row))
        
        # ✅ Estadísticas para gráficos
        if rows:
            # Gráfico por horas 
            horas = [row[2].hour for row in rows if row[2]]
            contador_horas = Counter(horas)
            labels_horas = sorted(contador_horas.keys())
            data_horas = [contador_horas[h] for h in labels_horas]
            
            # Gráfico por status
            status_counts = Counter([row[7] for row in rows])
            status_labels = list(status_counts.keys())
            status_data = list(status_counts.values())
            
            # Gráfico por dirección
            direction_counts = Counter([row[8] for row in rows])
            direction_labels = list(direction_counts.keys())
            direction_data = list(direction_counts.values())
        else:
            labels_horas = data_horas = []
            status_labels = status_data = []
            direction_labels = direction_data = []
        
        # ✅ Consulta de conteo con sintaxis corregida
        count_query = "SELECT COUNT(*) FROM cdr WHERE 1=1"
        count_params = {}

        if phone_number:
            count_query += " AND (calling_number LIKE :phone_pattern OR called_number LIKE :phone_pattern)"
            count_params['phone_pattern'] = f"%{phone_number}%"
        #if calling_number:
        #    count_query += " AND calling_number = :calling_number"
        #    count_params['calling_number'] = calling_number
        
        if start_date:
            count_query += " AND start_time >= :start_date"
            count_params['start_date'] = f"{start_date} 00:00:00"
        
        if end_date:
            count_query += " AND start_time <= :end_date"
            count_params['end_date'] = f"{end_date} 23:59:59"
        
        if min_duration and min_duration > 0:
            count_query += " AND COALESCE(duration_seconds, 0) >= :min_duration"
            count_params['min_duration'] = min_duration
            
        if status and status != 'all':
            count_query += " AND COALESCE(status, 'unknown') = :status"
            count_params['status'] = status
            
        if direction and direction != 'all':
            count_query += " AND COALESCE(direction, 'unknown') = :direction"
            count_params['direction'] = direction
        
        total_records = db.execute(text(count_query), count_params).fetchone()[0]
        total_pages = -(-total_records // per_page)
        
        # ✅ Estadísticas generales del día
        stats_query = """
            SELECT 
                COUNT(*) as total_calls,
                COUNT(CASE WHEN COALESCE(status, 'unknown') = 'disconnected' AND COALESCE(duration_seconds, 0) > 0 THEN 1 END) as completed_calls,
                COUNT(CASE WHEN COALESCE(status, 'unknown') = 'failed' THEN 1 END) as failed_calls,
                COUNT(CASE WHEN COALESCE(status, 'unknown') = 'disconnected' AND COALESCE(duration_seconds, 0) = 0 THEN 1 END) as unanswered_calls,
                COALESCE(SUM(cost), 0) as total_cost,
                COALESCE(SUM(duration_seconds), 0) as total_duration
            FROM cdr 
            WHERE start_time >= CURRENT_DATE
        """
        
        try:
            stats = db.execute(text(stats_query)).fetchone()
        except Exception as stats_error:
            print(f"Error en estadísticas: {stats_error}")
            stats = (0, 0, 0, 0, 0, 0)
        
    except Exception as e:
        print(f"Error en consulta principal: {e}")
        processed_rows = []
        labels_horas = data_horas = []
        status_labels = status_data = []
        direction_labels = direction_data = []
        total_records = total_pages = 0
        stats = (0, 0, 0, 0, 0, 0)
    
    finally:
        db.close()
    
    return templates.TemplateResponse("dashboard_cdr.html", {
        "request": request, 
        "rows": processed_rows,
        "labels": labels_horas, 
        "data": data_horas,
        "status_labels": status_labels,
        "status_data": status_data,
        "direction_labels": direction_labels,
        "direction_data": direction_data,
        "page": page, 
        "per_page": per_page, 
        "total_pages": total_pages,
        "total_records": total_records,
        "stats": {
            "total_calls": stats[0] if stats else 0,
            "completed_calls": stats[1] if stats else 0,
            "failed_calls": stats[2] if stats else 0,
            "unanswered_calls": stats[3] if stats else 0,
            "total_cost": float(stats[4]) if stats else 0.0,
            "total_duration": stats[5] if stats else 0
        },
        "user": user,
        "current_filters": {
            "phone_number": phone_number,
            "start_date": start_date,
            "end_date": end_date,
            "min_duration": min_duration,
            "status": status,
            "direction": direction
        }
    })

@app.get("/dashboard/finanzas")
async def dashboard_finanzas(request: Request, user=Depends(authenticated_user)):
    db = SessionLocal()
    query = text("""
        SELECT DATE(start_time) as fecha, SUM(cost) as total_cost
        FROM cdr
        GROUP BY DATE(start_time)
        ORDER BY fecha DESC
        LIMIT 30
    """)
    rows = db.execute(query).fetchall()
    db.close()

    labels = [str(row[0]) for row in rows]
    data = [float(row[1]) for row in rows]

    return templates.TemplateResponse("dashboard_finanzas.html", {
        "request": request, "labels": labels, "data": data, "user": user
    })


@app.get("/dashboard/recargas")
async def dashboard_recargas(request: Request, user=Depends(authenticated_user)):
    db = SessionLocal()
    query = text("SELECT calling_number, monto, fecha FROM recargas ORDER BY fecha DESC LIMIT 100")
    rows = db.execute(query).fetchall()
    db.close()

    return templates.TemplateResponse("dashboard_recargas.html", {
        "request": request, "rows": rows, "user": user
    })

@app.get("/dashboard/auditoria")
async def dashboard_auditoria(request: Request, user=Depends(admin_only)):
    db = SessionLocal()
    query = text("SELECT calling_number, saldo_anterior, saldo_nuevo, tipo_accion, fecha FROM saldo_auditoria ORDER BY fecha DESC LIMIT 100")
    rows = db.execute(query).fetchall()
    db.close()

    return templates.TemplateResponse("dashboard_auditoria.html", {
        "request": request, "rows": rows, "user": user
    })

@app.get("/dashboard/ranking_consumo")
async def dashboard_ranking_consumo(request: Request, user=Depends(authenticated_user)):
    db = SessionLocal()
    query = text("""
        SELECT calling_number, SUM(cost) as consumo_total
        FROM cdr
        GROUP BY calling_number
        ORDER BY consumo_total DESC
        LIMIT 10
    """)
    rows = db.execute(query).fetchall()
    db.close()

    labels = [row[0] for row in rows]
    data = [float(row[1]) for row in rows]
    
    # Calcular la suma total para evitar división por cero en la plantilla
    total_consumo = sum(data) if data else 0
    
    # Calcular porcentajes de forma segura
    porcentajes = []
    if total_consumo > 0 and rows:
        porcentajes = [(float(row[1]) / total_consumo * 100) for row in rows]
    
    return templates.TemplateResponse("dashboard_ranking_consumo.html", {
        "request": request, 
        "labels": labels, 
        "data": data, 
        "rows": rows, 
        "user": user,
        "total_consumo": total_consumo,
        "porcentajes": porcentajes
    })

@app.get("/export/saldo/pdf")
async def export_saldo_pdf(user=Depends(admin_only)):
    db = SessionLocal()
    query = text("SELECT calling_number, saldo FROM saldo_anexos ORDER BY calling_number ASC")
    rows = db.execute(query).fetchall()
    db.close()

    html_template = """
    <html>
    <body>
    <h1>Reporte de Saldos</h1>
    <table border="1">
        <thead>
            <tr><th>Anexo</th><th>Saldo</th></tr>
        </thead>
        <tbody>
        {% for row in rows %}
            <tr><td>{{ row[0] }}</td><td>${{ "%.2f"|format(row[1]) }}</td></tr>
        {% endfor %}
        </tbody>
    </table>
    </body>
    </html>
    """
    template = Template(html_template)
    html_content = template.render(rows=rows)

    pdf = HTML(string=html_content).write_pdf()

    return StreamingResponse(
        io.BytesIO(pdf),
        media_type="application/pdf",
        headers={"Content-Disposition": "attachment; filename=saldo_report.pdf"}
    )


@app.get("/dashboard/recarga_masiva")
async def form_recarga_masiva(request: Request, user=Depends(admin_only)):
    if isinstance(user, RedirectResponse):
        return user
    return templates.TemplateResponse("recarga_masiva.html", {"request": request, "user": user})

@app.post("/dashboard/recarga_masiva")
async def recarga_masiva(request: Request, file: UploadFile = File(...), user=Depends(admin_only)):
    db = SessionLocal()
    
    try:
        # Leer el contenido del archivo
        content = await file.read()
        
        # Determinar tipo de archivo por extensión
        filename = file.filename.lower()
        
        # Lista para almacenar las filas a procesar
        rows = []
        
        # Procesar según el tipo de archivo
        if filename.endswith('.xlsx') or filename.endswith('.xls'):
            # Archivo Excel
            try:
                import io
                import openpyxl
                
                # Cargar el archivo Excel
                wb = openpyxl.load_workbook(io.BytesIO(content))
                ws = wb.active
                
                # Leer filas (omitir la primera fila de encabezados)
                first_row = True
                for row in ws.rows:
                    if first_row:
                        first_row = False
                        continue
                    
                    if len(row) >= 2 and row[0].value and row[1].value:
                        rows.append([str(row[0].value), str(row[1].value)])
            except Exception as e:
                return templates.TemplateResponse("recarga_masiva.html", {
                    "request": request, 
                    "user": user,
                    "error": f"Error procesando archivo Excel: {str(e)}"
                })
        else:
            # Archivo CSV - intentar con diferentes codificaciones
            encodings = ['utf-8', 'latin-1', 'windows-1252', 'iso-8859-1']
            decoded = False
            
            for encoding in encodings:
                try:
                    # Intentar decodificar con esta codificación
                    text_content = content.decode(encoding).splitlines()
                    reader = csv.reader(text_content)
                    
                    # Omitir encabezados
                    next(reader, None)
                    
                    # Leer filas
                    for row in reader:
                        if len(row) >= 2:
                            rows.append(row)
                    
                    decoded = True
                    break  # Si llegamos aquí, la decodificación fue exitosa
                except UnicodeDecodeError:
                    continue  # Probar con la siguiente codificación
            
            if not decoded:
                return templates.TemplateResponse("recarga_masiva.html", {
                    "request": request, 
                    "user": user,
                    "error": "No se pudo decodificar el archivo. Asegúrese de que sea un CSV válido."
                })
        
        # Procesar las filas y actualizar la base de datos
        procesados = 0
        errores = []
        
        for row in rows:
            try:
                if len(row) >= 2:
                    # Extraer anexo y monto
                    calling_number = str(row[0]).strip()
                    
                    # Convertir monto a float, manejando posibles formatos
                    try:
                        amount = float(str(row[1]).strip().replace(',', '.'))
                    except ValueError:
                        errores.append(f"Error en anexo {calling_number}: '{row[1]}' no es un monto válido.")
                        continue
                    
                    # Buscar saldo actual
                    saldo_query = text("SELECT saldo FROM saldo_anexos WHERE calling_number = :calling_number")
                    saldo_actual = db.execute(saldo_query, {"calling_number": calling_number}).fetchone()

                    # Actualizar o insertar saldo
                    if saldo_actual:
                        update_query = text(
                            "UPDATE saldo_anexos SET saldo = saldo + :amount, fecha_ultima_recarga = CURRENT_TIMESTAMP WHERE calling_number = :calling_number"
                        )
                        db.execute(update_query, {"amount": amount, "calling_number": calling_number})
                    else:
                        insert_query = text(
                            "INSERT INTO saldo_anexos (calling_number, saldo, fecha_ultima_recarga) VALUES (:calling_number, :amount, CURRENT_TIMESTAMP)"
                        )
                        db.execute(insert_query, {"calling_number": calling_number, "amount": amount})

                    # Registrar en auditoría
                    audit_query = text(
                        "INSERT INTO saldo_auditoria (calling_number, saldo_anterior, saldo_nuevo, tipo_accion) "
                        "VALUES (:calling_number, :saldo_anterior, :saldo_nuevo, 'recarga_masiva')"
                    )
                    db.execute(audit_query, {
                        "calling_number": calling_number,
                        "saldo_anterior": saldo_actual[0] if saldo_actual else 0,
                        "saldo_nuevo": saldo_actual[0] + Decimal(str(amount)) if saldo_actual else Decimal(str(amount))
                    })
                    
                    procesados += 1
            except Exception as e:
                errores.append(f"Error procesando anexo {calling_number}: {str(e)}")
        
        # Confirmar transacción
        db.commit()
        
        # Generar mensaje de éxito
        success_message = f"Se procesaron {procesados} recargas exitosamente."
        
        # Devolver respuesta
        return templates.TemplateResponse("recarga_masiva.html", {
            "request": request, 
            "user": user,
            "success": success_message,
            "errores": errores if errores else None
        })
            
    except Exception as e:
        # Revertir cambios en caso de error
        db.rollback()
        
        return templates.TemplateResponse("recarga_masiva.html", {
            "request": request, 
            "user": user,
            "error": f"Error procesando el archivo: {str(e)}"
        })
    finally:
        db.close()

@app.get("/")
def root():
    return RedirectResponse(url="/dashboard/saldo")

@app.get("/logout")
def logout():
    response = RedirectResponse(url="/login")
    response.delete_cookie(manager.cookie_name)
    return response

# DASHBOARD: ANEXOS
@app.get("/dashboard/anexos")
async def dashboard_anexos(request: Request, 
                           user=Depends(admin_only),
                           page: int = Query(1, ge=1),
                           per_page: int = Query(20, ge=1, le=100),
                           buscar: str = Query(None)):
    if isinstance(user, RedirectResponse):
        return user
        
    db = SessionLocal()
    
    # Construir la consulta base para contar y para obtener registros
    base_query_str = """
        SELECT a.id, a.numero, a.usuario, a.area_nivel1, a.area_nivel2, a.area_nivel3, a.saldo_actual, a.pin, a.activo, s.saldo
        FROM anexos a
        LEFT JOIN saldo_anexos s ON a.numero = s.calling_number
        WHERE 1=1
    """
    count_query_str = "SELECT COUNT(*) FROM anexos WHERE 1=1"
    
    # Agregar condiciones de filtrado
    params = {}
    if buscar:
        base_query_str += """ 
            AND (a.numero LIKE :buscar OR a.usuario LIKE :buscar OR 
                 a.area_nivel1 LIKE :buscar OR a.area_nivel2 LIKE :buscar OR 
                 a.area_nivel3 LIKE :buscar)
        """
        count_query_str += """ 
            AND (numero LIKE :buscar OR usuario LIKE :buscar OR 
                 area_nivel1 LIKE :buscar OR area_nivel2 LIKE :buscar OR 
                 area_nivel3 LIKE :buscar)
        """
        params["buscar"] = f"%{buscar}%"

    # Calcular el total de registros para la paginación
    count_query = text(count_query_str)
    total_records = db.execute(count_query, params).scalar()
    total_pages = (total_records + per_page - 1) // per_page  # Ceiling division
    
    # Agregar límite y offset para la paginación
    offset = (page - 1) * per_page
    base_query_str += f" ORDER BY a.numero ASC LIMIT {per_page} OFFSET {offset}"
    
    # Ejecutar la consulta principal
    query = text(base_query_str)
    rows = db.execute(query, params).fetchall()
    
    # Consultar el valor de longitud de PIN actual
    pin_length_query = text("SELECT valor FROM configuracion WHERE clave = 'pin_length'")
    pin_length_row = db.execute(pin_length_query).fetchone()
    pin_length = int(pin_length_row[0]) if pin_length_row else 6  # Valor por defecto: 6
    
    # Consultar áreas para los dropdowns
    areas_nivel1_query = text("SELECT DISTINCT area_nivel1 FROM anexos WHERE area_nivel1 IS NOT NULL ORDER BY area_nivel1")
    areas_nivel1 = [row[0] for row in db.execute(areas_nivel1_query).fetchall()]
    
    areas_nivel2_query = text("SELECT DISTINCT area_nivel2 FROM anexos WHERE area_nivel2 IS NOT NULL ORDER BY area_nivel2")
    areas_nivel2 = [row[0] for row in db.execute(areas_nivel2_query).fetchall()]
    
    areas_nivel3_query = text("SELECT DISTINCT area_nivel3 FROM anexos WHERE area_nivel3 IS NOT NULL ORDER BY area_nivel3")
    areas_nivel3 = [row[0] for row in db.execute(areas_nivel3_query).fetchall()]
    
    db.close()

    return templates.TemplateResponse("dashboard_anexos.html", {
        "request": request,
        "rows": rows,
        "page": page,
        "per_page": per_page,
        "total_pages": total_pages,
        "total_records": total_records,
        "buscar": buscar or "",
        "user": user,
        "pin_length": pin_length,
        "areas_nivel1": areas_nivel1,
        "areas_nivel2": areas_nivel2,
        "areas_nivel3": areas_nivel3
    })

@app.get("/anexo/{anexo_id}")
async def get_anexo(anexo_id: int, user=Depends(admin_only)):
    if isinstance(user, RedirectResponse):
        return user
        
    db = SessionLocal()
    query = text("SELECT id, numero, usuario, area_nivel1, area_nivel2, area_nivel3, saldo_actual, activo FROM anexos WHERE id = :anexo_id")
    anexo = db.execute(query, {"anexo_id": anexo_id}).fetchone()
    db.close()
    
    if not anexo:
        raise HTTPException(status_code=404, detail="Anexo no encontrado")
        
    return {
        "id": anexo[0],
        "numero": anexo[1],
        "usuario": anexo[2],
        "area_nivel1": anexo[3],
        "area_nivel2": anexo[4],
        "area_nivel3": anexo[5],
        "saldo_actual": float(anexo[6]),
        "activo": anexo[7]
    }

# Modelo para crear/actualizar anexo
class AnexoCreate(BaseModel):
    numero: str
    usuario: str
    area_nivel1: str
    area_nivel2: Optional[str] = None
    area_nivel3: Optional[str] = None
    pin: Optional[str] = None
    saldo_actual: Optional[float] = 0
    activo: Optional[bool] = True
    
    # Validadores (opcional)
    @validator('numero')
    def numero_valid(cls, v):
        if not v or not v.strip():
            raise ValueError('El número no puede estar vacío')
        return v
    
    @validator('usuario')
    def usuario_valid(cls, v):
        if not v or not v.strip():
            raise ValueError('El usuario no puede estar vacío')
        return v
    
    #@validator('saldo_actual')
    #def saldo_valid(cls, v):
    #    if v < 0:
    #        raise ValueError('El saldo inicial no puede ser negativo')
    #    return v


@app.post("/anexo")
async def crear_anexo(anexo: AnexoCreate, user=Depends(admin_only)):
    if isinstance(user, RedirectResponse):
        return user
    
    # Imprimir toda la información para depuración
    print(f"Datos recibidos del formulario:")
    print(f"  Número: '{anexo.numero}'")
    print(f"  Usuario: '{anexo.usuario}'")
    print(f"  Área Nivel 1: '{anexo.area_nivel1}'")
    print(f"  Área Nivel 2: '{getattr(anexo, 'area_nivel2', '')}'")
    print(f"  Área Nivel 3: '{getattr(anexo, 'area_nivel3', '')}'")
    print(f"  PIN: '{'*****' if anexo.pin else 'No proporcionado'}'")
    #print(f"  Saldo Actual: {anexo.saldo_actual}")
    print(f"  Activo: {getattr(anexo, 'activo', True)}")
            
    try:    
        db = SessionLocal()
        
        # Verificar si el número de anexo ya existe
        check_query = text("SELECT id FROM anexos WHERE numero = :numero")
        existing = db.execute(check_query, {"numero": anexo.numero}).fetchone()
        
        if existing:
            print(f"Error: El anexo {anexo.numero} ya existe")
            db.close()
            raise HTTPException(status_code=422, detail=f"El número de anexo {anexo.numero} ya existe")
        
        # Generar PIN automático si no se proporciona
        pin = anexo.pin
        if not pin:
            # Obtener longitud configurada del PIN
            try:
                pin_length_query = text("SELECT valor FROM configuracion WHERE clave = 'pin_length'")
                pin_length_row = db.execute(pin_length_query).fetchone()
                pin_length = int(pin_length_row[0]) if pin_length_row else 6
            except Exception as e:
                print(f"Error al obtener longitud de PIN: {e}")
                pin_length = 6  # Valor por defecto si hay error
            
            # Generar PIN aleatorio
            import random
            pin = ''.join(random.choices('0123456789', k=pin_length))
            print(f"PIN generado automáticamente: {pin}")
        
        # Hashear el PIN para almacenarlo de forma segura
        #try:
        #    hashed_pin = pwd_context.hash(pin)
        #except Exception as e:
        #    print(f"Error al hashear PIN: {e}")
        #    raise HTTPException(status_code=500, detail="Error al procesar el PIN")
        
        # Preparar los parámetros para la inserción, garantizando valores por defecto para campos opcionales
        params = {
            "numero": anexo.numero,
            "usuario": anexo.usuario,
            "area_nivel1": anexo.area_nivel1,
            "area_nivel2": anexo.area_nivel2 if hasattr(anexo, 'area_nivel2') and anexo.area_nivel2 else "",
            "area_nivel3": anexo.area_nivel3 if hasattr(anexo, 'area_nivel3') and anexo.area_nivel3 else "",
            "pin": pin,
            "activo": anexo.activo if hasattr(anexo, 'activo') else True
        }
        
        # Insertar el nuevo anexo
        try:
            insert_query = text("""
                INSERT INTO anexos (numero, usuario, area_nivel1, area_nivel2, area_nivel3, pin, activo)
                VALUES (:numero, :usuario, :area_nivel1, :area_nivel2, :area_nivel3, :pin, :activo)
                RETURNING id
            """)
            
            result = db.execute(insert_query, params)
            anexo_id = result.fetchone()[0]
            print(f"Anexo creado con ID: {anexo_id}")
            
        except Exception as e:
            print(f"Error al insertar anexo: {e}")
            db.rollback()
            db.close()
            raise HTTPException(status_code=500, detail=f"Error al crear anexo: {str(e)}")
        
        # Inicializar saldo en la tabla saldo_anexos si tiene saldo inicial
        try:
            if anexo.saldo_actual > 0:
                saldo_query = text("""
                    INSERT INTO saldo_anexos (calling_number, saldo, fecha_ultima_recarga)
                    VALUES (:numero, :saldo, CURRENT_TIMESTAMP)
                """)
                db.execute(saldo_query, {"numero": anexo.numero, "saldo": anexo.saldo_actual})
                
                # Registrar en auditoría
                audit_query = text("""
                    INSERT INTO saldo_auditoria (calling_number, saldo_anterior, saldo_nuevo, tipo_accion)
                    VALUES (:numero, 0, :saldo, 'creacion_anexo')
                """)
                db.execute(audit_query, {"numero": anexo.numero, "saldo": anexo.saldo_actual})
                
                print(f"Saldo inicial registrado: {anexo.saldo_actual}")
        except Exception as e:
            print(f"Error al registrar saldo: {e}")
            # No fallamos la operación completa si solo falla la parte del saldo
            # Pero registramos el error para investigación
        
        db.commit()
        db.close()
        
        return {"id": anexo_id, "mensaje": "Anexo creado exitosamente", "pin": pin}
    
    except HTTPException:
        # Re-lanzar excepciones HTTP tal cual
        raise
    
    except Exception as e:
        print(f"Error inesperado creando anexo: {e}")
        # Si llegamos aquí es un error inesperado
        raise HTTPException(status_code=500, detail=f"Error interno del servidor: {str(e)}")

    
@app.put("/anexo/{anexo_id}")
async def actualizar_anexo(anexo_id: int, anexo: AnexoCreate, user=Depends(admin_only)):
    if isinstance(user, RedirectResponse):
        return user
        
    db = SessionLocal()
    
    # Verificar si el anexo existe
    check_query = text("SELECT numero, saldo_actual FROM anexos WHERE id = :anexo_id")
    existing = db.execute(check_query, {"anexo_id": anexo_id}).fetchone()
    
    if not existing:
        db.close()
        raise HTTPException(status_code=404, detail="Anexo no encontrado")
    
    old_numero = existing[0]
    old_saldo = float(existing[1])
    
    # Verificar si el nuevo número ya existe (y no es el mismo anexo)
    if anexo.numero != old_numero:
        check_duplicate_query = text("SELECT id FROM anexos WHERE numero = :numero")
        duplicate = db.execute(check_duplicate_query, {"numero": anexo.numero}).fetchone()
        
        if duplicate:
            db.close()
            raise HTTPException(status_code=400, detail="El número de anexo ya existe")
    
    # Preparar actualización de PIN si se proporciona
    params = {
        "anexo_id": anexo_id,
        "numero": anexo.numero,
        "usuario": anexo.usuario,
        "area_nivel1": anexo.area_nivel1,
        "area_nivel2": anexo.area_nivel2,
        "area_nivel3": anexo.area_nivel3,
        "activo": anexo.activo
    }
    
    update_query_str = """
        UPDATE anexos SET 
        numero = :numero,
        usuario = :usuario,
        area_nivel1 = :area_nivel1,
        area_nivel2 = :area_nivel2,
        area_nivel3 = :area_nivel3,
        activo = :activo
    """
    
    # Si se proporciona PIN, actualizarlo
    if anexo.pin:
        #hashed_pin = pwd_context.hash(anexo.pin)
        update_query_str += ", pin = :pin"
        params["pin"] = anexo.pin
    
    update_query_str += " WHERE id = :anexo_id"
    update_query = text(update_query_str)
    
    db.execute(update_query, params)
    
    db.commit()
    db.close()
    
    return {"mensaje": "Anexo actualizado exitosamente"}

@app.delete("/anexo/{anexo_id}")
async def eliminar_anexo(anexo_id: int, user=Depends(admin_only)):
    if isinstance(user, RedirectResponse):
        return user
        
    db = SessionLocal()
    
    try:
        # Verificar si el anexo existe
        check_query = text("SELECT numero FROM anexos WHERE id = :anexo_id")
        existing = db.execute(check_query, {"anexo_id": anexo_id}).fetchone()
        
        if not existing:
            raise HTTPException(status_code=404, detail="Anexo no encontrado")
        
        numero_anexo = existing[0]
        
        # Eliminación física (delete del registro)
        delete_query = text("DELETE FROM anexos WHERE id = :anexo_id")
        result = db.execute(delete_query, {"anexo_id": anexo_id})
        
        # Verificar que se eliminó efectivamente
        if result.rowcount == 0:
            raise HTTPException(status_code=500, detail="Error al eliminar el anexo")
        
        db.commit()
        
        return {"mensaje": f"Anexo {numero_anexo} eliminado exitosamente"}
        
    except HTTPException:
        db.rollback()
        raise
    except Exception as e:
        db.rollback()
        raise HTTPException(status_code=500, detail=f"Error interno del servidor: {str(e)}")
    finally:
        db.close()

# Configuración de longitud de PIN
@app.put("/configuracion/pin_length")
async def actualizar_longitud_pin(pin_length: int = Form(...), user=Depends(admin_only)):
    if isinstance(user, RedirectResponse):
        return user
        
    if pin_length < 4 or pin_length > 10:
        raise HTTPException(status_code=400, detail="La longitud del PIN debe estar entre 4 y 10 dígitos")
        
    db = SessionLocal()
    
    # Verificar si ya existe la configuración
    check_query = text("SELECT id FROM configuracion WHERE clave = 'pin_length'")
    existing = db.execute(check_query).fetchone()
    
    if existing:
        # Actualizar configuración existente
        update_query = text("UPDATE configuracion SET valor = :valor WHERE clave = 'pin_length'")
        db.execute(update_query, {"valor": str(pin_length)})
    else:
        # Crear nueva configuración
        insert_query = text("""
            INSERT INTO configuracion (clave, valor, descripcion)
            VALUES ('pin_length', :valor, 'Longitud de PIN para anexos')
        """)
        db.execute(insert_query, {"valor": str(pin_length)})
    
    db.commit()
    db.close()
    
    return {"mensaje": f"Longitud de PIN actualizada a {pin_length} dígitos"}

# Generación masiva de PINs
@app.post("/anexos/generar_pines")
async def generar_pines_masivos(request: Request, user=Depends(admin_only)):
    if isinstance(user, RedirectResponse):
        return user
        
    db = SessionLocal()
    
    # Obtener longitud configurada del PIN
    pin_length_query = text("SELECT valor FROM configuracion WHERE clave = 'pin_length'")
    pin_length_row = db.execute(pin_length_query).fetchone()
    pin_length = int(pin_length_row[0]) if pin_length_row else 6
    
    # Obtener todos los anexos activos
    anexos_query = text("SELECT id, numero FROM anexos WHERE activo = TRUE")
    anexos = db.execute(anexos_query).fetchall()
    
    import random
    resultados = []
    
    for anexo in anexos:
        anexo_id, numero = anexo
        
        # Generar nuevo PIN
        pin = ''.join(random.choices('0123456789', k=pin_length))
        hashed_pin = pwd_context.hash(pin)
        
        # Actualizar PIN
        update_query = text("UPDATE anexos SET pin = :pin WHERE id = :anexo_id")
        db.execute(update_query, {"pin": hashed_pin, "anexo_id": anexo_id})
        
        resultados.append({"numero": numero, "pin": pin})
    
    db.commit()
    db.close()
    
    # Generar CSV para descarga
    csv_content = io.StringIO()
    writer = csv.writer(csv_content)
    writer.writerow(["Número de Anexo", "PIN"])
    
    for resultado in resultados:
        writer.writerow([resultado["numero"], resultado["pin"]])
    
    csv_content.seek(0)
    
    return StreamingResponse(
        io.BytesIO(csv_content.getvalue().encode()),
        media_type="text/csv",
        headers={"Content-Disposition": f"attachment; filename=pines_anexos_{datetime.now().strftime('%Y%m%d%H%M%S')}.csv"}
    )

# Conexion CUCM
@app.get("/dashboard/cucm")
async def dashboard_cucm(request: Request, user=Depends(admin_only)):
    if isinstance(user, RedirectResponse):
        return user
        
    return templates.TemplateResponse("dashboard_cucm.html", {
        "request": request,
        "user": user
    })

# Carga masiva de anexos
@app.get("/dashboard/anexos/carga_masiva")
async def form_carga_masiva_anexos(request: Request, user=Depends(admin_only)):
    if isinstance(user, RedirectResponse):
        return user
        
    return templates.TemplateResponse("carga_masiva_anexos.html", {"request": request, "user": user})

@app.post("/dashboard/anexos/carga_masiva")
async def carga_masiva_anexos(
    request: Request, 
    file: UploadFile = File(...), 
    generar_pin: bool = Form(True),
    continuar_errores: bool = Form(False),
    user=Depends(admin_only)
):
    """
    Procesa un archivo CSV o Excel para cargar anexos masivamente.
    
    Params:
    - file: Archivo CSV o Excel con los datos de anexos
    - generar_pin: Si se debe generar PIN automáticamente cuando no se proporciona
    - continuar_errores: Si se debe continuar procesando a pesar de errores
    """
    if isinstance(user, RedirectResponse):
        return user
    
    db = SessionLocal()
    
    # Variables para el seguimiento
    procesados = 0
    exitosos = 0
    errores = []
    
    try:
        # Leer el contenido del archivo
        content = await file.read()
        
        # Determinar tipo de archivo por extensión
        filename = file.filename.lower()
        
        # Lista para almacenar las filas a procesar
        rows = []
        headers = []
        
        # Procesar según el tipo de archivo
        if filename.endswith('.xlsx') or filename.endswith('.xls'):
            # Procesar archivo Excel
            try:
                # Intentar usar la biblioteca openpyxl si está disponible
                try:
                    import io
                    import openpyxl
                    
                    # Cargar el archivo Excel
                    wb = openpyxl.load_workbook(io.BytesIO(content))
                    ws = wb.active
                    
                    # Leer encabezados (primera fila)
                    headers = [str(cell.value).strip() if cell.value else "" for cell in next(ws.rows)]
                    
                    # Verificar encabezados mínimos requeridos
                    if not headers or 'numero' not in headers or 'usuario' not in headers or 'area_nivel1' not in headers:
                        return templates.TemplateResponse("carga_masiva_anexos.html", {
                            "request": request, 
                            "user": user,
                            "error": "El archivo no tiene los encabezados requeridos: numero, usuario, area_nivel1"
                        })
                    
                    # Leer filas (omitir la primera fila de encabezados)
                    for row in list(ws.rows)[1:]:
                        # Convertir celdas a valores de texto
                        row_values = [str(cell.value).strip() if cell.value is not None else "" for cell in row]
                        if any(row_values):  # Omitir filas vacías
                            rows.append(row_values)
                    
                except ImportError:
                    # Si openpyxl no está disponible, intentar usar xlrd (para archivos .xls)
                    try:
                        import io
                        import xlrd
                        
                        # Abrir el libro Excel
                        workbook = xlrd.open_workbook(file_contents=content)
                        sheet = workbook.sheet_by_index(0)
                        
                        # Leer encabezados (primera fila)
                        headers = [str(sheet.cell_value(0, col)).strip() for col in range(sheet.ncols)]
                        
                        # Verificar encabezados mínimos requeridos
                        if not headers or 'numero' not in headers or 'usuario' not in headers or 'area_nivel1' not in headers:
                            return templates.TemplateResponse("carga_masiva_anexos.html", {
                                "request": request, 
                                "user": user,
                                "error": "El archivo no tiene los encabezados requeridos: numero, usuario, area_nivel1"
                            })
                        
                        # Leer filas (omitir la primera fila de encabezados)
                        for row_idx in range(1, sheet.nrows):
                            row_values = [str(sheet.cell_value(row_idx, col)).strip() for col in range(sheet.ncols)]
                            if any(row_values):  # Omitir filas vacías
                                rows.append(row_values)
                    
                    except ImportError:
                        # Si ninguna biblioteca Excel está disponible, intentar con pandas
                        try:
                            import io
                            import pandas as pd
                            
                            # Leer el archivo Excel con pandas
                            df = pd.read_excel(io.BytesIO(content))
                            
                            # Verificar encabezados mínimos requeridos
                            df_columns = df.columns.tolist()
                            headers = [str(col).strip() for col in df_columns]
                            
                            if not headers or 'numero' not in headers or 'usuario' not in headers or 'area_nivel1' not in headers:
                                return templates.TemplateResponse("carga_masiva_anexos.html", {
                                    "request": request, 
                                    "user": user,
                                    "error": "El archivo no tiene los encabezados requeridos: numero, usuario, area_nivel1"
                                })
                            
                            # Convertir DataFrame a lista de filas
                            rows = df.values.tolist()
                            
                        except ImportError:
                            # Si ninguna de las bibliotecas está disponible
                            return templates.TemplateResponse("carga_masiva_anexos.html", {
                                "request": request, 
                                "user": user,
                                "error": "No se pueden procesar archivos Excel en este servidor. Por favor, exporte a CSV e intente de nuevo."
                            })
            
            except Exception as e:
                print(f"Error procesando archivo Excel: {str(e)}")
                return templates.TemplateResponse("carga_masiva_anexos.html", {
                    "request": request, 
                    "user": user,
                    "error": f"Error procesando archivo Excel: {str(e)}"
                })
        
        else:
            # Archivo CSV - intentar con diferentes codificaciones
            encodings = ['utf-8', 'latin-1', 'windows-1252', 'iso-8859-1']
            decoded = False
            
            for encoding in encodings:
                try:
                    # Intentar decodificar con esta codificación
                    text_content = content.decode(encoding).splitlines()
                    reader = csv.reader(text_content)
                    
                    # Leer encabezados
                    headers = next(reader, None)
                    
                    # Verificar encabezados mínimos requeridos
                    if not headers or 'numero' not in headers or 'usuario' not in headers or 'area_nivel1' not in headers:
                        return templates.TemplateResponse("carga_masiva_anexos.html", {
                            "request": request, 
                            "user": user,
                            "error": "El archivo no tiene los encabezados requeridos: numero, usuario, area_nivel1"
                        })
                    
                    # Leer filas
                    rows = list(reader)
                    decoded = True
                    print(f"✅ Archivo decodificado correctamente con codificación: {encoding}")
                    break  # Si llegamos aquí, la decodificación fue exitosa
                except UnicodeDecodeError:
                    print(f"❌ Fallo al decodificar con {encoding}")
                    continue  # Probar con la siguiente codificación
            
            if not decoded:
                return templates.TemplateResponse("carga_masiva_anexos.html", {
                    "request": request, 
                    "user": user,
                    "error": "No se pudo decodificar el archivo. Asegúrese de que sea un CSV válido."
                })
        
        # Si no hay filas para procesar
        if not rows:
            return templates.TemplateResponse("carga_masiva_anexos.html", {
                "request": request, 
                "user": user,
                "error": "El archivo no contiene datos para procesar."
            })
        
        # Mapear índices de columnas
        column_indices = {}
        for col in ['numero', 'usuario', 'area_nivel1', 'area_nivel2', 'area_nivel3', 'pin', 'saldo_actual']:
            column_indices[col] = headers.index(col) if col in headers else -1
        
        # Obtener longitud configurada del PIN
        pin_length_query = text("SELECT valor FROM configuracion WHERE clave = 'pin_length'")
        pin_length_row = db.execute(pin_length_query).fetchone()
        pin_length = int(pin_length_row[0]) if pin_length_row else 6  # Valor por defecto: 6
        
        # Procesar filas
        for i, row in enumerate(rows, start=2):  # start=2 para considerar la fila 1 como encabezados
            try:
                # Asegurarse de que la fila tiene suficientes columnas
                if len(row) <= max(idx for idx in column_indices.values() if idx >= 0):
                    # Extender la fila con valores vacíos si es necesario
                    row.extend([''] * (max(idx for idx in column_indices.values() if idx >= 0) - len(row) + 1))
                
                # Validar que tenga los campos requeridos
                if column_indices['numero'] < 0 or column_indices['usuario'] < 0 or column_indices['area_nivel1'] < 0:
                    raise ValueError("No se encontraron las columnas requeridas: numero, usuario, area_nivel1")
                
                # Verificar que los campos requeridos no estén vacíos
                if not row[column_indices['numero']] or not row[column_indices['usuario']] or not row[column_indices['area_nivel1']]:
                    raise ValueError("Faltan valores en campos requeridos: numero, usuario, area_nivel1")
                
                # Extraer datos básicos
                numero = str(row[column_indices['numero']]).strip()
                usuario = str(row[column_indices['usuario']]).strip()
                area_nivel1 = str(row[column_indices['area_nivel1']]).strip()
                
                # Extraer datos opcionales
                area_nivel2 = str(row[column_indices['area_nivel2']]).strip() if column_indices['area_nivel2'] >= 0 and len(row) > column_indices['area_nivel2'] else ''
                area_nivel3 = str(row[column_indices['area_nivel3']]).strip() if column_indices['area_nivel3'] >= 0 and len(row) > column_indices['area_nivel3'] else ''
                
                # Extraer PIN si existe
                pin = None
                if column_indices['pin'] >= 0 and len(row) > column_indices['pin'] and row[column_indices['pin']]:
                    pin = str(row[column_indices['pin']]).strip()
                
                # Extraer saldo inicial si existe
                saldo_actual = 0.0
                if column_indices['saldo_actual'] >= 0 and len(row) > column_indices['saldo_actual'] and row[column_indices['saldo_actual']]:
                    try:
                        saldo_value = str(row[column_indices['saldo_actual']]).strip().replace(',', '.')
                        # Manejar casos donde el valor puede ser un float en formato de texto o tener caracteres no numéricos
                        saldo_actual = float(''.join(c for c in saldo_value if c.isdigit() or c == '.'))
                    except ValueError:
                        print(f"⚠️ Error convirtiendo saldo: {row[column_indices['saldo_actual']]}")
                        saldo_actual = 0.0
                
                # Verificar si el número ya existe
                check_query = text("SELECT id FROM anexos WHERE numero = :numero")
                existing = db.execute(check_query, {"numero": numero}).fetchone()
                
                if existing:
                    raise ValueError(f"El anexo {numero} ya existe")
                
                # Generar PIN si no se proporcionó y está activada la opción
                if not pin and generar_pin:
                    import random
                    pin = ''.join(random.choices('0123456789', k=pin_length))
                    print(f"PIN generado para {numero}: {pin}")
                
                # Hashear el PIN
                from passlib.context import CryptContext
                pwd_context = CryptContext(schemes=["bcrypt"], deprecated="auto")
                hashed_pin = pwd_context.hash(pin) if pin else None
                
                # Insertar el nuevo anexo
                insert_query = text("""
                    INSERT INTO anexos (numero, usuario, area_nivel1, area_nivel2, area_nivel3, pin, saldo_actual, activo)
                    VALUES (:numero, :usuario, :area_nivel1, :area_nivel2, :area_nivel3, :pin, :saldo_actual, TRUE)
                    RETURNING id
                """)
                
                result = db.execute(insert_query, {
                    "numero": numero,
                    "usuario": usuario,
                    "area_nivel1": area_nivel1,
                    "area_nivel2": area_nivel2,
                    "area_nivel3": area_nivel3,
                    "pin": hashed_pin,
                    "saldo_actual": saldo_actual,
                    "activo": True
                })
                
                anexo_id = result.fetchone()[0]
                
                # Inicializar saldo en la tabla saldo_anexos si tiene saldo inicial
                if saldo_actual > 0:
                    saldo_query = text("""
                        INSERT INTO saldo_anexos (calling_number, saldo, fecha_ultima_recarga)
                        VALUES (:numero, :saldo, CURRENT_TIMESTAMP)
                    """)
                    db.execute(saldo_query, {"numero": numero, "saldo": saldo_actual})
                    
                    # Registrar en auditoría
                    audit_query = text("""
                        INSERT INTO saldo_auditoria (calling_number, saldo_anterior, saldo_nuevo, tipo_accion)
                        VALUES (:numero, 0, :saldo, 'creacion_anexo')
                    """)
                    db.execute(audit_query, {"numero": numero, "saldo": saldo_actual})
                
                procesados += 1
                exitosos += 1
                
            except Exception as e:
                # Registrar error
                error_msg = f"Fila {i}: {str(e)}"
                errores.append(error_msg)
                
                # Si no debemos continuar ante errores, terminar
                if not continuar_errores:
                    db.rollback()
                    return templates.TemplateResponse("carga_masiva_anexos.html", {
                        "request": request,
                        "user": user,
                        "error": f"Error en la fila {i}: {str(e)}. Proceso abortado.",
                        "errores": errores
                    })
        
        # Confirmar cambios en la base de datos
        db.commit()
        
        # Mensaje de éxito
        success_message = f"Se procesaron {procesados} registros. {exitosos} anexos creados con éxito."
        if errores:
            success_message += f" Se encontraron {len(errores)} errores."
        
        return templates.TemplateResponse("carga_masiva_anexos.html", {
            "request": request,
            "user": user,
            "success": success_message,
            "errores": errores if errores else None
        })
        
    except Exception as e:
        # En caso de error general, hacer rollback
        db.rollback()
        return templates.TemplateResponse("carga_masiva_anexos.html", {
            "request": request,
            "user": user,
            "error": f"Error general: {str(e)}"
        })
    finally:
        db.close()
        
# Modelo Pydantic para la configuración de CUCM
class CucmConfigModel(BaseModel):
    server_ip: str
    server_port: int = 2748
    username: str
    password: str
    app_info: str = "TarificadorApp"
    reconnect_delay: int = 30
    check_interval: int = 60
    enabled: bool = True

# Ruta para obtener la configuración actual
@app.get("/api/cucm/config")
async def get_cucm_config(user=Depends(admin_only)):
    if isinstance(user, RedirectResponse):
        return user
        
    db = SessionLocal()
    query = text("SELECT server_ip, server_port, username, password, app_info, reconnect_delay, check_interval, enabled, last_status, last_status_update FROM cucm_config ORDER BY id DESC LIMIT 1")
    result = db.execute(query).fetchone()
    
    if not result:
        # Si no hay configuración, devolver valores por defecto
        db.close()
        return {
            "server_ip": "190.105.250.127",
            "server_port": 2748,
            "username": "jtapiuser",
            "password": "********",  # Ocultamos la contraseña real
            "app_info": "TarificadorApp",
            "reconnect_delay": 30,
            "check_interval": 60,
            "enabled": True,
            "last_status": "unknown",
            "last_status_update": None
        }
    
    config = {
        "server_ip": result[0],
        "server_port": result[1],
        "username": result[2],
        "password": "********",  # Ocultamos la contraseña real
        "app_info": result[4],
        "reconnect_delay": result[5],
        "check_interval": result[6],
        "enabled": result[7],
        "last_status": result[8],
        "last_status_update": result[9]
    }
    
    db.close()
    return config

# Ruta para actualizar la configuración de CUCM
@app.post("/api/cucm/config")
async def update_cucm_config(config: CucmConfigModel, user=Depends(admin_only)):
    if isinstance(user, RedirectResponse):
        return user
        
    db = SessionLocal()
    
    # Verificar si ya existe una configuración
    check_query = text("SELECT id FROM cucm_config LIMIT 1")
    existing = db.execute(check_query).fetchone()
    
    if existing:
        # Actualizar configuración existente
        update_query = text("""
            UPDATE cucm_config SET 
            server_ip = :server_ip,
            server_port = :server_port,
            username = :username,
            password = :password,
            app_info = :app_info,
            reconnect_delay = :reconnect_delay,
            check_interval = :check_interval,
            enabled = :enabled,
            last_updated = CURRENT_TIMESTAMP
            WHERE id = :id
        """)
        
        db.execute(update_query, {
            "id": existing[0],
            "server_ip": config.server_ip,
            "server_port": config.server_port,
            "username": config.username,
            "password": config.password,
            "app_info": config.app_info,
            "reconnect_delay": config.reconnect_delay,
            "check_interval": config.check_interval,
            "enabled": config.enabled
        })
    else:
        # Crear nueva configuración
        insert_query = text("""
            INSERT INTO cucm_config (server_ip, server_port, username, password, app_info, reconnect_delay, check_interval, enabled)
            VALUES (:server_ip, :server_port, :username, :password, :app_info, :reconnect_delay, :check_interval, :enabled)
        """)
        
        db.execute(insert_query, {
            "server_ip": config.server_ip,
            "server_port": config.server_port,
            "username": config.username,
            "password": config.password,
            "app_info": config.app_info,
            "reconnect_delay": config.reconnect_delay,
            "check_interval": config.check_interval,
            "enabled": config.enabled
        })
    
    # Generar nuevo archivo de configuración para el servicio Java
    generate_java_config(config, db)
    
    # Señalizar al servicio que debe recargar la configuración
    notify_java_service(config)
    
    db.commit()
    db.close()
    
    return {"message": "Configuración actualizada correctamente"}

# Ruta para probar la conexión
@app.post("/api/cucm/test_connection")
async def test_cucm_connection(config: CucmConfigModel, user=Depends(admin_only)):
    if isinstance(user, RedirectResponse):
        return user
    
    # Aquí implementamos la lógica para probar la conexión
    # Esta función debería llamar al servicio Java para probar la conexión
    # y devolver el resultado

    try:
        # Simulamos un retraso de respuesta del servicio
        await asyncio.sleep(2)
        
        # En un entorno real, esto llamaría a un endpoint en el servicio Java
        # Por ahora, simplemente simularemos una respuesta exitosa
        success = True
        message = "Conexión exitosa con el servidor CUCM"
        
        # Si la conexión es exitosa, actualizamos el estado en la base de datos
        if success:
            db = SessionLocal()
            update_query = text("""
                UPDATE cucm_config SET 
                last_status = :status,
                last_status_update = CURRENT_TIMESTAMP
                WHERE id = (SELECT id FROM cucm_config ORDER BY id DESC LIMIT 1)
            """)
            
            db.execute(update_query, {"status": "connected"})
            db.commit()
            db.close()
        
        return {"success": success, "message": message}
    except Exception as e:
        return {"success": False, "message": f"Error al probar la conexión: {str(e)}"}

# Función para generar archivo de configuración Java
def generate_java_config(config, db):
    try:
        # Ruta del archivo de configuración
        config_path = "/opt/tarificador/java-service/config/application.properties"
        
        # Crear contenido del archivo
        content = f"""# Configuración generada automáticamente por el dashboard
# Última actualización: {datetime.now().strftime('%Y-%m-%d %H:%M:%S')}

# Configuración de conexión CUCM
cucm.server.ip={config.server_ip}
cucm.server.port={config.server_port}
cucm.server.username={config.username}
cucm.server.password={config.password}
cucm.server.appinfo={config.app_info}
cucm.enabled={str(config.enabled).lower()}

# Configuración del servicio web
webservice.base.url=http://localhost:8000
webservice.endpoint.cdr=/cdr
webservice.endpoint.check_balance=/check_balance/
webservice.endpoint.call_start=/call_start

# Configuración de reconexión
reconnect.initial.delay={config.reconnect_delay}
reconnect.check.interval={config.check_interval}

# Configuración de logging
logging.level=INFO
logging.file.path=/var/log/tarificador
logging.file.name=cucm-service.log
"""
        
        # Escribir archivo
        with open(config_path, "w") as f:
            f.write(content)
            
        # Actualizar estado en la base de datos
        update_query = text("""
            UPDATE cucm_config SET 
            last_status = :status,
            last_status_update = CURRENT_TIMESTAMP
            WHERE id = (SELECT id FROM cucm_config ORDER BY id DESC LIMIT 1)
        """)
        
        db.execute(update_query, {"status": "config_updated"})
        
        return True
    except Exception as e:
        print(f"Error al generar archivo de configuración: {str(e)}")
        return False

# Función para notificar al servicio Java
def notify_java_service(config):
    try:
        # En un entorno real, esto podría:
        # 1. Enviar una señal al proceso Java (por ejemplo, usando SIGHUP)
        # 2. Usar un endpoint REST en el servicio Java para indicarle que recargue la configuración
        # 3. Usar un archivo temporal o un socket para comunicarse con el servicio
        
        # Por simplicidad, aquí usaremos systemctl para reiniciar el servicio
        if os.path.exists("/bin/systemctl"):
            if config.enabled:
                os.system("systemctl restart tarificador-cucm.service")
            else:
                os.system("systemctl stop tarificador-cucm.service")
        
        return True
    except Exception as e:
        print(f"Error al notificar al servicio Java: {str(e)}")
        return False


# Ruta para obtener el estado del servicio CUCM
@app.get("/api/cucm/status")
async def get_cucm_status(user=Depends(authenticated_user)):
    if isinstance(user, RedirectResponse):
        return user
        
    try:
        # Verificar el estado del servicio systemd
        is_active = False
        status_message = "Desconocido"
        
        if os.path.exists("/bin/systemctl"):
            # Verificar si el servicio está activo
            result = os.popen("systemctl is-active tarificador-cucm.service").read().strip()
            is_active = (result == "active")
            
            # Obtener estado más detallado
            status_output = os.popen("systemctl status tarificador-cucm.service --no-pager").read()
            
            # Extraer línea de estado
            if "Active:" in status_output:
                status_line = [line for line in status_output.split('\n') if "Active:" in line][0]
                status_message = status_line.strip()
        
        # Obtener el último estado registrado en la base de datos
        db = SessionLocal()
        query = text("SELECT last_status, last_status_update FROM cucm_config ORDER BY id DESC LIMIT 1")
        result = db.execute(query).fetchone()
        db.close()
        
        db_status = "unknown"
        db_status_time = None
        
        if result:
            db_status = result[0]
            db_status_time = result[1]
        
        return {
            "service_active": is_active,
            "service_status": status_message,
            "last_known_status": db_status,
            "last_status_update": db_status_time
        }
    except Exception as e:
        return {"error": f"Error al obtener estado: {str(e)}"}
    

# Endpoint para obtener logs del servicio
@app.get("/api/cucm/logs")
async def get_cucm_logs(lines: int = 100, user=Depends(authenticated_user)):
    if isinstance(user, RedirectResponse):
        return user
        
    try:
        log_path = "/var/log/tarificador/cucm-service.log"
        
        if not os.path.exists(log_path):
            return {"logs": "Archivo de log no encontrado"}
        
        # Leer las últimas líneas del archivo de log
        with open(log_path, 'r') as f:
            # Leer todo el archivo si es pequeño, o usar readlines con un buffer para archivos grandes
            if os.path.getsize(log_path) < 1024 * 1024:  # < 1MB
                all_lines = f.readlines()
            else:
                # Para archivos grandes, leer solo las últimas N líneas
                f.seek(0, os.SEEK_END)
                buffer_size = 8192
                file_size = f.tell()
                block_end = file_size
                
                # Colectar líneas hasta que tengamos suficientes o lleguemos al inicio del archivo
                all_lines = []
                while len(all_lines) < lines and block_end > 0:
                    block_start = max(0, block_end - buffer_size)
                    f.seek(block_start)
                    
                    # Si no estamos al inicio del archivo, descartar la primera línea parcial
                    if block_start > 0:
                        f.readline()
                    
                    # Leer las líneas del bloque
                    block_lines = f.readlines()
                    
                    # Actualizar para el siguiente bloque
                    block_end = block_start
                    
                    # Agregar líneas al inicio (para mantener el orden)
                    all_lines = block_lines + all_lines
                
                # Limitar al número de líneas solicitadas
                all_lines = all_lines[-lines:]
        
        return {"logs": "".join(all_lines[-lines:])}
    except Exception as e:
        return {"logs": f"Error al leer logs: {str(e)}"}

# Endpoint para controlar el servicio
@app.post("/api/cucm/service/{action}")
async def control_cucm_service(action: str, user=Depends(admin_only)):
    if isinstance(user, RedirectResponse):
        return user
        
    if action not in ["start", "stop", "restart"]:
        raise HTTPException(status_code=400, detail=f"Acción no válida: {action}")
    
    try:
        # Verificar si systemctl está disponible
        if not os.path.exists("/bin/systemctl"):
            raise HTTPException(status_code=500, detail="El control de servicios systemd no está disponible en este sistema")
        
        # Ejecutar el comando correspondiente
        result = os.system(f"systemctl {action} tarificador-cucm.service")
        
        if result != 0:
            raise HTTPException(status_code=500, detail=f"Error al {action} el servicio")
        
        # Actualizar el estado en la base de datos
        db = SessionLocal()
        
        status_map = {
            "start": "starting",
            "stop": "stopping",
            "restart": "restarting"
        }
        
        update_query = text("""
            UPDATE cucm_config SET 
            last_status = :status,
            last_status_update = CURRENT_TIMESTAMP
            WHERE id = (SELECT id FROM cucm_config ORDER BY id DESC LIMIT 1)
        """)
        
        db.execute(update_query, {"status": status_map[action]})
        db.commit()
        db.close()
        
        action_name = {"start": "iniciar", "stop": "detener", "restart": "reiniciar"}[action]
        return {"message": f"Orden para {action_name} el servicio enviada correctamente"}
    except HTTPException:
        raise
    except Exception as e:
        raise HTTPException(status_code=500, detail=f"Error al controlar el servicio: {str(e)}")
    
# Endpoint para obtener configuración del servicio
@app.get("/api/config")
def get_service_config():
    # Retorna la configuración del servicio
    return {
        "cucm.host": "190.105.250.127",
        "cucm.user": "jtapiuser",
        "cucm.password": "fr4v4t3l",
        "cucm.appinfo": "TarificadorApp",
        "api.url": "http://localhost:8000",
        "monitor.extensions": "all",
        "reconnect.delay": "60",
        "log.level": "INFO"
    }

@app.post("/cdr/rejected")
def rejected_call(event: CallEvent):
    db = SessionLocal()
    
    # Registrar la llamada rechazada
    cdr = CDR(
        calling_number=event.calling_number,
        called_number=event.called_number,
        start_time=event.start_time,
        end_time=event.end_time,
        duration_seconds=0,
        cost=0,
        status="rejected_insufficient_balance"
    )
    db.add(cdr)
    db.commit()
    db.close()
    
    return {"message": "Llamada rechazada registrada"}

@app.get("/dashboard/monitoreo")
async def monitoreo(request: Request):
    # Verifica si el usuario está autenticado
    user = await authenticated_user(request)
    if not user:
        return RedirectResponse(url="/login", status_code=302)
    
    db = SessionLocal()
    
    try:
        # Estadísticas básicas para mostrar inicialmente
        today = datetime.now().date()

        llamadas_hoy = db.execute(
            text("SELECT COUNT(*) FROM cdr WHERE DATE(start_time) = '{today}'")
        ).scalar() or 0
        
        minutos_hoy = db.execute(
            text("SELECT COALESCE(SUM(duration_seconds)/60, 0) FROM cdr WHERE DATE(start_time) = '{today}'")
        ).scalar() or 0
        
        alertas_saldo = db.execute(
            text("SELECT calling_number, saldo FROM saldo_anexos WHERE saldo < 5.0 ORDER BY saldo ASC LIMIT 5")
        ).fetchall()
        
        llamadas_recientes = db.execute(
            text("""
            SELECT * FROM cdr 
            ORDER BY start_time DESC 
            LIMIT 10
            """)
        ).fetchall()
    
    finally:
        db.close()
    
    return templates.TemplateResponse("monitoreo.html", {
        "request": request,
        "user": user,
        "llamadas_hoy": llamadas_hoy,
        "minutos_hoy": round(float(minutos_hoy), 1),
        "alertas_saldo": alertas_saldo,
        "llamadas_recientes": llamadas_recientes
    })

# Nuevos endpoints para el módulo de zonas, prefijos y tarifas
@app.get("/dashboard/zonas")
async def dashboard_zonas(request: Request, user=Depends(admin_only)):
    if isinstance(user, RedirectResponse):
        return user
        
    db = SessionLocal()
    
    query = text("SELECT id, zone_name as nombre, description as descripcion FROM zones ORDER BY zone_name")
    zonas = db.execute(query).fetchall()
    
    db.close()
    
    return templates.TemplateResponse("dashboard_zonas.html", {
        "request": request, "zonas": zonas, "user": user
    })


@app.get("/dashboard/prefijos")
async def dashboard_prefijos(request: Request, user=Depends(admin_only), zona_id: int = Query(None)):
    if isinstance(user, RedirectResponse):
        return user
        
    db = SessionLocal()
    try:
        # Obtener zonas para el dropdown
        zonas_query = db.execute(text("""
            SELECT id, zone_name 
            FROM zones 
            WHERE enabled = true
            ORDER BY zone_name
        """)).fetchall()
        zonas = [[z.id, z.zone_name] for z in zonas_query]
        
        # Obtener prefijos
        if zona_id:
            prefijos_query = db.execute(text("""
                SELECT 
                    p.id,
                    p.zone_id,
                    p.prefix,
                    LENGTH(p.prefix) as longitud_minima,  -- Longitud del prefijo mismo
                    p.prefix_length as longitud_maxima,   -- Longitud máxima del número completo
                    z.zone_name,
                    COALESCE(p.operator_name, 'N/A')
                FROM prefixes p
                LEFT JOIN zones z ON p.zone_id = z.id
                WHERE p.zone_id = :zona_id AND p.enabled = true
                ORDER BY p.prefix
            """), {"zona_id": zona_id}).fetchall()
            
            prefijos = [[p[0], p[1], p[2], p[3], p[4], p[5], p[6]] for p in prefijos_query]
            zona_actual = [zona_id, next((z[1] for z in zonas if z[0] == zona_id), None)]
        else:
            prefijos_query = db.execute(text("""
                SELECT 
                    p.id,
                    p.zone_id,
                    p.prefix,
                    LENGTH(p.prefix) as longitud_minima,
                    p.prefix_length as longitud_maxima,
                    z.zone_name,
                    COALESCE(p.operator_name, 'N/A')
                FROM prefixes p
                LEFT JOIN zones z ON p.zone_id = z.id
                WHERE p.enabled = true
                ORDER BY p.prefix
                LIMIT 100
            """)).fetchall()
            
            prefijos = [[p[0], p[1], p[2], p[3], p[4], p[5], p[6]] for p in prefijos_query]
            zona_actual = None
        
        return templates.TemplateResponse("dashboard_prefijos.html", {
            "request": request,
            "prefijos": prefijos,
            "zonas": zonas,
            "zona_actual": zona_actual,
            "user": user
        })
    finally:
        db.close()

@app.get("/dashboard/tarifas")
async def dashboard_tarifas(request: Request, zona_id: int = None, user=Depends(admin_only)):
    if isinstance(user, RedirectResponse):
        return user
        
    db = SessionLocal()
    
    zonas_query = text("SELECT id, zone_name as nombre FROM zones ORDER BY zone_name")
    zonas = db.execute(zonas_query).fetchall()
    
    if zona_id:
        tarifas_query = text("""
            SELECT t.id, t.zone_id, t.rate_per_minute / 60 as tarifa_segundo, t.effective_from as fecha_inicio, t.enabled as activa, z.zone_name as zona_nombre
            FROM rate_zones t
            JOIN zones z ON t.zone_id = z.id
            WHERE t.zone_id = :zona_id
            ORDER BY t.effective_from DESC
        """)
        tarifas = db.execute(tarifas_query, {"zona_id": zona_id}).fetchall()
        
        zona_query = text("SELECT id, zone_name as nombre FROM zones WHERE id = :zona_id")
        zona_actual = db.execute(zona_query, {"zona_id": zona_id}).fetchone()
    else:
        tarifas_query = text("""
            SELECT t.id, t.zone_id, t.rate_per_minute / 60 as tarifa_segundo, t.effective_from as fecha_inicio, t.enabled as activa, z.zone_name as zona_nombre
            FROM rate_zones t
            JOIN zones z ON t.zone_id = z.id
            ORDER BY z.zone_name, t.effective_from DESC
        """)
        tarifas = db.execute(tarifas_query).fetchall()
        zona_actual = None
    
    db.close()
    
    return templates.TemplateResponse("dashboard_tarifas.html", {
        "request": request, "tarifas": tarifas, "zonas": zonas, 
        "zona_actual": zona_actual, "user": user
    })

@app.get("/dashboard/estadisticas_zona")
def dashboard_estadisticas_zona(request: Request, 
                               user=Depends(authenticated_user),
                               fecha_inicio: str = Query(None),
                               fecha_fin: str = Query(None)):
    db = SessionLocal()
    # Establecer fechas por defecto si no se proporcionan
    if not fecha_inicio:
        fecha_inicio = (datetime.now() - timedelta(days=30)).strftime('%Y-%m-%d')
    if not fecha_fin:
        fecha_fin = datetime.now().strftime('%Y-%m-%d')
    
    # Consulta SQL
    query = f"""
        SELECT 
            z.zone_name as zona_nombre,
            COUNT(*) as total_llamadas,
            SUM(c.duration_seconds) / 60.0 as duracion_total_minutos,
            SUM(c.cost) as costo_total,
            AVG(c.cost) as costo_promedio,
            AVG(c.duration_seconds) / 60.0 as duracion_promedio_minutos
        FROM cdr c
        JOIN prefixes p ON SUBSTR(c.called_number, 1, LENGTH(p.prefix)) = p.prefix
        JOIN zones z ON p.zone_id = z.id
        WHERE c.start_time >= '{fecha_inicio} 00:00:00'
          AND c.start_time <= '{fecha_fin} 23:59:59'
        GROUP BY z.id, z.zone_name
        ORDER BY costo_total DESC
    """
    estadisticas = db.execute(text(query)).fetchall()
    db.close()

    # Calcular totales y porcentajes
    total_costo = sum(float(e.costo_total) if e.costo_total else 0 for e in estadisticas)

    # Crear nuevas estadísticas con el campo porcentaje_total
    estadisticas_modificadas = []
    for stat in estadisticas:
        costo_total = float(stat.costo_total) if stat.costo_total else 0
        porcentaje_total = (costo_total / total_costo * 100) if total_costo > 0 else 0

        estadisticas_modificadas.append({
            "zona_nombre": stat.zona_nombre,
            "total_llamadas": stat.total_llamadas,
            "duracion_total_minutos": stat.duracion_total_minutos,
            "costo_total": costo_total,
            "costo_promedio": stat.costo_promedio,
            "duracion_promedio_minutos": stat.duracion_promedio_minutos,
            "porcentaje_total": porcentaje_total
        })

    # Preparar datos para gráficos
    zonas_labels = [e.zona_nombre for e in estadisticas] if estadisticas else []
    llamadas_data = [e.total_llamadas for e in estadisticas] if estadisticas else []
    costo_data = [float(e.costo_total) for e in estadisticas] if estadisticas else []

    return templates.TemplateResponse("dashboard_estadisticas_zona.html", {
        "request": request,
        "user": user,
        "estadisticas": estadisticas_modificadas,
        "total_llamadas": sum(e.total_llamadas for e in estadisticas) if estadisticas else 0,
        "total_minutos": sum(e.duracion_total_minutos for e in estadisticas) if estadisticas else 0,
        "total_costo": total_costo,
        "zonas_activas": len(estadisticas),
        "zonas_labels": zonas_labels,
        "llamadas_data": llamadas_data,
        "costo_data": costo_data,
        "fecha_inicio": fecha_inicio,
        "fecha_fin": fecha_fin
    })

@app.get("/export/cdr/pdf")
def export_cdr_pdf(
    user=Depends(admin_only),
    start_date: str = Query(None),
    end_date: str = Query(None),
    calling_number: str = Query(None),
    called_number: str = Query(None),
    # ✅ AGREGAR NUEVOS PARÁMETROS
    phone_number: str = Query(None),
    status: str = Query(None),
    direction: str = Query(None)
):
    """Exporta los registros CDR a un archivo PDF con todos los filtros del dashboard."""
    from sqlalchemy import text
    from datetime import datetime
    from jinja2 import Template
    from weasyprint import HTML
    import io
    
    db = SessionLocal()
    
    try:
        # ✅ CONSULTA BASE ADAPTADA A TU ESQUEMA
        query = """
            SELECT c.calling_number, c.called_number, 
                   c.start_time, c.end_time, c.duration_seconds, 
                   c.duration_billable, c.cost, c.status,
                   z.zone_name as zona
            FROM cdr c
            LEFT JOIN zones z ON c.zona_id = z.id
            WHERE 1=1
        """
        
        params = {}
        
        # ✅ FILTRO DE NÚMERO MEJORADO (busca en origen Y destino)
        if phone_number:
            query += " AND (c.calling_number LIKE :phone_pattern OR c.called_number LIKE :phone_pattern)"
            params["phone_pattern"] = f"%{phone_number}%"
        
        # ✅ MANTENER COMPATIBILIDAD CON FILTROS EXISTENTES
        if calling_number and not phone_number:  # Solo si no se usa phone_number
            query += " AND c.calling_number = :calling_number"
            params["calling_number"] = calling_number
        
        if called_number and not phone_number:  # Solo si no se usa phone_number
            query += " AND c.called_number = :called_number"
            params["called_number"] = called_number
        
        # ✅ FILTROS DE FECHA (mantener igual)
        if start_date:
            query += " AND c.start_time >= :start_date"
            params["start_date"] = f"{start_date} 00:00:00"
        
        if end_date:
            query += " AND c.end_time <= :end_date"
            params["end_date"] = f"{end_date} 23:59:59"
        
        # ✅ NUEVO: FILTRO DE ESTADO
        if status:
            query += " AND c.status = :status"
            params["status"] = status
        
        # ✅ NUEVO: FILTRO DE DIRECCIÓN  
        if direction:
            query += " AND c.direction = :direction"
            params["direction"] = direction
        
        # Ordenar por fecha descendente
        query += " ORDER BY c.start_time DESC LIMIT 1000"
        
        print(f"🔍 Query PDF: {query}")
        print(f"🔍 Params PDF: {params}")
        
        # Ejecutar la consulta
        rows = db.execute(text(query), params).fetchall()
        
        print(f"📊 Registros encontrados para PDF: {len(rows)}")
        
        # ✅ HTML TEMPLATE MEJORADO CON INFO DE FILTROS
        html_template = """
        <html>
        <head>
            <style>
                body { font-family: Arial, sans-serif; margin: 20px; }
                table { width: 100%; border-collapse: collapse; margin-top: 20px; }
                th, td { border: 1px solid #ddd; padding: 8px; text-align: left; font-size: 10px; }
                th { background-color: #f2f2f2; font-weight: bold; }
                .header { text-align: center; margin-bottom: 20px; }
                .filters { background-color: #f9f9f9; padding: 10px; margin-bottom: 20px; border-radius: 5px; }
                .footer { text-align: center; margin-top: 20px; font-size: 0.8em; color: #666; }
                h1 { color: #333; margin-bottom: 10px; }
                .filter-item { display: inline-block; margin-right: 15px; font-size: 0.9em; }
            </style>
        </head>
        <body>
            <div class="header">
                <h1>📞 Registro de Llamadas (CDR)</h1>
                <p><strong>Total de registros:</strong> {{ rows|length }}</p>
                <p><strong>Generado:</strong> {{ datetime.now().strftime('%Y-%m-%d %H:%M:%S') }}</p>
            </div>
            
            <!-- ✅ MOSTRAR FILTROS APLICADOS -->
            {% if phone_number or calling_number or called_number or start_date or end_date or status or direction %}
            <div class="filters">
                <strong>🔍 Filtros aplicados:</strong><br/>
                {% if phone_number %}
                <span class="filter-item">📞 <strong>Número:</strong> {{ phone_number }}</span>
                {% endif %}
                {% if calling_number %}
                <span class="filter-item">📞 <strong>Origen:</strong> {{ calling_number }}</span>
                {% endif %}
                {% if called_number %}
                <span class="filter-item">📱 <strong>Destino:</strong> {{ called_number }}</span>
                {% endif %}
                {% if start_date %}
                <span class="filter-item">📅 <strong>Desde:</strong> {{ start_date }}</span>
                {% endif %}
                {% if end_date %}
                <span class="filter-item">📅 <strong>Hasta:</strong> {{ end_date }}</span>
                {% endif %}
                {% if status %}
                <span class="filter-item">📊 <strong>Estado:</strong> {{ status }}</span>
                {% endif %}
                {% if direction %}
                <span class="filter-item">🔄 <strong>Dirección:</strong> {{ direction }}</span>
                {% endif %}
            </div>
            {% endif %}
            
            {% if rows %}
            <table>
                <thead>
                    <tr>
                        <th>📞 Origen</th>
                        <th>📱 Destino</th>
                        <th>📅 Fecha/Hora Inicio</th>
                        <th>⏱️ Duración</th>
                        <th>💰 Facturada</th>
                        <th>🌍 Zona</th>
                        <th>💵 Costo</th>
                        <th>📊 Estado</th>
                    </tr>
                </thead>
                <tbody>
                {% for row in rows %}
                    <tr>
                        <td>{{ row[0] }}</td>
                        <td>{{ row[1] }}</td>
                        <td>{{ row[2].strftime('%Y-%m-%d %H:%M:%S') if row[2] else 'N/A' }}</td>
                        <td>{{ "%d:%02d"|format(row[4]//60, row[4]%60) if row[4] else '0:00' }}</td>
                        <td>{{ "%d:%02d"|format(row[5]//60, row[5]%60) if row[5] else '0:00' }}</td>
                        <td>{{ row[8] or 'N/A' }}</td>
                        <td>S/{{ "%.6f"|format(row[6]) if row[6] else '0.000000' }}</td>
                        <td>{{ row[7] or 'N/A' }}</td>
                    </tr>
                {% endfor %}
                </tbody>
            </table>
            {% else %}
            <div style="text-align: center; padding: 40px; color: #666;">
                <h3>📭 No se encontraron registros</h3>
                <p>No hay llamadas que coincidan con los filtros aplicados.</p>
            </div>
            {% endif %}
            
            <div class="footer">
                <p>📄 Reporte generado el {{ datetime.now().strftime('%Y-%m-%d %H:%M:%S') }} | 
                   📊 Total: {{ rows|length }} registro{{ 's' if rows|length != 1 else '' }}</p>
            </div>
        </body>
        </html>
        """
        
        # Renderizar el HTML con Jinja2
        template = Template(html_template)
        html_content = template.render(
            rows=rows, 
            datetime=datetime,
            start_date=start_date,
            end_date=end_date,
            calling_number=calling_number,
            called_number=called_number,
            phone_number=phone_number,
            status=status,
            direction=direction
        )
        
        # Generar el PDF usando WeasyPrint
        pdf = HTML(string=html_content).write_pdf()
        
        # ✅ NOMBRE DE ARCHIVO CON INFO DE FILTROS
        filename_parts = ["cdr_report"]
        if phone_number:
            filename_parts.append(f"num_{phone_number}")
        if status:
            filename_parts.append(f"status_{status}")
        if direction:
            filename_parts.append(f"dir_{direction}")
        
        filename_parts.append(datetime.now().strftime('%Y%m%d_%H%M%S'))
        filename = "_".join(filename_parts) + ".pdf"
        
        db.close()
        
        # Devolver el PDF como respuesta
        return StreamingResponse(
            io.BytesIO(pdf),
            media_type="application/pdf",
            headers={"Content-Disposition": f"attachment; filename={filename}"}
        )
        
    except Exception as e:
        db.close()
        import traceback
        traceback.print_exc()
        return {"error": f"Error al generar PDF: {str(e)}"}

# ✅ ENDPOINT EXCEL USANDO LA MISMA LÓGICA
@app.get("/export/cdr/excel")
def export_cdr_excel(
    user=Depends(admin_only),
    start_date: str = Query(None),
    end_date: str = Query(None),
    calling_number: str = Query(None),
    called_number: str = Query(None),
    phone_number: str = Query(None),
    status: str = Query(None),
    direction: str = Query(None)
):
    """Exporta los registros CDR a un archivo Excel con todos los filtros."""
    from sqlalchemy import text
    from datetime import datetime
    import pandas as pd
    import io
    
    db = SessionLocal()
    
    try:
        # ✅ USAR LA MISMA LÓGICA DE CONSULTA QUE EL PDF
        query = """
            SELECT c.calling_number, c.called_number, 
                   c.start_time, c.end_time, c.duration_seconds, 
                   c.duration_billable, c.cost, c.status,
                   z.zone_name as zona
            FROM cdr c
            LEFT JOIN zones z ON c.zona_id = z.id
            WHERE 1=1
        """
        
        params = {}
        
        # Aplicar filtros (misma lógica que PDF)
        if phone_number:
            query += " AND (c.calling_number LIKE :phone_pattern OR c.called_number LIKE :phone_pattern)"
            params["phone_pattern"] = f"%{phone_number}%"
        
        if calling_number and not phone_number:
            query += " AND c.calling_number = :calling_number"
            params["calling_number"] = calling_number
        
        if called_number and not phone_number:
            query += " AND c.called_number = :called_number"
            params["called_number"] = called_number
        
        if start_date:
            query += " AND c.start_time >= :start_date"
            params["start_date"] = f"{start_date} 00:00:00"
        
        if end_date:
            query += " AND c.end_time <= :end_date"
            params["end_date"] = f"{end_date} 23:59:59"
        
        if status:
            query += " AND c.status = :status"
            params["status"] = status
        
        if direction:
            query += " AND c.direction = :direction"
            params["direction"] = direction
        
        query += " ORDER BY c.start_time DESC LIMIT 1000"
        
        print(f"🔍 Query Excel: {query}")
        print(f"🔍 Params Excel: {params}")
        
        # Ejecutar consulta
        rows = db.execute(text(query), params).fetchall()
        
        print(f"📊 Registros encontrados para Excel: {len(rows)}")
        
        # Crear DataFrame
        if rows:
            df_data = []
            for row in rows:
                duration_min = round(row[4]/60, 2) if row[4] else 0
                billable_min = round(row[5]/60, 2) if row[5] else 0
                
                df_data.append({
                    'Número Origen': row[0],
                    'Número Destino': row[1],
                    'Fecha Inicio': row[2].strftime('%Y-%m-%d %H:%M:%S') if row[2] else '',
                    'Fecha Fin': row[3].strftime('%Y-%m-%d %H:%M:%S') if row[3] else '',
                    'Duración (seg)': row[4] if row[4] else 0,
                    'Duración (min)': duration_min,
                    'Facturable (seg)': row[5] if row[5] else 0,
                    'Facturable (min)': billable_min,
                    'Costo': row[6] if row[6] else 0,
                    'Estado': row[7] if row[7] else '',
                    'Zona': row[8] if row[8] else ''
                })
            
            df = pd.DataFrame(df_data)
        else:
            df = pd.DataFrame()
        
        # Crear Excel
        buffer = io.BytesIO()
        with pd.ExcelWriter(buffer, engine='openpyxl') as writer:
            # Hoja principal con datos
            df.to_excel(writer, sheet_name='CDR_Data', index=False)
            
            # Hoja de resumen con filtros aplicados
            summary_data = {
                'Filtro': [],
                'Valor': []
            }
            
            if phone_number:
                summary_data['Filtro'].append('Número')
                summary_data['Valor'].append(phone_number)
            if calling_number:
                summary_data['Filtro'].append('Origen')
                summary_data['Valor'].append(calling_number)
            if called_number:
                summary_data['Filtro'].append('Destino')
                summary_data['Valor'].append(called_number)
            if start_date:
                summary_data['Filtro'].append('Fecha Inicio')
                summary_data['Valor'].append(start_date)
            if end_date:
                summary_data['Filtro'].append('Fecha Fin')
                summary_data['Valor'].append(end_date)
            if status:
                summary_data['Filtro'].append('Estado')
                summary_data['Valor'].append(status)
            if direction:
                summary_data['Filtro'].append('Dirección')
                summary_data['Valor'].append(direction)
            
            summary_data['Filtro'].extend(['Total Registros', 'Fecha Generación'])
            summary_data['Valor'].extend([len(rows), datetime.now().strftime('%Y-%m-%d %H:%M:%S')])
            
            summary_df = pd.DataFrame(summary_data)
            summary_df.to_excel(writer, sheet_name='Resumen', index=False)
        
        buffer.seek(0)
        
        # Nombre de archivo
        filename_parts = ["cdr_report"]
        if phone_number:
            filename_parts.append(f"num_{phone_number}")
        if status:
            filename_parts.append(f"status_{status}")
        
        filename_parts.append(datetime.now().strftime('%Y%m%d_%H%M%S'))
        filename = "_".join(filename_parts) + ".xlsx"
        
        db.close()
        
        return StreamingResponse(
            io.BytesIO(buffer.getvalue()),
            media_type="application/vnd.openxmlformats-officedocument.spreadsheetml.sheet",
            headers={"Content-Disposition": f"attachment; filename={filename}"}
        )
        
    except Exception as e:
        db.close()
        import traceback
        traceback.print_exc()
        return {"error": f"Error al generar Excel: {str(e)}"}
                    
# Exportar reporte de consumo por zona
@app.get("/export/consumo_zona/pdf")
async def export_consumo_zona_pdf(user=Depends(admin_only)):
    if isinstance(user, RedirectResponse):
        return user
    
    db = SessionLocal()
    
    # Estadísticas de consumo por zona (últimos 30 días)
    stats_query = text("""
        SELECT z.zone_name, COUNT(c.id) as total_llamadas, 
               SUM(c.duration_seconds) as total_duracion, 
               SUM(c.cost) as total_costo
        FROM cdr c
        JOIN zones z ON c.zona_id = z.id
        WHERE c.start_time >= NOW() - INTERVAL '30 days'
        GROUP BY z.zone_name
        ORDER BY total_costo DESC
    """)
    estadisticas = db.execute(stats_query).fetchall()
    db.close()

    html_template = """
    <html>
    <body>
    <h1>Reporte de Consumo por Zona</h1>
    <h3>Últimos 30 días</h3>
    <table border="1">
        <thead>
            <tr>
                <th>Zona</th>
                <th>Total Llamadas</th>
                <th>Duración Total (seg)</th>
                <th>Costo Total</th>
            </tr>
        </thead>
        <tbody>
        {% for row in estadisticas %}
            <tr>
                <td>{{ row[0] }}</td>
                <td>{{ row[1] }}</td>
                <td>{{ row[2] if row[2] else 0 }}</td>
                <td>${{ "%.2f"|format(row[3] if row[3] else 0) }}</td>
            </tr>
        {% endfor %}
        </tbody>
    </table>
    </body>
    </html>
    """
    template = Template(html_template)
    html_content = template.render(estadisticas=estadisticas)

    pdf = HTML(string=html_content).write_pdf()

    return StreamingResponse(
        io.BytesIO(pdf),
        media_type="application/pdf",
        headers={"Content-Disposition": "attachment; filename=consumo_zona_report.pdf"}
    )

# API para el módulo de zonas
@app.get("/api/zonas")
async def listar_zonas(user=Depends(authenticated_user)):
    if isinstance(user, RedirectResponse):
        return user
        
    db = SessionLocal()
    query = text("SELECT id, zone_name as nombre, description as descripcion FROM zones ORDER BY zone_name")
    zonas = db.execute(query).fetchall()
    
    result = []
    for zona in zonas:
        result.append({
            "id": zona[0],
            "nombre": zona[1],
            "descripcion": zona[2]
        })
    
    db.close()
    return result

@app.post("/api/zonas")
async def crear_zona(zona: ZonaCreate, user=Depends(admin_only)):
    if isinstance(user, RedirectResponse):
        return user
        
    db = SessionLocal()
    
    # Verificar que no exista otra zona con el mismo nombre
    # Verificar que no exista otra zona con el mismo nombre
    check_query = text("SELECT id FROM zones WHERE zone_name = :nombre")
    existing = db.execute(check_query, {"nombre": zona.nombre}).fetchone()
    
    if existing:
        db.close()
        raise HTTPException(status_code=400, detail="Ya existe una zona con ese nombre")
    
    # Crear la zona
    # Crear la zona con defaults para la tabla zones (apolobilling)
    insert_query = text("""
        INSERT INTO zones (zone_name, description, country_id, zone_code, zone_type, region_name, enabled, created_at, updated_at)
        VALUES (:nombre, :descripcion, 4, :code, 'GEOGRAPHIC', 'Default', TRUE, CURRENT_TIMESTAMP, CURRENT_TIMESTAMP)
        RETURNING id
    """)
    
    # Generar un código simple basado en el nombre
    code = zona.nombre.upper().replace(" ", "-")[:20]
    
    result = db.execute(insert_query, {
        "nombre": zona.nombre,
        "descripcion": zona.descripcion,
        "code": code
    })
    
    zona_id = result.fetchone()[0]
    
    # Crear una tarifa por defecto para la zona
    # Crear una tarifa por defecto para la zona en rate_zones
    # rate_zones requeridos: zone_id, rate_name, rate_per_minute, rate_per_call, billing_increment, min_duration, effective_from, currency, priority, enabled
    insert_tarifa_query = text("""
        INSERT INTO rate_zones (
            zone_id, rate_name, rate_per_minute, rate_per_call, billing_increment, 
            min_duration, effective_from, currency, priority, enabled, created_at, updated_at
        )
        VALUES (
            :zona_id, 'Default Rate', :rate_per_minute, 0, 60, 
            0, CURRENT_TIMESTAMP, 'USD', 1, TRUE, CURRENT_TIMESTAMP, CURRENT_TIMESTAMP
        )
    """)
    
    db.execute(insert_tarifa_query, {
        "zona_id": zona_id,
        "rate_per_minute": 0.03  # $0.03/min
    })
    
    db.commit()
    db.close()
    
    return {"id": zona_id, "nombre": zona.nombre, "descripcion": zona.descripcion}

@app.put("/api/zonas/{zona_id}")
async def actualizar_zona(zona_id: int, zona: ZonaCreate, user=Depends(admin_only)):
    if isinstance(user, RedirectResponse):
        return user
        
    db = SessionLocal()
    
    # Verificar que la zona exista
    check_query = text("SELECT id FROM zones WHERE id = :zona_id")
    existing = db.execute(check_query, {"zona_id": zona_id}).fetchone()
    
    if not existing:
        db.close()
        raise HTTPException(status_code=404, detail="Zona no encontrada")
    
    # Verificar que no exista otra zona con el mismo nombre
    check_name_query = text("SELECT id FROM zones WHERE zone_name = :nombre AND id != :zona_id")
    existing_name = db.execute(check_name_query, {"nombre": zona.nombre, "zona_id": zona_id}).fetchone()
    
    if existing_name:
        db.close()
        raise HTTPException(status_code=400, detail="Ya existe otra zona con ese nombre")
    
    # Actualizar la zona
    update_query = text("""
        UPDATE zones
        SET zone_name = :nombre, description = :descripcion, updated_at = CURRENT_TIMESTAMP
        WHERE id = :zona_id
    """)
    
    db.execute(update_query, {
        "zona_id": zona_id,
        "nombre": zona.nombre,
        "descripcion": zona.descripcion
    })
    
    db.commit()
    db.close()
    
    return {"id": zona_id, "nombre": zona.nombre, "descripcion": zona.descripcion}

@app.delete("/api/zonas/{zona_id}")
async def eliminar_zona(zona_id: int, user=Depends(admin_only)):
    if isinstance(user, RedirectResponse):
        return user
        
    db = SessionLocal()
    
    # Verificar que la zona exista
    check_query = text("SELECT id FROM zones WHERE id = :zona_id")
    existing = db.execute(check_query, {"zona_id": zona_id}).fetchone()
    
    if not existing:
        db.close()
        raise HTTPException(status_code=404, detail="Zona no encontrada")
    
    # Verificar si tiene prefijos o tarifas asociadas
    check_prefijos_query = text("SELECT COUNT(*) FROM prefixes WHERE zone_id = :zona_id")
    prefijos_count = db.execute(check_prefijos_query, {"zona_id": zona_id}).scalar()
    
    check_tarifas_query = text("SELECT COUNT(*) FROM rate_zones WHERE zone_id = :zona_id")
    tarifas_count = db.execute(check_tarifas_query, {"zona_id": zona_id}).scalar()
    
    if prefijos_count > 0 or tarifas_count > 0:
        db.close()
        raise HTTPException(
            status_code=400, 
            detail="No se puede eliminar la zona porque tiene prefijos o tarifas asociadas"
        )
    
    # Eliminar la zona
    delete_query = text("DELETE FROM zones WHERE id = :zona_id")
    db.execute(delete_query, {"zona_id": zona_id})
    
    db.commit()
    db.close()
    
    return {"message": "Zona eliminada correctamente"}


# Endpoint de diagnóstico para verificar zonas
@app.get("/api/debug/zonas")
async def debug_zonas(user=Depends(authenticated_user)):
    """Endpoint de diagnóstico para verificar el estado de las zonas en la base de datos"""
    if isinstance(user, RedirectResponse):
        return user
        
    db = SessionLocal()
    
    try:
        # Contar zonas
        count_query = text("SELECT COUNT(*) FROM zones")
        total_zonas = db.execute(count_query).scalar()
        
        # Obtener todas las zonas con todos sus campos
        zonas_query = text("SELECT id, zone_name as nombre, description as descripcion FROM zones ORDER BY id")
        zonas = db.execute(zonas_query).fetchall()
        
        # Contar prefijos por zona
        prefijos_query = text("""
            SELECT z.id, z.zone_name, COUNT(p.id) as total_prefijos
            FROM zones z
            LEFT JOIN prefixes p ON z.id = p.zone_id
            GROUP BY z.id, z.zone_name
            ORDER BY z.id
        """)
        prefijos_por_zona = db.execute(prefijos_query).fetchall()
        
        zonas_detalle = []
        for zona in zonas:
            # Buscar el conteo de prefijos para esta zona
            prefijos_count = 0
            for pz in prefijos_por_zona:
                if pz[0] == zona[0]:
                    prefijos_count = pz[2]
                    break
            
            zonas_detalle.append({
                "id": zona[0],
                "nombre": zona[1],
                "descripcion": zona[2],
                "total_prefijos": prefijos_count
            })
        
        return {
            "total_zonas": total_zonas,
            "zonas": zonas_detalle,
            "mensaje": f"Se encontraron {total_zonas} zonas en la base de datos"
        }
    except Exception as e:
        return {
            "error": str(e),
            "mensaje": "Error al consultar la base de datos"
        }
    finally:
        db.close()

# API para el módulo de prefijos
@app.get("/api/prefijos")
async def listar_prefijos(zona_id: int = None, user=Depends(authenticated_user)):
    if isinstance(user, RedirectResponse):
        return user
        
    db = SessionLocal()
    
    if zona_id:
        query = text("""
            SELECT p.id, p.zone_id as zona_id, p.prefix as prefijo, p.prefix_length as longitud_minima, p.prefix_length as longitud_maxima, z.zone_name as zona_nombre
            FROM prefixes p
            JOIN zones z ON p.zone_id = z.id
            WHERE p.zone_id = :zona_id
            ORDER BY p.prefix
        """)
        prefijos = db.execute(query, {"zona_id": zona_id}).fetchall()
    else:
        query = text("""
            SELECT p.id, p.zone_id as zona_id, p.prefix as prefijo, p.prefix_length as longitud_minima, p.prefix_length as longitud_maxima, z.zone_name as zona_nombre
            FROM prefixes p
            JOIN zones z ON p.zone_id = z.id
            ORDER BY z.zone_name, p.prefix
        """)
        prefijos = db.execute(query).fetchall()
    
    result = []
    for prefijo in prefijos:
        result.append({
            "id": prefijo[0],
            "zona_id": prefijo[1],
            "prefijo": prefijo[2],
            "longitud_minima": prefijo[3],
            "longitud_maxima": prefijo[4],
            "zona_nombre": prefijo[5]
        })
    
    db.close()
    return result

@app.post("/api/prefijos")
async def crear_prefijo(prefijo: PrefijoCreate, user=Depends(admin_only)):
    if isinstance(user, RedirectResponse):
        return user
        
    db = SessionLocal()
    
    # Verificar que la zona exista
    check_zona_query = text("SELECT id FROM zones WHERE id = :zona_id")
    existing_zona = db.execute(check_zona_query, {"zona_id": prefijo.zona_id}).fetchone()
    
    if not existing_zona:
        db.close()
        raise HTTPException(status_code=404, detail="Zona no encontrada")
    
    # Validar longitudes
    if prefijo.longitud_minima > prefijo.longitud_maxima:
        db.close()
        raise HTTPException(status_code=400, detail="La longitud mínima no puede ser mayor que la longitud máxima")
    
    # Insertar el prefijo
    # Insertar el prefijo en prefixes
    insert_query = text("""
        INSERT INTO prefixes (zone_id, prefix, prefix_length, operator_name, network_type, enabled, created_at, updated_at)
        VALUES (:zona_id, :prefijo, :longitud_minima, 'Default', 'UNKNOWN', TRUE, CURRENT_TIMESTAMP, CURRENT_TIMESTAMP)
        RETURNING id
    """)
    
    result = db.execute(insert_query, {
        "zona_id": prefijo.zona_id,
        "prefijo": prefijo.prefijo,
        "longitud_minima": prefijo.longitud_minima
    })
    
    prefijo_id = result.fetchone()[0]
    
    # Obtener el nombre de la zona
    zona_query = text("SELECT zone_name as nombre FROM zones WHERE id = :zona_id")
    zona_nombre = db.execute(zona_query, {"zona_id": prefijo.zona_id}).fetchone()[0]
    
    db.commit()
    
    # Sincronizar con el motor Rust
    sync_rate_cards(db)
    
    db.close()
    
    return {
        "id": prefijo_id,
        "zona_id": prefijo.zona_id,
        "prefijo": prefijo.prefijo,
        "longitud_minima": prefijo.longitud_minima,
        "longitud_maxima": prefijo.longitud_maxima,
        "zona_nombre": zona_nombre
    }


@app.put("/api/prefijos/{prefijo_id}")
async def api_update_prefijo(
    prefijo_id: int,
    zona_id: int = Form(...),
    prefijo: str = Form(...),
    longitud_minima: int = Form(...),
    longitud_maxima: int = Form(...),
    db: Session = Depends(get_db),
    user = Depends(get_admin_user)
):
    """Actualizar un prefijo existente"""
    try:
        # Verificar que el prefijo existe
        existing = db.execute(text("SELECT id FROM prefixes WHERE id = :id"), 
                            {"id": prefijo_id}).fetchone()
        if not existing:
            raise HTTPException(status_code=404, detail="Prefijo no encontrado")
        
        # Validar que la zona existe
        zona = db.execute(text("SELECT id FROM zones WHERE id = :zona_id"), 
                         {"zona_id": zona_id}).fetchone()
        if not zona:
            raise HTTPException(status_code=404, detail="Zona no encontrada")
        
        # Validar longitudes
        if longitud_minima > longitud_maxima:
            raise HTTPException(status_code=400, 
                              detail="La longitud mínima no puede ser mayor que la máxima")
        
        # Actualizar prefijo (solo guardamos longitud_maxima en prefix_length)
        db.execute(text("""
            UPDATE prefixes
            SET zone_id = :zone_id,
                prefix = :prefix,
                prefix_length = :prefix_length,
                updated_at = CURRENT_TIMESTAMP
            WHERE id = :id
        """), {
            "id": prefijo_id,
            "zone_id": zona_id,
            "prefix": prefijo,
            "prefix_length": longitud_maxima  # Solo guardamos la máxima
        })
        db.commit()
        
        # Sincronizar con motor Rust
        sync_rate_cards(db)
        
        return {"success": True, "message": "Prefijo actualizado correctamente"}
        
    except HTTPException:
        raise
    except Exception as e:
        db.rollback()
        raise HTTPException(status_code=500, detail=str(e))
    finally:
        db.close()
        

@app.delete("/api/prefijos/{prefijo_id}")
async def eliminar_prefijo(prefijo_id: int, user=Depends(admin_only)):
    if isinstance(user, RedirectResponse):
        return user
        
    db = SessionLocal()
    
    # Verificar que el prefijo exista
    check_query = text("SELECT id FROM prefixes WHERE id = :prefijo_id")
    existing = db.execute(check_query, {"prefijo_id": prefijo_id}).fetchone()
    
    if not existing:
        db.close()
        raise HTTPException(status_code=404, detail="Prefijo no encontrado")
    
    # Eliminar el prefijo
    # Eliminar el prefijo
    delete_query = text("DELETE FROM prefixes WHERE id = :prefijo_id")
    db.execute(delete_query, {"prefijo_id": prefijo_id})
    
    db.commit()
    
    # Sincronizar con el motor Rust
    sync_rate_cards(db)
    
    db.close()
    
    return {"message": "Prefijo eliminado correctamente"}

# API para el módulo de tarifas
@app.get("/api/tarifas")
async def listar_tarifas(zona_id: int = None, user=Depends(authenticated_user)):
    if isinstance(user, RedirectResponse):
        return user
        
    db = SessionLocal()
    
    if zona_id:
        query = text("""
            SELECT t.id, t.zone_id, t.rate_per_minute / 60 as tarifa_segundo, t.effective_from as fecha_inicio, t.enabled as activa, z.zone_name as zona_nombre
            FROM rate_zones t
            JOIN zones z ON t.zone_id = z.id
            WHERE t.zone_id = :zona_id
            ORDER BY t.effective_from DESC
        """)
        tarifas = db.execute(query, {"zona_id": zona_id}).fetchall()
    else:
        query = text("""
            SELECT t.id, t.zone_id, t.rate_per_minute / 60 as tarifa_segundo, t.effective_from as fecha_inicio, t.enabled as activa, z.zone_name as zona_nombre
            FROM rate_zones t
            JOIN zones z ON t.zone_id = z.id
            ORDER BY z.zone_name, t.effective_from DESC
        """)
        tarifas = db.execute(query).fetchall()
    
    result = []
    for tarifa in tarifas:
        result.append({
            "id": tarifa[0],
            "zona_id": tarifa[1],
            "tarifa_segundo": float(tarifa[2]),
            "fecha_inicio": tarifa[3].isoformat() if tarifa[3] else None,
            "activa": tarifa[4],
            "zona_nombre": tarifa[5]
        })
    
    db.close()
    return result

@app.post("/api/tarifas")
async def crear_tarifa(tarifa: TarifaCreate, user=Depends(admin_only)):
    if isinstance(user, RedirectResponse):
        return user
        
    db = SessionLocal()
    
    # Verificar que la zona exista
    check_zona_query = text("SELECT id FROM zones WHERE id = :zona_id")
    existing_zona = db.execute(check_zona_query, {"zona_id": tarifa.zona_id}).fetchone()
    
    if not existing_zona:
        db.close()
        raise HTTPException(status_code=404, detail="Zona no encontrada")
    
    # Desactivar las tarifas anteriores de esta zona
    update_query = text("""
        UPDATE rate_zones 
        SET enabled = FALSE 
        WHERE zone_id = :zona_id AND enabled = TRUE
    """)
    
    db.execute(update_query, {"zona_id": tarifa.zona_id})
    
    # Insertar la nueva tarifa
    # Insertar la nueva tarifa en rate_zones
    # tarifa_segundo entra, guardar como rate_per_minute (*60)
    insert_query = text("""
        INSERT INTO rate_zones (
            zone_id, rate_name, rate_per_minute, rate_per_call, billing_increment, 
            min_duration, effective_from, currency, priority, enabled, created_at, updated_at
        )
        VALUES (
            :zona_id, 'Manual Rate', :rate_per_minute, 0, 60, 
            0, CURRENT_TIMESTAMP, 'USD', 1, TRUE, CURRENT_TIMESTAMP, CURRENT_TIMESTAMP
        )
        RETURNING id, effective_from
    """)
    
    result = db.execute(insert_query, {
        "zona_id": tarifa.zona_id,
        "rate_per_minute": float(tarifa.tarifa_segundo) * 60
    })
    
    id_fecha = result.fetchone()
    tarifa_id = id_fecha[0]
    fecha_inicio = id_fecha[1]
    
    # Obtener el nombre de la zona
    zona_query = text("SELECT zone_name as nombre FROM zones WHERE id = :zona_id")
    zona_nombre = db.execute(zona_query, {"zona_id": tarifa.zona_id}).fetchone()[0]
    
    db.commit()
    
    # Sincronizar con el motor Rust
    sync_rate_cards(db)
    
    db.close()
    
    return {
        "id": tarifa_id,
        "zona_id": tarifa.zona_id,
        "tarifa_segundo": tarifa.tarifa_segundo,
        "fecha_inicio": fecha_inicio.isoformat() if fecha_inicio else None,
        "activa": True,
        "zona_nombre": zona_nombre
    }

@app.put("/api/tarifas/{tarifa_id}/activar")
async def activar_tarifa(tarifa_id: int, user=Depends(admin_only)):
    if isinstance(user, RedirectResponse):
        return user
        
    db = SessionLocal()
    
    # Verificar que la tarifa exista
    check_query = text("SELECT id, zone_id FROM rate_zones WHERE id = :tarifa_id")
    existing = db.execute(check_query, {"tarifa_id": tarifa_id}).fetchone()
    
    if not existing:
        db.close()
        raise HTTPException(status_code=404, detail="Tarifa no encontrada")
    
    zona_id = existing[1]
    
    # Desactivar las tarifas anteriores de esta zona
    update_other_query = text("""
        UPDATE rate_zones 
        SET enabled = FALSE 
        WHERE zone_id = :zona_id AND id != :tarifa_id AND enabled = TRUE
    """)
    
    db.execute(update_other_query, {"zona_id": zona_id, "tarifa_id": tarifa_id})
    
    # Activar esta tarifa
    update_query = text("""
        UPDATE rate_zones 
        SET enabled = TRUE 
        WHERE id = :tarifa_id
    """)
    
    db.execute(update_query, {"tarifa_id": tarifa_id})
    
    # Obtener datos actualizados de la tarifa
    tarifa_query = text("""
        SELECT t.id, t.zona_id, t.tarifa_segundo, t.fecha_inicio, t.activa, z.nombre as zona_nombre
        FROM tarifas t
        JOIN zonas z ON t.zona_id = z.id
        WHERE t.id = :tarifa_id
    """)
    
    tarifa = db.execute(tarifa_query, {"tarifa_id": tarifa_id}).fetchone()
    
    db.commit()
    db.close()
    
    return {
        "id": tarifa[0],
        "zona_id": tarifa[1],
        "tarifa_segundo": float(tarifa[2]),
        "fecha_inicio": tarifa[3].isoformat() if tarifa[3] else None,
        "activa": tarifa[4],
        "zona_nombre": tarifa[5],
        "message": "Tarifa activada correctamente"
    }

@app.delete("/api/tarifas/{tarifa_id}")
async def eliminar_tarifa(tarifa_id: int, user=Depends(admin_only)):
    if isinstance(user, RedirectResponse):
        return user
        
    db = SessionLocal()
    
    # Verificar que la tarifa exista
    check_query = text("SELECT id, zona_id, activa FROM tarifas WHERE id = :tarifa_id")
    existing = db.execute(check_query, {"tarifa_id": tarifa_id}).fetchone()
    
    if not existing:
        db.close()
        raise HTTPException(status_code=404, detail="Tarifa no encontrada")
    
    zona_id = existing[1]
    is_active = existing[2]
    
    # Si la tarifa está activa, verificar que haya otra tarifa que se pueda activar
    if is_active:
        check_others_query = text("""
            SELECT COUNT(*) 
            FROM tarifas 
            WHERE zona_id = :zona_id AND id != :tarifa_id
        """)
        
        other_count = db.execute(check_others_query, {"zona_id": zona_id, "tarifa_id": tarifa_id}).scalar()
        
        if other_count == 0:
            db.close()
            raise HTTPException(
                status_code=400, 
                detail="No se puede eliminar la única tarifa de la zona"
            )
        
        # Activar la tarifa más reciente de la zona
        activate_query = text("""
            UPDATE tarifas 
            SET activa = TRUE 
            WHERE zona_id = :zona_id AND id != :tarifa_id 
            ORDER BY fecha_inicio DESC 
            LIMIT 1
        """)
        
        db.execute(activate_query, {"zona_id": zona_id, "tarifa_id": tarifa_id})
    
    # Eliminar la tarifa
    delete_query = text("DELETE FROM tarifas WHERE id = :tarifa_id")
    db.execute(delete_query, {"tarifa_id": tarifa_id})
    
    db.commit()
    db.close()
    
    return {"message": "Tarifa eliminada correctamente"}


class FacCode(Base):
    __tablename__ = "fac_codes"
    id = Column(Integer, primary_key=True, index=True)
    authorization_code = Column(String, unique=True, index=True)
    authorization_code_name = Column(String)  # Nombre descriptivo para CUCM
    authorization_level = Column(Integer)     # Nivel de autorización (0-255)
    description = Column(String, nullable=True)
    active = Column(Boolean, default=True)
    created_at = Column(DateTime, default=datetime.utcnow)
    updated_at = Column(DateTime, default=datetime.utcnow, onupdate=datetime.utcnow)
    cucm_synced = Column(Boolean, default=False)  # Indicador de sincronización
    
class FacAudit(Base):
    __tablename__ = "fac_audit"
    id = Column(Integer, primary_key=True, index=True)
    authorization_code = Column(String)
    action = Column(String)  # 'create', 'update', 'delete', 'sync', 'sync_create', 'sync_update'
    admin_user = Column(String)  # Usuario que realizó la acción
    timestamp = Column(DateTime, default=datetime.utcnow)
    details = Column(String, nullable=True)
    success = Column(Boolean, default=True)


# Modelos de entrada/salida para códigos FAC
class FacCodeBase(BaseModel):
    authorization_code: str = Field(..., min_length=1, max_length=16)
    authorization_code_name: str = Field(..., min_length=1, max_length=50)
    authorization_level: int = Field(..., ge=0, le=255)
    description: Optional[str] = None
    active: bool = True

class FacCodeCreate(FacCodeBase):
    pass

class FacCodeUpdate(BaseModel):
    authorization_code_name: Optional[str] = Field(None, min_length=1, max_length=50)
    authorization_level: Optional[int] = Field(None, ge=0, le=255)
    description: Optional[str] = None
    active: Optional[bool] = None

class FacCodeResponse(FacCodeBase):
    id: int
    created_at: datetime
    updated_at: datetime
    cucm_synced: bool

    class Config:
        orm_mode = True

# Modelos para auditoría
class FacAuditBase(BaseModel):
    authorization_code: str
    action: str
    admin_user: str
    details: Optional[str] = None
    success: bool = True

class FacAuditCreate(FacAuditBase):
    pass

class FacAuditResponse(FacAuditBase):
    id: int
    timestamp: datetime

    class Config:
        orm_mode = True

# Modelo para resultados de sincronización
class SyncResult(BaseModel):
    message: str
    status: str
    created: Optional[int] = None
    updated: Optional[int] = None
    errors: Optional[int] = None

# Dependency para obtener la sesión DB
def get_db():
    db = SessionLocal()
    try:
        yield db
    finally:
        db.close()

class UserAuthCode(Base):
    __tablename__ = "user_auth_codes"
    id = Column(Integer, primary_key=True, index=True)
    extension = Column(String, unique=True, index=True)
    auth_code = Column(String, unique=True)
    auth_level = Column(Integer)
    description = Column(String, nullable=True)
    active = Column(Boolean, default=True)
    created_at = Column(DateTime, default=datetime.utcnow)
    updated_at = Column(DateTime, default=datetime.utcnow, onupdate=datetime.utcnow)


class UserAuthCodeAudit(Base):
    __tablename__ = "user_auth_code_audit"
    id = Column(Integer, primary_key=True, index=True)
    extension = Column(String)
    auth_code = Column(String)
    action = Column(String)  # 'create', 'update', 'delete'
    admin_user = Column(String)  # Usuario administrador que realizó la acción
    timestamp = Column(DateTime, default=datetime.utcnow)
    details = Column(String, nullable=True)

class UserAuthCodeBase(BaseModel):
    extension: str
    auth_code: str
    auth_level: int
    description: Optional[str] = None
    active: bool = True

class UserAuthCodeCreate(UserAuthCodeBase):
    pass

class UserAuthCodeUpdate(BaseModel):
    auth_code: Optional[str] = None
    auth_level: Optional[int] = None
    description: Optional[str] = None
    active: Optional[bool] = None

class UserAuthCodeResponse(UserAuthCodeBase):
    id: int
    created_at: datetime
    updated_at: datetime

    class Config:
        orm_mode = True

from zeep import Client, Settings
from zeep.transports import Transport
from requests import Session
from requests.auth import HTTPBasicAuth
import urllib3
from zeep.exceptions import Fault

# Desactivar advertencias SSL
urllib3.disable_warnings(urllib3.exceptions.InsecureRequestWarning)

# Función para conectar con CUCM
def get_cucm_client():
    """Crea un cliente SOAP para conectarse a CUCM"""
    # Cargar configuración
    CUCM_ADDRESS = os.getenv('CUCM_ADDRESS', '190.105.250.127')
    CUCM_USERNAME = os.getenv('CUCM_USERNAME', 'admin')
    CUCM_PASSWORD = os.getenv('CUCM_PASSWORD', 'fr4v4t3l')
    WSDL_FILE = 'schema/AXLAPI.wsdl'
    
    # Configurar sesión
    session = Session()
    session.verify = False
    session.auth = HTTPBasicAuth(CUCM_USERNAME, CUCM_PASSWORD)
    
    # Configurar transporte
    transport = Transport(session=session, timeout=10)
    settings = Settings(strict=False, xml_huge_tree=True)
    
    # Crear cliente
    client = Client(WSDL_FILE, settings=settings, transport=transport)
    
    # Crear servicio
    service = client.create_service(
        '{http://www.cisco.com/AXLAPIService/}AXLAPIBinding',
        f'https://{CUCM_ADDRESS}:8443/axl/'
    )
    
    return service

# Clase para gestionar códigos FAC
class CucmFacManager:
    """Gestor de códigos de autorización forzada (FAC) para CUCM"""
    
    def __init__(self):
        self.client = get_cucm_client()

    def list_fac_info(self):
        """
        Lista todos los códigos de autorización forzada
        Usando la variante 2 que funciona correctamente
        """
        try:
            print("Llamando a listFacInfo con returnedTags y searchCriteria como keywords")
            response = self.client.listFacInfo(
                searchCriteria={"name": "%"},
                returnedTags={"name": "", "code": "", "authorizationLevel": ""}
            )
            
            # Debug detallado para inspeccionar la respuesta
            print(f"Tipo de respuesta: {type(response)}")
            print(f"Contenido de respuesta: {response}")
            
            # Inspeccionar atributos de la respuesta
            if hasattr(response, '__dict__'):
                print(f"Atributos de respuesta: {dir(response)}")
            
            return response
        except Exception as e:
            print(f"Error en list_fac_info: {e}")
            import traceback
            print(traceback.format_exc())
            return None

    def process_fac_info(response):
        """
        Procesa la respuesta de CUCM y devuelve una lista de códigos FAC
        Adaptado a la estructura específica de la respuesta
        """
        fac_list = []
        print(f"Procesando respuesta de tipo: {type(response)}")
        
        if not response:
            print("Respuesta vacía")
            return fac_list
        
        # Usar getattr para acceder al atributo 'return' que es una palabra reservada
        response_return = getattr(response, 'return', None)
        if not response_return:
            print("No se encontró el atributo 'return' en la respuesta")
            return fac_list
        
        # Acceder a facInfo dentro del atributo return
        fac_info = getattr(response_return, 'facInfo', None)
        if not fac_info:
            print("No se encontró 'facInfo' en la respuesta")
            return fac_list
        
        # Procesar la lista de códigos FAC
        if isinstance(fac_info, list):
            print(f"facInfo es una lista con {len(fac_info)} elementos")
            for fac in fac_info:
                fac_data = {
                    'uuid': fac.get('uuid', ''),
                    'name': fac.get('name', ''),
                    'code': fac.get('code', ''),
                    'level': fac.get('authorizationLevel', 0),
                    'source': 'cucm'
                }
                print(f"Procesado FAC: {fac_data}")
                fac_list.append(fac_data)
        else:
            # Procesar un único elemento
            print("facInfo es un único elemento")
            fac_data = {
                'uuid': getattr(fac_info, 'uuid', ''),
                'name': getattr(fac_info, 'name', ''),
                'code': getattr(fac_info, 'code', ''),
                'level': getattr(fac_info, 'authorizationLevel', 0),
                'source': 'cucm'
            }
            print(f"Procesado FAC: {fac_data}")
            fac_list.append(fac_data)
        
        print(f"Total de FACs procesados: {len(fac_list)}")
        return fac_list

    def add_fac_info(self, code, name, auth_level):
        """
        Añade un nuevo código de autorización forzada
        Adaptado exactamente del ejemplo PHP proporcionado
        """
        try:
            # Crear la estructura exacta del ejemplo PHP
            fac_info = {
                "name": name,
                "code": code,
                "authorizationLevel": auth_level
            }
            
            # Llamada igual al ejemplo PHP: $client->addFacInfo(array("facInfo"=>$facInfo))
            response = self.client.addFacInfo(facInfo=fac_info)
            return True
        except Fault as e:
            print(f"SOAP Fault al añadir FAC: {e}")
            return False
        except Exception as e:
            print(f"Error general al añadir FAC: {e}")
            return False
    
    def update_fac_info(self, code, name=None, auth_level=None):
        """Actualiza un código de autorización existente"""
        try:
            # Crear el objeto para la actualización
            update_data = {}
            
            # Siempre incluir el código (requerido para identificar el FAC)
            update_data["code"] = code
            
            if name is not None:
                update_data["name"] = name
            
            if auth_level is not None:
                update_data["authorizationLevel"] = auth_level
            
            # Estructura similar a addFacInfo
            response = self.client.updateFacInfo(facInfo=update_data)
            return True
        except Exception as e:
            print(f"Error al actualizar FAC {code}: {e}")
            return False
    
    def remove_fac_info(self, code):
        """Elimina un código de autorización forzada"""
        try:
            # Estructura según el patrón establecido
            response = self.client.removeFacInfo(name=code)
            return True
        except Exception as e:
            print(f"Error al eliminar FAC {code}: {e}")
            return False
        

class CucmFacSyncManager:
    """Gestor avanzado para sincronización bidireccional de códigos FAC"""
    
    def __init__(self):
        self.fac_manager = CucmFacManager()
    
    def get_all_cucm_fac_codes(self):
        """Obtiene todos los códigos FAC desde CUCM"""
        try:
            response = self.fac_manager.list_fac_info()
            return process_fac_info(response) if response else []
        except Exception as e:
            logger.error(f"Error obteniendo códigos FAC de CUCM: {e}")
            return []
    
    def get_all_local_fac_codes(self, db):
        """Obtiene todos los códigos FAC de la base de datos local"""
        try:
            return db.query(FacCode).all()
        except Exception as e:
            logger.error(f"Error obteniendo códigos FAC locales: {e}")
            return []
    
    def sync_with_cucm_as_authority(self, admin_username: str = "system"):
        """
        Sincronización completa con CUCM como autoridad principal
        - CUCM es la fuente de verdad
        - Códigos en CUCM pero no en BD → Se crean en BD
        - Códigos en BD pero no en CUCM → Se eliminan de BD
        - Códigos diferentes → Se actualizan en BD según CUCM
        """
        db = SessionLocal()
        
        try:
            logger.info("🔄 Iniciando sincronización con CUCM como autoridad")
            
            # 1. Obtener códigos de ambas fuentes
            cucm_codes = self.get_all_cucm_fac_codes()
            local_codes = self.get_all_local_fac_codes(db)
            
            # Crear diccionarios para comparación
            cucm_dict = {code['code']: code for code in cucm_codes if code.get('code')}
            local_dict = {code.authorization_code: code for code in local_codes}
            
            logger.info(f"📊 CUCM: {len(cucm_dict)} códigos, BD Local: {len(local_dict)} códigos")
            
            stats = {
                "created": 0,
                "updated": 0, 
                "deleted": 0,
                "unchanged": 0,
                "errors": 0
            }
            
            # 2. PROCESAR CÓDIGOS EN CUCM (crear/actualizar en BD)
            for code, cucm_data in cucm_dict.items():
                try:
                    if code in local_dict:
                        # Verificar si necesita actualización
                        local_code = local_dict[code]
                        needs_update = (
                            local_code.authorization_code_name != cucm_data.get('name', '') or
                            local_code.authorization_level != cucm_data.get('level', 0)
                        )
                        
                        if needs_update:
                            # ACTUALIZAR código existente
                            local_code.authorization_code_name = cucm_data.get('name', '')
                            local_code.authorization_level = cucm_data.get('level', 0)
                            local_code.active = True
                            local_code.cucm_synced = True
                            local_code.updated_at = datetime.utcnow()
                            
                            stats["updated"] += 1
                            self._audit_log(db, code, "sync_update_from_cucm", admin_username,
                                          f"Actualizado desde CUCM: {cucm_data.get('name', '')}")
                            logger.info(f"📝 Actualizado: {code}")
                        else:
                            # Marcar como sincronizado sin cambios
                            local_code.cucm_synced = True
                            stats["unchanged"] += 1
                    else:
                        # CREAR nuevo código desde CUCM
                        new_code = FacCode(
                            authorization_code=code,
                            authorization_code_name=cucm_data.get('name', f'FAC_{code}'),
                            authorization_level=cucm_data.get('level', 0),
                            description=f"Sincronizado desde CUCM el {datetime.utcnow().strftime('%Y-%m-%d %H:%M')}",
                            active=True,
                            cucm_synced=True
                        )
                        
                        db.add(new_code)
                        stats["created"] += 1
                        self._audit_log(db, code, "sync_create_from_cucm", admin_username,
                                      f"Creado desde CUCM: {cucm_data.get('name', '')}")
                        logger.info(f"➕ Creado: {code}")
                
                except Exception as e:
                    stats["errors"] += 1
                    self._audit_log(db, code, "sync_error", admin_username, f"Error: {str(e)}", False)
                    logger.error(f"❌ Error procesando {code}: {e}")
            
            # 3. ELIMINAR CÓDIGOS QUE ESTÁN EN BD PERO NO EN CUCM
            for code, local_code in local_dict.items():
                if code not in cucm_dict:
                    try:
                        # ELIMINAR código que no está en CUCM
                        db.delete(local_code)
                        stats["deleted"] += 1
                        self._audit_log(db, code, "sync_delete_not_in_cucm", admin_username,
                                      f"Eliminado - No encontrado en CUCM")
                        logger.info(f"🗑️ Eliminado: {code}")
                    except Exception as e:
                        stats["errors"] += 1
                        self._audit_log(db, code, "sync_delete_error", admin_username, f"Error eliminando: {str(e)}", False)
                        logger.error(f"❌ Error eliminando {code}: {e}")
            
            # 4. CONFIRMAR CAMBIOS
            db.commit()
            
            # 5. REGISTRAR RESUMEN
            summary_msg = f"Sincronización completada - Creados: {stats['created']}, Actualizados: {stats['updated']}, Eliminados: {stats['deleted']}, Sin cambios: {stats['unchanged']}, Errores: {stats['errors']}"
            self._audit_log(db, "SYNC_SUMMARY", "sync_complete", admin_username, summary_msg)
            
            logger.info(f"✅ {summary_msg}")
            
            return {
                "success": True,
                "message": summary_msg,
                "stats": stats
            }
        
        except Exception as e:
            db.rollback()
            error_msg = f"Error general en sincronización: {str(e)}"
            self._audit_log(db, "SYNC_ERROR", "sync_error", admin_username, error_msg, False)
            logger.error(f"❌ {error_msg}")
            
            return {
                "success": False,
                "message": error_msg,
                "stats": {"errors": 1}
            }
        
        finally:
            db.close()
    
    def delete_fac_from_both_systems(self, code: str, admin_username: str):
        """
        Elimina un código FAC tanto del CUCM como de la base de datos local
        """
        db = SessionLocal()
        
        try:
            logger.info(f"🗑️ Eliminando código FAC {code} de ambos sistemas")
            
            # 1. Verificar que existe en BD local
            local_code = db.query(FacCode).filter(FacCode.authorization_code == code).first()
            if not local_code:
                return {
                    "success": False,
                    "message": f"Código {code} no encontrado en la base de datos local"
                }
            
            # 2. Eliminar del CUCM primero
            cucm_success = self.fac_manager.remove_fac_info(code)
            
            # 3. Eliminar de la BD local
            db.delete(local_code)
            
            # 4. Registrar auditoría
            self._audit_log(db, code, "manual_delete_both", admin_username,
                          f"Eliminado de ambos sistemas - CUCM: {'éxito' if cucm_success else 'falló'}")
            
            db.commit()
            
            message = f"Código {code} eliminado exitosamente"
            if not cucm_success:
                message += " (advertencia: falló eliminación en CUCM)"
            
            logger.info(f"✅ {message}")
            
            return {
                "success": True,
                "message": message,
                "cucm_deleted": cucm_success,
                "local_deleted": True
            }
        
        except Exception as e:
            db.rollback()
            error_msg = f"Error eliminando código {code}: {str(e)}"
            self._audit_log(db, code, "delete_error", admin_username, error_msg, False)
            logger.error(f"❌ {error_msg}")
            
            return {
                "success": False,
                "message": error_msg
            }
        
        finally:
            db.close()
    
    def create_fac_in_both_systems(self, fac_data: dict, admin_username: str):
        """
        Crea un código FAC en ambos sistemas (CUCM y BD local)
        """
        db = SessionLocal()
        
        try:
            code = fac_data['authorization_code']
            name = fac_data['authorization_code_name']
            level = fac_data['authorization_level']
            
            logger.info(f"➕ Creando código FAC {code} en ambos sistemas")
            
            # 1. Verificar que no existe en BD local
            existing = db.query(FacCode).filter(FacCode.authorization_code == code).first()
            if existing:
                return {
                    "success": False,
                    "message": f"El código {code} ya existe en la base de datos local"
                }
            
            # 2. Crear en CUCM primero
            cucm_success = self.fac_manager.add_fac_info(code, name, level)
            
            # 3. Crear en BD local
            new_fac = FacCode(
                authorization_code=code,
                authorization_code_name=name,
                authorization_level=level,
                description=fac_data.get('description', ''),
                active=True,
                cucm_synced=cucm_success
            )
            
            db.add(new_fac)
            
            # 4. Registrar auditoría
            self._audit_log(db, code, "manual_create_both", admin_username,
                          f"Creado en ambos sistemas - CUCM: {'éxito' if cucm_success else 'falló'}")
            
            db.commit()
            
            message = f"Código {code} creado exitosamente"
            if not cucm_success:
                message += " (advertencia: falló creación en CUCM)"
            
            logger.info(f"✅ {message}")
            
            return {
                "success": True,
                "message": message,
                "cucm_created": cucm_success,
                "local_created": True,
                "id": new_fac.id
            }
        
        except Exception as e:
            db.rollback()
            error_msg = f"Error creando código {code}: {str(e)}"
            self._audit_log(db, code, "create_error", admin_username, error_msg, False)
            logger.error(f"❌ {error_msg}")
            
            return {
                "success": False,
                "message": error_msg
            }
        
        finally:
            db.close()
    
    def _audit_log(self, db, code: str, action: str, username: str, details: str, success: bool = True):
        """Método auxiliar para registrar auditoría"""
        audit = FacAudit(
            authorization_code=code,
            action=action,
            admin_user=username,
            details=details,
            success=success
        )
        db.add(audit)

# IMPLEMENTACIÓN DE FAC (FORCED AUTHORIZATION CODES)
@app.get("/dashboard/fac")
async def dashboard_fac(request: Request, user=Depends(admin_only)):
    """Dashboard corregido para mostrar códigos FAC con todos los campos"""
    if isinstance(user, RedirectResponse):
        return user
    
    db = SessionLocal()
    
    # 1. Obtener códigos FAC de la base de datos local con TODOS los campos
    local_fac_list = []
    try:
        # ✅ Consulta explícita para asegurar que obtenemos todos los campos
        local_codes = db.query(FacCode).order_by(FacCode.authorization_code).all()
        
        for code in local_codes:
            local_fac_list.append({
                'id': code.id,
                'code': code.authorization_code,
                'name': code.authorization_code_name,
                'level': code.authorization_level,  # ✅ Asegurar que se mapea correctamente
                'description': code.description or '',
                'active': code.active,
                'cucm_synced': code.cucm_synced,
                'source': 'local',
                'created_at': code.created_at.isoformat() if code.created_at else None,
                'updated_at': code.updated_at.isoformat() if code.updated_at else None
            })
            
        logger.info(f"📊 Códigos locales obtenidos: {len(local_fac_list)}")
        
        # ✅ Debug: Imprimir primer código para verificar datos
        if local_fac_list:
            first_code = local_fac_list[0]
            logger.info(f"🔍 Primer código: {first_code['code']} - Nivel: {first_code['level']} - Nombre: {first_code['name']}")
            
    except Exception as e:
        logger.error(f"❌ Error obteniendo códigos FAC locales: {e}")
        import traceback
        traceback.print_exc()
    
    # 2. Intentar obtener códigos del CUCM
    cucm_fac_list = []
    cucm_error = None
    try:
        logger.info("🔄 Intentando conectar con CUCM...")
        sync_manager = CucmFacSyncManager()
        cucm_codes = sync_manager.get_all_cucm_fac_codes()
        
        logger.info(f"📡 Códigos CUCM obtenidos: {len(cucm_codes)}")
        
        # ✅ Procesar códigos de CUCM con verificación de campos
        for cucm_item in cucm_codes:
            if cucm_item and isinstance(cucm_item, dict):
                cucm_fac_list.append({
                    'code': cucm_item.get('code', ''),
                    'name': cucm_item.get('name', ''),
                    'level': cucm_item.get('level', 0),  # ✅ Asegurar valor por defecto
                    'source': 'cucm',
                    'description': f"Desde CUCM - Nivel: {cucm_item.get('level', 0)}"
                })
                
        # ✅ Debug: Imprimir primer código CUCM
        if cucm_fac_list:
            first_cucm = cucm_fac_list[0]
            logger.info(f"🔍 Primer código CUCM: {first_cucm['code']} - Nivel: {first_cucm['level']} - Nombre: {first_cucm['name']}")
            
    except Exception as e:
        cucm_error = str(e)
        logger.error(f"❌ Error conectando con CUCM: {e}")
    
    db.close()
    
    # 3. Combinar listas - priorizar datos locales
    combined_list = local_fac_list.copy()
    local_codes_set = {item['code'] for item in local_fac_list}
    
    # Agregar códigos que solo están en CUCM
    for cucm_item in cucm_fac_list:
        if cucm_item['code'] not in local_codes_set:
            combined_list.append(cucm_item)
    
    # 4. Estadísticas
    stats = {
        "local_count": len(local_fac_list),
        "cucm_count": len(cucm_fac_list),
        "total_count": len(combined_list),
        "cucm_error": cucm_error
    }
    
    # ✅ Debug: Imprimir estadísticas finales
    logger.info(f"📊 Estadísticas finales: {stats}")
    logger.info(f"📋 Total códigos a mostrar: {len(combined_list)}")
    
    return templates.TemplateResponse("dashboard_fac.html", {
        "request": request, 
        "fac_list": combined_list, 
        "user": user,
        **stats
    })


@app.get("/api/fac")
async def get_fac_list(user=Depends(admin_only)):
    """API para obtener todos los códigos FAC"""
    if isinstance(user, RedirectResponse):
        return user
    
    fac_manager = CucmFacManager()
    response = fac_manager.list_fac_info()
    
    # Procesar resultados
    fac_list = process_fac_info(response)

    return {"fac_codes": fac_list}


def process_fac_info(response):
    """
    Procesa la respuesta de CUCM y devuelve una lista de códigos FAC
    """
    fac_list = []

    if not response or not hasattr(response, 'return'):
        return fac_list

    result_data = getattr(response, 'return', None)

    if not hasattr(result_data, 'facInfo'):
        return fac_list

    fac_info = result_data.facInfo

    # Si hay múltiples resultados, es una lista
    if isinstance(fac_info, list):
        for fac in fac_info:
            fac_list.append({
                "uuid": getattr(fac, 'uuid', None),
                "name": getattr(fac, 'name', None),
                "code": getattr(fac, 'code', None),
                "level": getattr(fac, 'authorizationLevel', None)
            })
    else:
        # Solo un resultado
        fac_list.append({
            "uuid": getattr(fac_info, 'uuid', None),
            "name": getattr(fac_info, 'name', None),
            "code": getattr(fac_info, 'code', None),
            "level": getattr(fac_info, 'authorizationLevel', None)
        })

    return fac_list

@app.post("/api/fac")
async def create_fac(
    fac: FacCodeCreate,
    user: dict = Depends(admin_only)  # Asegúrate de que esto devuelva un usuario autenticado
):
    """API para crear un nuevo código FAC"""
    #if isinstance(user, RedirectResponse):
    #    return user
    
    # Instanciar el gestor
    fac_manager = CucmFacManager()
    
    # Registrar en la base de datos local
    db = SessionLocal()
    try:
        # Verificar si ya existe
        existing = db.query(FacCode).filter(FacCode.authorization_code == fac.authorization_code).first()
        if existing:
            db.close()
            raise HTTPException(status_code=400, detail="El código de autorización ya existe")
        
        # Crear nuevo código FAC en la BD local
        db_fac = FacCode(
            authorization_code=fac.authorization_code,
            authorization_code_name=fac.authorization_code_name,
            authorization_level=fac.authorization_level,
            description="",
            active=True,
            cucm_synced=False
        )
        db.add(db_fac)
        
        # Registrar en auditoría
        audit = FacAudit(
            authorization_code=fac.authorization_code,
            action="create",
            admin_user=user["username"],
            details="Creación manual desde dashboard",
            success=True
        )
        db.add(audit)
        db.commit()
        
        # Intentar crear en CUCM
        success = fac_manager.add_fac_info(fac.authorization_code, fac.authorization_code_name, fac.authorization_level)
        
        if success:
            # Actualizar estado de sincronización
            db_fac.cucm_synced = True
            
            # Registrar en auditoría
            audit = FacAudit(
                authorization_code=fac.authorization_code,
                action="sync_create",
                admin_user=user["username"],
                details="Creado automáticamente en CUCM",
                success=True
            )
            db.add(audit)
            db.commit()
        
        db.close()
        return {"success": True, "message": "Código FAC creado exitosamente", "synced": success}
    except HTTPException:
        db.close()
        raise
    except Exception as e:
        db.rollback()
        db.close()
        raise HTTPException(status_code=500, detail=f"Error al crear código FAC: {str(e)}")

@app.put("/api/fac/{code}")
async def update_fac(
    code: str,
    name: Optional[str] = Form(None),
    auth_level: Optional[int] = Form(None),
    description: Optional[str] = Form(None),
    active: Optional[bool] = Form(None),
    user=Depends(admin_only)
):
    """API para actualizar un código FAC existente"""
    if isinstance(user, RedirectResponse):
        return user
    
    db = SessionLocal()
    try:
        # Buscar el código en la BD local
        fac = db.query(FacCode).filter(FacCode.authorization_code == code).first()
        if not fac:
            db.close()
            raise HTTPException(status_code=404, detail="Código FAC no encontrado")
        
        # Actualizar campos proporcionados
        if name is not None:
            fac.authorization_code_name = name
        
        if auth_level is not None:
            fac.authorization_level = auth_level
            
        if description is not None:
            fac.description = description
            
        if active is not None:
            fac.active = active
        
        # Marcar como no sincronizado
        fac.cucm_synced = False
        
        # Registrar en auditoría
        audit = FacAudit(
            authorization_code=code,
            action="update",
            admin_user=user["username"],
            details="Actualización manual desde dashboard",
            success=True
        )
        db.add(audit)
        db.commit()
        
        # Intentar actualizar en CUCM si está activo
        if fac.active:
            fac_manager = CucmFacManager()
            success = fac_manager.update_fac_info(code, name, auth_level)
            
            if success:
                # Actualizar estado de sincronización
                fac.cucm_synced = True
                
                # Registrar en auditoría
                audit = FacAudit(
                    authorization_code=code,
                    action="sync_update",
                    admin_user=user["username"],
                    details="Actualizado automáticamente en CUCM",
                    success=True
                )
                db.add(audit)
                db.commit()
        
        db.close()
        return {"success": True, "message": f"Código FAC {code} actualizado exitosamente"}
    except HTTPException:
        db.close()
        raise
    except Exception as e:
        db.rollback()
        db.close()
        raise HTTPException(status_code=500, detail=f"Error al actualizar código FAC: {str(e)}")

@app.delete("/api/fac/{fac_id}")
async def delete_fac(fac_id: int, user=Depends(admin_only)):
    """API para eliminar un código FAC por ID"""
    if isinstance(user, RedirectResponse):
        return user
    
    db = SessionLocal()
    try:
        # Buscar el código por ID en la BD local
        fac = db.query(FacCode).filter(FacCode.id == fac_id).first()
        if not fac:
            db.close()
            raise HTTPException(status_code=404, detail="Código FAC no encontrado")
        
        code = fac.authorization_code_name  # Guardar el código para usar en CUCM
        
        # Eliminar de CUCM primero
        fac_manager = CucmFacManager()
        cucm_success = fac_manager.remove_fac_info(code)
        
        # Eliminar de la BD local
        db.delete(fac)
        
        # Registrar en auditoría
        audit = FacAudit(
            authorization_code=code,
            action="delete",
            admin_user=user["username"],
            details=f"Eliminación manual desde dashboard (CUCM: {'éxito' if cucm_success else 'fallido'})",
            success=True
        )
        db.add(audit)
        db.commit()
        
        db.close()
        return {"success": True, "message": f"Código FAC {code} eliminado exitosamente"}
    except HTTPException:
        db.close()
        raise
    except Exception as e:
        db.rollback()
        db.close()
        raise HTTPException(status_code=500, detail=f"Error al eliminar código FAC: {str(e)}")
        
@app.get("/api/fac/test-connection")
async def test_fac_connection(user=Depends(admin_only)):
    """API para probar la conexión con CUCM y listar códigos FAC"""
    if isinstance(user, RedirectResponse):
        return user
    
    try:
        # Probar la conexión utilizando el mismo patrón que el ejemplo PHP
        client = get_cucm_client()
        
        # Construir la solicitud exactamente como en PHP
        returned_tags = {
            "name": "",
            "code": "",
            "authorizationLevel": ""
        }
        
        search_criteria = {
            "name": "%"  # Wildcard para encontrar todos los FAC
        }
        
        try:
            # Llamada equivalente a la del ejemplo PHP
            response = client.listFacInfo(
                returnedTags=returned_tags,
                searchCriteria=search_criteria
            )
            
            # Analizar la estructura de la respuesta para depuración
            response_info = {
                "type": type(response).__name__,
                "has_return_": hasattr(response, "return_")
            }
            
            if hasattr(response, "return_"):
                return_obj = response.return_
                response_info["return_type"] = type(return_obj).__name__
                response_info["has_facInfo"] = hasattr(return_obj, "facInfo")
                
                if hasattr(return_obj, "facInfo"):
                    fac_info = return_obj.facInfo
                    response_info["facInfo_type"] = type(fac_info).__name__
                    response_info["facInfo_is_list"] = isinstance(fac_info, list)
                    
                    if isinstance(fac_info, list):
                        response_info["facInfo_count"] = len(fac_info)
                        if len(fac_info) > 0:
                            sample = fac_info[0]
                            response_info["sample_fac"] = {
                                "has_uuid": hasattr(sample, "uuid"),
                                "has_name": hasattr(sample, "name"),
                                "has_code": hasattr(sample, "code"),
                                "has_authLevel": hasattr(sample, "authorizationLevel")
                            }
                    else:
                        response_info["sample_fac"] = {
                            "has_uuid": hasattr(fac_info, "uuid"),
                            "has_name": hasattr(fac_info, "name"),
                            "has_code": hasattr(fac_info, "code"),
                            "has_authLevel": hasattr(fac_info, "authorizationLevel")
                        }
            
            return {
                "success": True, 
                "message": "Conexión exitosa con CUCM",
                "response_info": response_info
            }
            
        except Fault as e:
            # Error SOAP específico
            return {
                "success": False,
                "message": f"Error de SOAP: {str(e)}",
                "error_type": "soap_fault"
            }
            
    except Exception as e:
        # Error general de conexión
        return {
            "success": False,
            "message": f"Error de conexión: {str(e)}",
            "error_type": "connection_error"
        }


@app.get("/api/fac/raw-test")
async def test_fac_raw(user=Depends(admin_only)):
    """Prueba directa con SOAP crudo para diagnosticar el problema"""
    if isinstance(user, RedirectResponse):
        return user
    
    try:
        # Configuración
        cucm_address = "190.105.250.127"
        cucm_username = "admin" 
        cucm_password = "fr4v4t3l"
        
        # URL del servicio AXL
        url = f"https://{cucm_address}:8443/axl/"
        
        # Encabezados SOAP
        headers = {
            "Content-Type": "text/xml; charset=utf-8",
            "SOAPAction": "CUCM:DB ver=12.5 listFacInfo"
        }
        
        # Cuerpo SOAP con la estructura exacta basada en el ejemplo PHP
        body = """
        <soapenv:Envelope xmlns:soapenv="http://schemas.xmlsoap.org/soap/envelope/" 
                         xmlns:ns="http://www.cisco.com/AXL/API/12.5">
           <soapenv:Header/>
           <soapenv:Body>
              <ns:listFacInfo>
                 <searchCriteria>
                    <name>%</name>
                 </searchCriteria>
                 <returnedTags>
                    <name/>
                    <code/>
                    <authorizationLevel/>
                 </returnedTags>
              </ns:listFacInfo>
           </soapenv:Body>
        </soapenv:Envelope>
        """
        
        # Realizar la solicitud
        session = Session()
        session.verify = False
        session.auth = HTTPBasicAuth(cucm_username, cucm_password)
        
        response = session.post(url, headers=headers, data=body)
        
        # Imprimir respuesta
        response_text = response.text
        
        return {
            "status_code": response.status_code,
            "response": response_text
        }
    except Exception as e:
        return {"error": str(e)}


# Sincronización con CUCM
@app.post("/api/fac/sync-with-cucm")
async def sync_fac_with_cucm(
    background_tasks: BackgroundTasks, 
    db = Depends(lambda: SessionLocal()), 
    user=Depends(admin_only)
):
    """Inicia la sincronización de FAC con CUCM en segundo plano"""
    if isinstance(user, RedirectResponse):
        return user
        
    try:
        # Obtener nombre de usuario
        username = user["username"] if isinstance(user, dict) and "username" in user else "sistema"
        
        # Iniciar tarea en segundo plano
        background_tasks.add_task(sync_all_fac_with_cucm, username, db)
        
        return {"message": "Sincronización iniciada en segundo plano", "status": "success"}
    except Exception as e:
        import traceback
        print(f"Error al iniciar sincronización: {str(e)}")
        print(traceback.format_exc())
        raise HTTPException(status_code=500, detail=f"Error al iniciar sincronización: {str(e)}")

# Función de sincronización que se ejecuta en segundo plano
def sync_all_fac_with_cucm(admin_username: str, db: SessionLocal):
    """Sincroniza todos los códigos FAC con CUCM"""
    try:
        # Obtener códigos FAC desde la base de datos
        fac_codes = db.query(FacCode).filter(FacCode.active == True).all()
        
        # Conectar con CUCM
        fac_manager = CucmFacManager()
        
        # Obtener códigos existentes en CUCM
        cucm_codes = {}
        try:
            # Listar códigos FAC existentes en CUCM
            response = fac_manager.listFacInfo()
            
            # Procesar respuesta para obtener códigos existentes
            if hasattr(response, 'return_') and hasattr(response.return_, 'facInfo'):
                fac_info = response.return_.facInfo
                
                # Si es una lista
                if isinstance(fac_info, list):
                    for fac in fac_info:
                        if hasattr(fac, 'code') and hasattr(fac, 'name'):
                            cucm_codes[fac.code] = {
                                'name': fac.name,
                                'level': fac.authorizationLevel if hasattr(fac, 'authorizationLevel') else None
                            }
                # Si es un solo objeto
                elif hasattr(fac_info, 'code') and hasattr(fac_info, 'name'):
                    cucm_codes[fac_info.code] = {
                        'name': fac_info.name,
                        'level': fac_info.authorizationLevel if hasattr(fac_info, 'authorizationLevel') else None
                    }
        except Exception as e:
            # Registrar error al obtener códigos existentes
            audit_entry = FacAudit(
                authorization_code="N/A",
                action="sync",
                admin_user=admin_username,
                details=f"Error obteniendo códigos de CUCM: {str(e)}",
                success=False
            )
            db.add(audit_entry)
            db.commit()
            print(f"Error obteniendo códigos de CUCM: {str(e)}")
        
        # Sincronizar códigos
        created_count = 0
        updated_count = 0
        error_count = 0
        
        for code in fac_codes:
            try:
                if code.authorization_code in cucm_codes:
                    # Verificar si necesita actualización
                    cucm_code = cucm_codes[code.authorization_code]
                    needs_update = (
                        code.authorization_code_name != cucm_code['name'] or
                        code.authorization_level != cucm_code['level']
                    )
                    
                    if needs_update:
                        # Actualizar código existente usando el método corregido
                        success = fac_manager.update_fac_info(
                            code.authorization_code,
                            code.authorization_code_name,
                            code.authorization_level
                        )
                        
                        if success:
                            # Registrar auditoría
                            audit_entry = FacAudit(
                                authorization_code=code.authorization_code,
                                action="sync_update",
                                admin_user=admin_username,
                                details=f"Código actualizado en CUCM",
                                success=True
                            )
                            db.add(audit_entry)
                            updated_count += 1
                            
                            # Marcar como sincronizado
                            code.cucm_synced = True
                            db.commit()
                        else:
                            raise Exception("Fallo al actualizar en CUCM")
                else:
                    # Crear nuevo código usando el método corregido
                    success = fac_manager.add_fac_info(
                        code.authorization_code,
                        code.authorization_code_name,
                        code.authorization_level
                    )
                    
                    if success:
                        # Registrar auditoría
                        audit_entry = FacAudit(
                            authorization_code=code.authorization_code,
                            action="sync_create",
                            admin_user=admin_username,
                            details=f"Código creado en CUCM",
                            success=True
                        )
                        db.add(audit_entry)
                        created_count += 1
                        
                        # Marcar como sincronizado
                        code.cucm_synced = True
                        db.commit()
                    else:
                        raise Exception("Fallo al crear en CUCM")
                
            except Exception as e:
                # Registrar error
                error_count += 1
                audit_entry = FacAudit(
                    authorization_code=code.authorization_code,
                    action="sync",
                    admin_user=admin_username,
                    details=f"Error de sincronización: {str(e)}",
                    success=False
                )
                db.add(audit_entry)
                db.commit()
                print(f"Error sincronizando código {code.authorization_code}: {str(e)}")
                
        # Verificar si hay códigos en CUCM que no están en nuestro sistema
        orphan_count = 0
        for cucm_code in cucm_codes:
            if not db.query(FacCode).filter(FacCode.authorization_code == cucm_code).first():
                orphan_count += 1
                audit_entry = FacAudit(
                    authorization_code=cucm_code,
                    action="sync_detect",
                    admin_user=admin_username,
                    details=f"Código encontrado en CUCM pero no en sistema local",
                    success=True
                )
                db.add(audit_entry)
                db.commit()
        
        # Registrar resumen final
        summary_entry = FacAudit(
            authorization_code="SUMMARY",
            action="sync_complete",
            admin_user=admin_username,
            details=f"Sincronización completada. Creados: {created_count}, Actualizados: {updated_count}, Errores: {error_count}, Huérfanos detectados: {orphan_count}",
            success=True
        )
        db.add(summary_entry)
        db.commit()
        print(f"Sincronización completada. Creados: {created_count}, Actualizados: {updated_count}, Errores: {error_count}, Huérfanos: {orphan_count}")
                
    except Exception as e:
        # Registrar error general
        audit_entry = FacAudit(
            authorization_code="N/A",
            action="sync",
            admin_user=admin_username,
            details=f"Error general de sincronización: {str(e)}",
            success=False
        )
        db.add(audit_entry)
        db.commit()
        print(f"Error general en sincronización: {str(e)}")


@app.get("/dashboard/fac/historial")
async def fac_historial(
    request: Request, 
    limit: int = Query(100, ge=10, le=500),
    user=Depends(admin_only)
):
    """Muestra el historial de cambios y sincronización de FAC"""
    if isinstance(user, RedirectResponse):
        return user
        
    db = SessionLocal()
    
    # Obtener registros de auditoría
    try:
        # Consultar registros de auditoría ordenados por fecha descendente
        audit_logs = db.query(FacAudit).order_by(FacAudit.timestamp.desc()).limit(limit).all()
        
        # Preparar estadísticas resumidas
        stats = {
            "total_records": len(audit_logs),
            "success_count": sum(1 for log in audit_logs if log.success),
            "error_count": sum(1 for log in audit_logs if not log.success),
            "by_action": {}
        }
        
        # Contar registros por tipo de acción
        action_counts = {}
        for log in audit_logs:
            action = log.action
            if action not in action_counts:
                action_counts[action] = 0
            action_counts[action] += 1
        
        stats["by_action"] = action_counts
        
    except Exception as e:
        print(f"Error al consultar registros de auditoría: {e}")
        audit_logs = []
        stats = {
            "error": str(e),
            "total_records": 0
        }
    
    db.close()
    
    return templates.TemplateResponse("dashboard_fac_historial.html", {
        "request": request,
        "audit_logs": audit_logs,
        "stats": stats,
        "user": user,
        "limit": limit
    })

@app.get("/dashboard/fac/sync")
async def dashboard_fac_sync(request: Request, user=Depends(admin_only)):
    """Dashboard para sincronización de FAC con CUCM"""
    if isinstance(user, RedirectResponse):
        return user
        
    db = SessionLocal()
    
    # Obtener estadísticas de sincronización
    stats = {
        "total_local": db.query(FacCode).count(),
        "total_synced": db.query(FacCode).filter(FacCode.cucm_synced == True).count()
    }
    
    # Obtener últimos eventos de auditoría
    audit_logs = db.query(FacAudit).order_by(FacAudit.timestamp.desc()).limit(20).all()
    
    # Obtener última sincronización completa
    last_sync = db.query(FacAudit).filter(
        FacAudit.action == "sync_complete"
    ).order_by(FacAudit.timestamp.desc()).first()
    
    db.close()
    
    return templates.TemplateResponse("dashboard_fac_sync.html", {
        "request": request,
        "stats": stats,
        "audit_logs": audit_logs,
        "last_sync": last_sync,
        "user": user
    })

@app.post("/api/fac/sync-from-cucm")
async def sync_fac_from_cucm(background_tasks: BackgroundTasks, user=Depends(admin_only)):
    """Importar códigos FAC desde CUCM a la base de datos local"""
    if isinstance(user, RedirectResponse):
        return user
        
    background_tasks.add_task(import_fac_from_cucm, user["username"])
    
    return {"message": "Importación iniciada en segundo plano", "status": "success"}

def sync_fac_from_cucm_to_database(admin_username: str = "system"):
    """
    NUEVA FUNCIÓN: Sincroniza códigos FAC desde CUCM hacia la base de datos
    Esta función SÍ inserta los códigos que faltan
    """
    db = SessionLocal()
    
    try:
        # 1. Obtener códigos de CUCM usando tu función existente
        fac_manager = CucmFacManager()
        response = fac_manager.list_fac_info()
        
        # Usar tu función process_fac_info para procesar la respuesta
        cucm_fac_list = process_fac_info(response)
        logger.info(f"Obtenidos {len(cucm_fac_list)} códigos de CUCM")
        
        if not cucm_fac_list:
            logger.warning("No se obtuvieron códigos de CUCM")
            return {
                "success": False,
                "message": "No se pudieron obtener códigos de CUCM",
                "stats": {"created": 0, "updated": 0, "errors": 1}
            }
        
        # 2. Obtener códigos existentes en la base de datos
        existing_codes = {fac.authorization_code: fac for fac in db.query(FacCode).all()}
        logger.info(f"Códigos existentes en BD: {len(existing_codes)}")
        
        # 3. Estadísticas de sincronización
        stats = {
            "created": 0,
            "updated": 0,
            "deactivated": 0,
            "errors": 0,
            "cucm_total": len(cucm_fac_list),
            "local_total": len(existing_codes)
        }
        
        # 4. SINCRONIZAR: Para cada código en CUCM
        for cucm_fac in cucm_fac_list:
            code = cucm_fac['code']
            
            if not code:  # Saltar códigos sin código válido
                continue
                
            try:
                if code in existing_codes:
                    # ACTUALIZAR código existente si hay diferencias
                    local_fac = existing_codes[code]
                    updated = False
                    
                    # Verificar si necesita actualización
                    if local_fac.authorization_code_name != cucm_fac['name']:
                        logger.info(f"Actualizando nombre para {code}: '{local_fac.authorization_code_name}' → '{cucm_fac['name']}'")
                        local_fac.authorization_code_name = cucm_fac['name']
                        updated = True
                    
                    if local_fac.authorization_level != cucm_fac['level']:
                        logger.info(f"Actualizando nivel para {code}: {local_fac.authorization_level} → {cucm_fac['level']}")
                        local_fac.authorization_level = cucm_fac['level']
                        updated = True
                    
                    # Si está en CUCM, debe estar activo
                    if not local_fac.active:
                        logger.info(f"Reactivando código {code}")
                        local_fac.active = True
                        updated = True
                    
                    if updated:
                        local_fac.cucm_synced = True
                        local_fac.updated_at = datetime.utcnow()
                        stats["updated"] += 1
                        
                        # Registrar auditoría
                        audit = FacAudit(
                            authorization_code=code,
                            action="sync_update_from_cucm",
                            admin_user=admin_username,
                            details=f"Actualizado desde CUCM: {cucm_fac['name']} (Nivel: {cucm_fac['level']})",
                            success=True
                        )
                        db.add(audit)
                    else:
                        # Marcar como sincronizado aunque no haya cambios
                        if not local_fac.cucm_synced:
                            local_fac.cucm_synced = True
                            local_fac.updated_at = datetime.utcnow()
                else:
                    # CREAR nuevo código que está en CUCM pero no en BD
                    logger.info(f"CREANDO nuevo código desde CUCM: {code} - {cucm_fac['name']}")
                    
                    new_fac = FacCode(
                        authorization_code=code,
                        authorization_code_name=cucm_fac['name'] or f"FAC_{code}",
                        authorization_level=cucm_fac['level'] or 0,
                        description=f"Sincronizado automáticamente desde CUCM el {datetime.utcnow().strftime('%Y-%m-%d %H:%M')}",
                        active=True,
                        cucm_synced=True
                    )
                    
                    db.add(new_fac)
                    stats["created"] += 1
                    
                    # Registrar auditoría
                    audit = FacAudit(
                        authorization_code=code,
                        action="sync_create_from_cucm",
                        admin_user=admin_username,
                        details=f"Creado automáticamente desde CUCM: {cucm_fac['name']} (Nivel: {cucm_fac['level']})",
                        success=True
                    )
                    db.add(audit)
                    
            except Exception as e:
                logger.error(f"Error sincronizando código {code}: {e}")
                stats["errors"] += 1
                
                # Registrar error en auditoría
                audit = FacAudit(
                    authorization_code=code,
                    action="sync_error",
                    admin_user=admin_username,
                    details=f"Error sincronizando desde CUCM: {str(e)}",
                    success=False
                )
                db.add(audit)
                continue
        
        # 5. Desactivar códigos que están en BD pero NO en CUCM
        cucm_codes_set = {fac['code'] for fac in cucm_fac_list if fac['code']}
        for code, local_fac in existing_codes.items():
            if code not in cucm_codes_set and local_fac.active:
                logger.info(f"Desactivando código {code} (no está en CUCM)")
                local_fac.active = False
                local_fac.cucm_synced = False
                local_fac.description = f"DESACTIVADO - No encontrado en CUCM (Sync: {datetime.utcnow().strftime('%Y-%m-%d %H:%M')})"
                local_fac.updated_at = datetime.utcnow()
                stats["deactivated"] += 1
                
                # Auditoría
                audit = FacAudit(
                    authorization_code=code,
                    action="sync_deactivate",
                    admin_user=admin_username,
                    details="Desactivado - No encontrado en CUCM",
                    success=True
                )
                db.add(audit)
        
        # 6. Registrar resumen final
        summary = FacAudit(
            authorization_code="SYNC_SUMMARY",
            action="sync_complete",
            admin_user=admin_username,
            details=f"Sincronización completada - CUCM: {stats['cucm_total']}, BD Local: {stats['local_total']}, Creados: {stats['created']}, Actualizados: {stats['updated']}, Desactivados: {stats['deactivated']}, Errores: {stats['errors']}",
            success=True
        )
        db.add(summary)
        
        # Confirmar transacción
        db.commit()
        
        logger.info(f"Sincronización completada: {stats}")
        
        return {
            "success": True,
            "message": f"Sincronización completada. Creados: {stats['created']}, Actualizados: {stats['updated']}, Desactivados: {stats['deactivated']}",
            "stats": stats
        }
        
    except Exception as e:
        db.rollback()
        logger.error(f"Error en sincronización: {e}")
        
        # Registrar error general
        error_audit = FacAudit(
            authorization_code="SYNC_ERROR",
            action="sync_error",
            admin_user=admin_username,
            details=f"Error general en sincronización: {str(e)}",
            success=False
        )
        db.add(error_audit)
        db.commit()
        
        return {
            "success": False,
            "message": f"Error en sincronización: {str(e)}",
            "stats": {"errors": 1}
        }
        
    finally:
        db.close()

@app.post("/api/fac/sync-from-cucm-manual")
async def sync_fac_manual(background_tasks: BackgroundTasks, user=Depends(admin_only)):
    """Endpoint para ejecutar sincronización manual desde el dashboard"""
    if isinstance(user, RedirectResponse):
        return user
    
    # Ejecutar sincronización en background
    background_tasks.add_task(run_sync_with_logging, user["username"])
    
    return {
        "success": True,
        "message": "Sincronización iniciada. Los códigos de CUCM se insertarán automáticamente en la BD.",
        "action": "sync_started"
    }

def run_sync_with_logging(username: str):
    """Ejecuta la sincronización con logging detallado"""
    logger.info(f"Iniciando sincronización manual por usuario: {username}")
    result = sync_fac_from_cucm_to_database(username)
    logger.info(f"Sincronización completada: {result}")

def import_fac_from_cucm(admin_username: str):
    """Importa códigos FAC desde CUCM a la base de datos local"""
    db = SessionLocal()
    
    try:
        # Conectar con CUCM
        client = get_cucm_client()

        # Listar códigos existentes en CUCM
        returned_tags = {
            "name": "",
            "code": "",
            "authorizationLevel": ""
        }
        
        search_criteria = {
            "name": "%"  # Wildcard para encontrar todos los FAC
        }
        
        # Listar códigos FAC existentes en CUCM
        response = client.listFacInfo(
            returnedTags=returned_tags,
            searchCriteria=search_criteria
        )
        
        # Procesar respuesta para obtener códigos existentes
        created_count = 0
        updated_count = 0
        unchanged_count = 0
        
        if hasattr(response, 'return_') and hasattr(response.return_, 'facInfo'):
            fac_info = response.return_.facInfo
            
            # Procesar lista de códigos
            fac_list = []
            if isinstance(fac_info, list):
                fac_list = fac_info
            else:
                fac_list = [fac_info]
            
            # Importar cada código
            for fac in fac_list:
                if hasattr(fac, 'code') and hasattr(fac, 'name'):
                    code = fac.code
                    name = fac.name
                    level = fac.authorizationLevel if hasattr(fac, 'authorizationLevel') else 0
                    
                    # Verificar si el código ya existe
                    existing = db.query(FacCode).filter(FacCode.authorization_code == code).first()
                    
                    if existing:
                        # Verificar si necesita actualización
                        if (existing.authorization_code_name != name or 
                            existing.authorization_level != level):
                            
                            # Actualizar código existente
                            existing.authorization_code_name = name
                            existing.authorization_level = level
                            existing.cucm_synced = True
                            existing.updated_at = datetime.utcnow()
                            
                            # Registrar auditoría
                            audit_entry = FacAudit(
                                authorization_code=code,
                                action="import_update",
                                admin_user=admin_username,
                                details=f"Código actualizado desde CUCM",
                                success=True
                            )
                            db.add(audit_entry)
                            updated_count += 1
                        else:
                            # No necesita actualización
                            existing.cucm_synced = True
                            unchanged_count += 1
                    else:
                        # Crear nuevo código
                        new_fac = FacCode(
                            authorization_code=code,
                            authorization_code_name=name,
                            authorization_level=level,
                            description=f"Importado desde CUCM el {datetime.utcnow().strftime('%Y-%m-%d %H:%M')}",
                            active=True,
                            cucm_synced=True
                        )
                        
                        db.add(new_fac)
                        
                        # Registrar auditoría
                        audit_entry = FacAudit(
                            authorization_code=code,
                            action="import_create",
                            admin_user=admin_username,
                            details=f"Código importado desde CUCM",
                            success=True
                        )
                        db.add(audit_entry)
                        created_count += 1
            
            # Registrar resumen
            summary_entry = FacAudit(
                authorization_code="SUMMARY",
                action="import_complete",
                admin_user=admin_username,
                details=f"Importación completada. Creados: {created_count}, Actualizados: {updated_count}, Sin cambios: {unchanged_count}",
                success=True
            )
            db.add(summary_entry)
            
            db.commit()
            print(f"Importación completada. Creados: {created_count}, Actualizados: {updated_count}, Sin cambios: {unchanged_count}")
        
    except Exception as e:
        # Registrar error
        audit_entry = FacAudit(
            authorization_code="N/A",
            action="import",
            admin_user=admin_username,
            details=f"Error de importación: {str(e)}",
            success=False
        )
        db.add(audit_entry)
        db.commit()
        print(f"Error importando códigos FAC desde CUCM: {str(e)}")
    
    finally:
        db.close()

def is_internal_extension(number: str) -> bool:
    """Determina si un número es un anexo interno"""
    if not number:
        return False
    
    clean_number = ''.join(filter(str.isdigit, str(number)))
    
    if len(clean_number) == 4 and clean_number[0] in ['3', '4', '5']:
        return True
    
    try:
        num = int(clean_number)
        if 3000 <= num <= 5999:
            return True
    except:
        pass
    
    return False

@app.post("/test-cdr-validation")
async def test_cdr_validation(request: Request):
    """
    Endpoint para probar si los datos de Java se pueden procesar correctamente
    NO guarda nada en la base de datos, solo valida el modelo
    """
    try:
        # Obtener datos raw
        raw_data = await request.json()
        print("=" * 50)
        print("🧪 TESTING CDR VALIDATION (Pydantic V2)")
        print("=" * 50)
        print(f"📦 Datos recibidos:")
        print(json.dumps(raw_data, indent=2))
        
        # Intentar crear CallEvent
        try:
            event = CallEvent.model_validate(raw_data)  # Pydantic V2 syntax
            print(f"\n✅ Validación EXITOSA!")
            print(f"   Calling: {event.calling_number}")
            print(f"   Called: {event.called_number}")
            print(f"   Direction: {event.direction}")
            print(f"   Start time: {event.start_time}")
            print(f"   End time: {event.end_time}")
            print(f"   Answer time: {event.answer_time}")
            print(f"   Duration billable: {event.duration_billable}")
            
            # Detectar tipo de llamada
            calling_is_internal = is_internal_extension(event.calling_number)
            called_is_internal = is_internal_extension(event.called_number)
            
            if calling_is_internal and not called_is_internal:
                detected_direction = "outbound"
            elif not calling_is_internal and called_is_internal:
                detected_direction = "inbound"
            elif calling_is_internal and called_is_internal:
                detected_direction = "internal"
            else:
                detected_direction = "unknown"
            
            print(f"   Direction detectada: {detected_direction}")
            
            return {
                "status": "SUCCESS",
                "message": "Modelo validado correctamente con Pydantic V2",
                "parsed_data": {
                    "calling_number": event.calling_number,
                    "called_number": event.called_number,
                    "direction_original": event.direction,
                    "direction_detectada": detected_direction,
                    "start_time": event.start_time.isoformat() if event.start_time else None,
                    "end_time": event.end_time.isoformat() if event.end_time else None,
                    "answer_time": event.answer_time.isoformat() if event.answer_time else None,
                    "duration_billable": event.duration_billable,
                    "calling_is_internal": calling_is_internal,
                    "called_is_internal": called_is_internal
                }
            }
            
        except ValidationError as e:
            print(f"\n❌ Error en validación Pydantic V2:")
            errors = []
            for error in e.errors():
                error_detail = {
                    "field": " -> ".join(str(loc) for loc in error['loc']),
                    "message": error['msg'],
                    "type": error['type'],
                    "input": error.get('input', 'N/A')
                }
                errors.append(error_detail)
                print(f"   Campo: {error_detail['field']}")
                print(f"   Error: {error_detail['message']}")
                print(f"   Tipo: {error_detail['type']}")
                print(f"   Valor: {error_detail['input']}")
            
            return {
                "status": "VALIDATION_ERROR",
                "message": "Error de validación con Pydantic V2",
                "errors": errors,
                "raw_data": raw_data
            }
            
        except Exception as e:
            print(f"\n❌ Error inesperado: {e}")
            return {
                "status": "UNEXPECTED_ERROR",
                "error": str(e),
                "raw_data": raw_data
            }
        
    except Exception as e:
        print(f"❌ Error general: {e}")
        return {
            "status": "ERROR",
            "error": str(e)
        }

@app.post("/test-with-your-data-v2")
async def test_with_your_data_v2():
    """Prueba con los datos exactos de tu log usando Pydantic V2"""
    
    # Datos exactos de tu log
    test_data = {
        "answer_time": "2025-05-31T21:11:19.351733420Z",
        "duration_seconds": 16,
        "calling_number": "983434724",
        "duration_billable": 7,
        "end_time": "2025-05-31T21:11:26.924605727Z",
        "network_alerting_time": None,
        "network_reached_time": None,
        "start_time": "2025-05-31T21:11:10.543149610Z",
        "called_number": "4198",
        "dialing_time": None,
        "release_cause": 0,
        "status": "disconnected",
        "direction": "unknown"
    }
    
    try:
        print("🧪 Probando con datos de tu log usando Pydantic V2...")
        event = CallEvent.model_validate(test_data)  # Pydantic V2 syntax
        
        # Analizar la llamada
        calling_is_internal = is_internal_extension(event.calling_number)
        called_is_internal = is_internal_extension(event.called_number)
        
        if calling_is_internal and not called_is_internal:
            detected_direction = "outbound"
        elif not calling_is_internal and called_is_internal:
            detected_direction = "inbound"
        elif calling_is_internal and called_is_internal:
            detected_direction = "internal"
        else:
            detected_direction = "unknown"
        
        return {
            "status": "SUCCESS",
            "message": "Datos de tu log procesados correctamente con Pydantic V2",
            "analysis": {
                "call_type": detected_direction,
                "explanation": f"{event.calling_number} → {event.called_number}",
                "calling_is_internal": calling_is_internal,
                "called_is_internal": called_is_internal,
                "duration_billable": event.duration_billable,
                "would_be_charged": detected_direction == "outbound",
                "cost_calculation": "FREE (inbound call)" if detected_direction == "inbound" else "CALCULATED (outbound call)" if detected_direction == "outbound" else "FREE (internal call)"
            },
            "parsed_timestamps": {
                "start_time": event.start_time.isoformat() if event.start_time else None,
                "end_time": event.end_time.isoformat() if event.end_time else None,
                "answer_time": event.answer_time.isoformat() if event.answer_time else None
            }
        }
        
    except ValidationError as e:
        return {
            "status": "VALIDATION_ERROR",
            "message": "Error de validación con Pydantic V2",
            "errors": [
                {
                    "field": " -> ".join(str(loc) for loc in error['loc']),
                    "message": error['msg'],
                    "type": error['type']
                }
                for error in e.errors()
            ],
            "test_data": test_data
        }
        
    except Exception as e:
        return {
            "status": "ERROR",
            "error": str(e),
            "test_data": test_data
        }


# ✅ PÁGINA PRINCIPAL DE GESTIÓN FAC PINS

# ✅ MODELO CORREGIDO para asociaciones (referencia a fac_codes)
class FacPinAssociation(Base):
    __tablename__ = "fac_pin_associations"
    
    id = Column(Integer, primary_key=True, index=True)
    extension = Column(String(10), unique=True, index=True, nullable=False)
    fac_code_id = Column(Integer, nullable=False)  # ← REFERENCIA A fac_codes.id
    user_name = Column(String(100), nullable=True)
    user_email = Column(String(100), nullable=True)
    department = Column(String(50), nullable=True)
    status = Column(String(20), default="active")  # active, inactive
    notes = Column(String(500), nullable=True)
    created_at = Column(DateTime, default=datetime.utcnow)
    updated_at = Column(DateTime, default=datetime.utcnow, onupdate=datetime.utcnow)
    created_by = Column(String(100), nullable=True)
    
    def to_dict(self):
        return {
            'id': self.id,
            'extension': self.extension,
            'fac_code_id': self.fac_code_id,
            'user_name': self.user_name,
            'user_email': self.user_email,
            'department': self.department,
            'status': self.status,
            'notes': self.notes,
            'created_at': self.created_at,
            'updated_at': self.updated_at,
            'created_by': self.created_by
        }

# ✅ DASHBOARD PRINCIPAL CORREGIDO
@app.get("/dashboard/fac_pins")
async def dashboard_fac_pins(
    request: Request, 
    user=Depends(admin_only),
    extension: Optional[str] = None,
    fac_code_id: Optional[int] = None,
    status: Optional[str] = None,
    department: Optional[str] = None,
    page: int = 1
):
    if isinstance(user, RedirectResponse):
        return user
    
    db = SessionLocal()
    try:
        per_page = 50
        offset = (page - 1) * per_page
        
        # ✅ QUERY CON JOIN a fac_codes
        query = db.query(
            FacPinAssociation,
            FacCode.authorization_code,
            FacCode.authorization_code_name
        ).join(
            FacCode, FacPinAssociation.fac_code_id == FacCode.id
        ).filter(
            FacCode.active == True  # Solo códigos FAC activos
        )
        
        # Aplicar filtros
        if extension:
            query = query.filter(FacPinAssociation.extension.ilike(f"%{extension}%"))
        if fac_code_id:
            query = query.filter(FacPinAssociation.fac_code_id == fac_code_id)
        if status:
            query = query.filter(FacPinAssociation.status == status)
        if department:
            query = query.filter(FacPinAssociation.department == department)
        
        # Contar total y obtener registros
        total_records = query.count()
        total_pages = (total_records + per_page - 1) // per_page
        results = query.order_by(FacPinAssociation.extension).offset(offset).limit(per_page).all()
        
        # ✅ PROCESAR RESULTADOS PARA EL TEMPLATE
        rows = []
        for assoc, auth_code, auth_name in results:
            row_data = assoc.to_dict()
            row_data['authorization_code'] = auth_code
            row_data['authorization_code_name'] = auth_name
            rows.append(row_data)
        
        # ✅ OBTENER CÓDIGOS FAC ACTIVOS PARA EL FORMULARIO
        active_fac_codes = db.query(FacCode).filter(FacCode.active == True).order_by(FacCode.authorization_code_name).all()
        
        # ✅ CALCULAR ESTADÍSTICAS
        stats = calculate_fac_stats()
        
        return templates.TemplateResponse("fac_pins_management.html", {
            "request": request,
            "user": user,
            "rows": rows,
            "total_records": total_records,
            "total_pages": total_pages,
            "page": page,
            "stats": stats,
            "active_fac_codes": active_fac_codes  # ← PARA EL DROPDOWN
        })
        
    except Exception as e:
        logger.error(f"Error en dashboard_fac_pins: {e}")
        return templates.TemplateResponse("fac_pins_management.html", {
            "request": request,
            "user": user,
            "rows": [],
            "error": str(e),
            "active_fac_codes": []
        })

def calculate_fac_stats():
    """Calcular estadísticas usando fac_codes activos"""
    db = SessionLocal()
    try:
        # Total de asociaciones
        total_extensions = db.query(FacPinAssociation).count()
        
        # Asociaciones activas (con códigos FAC activos)
        active_pins = db.query(FacPinAssociation).join(
            FacCode, FacPinAssociation.fac_code_id == FacCode.id
        ).filter(
            and_(FacPinAssociation.status == 'active', FacCode.active == True)
        ).count()
        
        # Asociaciones inactivas
        inactive_pins = db.query(FacPinAssociation).filter(FacPinAssociation.status == 'inactive').count()
        
        # Códigos FAC disponibles sin asignar
        assigned_fac_codes = db.query(FacPinAssociation.fac_code_id).filter(FacPinAssociation.status == 'active').all()
        assigned_ids = [row[0] for row in assigned_fac_codes]
        
        if assigned_ids:
            available_fac_codes = db.query(FacCode).filter(
                and_(FacCode.active == True, ~FacCode.id.in_(assigned_ids))
            ).count()
        else:
            # Si no hay asignados, todos los activos están disponibles
            available_fac_codes = db.query(FacCode).filter(FacCode.active == True).count()
        
        # Días desde última actualización
        recent_update = db.query(FacPinAssociation).order_by(FacPinAssociation.updated_at.desc()).first()
        last_updated_days = 0
        if recent_update:
            delta = datetime.utcnow() - recent_update.updated_at
            last_updated_days = delta.days
        
        # ✅ DEBUG LOGS PARA VERIFICAR VALORES
        logger.info(f"DEBUG FAC Stats - Total: {total_extensions}, Activos: {active_pins}, Disponibles: {available_fac_codes}")
        
        return {
            'total_extensions': total_extensions,
            'active_pins': active_pins,
            'inactive_pins': inactive_pins,
            'unassigned_extensions': available_fac_codes,
            'last_updated_days': last_updated_days
        }
    except Exception as e:
        logger.error(f"Error calculando estadísticas FAC: {e}")
        import traceback
        logger.error(f"Traceback: {traceback.format_exc()}")
        return {
            'total_extensions': 0,
            'active_pins': 0,
            'inactive_pins': 0,
            'unassigned_extensions': 0,
            'last_updated_days': 0
        }
    finally:
        # ✅ IMPORTANTE: Cerrar la sesión SIEMPRE
        db.close()

# ✅ API PARA CREAR NUEVA ASOCIACIÓN (CORREGIDA)
@app.post("/api/fac-pins")
async def create_fac_pin_association(
    request: Request,
    user=Depends(admin_only),
    extension: str = Form(...),
    fac_code_id: int = Form(...),  # ← CAMBIO: ID del código FAC
    user_name: Optional[str] = Form(None),
    user_email: Optional[str] = Form(None),
    department: Optional[str] = Form(None),
    status: str = Form("active"),
    notes: Optional[str] = Form(None)
):
    if isinstance(user, RedirectResponse):
        return user
    
    db = SessionLocal()
    try:
        # Verificar que la extensión no exista
        existing = db.query(FacPinAssociation).filter(FacPinAssociation.extension == extension).first()
        if existing:
            return RedirectResponse(url="/dashboard/fac_pins?error=extension_exists", status_code=303)
        
        # Verificar que el código FAC existe y está activo
        fac_code = db.query(FacCode).filter(and_(FacCode.id == fac_code_id, FacCode.active == True)).first()
        if not fac_code:
            return RedirectResponse(url="/dashboard/fac_pins?error=fac_code_invalid", status_code=303)
        
        # Verificar que el código FAC no esté ya asignado
        existing_fac = db.query(FacPinAssociation).filter(
            and_(FacPinAssociation.fac_code_id == fac_code_id, FacPinAssociation.status == 'active')
        ).first()
        if existing_fac:
            return RedirectResponse(url="/dashboard/fac_pins?error=fac_code_assigned", status_code=303)
        
        # Crear nueva asociación
        new_association = FacPinAssociation(
            extension=extension,
            fac_code_id=fac_code_id,
            user_name=user_name,
            user_email=user_email,
            department=department,
            status=status,
            notes=notes,
            created_by=user.username if hasattr(user, 'username') else 'admin'
        )
        
        db.add(new_association)
        db.commit()
        
        logger.info(f"Nueva asociación FAC creada: {extension} - {fac_code.authorization_code} por {user.username if hasattr(user, 'username') else 'admin'}")
        return RedirectResponse(url="/dashboard/fac_pins?success=created", status_code=303)
        
    except Exception as e:
        db.rollback()
        logger.error(f"Error creando asociación FAC: {e}")
        return RedirectResponse(url="/dashboard/fac_pins?error=create_failed", status_code=303)
    finally:
        db.close()

# ✅ API PARA ACTUALIZAR ASOCIACIÓN (CORREGIDA)
@app.post("/api/fac-pins/update")
async def update_fac_pin_association(
    request: Request,
    user=Depends(admin_only),
    id: int = Form(...),
    extension: str = Form(...),
    fac_code_id: int = Form(...),  # ← CAMBIO: ID del código FAC
    user_name: Optional[str] = Form(None),
    department: Optional[str] = Form(None),
    status: str = Form("active")
):
    if isinstance(user, RedirectResponse):
        return user
    
    db = SessionLocal()
    try:
        # Buscar asociación existente
        association = dashboard_fac.query(FacPinAssociation).filter(FacPinAssociation.id == id).first()
        if not association:
            return RedirectResponse(url="/dashboard/fac_pins?error=not_found", status_code=303)
        
        # Verificar unicidad de extensión (si cambió)
        if association.extension != extension:
            existing = db.query(FacPinAssociation).filter(
                and_(FacPinAssociation.extension == extension, FacPinAssociation.id != id)
            ).first()
            if existing:
                return RedirectResponse(url="/dashboard/fac_pins?error=extension_exists", status_code=303)
        
        # Verificar que el código FAC existe y está activo
        fac_code = db.query(FacCode).filter(and_(FacCode.id == fac_code_id, FacCode.active == True)).first()
        if not fac_code:
            return RedirectResponse(url="/dashboard/fac_pins?error=fac_code_invalid", status_code=303)
        
        # Verificar unicidad de código FAC (si cambió)
        if association.fac_code_id != fac_code_id:
            existing_fac = db.query(FacPinAssociation).filter(
                and_(
                    FacPinAssociation.fac_code_id == fac_code_id,
                    FacPinAssociation.status == 'active',
                    FacPinAssociation.id != id
                )
            ).first()
            if existing_fac:
                return RedirectResponse(url="/dashboard/fac_pins?error=fac_code_assigned", status_code=303)
        
        # Actualizar campos
        association.extension = extension
        association.fac_code_id = fac_code_id
        association.user_name = user_name
        association.department = department
        association.status = status
        association.updated_at = datetime.utcnow()
        
        db.commit()
        
        logger.info(f"Asociación FAC actualizada: {extension} por {user.username if hasattr(user, 'username') else 'admin'}")
        return RedirectResponse(url="/dashboard/fac_pins?success=updated", status_code=303)
        
    except Exception as e:
        db.rollback()
        logger.error(f"Error actualizando asociación FAC: {e}")
        return RedirectResponse(url="/dashboard/fac_pins?error=update_failed", status_code=303)
    finally:
        # ✅ IMPORTANTE: Cerrar la sesión SIEMPRE
        db.close()

# ✅ FUNCIÓN PARA VALIDAR PIN FAC (CORREGIDA)
def validate_fac_pin(extension: str, authorization_code: str) -> bool:
    """
    Valida si un código de autorización es válido para una extensión específica
    """
    try:
        db = SessionLocal()
        # Buscar asociación activa con código FAC activo
        association = db.query(FacPinAssociation).join(
            FacCode, FacPinAssociation.fac_code_id == FacCode.id
        ).filter(
            and_(
                FacPinAssociation.extension == extension,
                FacCode.authorization_code == authorization_code,
                FacPinAssociation.status == 'active',
                FacCode.active == True
            )
        ).first()
        
        return association is not None
        
    except Exception as e:
        logger.error(f"Error validando código FAC: {e}")
        return False

# ✅ ENDPOINT PARA VALIDACIÓN EN TIEMPO REAL (CORREGIDO)
@app.post("/api/fac-pins/validate")
async def validate_fac_pin_endpoint(
    extension: str = Form(...),
    authorization_code: str = Form(...)  # ← CAMBIO: código de autorización
):
    """Endpoint para validar código FAC desde el sistema telefónico"""
    is_valid = validate_fac_pin(extension, authorization_code)
    
    if is_valid:
        logger.info(f"Código FAC válido usado: Extensión {extension}, Código {authorization_code}")
        
    return {
        "valid": is_valid,
        "extension": extension,
        "authorization_code": authorization_code,
        "message": "Código válido" if is_valid else "Código inválido o inactivo"
    }