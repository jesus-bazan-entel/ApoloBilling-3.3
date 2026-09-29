# Integración ACQ (All Call Query) con Portax

## Descripción

El sistema ACQ (All Call Query) permite consultar el operador que sirve un número de teléfono en Perú, considerando la portabilidad numérica. Esto permite enrutar llamadas salientes al carrier correcto según el operador actual del número, no el operador original del bloque numérico.

**Formato de número saliente INTEGRATEL:** `RN_destino + RN_origen(67) + MSISDN`
- `RN_destino`: Código del operador que actualmente sirve el número (obtenido de ACQ)
- `RN_origen`: Código de FIBERTEL = 67
- `MSISDN`: Número telefónico de 9 dígitos

**Ejemplo:** Llamar a 938391839 (MOVISTAR, RN=22) → `2267938391839`

## Arquitectura

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                     FLUJO ACQ EN KAMAILIO (SINCRÓNICO)                      │
└─────────────────────────────────────────────────────────────────────────────┘

  Softphone disca: 938391839 (9 dígitos, sin prefijo)
            │
            ▼
  ┌───────────────────────────────────────────────────────────────────────────┐
  │   KAMAILIO - route[ACQ_LOOKUP]                                            │
  │                                                                           │
  │   1. Verifica que el número tiene 9 dígitos (formato peruano)             │
  │   2. Ejecuta curl SINCRÓNICAMENTE usando módulo exec:                     │
  │      curl -s --max-time 0.5 -H 'X-API-Key: <api_key>'                     │
  │           'http://10.10.22.11:8000/api/acq/938391839'                     │
  │   3. Captura respuesta JSON en $avp(acq_response)                         │
  │                                                                           │
  │   Respuesta JSON:                                                         │
  │   {"numero":"938391839","rn":"22","encontrado":true,"portado":false}      │
  │                                                                           │
  │   4. Parsea RN="22" con jansson                                           │
  │   5. Transforma número: 938391839 → 2267938391839                         │
  │      (RN_destino=22 + RN_origen=67 + MSISDN=938391839)                    │
  └───────────────────────┬───────────────────────────────────────────────────┘
                          │
                          ▼
  ┌───────────────────────────────────────────────────────────────────────────┐
  │   KAMAILIO - route[OUTBOUND_DROUTING]                                     │
  │                                                                           │
  │   6. do_routing(8000) busca prefijo "2267" en dr_rules                    │
  │   7. Match: prefix="2267" → gwlist=60 (INTEGRATEL)                        │
  │   8. Relay a INTEGRATEL                                                   │
  └───────────────────────────────────────────────────────────────────────────┘
```

**Nota técnica:** Se usa el módulo `exec` con curl en lugar de `http_async_client` porque
la API responde en <10ms y el enfoque sincrónico mantiene el contexto de la transacción SIP.

## Configuración

### 1. Parámetros en kamailio.cfg

Los parámetros se configuran en la sección de `cfg_rpc`:

```cfg
#!ifdef WITH_ACQ
# ACQ (All Call Query) - Portax Integration for Number Portability (Peru)
acq.enabled = 1 desc "Enable ACQ lookup for outbound calls"
acq.host = "http://10.10.22.11" desc "Portax ACQ API host (without trailing slash)"
acq.api_key = "YOUR_API_KEY_HERE" desc "API Key for Portax ACQ"
acq.timeout_ms = 500 desc "Timeout for ACQ query in milliseconds"
acq.prefix_mode = "prepend" desc "How to use RN: prepend (add RN prefix)"
#!endif
```

### 2. Habilitar ACQ

El feature flag ya está habilitado en la configuración:

```cfg
#!define WITH_ACQ
```

### 3. Configurar API Key

**IMPORTANTE:** Debes configurar la API key que te proporcionó el administrador de Portax:

```bash
# Editar kamailio.cfg
vi /etc/kamailio/kamailio.cfg

# Buscar la línea:
acq.api_key = ""

# Cambiar a:
acq.api_key = "tu_api_key_aqui"
```

### 4. Configurar Reglas de Enrutamiento (dr_rules)

Debes crear reglas en `dr_rules` que matcheen con los códigos de operador (RN).

**Códigos de Operador (RN) en Perú:**

| RN | Operador | Tipo |
|----|----------|------|
| 01 | CLARO | Móvil/Fijo |
| 02 | ENTEL | Móvil |
| 04 | BITEL | Móvil |
| 06 | AMERICA MOVIL | Móvil |
| 20 | TELEFONICA | Fijo |
| 22 | MOVISTAR | Móvil |

**Crear reglas en MySQL:**

```sql
-- Conectar a la base de datos de Kamailio
mysql -u kamailio -p kamailio

-- Ver reglas existentes
SELECT ruleid, groupid, prefix, gwlist, description FROM dr_rules WHERE groupid = 8000;

-- Agregar reglas por RN (ajustar gwlist según tu configuración)
-- gwlist=60 es INTEGRATEL en este ejemplo

