-- Migration: Per-user module grants + backfill for existing accounts
-- Date: 2026-09-23
-- Description: usuario_modulos holds explicit module grants for accounts with
--              role = 'usuario'. Admin/superadmin access is computed from role,
--              not from this table (see UserRole::effective_modules in apolo-core).
--
--              The backfill below grants every existing 'usuario' account (just
--              renamed from 'operator' in migration 015) the same set of modules
--              it could already see before this change, so nobody loses access on
--              cutover. Admin/Superadmin can tighten this afterwards per user.

BEGIN;

CREATE TABLE IF NOT EXISTS usuario_modulos (
    id SERIAL PRIMARY KEY,
    usuario_id INTEGER NOT NULL REFERENCES usuarios(id) ON DELETE CASCADE,
    module_key VARCHAR(50) NOT NULL REFERENCES system_modules(module_key) ON DELETE CASCADE,
    otorgado_por INTEGER REFERENCES usuarios(id) ON DELETE SET NULL,
    otorgado_en TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (usuario_id, module_key)
);

CREATE INDEX IF NOT EXISTS idx_usuario_modulos_usuario ON usuario_modulos(usuario_id);

-- Backfill: preserve pre-existing visibility for accounts that used to be 'operator'.
INSERT INTO usuario_modulos (usuario_id, module_key, otorgado_por)
SELECT u.id, m.module_key, NULL
FROM usuarios u
CROSS JOIN system_modules m
WHERE u.role = 'usuario'
  AND m.module_key IN (
      'dashboard', 'active_calls', 'cdr',
      'accounts', 'plans', 'balance',
      'zones', 'rates'
  )
ON CONFLICT (usuario_id, module_key) DO NOTHING;

COMMIT;
