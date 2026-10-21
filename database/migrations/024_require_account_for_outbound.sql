-- 024_require_account_for_outbound.sql
-- Require account association for outbound calls
-- When enabled, outbound calls from endpoints without account_id are rejected

BEGIN;

INSERT INTO system_settings (key, value, description, updated_by) VALUES 
('require_account_for_outbound', 'false',
 'Require outbound calls to have an associated billing account via endpoint. If true, calls from endpoints without account_id are rejected.',
 'system')
ON CONFLICT (key) DO NOTHING;

COMMIT;