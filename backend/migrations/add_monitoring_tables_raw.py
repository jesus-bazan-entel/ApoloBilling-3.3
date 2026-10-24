"""
Database migration for monitoring tables - Raw SQL version
"""
import sys
import os

# Force correct DATABASE_URL before importing anything else
os.environ['DATABASE_URL'] = "postgresql://apolo_user:apolo_password_2024@127.0.0.1:5432/apolo_billing"

import psycopg

def create_monitoring_tables():
    """Create monitoring tables using raw SQL"""
    conn = psycopg.connect(os.environ['DATABASE_URL'], autocommit=True)
    cur = conn.cursor()
    
    try:
        # Drop existing objects first
        cur.execute("""
            DROP TABLE IF EXISTS monitoring_kamailio_metrics CASCADE;
            DROP TABLE IF EXISTS monitoring_rtpengine_metrics CASCADE;
            DROP TABLE IF EXISTS monitoring_mariadb_metrics CASCADE;
            DROP TABLE IF EXISTS monitoring_system_metrics CASCADE;
            DROP TABLE IF EXISTS monitoring_alerts CASCADE;
            DROP TABLE IF EXISTS monitoring_config CASCADE;
        """)
        
        # Create tables
        
        # Kamailio metrics
        cur.execute("""
            CREATE TABLE monitoring_kamailio_metrics (
                id BIGSERIAL PRIMARY KEY,
                timestamp TIMESTAMP WITHOUT TIME ZONE NOT NULL DEFAULT NOW(),
                active_dialogs INTEGER DEFAULT 0,
                active_calls INTEGER DEFAULT 0,
                early_dialogs INTEGER DEFAULT 0,
                confirmed_dialogs INTEGER DEFAULT 0,
                invite_received INTEGER DEFAULT 0,
                invite_sent INTEGER DEFAULT 0,
                invite_200ok INTEGER DEFAULT 0,
                invite_4xx INTEGER DEFAULT 0,
                invite_5xx INTEGER DEFAULT 0,
                bye_received INTEGER DEFAULT 0,
                cancel_received INTEGER DEFAULT 0,
                ack_received INTEGER DEFAULT 0,
                options_received INTEGER DEFAULT 0,
                register_received INTEGER DEFAULT 0,
                register_200ok INTEGER DEFAULT 0,
                dispatcher_active_destinations INTEGER DEFAULT 0,
                dispatcher_inactive_destinations INTEGER DEFAULT 0,
                trunks_online INTEGER DEFAULT 0,
                trunks_total INTEGER DEFAULT 0,
                active_transactions INTEGER DEFAULT 0,
                transaction_timeouts INTEGER DEFAULT 0,
                shm_used_mb FLOAT DEFAULT 0.0,
                shm_free_mb FLOAT DEFAULT 0.0,
                shm_fragments INTEGER DEFAULT 0,
                pkg_used_mb FLOAT DEFAULT 0.0,
                parse_errors INTEGER DEFAULT 0,
                script_errors INTEGER DEFAULT 0,
                db_errors INTEGER DEFAULT 0,
                raw_data JSONB
            );
            CREATE INDEX ix_kamailio_metrics_timestamp ON monitoring_kamailio_metrics (timestamp);
        """)
        
        # RTPengine metrics
        cur.execute("""
            CREATE TABLE monitoring_rtpengine_metrics (
                id BIGSERIAL PRIMARY KEY,
                timestamp TIMESTAMP WITHOUT TIME ZONE NOT NULL DEFAULT NOW(),
                active_sessions INTEGER DEFAULT 0,
                active_calls INTEGER DEFAULT 0,
                one_sided_sessions INTEGER DEFAULT 0,
                deleted_sessions INTEGER DEFAULT 0,
                ports_used INTEGER DEFAULT 0,
                ports_free INTEGER DEFAULT 0,
                port_usage_percent FLOAT DEFAULT 0.0,
                packets_in BIGINT DEFAULT 0,
                packets_out BIGINT DEFAULT 0,
                bytes_in BIGINT DEFAULT 0,
                bytes_out BIGINT DEFAULT 0,
                packets_lost_in BIGINT DEFAULT 0,
                packets_lost_out BIGINT DEFAULT 0,
                jitter_avg_ms FLOAT DEFAULT 0.0,
                rtt_avg_ms FLOAT DEFAULT 0.0,
                mos_avg FLOAT DEFAULT 0.0,
                offer_errors INTEGER DEFAULT 0,
                answer_errors INTEGER DEFAULT 0,
                delete_errors INTEGER DEFAULT 0,
                timeout_errors INTEGER DEFAULT 0,
                cpu_percent FLOAT DEFAULT 0.0,
                memory_mb FLOAT DEFAULT 0.0,
                threads INTEGER DEFAULT 0,
                raw_data JSONB
            );
            CREATE INDEX ix_rtpengine_metrics_timestamp ON monitoring_rtpengine_metrics (timestamp);
        """)
        
        # MariaDB metrics
        cur.execute("""
            CREATE TABLE monitoring_mariadb_metrics (
                id BIGSERIAL PRIMARY KEY,
                timestamp TIMESTAMP WITHOUT TIME ZONE NOT NULL DEFAULT NOW(),
                threads_connected INTEGER DEFAULT 0,
                threads_running INTEGER DEFAULT 0,
                max_used_connections INTEGER DEFAULT 0,
                aborted_connects INTEGER DEFAULT 0,
                connection_errors INTEGER DEFAULT 0,
                questions BIGINT DEFAULT 0,
                slow_queries INTEGER DEFAULT 0,
                queries_per_second FLOAT DEFAULT 0.0,
                innodb_buffer_pool_used_mb FLOAT DEFAULT 0.0,
                innodb_buffer_pool_free_mb FLOAT DEFAULT 0.0,
                innodb_buffer_pool_pages_dirty INTEGER DEFAULT 0,
                innodb_row_lock_waits INTEGER DEFAULT 0,
                innodb_deadlocks INTEGER DEFAULT 0,
                slave_sql_running BOOLEAN,
                slave_io_running BOOLEAN,
                seconds_behind_master INTEGER,
                created_tmp_disk_tables INTEGER DEFAULT 0,
                created_tmp_tables INTEGER DEFAULT 0,
                raw_data JSONB
            );
            CREATE INDEX ix_mariadb_metrics_timestamp ON monitoring_mariadb_metrics (timestamp);
        """)
        
        # System metrics
        cur.execute("""
            CREATE TABLE monitoring_system_metrics (
                id BIGSERIAL PRIMARY KEY,
                timestamp TIMESTAMP WITHOUT TIME ZONE NOT NULL DEFAULT NOW(),
                hostname VARCHAR(255) DEFAULT 'winproxy',
                cpu_percent FLOAT DEFAULT 0.0,
                cpu_user_percent FLOAT DEFAULT 0.0,
                cpu_system_percent FLOAT DEFAULT 0.0,
                cpu_iowait_percent FLOAT DEFAULT 0.0,
                load_avg_1m FLOAT DEFAULT 0.0,
                load_avg_5m FLOAT DEFAULT 0.0,
                load_avg_15m FLOAT DEFAULT 0.0,
                cpu_cores INTEGER DEFAULT 0,
                memory_total_mb FLOAT DEFAULT 0.0,
                memory_used_mb FLOAT DEFAULT 0.0,
                memory_free_mb FLOAT DEFAULT 0.0,
                memory_available_mb FLOAT DEFAULT 0.0,
                memory_percent FLOAT DEFAULT 0.0,
                swap_total_mb FLOAT DEFAULT 0.0,
                swap_used_mb FLOAT DEFAULT 0.0,
                swap_percent FLOAT DEFAULT 0.0,
                disk_total_gb FLOAT DEFAULT 0.0,
                disk_used_gb FLOAT DEFAULT 0.0,
                disk_free_gb FLOAT DEFAULT 0.0,
                disk_percent FLOAT DEFAULT 0.0,
                disk_read_mbps FLOAT DEFAULT 0.0,
                disk_write_mbps FLOAT DEFAULT 0.0,
                disk_iops_read INTEGER DEFAULT 0,
                disk_iops_write INTEGER DEFAULT 0,
                net_interface VARCHAR(64) DEFAULT 'eth0',
                net_rx_mbps FLOAT DEFAULT 0.0,
                net_tx_mbps FLOAT DEFAULT 0.0,
                net_rx_pps INTEGER DEFAULT 0,
                net_tx_pps INTEGER DEFAULT 0,
                net_errors INTEGER DEFAULT 0,
                net_drops INTEGER DEFAULT 0,
                kamailio_cpu_percent FLOAT DEFAULT 0.0,
                kamailio_memory_mb FLOAT DEFAULT 0.0,
                kamailio_threads INTEGER DEFAULT 0,
                rtpengine_cpu_percent FLOAT DEFAULT 0.0,
                rtpengine_memory_mb FLOAT DEFAULT 0.0,
                freeswitch_cpu_percent FLOAT DEFAULT 0.0,
                freeswitch_memory_mb FLOAT DEFAULT 0.0,
                postgres_cpu_percent FLOAT DEFAULT 0.0,
                postgres_memory_mb FLOAT DEFAULT 0.0,
                raw_data JSONB
            );
            CREATE INDEX ix_system_metrics_timestamp ON monitoring_system_metrics (timestamp);
        """)
        
        # Alerts
        cur.execute("""
            CREATE TABLE monitoring_alerts (
                id BIGSERIAL PRIMARY KEY,
                timestamp TIMESTAMP WITHOUT TIME ZONE NOT NULL DEFAULT NOW(),
                alert_name VARCHAR(128) NOT NULL,
                severity VARCHAR(16) NOT NULL,
                component VARCHAR(64) NOT NULL,
                metric_name VARCHAR(128) NOT NULL,
                threshold_value FLOAT NOT NULL,
                current_value FLOAT NOT NULL,
                operator VARCHAR(8) NOT NULL,
                status VARCHAR(16) DEFAULT 'firing',
                acknowledged_by VARCHAR(128),
                acknowledged_at TIMESTAMP WITHOUT TIME ZONE,
                resolved_at TIMESTAMP WITHOUT TIME ZONE,
                message TEXT,
                notification_sent BOOLEAN DEFAULT FALSE
            );
            CREATE INDEX ix_monitoring_alerts_timestamp ON monitoring_alerts (timestamp);
            CREATE INDEX ix_monitoring_alerts_component_status ON monitoring_alerts (component, status);
        """)
        
        # Config
        cur.execute("""
            CREATE TABLE monitoring_config (
                id BIGSERIAL PRIMARY KEY,
                component VARCHAR(64) NOT NULL UNIQUE,
                thresholds JSONB NOT NULL,
                interval_seconds INTEGER DEFAULT 60,
                retention_days INTEGER DEFAULT 30,
                enabled BOOLEAN DEFAULT TRUE,
                webhook_url VARCHAR(512),
                email_recipients JSONB,
                updated_at TIMESTAMP WITHOUT TIME ZONE DEFAULT NOW(),
                updated_by VARCHAR(128)
            );
        """)
        
        # Grant permissions
        cur.execute("""
            GRANT ALL ON ALL TABLES IN SCHEMA public TO apolo_user;
            GRANT ALL ON ALL SEQUENCES IN SCHEMA public TO apolo_user;
        """)
        
        print("✅ Monitoring tables created successfully")
        
        # Verify
        cur.execute("""
            SELECT table_name FROM information_schema.tables 
            WHERE table_schema = 'public' 
            AND table_name LIKE 'monitoring_%'
            ORDER BY table_name
        """)
        tables = [row[0] for row in cur.fetchall()]
        print(f"Created tables: {tables}")
        
    except Exception as e:
        print(f"❌ Error creating monitoring tables: {e}")
        raise
    finally:
        cur.close()
        conn.close()

