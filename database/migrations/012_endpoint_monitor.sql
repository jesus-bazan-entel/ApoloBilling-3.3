-- Migration: Endpoint Monitor Probes Table
-- Description: Creates table for storing SIP endpoint connectivity probe results
-- Date: 2026-08-16

-- Table for storing probe results from the endpoint monitor service
CREATE TABLE IF NOT EXISTS endpoint_monitor_probes (
    id BIGSERIAL PRIMARY KEY,
    ts BIGINT NOT NULL,                    -- Unix epoch UTC of the probe
    trunk_id UUID NOT NULL REFERENCES routing_trunks(id) ON DELETE CASCADE,
    trunk_name VARCHAR(128) NOT NULL DEFAULT '',
    trunk_type VARCHAR(20) NOT NULL DEFAULT 'public',
    ip VARCHAR(64) NOT NULL DEFAULT '',
    port SMALLINT NOT NULL DEFAULT 5060,
    status VARCHAR(10) NOT NULL DEFAULT 'down' CHECK (status IN ('up', 'down')),
    resp_code SMALLINT NOT NULL DEFAULT 0,  -- SIP response code (200/404/503...) or 0 for timeout
    rtt_ms DECIMAL(8,3),                    -- Round-trip time in milliseconds, NULL if down
    evidence TEXT                           -- Raw sipsak trace for debugging/audit
);

-- Index for time-based queries (dashboard, recent probes)
CREATE INDEX IF NOT EXISTS idx_endpoint_monitor_ts ON endpoint_monitor_probes(ts);

-- Index for trunk-specific history queries
CREATE INDEX IF NOT EXISTS idx_endpoint_monitor_trunk_ts ON endpoint_monitor_probes(trunk_id, ts);

-- Index for IP-based filtering
CREATE INDEX IF NOT EXISTS idx_endpoint_monitor_ip_ts ON endpoint_monitor_probes(ip, ts);

-- Index for status filtering (finding failures)
CREATE INDEX IF NOT EXISTS idx_endpoint_monitor_status_ts ON endpoint_monitor_probes(status, ts);

-- Comment on table
COMMENT ON TABLE endpoint_monitor_probes IS 'SIP endpoint connectivity probe results from the monitor service';
COMMENT ON COLUMN endpoint_monitor_probes.ts IS 'Unix epoch UTC timestamp of the probe';
COMMENT ON COLUMN endpoint_monitor_probes.status IS 'Probe result: up (any SIP response) or down (timeout/unreachable)';
COMMENT ON COLUMN endpoint_monitor_probes.resp_code IS 'SIP response code (200, 404, 503, etc.) or 0 if no response';
COMMENT ON COLUMN endpoint_monitor_probes.rtt_ms IS 'Round-trip time in milliseconds, NULL if endpoint is down';
COMMENT ON COLUMN endpoint_monitor_probes.evidence IS 'Raw sipsak output trace for audit/debugging';
