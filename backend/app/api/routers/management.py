from fastapi import APIRouter, Depends, HTTPException, Body
from sqlalchemy.orm import Session
from sqlalchemy import text
from typing import List, Optional
from pydantic import BaseModel
from datetime import datetime

from app.db.database import get_db
from app.models.zones import Zone, Prefix, RateZone
from app.services.billing_sync import sync_rate_cards

router = APIRouter()

# --- Schemas ---
class ZoneCreate(BaseModel):
    nombre: str
    descripcion: str = None
    # Add other fields as optional or defaults

class ZoneUpdate(BaseModel):
    nombre: str
    descripcion: str = None

class PrefixCreate(BaseModel):
    zona_id: int
    prefijo: str
    longitud_minima: int = 0 # Default

class TariffCreate(BaseModel):
    zona_id: int
    tarifa: float # Assuming rate_per_minute

# --- Zones Endpoints ---
@router.post("/zonas")
def create_zona(data: ZoneCreate, db: Session = Depends(get_db)):
    # Map 'nombre' -> 'zone_name'
    new_zone = Zone(
        zone_name=data.nombre,
        description=data.descripcion,
        zone_type="GEOGRAPHIC",
        country_id=1, # Default
        zone_code=data.nombre[:10].upper()
    )
    db.add(new_zone)
    db.commit()
    db.refresh(new_zone)
    
    sync_rate_cards(db) # Sync
    return {"id": new_zone.id, "nombre": new_zone.zone_name}

@router.put("/zonas/{zona_id}")
def update_zona(zona_id: int, data: ZoneUpdate, db: Session = Depends(get_db)):
    zone = db.query(Zone).filter(Zone.id == zona_id).first()
    if not zone:
        raise HTTPException(status_code=404, detail="Zone not found")
    
    zone.zone_name = data.nombre
    zone.description = data.descripcion
    db.commit()
    
    sync_rate_cards(db) # Sync
    return {"status": "ok"}

@router.delete("/zonas/{zona_id}")
def delete_zona(zona_id: int, db: Session = Depends(get_db)):
    zone = db.query(Zone).filter(Zone.id == zona_id).first()
    if not zone:
        raise HTTPException(status_code=404, detail="Zone not found")
    
    db.delete(zone)
    db.commit()
    
    sync_rate_cards(db) # Sync
    return {"status": "ok"}

# --- Prefixes Endpoints ---
@router.post("/prefijos")
def create_prefijo(data: PrefixCreate, db: Session = Depends(get_db)):
    new_prefix = Prefix(
        zone_id=data.zona_id,
        prefix=data.prefijo,
        prefix_length=len(data.prefijo),
        enabled=True
    )
    db.add(new_prefix)
    db.commit()
    
    sync_rate_cards(db)
    return {"status": "ok"}

@router.delete("/prefijos/{id}")
def delete_prefijo(id: int, db: Session = Depends(get_db)):
    prefix = db.query(Prefix).filter(Prefix.id == id).first()
    if prefix:
        db.delete(prefix)
        db.commit()
        sync_rate_cards(db)
    return {"status": "ok"}

# --- Tariffs Endpoints ---
@router.post("/tarifas")
def create_tarifa(data: TariffCreate, db: Session = Depends(get_db)):
    # rate_per_minute input is likely in USD/min
    new_rate = RateZone(
        zone_id=data.zona_id,
        rate_per_minute=data.tarifa,
        rate_name="Manual",
        enabled=True,
        effective_from=datetime.utcnow()
    )
    db.add(new_rate)
    db.commit()
    
    sync_rate_cards(db)
    return {"status": "ok"}

@router.put("/tarifas/{id}")
def update_tarifa(id: int, rate: float = Body(..., embed=True), db: Session = Depends(get_db)):
    # Simple rate update
    t = db.query(RateZone).filter(RateZone.id == id).first()
    if t:
        t.rate_per_minute = rate
        db.commit()
        sync_rate_cards(db)
    return {"status": "ok"}

@router.delete("/tarifas/{id}")
def delete_tarifa(id: int, db: Session = Depends(get_db)):
    t = db.query(RateZone).filter(RateZone.id == id).first()
    if t:
        db.delete(t)
        db.commit()
        sync_rate_cards(db)
    return {"status": "ok"}
