# 🚀 Guía de Despliegue - Apolo Billing en WSL Debian

## 📌 Requisitos del Sistema

- Windows 10/11 con WSL2
- Debian instalado en WSL
- Al menos 4GB de RAM disponible
- 10GB de espacio en disco

---

## 📦 Instalación de Dependencias

### 1️⃣ Actualizar Sistema Debian

```bash
sudo apt update && sudo apt upgrade -y
```

### 2️⃣ Instalar Python 3.11+ y pip

```bash
# Instalar Python
sudo apt install python3 python3-pip python3-venv -y

# Verificar versión
python3 --version  # Debe ser >= 3.11
```

### 3️⃣ Instalar PostgreSQL

```bash
# Instalar PostgreSQL
sudo apt install postgresql postgresql-contrib -y

# Iniciar servicio
sudo service postgresql start

# Verificar estado
sudo service postgresql status
```

### 4️⃣ Instalar Redis

```bash
# Instalar Redis
sudo apt install redis-server -y

# Iniciar servicio
sudo service redis-server start

# Verificar
redis-cli ping  # Debe responder "PONG"
```

### 5️⃣ Instalar Rust (para el Billing Engine)

```bash
# Instalar Rust
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh

# Cargar entorno
source $HOME/.cargo/env

# Verificar
rustc --version
cargo --version
```

### 6️⃣ Instalar Node.js (opcional, para herramientas frontend)

```bash
# Instalar Node.js
curl -fsSL https://deb.nodesource.com/setup_20.x | sudo -E bash -
sudo apt install nodejs -y

# Verificar
node --version
npm --version
```

---

## 🗄️ Configuración de Base de Datos

### 1️⃣ Crear Usuario y Base de Datos PostgreSQL

```bash
# Acceder como usuario postgres
sudo -u postgres psql

# Dentro de PostgreSQL, ejecutar:
CREATE USER apolo_user WITH PASSWORD 'apolo_password_2024';
CREATE DATABASE apolo_billing OWNER apolo_user;
GRANT ALL PRIVILEGES ON DATABASE apolo_billing TO apolo_user;

# Salir
\q
```

### 2️⃣ Verificar Conexión

```bash
# Probar conexión
psql -h localhost -U apolo_user -d apolo_billing -c "SELECT version();"
# Ingresar password: apolo_password_2024
```

### 3️⃣ Configurar PostgreSQL para Escuchar en localhost

```bash
# Editar postgresql.conf
sudo nano /etc/postgresql/*/main/postgresql.conf

# Buscar y descomentar:
listen_addresses = 'localhost'

# Editar pg_hba.conf
sudo nano /etc/postgresql/*/main/pg_hba.conf

# Agregar al final:
host    apolo_billing    apolo_user    127.0.0.1/32    md5

# Reiniciar PostgreSQL
sudo service postgresql restart
```

---

## 📂 Clonar y Configurar Proyecto

### 1️⃣ Clonar Repositorio

```bash
# Ir a tu directorio de proyectos
cd ~
mkdir -p projects
cd projects

# Clonar el repositorio
git clone https://github.com/jesus-bazan-entel/ApoloBilling.git
cd ApoloBilling

# Cambiar a branch de desarrollo
git checkout genspark_ai_developer

# Verificar archivos
ls -la
```

### 2️⃣ Configurar Variables de Entorno - Backend Python

```bash
cd backend

# Crear archivo .env
cat > .env << 'EOF'
# Database Configuration
DATABASE_URL=postgresql://apolo_user:apolo_password_2024@localhost/apolo_billing

# Redis Configuration
REDIS_URL=redis://localhost:6379/0

# Application Settings
SECRET_KEY=your-secret-key-change-in-production
DEBUG=True
ALLOWED_HOSTS=localhost,127.0.0.1

# JWT Configuration
JWT_SECRET_KEY=your-jwt-secret-key-change-in-production
JWT_ALGORITHM=HS256
ACCESS_TOKEN_EXPIRE_MINUTES=30

# CORS Configuration
CORS_ORIGINS=http://localhost:8000,http://127.0.0.1:8000

# Logging
LOG_LEVEL=INFO
EOF

echo "✅ Backend .env creado"
```

