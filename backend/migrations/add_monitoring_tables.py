"""
Database migration for monitoring tables
Run this after deploying the monitoring module
"""
import sys
import os

# Add the project root to path
sys.path.insert(0, '/opt/ApoloBilling')

# Force correct DATABASE_URL before importing anything else
os.environ['DATABASE_URL'] = "postgresql://apolo_user:apolo_password_2024@127.0.0.1:5432/apolo_billing"

from app.db.session import engine, Base
from app.models import cdr
from app.models.monitoring import Base as MonitoringBase
from sqlalchemy import text

def create_monitoring_tables():
    """Create monitoring tables in PostgreSQL"""
    try:
        # Create all tables defined in monitoring models
        MonitoringBase.metadata.create_all(bind=engine)
        
        print("✅ Monitoring tables created successfully")
        
        # Verify tables exist
        with engine.connect() as conn:
            result = conn.execute(text("""
                SELECT table_name FROM information_schema.tables 
                WHERE table_schema = 'public' 
                AND table_name LIKE 'monitoring_%'
                ORDER BY table_name
            """))
            tables = [row[0] for row in result.fetchall()]
            print(f"Created tables: {tables}")
            
    except Exception as e:
        print(f"❌ Error creating monitoring tables: {e}")
        raise

def insert_default_configs():
    """Insert default monitoring configuration"""
    from app.db.session import SessionLocal
    from app.models.monitoring import MonitoringConfig
    from datetime import datetime
    
    db = SessionLocal()
    try:
        default_configs = [
            {
                "component": "kamailio",
                "thresholds": {
                    "shm_high": {"metric": "shm_usage_percent", "threshold": 90, "operator": ">", "severity": "critical", "enabled": True},
                    "shm_warning": {"metric": "shm_usage_percent", "threshold": 75, "operator": ">", "severity": "warning", "enabled": True},
                    "active_calls_high": {"metric": "active_calls", "threshold": 1000, "operator": ">", "severity": "warning", "enabled": True},
                    "success_rate_low": {"metric": "success_rate", "threshold": 95, "operator": "<", "severity": "critical", "enabled": True},
                    "db_errors": {"metric": "db_errors", "threshold": 0, "operator": ">", "severity": "critical", "enabled": True}
                },
                "interval_seconds": 60,
                "retention_days": 30,
                "enabled": True
            },
            {
                "component": "rtpengine",
                "thresholds": {
                    "cpu_high": {"metric": "cpu_percent", "threshold": 80, "operator": ">", "severity": "critical", "enabled": True},
                    "cpu_warning": {"metric": "cpu_percent", "threshold": 60, "operator": ">", "severity": "warning", "enabled": True},
                    "port_usage_high": {"metric": "port_usage_percent", "threshold": 90, "operator": ">", "severity": "critical", "enabled": True},
                    "port_usage_warning": {"metric": "port_usage_percent", "threshold": 75, "operator": ">", "severity": "warning", "enabled": True},
                    "packet_loss": {"metric": "packet_loss_percent", "threshold": 1, "operator": ">", "severity": "warning", "enabled": True},
                    "mos_low": {"metric": "mos_avg", "threshold": 3.5, "operator": "<", "severity": "critical", "enabled": True}
                },
                "interval_seconds": 60,
                "retention_days": 30,
                "enabled": True
            },
            {
                "component": "mariadb",
                "thresholds": {
                    "conn_usage_critical": {"metric": "connection_usage_percent", "threshold": 90, "operator": ">", "severity": "critical", "enabled": True},
                    "conn_usage_warning": {"metric": "connection_usage_percent", "threshold": 75, "operator": ">", "severity": "warning", "enabled": True},
                    "slow_queries": {"metric": "slow_queries", "threshold": 10, "operator": ">", "severity": "warning", "enabled": True},
                    "deadlocks": {"metric": "innodb_deadlocks", "threshold": 5, "operator": ">", "severity": "critical", "enabled": True},
                    "replication_lag": {"metric": "seconds_behind_master", "threshold": 30, "operator": ">", "severity": "warning", "enabled": True}
                },
                "interval_seconds": 60,
                "retention_days": 30,
                "enabled": True
            },
            {
                "component": "system",
                "thresholds": {
                    "cpu_critical": {"metric": "cpu_percent", "threshold": 85, "operator": ">", "severity": "critical", "enabled": True},
                    "cpu_warning": {"metric": "cpu_percent", "threshold": 70, "operator": ">", "severity": "warning", "enabled": True},
                    "memory_critical": {"metric": "memory_percent", "threshold": 90, "operator": ">", "severity": "critical", "enabled": True},
                    "memory_warning": {"metric": "memory_percent", "threshold": 80, "operator": ">", "severity": "warning", "enabled": True},
                    "disk_critical": {"metric": "disk_percent", "threshold": 90, "operator": ">", "severity": "critical", "enabled": True},
                    "disk_warning": {"metric": "disk_percent", "threshold": 80, "operator": ">", "severity": "warning", "enabled": True},
                    "load_high": {"metric": "load_avg_1m", "threshold": 16, "operator": ">", "severity": "warning", "enabled": True}
                },
                "interval_seconds": 60,
                "retention_days": 30,
                "enabled": True
            }
        ]
        
        for config_data in default_configs:
            existing = db.query(MonitoringConfig).filter(
                MonitoringConfig.component == config_data["component"]
            ).first()
            
            if not existing:
                config = MonitoringConfig(**config_data)
                db.add(config)
                print(f"Added default config for {config_data['component']}")
            else:
                print(f"Config for {config_data['component']} already exists")
        
        db.commit()
        print("✅ Default monitoring configs inserted")
        
    except Exception as e:
        db.rollback()
        print(f"❌ Error inserting default configs: {e}")
        raise
    finally:
        db.close()

def main():
    print("=" * 60)
    print("ApoloBilling Monitoring Module - Database Migration")
    print("=" * 60)
    
    print("\n1. Creating monitoring tables...")
    create_monitoring_tables()
    
    print("\n2. Inserting default configurations...")
    insert_default_configs()
    
    print("\n" + "=" * 60)
    print("✅ Migration completed successfully!")
    print("=" * 60)
    print("\nNext steps:")
    print("1. Restart the ApoloBilling backend service")
    print("2. Access monitoring at /monitoring in the frontend")
    print("3. Configure alert webhooks in monitoring_config table if needed")

if __name__ == "__main__":
    main()