# Troubleshooting: Llamadas Entrantes ENTEL (Prefijo 6720)

**Fecha:** 2026-08-10
**Sistema:** ApoloBilling SBC (FreeSWITCH + Kamailio + RTPEngine)
**Carrier:** ENTEL (RN 20, prefijo 6720)

---

## Resumen Ejecutivo

Este documento detalla los problemas encontrados y soluciones implementadas para:
1. Sincronización del dialplan XML desde la interfaz web
2. Configuración del prefijo 6720 para llamadas entrantes de ENTEL
3. Enrutamiento de llamadas entrantes hacia softphones registrados

---

## 1. Problemas de Sincronización XML desde la Web

### 1.1 Error de Permisos en Archivo Dialplan

**Síntoma:**
```
Error al sincronizar rutas: Permission denied (os error 13)
```

**Causa:**
El archivo `/etc/freeswitch/dialplan/to-kamailio.xml` no tenía permisos de escritura para el proceso del backend.

**Solución:**
```bash
sudo chown freeswitch:freeswitch /etc/freeswitch/dialplan/to-kamailio.xml
sudo chmod 664 /etc/freeswitch/dialplan/to-kamailio.xml
```

**Verificación:**
```bash
ls -la /etc/freeswitch/dialplan/to-kamailio.xml
# -rw-rw-r-- 1 freeswitch freeswitch ... to-kamailio.xml
```

---

### 1.2 Error de Permisos en Socket Kamailio

**Síntoma:**
```
Permission denied al ejecutar kamcmd drouting.reload
```

**Causa:**
El socket de control de Kamailio (`/var/run/kamailio/kamailio_ctl`) tenía permisos restrictivos (0600).

**Solución Temporal:**
```bash
sudo chmod 660 /var/run/kamailio/kamailio_ctl
```

**Solución Permanente:**
Editar `/etc/kamailio/kamailio.cfg` y agregar:

```kamailio
# ----- ctl params -----
modparam("ctl", "binrpc", "unix:/var/run/kamailio/kamailio_ctl")
modparam("ctl", "mode", 0660)
```

Luego reiniciar Kamailio:
```bash
systemctl restart kamailio
```

---

### 1.3 Backend No Generaba Configuración IMS Completa

**Síntoma:**
Al sincronizar desde la web, el dialplan generado no incluía:
- Soporte 100rel/PRACK
- Handlers de timeout con tonos
- Manejo de early media

**Causa:**
El backend no tenía el campo `ims_mode` en la base de datos ni generaba los handlers IMS.

**Solución:**

1. **Migración de base de datos** (`database/migrations/010_add_ims_mode.sql`):
```sql
ALTER TABLE routing_inbound_routes
ADD COLUMN IF NOT EXISTS ims_mode BOOLEAN NOT NULL DEFAULT false;

COMMENT ON COLUMN routing_inbound_routes.ims_mode IS
  'Enable IMS interworking mode: 100rel/PRACK, timeout handlers, congestion tones';

-- Activar IMS mode para rutas de carriers IMS (ENTEL, INTEGRATEL)
UPDATE routing_inbound_routes
SET ims_mode = true
WHERE did_pattern LIKE '%672%';
```

2. **Actualización del backend** (`unified_routing.rs`):
   - Agregar campo `ims_mode` a DTOs
   - Modificar queries para incluir `ims_mode`
   - Generar configuración IMS cuando `ims_mode=true`

---

## 2. Configuración del Prefijo 6720 (ENTEL)

### 2.1 Arquitectura de Enrutamiento