### 3️⃣ Configurar Variables de Entorno - Rust Billing Engine

```bash
cd ../rust-billing-engine

# Crear archivo .env
cat > .env << 'EOF'
# Database Configuration
DATABASE_URL=postgresql://apolo_user:apolo_password_2024@localhost/apolo_billing

# Redis Configuration
REDIS_URL=redis://localhost:6379/0

# FreeSWITCH ESL Configuration
FREESWITCH_HOST=localhost
FREESWITCH_PORT=8021
FREESWITCH_PASSWORD=ClueCon

# Billing Configuration
INITIAL_RESERVATION_MINUTES=5
RESERVATION_BUFFER_PERCENT=8
MIN_RESERVATION_AMOUNT=0.30
MAX_RESERVATION_AMOUNT=30.00
RESERVATION_TTL=2700

# Application Settings
RUST_LOG=info
RUST_BACKTRACE=1
EOF

echo "✅ Rust billing engine .env creado"
cd ..
```

---

## 🐍 Instalación del Backend Python (FastAPI)

### 1️⃣ Crear Entorno Virtual

```bash
cd backend

# Crear entorno virtual
python3 -m venv venv

# Activar entorno virtual
source venv/bin/activate

# Actualizar pip
pip install --upgrade pip
```

### 2️⃣ Instalar Dependencias

```bash
# Instalar dependencias del proyecto
pip install -r requirements.txt

# Si hay error, instalar manualmente las principales:
pip install fastapi uvicorn sqlalchemy psycopg2-binary redis python-multipart jinja2
```

### 3️⃣ Inicializar Base de Datos

```bash
# Ejecutar script de inicialización
python init_database.py

# O usar alembic si está configurado
# alembic upgrade head
```

### 4️⃣ Crear Usuario Admin (Opcional)

```bash
# Crear script temporal
cat > create_admin.py << 'EOF'
from app.db.database import SessionLocal
from app.models.user import User
from passlib.context import CryptContext

pwd_context = CryptContext(schemes=["bcrypt"], deprecated="auto")

def create_admin():
    db = SessionLocal()
    
    # Verificar si ya existe
    existing = db.query(User).filter(User.username == "admin").first()
    if existing:
        print("❌ Usuario admin ya existe")
        return
    
    # Crear usuario admin
    admin = User(
        username="admin",
        email="admin@apolobilling.com",
        hashed_password=pwd_context.hash("admin123"),
        role="superadmin",
        is_active=True
    )
    
    db.add(admin)
    db.commit()
    print("✅ Usuario admin creado")
    print("   Username: admin")
    print("   Password: admin123")
    print("   ⚠️  Cambiar password en producción!")
    
    db.close()

if __name__ == "__main__":
    create_admin()
EOF

# Ejecutar
python create_admin.py
```

### 5️⃣ Iniciar Backend

```bash
# Método 1: Uvicorn directo (desarrollo)
uvicorn main:app --reload --host 0.0.0.0 --port 8000

# Método 2: Python directo
python main.py

# Método 3: Con más workers (producción)
uvicorn main:app --host 0.0.0.0 --port 8000 --workers 4
```

**Acceder al dashboard:**
- URL: http://localhost:8000
- Login: http://localhost:8000/login
- Dashboard: http://localhost:8000/dashboard/rate-cards

---

## 🦀 Compilación del Rust Billing Engine

### 1️⃣ Compilar Proyecto

```bash
cd rust-billing-engine

# Compilar en modo desarrollo
cargo build

# O compilar en modo release (más rápido)
cargo build --release
```

### 2️⃣ Ejecutar Billing Engine

```bash
# Modo desarrollo
cargo run

# Modo release (recomendado)
./target/release/apolo-billing-engine

# Con logs detallados
RUST_LOG=debug cargo run
```

### 3️⃣ Verificar Funcionamiento

El billing engine mostrará:
```
INFO apolo_billing_engine: Starting Apolo Billing Engine
INFO apolo_billing_engine: Database pool created
INFO apolo_billing_engine: Redis connection established
INFO apolo_billing_engine: FreeSWITCH ESL connection established
INFO apolo_billing_engine: HTTP API listening on 0.0.0.0:3000
```

---

## 🔧 Configuración de FreeSWITCH (Opcional)

