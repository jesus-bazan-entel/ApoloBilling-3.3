-- 019_remove_balance_billing.sql
--
-- Removes the legacy dollar/balance billing model entirely. Every account
-- and plan now bills exclusively via the minute-bucket model (Plan Control
-- / Plan Abierto) introduced in 018_minute_bucket_billing.sql. Applied with
-- zero real accounts on the balance model (verified before running).

BEGIN;

-- Views/tables that depend on the columns/tables being dropped.
DROP VIEW IF EXISTS v_available_balance;
DROP TABLE IF EXISTS balance_transactions;
DROP TABLE IF EXISTS balance_reservations CASCADE;

-- accounts: drop balance-model columns, make bucket config mandatory.
ALTER TABLE accounts
    DROP CONSTRAINT IF EXISTS accounts_billing_model_check,
    DROP COLUMN IF EXISTS billing_model,
    DROP COLUMN IF EXISTS account_type,
    DROP COLUMN IF EXISTS balance,
    DROP COLUMN IF EXISTS currency,
    DROP COLUMN IF EXISTS credit_limit,
    ALTER COLUMN bucket_type SET NOT NULL;

-- plans: same.
ALTER TABLE plans
    DROP CONSTRAINT IF EXISTS plans_billing_model_check,
    DROP CONSTRAINT IF EXISTS plans_account_type_check,
    DROP COLUMN IF EXISTS billing_model,
    DROP COLUMN IF EXISTS account_type,
    DROP COLUMN IF EXISTS initial_balance,
    DROP COLUMN IF EXISTS credit_limit,
    ALTER COLUMN bucket_type SET NOT NULL;

COMMIT;
