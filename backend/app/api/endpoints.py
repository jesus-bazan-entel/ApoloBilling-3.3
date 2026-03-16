
from fastapi import APIRouter, Depends, HTTPException
from sqlalchemy.orm import Session
from typing import List, Dict
from datetime import datetime
import json

from app.db.session import SessionLocal
from app.models.cdr import ActiveCall, CDR
from app.services.rating import determinar_zona_y_tarifa

router = APIRouter()

def get_db():
    db = SessionLocal()
    try:
        yield db
    finally:
        db.close()



@router.get("/active-calls")
def get_active_calls(db: Session = Depends(get_db)):
    active_calls = db.query(ActiveCall).order_by(ActiveCall.start_time.desc()).all()
    return active_calls

@router.post("/active-calls")
async def report_active_call(call_data: dict, db: Session = Depends(get_db)):
    call_id = call_data.get("call_id")
    if not call_id:
        return {"status": "error", "message": "call_id is required"}

    # Calculate zone and tariff if not provided
    called_number = call_data.get("called_number") or call_data.get("destination")
    if called_number:
        rating = determinar_zona_y_tarifa(called_number, db)
        # Here we could update cost based on duration and rating
        # But for now we just trust call_data or use defaults
        pass

    # Upsert logic
    existing_call = db.query(ActiveCall).filter(ActiveCall.call_id == call_id).first()
    
    if existing_call:
        for key, value in call_data.items():
            if hasattr(existing_call, key):
                setattr(existing_call, key, value)
        existing_call.last_updated = datetime.now()
    else:
        new_call = ActiveCall(
            call_id=call_id,
            calling_number=call_data.get("calling_number"),
            called_number=called_number,
            direction=call_data.get("direction", "unknown"),
            start_time=datetime.fromisoformat(call_data.get("start_time").replace('Z', '+00:00')) if call_data.get("start_time") else datetime.now(),
            current_duration=call_data.get("duration", 0),
            current_cost=call_data.get("cost", 0.0),
            connection_id=call_data.get("connection_id")
        )
        db.add(new_call)
    
    db.commit()
    
    # Broadcast to WS (Removed)
    # active_calls = db.query(ActiveCall).all()
    # ws_manager call removed

    
    return {"status": "ok"}

@router.delete("/active-calls/{call_id}")
async def remove_active_call(call_id: str, db: Session = Depends(get_db)):
    call = db.query(ActiveCall).filter(ActiveCall.call_id == call_id).first()
    if call:
        db.delete(call)
        db.commit()
        return {"status": "ok"}
    return {"status": "not_found"}

@router.post("/cdr")
def create_cdr(cdr_data: dict, db: Session = Depends(get_db)):
    new_cdr = CDR(**cdr_data)
    db.add(new_cdr)
    db.commit()
    return {"status": "ok", "id": new_cdr.id}

# ... (previous code)
