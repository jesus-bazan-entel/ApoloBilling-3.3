
from sqlalchemy import Column, Integer, String, Numeric, DateTime, Boolean
from app.db.base_class import Base
from datetime import datetime

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

class Configuracion(Base):
    __tablename__ = "configuracion"
    id = Column(Integer, primary_key=True, index=True)
    clave = Column(String, unique=True)
    valor = Column(String)
    descripcion = Column(String, nullable=True)

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
