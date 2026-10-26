-- Migration: Partition cdrs by month (start_time)
-- Date: 2026-08-09
-- Description: cdrs was a single monolithic table (4.9M rows / 2.5GB and
--              growing ~43k rows/day) that had NEVER been vacuumed/analyzed,
--              causing stale planner stats and full sequential scans on
--              billing queries. Converts it to native range partitioning by
--              month on start_time, which keeps autovacuum/analyze cheap per
--              partition and enables partition pruning on date-ranged
--              queries/reports.
--
-- Schema note: Postgres requires the partition key (start_time) to be part
-- of any PRIMARY KEY / UNIQUE constraint on a partitioned table, so:
--   PRIMARY KEY (id)         -> PRIMARY KEY (id, start_time)
--   UNIQUE (call_uuid)       -> UNIQUE (call_uuid, start_time)
-- In practice this changes nothing: `id` still comes from a single global
-- sequence (cdrs_id_seq) and `call_uuid` is still one-per-call. No call site
-- in rust-billing-engine or rust-backend relies on those constraints by name
-- or does INSERT ... ON CONFLICT against them (verified 2026-08-09) - the
-- only write paths are plain INSERT/UPDATE/DELETE by `id`.
--
-- On THIS production database this migration was already applied by hand on
-- 2026-08-09 (online, with a batched backfill from the old monolithic table -
-- see /var/backups/postgres-cdrs/ for the pre-migration pg_dump and
-- cdrs_monolith_backup for the retained old table). The block below is a
-- no-op there (guarded by the relkind check) and exists so that a FRESH
-- database provisioned from these migrations ends up with the same
-- partitioned schema instead of silently drifting back to a flat table.

DO $$
BEGIN
    IF EXISTS (SELECT 1 FROM pg_class WHERE relname = 'cdrs' AND relkind = 'p') THEN
        RAISE NOTICE 'cdrs is already partitioned - skipping (already applied, e.g. on prod via the 2026-08-09 online migration).';
        RETURN;
    END IF;

    IF EXISTS (SELECT 1 FROM cdrs LIMIT 1) THEN
        RAISE EXCEPTION 'cdrs exists, is NOT partitioned, and already has data. '
            'This migration only auto-partitions an EMPTY cdrs table (fresh '
            'install). For a populated table, do an online migration with a '
            'batched backfill instead of running this blindly - see the '
            '2026-08-09 runbook / conversation history for the exact steps '
            '(create cdrs_new partitioned, backfill in batches by id, stop '
            'writers briefly, delta-backfill, rename swap, restart writers).';
    END IF;

    -- cdrs exists but is empty (fresh install) - safe to drop and recreate
    -- partitioned directly. schema.sql's cdrs.id is a serial that OWNS
    -- cdrs_id_seq: release it first or DROP TABLE takes the sequence along and
    -- the new table's nextval('cdrs_id_seq') aborts the whole block.
    ALTER SEQUENCE IF EXISTS cdrs_id_seq OWNED BY NONE;
    DROP TABLE cdrs;
    CREATE SEQUENCE IF NOT EXISTS cdrs_id_seq;

    CREATE TABLE cdrs (
        id                    bigint NOT NULL DEFAULT nextval('cdrs_id_seq'::regclass),
        call_uuid             character varying(100) NOT NULL,
        account_id            integer,
        caller_number         character varying(50) NOT NULL,
        called_number         character varying(50) NOT NULL,
        destination_prefix    character varying(20),
        start_time            timestamp with time zone NOT NULL,
        answer_time           timestamp with time zone,
        end_time              timestamp with time zone NOT NULL,
        duration              integer NOT NULL DEFAULT 0,
        billsec               integer NOT NULL DEFAULT 0,
        rate_per_minute       numeric(10,6),
        cost                  numeric(12,4) DEFAULT 0.0000,
        hangup_cause          character varying(50),
        direction             character varying(20),
        freeswitch_server_id  character varying(100),
        reservation_id        uuid,
        created_at            timestamp with time zone DEFAULT now(),
        processed_at          timestamp with time zone,
        rate_id               integer,
        CONSTRAINT cdrs_pkey PRIMARY KEY (id, start_time),
        CONSTRAINT cdrs_call_uuid_start_time_key UNIQUE (call_uuid, start_time),
        CONSTRAINT cdrs_account_id_fkey FOREIGN KEY (account_id) REFERENCES accounts(id) ON DELETE SET NULL,
        CONSTRAINT cdrs_reservation_id_fkey FOREIGN KEY (reservation_id) REFERENCES balance_reservations(id) ON DELETE SET NULL
    ) PARTITION BY RANGE (start_time);

    CREATE INDEX idx_cdrs_account_id ON cdrs (account_id);
    CREATE INDEX idx_cdr_account_start ON cdrs (account_id, start_time);
    CREATE INDEX idx_cdr_callee ON cdrs (called_number);
    CREATE INDEX idx_cdrs_caller ON cdrs (caller_number);
    CREATE INDEX idx_cdr_reservation ON cdrs (reservation_id);
    CREATE INDEX idx_cdrs_start_time ON cdrs (start_time);

    -- Starter partitions; extend forward over time (see housekeeping note
    -- below) - DEFAULT catches anything outside the declared ranges instead
    -- of failing inserts.
    ALTER SEQUENCE cdrs_id_seq OWNED BY cdrs.id;

    CREATE TABLE cdrs_y2026m04 PARTITION OF cdrs FOR VALUES FROM ('2026-04-01') TO ('2026-05-01');
    CREATE TABLE cdrs_y2026m05 PARTITION OF cdrs FOR VALUES FROM ('2026-05-01') TO ('2026-06-01');
    CREATE TABLE cdrs_y2026m06 PARTITION OF cdrs FOR VALUES FROM ('2026-06-01') TO ('2026-07-01');
    CREATE TABLE cdrs_y2026m07 PARTITION OF cdrs FOR VALUES FROM ('2026-07-01') TO ('2026-08-01');
    CREATE TABLE cdrs_y2026m08 PARTITION OF cdrs FOR VALUES FROM ('2026-08-01') TO ('2026-09-01');
    CREATE TABLE cdrs_y2026m09 PARTITION OF cdrs FOR VALUES FROM ('2026-09-01') TO ('2026-10-01');
    CREATE TABLE cdrs_y2026m10 PARTITION OF cdrs FOR VALUES FROM ('2026-10-01') TO ('2026-11-01');
    CREATE TABLE cdrs_default PARTITION OF cdrs DEFAULT;
END $$;

-- Housekeeping TODO (not automated yet): a new monthly partition needs to
-- exist before that month starts, or inserts fall into cdrs_default. Either
-- add a monthly cron (`CREATE TABLE cdrs_yYYYYmMM PARTITION OF cdrs FOR
-- VALUES FROM (...) TO (...)`) or install pg_partman to manage this.
