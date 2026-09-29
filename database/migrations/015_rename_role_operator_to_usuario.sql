-- Migration: Rename role 'operator' to 'usuario'
-- Date: 2026-09-23
-- Description: Part of the new user management model (Superadmin / Admin / Usuario).
--              'operator' had no real business meaning; it becomes 'usuario' (staff
--              from other areas of the client company, with module-level visibility
--              controlled by Admin/Superadmin — see migrations 016/017).
--              The Rust backend still accepts "operator" as a parse alias for one
--              release, but always writes/reads "usuario" going forward.

BEGIN;

UPDATE usuarios SET role = 'usuario' WHERE role = 'operator';

ALTER TABLE usuarios ALTER COLUMN role SET DEFAULT 'usuario';

COMMIT;
