
from sqlalchemy import Column, Integer, String, Numeric, DateTime, ForeignKey, BigInteger
from sqlalchemy.orm import relationship
from app.db.base_class import Base
from datetime import datetime

class CDR(Base):
    __tablename__ = "cdrs"

    id = Column(BigInteger, primary_key=True, index=True)
    call_uuid = Column(String, unique=True, index=True, nullable=False)  # CORREGIDO de 'uuid'
    account_id = Column(Integer, ForeignKey("accounts.id"), nullable=True)
    caller_number = Column(String, nullable=False)  # CORREGIDO de 'caller'
    called_number = Column(String, nullable=False)  # CORREGIDO de 'callee'
    start_time = Column(DateTime, nullable=False)
    answer_time = Column(DateTime, nullable=True)
    end_time = Column(DateTime, nullable=False)
    duration = Column(Integer, default=0)
    billsec = Column(Integer, default=0)
    hangup_cause = Column(String)
    rate_id = Column(Integer, nullable=True)  # CORREGIDO de 'rate_applied'
    cost = Column(Numeric(10, 4), nullable=True)
    direction = Column(String, default="outbound")
    freeswitch_server_id = Column(String, nullable=True)
    created_at = Column(DateTime, default=datetime.utcnow)

    account = relationship("app.models.billing.Account", back_populates="cdrs")

class ActiveCall(Base):
    __tablename__ = "active_calls"

    id = Column(Integer, primary_key=True, index=True)
    call_id = Column(String, unique=True, index=True, nullable=False)
    calling_number = Column(String, nullable=True)
    called_number = Column(String, nullable=True)
    direction = Column(String, default="unknown")
    start_time = Column(DateTime, default=datetime.utcnow)
    current_duration = Column(Integer, default=0)
    current_cost = Column(Numeric(10, 4), default=0.0000)
    connection_id = Column(String, nullable=True)
    last_updated = Column(DateTime, default=datetime.utcnow, onupdate=datetime.utcnow)
    server = Column(String, nullable=True)
