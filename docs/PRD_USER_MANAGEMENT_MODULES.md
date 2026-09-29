# PRD — Gestión de Usuarios y Permisos por Módulo

**Producto:** ApoloBilling (Backend Rust + Frontend React)
**Autor:** Jesus Bazan (Superadmin / Soporte de fábrica)
**Estado:** Propuesta — pendiente de validación
**Versión:** 1.0
**Fecha:** 2026-09-23

---

## 1. Resumen ejecutivo

Hoy ApoloBilling tiene tres roles (`operator`, `admin`, `superadmin`) pero en la práctica el frontend solo distingue **dos niveles**: "superadmin" (ve todo) y "todos los demás" (ven un subconjunto fijo, hardcodeado por ruta). No existe forma de que un cliente administre el acceso de su propio personal.

Este PRD define un nuevo modelo de tres roles con **significado real de negocio**:

| Rol | Quién es | Alcance |
|---|---|---|
| **Superadmin** | Jesus, soporte de fábrica del producto | Acceso total, incluida infraestructura core del SBC y gestión de administradores |
| **Admin** | Administrador de la empresa cliente | Acceso total a los módulos operativos/de negocio; crea y gestiona usuarios "normales" de su empresa |
| **Usuario** | Personal de otras áreas del cliente (ventas, soporte, finanzas, NOC, etc.) | Solo ve los módulos que Admin o Superadmin le habiliten explícitamente |

El cambio central es introducir un **catálogo de módulos** y una tabla de **permisos por usuario**, de forma que tanto Superadmin como Admin puedan decidir, módulo por módulo, qué ve cada usuario "normal" — sin tocar código.

**Decisiones de alcance ya validadas con el usuario:**
1. El control es **solo de visibilidad** (ver/ocultar módulo), no de acciones dentro del módulo (ver vs. editar). Queda como posible Fase 2.
2. **Admin siempre tiene acceso completo** a todos los módulos salvo un set fijo "exclusivo de fábrica" (infraestructura core / configuración avanzada del sistema). Superadmin no restringe módulos a Admin en esta fase.
3. El modelo soporta **uno o varios Admin sin jerarquía entre ellos** — cualquier Admin puede gestionar a cualquier Usuario de la instalación (no hay "dueño" por usuario). No hay aislamiento multi-cliente/tenant: se asume una sola empresa cliente por instalación.

---

## 2. Contexto actual (estado del código)

Verificado en el repo al momento de escribir este PRD:

- **Rol:** `apolo-core::models::user::UserRole` = `Operator | Admin | Superadmin`, con `level()` (1/2/3) y `can_manage()` ya implementado por jerarquía (`self.level() > other.level()`). Esta función es reutilizable tal cual.
- **Backend:** `rust-backend/src/main.rs` registra casi todas las rutas de administración (`/users`, `/audit-logs`, `/dialplan`, `/kamailio-dialplan`, `/routing/*`) como **superadmin only**. No existe distinción real de "admin".
- **Frontend:** `frontend/src/components/Layout.tsx` filtra secciones del menú con un único booleano `adminOnly` comparado contra `role === 'superadmin'`. `App.tsx` protege rutas con `requiredRole="superadmin"` hardcodeado por ruta. No hay concepto de módulo ni de permiso por usuario.
- **No existe** ninguna tabla de catálogo de módulos ni de permisos granulares — la única variable de control es el rol.

**Problema a resolver:** el rol `operator` no tiene ningún propósito de negocio hoy (nadie lo usa para nada distinto de "no-admin"), y no hay manera de que el cliente autogestione qué ve su propio personal sin pedirle un cambio de código a Jesus.

---

## 3. Objetivos

1. Redefinir los tres roles con semántica de negocio clara (Superadmin / Admin / Usuario).
2. Permitir que **Superadmin y Admin** habiliten o deshabiliten, por usuario, qué módulos del menú/rutas puede ver un Usuario "normal".
3. Que la restricción sea real (server-side, no solo ocultar un botón en el frontend).
4. Migrar los datos y usuarios existentes sin dejar a nadie fuera del sistema el día del corte.
5. Dejar trazabilidad (auditoría) de quién otorgó/quitó acceso a qué módulo y cuándo.

### Fuera de alcance (explícito)
- Permisos por acción (crear/editar/eliminar) dentro de un módulo — queda para una Fase 2 futura.
- Multi-tenancy real (múltiples empresas cliente aisladas en la misma instancia).
- Jerarquía o "propiedad" entre distintos Admin sobre distintos Usuarios.
- Cambios a la infraestructura de FreeSWITCH/Kamailio en sí (INTEGRATEL, dialplans, etc.) — no se toca nada de eso.

