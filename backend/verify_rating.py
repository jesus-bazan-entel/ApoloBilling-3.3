from sqlalchemy import create_engine
from sqlalchemy.orm import sessionmaker
from app.db.base_class import Base
from app.models.billing import RateCard
import app.models.cdr # Import to register CDR model for relationships
from app.services.rating import determinar_zona_y_tarifa
from decimal import Decimal

# Create in-memory DB
engine = create_engine('sqlite:///:memory:')
Session = sessionmaker(bind=engine)
Base.metadata.create_all(engine)

session = Session()

# Setup Data
rc1 = RateCard(destination_prefix="1", destination_name="USA", rate_per_minute=0.01)
rc2 = RateCard(destination_prefix="12", destination_name="USA Area", rate_per_minute=0.02)
rc3 = RateCard(destination_prefix="123", destination_name="USA SubArea", rate_per_minute=0.03)

session.add_all([rc1, rc2, rc3])
session.commit()

# Test Cases
t1 = determinar_zona_y_tarifa("15551234", session) # Should match "1"
t2 = determinar_zona_y_tarifa("125551234", session) # Should match "12"
t3 = determinar_zona_y_tarifa("1235551234", session) # Should match "123"
t4 = determinar_zona_y_tarifa("999", session) # Should match None

print(f"T1 (Expected 1): Found {t1['prefijo']}")
assert t1['prefijo'] == "1"

print(f"T2 (Expected 12): Found {t2['prefijo']}")
assert t2['prefijo'] == "12"

print(f"T3 (Expected 123): Found {t3['prefijo']}")
assert t3['prefijo'] == "123"

print(f"T4 (Expected UNKNOWN): Found {t4['prefijo']}")
assert t4['prefijo'] == "UNKNOWN"

print("All tests passed!")
