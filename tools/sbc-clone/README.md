# sbc-clone — clonación del SBC en otro Debian 12

Clona este SBC (Kamailio + RTPEngine + FreeSWITCH + ApoloBilling) en un servidor
Debian 12 nuevo por SSH. Se ejecuta **en el SBC de origen, como root**.

```bash
sudo /opt/ApoloBilling/tools/sbc-clone/sbc-clone.sh
```

Pide:

- el acceso SSH al destino (IP, puerto, usuario root o con sudo, y contraseña o llave);
- el hostname y el dominio SIP;
- las IPs del destino: gestión/portal, señalización privada y pública, RTP privada y pública, y el rango RTP;
- el gateway equivalente para cada grupo de rutas estáticas del origen.

Las IPs ya deben estar configuradas en las interfaces del destino. El script lo comprueba antes de empezar.

Al final muestra y guarda `REPORTE.txt`, con la URL y las credenciales del portal, las de las
bases de datos, los demás secretos y el resultado de cada validación.

## Agentes

| Agente | Qué clona |
|---|---|
| `system` | hostname, `/etc/hosts`, zona horaria, locale, sysctl, límites, usuarios `apolo`/`dsiprouter`, rutas estáticas (`sbc-static-routes.service`) |
| `packages` | repositorios APT y llaves, los mismos paquetes y versiones, `.deb` de RTPEngine compilados en el origen, librerías sin paquete (libks, libjwt, libstirshaken, `stirshaken.so`), pip y npm globales |
| `database` | PostgreSQL `apolo_billing` y MariaDB `kamailio`: estructura completa y datos de configuración, sin CDRs, `acc` ni métricas; usuarios con contraseñas nuevas |
| `kamailio` | `kamailio.cfg` en producción, includes y certificados de `/etc/dsiprouter`, `kamctlrc`, override de systemd, rsyslog/logrotate |
| `rtpengine` | `rtpengine.conf` (interfaces y puertos nuevos), override, módulo de kernel, backend iptables-legacy |
| `freeswitch` | `/etc/freeswitch` completo y contraseña ESL nueva |
| `backend` | `/opt/ApoloBilling` con los binarios Rust ya compilados, `.env` con secretos nuevos, `apolo-backend` y `apolo-billing-engine` |
| `frontend` | portal compilado (`frontend/dist`) con las IPs nuevas, `apolo-frontend` |
| `monitoring` | `apolo-monitoring`, scripts de `/usr/local/bin`, timers, crontab de root, snmpd, nullmailer, Redis |
| `nginx` | sitio del portal con un certificado HTTPS autofirmado nuevo |
| `firewall` | firewalld con las mismas zonas, puertos y reglas; tiene reversión automática si se pierde el acceso SSH |

## Credenciales

Todas las credenciales del clon son nuevas, con una excepción: `SIP_ENCRYPTION_KEY` se conserva
porque cifra las contraseñas SIP que ya están en la base copiada.

Las contraseñas antiguas se buscan en las URLs de conexión y en `kamctlrc`, y se reemplazan donde
aparezcan, incluso escritas a mano en el código (el colector de MariaDB del monitoreo, por ejemplo).
Si dos cuentas compartían contraseña en el origen, siguen compartiendo la nueva.

Solo se cambia la contraseña del administrador del portal; los demás usuarios conservan la suya.

## Opciones

- `--params FILE`: reutiliza respuestas guardadas. Cada ejecución guarda las suyas, sin secretos, en `/var/tmp/sbc-clone/<fecha>/params.env`.
- `--capture-only`: solo arma el paquete en el origen, sin conectarse al destino.
- `--yes`: no pide la confirmación final.

## Qué no se clona

- El histórico: CDRs, `kamailio.acc` y métricas de monitoreo.
- falcon-sensor (requiere la licencia de la organización).
- Docker y OpenSIPS (no se usan).
- La GUI de dSIPRouter (está desconectada en el origen).
