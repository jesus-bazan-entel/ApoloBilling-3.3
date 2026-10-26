-- Migration: monthly cdrs partitions created ahead of time
-- Date: 2026-10-26
-- Description: 016 declared partitions only up to 2026-10 plus cdrs_default
--              and left "create next month's partition" as a TODO. Without it
--              every CDR from 2026-11-01 lands in cdrs_default, and a later
--              CREATE TABLE ... PARTITION OF for that month fails while the
--              default partition holds rows of its range.
--
-- apolo_ensure_cdr_partitions(months_ahead) creates cdrs_yYYYYmMM for the
-- current month and the next `months_ahead` months when missing. Rows of that
-- range already sitting in cdrs_default are moved into the new partition in
-- the same transaction (create detached -> move -> ATTACH), so it also repairs
-- a month that started without its partition. Idempotent; run daily by
-- /etc/cron.d/apolo-cdr-partitions (install_server.sh).
--
-- Bounds are Lima midnights, like 016's ('2026-10-01' read in America/Lima ->
-- '2026-10-01 00:00:00-05'), whatever the calling session's TimeZone is.
--
-- SECURITY DEFINER so the cron (as postgres) creates partitions owned by the
-- function owner, i.e. the same role that owns cdrs.

CREATE OR REPLACE FUNCTION apolo_ensure_cdr_partitions(months_ahead integer DEFAULT 3)
RETURNS integer
LANGUAGE plpgsql
SECURITY DEFINER
SET search_path = public
AS $$
DECLARE
    tz      constant text := 'America/Lima';
    m_month date;
    m_start timestamptz;
    m_end   timestamptz;
    part    text;
    moved   bigint;
    created integer := 0;
BEGIN
    IF NOT EXISTS (SELECT 1 FROM pg_class WHERE relname = 'cdrs' AND relkind = 'p') THEN
        RAISE NOTICE 'cdrs is not partitioned - nothing to do';
        RETURN 0;
    END IF;

    FOR i IN 0..months_ahead LOOP
        m_month := (date_trunc('month', now() AT TIME ZONE tz) + make_interval(months => i))::date;
        m_start := m_month::timestamp AT TIME ZONE tz;
        m_end   := (m_month + interval '1 month')::timestamp AT TIME ZONE tz;
        part    := format('cdrs_y%sm%s', to_char(m_month, 'YYYY'), to_char(m_month, 'MM'));

        CONTINUE WHEN EXISTS (SELECT 1 FROM pg_class WHERE relname = part);

        EXECUTE format('CREATE TABLE %I (LIKE cdrs INCLUDING DEFAULTS INCLUDING CONSTRAINTS)', part);
        IF EXISTS (SELECT 1 FROM pg_class WHERE relname = 'cdrs_default') THEN
            EXECUTE format(
                'WITH m AS (DELETE FROM cdrs_default WHERE start_time >= %L AND start_time < %L RETURNING *)
                 INSERT INTO %I SELECT * FROM m', m_start, m_end, part);
            GET DIAGNOSTICS moved = ROW_COUNT;
        ELSE
            moved := 0;
        END IF;
        EXECUTE format('ALTER TABLE cdrs ATTACH PARTITION %I FOR VALUES FROM (%L) TO (%L)',
                       part, m_start, m_end);
        RAISE NOTICE 'created partition % [% , %) - % rows moved from cdrs_default', part, m_start, m_end, moved;
        created := created + 1;
    END LOOP;

    RETURN created;
END;
$$;

SELECT apolo_ensure_cdr_partitions(3);
