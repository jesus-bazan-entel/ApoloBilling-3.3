
from fastapi import APIRouter, Depends, HTTPException
from sqlalchemy.orm import Session
from typing import List
from app.db.session import SessionLocal
from app.models.billing import Account, AccountType, AccountStatus
from pydantic import BaseModel
from decimal import Decimal

router = APIRouter()

def get_db():
    db = SessionLocal()
    try:
        yield db
    finally:
        db.close()

class AccountCreate(BaseModel):
    account_number: str
    customer_phone: str = None
    account_type: AccountType = AccountType.PREPAID
    credit_limit: Decimal = 0.0
    currency: str = "USD"
    max_concurrent_calls: int = 1

class AccountUpdate(BaseModel):
    customer_phone: str = None
    status: AccountStatus = None
    credit_limit: Decimal = None
    max_concurrent_calls: int = None

class AccountResponse(AccountCreate):
    id: int
    balance: Decimal
    status: AccountStatus

    class Config:
        orm_mode = True

@router.get("/", response_model=List[AccountResponse])
def read_accounts(skip: int = 0, limit: int = 100, db: Session = Depends(get_db)):
    accounts = db.query(Account).offset(skip).limit(limit).all()
    return accounts

@router.post("/", response_model=AccountResponse)
def create_account(account: AccountCreate, db: Session = Depends(get_db)):
    db_account = db.query(Account).filter(Account.account_number == account.account_number).first()
    if db_account:
        raise HTTPException(status_code=400, detail="Account already exists")
    
    new_account = Account(**account.dict())
    db.add(new_account)
    db.commit()
    db.refresh(new_account)
    return new_account

@router.get("/{account_id}", response_model=AccountResponse)
def read_account(account_id: int, db: Session = Depends(get_db)):
    db_account = db.query(Account).filter(Account.id == account_id).first()
    if db_account is None:
        raise HTTPException(status_code=404, detail="Account not found")
    return db_account

@router.put("/{account_id}", response_model=AccountResponse)
def update_account(account_id: int, account: AccountUpdate, db: Session = Depends(get_db)):
    db_account = db.query(Account).filter(Account.id == account_id).first()
    if db_account is None:
        raise HTTPException(status_code=404, detail="Account not found")
    
    update_data = account.dict(exclude_unset=True)
    for key, value in update_data.items():
        setattr(db_account, key, value)
    
    db.commit()
    db.refresh(db_account)
    return db_account

@router.post("/{account_id}/topup")
def topup_account(account_id: int, amount: Decimal, db: Session = Depends(get_db)):
    db_account = db.query(Account).filter(Account.id == account_id).first()
    if db_account is None:
        raise HTTPException(status_code=404, detail="Account not found")
    
    db_account.balance += amount
    db.commit()
    return {"status": "success", "new_balance": db_account.balance}
