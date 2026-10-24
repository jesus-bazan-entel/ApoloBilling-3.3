-- Migration: Add provisioning metadata fields to accounts
-- Date: 2026-06-12
-- Description: Adds customer-order metadata columns used by the Provisioning module
--              (bulk account + SIP device creation from a CSV/Excel template).
--              CLIENTE is stored in the existing (previously unused) accounts.account_name
--              column; this migration adds the remaining template fields:
--              cod_ped (order code, mirrored to the SIP device display_name),
--              departamento, provincia, distrito (customer location).

ALTER TABLE accounts
ADD COLUMN IF NOT EXISTS cod_ped VARCHAR(50),
ADD COLUMN IF NOT EXISTS departamento VARCHAR(100),
ADD COLUMN IF NOT EXISTS provincia VARCHAR(100),
ADD COLUMN IF NOT EXISTS distrito VARCHAR(100);

COMMENT ON COLUMN accounts.account_name IS 'Customer name (CLIENTE) - populated by the provisioning module';
COMMENT ON COLUMN accounts.cod_ped IS 'Order code (COD_PED) from the provisioning template; also used as the SIP device display_name';
COMMENT ON COLUMN accounts.departamento IS 'Customer location: department/region';
COMMENT ON COLUMN accounts.provincia IS 'Customer location: province';
COMMENT ON COLUMN accounts.distrito IS 'Customer location: district';
