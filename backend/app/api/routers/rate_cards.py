from fastapi import APIRouter, Depends, HTTPException
from sqlalchemy.orm import Session
from sqlalchemy import func
from typing import List, Optional
from pydantic import BaseModel
from datetime import datetime
from decimal import Decimal

from app.db.session import SessionLocal
from app.models.billing import RateCard

router = APIRouter()

def get_db():
    db = SessionLocal()
    try:
        yield db
    finally:
        db.close()

# --- Schemas ---
class RateCardCreate(BaseModel):
    destination_prefix: str
    destination_name: str
    rate_per_minute: Decimal
    billing_increment: int = 6
    connection_fee: Decimal = Decimal('0.0000')
    priority: int = 0
    
class RateCardUpdate(BaseModel):
    destination_name: Optional[str] = None
    rate_per_minute: Optional[Decimal] = None
    billing_increment: Optional[int] = None
    connection_fee: Optional[Decimal] = None
    priority: Optional[int] = None
    effective_end: Optional[datetime] = None

class RateCardResponse(BaseModel):
    id: int
    destination_prefix: str
    destination_name: str
    rate_per_minute: Decimal
    billing_increment: int
    connection_fee: Decimal
    effective_start: datetime
    effective_end: Optional[datetime]
    priority: int
    
    class Config:
        from_attributes = True

# --- CRUD Endpoints ---

@router.get("/rate-cards", response_model=List[RateCardResponse])
def list_rate_cards(
    prefix: Optional[str] = None,
    name: Optional[str] = None,
    skip: int = 0,
    limit: int = 100,
    db: Session = Depends(get_db)
):
    """List all rate cards with optional filtering"""
    query = db.query(RateCard)
    
    if prefix:
        query = query.filter(RateCard.destination_prefix.like(f"{prefix}%"))
    if name:
        query = query.filter(RateCard.destination_name.ilike(f"%{name}%"))
    
    # Only active rates (no effective_end or future effective_end)
    query = query.filter(
        (RateCard.effective_end.is_(None)) | 
        (RateCard.effective_end > datetime.utcnow())
    )
    
    return query.order_by(
        RateCard.destination_prefix, 
        RateCard.priority.desc()
    ).offset(skip).limit(limit).all()

@router.get("/rate-cards/{rate_id}", response_model=RateCardResponse)
def get_rate_card(rate_id: int, db: Session = Depends(get_db)):
    """Get specific rate card"""
    rate = db.query(RateCard).filter(RateCard.id == rate_id).first()
    if not rate:
        raise HTTPException(status_code=404, detail="Rate card not found")
    return rate

@router.post("/rate-cards", response_model=RateCardResponse, status_code=201)
def create_rate_card(data: RateCardCreate, db: Session = Depends(get_db)):
    """Create new rate card"""
    
    # Check for duplicate active prefix
    existing = db.query(RateCard).filter(
        RateCard.destination_prefix == data.destination_prefix,
        (RateCard.effective_end.is_(None)) | 
        (RateCard.effective_end > datetime.utcnow())
    ).first()
    
    if existing:
        raise HTTPException(
            status_code=409, 
            detail=f"Active rate already exists for prefix {data.destination_prefix}"
        )
    
    rate = RateCard(
        destination_prefix=data.destination_prefix,
        destination_name=data.destination_name,
        rate_per_minute=data.rate_per_minute,
        billing_increment=data.billing_increment,
        connection_fee=data.connection_fee,
        effective_start=datetime.utcnow(),
        priority=data.priority,
    )
    
    db.add(rate)
    db.commit()
    db.refresh(rate)
    
    return rate

