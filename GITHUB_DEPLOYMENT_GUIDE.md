# 🚀 Guía Completa: GitHub + Despliegue en Producción

## 📋 **PARTE 1: SUBIR CÓDIGO A GITHUB**

### **Paso 1: Configurar Repositorio Local**

```bash
# 1. Inicializar repositorio Git (si no está inicializado)
git init

# 2. Configurar usuario Git (si no está configurado)
git config --global user.name "Jesús Bazán Entel"
git config --global user.email "jesus-bazan-entel@entel.pe"

# 3. Verificar estado actual
git status
```

### **Paso 2: Preparar Archivos para Commit**

```bash
# 1. Crear .gitignore para excluir archivos innecesarios
cat > .gitignore << 'EOF'
# Archivos de Python
__pycache__/
*.py[cod]
*$py.class
*.so
.Python
env/
venv/
ENV/
env.bak/
venv.bak/
pip-log.txt
.DS_Store
*.db
*.sqlite3

# Archivos de Java
target/
*.class
*.jar
*.war
*.ear
*.zip
*.tar.gz
*.rar

# Archivos de Node.js
node_modules/
npm-debug.log*
yarn-debug.log*
yarn-error.log*

# Archivos de Rust
target/
Cargo.lock

# Archivos de configuración local
.env.local
.env.production
.env.development

# Logs
logs/
*.log

# Archivos temporales
*.tmp
*.temp
*.swp
*.swo
*~

# Archivos del sistema
.DS_Store
Thumbs.db
EOF

# 2. Agregar todos los archivos
git add .

# 3. Ver qué se va a commitear
git status
```

### **Paso 3: Hacer Primer Commit**

```bash
# Crear commit inicial
git commit -m "Initial commit: ApoloBilling Complete System

- Complete billing engine with FreeSWITCH ESL integration
- Multi-language support (Python, Rust, Java, JavaScript)
- Database models and API endpoints
- FreeSWITCH simulation and testing tools
- Docker containerization support
- Complete deployment documentation"
```

### **Paso 4: Conectar con GitHub**

```bash
# 1. Agregar repositorio remoto
git remote add origin https://github.com/jesus-bazan-entel/ApoloBilling.git

# 2. Verificar remote
git remote -v

# 3. Subir código a GitHub
git branch -M main
git push -u origin main
```

### **Paso 5: Crear Tags de Versión**

```bash
# Crear tag para la versión actual
git tag -a v1.0.0 -m "Release v1.0.0: ApoloBilling System"

# Subir tag
git push origin v1.0.0
```

---

## 🌐 **PARTE 2: DESPLIEGUE EN PRODUCCIÓN**

### **Opción A: Despliegue con Docker (Recomendado)**

#### **Paso 1: Preparar Servidor de Producción**

```bash
# 1. Conectar al servidor
ssh usuario@servidor-produccion

# 2. Instalar Docker y Docker Compose
sudo apt update
sudo apt install docker.io docker-compose -y

# 3. Agregar usuario al grupo docker
sudo usermod -aG docker $USER

# 4. Salir y volver a entrar para aplicar cambios
exit
ssh usuario@servidor-produccion
```

#### **Paso 2: Crear Docker Compose para Producción**

```bash
# Crear directorio para el proyecto
mkdir -p /opt/ApoloBilling
cd /opt/ApoloBilling

# Clonar repositorio
git clone https://github.com/jesus-bazan-entel/ApoloBilling.git .
git checkout v1.0.0

# Crear archivo docker-compose.prod.yml
cat > docker-compose.prod.yml << 'EOF'
version: '3.8'

services:
  # Base de datos
  postgres:
    image: postgres:15-alpine
    container_name: apolo_postgres
    restart: unless-stopped
    environment:
      POSTGRES_DB: apolo_billing
      POSTGRES_USER: apolo_user
      POSTGRES_PASSWORD: ${DB_PASSWORD}
    volumes:
      - postgres_data:/var/lib/postgresql/data
      - ./backend/init_db.sql:/docker-entrypoint-initdb.d/init.sql
    ports:
      - "5432:5432"
    networks:
      - apolo_network

  # Redis Cache
  redis:
    image: redis:7-alpine
    container_name: apolo_redis
    restart: unless-stopped
    ports:
      - "6379:6379"
    networks:
      - apolo_network

  # Billing Engine (Rust)
  billing-engine:
    build:
      context: ./rust-billing-engine
      dockerfile: Dockerfile
    container_name: apolo_billing_engine
    restart: unless-stopped
    environment:
      DATABASE_URL: postgres://apolo_user:${DB_PASSWORD}@postgres:5432/apolo_billing
      REDIS_URL: redis://redis:6379
      ESL_HOST: ${ESL_HOST}
      ESL_PORT: ${ESL_PORT}
    depends_on:
      - postgres
      - redis
    ports:
      - "8080:8080"
    networks:
      - apolo_network

  # API Backend (Python)
  api-backend:
    build:
      context: ./backend
      dockerfile: Dockerfile
    container_name: apolo_api_backend
    restart: unless-stopped
    environment:
      DATABASE_URL: postgresql://apolo_user:${DB_PASSWORD}@postgres:5432/apolo_billing
      REDIS_URL: redis://redis:6379
      BILLING_ENGINE_URL: http://billing-engine:8080
    depends_on:
      - postgres
      - redis
      - billing-engine
    ports:
      - "8000:8000"
    networks:
      - apolo_network

  # ESL Listener
  esl-listener:
    build:
      context: .
      dockerfile: Dockerfile.esl
    container_name: apolo_esl_listener
    restart: unless-stopped
    environment:
      ESL_HOST: ${ESL_HOST}
      ESL_PORT: ${ESL_PORT}
      BACKEND_URL: http://api-backend:8000/api
    depends_on:
      - api-backend
    networks:
      - apolo_network

  # Web Frontend (Nginx)
  frontend:
    build:
      context: ./frontend
      dockerfile: Dockerfile
    container_name: apolo_frontend
    restart: unless-stopped
    depends_on:
      - api-backend
    ports:
      - "80:80"
      - "443:443"
    volumes:
      - ./ssl:/etc/nginx/ssl:ro
    networks:
      - apolo_network

volumes:
  postgres_data:

networks:
  apolo_network:
    driver: bridge
EOF
```

