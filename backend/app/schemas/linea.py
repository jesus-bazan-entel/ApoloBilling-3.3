from pydantic import BaseModel
from typing import Optional
from decimal import Decimal

class LineaBase(BaseModel):
    numero: str
    tipo: str = "anexo"
    usuario: str
    area_nivel1: str
    area_nivel2: Optional[str] = None
    documento_id: Optional[str] = None
    activo: bool = True

class LineaCreate(LineaBase):
    pass

class LineaUpdate(BaseModel):
    usuario: Optional[str] = None
    area_nivel1: Optional[str] = None
    area_nivel2: Optional[str] = None
    documento_id: Optional[str] = None
    activo: Optional[bool] = None
    saldo: Optional[Decimal] = None

class LineaInDB(LineaBase):
    id: int
    saldo: Decimal

    class Config:
        from_attributes = True
