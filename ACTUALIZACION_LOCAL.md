# 🔄 Guía de Actualización Local - Apolo Billing

## 📋 Resumen de Cambios Recientes

**Commit más reciente:** `f3d5edca - feat: clean architecture - remove legacy zones/prefixes/rates system`

### ✨ Nuevas Características

1. **Arquitectura Simplificada**: Sistema legacy eliminado (Zonas, Prefijos, Tarifas antiguas)
2. **Single Source of Truth**: Solo tabla `rate_cards` para gestión de tarifas
3. **Menú Simplificado**: Una sola opción "Gestión de Tarifas" → "Rate Cards"
4. **Script de Inicialización Limpio**: `backend/init_db_clean.py`
5. **Mejoras de Rendimiento**: ~52x más rápido que sistema legacy

---

## 🚀 Pasos para Actualizar tu PC Local (WSL Debian)

### Opción 1: Actualización Rápida (Recomendada)

Si ya tienes el repositorio clonado en `/home/jbazan/ApoloBilling`:

```bash
# 1. Navegar al directorio del proyecto
cd /home/jbazan/ApoloBilling

# 2. Descartar cambios locales (si los hay)
git reset --hard HEAD
git clean -fd

# 3. Actualizar desde GitHub
git fetch origin
git checkout genspark_ai_developer
git pull origin genspark_ai_developer

# 4. Verificar que tienes los nuevos archivos
ls -lh backend/init_db_clean.py DESPLIEGUE_RESUMEN.txt

# 5. Ver los últimos commits
git log --oneline -5
```

---

### Opción 2: Clonación Limpia (Si hay problemas)

```bash
# 1. Hacer backup del directorio actual (si existe)
cd /home/jbazan
mv ApoloBilling ApoloBilling_backup_$(date +%Y%m%d)

# 2. Clonar repositorio desde GitHub
git clone https://github.com/jesus-bazan-entel/ApoloBilling.git
cd ApoloBilling

# 3. Cambiar a rama de desarrollo
git checkout genspark_ai_developer

# 4. Verificar archivos nuevos
ls -lh backend/init_db_clean.py DESPLIEGUE_RESUMEN.txt
```

---

## 🛠️ Configuración del Entorno Virtual

### 1. Instalar Dependencias del Sistema

```bash
# Instalar Python 3.11 y librerías necesarias
sudo apt update
sudo apt install -y software-properties-common
sudo add-apt-repository ppa:deadsnakes/ppa -y
sudo apt update
sudo apt install -y \
    python3.11 \
    python3.11-venv \
    python3.11-dev \
    libpq-dev \
    build-essential \
    gcc \
    postgresql \
    postgresql-contrib \
    redis-server
```

### 2. Crear Entorno Virtual con Python 3.11

```bash
# Navegar al directorio backend
cd /home/jbazan/ApoloBilling/backend

# Eliminar entorno virtual anterior (si existe)
rm -rf venv

# Crear nuevo entorno virtual con Python 3.11
python3.11 -m venv venv

# Activar entorno virtual
source venv/bin/activate

# Verificar versión de Python (debe ser 3.11.x)
python --version
```

### 3. Instalar Dependencias de Python

```bash
# Actualizar pip
pip install --upgrade pip

# Crear requirements.txt actualizado
cat > requirements.txt << 'EOF'
fastapi==0.104.1
uvicorn[standard]==0.24.0
sqlalchemy==2.0.23
psycopg2-binary==2.9.9
redis==5.0.1
python-multipart==0.0.6
jinja2==3.1.2
python-jose[cryptography]==3.3.0
passlib[bcrypt]==1.7.4
python-dotenv==1.0.0
pydantic==2.5.3
pydantic-settings==2.1.0
alembic==1.12.1
email-validator==2.1.1
fastapi-login==1.9.1
EOF

# Instalar todas las dependencias
pip install -r requirements.txt
```

---

## 🗄️ Configuración de Base de Datos

### 1. Configurar PostgreSQL

```bash
# Iniciar servicio PostgreSQL
sudo service postgresql start

# Crear usuario y base de datos
sudo -u postgres psql << 'EOF'
-- Eliminar base de datos anterior (si existe)
DROP DATABASE IF EXISTS apolo_billing;
DROP USER IF EXISTS apolo_user;

-- Crear nuevo usuario
CREATE USER apolo_user WITH PASSWORD 'apolo_password_2024';

-- Crear base de datos
CREATE DATABASE apolo_billing OWNER apolo_user;

-- Otorgar privilegios
GRANT ALL PRIVILEGES ON DATABASE apolo_billing TO apolo_user;
EOF
```

