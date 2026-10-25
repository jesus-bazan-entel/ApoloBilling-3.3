-- Migration: NOC dashboard annotations
-- Date: 2026-10-24
-- Description: Marks drawn on the dashboard's traffic chart (incidents,
--              campaigns, maintenance windows, route migrations) so a spike or
--              a drop has its context next to it. Managed from the dashboard
--              (admin+) via POST/DELETE /api/v1/stats/noc/events.
--
--              Thresholds and capacity limits shown on the dashboard live in
--              system_settings (all optional, defaults in parentheses):
--                noc_asr_warn (50) / noc_asr_crit (40)   ASR % levels
--                noc_ner_warn (99) / noc_ner_crit (97)   NER % levels
--                noc_min_range_attempts (20)             DID ranges shown by default
--                noc_max_channels / noc_max_cps          capacity (unset = not shown)

CREATE TABLE IF NOT EXISTS noc_events (
    id          BIGSERIAL PRIMARY KEY,
    kind        VARCHAR(20)  NOT NULL DEFAULT 'other'
                CHECK (kind IN ('incident', 'campaign', 'maintenance', 'migration', 'other')),
    title       VARCHAR(120) NOT NULL,
    starts_at   TIMESTAMPTZ  NOT NULL,
    ends_at     TIMESTAMPTZ,
    created_by  VARCHAR(100),
    created_at  TIMESTAMPTZ  NOT NULL DEFAULT now(),
    CHECK (ends_at IS NULL OR ends_at >= starts_at)
);

CREATE INDEX IF NOT EXISTS idx_noc_events_starts_at ON noc_events (starts_at);

-- The installer runs migrations as postgres; the backend connects as its own
-- role (apolo_user on fresh installs, apolo on older ones).
DO $$
DECLARE r TEXT;
BEGIN
    FOREACH r IN ARRAY ARRAY['apolo_user', 'apolo'] LOOP
        IF EXISTS (SELECT 1 FROM pg_roles WHERE rolname = r) THEN
            EXECUTE format('GRANT SELECT, INSERT, DELETE ON noc_events TO %I', r);
            EXECUTE format('GRANT USAGE ON SEQUENCE noc_events_id_seq TO %I', r);
        END IF;
    END LOOP;
END $$;
