# PRD — Enrutamiento de Llamadas Entrantes (Inbound Routing)

**Sistema:** ApoloBilling SBC (Kamailio + FreeSWITCH + RTPEngine)
**Versión:** 3.0 — dos etapas + L3 selectivo + PBX Groups (cero-config por DID)
**Fecha:** 2026-10-03
**Estado:** Implementado y activo en producción (`sbc-core-01`)

---

## 1. Objetivo y alcance

Explicar — de forma didáctica y técnicamente precisa — cómo el SBC decide el
destino de **toda llamada entrante** (PSTN/móvil → carrier → SBC → extensión o
PBX), **sin configurar una ruta por cada línea o DID**.

**Alcance:** solo tráfico *inbound* (contexto FreeSWITCH `to-kamailio` →
`national-routing`, grupo Kamailio `9000`). El saliente (`from-pbx`, grupo
`8000`, ACQ/Portax) queda fuera.

**Audiencia:** operadores NOC, ingenieros de voz y desarrolladores del backend.

---

## 2. La idea en una frase (y su analogía carrier)

> **El SBC funciona como Vodafone o AT&T, en dos etapas: primero normaliza
> (ETAPA 1: del formato del carrier al número nacional + clase de carrier) y
> después decide (ETAPA 2: el nacional se busca en el "directorio vivo" —
> extensiones registradas, luego la PBX dueña del rango). Solo los servicios
> especiales tienen regla propia.**

| SBC Apolo | Equivalente en Vodafone/AT&T | Pregunta que responde |
|---|---|---|
| ETAPA 1 · Clasificadores RN (`6720*`…) | Normalización a E.164 + perfil de interconexión por carrier | "¿De qué red viene y cuál es el número real?" |
| ETAPA 1 · Exactas (hoy: ninguna) | Números especiales (emergencias, IVRs) | "¿Tiene instrucción manual?" |
| ETAPA 2 · L2 extensiones (`sofia_contact`) | HLR/HSS: "¿este abonado está registrado y dónde?" | "¿Es un abonado nuestro en línea?" |
| ETAPA 2 · L3 rangos (`did_ranges`) | Portabilidad/transit: cada bloque tiene un operador dueño | "¿Qué compañía es dueña de este rango?" |
| ETAPA 2 · L4 `480` | Tono de "temporalmente no disponible" de la red | "Si nadie lo quiere, ¿qué digo a la red?" |
| PBX Groups | DNSET: conjunto destino del mismo dueño con failover | "¿Primaria o secundaria del mismo cliente?" |

Consecuencia operativa: **alta de línea = alta de extensión o asignación de
rango a la PBX. Cero filas por DID.**

---

## 3. Arquitectura y flujo real

### 3.1 Topología (producción verificada)

```
PSTN / Móvil (ENTEL, CLARO, INTEGRATEL…)
   │  INVITE sip:<DID>@10.124.194.14:5060   (ej: 672217402000)
   ▼
KAMAILIO  10.10.22.4:5060 (borde; sin reglas por DID)
   │  FLB_SRC_CARRIER → P-hint: inbound, P-Early-Media, track IMS
   │  do_routing(9000): UNA regla catch-all → FreeSWITCH local
   ▼
FREESWITCH · ETAPA 1 (ctx to-kamailio): clasifica + normaliza
   │  ej: 672217402000 ─strip 4─► national=17402000 ─transfer─►
   ▼
FREESWITCH · ETAPA 2 (ctx national-routing): decide sobre el nacional
   ├─► L2 registrada:  bridge user/17402000…@10.10.22.4
   ├─► L3 rango dueño:  bridge sofia/internal/17402000@<pbx>:<puerto>
   └─► nadie:          respond 480 Temporarily Unavailable (Q.850/20)
```

### 3.2 Qué hace Kamailio — nada por DID, a propósito

El bloque `From Carrier` (`route[NEXTHOP]`, `FLB_SRC_CARRIER`) aplica a **todo**
lo que llegue de un carrier: normalización del Invite, `P-hint: inbound`,
`P-Early-Media: supported` (RFC 5009), tracking anti-timeout IMS en
`ims_early_dlg` (carriers IMS como INTEGRATEL cancelan a ~62 s), y
`do_routing(9000)` con **una sola regla de prefijo vacío** → todo el inbound al
FreeSWITCH local. Kamailio **no decide destinos finales**; separa origen
carrier vs PBX y entrega. Toda la inteligencia vive en FreeSWITCH.

### 3.3 El dialplan vivo — dos contextos, evaluación en orden