---

## 4. Roles y matriz de capacidades

| Capacidad | Superadmin | Admin | Usuario |
|---|:---:|:---:|:---:|
| Login / cambiar su propia contraseña | ✅ | ✅ | ✅ |
| Ver módulos operativos/negocio (Dashboard, CDR, Cuentas, Tarifas, Routing, SIP, etc.) | ✅ Todos | ✅ Todos | 🔒 Solo los que le fueron habilitados |
| Crear/editar/eliminar cuentas con rol **Usuario** | ✅ | ✅ | ❌ |
| Otorgar/quitar módulos a un **Usuario** | ✅ | ✅ | ❌ |
| Crear/editar/eliminar cuentas con rol **Admin** o **Superadmin** | ✅ | ❌ | ❌ |
| Ver/gestionar módulos "exclusivos de fábrica" (Dialplan XML, Routing Legacy, Configuración del Sistema) | ✅ | ❌ | ❌ |
| Ver auditoría del sistema | ✅ | ✅ (de su operación) | ❌ (salvo que se habilite en Fase 2) |

Esta tabla usa el `can_manage()` que **ya existe** en `UserRole` (nivel 3 > 2 > 1), así que no hay que reinventar la jerarquía — solo hay que empezar a **aplicarla** en los handlers de `/users`, que hoy son ciegamente superadmin-only.

---

## 5. Catálogo de módulos (propuesta inicial)

Cada módulo tiene una **clave estable** (`module_key`), un nombre visible, una sección de menú, y un **`min_role`**: el rol mínimo que puede acceder a él sin necesidad de un permiso explícito.

- `min_role = usuario` → visible para Admin/Superadmin siempre; para Usuario **solo si tiene el permiso explícito**.
- `min_role = admin` → visible para Admin/Superadmin; nunca asignable a Usuario.
- `min_role = superadmin` → exclusivo de fábrica; nunca asignable a Admin ni a Usuario.

| `module_key` | Nombre en menú | Sección | `min_role` |
|---|---|---|:---:|
| `dashboard` | Panel de Control | Principal | usuario *(siempre visible, ver §5.1)* |
| `active_calls` | Llamadas Activas | Principal | usuario |
| `cdr` | Registros CDR | Principal | usuario |
| `accounts` | Cuentas | Facturación | usuario |
| `plans` | Planes | Facturación | usuario |
| `balance` | Saldos | Facturación | usuario |
| `zones` | Zonas | Tarifas | usuario |
| `rates` | Tarifas | Tarifas | usuario |
| `sip_devices` | Dispositivos SIP | Operación | usuario |
| `inbound_routing` | Rutas Entrantes | Operación | usuario |
| `outbound_routing` | Rutas Salientes | Operación | usuario |
| `carriers` | Carriers | Operación | usuario |
| `carrier_groups` | Carrier Groups | Operación | usuario |
| `endpoints` | Endpoints | Operación | usuario |
| `endpoint_monitor` | Monitor de Endpoints | Operación | usuario |
| `audit_logs` | Auditoría | Sistema | **admin** |
| `users` | Usuarios y Permisos | Sistema | **admin** |
| `dialplan` | Dialplan FreeSWITCH (XML) | Sistema Avanzado | **superadmin** |
| `legacy_routing` | Routing Legacy (Unified/Internal) | Sistema Avanzado | **superadmin** |
| `system_settings` | Configuración del Sistema | Sistema Avanzado | **superadmin** |

> Este catálogo es la propuesta inicial de arranque — queda como **dato de configuración**, no hardcodeado, para poder agregar/quitar módulos sin migración de esquema (ver §6).

### 5.1 Caso especial: `dashboard`
El Dashboard actúa como "home" tras el login. Se recomienda que **siempre sea visible para cualquier usuario autenticado**, sin requerir grant explícito, para evitar pantallas en blanco tras iniciar sesión. Se marca en el catálogo como módulo "siempre visible" (`always_visible = true`) en vez de depender del grant.

---

## 6. Modelo de datos

### 6.1 Rol: renombrar `operator` → `usuario`