Si no tienes FreeSWITCH instalado, puedes:

### Opción 1: Instalar FreeSWITCH en WSL

```bash
# Agregar repositorio
sudo apt install gnupg2 wget -y
wget -O - https://files.freeswitch.org/repo/deb/debian-release/fsstretch-archive-keyring.asc | sudo apt-key add -

# Agregar source
echo "deb http://files.freeswitch.org/repo/deb/debian-release/ `lsb_release -sc` main" | sudo tee /etc/apt/sources.list.d/freeswitch.list

# Instalar
sudo apt update
sudo apt install freeswitch-meta-all -y

# Iniciar
sudo systemctl start freeswitch
```

### Opción 2: Modo de Prueba (Sin FreeSWITCH)

Puedes comentar temporalmente la conexión ESL en el Rust engine:

```bash
cd rust-billing-engine/src
nano main.rs

# Comentar líneas de FreeSWITCH cluster:
// let freeswitch_cluster = FreeSwitchCluster::new(...).await?;
```

---

## 🧪 Verificación del Despliegue

### 1️⃣ Verificar Servicios

```bash
# PostgreSQL
sudo service postgresql status

# Redis
sudo service redis-server status
redis-cli ping

# Backend Python
curl http://localhost:8000/docs  # Swagger UI

# Rust Billing Engine (si está corriendo)
curl http://localhost:3000/health
```

### 2️⃣ Probar Dashboard

```bash
# Abrir en navegador Windows:
# 1. Abrir Chrome/Edge
# 2. Ir a: http://localhost:8000

# Credenciales de prueba:
# Username: admin
# Password: admin123
```

### 3️⃣ Probar API de Rate Cards

```bash
# Listar rate cards
curl http://localhost:8000/api/rate-cards | jq

# Crear rate card
curl -X POST http://localhost:8000/api/rate-cards \
  -H "Content-Type: application/json" \
  -d '{
    "destination_prefix": "51999",
    "destination_name": "Test Perú Móvil",
    "rate_per_minute": 0.08,
    "billing_increment": 6,
    "connection_fee": 0.0,
    "priority": 100
  }'

# Buscar rate card (LPM)
curl http://localhost:8000/api/rate-cards/search?destination=51999123456
```

---

## 🔄 Scripts de Automatización

### Script de Inicio Completo

```bash
cd ~/projects/ApoloBilling

cat > start_all.sh << 'EOF'
#!/bin/bash

echo "🚀 Iniciando Apolo Billing System..."

# Iniciar servicios
echo "📦 Iniciando PostgreSQL..."
sudo service postgresql start

echo "📦 Iniciando Redis..."
sudo service redis-server start

# Esperar a que servicios estén listos
sleep 3

# Iniciar Backend Python
echo "🐍 Iniciando Backend Python..."
cd backend
source venv/bin/activate
uvicorn main:app --host 0.0.0.0 --port 8000 --reload &
BACKEND_PID=$!

# Iniciar Rust Billing Engine (opcional)
# echo "🦀 Iniciando Rust Billing Engine..."
# cd ../rust-billing-engine
# cargo run --release &
# RUST_PID=$!

echo ""
echo "✅ Sistema iniciado exitosamente!"
echo ""
echo "📊 Dashboard UI:     http://localhost:8000"
echo "📚 API Docs:         http://localhost:8000/docs"
echo "🔧 Rate Cards UI:    http://localhost:8000/dashboard/rate-cards"
echo ""
echo "🛑 Para detener: Ctrl+C o ejecutar ./stop_all.sh"
echo ""
echo "Backend PID: $BACKEND_PID"

# Mantener script activo
wait
EOF

chmod +x start_all.sh
```

### Script de Detención

```bash
cat > stop_all.sh << 'EOF'
#!/bin/bash

echo "🛑 Deteniendo Apolo Billing System..."

# Detener procesos Python
pkill -f "uvicorn main:app"

# Detener Rust engine
pkill -f "apolo-billing-engine"

echo "✅ Sistema detenido"
EOF

chmod +x stop_all.sh
```

---

## 📊 Monitoreo y Logs

### Ver Logs del Backend

