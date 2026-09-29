-- Migration: Module catalog for per-user module visibility
-- Date: 2026-09-23
-- Description: system_modules is the configurable catalog of menu/route modules.
--              min_role is the minimum role that can access the module WITHOUT an
--              explicit grant:
--                - 'usuario'    -> visible to admin/superadmin always; usuario needs
--                                  an explicit grant in usuario_modulos (migration 017)
--                - 'admin'      -> visible to admin/superadmin, never assignable to usuario
--                - 'superadmin' -> exclusive to factory support, never assignable
--              always_visible modules (dashboard) are shown to every authenticated
--              user regardless of role or grants, so nobody lands on a blank screen
--              right after login.

BEGIN;

CREATE TABLE IF NOT EXISTS system_modules (
    id SERIAL PRIMARY KEY,
    module_key VARCHAR(50) UNIQUE NOT NULL,
    nombre VARCHAR(100) NOT NULL,
    seccion VARCHAR(50) NOT NULL,
    min_role VARCHAR(20) NOT NULL DEFAULT 'usuario'
        CHECK (min_role IN ('usuario', 'admin', 'superadmin')),
    always_visible BOOLEAN NOT NULL DEFAULT false,
    orden INTEGER NOT NULL DEFAULT 0,
    activo BOOLEAN NOT NULL DEFAULT true
);

CREATE INDEX IF NOT EXISTS idx_system_modules_activo ON system_modules(activo);

INSERT INTO system_modules (module_key, nombre, seccion, min_role, always_visible, orden) VALUES
    ('dashboard',         'Panel de Control',          'Principal',         'usuario',    true,  10),
    ('active_calls',      'Llamadas Activas',          'Principal',         'usuario',    false, 20),
    ('cdr',               'Registros CDR',             'Principal',         'usuario',    false, 30),
    ('accounts',          'Cuentas',                   'Facturacion',       'usuario',    false, 40),
    ('plans',             'Planes',                    'Facturacion',       'usuario',    false, 50),
    ('balance',           'Saldos',                    'Facturacion',       'usuario',    false, 60),
    ('zones',             'Zonas',                      'Tarifas',          'usuario',    false, 70),
    ('rates',             'Tarifas',                    'Tarifas',          'usuario',    false, 80),
    ('sip_devices',       'Dispositivos SIP',           'Operacion',        'usuario',    false, 90),
    ('inbound_routing',   'Rutas Entrantes',            'Operacion',        'usuario',    false, 100),
    ('outbound_routing',  'Rutas Salientes',            'Operacion',        'usuario',    false, 110),
    ('carriers',          'Carriers',                   'Operacion',        'usuario',    false, 120),
    ('carrier_groups',    'Carrier Groups',              'Operacion',       'usuario',    false, 130),
    ('endpoints',         'Endpoints',                   'Operacion',       'usuario',    false, 140),
    ('endpoint_monitor',  'Monitor de Endpoints',        'Operacion',       'usuario',    false, 150),
    ('audit_logs',        'Auditoria',                   'Sistema',         'admin',      false, 160),
    ('users',             'Usuarios y Permisos',         'Sistema',         'admin',      false, 170),
    ('dialplan',          'Dialplan FreeSWITCH (XML)',   'Sistema Avanzado','superadmin', false, 180),
    ('legacy_routing',    'Routing Legacy',              'Sistema Avanzado','superadmin', false, 190),
    ('system_settings',   'Configuracion del Sistema',   'Sistema Avanzado','superadmin', false, 200)
ON CONFLICT (module_key) DO NOTHING;

COMMIT;
