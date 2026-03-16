
from sqlalchemy import Column, Integer, String, Numeric, Boolean
from app.db.base_class import Base

class Linea(Base):
    __tablename__ = "lineas"

    id = Column(Integer, primary_key=True, index=True)
    numero = Column(String, unique=True, index=True)
    tipo = Column(String, default="anexo") # 'anexo', 'extension', 'linea'
    usuario = Column(String)
    area_nivel1 = Column(String)
    area_nivel2 = Column(String, nullable=True)
    area_nivel3 = Column(String, nullable=True)
    saldo = Column(Numeric(10, 2), default=0.00)
    pin = Column(String, nullable=True)
    activo = Column(Boolean, default=True)
