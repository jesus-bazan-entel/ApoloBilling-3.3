-- Migration: live media quality and PDD for the NOC dashboard
-- Date: 2026-10-24
-- Description:
--   1. cdrs.pdd_ms — Post-Dial Delay (INVITE → first 180/183) in ms, written by
--      the billing engine from FreeSWITCH's variable_start_uepoch and
--      variable_progress[_media]_uepoch. NULL for calls that never rang and
--      for direct Kamailio CDRs (acc_cdrs has no progress time).
--   2. Read access for the backend role to the monitoring collector's 1-minute
--      samples, so the dashboard shows the last 15 minutes instead of the last
--      completed hourly rollup (which lagged up to ~2 h).

-- Nullable without default: metadata-only on the partitioned table, but it
-- still needs a short exclusive lock while the engine is inserting CDRs.
SET lock_timeout = '5s';
ALTER TABLE cdrs ADD COLUMN IF NOT EXISTS pdd_ms INTEGER;
RESET lock_timeout;

COMMENT ON COLUMN cdrs.pdd_ms IS
    'Post-Dial Delay in ms: INVITE to first 180/183 (FreeSWITCH path only)';

DO $$
DECLARE r TEXT;
BEGIN
    FOREACH r IN ARRAY ARRAY['apolo_user', 'apolo'] LOOP
        IF EXISTS (SELECT 1 FROM pg_roles WHERE rolname = r) THEN
            IF to_regclass('public.monitoring_rtpengine_metrics') IS NOT NULL THEN
                EXECUTE format('GRANT SELECT ON monitoring_rtpengine_metrics TO %I', r);
            END IF;
            IF to_regclass('public.monitoring_kamailio_metrics') IS NOT NULL THEN
                EXECUTE format('GRANT SELECT ON monitoring_kamailio_metrics TO %I', r);
            END IF;
        END IF;
    END LOOP;
END $$;
