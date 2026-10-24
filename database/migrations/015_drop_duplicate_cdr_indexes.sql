-- Migration 015: Drop duplicate indexes on cdrs
--
-- schema.sql (a pg_dump snapshot) accumulated two overlapping naming
-- generations of the same indexes on public.cdrs:
--   idx_cdr_account     == idx_cdrs_account_id   (account_id)
--   idx_cdr_caller      == idx_cdrs_caller       (caller_number)
--   idx_cdr_start_time  == idx_cdrs_start_time   (start_time)
--   idx_cdr_uuid        == idx_cdrs_uuid         (call_uuid)
--
-- Every CDR insert (one per call) was maintaining both copies of each pair
-- for no query-planning benefit, and the extra index pages grow with the
-- table indefinitely. Keep the idx_cdrs_* generation (matches the naming
-- used by every other idx_cdrs_* index already in this table) and drop the
-- older idx_cdr_* duplicates. CONCURRENTLY avoids locking cdrs against
-- inserts from the billing engine while dropping.

DROP INDEX CONCURRENTLY IF EXISTS public.idx_cdr_account;
DROP INDEX CONCURRENTLY IF EXISTS public.idx_cdr_caller;
DROP INDEX CONCURRENTLY IF EXISTS public.idx_cdr_start_time;
DROP INDEX CONCURRENTLY IF EXISTS public.idx_cdr_uuid;