@router.put("/rate-cards/{rate_id}", response_model=RateCardResponse)
def update_rate_card(
    rate_id: int, 
    data: RateCardUpdate, 
    db: Session = Depends(get_db)
):
    """Update existing rate card"""
    rate = db.query(RateCard).filter(RateCard.id == rate_id).first()
    
    if not rate:
        raise HTTPException(status_code=404, detail="Rate card not found")
    
    # Update only provided fields
    if data.destination_name is not None:
        rate.destination_name = data.destination_name
    if data.rate_per_minute is not None:
        rate.rate_per_minute = data.rate_per_minute
    if data.billing_increment is not None:
        rate.billing_increment = data.billing_increment
    if data.connection_fee is not None:
        rate.connection_fee = data.connection_fee
    if data.priority is not None:
        rate.priority = data.priority
    if data.effective_end is not None:
        rate.effective_end = data.effective_end
    
    rate.updated_at = datetime.utcnow()
    
    db.commit()
    db.refresh(rate)
    
    return rate

@router.delete("/rate-cards/{rate_id}")
def delete_rate_card(rate_id: int, db: Session = Depends(get_db)):
    """Soft delete: set effective_end to now"""
    rate = db.query(RateCard).filter(RateCard.id == rate_id).first()
    
    if not rate:
        raise HTTPException(status_code=404, detail="Rate card not found")
    
    # Soft delete: mark as ended
    rate.effective_end = datetime.utcnow()
    rate.updated_at = datetime.utcnow()
    
    db.commit()
    
    return {"status": "ok", "message": "Rate card deactivated"}

@router.post("/rate-cards/bulk", status_code=201)
def bulk_create_rate_cards(
    rates: List[RateCardCreate], 
    db: Session = Depends(get_db)
):
    """Bulk create rate cards (for importing CSV)"""
    created = []
    errors = []
    
    for idx, data in enumerate(rates):
        try:
            # Check for duplicate
            existing = db.query(RateCard).filter(
                RateCard.destination_prefix == data.destination_prefix,
                (RateCard.effective_end.is_(None)) | 
                (RateCard.effective_end > datetime.utcnow())
            ).first()
            
            if existing:
                errors.append({
                    "index": idx,
                    "prefix": data.destination_prefix,
                    "error": "Duplicate active prefix"
                })
                continue
            
            rate = RateCard(
                destination_prefix=data.destination_prefix,
                destination_name=data.destination_name,
                rate_per_minute=data.rate_per_minute,
                billing_increment=data.billing_increment,
                connection_fee=data.connection_fee,
                effective_start=datetime.utcnow(),
                priority=data.priority,
            )
            
            db.add(rate)
            created.append(data.destination_prefix)
            
        except Exception as e:
            errors.append({
                "index": idx,
                "prefix": data.destination_prefix,
                "error": str(e)
            })
    
    db.commit()
    
    return {
        "created": len(created),
        "errors": len(errors),
        "created_prefixes": created,
        "errors_detail": errors
    }

@router.get("/rate-cards/search/{phone_number}")
def search_rate_for_number(phone_number: str, db: Session = Depends(get_db)):
    """Find matching rate for a phone number (Longest Prefix Match)"""
    
    # Clean number
    clean_number = ''.join(filter(str.isdigit, phone_number))
    
    if not clean_number:
        raise HTTPException(status_code=400, detail="Invalid phone number")
    
    # Generate all possible prefixes (descending length)
    prefixes = [clean_number[:i] for i in range(len(clean_number), 0, -1)]
    
    # Query with LPM
    rate = db.query(RateCard).filter(
        RateCard.destination_prefix.in_(prefixes),
        RateCard.effective_start <= datetime.utcnow(),
        (RateCard.effective_end.is_(None)) | 
        (RateCard.effective_end > datetime.utcnow())
    ).order_by(
        func.length(RateCard.destination_prefix).desc(),
        RateCard.priority.desc()
    ).first()
    
    if not rate:
        raise HTTPException(
            status_code=404, 
            detail=f"No rate found for number {phone_number}"
        )
    
    return {
        "phone_number": phone_number,
        "matched_prefix": rate.destination_prefix,
        "destination_name": rate.destination_name,
        "rate_per_minute": float(rate.rate_per_minute),
        "billing_increment": rate.billing_increment,
        "rate_id": rate.id
    }