#### **Paso 3: Configurar Variables de Entorno**

```bash
# Crear archivo de variables de entorno
cat > .env.production << 'EOF'
# Database
DB_PASSWORD=TuPasswordSuperSeguro2024!

# FreeSWITCH
ESL_HOST=tu-freeswitch-ip
ESL_PORT=8021

# Security
JWT_SECRET=tu-jwt-secret-muy-seguro-2024
API_KEY=tu-api-key-secreta

# SSL (si usas HTTPS)
SSL_CERT_PATH=/etc/nginx/ssl/cert.pem
SSL_KEY_PATH=/etc/nginx/ssl/key.pem

# Monitoring
LOG_LEVEL=INFO
SENTRY_DSN=tu-sentry-dsn-opcional
EOF
```

#### **Paso 4: Construir y Desplegar**

```bash
# 1. Construir imágenes
docker-compose -f docker-compose.prod.yml build

# 2. Iniciar servicios
docker-compose -f docker-compose.prod.yml up -d

# 3. Verificar estado
docker-compose -f docker-compose.prod.yml ps

# 4. Ver logs
docker-compose -f docker-compose.prod.yml logs -f
```

---

### **Opción B: Despliegue Manual (Sin Docker)**

#### **Paso 1: Preparar Servidor**

```bash
# Conectar al servidor
ssh usuario@servidor-produccion

# Instalar dependencias
sudo apt update
sudo apt install python3 python3-pip python3-venv nodejs npm nginx postgresql postgresql-contrib redis-server -y

# Instalar Rust
curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh
source ~/.cargo/env
```

#### **Paso 2: Configurar Base de Datos**

```bash
# 1. Configurar PostgreSQL
sudo -u postgres psql
CREATE DATABASE apolo_billing;
CREATE USER apolo_user WITH PASSWORD 'TuPasswordSuperSeguro2024!';
GRANT ALL PRIVILEGES ON DATABASE apolo_billing TO apolo_user;
\q

# 2. Configurar Redis
sudo systemctl enable redis-server
sudo systemctl start redis-server
```

#### **Paso 3: Desplegar Aplicaciones**

```bash
# 1. Clonar repositorio
cd /opt
sudo git clone https://github.com/jesus-bazan-entel/ApoloBilling.git
cd ApoloBilling

# 2. Instalar y ejecutar Billing Engine (Rust)
cd rust-billing-engine
cargo build --release
sudo cp target/release/billing-engine /usr/local/bin/

# 3. Crear servicio systemd para billing engine
sudo tee /etc/systemd/system/apolo-billing-engine.service > /dev/null <<EOF
[Unit]
Description=Apolo Billing Engine
After=network.target

[Service]
Type=simple
User=apolo
ExecStart=/usr/local/bin/billing-engine
Restart=always
Environment=DATABASE_URL=postgresql://apolo_user:TuPasswordSuperSeguro2024!@localhost/apolo_billing

[Install]
WantedBy=multi-user.target
EOF

sudo systemctl enable apolo-billing-engine
sudo systemctl start apolo-billing-engine

# 4. Instalar y ejecutar API Backend (Python)
cd ../backend
python3 -m venv venv
source venv/bin/activate
pip install -r requirements.txt

# Crear servicio systemd para API
sudo tee /etc/systemd/system/apolo-api-backend.service > /dev/null <<EOF
[Unit]
Description=Apolo API Backend
After=network.target

[Service]
Type=simple
User=apolo
WorkingDirectory=/opt/ApoloBilling/backend
ExecStart=/opt/ApoloBilling/backend/venv/bin/uvicorn app.main:app --host 0.0.0.0 --port 8000
Restart=always

[Install]
WantedBy=multi-user.target
EOF

sudo systemctl enable apolo-api-backend
sudo systemctl start apolo-api-backend

# 5. Instalar ESL Listener
cd ..
python3 -m venv venv_esl
source venv_esl/bin/activate
pip install -r requirements.txt

# Crear servicio systemd para ESL Listener
sudo tee /etc/systemd/system/apolo-esl-listener.service > /dev/null <<EOF
[Unit]
Description=Apolo ESL Listener
After=network.target

[Service]
Type=simple
User=apolo
WorkingDirectory=/opt/ApoloBilling
ExecStart=/opt/ApoloBilling/venv_esl/bin/python connectors/freeswitch/esl_listener.py
Restart=always

[Install]
WantedBy=multi-user.target
EOF

sudo systemctl enable apolo-esl-listener
sudo systemctl start apolo-esl-listener
```

