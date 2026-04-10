# ApoloBilling - Instalador Automatizado

```
     █████╗ ██████╗  ██████╗ ██╗      ██████╗
    ██╔══██╗██╔══██╗██╔═══██╗██║     ██╔═══██╗
    ███████║██████╔╝██║   ██║██║     ██║   ██║
    ██╔══██║██╔═══╝ ██║   ██║██║     ██║   ██║
    ██║  ██║██║     ╚██████╔╝███████╗╚██████╔╝
    ╚═╝  ╚═╝╚═╝      ╚═════╝ ╚══════╝ ╚═════╝
```

## Instalación Rápida (Una Línea)

```bash
curl -sSL https://raw.githubusercontent.com/jesus-bazan-entel/ApoloBillingv3/main/install/apolobilling.sh | sudo bash -s install
```

## Instalación Manual

```bash
# Clonar repositorio
git clone https://github.com/jesus-bazan-entel/ApoloBillingv3.git /opt/ApoloBilling

# Ejecutar instalador
cd /opt/ApoloBilling/install
chmod +x apolobilling.sh
sudo ./apolobilling.sh install
```

## Comandos Disponibles

| Comando | Descripción |
|---------|-------------|
| `install` | Instala ApoloBilling completo |
| `uninstall` | Desinstala ApoloBilling |
| `upgrade` | Actualiza a la última versión |
| `status` | Muestra estado de servicios |
| `help` | Muestra ayuda |

```bash
# Ejemplos
sudo ./apolobilling.sh install
sudo ./apolobilling.sh status
sudo ./apolobilling.sh upgrade
```

## Requisitos del Sistema

| Recurso | Mínimo | Recomendado |
|---------|--------|-------------|
| OS | Debian 11+ | Debian 12 |
| CPU | 4 cores | 8+ cores |
| RAM | 8 GB | 16+ GB |
| Disco | 50 GB SSD | 100+ GB SSD |

## Qué se Instala

El script instala y configura automáticamente:

1. **Bases de Datos**
   - PostgreSQL 15 (apolo_billing)
   - MariaDB (kamailio)
   - Redis

2. **Lenguajes/Runtime**
   - Rust (última versión estable)
   - Node.js 20.x

3. **Componentes ApoloBilling**
   - Backend Rust (API REST)
   - Billing Engine (facturación en tiempo real)
   - Frontend React (interfaz web)

4. **Servicios**
   - Nginx (reverse proxy)
   - Systemd services

## Puertos

```
Públicos:
  :80/:443    Nginx (Web UI)
  :5060       Kamailio (SIP)
  :5080       FreeSWITCH (SIP interno)
  :10000-20000  RTPEngine (media)

Internos:
  :8000       Backend API
  :9000       Billing Engine
  :5432       PostgreSQL
  :6379       Redis
  :3306       MySQL
  :8021       FreeSWITCH ESL
```

## Post-Instalación

Después de la instalación:

1. **Acceder al sistema:**
   - URL: `http://<IP_SERVIDOR>`
   - Usuario: `admin`
   - Password: `admin123`

2. **Cambiar la contraseña del admin inmediatamente**

3. **Las credenciales se guardan en:**
   ```
   /opt/ApoloBilling/credentials.txt
   ```
   **¡Eliminar este archivo después de guardar las credenciales!**

4. **Configurar SSL (producción):**
   ```bash
   apt install -y certbot python3-certbot-nginx
   certbot --nginx -d tu-dominio.com
   ```

## Estructura de Archivos

```
/opt/ApoloBilling/
├── install/
│   ├── apolobilling.sh      # Script principal
│   ├── README.md             # Esta documentación
│   └── docs/
│       └── INSTALLATION_GUIDE.html
├── rust-backend/
│   ├── .env                  # Configuración (generada)
│   └── target/release/       # Binario compilado
├── rust-billing-engine/
│   ├── .env                  # Configuración (generada)
│   └── target/release/       # Binario compilado
├── frontend/
│   ├── .env                  # Configuración (generada)
│   └── dist/                 # Build de producción
├── database/
│   ├── schema.sql
│   └── migrations/
└── credentials.txt           # Credenciales (eliminar después)
```

## Logs

```bash
# Ver logs del backend
journalctl -u apolo-backend -f

# Ver logs del billing engine
journalctl -u apolo-billing-engine -f

# Ver log de instalación
cat /var/log/apolobilling/install.log
```

## Troubleshooting

### El backend no inicia

```bash
# Ver error específico
journalctl -u apolo-backend -n 50 --no-pager

# Verificar que PostgreSQL está corriendo
systemctl status postgresql
pg_isready

# Verificar configuración
cat /opt/ApoloBilling/rust-backend/.env
```

### Frontend muestra página en blanco

```bash
# Verificar que dist/ existe
ls -la /opt/ApoloBilling/frontend/dist/

# Verificar Nginx
nginx -t
systemctl status nginx

# Verificar que el backend responde
curl http://localhost:8000/api/v1/health
```

### Error de conexión a base de datos

```bash
# Verificar conexión PostgreSQL
sudo -u postgres psql -c "\l"

# Verificar usuario
sudo -u postgres psql -c "\du"

# Probar conexión
psql -U apolo_user -h localhost -d apolo_billing
```

## Actualización

```bash
cd /opt/ApoloBilling/install
sudo ./apolobilling.sh upgrade
```

Esto:
1. Hace backup de la configuración
2. Descarga la última versión
3. Recompila los componentes
4. Ejecuta nuevas migraciones
5. Reinicia los servicios

## Desinstalación

```bash
sudo ./apolobilling.sh uninstall
```

## Licencia

MIT License

## Soporte

- **Repositorio:** https://github.com/jesus-bazan-entel/ApoloBillingv3
- **Issues:** https://github.com/jesus-bazan-entel/ApoloBillingv3/issues
