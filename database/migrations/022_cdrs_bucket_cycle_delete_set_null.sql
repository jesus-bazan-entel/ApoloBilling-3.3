-- ============================================
-- Migration 022: Fix cdrs.bucket_cycle_id delete behavior
--
-- Deleting an account cascades to its minute_bucket_cycles rows (see
-- migration 018), but cdrs.bucket_cycle_id referenced those cycles with no
-- ON DELETE clause (NO ACTION/RESTRICT by default), so the cascade from
-- accounts -> minute_bucket_cycles got blocked the moment any CDR pointed
-- at that cycle - CDRs are historical records and must never block a
-- delete; they should just lose the back-reference, same as
-- cdrs.account_id already does.
-- ============================================

BEGIN;

ALTER TABLE cdrs
    DROP CONSTRAINT cdrs_bucket_cycle_id_fkey,
    ADD CONSTRAINT cdrs_bucket_cycle_id_fkey
        FOREIGN KEY (bucket_cycle_id) REFERENCES minute_bucket_cycles(id) ON DELETE SET NULL;

COMMIT;