```bash
cd backend

# Ver logs en tiempo real
tail -f logs/apolo_billing.log

# O usar el output directo de uvicorn
```

### Ver Logs del Rust Engine

```bash
cd rust-billing-engine

# Logs van a stderr/stdout
# Para guardar en archivo:
cargo run 2>&1 | tee logs/billing_engine.log
```

---

## 🐛 Solución de Problemas Comunes

### Error: PostgreSQL no inicia

```bash
# Verificar logs
sudo tail -f /var/log/postgresql/postgresql-*-main.log

# Reiniciar servicio
sudo service postgresql restart

# Si persiste, reinstalar
sudo apt remove postgresql postgresql-contrib
sudo apt autoremove
sudo apt install postgresql postgresql-contrib
```

### Error: Redis connection refused

```bash
# Verificar configuración
sudo nano /etc/redis/redis.conf

# Buscar:
bind 127.0.0.1
port 6379

# Reiniciar
sudo service redis-server restart
```

### Error: Python dependencies no instalan

```bash
# Actualizar pip
pip install --upgrade pip setuptools wheel

# Instalar dependencias del sistema
sudo apt install python3-dev libpq-dev

# Reinstalar requirements
pip install -r requirements.txt --no-cache-dir
```

### Error: Rust no compila

```bash
# Actualizar Rust
rustup update

# Limpiar cache
cargo clean

# Recompilar
cargo build --release
```

### Puerto 8000 ya en uso

```bash
# Encontrar proceso
sudo lsof -i :8000

# Matar proceso
sudo kill -9 <PID>

# O usar otro puerto
uvicorn main:app --port 8080
```

---

## 🌐 Acceso desde Windows

### Opción 1: localhost (Recomendado)

El backend escucha en `0.0.0.0:8000`, accesible desde Windows:
```
http://localhost:8000
```

### Opción 2: IP de WSL

```bash
# Obtener IP de WSL
ip addr show eth0 | grep "inet\b" | awk '{print $2}' | cut -d/ -f1

# Acceder desde Windows:
http://<WSL_IP>:8000
```

### Opción 3: Port Forwarding (Windows Firewall)

```powershell
# En PowerShell como Administrador (Windows):
netsh interface portproxy add v4tov4 listenport=8000 listenaddress=0.0.0.0 connectport=8000 connectaddress=<WSL_IP>

# Ver reglas
netsh interface portproxy show all
```

---

## 📝 Checklist de Despliegue

- [ ] WSL Debian instalado y actualizado
- [ ] PostgreSQL instalado y corriendo
- [ ] Redis instalado y corriendo
- [ ] Python 3.11+ instalado
- [ ] Rust instalado (para billing engine)
- [ ] Repositorio clonado
- [ ] Base de datos creada y configurada
- [ ] Variables de entorno configuradas (backend/.env)
- [ ] Variables de entorno configuradas (rust-billing-engine/.env)
- [ ] Dependencias Python instaladas
- [ ] Base de datos inicializada
- [ ] Usuario admin creado
- [ ] Backend iniciado y accesible
- [ ] Dashboard carga correctamente
- [ ] API endpoints responden
- [ ] Rate Cards UI funciona
- [ ] (Opcional) Rust billing engine compilado
- [ ] (Opcional) FreeSWITCH configurado

---

## 🎯 Próximos Pasos

1. **Importar Rate Cards**: Usar CSV bulk import en `/dashboard/rate-cards`
2. **Configurar Usuarios**: Crear cuentas adicionales desde el admin
3. **Testing**: Probar todas las funcionalidades CRUD
4. **Monitoreo**: Configurar logs y métricas
5. **Backup**: Configurar backup automático de PostgreSQL

---

## 📞 Soporte

Si encuentras problemas:

1. Revisar logs del backend: `backend/logs/`
2. Revisar logs de PostgreSQL: `/var/log/postgresql/`
3. Verificar servicios: `sudo service --status-all`
4. Consultar documentación adicional:
   - `UI_MIGRATION_COMPLETED.md`
   - `MIGRATION_PLAN_RATE_CARDS.md`
   - `DATABASE_ANALYSIS.md`

---

**✅ ¡Despliegue Completo!**

El sistema Apolo Billing está listo para usar en tu entorno WSL Debian.