INSERT INTO dr_rules (groupid, prefix, gwlist, description) VALUES
(8000, '01', '60', 'CLARO via INTEGRATEL'),
(8000, '02', '60', 'ENTEL via INTEGRATEL'),
(8000, '04', '60', 'BITEL via INTEGRATEL'),
(8000, '06', '60', 'AMERICA MOVIL via INTEGRATEL'),
(8000, '20', '60', 'TELEFONICA via INTEGRATEL'),
(8000, '22', '60', 'MOVISTAR via INTEGRATEL');
```

### 5. Recargar Configuración

```bash
# Validar sintaxis
kamailio -c -f /etc/kamailio/kamailio.cfg

# Reiniciar Kamailio (los cambios de cfg requieren reinicio)
systemctl restart kamailio

# Recargar reglas de drouting
kamcmd drouting.reload

# Verificar estado
systemctl status kamailio
```

## Flujo de Decisión

```
┌─────────────────────────────────────────────────────────────────────────────┐
│                           DECISIÓN ACQ                                      │
└─────────────────────────────────────────────────────────────────────────────┘

                    Número marcado
                          │
                          ▼
              ┌───────────────────────┐
              │ ¿Tiene 9 dígitos?     │
              └───────────┬───────────┘
                    │           │
                   SÍ          NO
                    │           │
                    ▼           ▼
           Consulta ACQ    Usar número original
                    │      (ya tiene prefijo)
                    ▼
              ┌───────────────────────┐
              │ ¿RN encontrado?       │
              └───────────┬───────────┘
                    │           │
                   SÍ          NO
                    │           │
                    ▼           ▼
           Prefijar con RN   Usar número original
           (ej: 22+número)   (ruta por defecto)
                    │           │
                    └─────┬─────┘
                          ▼
                     DROUTING
                    (dr_rules)
```

## Logs y Debugging

### Ver logs del ACQ

```bash
# En tiempo real
journalctl -u kamailio -f | grep -i acq

# Últimos logs
journalctl -u kamailio --since "5 minutes ago" | grep -i acq
```

### Mensajes de log esperados

**Consulta exitosa (número portado):**
```
ACQ: Querying http://10.10.22.11/api/acq/938391839 for 938391839
ACQ: 938391839 -> RN=22 (portado=true)
ACQ: Modified rU to 22938391839 for drouting
```

**Número no encontrado:**
```
ACQ: Number 938391839 not found in ACQ (rn=<null>, encontrado=false), using original
```

**ACQ deshabilitado o número no elegible:**
```
ACQ: Disabled, skipping lookup for 938391839
ACQ: Number 2267938391839 is not 9 digits, skipping ACQ
```

### Subir nivel de debug

```bash
# Subir verbosidad temporalmente
kamcmd debug.level 3

# Ver logs detallados
journalctl -u kamailio -f

# Bajar verbosidad después de debugging
kamcmd debug.level 1
```

## Pruebas

### 1. Probar API de Portax directamente

```bash
# Número portado
curl -s -H "X-API-Key: YOUR_API_KEY" http://10.10.22.11/api/acq/938391839 | jq

# Número no portado
curl -s -H "X-API-Key: YOUR_API_KEY" http://10.10.22.11/api/acq/14802000 | jq

# Medir latencia
curl -s -o /dev/null -w "%{http_code} %{time_total}s\n" \
     -H "X-API-Key: YOUR_API_KEY" http://10.10.22.11/api/acq/938391839
```

### 2. Probar llamada completa

```bash
# Ver tráfico SIP
sngrep host 10.35.13.100

# Hacer llamada de prueba desde softphone
# Discar: 938391839 (sin prefijo)

# Verificar en logs que ACQ agregó prefijo
journalctl -u kamailio --since "1 minute ago" | grep -i acq
```

## Troubleshooting

| Síntoma | Causa Probable | Solución |
|---------|----------------|----------|
| ACQ no se consulta | `acq.enabled = 0` o número no tiene 9 dígitos | Verificar config y formato del número |
| HTTP timeout | Portax no accesible | Verificar red: `curl http://10.10.22.11/api/acq/TEST` |
| 401 Unauthorized | API key incorrecta | Verificar `acq.api_key` en config |
| No match en drouting | Falta regla para el RN | Agregar regla en dr_rules para el prefijo RN |
| Llamada no sale | Gateway no disponible | Verificar estado del carrier en Kamailio |

## Compatibilidad con Sistema Actual

El ACQ es **compatible** con el sistema de prefijos de marcación existente:

| Escenario | Número Marcado | ACQ | Resultado |
|-----------|----------------|-----|-----------|
| Usuario marca con prefijo | `2267938391839` | No consulta | Usa prefijo existente |
| Usuario marca sin prefijo | `938391839` | Consulta | Agrega RN como prefijo |
| Número no encontrado | `938391839` | Consulta, no encuentra | Usa número original |

## Notas Operativas

1. **Sincronización de datos:** Los datos de Portax se sincronizan diariamente desde MEDIAFON. Un número recién portado aparece en el ACQ al día siguiente.

2. **Latencia:** La API de Portax responde en < 1ms (índice local). El timeout de 500ms es conservador.

3. **Fallback:** Si el ACQ falla o no responde, la llamada continúa con el número original.

4. **Mapeo RN → Gateway:** El mapeo de código de operador a gateway/carrier es configuración local (dr_rules). Portax solo entrega el código de operador.

---

**Última actualización:** 2026-07-20
**Versión:** 1.0