```
ENTEL (Carrier IMS)
    │
    │ INVITE sip:672017402002@10.124.194.14:5060
    ▼
┌─────────────────────────────────────────────────────┐
│                    KAMAILIO                          │
│  - IP Source: 10.35.13.100 (carrier address grp=8)  │
│  - setbflag(FLB_SRC_CARRIER)                        │
│  - do_routing(9000) → gwlist #100                   │
│  - Agrega P-Early-Media: supported                  │
└───────────────────────┬─────────────────────────────┘
                        │ INVITE sip:672017402002@10.10.22.4:5082
                        ▼
┌─────────────────────────────────────────────────────┐
│                   FREESWITCH                         │
│  Profile: inbound (:5082)                           │
│  Context: to-kamailio                               │
│  Extension: from-entel-rn-20 (^6720.+$)             │
│  Transform: 672017402002 → 17402002                 │
└───────────────────────┬─────────────────────────────┘
                        │ INVITE user/17402002@10.10.22.4
                        ▼
┌─────────────────────────────────────────────────────┐
│                    SOFTPHONE                         │
│  Registrado en internal profile (:5080)             │
│  Contact: sip:17402002@10.212.134.53:55340          │
└─────────────────────────────────────────────────────┘
```

### 2.2 Configuración Kamailio

**Tabla `address` (trusted carriers):**
```sql
-- INTEGRATEL/ENTEL comparten IP
INSERT INTO address (grp, ip_addr, mask, port, tag)
VALUES (8, '10.35.13.100', 32, 0, 'name:Integratel,gwgroup:11');
```

**Tabla `dr_gateways` (destino FreeSWITCH):**
```sql
SELECT * FROM dr_gateways WHERE gwid=100;
-- gwid=100, type=9, address=10.10.22.4:5082, description=FreeSWITCH PBX Local
```

**Tabla `dr_rules` (regla inbound):**
```sql
SELECT * FROM dr_rules WHERE groupid=9000;
-- ruleid=5, groupid=9000, prefix='', gwlist=#100, description=Inbound_to_FreeSWITCH
```

### 2.3 Configuración FreeSWITCH

**Profile Inbound** (`/etc/freeswitch/sip_profiles/inbound.xml`):
```xml
<profile name="inbound">
  <settings>
    <param name="sip-ip" value="10.10.22.4"/>
    <param name="sip-port" value="5082"/>
    <param name="context" value="to-kamailio"/>
    <param name="enable-100rel" value="true"/>
    <param name="inbound-bypass-media" value="false"/>
  </settings>
</profile>
```

**Extensión ENTEL** (`/etc/freeswitch/dialplan/to-kamailio.xml`):
```xml
<extension name="from-entel-rn-20">
  <condition field="destination_number" expression="^(6720.+)$">
    <action application="log" data="INFO Inbound From ENTEL RN 20: ${caller_id_number} -> ${destination_number}"/>
    <action application="set" data="original_destination=${destination_number}"/>
    <action application="set" data="transformed_number=${destination_number:4}"/>
    <action application="log" data="INFO Transformed: ${original_destination} -> ${transformed_number}"/>

    <!-- Timeouts - 55s para tener 7s de tono antes del CANCEL del carrier (62s) -->
    <action application="set" data="call_timeout=55"/>
    <action application="set" data="originate_timeout=55"/>
    <action application="set" data="progress_timeout=55"/>

    <!-- IMS 100rel/PRACK support -->
    <action application="set" data="sip_enable_100rel=true"/>
    <action application="export" data="sip_enable_100rel=true"/>

    <!-- Media handling -->
    <action application="set" data="bypass_media=false"/>
    <action application="set" data="inherit_codec=true"/>

    <!-- Early media con ringback -->
    <action application="set" data="ringback=${us-ring}"/>
    <action application="set" data="instant_ringback=true"/>
    <action application="pre_answer"/>

    <!-- Continuar después del bridge para manejar timeout -->
    <action application="set" data="hangup_after_bridge=false"/>
    <action application="set" data="continue_on_fail=true"/>

    <!-- Bridge al usuario interno -->
    <action application="bridge" data="user/${transformed_number}@10.10.22.4"/>

    <!-- Post-bridge: manejar fallo según causa -->
    <action application="log" data="WARNING FS: Bridge terminó - originate_disp=${originate_disposition}"/>
    <action application="transfer" data="_handle_bridge_failure_"/>
  </condition>
</extension>
```

