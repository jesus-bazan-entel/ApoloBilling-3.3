
from fastapi import APIRouter, Depends, HTTPException
from sqlalchemy.orm import Session
from typing import List
from app.db.session import SessionLocal
from app.models.billing import RateCard
from pydantic import BaseModel
from decimal import Decimal
from datetime import datetime

router = APIRouter()

def get_db():
    db = SessionLocal()
    try:
        yield db
    finally:
        db.close()

class RateCardCreate(BaseModel):
    destination_prefix: str
    destination_name: str
    rate_per_minute: Decimal
    billing_increment: int = 60
    connection_fee: Decimal = 0.0
    priority: int = 0

class RateCardResponse(RateCardCreate):
    id: int
    effective_start: datetime
    effective_end: datetime = None

    class Config:
        orm_mode = True

@router.get("/", response_model=List[RateCardResponse])
def read_rates(skip: int = 0, limit: int = 100, db: Session = Depends(get_db)):
    rates = db.query(RateCard).offset(skip).limit(limit).all()
    return rates

@router.post("/", response_model=RateCardResponse)
def create_rate(rate: RateCardCreate, db: Session = Depends(get_db)):
    # Check if prefix exists
    existing = db.query(RateCard).filter(RateCard.destination_prefix == rate.destination_prefix).first()
    if existing:
        # TODO: Implement versioning/update logic instead of error
        pass

    new_rate = RateCard(**rate.dict())
    db.add(new_rate)
    db.commit()
    db.refresh(new_rate)
    return new_rate

@router.delete("/{rate_id}")
def delete_rate(rate_id: int, db: Session = Depends(get_db)):
    rate = db.query(RateCard).filter(RateCard.id == rate_id).first()
    if not rate:
        raise HTTPException(status_code=404, detail="Rate not found")
    db.delete(rate)
    db.commit()
    return {"status": "success"}