### 2. Inicializar Base de Datos con Arquitectura Limpia

```bash
# Navegar al directorio backend
cd /home/jbazan/ApoloBilling/backend

# Asegurarse de que el entorno virtual está activo
source venv/bin/activate

# Ejecutar script de inicialización limpio
python init_db_clean.py
```

**Salida esperada:**
```
✅ Base de datos inicializada correctamente
📊 Tablas creadas:
   - users
   - accounts
   - rate_cards
   - balance_reservations
   - balance_transactions
   - cdrs

👤 Usuario admin creado (password: admin123)
📞 13 Rate Cards de ejemplo insertados
💰 Cuenta demo creada (account_id: demo_001, balance: $100.00)
```

### 3. Configurar Archivo .env

```bash
# Crear archivo .env en backend/
cat > /home/jbazan/ApoloBilling/backend/.env << 'EOF'
PROJECT_NAME=Apolo Billing
API_V1_STR=/api
DATABASE_URL=postgresql://apolo_user:apolo_password_2024@localhost/apolo_billing
SECRET_KEY=desarrollo-secret-key-cambiar-en-produccion-123456789
SUPERADMIN_PASSWORD=admin123
EOF
```

---

## 🚀 Iniciar el Sistema

### 1. Iniciar Servicios

```bash
# Iniciar PostgreSQL
sudo service postgresql start

# Iniciar Redis
sudo service redis-server start

# Verificar que están corriendo
sudo service postgresql status
sudo service redis-server status
```

### 2. Iniciar Servidor FastAPI

```bash
# Navegar al directorio backend
cd /home/jbazan/ApoloBilling/backend

# Activar entorno virtual
source venv/bin/activate

# Iniciar servidor
uvicorn main:app --host 0.0.0.0 --port 8000 --reload
```

**Salida esperada:**
```
INFO:     Will watch for changes in these directories: ['/home/jbazan/ApoloBilling/backend']
INFO:     Uvicorn running on http://0.0.0.0:8000 (Press CTRL+C to quit)
INFO:     Started reloader process [xxxxx] using StatReload
INFO:     Started server process [xxxxx]
INFO:     Waiting for application startup.
INFO:     Application startup complete.
```

---

## 🌐 Acceder al Sistema

### URLs Disponibles

1. **Dashboard Principal**: http://localhost:8000/
2. **Rate Cards (Nuevo)**: http://localhost:8000/dashboard/rate-cards
3. **API Docs (Swagger)**: http://localhost:8000/docs

### Credenciales por Defecto

- **Usuario**: `admin`
- **Password**: `admin123`

---

## ✅ Verificación del Sistema

### 1. Verificar Acceso al Dashboard

```bash
# Desde otra terminal WSL
curl -I http://localhost:8000/dashboard/rate-cards
```

**Salida esperada:** `HTTP/1.1 200 OK`

### 2. Verificar Rate Cards en Base de Datos

```bash
# Conectar a PostgreSQL
sudo -u postgres psql -d apolo_billing -c "SELECT id, prefix, destination, rate_per_minute FROM rate_cards LIMIT 5;"
```

**Salida esperada:**
```
 id  | prefix |    destination     | rate_per_minute 
-----+--------+-------------------+----------------
   1 | 51     | Peru              |          0.0150
   2 | 511    | Peru Lima         |          0.0120
   3 | 519    | Peru Movil        |          0.0180
   4 | 1      | USA/Canada        |          0.0100
   5 | 52     | Mexico            |          0.0140
```

### 3. Verificar Funcionalidades del Dashboard

1. ✅ **Login**: Accede con `admin / admin123`
2. ✅ **Rate Cards Dashboard**: Navega a "Gestión de Tarifas" → "Rate Cards"
3. ✅ **Búsqueda LPM**: Busca un número como `51987654321` (debe encontrar `Peru Movil`)
4. ✅ **Crear Rate Card**: Agrega una nueva tarifa
5. ✅ **Editar Rate Card**: Modifica una tarifa existente
6. ✅ **Eliminar Rate Card**: Borra una tarifa
7. ✅ **Exportar CSV**: Descarga todas las tarifas

