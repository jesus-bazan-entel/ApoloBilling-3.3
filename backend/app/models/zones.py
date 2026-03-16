from sqlalchemy import Column, Integer, String, Numeric, DateTime, Boolean, ForeignKey
from sqlalchemy.orm import relationship
from app.db.base_class import Base
from datetime import datetime

class Zone(Base):
    __tablename__ = "zones"

    id = Column(Integer, primary_key=True, index=True)
    country_id = Column(Integer)  # ForeignKey("countries.id") if we had Country model
    zone_name = Column(String, unique=True, index=True)
    zone_code = Column(String)
    description = Column(String, nullable=True)
    zone_type = Column(String, default="GEOGRAPHIC") # GEOGRAPHIC, MOBILE, SPECIAL
    region_name = Column(String, nullable=True)
    enabled = Column(Boolean, default=True)
    created_at = Column(DateTime, default=datetime.utcnow)
    updated_at = Column(DateTime, default=datetime.utcnow, onupdate=datetime.utcnow)

    # Relationships
    prefixes = relationship("Prefix", back_populates="zone", cascade="all, delete-orphan")
    rates = relationship("RateZone", back_populates="zone", cascade="all, delete-orphan")

class Prefix(Base):
    __tablename__ = "prefixes"

    id = Column(Integer, primary_key=True, index=True)
    zone_id = Column(Integer, ForeignKey("zones.id"), nullable=False)
    prefix = Column(String, index=True, nullable=False)
    prefix_length = Column(Integer, nullable=False)
    operator_name = Column(String, nullable=True)
    network_type = Column(String, default="UNKNOWN") # FIXED, MOBILE
    enabled = Column(Boolean, default=True)
    created_at = Column(DateTime, default=datetime.utcnow)
    updated_at = Column(DateTime, default=datetime.utcnow, onupdate=datetime.utcnow)

    zone = relationship("Zone", back_populates="prefixes")

class RateZone(Base):
    __tablename__ = "rate_zones"

    id = Column(Integer, primary_key=True, index=True)
    zone_id = Column(Integer, ForeignKey("zones.id"), nullable=False)
    rate_name = Column(String, nullable=True)
    rate_per_minute = Column(Numeric(10, 5), nullable=False)
    rate_per_call = Column(Numeric(10, 5), default=0.0)
    billing_increment = Column(Integer, default=60)
    min_duration = Column(Integer, default=0)
    effective_from = Column(DateTime, default=datetime.utcnow)
    currency = Column(String, default="USD")
    priority = Column(Integer, default=1)
    enabled = Column(Boolean, default=True)
    created_at = Column(DateTime, default=datetime.utcnow)
    updated_at = Column(DateTime, default=datetime.utcnow, onupdate=datetime.utcnow)

    zone = relationship("Zone", back_populates="rates")
