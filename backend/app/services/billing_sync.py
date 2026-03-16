from sqlalchemy.orm import Session
from sqlalchemy import text
import logging

logger = logging.getLogger(__name__)

def sync_rate_cards(db: Session):
    """
    Sincroniza la tabla rate_cards (usada por Rust) con los datos
    de zones, prefixes y rate_zones (usadas por el Dashboard).
    """
    try:
        logger.info("Iniciando sincronización de rate_cards...")
        
        # 1. Truncate rate_cards (reemplazo completo)
        db.execute(text("TRUNCATE TABLE rate_cards RESTART IDENTITY"))
        
        # 2. Insertar datos combinados
        query = text("""
            INSERT INTO rate_cards (
                destination_prefix, destination_name, rate_per_minute,
                billing_increment, connection_fee, effective_start, priority, created_at
            )
            SELECT 
                p.prefix, 
                z.zone_name, 
                t.rate_per_minute,
                t.billing_increment, 
                0, 
                t.effective_from,
                t.priority,
                CURRENT_TIMESTAMP
            FROM prefixes p
            JOIN zones z ON p.zone_id = z.id
            JOIN rate_zones t ON z.id = t.zone_id
            WHERE t.enabled = TRUE AND p.enabled = TRUE
        """)
        db.execute(query)
        db.commit()
        logger.info("✅ Sincronización completada exitosamente.")
        
    except Exception as e:
        logger.error(f"❌ Error sincronizando rate_cards: {str(e)}")
        db.rollback()
        raise e
