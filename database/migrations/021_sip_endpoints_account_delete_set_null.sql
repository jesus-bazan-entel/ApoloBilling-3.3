-- ============================================
-- Migration 021: Fix sip_endpoints.account_id delete behavior
--
-- Migration 020 created sip_endpoints.account_id without an ON DELETE
-- clause, which defaults to NO ACTION/RESTRICT in Postgres - this silently
-- blocked deleting any account that still had a linked SIP endpoint
-- (extension or trunk), with a bare foreign key violation and no helpful
-- error message on the way up. Deleting a billing account should only
-- unlink its SIP endpoints, not delete the physical device/trunk config
-- itself - the same ON DELETE SET NULL behavior cdrs.account_id already
-- uses.
-- ============================================

BEGIN;

ALTER TABLE sip_endpoints
    DROP CONSTRAINT sip_endpoints_account_id_fkey,
    ADD CONSTRAINT sip_endpoints_account_id_fkey
        FOREIGN KEY (account_id) REFERENCES accounts(id) ON DELETE SET NULL;

COMMIT;