```sql
-- Migración 015_rename_role_operator_to_usuario.sql
BEGIN;

-- 1. Relajar el constraint para aceptar el nuevo valor
ALTER TABLE usuarios DROP CONSTRAINT IF EXISTS usuarios_role_check;
ALTER TABLE usuarios ADD CONSTRAINT usuarios_role_check
    CHECK (role IN ('usuario', 'admin', 'superadmin', 'operator')); -- 'operator' temporal, ver Fase 4

-- 2. Migrar filas existentes
UPDATE usuarios SET role = 'usuario' WHERE role = 'operator';

-- 3. (En un release posterior, una vez confirmado que nada externo envía 'operator')
-- ALTER TABLE usuarios DROP CONSTRAINT usuarios_role_check;
-- ALTER TABLE usuarios ADD CONSTRAINT usuarios_role_check CHECK (role IN ('usuario','admin','superadmin'));

COMMIT;
```

En Rust, `UserRole::Operator` se renombra a `UserRole::Usuario` (serde `rename = "usuario"`). Para no romper compatibilidad durante el corte, `from_str()` acepta temporalmente tanto `"usuario"` como `"operator"` (alias) y siempre serializa como `"usuario"`.

### 6.2 Catálogo de módulos

```sql
-- Migración 016_module_catalog.sql
CREATE TABLE system_modules (
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

-- Seed: las 20 filas de la tabla en §5, con dashboard.always_visible = true
```

### 6.3 Permisos por usuario

```sql
-- Migración 017_usuario_modulos.sql
CREATE TABLE usuario_modulos (
    id SERIAL PRIMARY KEY,
    usuario_id INTEGER NOT NULL REFERENCES usuarios(id) ON DELETE CASCADE,
    module_key VARCHAR(50) NOT NULL REFERENCES system_modules(module_key) ON DELETE CASCADE,
    otorgado_por INTEGER REFERENCES usuarios(id),
    otorgado_en TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (usuario_id, module_key)
);

CREATE INDEX idx_usuario_modulos_usuario ON usuario_modulos(usuario_id);
```

Solo tiene sentido para cuentas con `role = 'usuario'`. Para `admin`/`superadmin` el acceso se calcula en código a partir del rol, no de esta tabla (evita duplicar datos y evita que quedar desincronizada rompa el acceso de un admin).

---

## 7. Reglas de negocio (server-side, no solo UI)

1. **Cálculo de módulos efectivos de una sesión** (al hacer login o refrescar `/auth/me`):
   - Si `role IN (admin, superadmin)`: módulos efectivos = todos los `system_modules` con `min_role <= role` (superadmin ve también los `min_role = superadmin`).
   - Si `role = usuario`: módulos efectivos = `{dashboard}` ∪ `usuario_modulos` del usuario, filtrado a módulos con `min_role = usuario` y `activo = true` (defensa: si alguien insertó un grant inválido a mano, no se respeta).

2. **Gestión de cuentas** (`POST/PUT/DELETE /users`):
   - Se reemplaza el guard de ruta `SuperadminUser` por `AdminUser` (admin o superadmin).
   - Dentro del handler: usar `current_user.role.can_manage(&target.role)` (ya existe) antes de permitir editar/eliminar. Un Admin que intenta editar a otro Admin o a Superadmin recibe `403 Forbidden`.
   - Al crear un usuario, el selector de rol en el payload se valida contra lo que el creador puede otorgar: Admin solo puede crear `role=usuario`; Superadmin puede crear cualquiera.

3. **Gestión de módulos** (`PUT /users/{id}/modules`):
   - Solo Admin/Superadmin.
   - El `target_user.role` debe ser `usuario` (no tiene sentido para admin/superadmin — se rechaza con 400).
   - Los `module_key` enviados deben existir en `system_modules` y tener `min_role = usuario` (no se puede "otorgar" un módulo de fábrica burlando el catálogo).

4. **Todo endpoint de negocio** (CDR, Cuentas, Rates, Routing, etc.) queda protegido por un guard de módulo, no solo por rol — ver §8.2. Esto es lo que hace la restricción real y no solo cosmética.

---

## 8. Cambios de Backend (Rust)

### 8.1 `apolo-core`
- Renombrar `UserRole::Operator` → `UserRole::Usuario` (con alias de parseo `"operator"`).
- Nuevos modelos: `SystemModule { module_key, nombre, seccion, min_role, always_visible }` y `ModuleGrant { usuario_id, module_key, otorgado_por, otorgado_en }`.
- Nueva función utilitaria `UserRole::effective_modules(&self, grants: &[String], catalog: &[SystemModule]) -> Vec<String>` que implementa la regla de §7.1 — centraliza la lógica para que backend y (si hace falta) tests la compartan.