#### **Paso 4: Configurar Nginx**

```bash
# Crear configuración de Nginx
sudo tee /etc/nginx/sites-available/apolo-billing > /dev/null <<EOF
server {
    listen 80;
    server_name tu-dominio.com;

    # API Backend
    location /api/ {
        proxy_pass http://127.0.0.1:8000;
        proxy_set_header Host \$host;
        proxy_set_header X-Real-IP \$remote_addr;
        proxy_set_header X-Forwarded-For \$proxy_add_x_forwarded_for;
        proxy_set_header X-Forwarded-Proto \$scheme;
    }

    # Static files (si tienes frontend)
    location /static/ {
        alias /opt/ApoloBilling/static/;
        expires 1y;
        add_header Cache-Control "public, immutable";
    }

    # Frontend
    location / {
        root /opt/ApoloBilling/frontend;
        try_files \$uri \$uri/ /index.html;
    }
}
EOF

# Activar sitio
sudo ln -s /etc/nginx/sites-available/apolo-billing /etc/nginx/sites-enabled/
sudo nginx -t
sudo systemctl reload nginx
```

---

## 🔄 **ACTUALIZACIONES FUTURAS**

### **Para Actualizar el Sistema:**

```bash
# 1. En el servidor de producción
cd /opt/ApoloBilling
git pull origin main

# 2. Si hay cambios en el código:
docker-compose -f docker-compose.prod.yml build
docker-compose -f docker-compose.prod.yml up -d

# O para despliegue manual:
sudo systemctl stop apolo-billing-engine apolo-api-backend apolo-esl-listener
# Reconstruir y reiniciar servicios
sudo systemctl start apolo-billing-engine apolo-api-backend apolo-esl-listener
```

---

## 📊 **MONITOREO Y LOGS**

### **Verificar Estado de Servicios:**

```bash
# Docker
docker-compose -f docker-compose.prod.yml ps
docker-compose -f docker-compose.prod.yml logs

# Systemd
sudo systemctl status apolo-billing-engine
sudo systemctl status apolo-api-backend
sudo systemctl status apolo-esl-listener

# Logs en tiempo real
journalctl -u apolo-billing-engine -f
```

---

## 🚨 **CHECKLIST DE DESPLIEGUE**

- [ ] Repositorio creado en GitHub
- [ ] Código subido con commit inicial
- [ ] Tags de versión creados
- [ ] Servidor de producción preparado
- [ ] Docker/dependencias instaladas
- [ ] Base de datos configurada
- [ ] Variables de entorno configuradas
- [ ] Servicios iniciados
- [ ] Nginx configurado
- [ ] SSL/HTTPS configurado (opcional)
- [ ] Monitoreo activo
- [ ] Backups programados

---

## 🔒 **CONFIGURACIONES DE SEGURIDAD**

### **Firewall:**
```bash
sudo ufw allow ssh
sudo ufw allow 80/tcp
sudo ufw allow 443/tcp
sudo ufw enable
```

### **SSL con Let's Encrypt:**
```bash
sudo apt install certbot python3-certbot-nginx
sudo certbot --nginx -d tu-dominio.com
```

### **Backup Automático:**
```bash
# Script de backup
cat > /opt/backup.sh << 'EOF'
#!/bin/bash
DATE=$(date +%Y%m%d_%H%M%S)
tar -czf /backup/ApoloBilling_$DATE.tar.gz /opt/ApoloBilling
pg_dump -U apolo_user apolo_billing > /backup/db_$DATE.sql
find /backup -name "*.tar.gz" -mtime +7 -delete
find /backup -name "*.sql" -mtime +7 -delete
EOF

# Programar backup diario
echo "0 2 * * * /opt/backup.sh" | sudo crontab -
```

¡Tu sistema estará completamente desplegado y funcionando en producción! 🚀