def insert_default_configs():
    """Insert default monitoring configuration using raw SQL"""
    import json
    conn = psycopg.connect(os.environ['DATABASE_URL'], autocommit=True)
    cur = conn.cursor()
    
    try:
        default_configs = [
            ("kamailio", {
                "shm_high": {"metric": "shm_usage_percent", "threshold": 90, "operator": ">", "severity": "critical", "enabled": True},
                "shm_warning": {"metric": "shm_usage_percent", "threshold": 75, "operator": ">", "severity": "warning", "enabled": True},
                "active_calls_high": {"metric": "active_calls", "threshold": 1000, "operator": ">", "severity": "warning", "enabled": True},
                "success_rate_low": {"metric": "success_rate", "threshold": 95, "operator": "<", "severity": "critical", "enabled": True},
                "db_errors": {"metric": "db_errors", "threshold": 0, "operator": ">", "severity": "critical", "enabled": True}
            }, 60, 30, True, None, None),
            ("rtpengine", {
                "cpu_high": {"metric": "cpu_percent", "threshold": 80, "operator": ">", "severity": "critical", "enabled": True},
                "cpu_warning": {"metric": "cpu_percent", "threshold": 60, "operator": ">", "severity": "warning", "enabled": True},
                "port_usage_high": {"metric": "port_usage_percent", "threshold": 90, "operator": ">", "severity": "critical", "enabled": True},
                "port_usage_warning": {"metric": "port_usage_percent", "threshold": 75, "operator": ">", "severity": "warning", "enabled": True},
                "packet_loss": {"metric": "packet_loss_percent", "threshold": 1, "operator": ">", "severity": "warning", "enabled": True},
                "mos_low": {"metric": "mos_avg", "threshold": 3.5, "operator": "<", "severity": "critical", "enabled": True}
            }, 60, 30, True, None, None),
            ("mariadb", {
                "conn_usage_critical": {"metric": "connection_usage_percent", "threshold": 90, "operator": ">", "severity": "critical", "enabled": True},
                "conn_usage_warning": {"metric": "connection_usage_percent", "threshold": 75, "operator": ">", "severity": "warning", "enabled": True},
                "slow_queries": {"metric": "slow_queries", "threshold": 10, "operator": ">", "severity": "warning", "enabled": True},
                "deadlocks": {"metric": "innodb_deadlocks", "threshold": 5, "operator": ">", "severity": "critical", "enabled": True},
                "replication_lag": {"metric": "seconds_behind_master", "threshold": 30, "operator": ">", "severity": "warning", "enabled": True}
            }, 60, 30, True, None, None),
            ("system", {
                "cpu_critical": {"metric": "cpu_percent", "threshold": 85, "operator": ">", "severity": "critical", "enabled": True},
                "cpu_warning": {"metric": "cpu_percent", "threshold": 70, "operator": ">", "severity": "warning", "enabled": True},
                "memory_critical": {"metric": "memory_percent", "threshold": 90, "operator": ">", "severity": "critical", "enabled": True},
                "memory_warning": {"metric": "memory_percent", "threshold": 80, "operator": ">", "severity": "warning", "enabled": True},
                "disk_critical": {"metric": "disk_percent", "threshold": 90, "operator": ">", "severity": "critical", "enabled": True},
                "disk_warning": {"metric": "disk_percent", "threshold": 80, "operator": ">", "severity": "warning", "enabled": True},
                "load_high": {"metric": "load_avg_1m", "threshold": 16, "operator": ">", "severity": "warning", "enabled": True}
            }, 60, 30, True, None, None)
        ]
        
        for component, thresholds, interval, retention, enabled, webhook, email in default_configs:
            cur.execute("""
                INSERT INTO monitoring_config (component, thresholds, interval_seconds, retention_days, enabled, webhook_url, email_recipients)
                VALUES (%s, %s, %s, %s, %s, %s, %s)
                ON CONFLICT (component) DO UPDATE SET
                    thresholds = EXCLUDED.thresholds,
                    interval_seconds = EXCLUDED.interval_seconds,
                    retention_days = EXCLUDED.retention_days,
                    enabled = EXCLUDED.enabled,
                    webhook_url = EXCLUDED.webhook_url,
                    email_recipients = EXCLUDED.email_recipients,
                    updated_at = NOW()
            """, (component, json.dumps(thresholds), interval, retention, enabled, webhook, json.dumps(email) if email else None))
            print(f"  Added/updated config for {component}")
        
        print("✅ Default monitoring configs inserted")
        
    except Exception as e:
        print(f"❌ Error inserting default configs: {e}")
        raise
    finally:
        cur.close()
        conn.close()

def main():
    print("=" * 60)
    print("ApoloBilling Monitoring Module - Database Migration (Raw SQL)")
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