### 8.2 `apolo-auth`
- Nuevo extractor `ModuleAccess<"cdr">` (o un middleware genérico `require_module(key)`) análogo a los ya existentes `AdminUser`/`SuperadminUser`, que lee la lista de módulos efectivos embebida en el JWT (ver abajo) y devuelve `403` si el módulo solicitado no está.
- El JWT (`Claims`) suma un campo `modules: Vec<String>` calculado en el login con la regla de §7.1. Se recalcula en cada login/renovación de token — no requiere consulta a DB en cada request.
  - *Trade-off aceptado:* si un Admin le quita un módulo a un Usuario ya logueado, el cambio tarda hasta `JWT_EXPIRATION_SECS` (30 min hoy) en aplicarse, salvo que el Admin fuerce el cierre de sesión del usuario (botón "Cerrar sesión" en `Usuarios`, invalida el refresh/token vía blacklist en Redis — mecanismo ya disponible en `apolo-cache`).

### 8.3 `apolo-api`
- Nuevos handlers:
  - `GET /api/v1/modules` — catálogo completo (Admin+).
  - `GET /api/v1/users/{id}/modules` — módulos otorgados a un usuario (Admin+).
  - `PUT /api/v1/users/{id}/modules` — reemplaza el set completo de módulos otorgados (Admin+, valida reglas de §7.3).
- Handlers existentes de `/users/*`: cambiar guard `SuperadminUser` → `AdminUser` + aplicar `can_manage()` (§7.2).
- `GET /auth/me`: la respuesta suma `"modules": ["dashboard", "cdr", ...]`.
- `main.rs`: mover rutas `/users/*` del bloque "superadmin only" al bloque "admin+", agregar rutas nuevas de módulos.
- Auditoría: nuevos eventos en `audit_logs` — `user_created`, `user_role_changed`, `module_grant_updated` (con diff antes/después), `user_force_logout`.

---

## 9. Cambios de Frontend (React)

### 9.1 Tipos (`types/index.ts`)
- `UserRole`: `'usuario' | 'admin' | 'superadmin'`.
- `CurrentUser` suma `modules: string[]`.

### 9.2 `App.tsx`
- Reemplazar `requiredRole="superadmin"` hardcodeado por `requiredModule="carriers"` (etc.), usando las mismas `module_key` del catálogo backend (constante compartida `MODULES` en `frontend/src/constants/modules.ts`).
- `ProtectedRoute` amplía su lógica: acceso si `user.role !== 'usuario'` **y** el módulo tiene `min_role <= user.role`, o si `user.role === 'usuario'` y `user.modules.includes(requiredModule)`.

### 9.3 `Layout.tsx`
- Cada `NavItem` suma `moduleKey`.
- El filtro de secciones deja de usar el booleano `adminOnly === (role === 'superadmin')` y pasa a: ítem visible si `moduleKey` está en los módulos efectivos de `currentUser` (mismo criterio que el backend, reutilizando la lista que ya viaja en `/auth/me`).
- Esto es solo UX (ocultar del menú); la seguridad real la da §9.2 + el backend (§8.2). Nunca confiar solo en el ocultamiento del menú.

### 9.4 `Users.tsx`
- Selector de rol al crear/editar: Admin solo ve la opción "Usuario"; Superadmin ve "Usuario / Admin / Superadmin".
- Nueva sección/tab **"Módulos visibles"**, visible solo si el usuario editado tiene `role = usuario`: checklist agrupado por sección (igual estructura que el menú), con botón "Guardar módulos" → `PUT /users/{id}/modules`.
- Acción "Cerrar sesión del usuario" (fuerza logout inmediato) para que un cambio de permisos aplique al instante si es necesario.

---

## 10. Migración de datos existentes

**Riesgo principal:** hoy cualquier cuenta `operator` ya ve Dashboard, Llamadas, CDR, Cuentas, Planes, Saldos, Zonas y Tarifas (todo lo no-`adminOnly`). Si el día del corte se migran a `usuario` con cero módulos otorgados, **pierden acceso a todo excepto Dashboard** y se genera un incidente de soporte.

**Mitigación (obligatoria, no opcional):** el script de migración de datos, además de renombrar el rol, debe **otorgar automáticamente** a cada cuenta `operator → usuario` existente el set de módulos que ya podía ver antes del cambio:

```sql
INSERT INTO usuario_modulos (usuario_id, module_key, otorgado_por)
SELECT u.id, m.module_key, NULL
FROM usuarios u
CROSS JOIN system_modules m
WHERE u.role = 'usuario'
  AND m.module_key IN ('dashboard','active_calls','cdr','accounts','plans','balance','zones','rates')
ON CONFLICT DO NOTHING;
```

Esto preserva el comportamiento actual el día del corte; a partir de ahí, Admin/Superadmin pueden ajustar (recortar o ampliar) por usuario según corresponda.

---

## 11. Plan de implementación por fases

| Fase | Contenido | Entregable |
|---|---|---|
| **0** | Validación de este PRD | Sign-off del catálogo de módulos y de las 3 decisiones de alcance |
| **1** | Backend — esquema | Migraciones 015/016/017, seed del catálogo, rename `Operator→Usuario` en Rust (con alias de compatibilidad) |
| **2** | Backend — API y enforcement | Endpoints de módulos, `Claims.modules`, extractor `ModuleAccess`, relajar `/users/*` a Admin+ con `can_manage()`, auditoría |
| **3** | Frontend | `ProtectedRoute` por módulo, `Layout.tsx` dinámico, UI de asignación de módulos en `Users.tsx`, restricción de selector de rol |
| **4** | Migración de datos | Backfill de módulos para cuentas `operator` existentes (§10), verificación en staging con cuenta de prueba por cada rol |
| **5** | QA y rollout | Matriz de pruebas por rol (ver §12), despliegue, aviso al Admin del cliente sobre la nueva funcionalidad |

Fases 1–2 pueden avanzar en paralelo con Fase 3 una vez el contrato de API (`/modules`, `/users/{id}/modules`, `modules` en `/auth/me`) esté congelado.

---

## 12. Criterios de aceptación

- [ ] Un Superadmin puede crear un Admin; un Admin **no puede** crear otro Admin ni un Superadmin (403).
- [ ] Un Admin puede crear un Usuario y, en el mismo flujo o inmediatamente después, elegir qué módulos ve.
- [ ] Un Usuario sin módulos otorgados solo ve el Dashboard al iniciar sesión; el menú no muestra ningún otro ítem.
- [ ] Si un Usuario intenta acceder por URL directa a una ruta de un módulo no otorgado, el backend responde `403` (no alcanza con que el frontend oculte el menú).
- [ ] Al quitarle un módulo a un Usuario y forzar su cierre de sesión, el usuario pierde el acceso de inmediato en su próximo request.
- [ ] Los módulos `dialplan`, `legacy_routing` y `system_settings` nunca aparecen como asignables en la UI de permisos (ni para Admin).
- [ ] Todas las cuentas `operator` existentes antes del corte conservan, tras la migración, acceso a los mismos módulos que tenían antes (verificar con diff de menú pre/post).
- [ ] Cada cambio de módulo otorgado/quitado y cada cambio de rol queda en `audit_logs` con actor, objetivo y timestamp.

---

## 13. Riesgos y mitigaciones

| Riesgo | Mitigación |
|---|---|
| Seguridad "de mentira" (solo se oculta el menú, la API sigue abierta) | El guard de módulo se aplica en el backend (§8.2/§8.3); el frontend es solo UX |
| Usuarios `operator` pierden acceso el día del corte | Backfill automático obligatorio (§10), no opcional |
| Cambios de permiso no aplican hasta que expire el JWT (30 min) | Botón de "forzar cierre de sesión" desde `Usuarios`; documentar el SLA de 30 min como aceptable |
| Referencias a `"operator"` hardcodeadas en otras partes del código (frontend/backend/scripts) que no se detecten en este PRD | Tarea de implementación: `grep -ri operator` sobre todo el repo antes de cerrar Fase 1, y mantener el alias de parseo por al menos un release |
| Catálogo de módulos desincronizado si se agrega una página nueva sin sumarla a `system_modules` | Checklist de PR: toda página nueva del frontend debe declarar su `module_key` y tener su fila correspondiente en el seed/migración |

---

## 14. Preguntas abiertas / trabajo futuro

- ¿Se quiere, en una Fase 2, granularidad de **acciones** (ver vs. editar) dentro de un módulo? (Explícitamente fuera de alcance por ahora, según decisión del usuario.)
- ¿Debería `audit_logs` ser parcialmente visible para `usuario` en el futuro (p. ej. su propio historial de acciones)?
- Si en el futuro se necesita más de una empresa cliente en la misma instancia (multi-tenant real), este modelo de "un solo pool de Admin/Usuario sin jerarquía" habría que revisitarlo — hoy se descarta explícitamente por decisión del usuario.