`/etc/freeswitch/dialplan/to-kamailio.xml` (16 extensiones; la primera
condición que casa, gana):

| # | Extensiones | Etapa | Origen |
|---|---|---|---|
| 1–3 | 3 wildcards RN (`6720*`, `6721*`, `6722*`) | **ETAPA 1** (`to-kamailio`) | `inbound_routes`: clasifican + transfieren, **no puentean** |
| 4 | `classify-passthrough` | **ETAPA 1** | Fija (formato ya nacional, timeout `default_timeout`) |
| 5–11 | `handle-bridge-*`, `hangup-*` (7) | Handlers IMS | Si hay rutas `ims_mode` |
| 12 | `fallback-reject` (seguridad) | **ETAPA 1** | Fija (`480`) |
| 13 | `auto-local-extensions` | **ETAPA 2** (`national-routing`) | Fija: `sofia_contact` sobre el nacional |
| 14–15 | `pbx-range-163751`, `pbx-range-174020` | **ETAPA 2** | `sip_endpoints.did_ranges` (longest-match) |
| 16 | `fallback-reject` | **ETAPA 2** | Fija configurable (`480`) |

Con `two_stage=false` todo colapsa al modelo legacy de una etapa (wildcards
puentean directo, L2/L3/L4 en `to-kamailio`). Hoy: `two_stage=true`.

---

## 4. ETAPA 1 — Clasificar y normalizar (cero-config por DID)

Dos tipos de fila en `inbound_routes` (hoy **3 filas, todas clasificadoras,
cero exactas**):

| Tipo | Ejemplo | Qué genera |
|---|---|---|
| Exacta (`672217402000`) | Servicios especiales: IVR, test con trato único | Override: puentea directo con destino fijo (hoy: ninguna) |
| Wildcard RN (`6722*`) | Un carrier/RN completo (estable, cambia rara vez) | **Clasificador**: strip/prefix → `national`, aplica timeout/ims/early-media del carrier y `transfer ${national} XML national-routing`. **Nunca puentea.** |

Clase de carrier por RN (producción):

| Patrón | Strip | Timeout | IMS/100rel | Early-media | Carrier |
|---|---|---|---|---|---|
| `6720*` | 4 | 55 s | sí | sí | ENTEL RN 20 |
| `6721*` | 4 | 55 s | sí | sí | CLARO RN 21 |
| `6722*` | 4 | 120 s | sí | sí | INTEGRATEL RN 22 |

Un DID nuevo **no necesita fila** (cae en el wildcard de su RN); un carrier
nuevo necesita **una** wildcard. En dialplan: `original_destination`
(auditoría/CDR) + `transformed_number` (lo enrutado); timeouts y
`original_destination` viajan en variables de canal al transfer. El
`pre_answer`+ringback del clasificador ocurre **antes** del transfer, así el
early-media/PRACK con el carrier no se interrumpe.

> Historial 2026-10-03: se eliminaron 11 exactas cuyo bridge
> `sofia/internal/…@10.10.22.4:5060` reinyectaba a Kamailio (hairpin →
> `500 No rules defined for number`, probado con `672214228505`). Respaldo en
> `/tmp/restore_7_routes.sql`. Sus destinos migraron a rangos; la ETAPA 1 quedó
> sin exactas. Regla: una exacta gateway **sin** PBX seleccionada contra el
> propio SBC es hairpin — la UI lo advierte en rojo al crearla.

---

## 5. ETAPA 2 · L2 — Extensiones registradas (el "HLR" del SBC)

```xml
<condition field="destination_number" expression="^(\d+)$" break="never">…
<condition field="${sofia_contact(user/${dialed_extension}@10.10.22.4)}" expression="^sofia/">
  … <action application="bridge" data="user/${dialed_extension}@10.10.22.4"/>
  <action application="transfer" data="_handle_bridge_failure_ XML to-kamailio"/>
```

Es **preguntar "¿estás en línea?" antes de llamar** sobre el número ya
nacional: si está registrado, `user/…` lo conecta (timeout
`inbound.local_timeout` = 30 s, caller-ID preservado); si no, `break="never"`
deja seguir a L3. El post-bridge deriva uniforme a los handlers IMS del otro
contexto (transfer cross-contexto). Producción: 2 extensiones
(`14228505`, `16440479`, auth por credenciales vía `mod_xml_curl`).

---

## 6. ETAPA 2 · L3 — Rango nacional → su PBX (nunca broadcast)