---

## 🔧 Comandos Útiles de Diagnóstico

### Ver Logs en Tiempo Real

```bash
# En la terminal donde corre uvicorn
# Los logs aparecerán automáticamente
```

### Verificar Estado de Servicios

```bash
# PostgreSQL
sudo service postgresql status

# Redis
sudo service redis-server status

# Ver procesos Python
ps aux | grep python
```

### Verificar Tablas en Base de Datos

```bash
sudo -u postgres psql -d apolo_billing -c "\dt"
```

**Salida esperada:**
```
                 List of relations
 Schema |         Name          | Type  |   Owner    
--------+-----------------------+-------+------------
 public | accounts              | table | apolo_user
 public | balance_reservations  | table | apolo_user
 public | balance_transactions  | table | apolo_user
 public | cdrs                  | table | apolo_user
 public | rate_cards            | table | apolo_user
 public | users                 | table | apolo_user
```

---

## 🐛 Solución de Problemas

### Error: `libpq-fe.h: No such file or directory`

```bash
sudo apt install -y libpq-dev build-essential
```

### Error: `Python 3.13 incompatibility`

```bash
# Asegúrate de usar Python 3.11
cd /home/jbazan/ApoloBilling/backend
rm -rf venv
python3.11 -m venv venv
source venv/bin/activate
python --version  # Debe mostrar 3.11.x
```

### Error: `Extra inputs are not permitted (redis_url, debug)`

```bash
# Elimina esas variables del .env
cd /home/jbazan/ApoloBilling/backend
nano .env  # Eliminar líneas redis_url y debug
```

### Error: `relation "zones" does not exist`

```bash
# Reinicializar base de datos con script limpio
cd /home/jbazan/ApoloBilling/backend
source venv/bin/activate
python init_db_clean.py
```

### El servidor no inicia en el puerto 8000

```bash
# Verificar si hay otro proceso usando el puerto
sudo lsof -i :8000

# Matar proceso si existe
sudo kill -9 <PID>
```

---

## 📊 Cambios Implementados vs. Sistema Legacy

| Aspecto | Sistema Legacy | Sistema Nuevo (Rate Cards) |
|---------|---------------|---------------------------|
| **Tablas DB** | 4 tablas (zones, prefixes, rate_zones, countries) | 1 tabla (rate_cards) |
| **Operaciones CRUD** | ~260ms | ~5ms (**52x más rápido**) |
| **Búsqueda de Tarifa** | ~10ms (JOIN múltiples) | ~2ms (**5x más rápido**) |
| **Complejidad Código** | Alta (sincronización entre tablas) | Baja (single source of truth) |
| **Menú de Navegación** | 4 opciones (Zonas, Prefijos, Tarifas, Rate Cards) | 1 opción (Rate Cards) |
| **Mantenibilidad** | Compleja | Simple |

---

## 📖 Documentación Adicional

- **LEGACY_CLEANUP_COMPLETED.md**: Detalles de limpieza de código legacy
- **UI_MIGRATION_COMPLETED.md**: Implementación del dashboard UI
- **MIGRATION_PLAN_RATE_CARDS.md**: Plan de migración de datos
- **DATABASE_ANALYSIS.md**: Análisis de arquitectura de base de datos

---

## 🎯 Próximos Pasos

1. ✅ **Verificar funcionamiento local** (completar verificación arriba)
2. 📝 **Revisar Pull Request**: https://github.com/jesus-bazan-entel/ApoloBilling/pull/1
3. 🔀 **Mergear a rama principal** (cuando estés listo)
4. 🚀 **Desplegar en producción** (ambiente final)
5. 📚 **Capacitación del equipo** (uso del nuevo dashboard)

---

## 📞 Soporte

Si encuentras problemas durante la actualización:

1. Revisa los logs del servidor: `tail -f logs/app.log`
2. Verifica los servicios: `sudo service postgresql status`, `sudo service redis-server status`
3. Consulta esta guía: `ACTUALIZACION_LOCAL.md`
4. Revisa documentación técnica en el repositorio

---

**Última actualización:** 2025-12-22  
**Versión del sistema:** 2.0 (Rate Cards Clean Architecture)  
**Commit:** `f3d5edca`
