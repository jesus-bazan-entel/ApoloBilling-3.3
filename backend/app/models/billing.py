
from sqlalchemy import Column, Integer, String, Numeric, DateTime, Enum, ForeignKey
from sqlalchemy.orm import relationship
from sqlalchemy.dialects.postgresql import UUID
from app.db.base_class import Base
from datetime import datetime
import uuid
import enum

class AccountType(str, enum.Enum):
    PREPAID = "prepaid"
    POSTPAID = "postpaid"

class AccountStatus(str, enum.Enum):
    ACTIVE = "active"
    SUSPENDED = "suspended"
    CLOSED = "closed"

class ReservationStatus(str, enum.Enum):
    ACTIVE = "active"
    RELEASED = "released"
    COMMITTED = "committed"
    EXPIRED = "expired"

class Account(Base):
    __tablename__ = "accounts"

    id = Column(Integer, primary_key=True, index=True)
    account_number = Column(String, unique=True, index=True, nullable=False)
    customer_phone = Column(String, index=True, nullable=True)
    account_type = Column(Enum(AccountType), default=AccountType.PREPAID, nullable=False)
    balance = Column(Numeric(10, 4), default=0.0000, nullable=False)
    credit_limit = Column(Numeric(10, 4), default=0.0000, nullable=False)
    currency = Column(String, default="USD", nullable=False)
    status = Column(Enum(AccountStatus), default=AccountStatus.ACTIVE, nullable=False)
    max_concurrent_calls = Column(Integer, default=1)
    created_at = Column(DateTime, default=datetime.utcnow)
    updated_at = Column(DateTime, default=datetime.utcnow, onupdate=datetime.utcnow)

    reservations = relationship("BalanceReservation", back_populates="account")
    transactions = relationship("BalanceTransaction", back_populates="account")
    cdrs = relationship("CDR", back_populates="account")

class RateCard(Base):
    __tablename__ = "rate_cards"

    id = Column(Integer, primary_key=True, index=True)
    destination_prefix = Column(String, index=True, nullable=False)
    destination_name = Column(String, nullable=False)
    rate_per_minute = Column(Numeric(10, 4), nullable=False)
    billing_increment = Column(Integer, default=60, nullable=False) # Seconds
    connection_fee = Column(Numeric(10, 4), default=0.0000)
    effective_start = Column(DateTime, default=datetime.utcnow)
    effective_end = Column(DateTime, nullable=True)
    priority = Column(Integer, default=0)

class BalanceReservation(Base):
    __tablename__ = "balance_reservations"

    id = Column(UUID(as_uuid=True), primary_key=True, default=uuid.uuid4)
    account_id = Column(Integer, ForeignKey("accounts.id"), nullable=False)
    call_uuid = Column(String, index=True, nullable=False)
    reserved_amount = Column(Numeric(10, 4), nullable=False)
    consumed_amount = Column(Numeric(10, 4), default=0.0000)
    released_amount = Column(Numeric(10, 4), default=0.0000)
    status = Column(Enum(ReservationStatus), default=ReservationStatus.ACTIVE)
    reservation_type = Column(String, default="initial") # Added for Rust compatibility
    destination_prefix = Column(String)
    rate_per_minute = Column(Numeric(10, 4))
    reserved_minutes = Column(Integer, default=0) # Added for Rust compatibility
    expires_at = Column(DateTime, nullable=False)
    created_at = Column(DateTime, default=datetime.utcnow)
    consumed_at = Column(DateTime, nullable=True) # Added for Rust compatibility
    created_by = Column(String, default="system") # Added for Rust compatibility
    updated_at = Column(DateTime, default=datetime.utcnow, onupdate=datetime.utcnow)

    account = relationship("Account", back_populates="reservations")

class BalanceTransaction(Base):
    __tablename__ = "balance_transactions"

    id = Column(Integer, primary_key=True, index=True)
    account_id = Column(Integer, ForeignKey("accounts.id"), nullable=False)
    amount = Column(Numeric(10, 4), nullable=False)
    previous_balance = Column(Numeric(10, 4), nullable=False)
    new_balance = Column(Numeric(10, 4), nullable=False)
    transaction_type = Column(String, nullable=False) # recharge, consumption, refund
    reason = Column(String)
    call_uuid = Column(String, nullable=True)
    created_at = Column(DateTime, default=datetime.utcnow)

    account = relationship("Account", back_populates="transactions")