Cada PBX trunk/ip declara sus rangos **en su propia ficha**
(`sip_endpoints.did_ranges`, prefijos con coma, ej `174020,174021`, editable en
Dispositivos SIP → *Rangos DID propios*, validado: solo dígitos y comas). El
generador publica `pbx-range-<prefijo>` por prefijo, longest-match, puenteando
**solo** a la dueña (`hangup_after_bridge=false` + `continue_on_fail=true` +
deriva a handlers; sin rebusque en terceros). Sin rango → L4. Transformación de
rangos: global (`inbound.pbx_strip_digits/prefix_to_add`, hoy `0`/vacío = DID
íntegro); lo especial va en L1 exacta.

Rangos reales: Asterisk Test `174020` → `10.10.22.8:5060`; PBX GC GROUP
`163751` → `10.40.41.124:5160`.

**Regla anti-solape (al guardar, nombra al dueño):** un prefijo = una sola PBX.
Se rechaza si es igual, prefijo-de o extensión-de otro rango (`1740` choca con
`174020` de *Asterisk Test*), más auto-solape en la ficha y `did_ranges` fuera
de troncales IP. Mensaje: *"Rango DID ya asignado: '1740' se solapa con
'174020' de 'Asterisk Test'…".* Excepción: miembros del **mismo grupo**
(mismo dueño, failover legítimo).

**PBX Groups — failover del mismo dueño (espejo de Carrier Groups):** una PBX
pertenece máximo a **un** grupo (`UNIQUE`). Si la dueña está agrupada, el L3
puentea `miembro1|miembro2` en orden de `priority` (primaria/secundaria del
mismo cliente). Gestión web en *PBX Groups*; al agregar miembro se valida
solape contra PBXs **fuera** del grupo. API: `GET/POST /api/v1/pbx-groups`,
`PUT/DELETE /{id}`, `POST/PUT/DELETE /{id}/members[/{mid}]` (migración `031`).

---

## 7. L4 — Fallback configurable (por qué 480 y no 403/404)

`480 Temporarily Unavailable` + `sip_reason=Q.850;cause=20`: dice "red OK,
destino no disponible", el tratamiento estándar (un `403` sugiere ilegitimidad
y un `404` seco rompe reintentos en algunas redes). Conmutable a `404`/`503`
vía `inbound.default_action`. Existe en ambos contextos (seguridad en ETAPA 1,
decisión en ETAPA 2).

---

## 8. Post-bridge IMS y tonos

Las rutas `ims_mode` (hoy los 3 clasificadores) y los bridges de ETAPA 2
derivan a `_handle_bridge_failure_`, que mapea `${originate_disposition}`
(producción):

| Causa FS | Tono | Respuesta |
|---|---|---|
| `SUCCESS` | ninguno | hangup normal |
| `USER_BUSY` | ocupado 4 s (500/500 ms, 480+620 Hz) | `486` |
| `NO_ANSWER`/`NO_USER_RESPONSE` | ocupado 7 s | `486` |
| `ORIGINATOR_CANCEL`/`NORMAL_CLEARING`/`CANCEL` | ocupado 7 s | `486` |
| resto | congestión 3 s (250/250 ms) | `503` |

Requisito INTEGRATEL: tono audible en early-media antes del final SIP (su
CANCEL llega a ~62 s; timeouts 55/120 s quedan por debajo/encima según clase).

---

## 9. Configuración viva

`system_settings` (10 keys; sin tocar XML) + por PBX `did_ranges`:

| Key | Prod | Efecto |
|---|---|---|
| `inbound.two_stage` | `true` | Dos etapas / legacy |
| `inbound.auto_local_extensions` | `true` | Activa L2 |
| `inbound.auto_pbx_hunt` | `true` | Activa L3 |
| `inbound.pbx_hunt_mode` | `ranges` | `ranges` (selectivo) / `hunt-all` (1 sola PBX) |
| `inbound.default_timeout` | `55` | Timeout passthrough (s) |
| `inbound.local_timeout` | `30` | Timeout a extensión (s) |
| `inbound.pbx_hunt_timeout` | `55` | Timeout a PBX (s) |
| `inbound.default_action` | `480` | `480`/`404`/`503` en L4 |
| `inbound.pbx_strip_digits` | `0` | Strip global rangos L3 |
| `inbound.pbx_prefix_to_add` | `` | Prefijo global rangos L3 |

El generador tolera keys/tablas ausentes (defaults seguros).

---

## 10. Operación día a día

1. **Alta de extensión:** Dispositivos SIP → credenciales. Sin tocar rutas;
   verificar registro `sofia status profile internal reg`.
2. **Alta de PBX:** troncal IP con host:puerto **+ Rangos DID propios**. El Sync
   publica `pbx-range-*` solo hacia ella.
