// Module catalog keys — must stay in sync with `system_modules` seed data
// (database/migrations/016_module_catalog.sql) and the Rust catalog used by
// the backend to compute a session's effective modules.
//
// Admin/Superadmin always pass `hasModule` (they get full access to every
// module their role qualifies for — see UserRole::effective_modules on the
// backend). A `usuario` session only passes for module keys explicitly
// granted by an Admin/Superadmin (see Users.tsx's module assignment UI).

export const MODULES = {
  DASHBOARD: 'dashboard',
  ACTIVE_CALLS: 'active_calls',
  CDR: 'cdr',
  ACCOUNTS: 'accounts',
  PLANS: 'plans',
  MINUTE_BUCKETS: 'minute_buckets',
  ZONES: 'zones',
  RATES: 'rates',
  SIP_DEVICES: 'sip_devices',
  INBOUND_ROUTING: 'inbound_routing',
  OUTBOUND_ROUTING: 'outbound_routing',
  CARRIERS: 'carriers',
  CARRIER_GROUPS: 'carrier_groups',
  ENDPOINT_MONITOR: 'endpoint_monitor',
  AUDIT_LOGS: 'audit_logs',
  USERS: 'users',
  DIALPLAN: 'dialplan',
  LEGACY_ROUTING: 'legacy_routing',
  SYSTEM_SETTINGS: 'system_settings',
} as const

export type ModuleKey = (typeof MODULES)[keyof typeof MODULES]

interface SessionLike {
  role?: string
  modules?: string[]
}

/**
 * Whether the given session can see the given module.
 *
 * Mirrors `AuthenticatedUser::has_module` on the backend: Admin/Superadmin
 * always pass; a `usuario` session needs the key present in their granted
 * module list. This only controls frontend menu/route visibility — the real
 * security boundary is the backend's own `has_module` check on each request.
 */
export function hasModule(user: SessionLike | null | undefined, key: ModuleKey): boolean {
  if (!user) return false
  if (user.role === 'admin' || user.role === 'superadmin') return true
  return (user.modules || []).includes(key)
}