---

## 3. Problemas de Enrutamiento al Softphone

### 3.1 Error NORMAL_TEMPORARY_FAILURE

**Síntoma:**
```
-ERR NORMAL_TEMPORARY_FAILURE
```
al intentar originar llamadas a usuarios registrados.

**Causas Identificadas:**

| Causa | Descripción | Solución |
|-------|-------------|----------|
| Registro expirado | El registro del softphone había expirado | Esperar re-registro o verificar con `sofia status profile internal reg` |
| IP/Puerto cambiado | El softphone cambió de red (NAT) | El nuevo REGISTER actualiza el contacto |
| Condición temporal | Problema de red momentáneo | Reintentar después de unos segundos |

**Diagnóstico:**
```bash
# Verificar registros activos
fs_cli -x "sofia status profile internal reg" | grep 17402002

# Obtener dial string actual
fs_cli -x "sofia_contact 17402002@10.10.22.4"

# Probar originate directo
fs_cli -x "originate user/17402002@10.10.22.4 &echo"
```

### 3.2 Error NO_ANSWER vs NORMAL_TEMPORARY_FAILURE

| Resultado | Significado | Estado |
|-----------|-------------|--------|
| `NO_ANSWER` | INVITE enviado, softphone timbra, nadie contesta | ✓ Correcto |
| `NORMAL_TEMPORARY_FAILURE` | No se puede alcanzar el endpoint | ✗ Problema de conectividad |
| `RECOVERY_ON_TIMER_EXPIRE` | Timeout esperando respuesta SIP | ✗ Problema de red/NAT |

### 3.3 Verificación de Conectividad

```bash
# Verificar ruta al softphone
ip route get 10.212.134.53

# Capturar tráfico SIP durante llamada
tcpdump -i ens18 -n "host 10.212.134.53" -A

# Verificar que el INVITE llega y se recibe 180 Ringing
```

**Ejemplo de flujo exitoso:**
```
10.10.22.4:5080 → 10.212.134.53:55340: INVITE (SDP con RTP IP)
10.212.134.53:55340 → 10.10.22.4:5080: 180 Ringing
10.212.134.53:55340 → 10.10.22.4:5080: 200 OK (si contestan)
```

---

## 4. Problema con Handler de Llamadas Exitosas

### 4.1 Síntoma

Después de una llamada exitosa (contestada y colgada normalmente), los logs mostraban:
```
INFO Hangup handler: CONGESTION/DEFAULT (SUCCESS) - sending 503
```

El sistema reproducía tono de congestión y enviaba 503 Service Unavailable para llamadas que habían sido exitosas.

### 4.2 Causa

El dialplan transfería incondicionalmente a `_handle_bridge_failure_` después del bridge:
```xml
<action application="bridge" data="user/${transformed_number}@10.10.22.4"/>
<action application="transfer" data="_handle_bridge_failure_"/>
```

El dispatcher de handlers no tenía caso para `originate_disposition=SUCCESS`, por lo que caía en el handler por defecto (congestión).

### 4.3 Solución

Agregar handler para llamadas exitosas ANTES del dispatcher:

```xml
<!-- SUCCESS = llamada completada normalmente, sin tonos -->
<extension name="handle-bridge-success">
  <condition field="destination_number" expression="^_handle_bridge_failure_$"/>
  <condition field="${originate_disposition}" expression="^SUCCESS$">
    <action application="log" data="INFO Call completed successfully - normal hangup"/>
    <action application="hangup" data="NORMAL_CLEARING"/>
  </condition>
</extension>

<!-- Luego el dispatcher para casos de fallo -->
<extension name="handle-bridge-failure-dispatcher">
  ...
</extension>
```

### 4.4 Matriz de Disposiciones