3. **Failover misma PBX:** *PBX Groups* → grupo + miembros con prioridad.
4. **Carrier nuevo:** una wildcard RN con su clase (timeout/ims/early-media).
5. **Baja temporal:** `enabled=false` (fila, PBX o grupo); el tráfico cae a la
   siguiente capa. Borrar un grupo no borra rangos.
6. **Prohibido:** editar `/etc/freeswitch/dialplan/to-kamailio.xml` a mano
   (cada guardado en la UI lo regenera; backup `to-kamailio.xml.bak`).

### Troubleshooting

| Síntoma | Capa | Acción |
|---|---|---|
| DID puntual mal enrutado | E1 | ¿Exacta obsoleta? ¿Wildcard de su RN existe? Ver `INFO ETAPA1` en log FS |
| Extensión registrada no suena | E2·L2 | `show registrations`; `inbound.auto_local_extensions`; buscar `AUTO-L2` |
| PBX no recibe nada | E2·L3 | `SELECT name,did_ranges…`; ¿rango asignado a la PBX correcta?; `sngrep host <pbx>`; buscar `AUTO-L3: rango X -> […]` |
| Llamada a PBX ajena | L3 | Revisar `did_ranges` solapados y grupos (UNIQUE por endpoint) |
| `500 No rules` en Kamailio | E1 | Hairpin: exacta gateway sin PBX contra el SBC — la UI lo advierte |
| Carrier recibe 480 inesperado | L4 | `AUTO-L4` = sin destino (normal si el rango no tiene dueña) |
| Tras Sync se pierde algo | Generador | Restaurar `.bak`, `reloadxml`, ver qué fila cambió |

---

## 11. Implementación (trazabilidad)

| Pieza | Fichero / objeto | Rol |
|---|---|---|
| Borde | `config/kamailio/kamailio.cfg` (`FLB_SRC_CARRIER`, `do_routing(9000)` catch-all) | Todo inbound → FS |
| Generador **activo** | `rust-backend/…/handlers/inbound_routing.rs` (`sync_…`, `get_setting`, `collect_range_prefixes`, `legs_for`) | ETAPA 1 + ETAPA 2, `reloadxml` |
| Generador legacy | `…/handlers/unified_routing.rs` (`routing_inbound_routes`) | Una etapa (compatibilidad) |
| Grupos | `…/handlers/pbx_group.rs` + `dto/pbx_group.rs` + página `PBXGroups.tsx` | CRUD `/api/v1/pbx-groups` |
| Endpoints | `sip_endpoint.{rs}` (modelo/repo/DTO/handler) + `SipEndpoints.tsx` | `did_ranges` con anti-solape |
| Rutas UI | `InboundRouting.tsx` (badges E1/override, panel ETAPA 2, alerta hairpin) | Distinguir etapas en web |
| Dialplan vivo | `/etc/freeswitch/dialplan/to-kamailio.xml` (16 extensiones, 2 contextos) | Lo que FS ejecuta |
| Datos | `inbound_routes` (3 wildcards), `sip_endpoints` (2 ext + 3 trunks), `pbx_groups` (0), `system_settings` (`inbound.*` ×10) | Estado vivo |
| Migraciones | `027` (settings L2/L3/L4), `029` (`did_ranges` en la PBX), `030` (`two_stage`), `031` (PBX Groups) | Cambios aplicados |
| Verificación | `cargo test` 134+ OK, `cargo check`, `rustfmt`, `tsc`, XML válido, `reloadxml` +OK | Calidad |

---

## 12. Decisiones y no-objetivos

- **Decidido:** dos etapas (normalizar→decidir); rangos en la ficha de la PBX
  (una sola fuente de verdad); L3 selectivo + grupos mismo-dueño; L4 `480`.
- **Descartado:** tabla separada de rangos (duplicaba la ficha); hunt ciego
  multi-cliente (solo `hunt-all` single-tenant); exactas por DID (solo servicios
  especiales).
- **DNSET evaluado:** un DNSET = conjunto destino del mismo dueño con failover.
  Se evaluó Kamailio `dispatcher` (set por PBX, `ds_select_dst` por prefijo
  nacional). **No ahora:** sacaría la llamada del control FS (facturación,
  early-media/PRACK, tonos) y exige recargas Kamailio por cambio. El modelo
  actual ya tiene semántica DNSET (rango = set de 1–N miembros mismo dueño);
  dispatcher queda como evolución para failover primario/secundario.
- **No objetivo:** LNP entrante (ACQ es saliente), IVRs/colas (exactas L1),
  tarificación inbound normal (solo 0800/1800 al llamado, motor aparte).