| `originate_disposition` | Handler | Tono | Respuesta SIP | Q.850 |
|------------------------|---------|------|---------------|-------|
| `SUCCESS` | handle-bridge-success | Ninguno | Normal hangup | - |
| `USER_BUSY` | hangup-user-busy | Ocupado 4s | 486 Busy Here | 17 |
| `NO_ANSWER` | hangup-originator-cancel | Ocupado 7s | 486 Busy Here | 17 |
| `NO_USER_RESPONSE` | hangup-originator-cancel | Ocupado 7s | 486 Busy Here | 17 |
| `CALL_REJECTED` | hangup-call-rejected | Ocupado 2s | 603 Decline | 21 |
| `ORIGINATOR_CANCEL` | hangup-originator-cancel | Ocupado 7s | 486 Busy Here | 17 |
| `SUBSCRIBER_ABSENT` | hangup-unavailable | Congestión 3s | 480 Unavailable | 20 |
| `USER_NOT_REGISTERED` | hangup-unavailable | Congestión 3s | 480 Unavailable | 20 |
| Otros | hangup-congestion | Congestión 3s | 503 Service Unavailable | 42 |

---

## 5. Comandos de Verificación

### 5.1 FreeSWITCH

```bash
# Recargar dialplan
fs_cli -x "reloadxml"

# Verificar profile inbound
fs_cli -x "sofia status profile inbound"

# Ver registros de usuario
fs_cli -x "sofia status profile internal reg" | grep <extension>

# Obtener dial string
fs_cli -x "sofia_contact <user>@<domain>"

# Probar llamada
fs_cli -x "originate sofia/inbound/<dest>@10.10.22.4:5082 &park"

# Ver llamadas activas
fs_cli -x "show calls"
```

### 5.2 Kamailio

```bash
# Recargar rutas
kamcmd drouting.reload

# Ver reglas cargadas
kamcmd drouting.dump

# Ver gateways
mysql kamailio -e "SELECT * FROM dr_gateways;"

# Ver reglas
mysql kamailio -e "SELECT * FROM dr_rules WHERE groupid=9000;"
```

### 5.3 Logs

```bash
# FreeSWITCH
tail -f /var/log/freeswitch/freeswitch.log | grep -E "(6720|ENTEL|bridge|originate)"

# Kamailio
journalctl -u kamailio -f

# Captura SIP
sngrep -d ens18
```

---

## 6. Archivos Modificados

| Archivo | Cambio |
|---------|--------|
| `/etc/freeswitch/dialplan/to-kamailio.xml` | Extensión ENTEL + handler SUCCESS |
| `/etc/kamailio/kamailio.cfg` | Permisos socket CTL permanentes |
| `database/migrations/010_add_ims_mode.sql` | Campo ims_mode en routing_inbound_routes |
| `rust-backend/.../dto/routing.rs` | Campo ims_mode en DTOs |
| `rust-backend/.../handlers/unified_routing.rs` | Generación XML con IMS handlers |

---

## 7. Checklist de Verificación

- [ ] Permisos de `/etc/freeswitch/dialplan/to-kamailio.xml` (freeswitch:freeswitch, 664)
- [ ] Permisos de socket Kamailio (0660 en kamailio.cfg)
- [ ] IP del carrier en tabla `address` con grp=8
- [ ] Gateway FreeSWITCH (gwid=100, type=9) en `dr_gateways`
- [ ] Regla inbound (groupid=9000) apuntando a #100
- [ ] Profile inbound escuchando en puerto 5082
- [ ] Contexto del profile inbound = `to-kamailio`
- [ ] Extensión ENTEL con patrón `^(6720.+)$`
- [ ] Handler SUCCESS antes del dispatcher de fallos
- [ ] Softphone registrado y alcanzable
- [ ] Backend compilado con soporte IMS

---

## 8. Contacto y Soporte

Para problemas adicionales, revisar:
- `CLAUDE.md` - Documentación general del sistema
- `docs/RTP_FLOW_ARCHITECTURE.md` - Arquitectura de flujos RTP
- `docs/INTEGRATEL_CONGESTION_TONE_CONFIG.md` - Configuración de tonos INTEGRATEL
