#!/bin/bash

###############################################################################
# Script de Actualización Automática - Apolo Billing
# Descripción: Actualiza el repositorio local desde GitHub y configura el entorno
# Autor: GenSpark AI Developer
# Fecha: 2025-12-22
###############################################################################

set -e  # Exit on error

# Colores para output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Función de log
log_info() {
    echo -e "${BLUE}ℹ️  $1${NC}"
}

log_success() {
    echo -e "${GREEN}✅ $1${NC}"
}

log_warning() {
    echo -e "${YELLOW}⚠️  $1${NC}"
}

log_error() {
    echo -e "${RED}❌ $1${NC}"
}

# Banner
echo "═════════════════════════════════════════════════════════════"
echo "   🔄 Script de Actualización Automática - Apolo Billing   "
echo "═════════════════════════════════════════════════════════════"
echo ""

# Verificar directorio
if [ ! -d "/home/jbazan/ApoloBilling" ]; then
    log_error "Directorio /home/jbazan/ApoloBilling no encontrado"
    log_info "¿Deseas clonar el repositorio? (s/n)"
    read -r respuesta
    if [ "$respuesta" = "s" ]; then
        cd /home/jbazan
        log_info "Clonando repositorio desde GitHub..."
        git clone https://github.com/jesus-bazan-entel/ApoloBilling.git
        cd ApoloBilling
        git checkout genspark_ai_developer
        log_success "Repositorio clonado correctamente"
    else
        log_error "Operación cancelada"
        exit 1
    fi
fi

# Navegar al directorio
cd /home/jbazan/ApoloBilling

log_info "Directorio actual: $(pwd)"
echo ""

###############################################################################
# PASO 1: Actualizar repositorio desde GitHub
###############################################################################
log_info "PASO 1: Actualizando repositorio desde GitHub..."

# Verificar cambios locales
if [ -n "$(git status --porcelain)" ]; then
    log_warning "Hay cambios locales sin commit"
    log_info "¿Deseas descartarlos y actualizar? (s/n)"
    read -r respuesta
    if [ "$respuesta" = "s" ]; then
        git reset --hard HEAD
        git clean -fd
        log_success "Cambios locales descartados"
    else
        log_error "Actualización cancelada. Commit tus cambios primero."
        exit 1
    fi
fi

# Actualizar desde GitHub
log_info "Descargando últimos cambios..."
git fetch origin
git checkout genspark_ai_developer
git pull origin genspark_ai_developer

# Verificar commit actual
CURRENT_COMMIT=$(git rev-parse --short HEAD)
log_success "Repositorio actualizado - Commit: $CURRENT_COMMIT"
echo ""

# Mostrar últimos commits
log_info "Últimos 5 commits:"
git log --oneline --graph --decorate -5
echo ""

###############################################################################
# PASO 2: Verificar archivos nuevos
###############################################################################
log_info "PASO 2: Verificando archivos nuevos..."

if [ -f "backend/init_db_clean.py" ]; then
    log_success "backend/init_db_clean.py encontrado"
else
    log_warning "backend/init_db_clean.py NO encontrado"
fi

if [ -f "DESPLIEGUE_RESUMEN.txt" ]; then
    log_success "DESPLIEGUE_RESUMEN.txt encontrado"
else
    log_warning "DESPLIEGUE_RESUMEN.txt NO encontrado"
fi

if [ -f "ACTUALIZACION_LOCAL.md" ]; then
    log_success "ACTUALIZACION_LOCAL.md encontrado"
else
    log_warning "ACTUALIZACION_LOCAL.md NO encontrado"
fi

echo ""

###############################################################################
# PASO 3: Verificar dependencias del sistema
###############################################################################
log_info "PASO 3: Verificando dependencias del sistema..."

# Verificar Python 3.11
if command -v python3.11 &> /dev/null; then
    PYTHON_VERSION=$(python3.11 --version)
    log_success "Python 3.11 instalado: $PYTHON_VERSION"
else
    log_warning "Python 3.11 NO instalado"
    log_info "¿Deseas instalarlo ahora? (s/n)"
    read -r respuesta
    if [ "$respuesta" = "s" ]; then
        log_info "Instalando Python 3.11 y dependencias..."
        sudo apt update
        sudo apt install -y software-properties-common
        sudo add-apt-repository ppa:deadsnakes/ppa -y
        sudo apt update
        sudo apt install -y python3.11 python3.11-venv python3.11-dev \
            libpq-dev build-essential gcc
        log_success "Python 3.11 instalado correctamente"
    fi
fi

# Verificar PostgreSQL
if command -v psql &> /dev/null; then
    log_success "PostgreSQL instalado"
else
    log_warning "PostgreSQL NO instalado"
    log_info "Instala con: sudo apt install -y postgresql postgresql-contrib"
fi

# Verificar Redis
if command -v redis-server &> /dev/null; then
    log_success "Redis instalado"
else
    log_warning "Redis NO instalado"
    log_info "Instala con: sudo apt install -y redis-server"
fi

echo ""

###############################################################################
# PASO 4: Configurar entorno virtual
###############################################################################
log_info "PASO 4: Configurando entorno virtual..."

cd /home/jbazan/ApoloBilling/backend

# Eliminar venv anterior si existe
if [ -d "venv" ]; then
    log_warning "Entorno virtual anterior encontrado"
    log_info "¿Deseas recrearlo? (s/n)"
    read -r respuesta
    if [ "$respuesta" = "s" ]; then
        rm -rf venv
        log_success "Entorno virtual anterior eliminado"
    fi
fi

# Crear nuevo venv si no existe
if [ ! -d "venv" ]; then
    log_info "Creando nuevo entorno virtual con Python 3.11..."
    python3.11 -m venv venv
    log_success "Entorno virtual creado"
fi

# Activar venv
source venv/bin/activate

# Verificar versión de Python en venv
VENV_PYTHON_VERSION=$(python --version)
log_success "Python en venv: $VENV_PYTHON_VERSION"

# Actualizar pip
log_info "Actualizando pip..."
pip install --upgrade pip > /dev/null 2>&1
log_success "pip actualizado"

echo ""

###############################################################################
# PASO 5: Instalar/Actualizar dependencias Python
###############################################################################
log_info "PASO 5: Instalando dependencias de Python..."

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

log_info "Instalando paquetes (esto puede tardar unos minutos)..."
pip install -r requirements.txt > /dev/null 2>&1 && log_success "Dependencias instaladas correctamente" || log_error "Error instalando dependencias"

echo ""

###############################################################################
# PASO 6: Configurar archivo .env
###############################################################################
log_info "PASO 6: Configurando archivo .env..."

if [ -f ".env" ]; then
    log_warning "Archivo .env ya existe"
    log_info "¿Deseas sobrescribirlo? (s/n)"
    read -r respuesta
    if [ "$respuesta" != "s" ]; then
        log_info ".env mantenido sin cambios"
    else
        cat > .env << 'EOF'
PROJECT_NAME=Apolo Billing
API_V1_STR=/api
DATABASE_URL=postgresql://apolo_user:apolo_password_2024@localhost/apolo_billing
SECRET_KEY=desarrollo-secret-key-cambiar-en-produccion-123456789
SUPERADMIN_PASSWORD=admin123
EOF
        log_success "Archivo .env actualizado"
    fi
else
    cat > .env << 'EOF'
PROJECT_NAME=Apolo Billing
API_V1_STR=/api
DATABASE_URL=postgresql://apolo_user:apolo_password_2024@localhost/apolo_billing
SECRET_KEY=desarrollo-secret-key-cambiar-en-produccion-123456789
SUPERADMIN_PASSWORD=admin123
EOF
    log_success "Archivo .env creado"
fi

echo ""

###############################################################################
# PASO 7: Configurar base de datos
###############################################################################
log_info "PASO 7: Configurando base de datos PostgreSQL..."

log_info "¿Deseas reinicializar la base de datos? (s/n)"
log_warning "ADVERTENCIA: Esto eliminará todos los datos existentes"
read -r respuesta

if [ "$respuesta" = "s" ]; then
    log_info "Iniciando servicio PostgreSQL..."
    sudo service postgresql start
    
    log_info "Creando base de datos y usuario..."
    sudo -u postgres psql << 'EOF'
DROP DATABASE IF EXISTS apolo_billing;
DROP USER IF EXISTS apolo_user;
CREATE USER apolo_user WITH PASSWORD 'apolo_password_2024';
CREATE DATABASE apolo_billing OWNER apolo_user;
GRANT ALL PRIVILEGES ON DATABASE apolo_billing TO apolo_user;
EOF
    
    log_success "Base de datos configurada"
    
    log_info "Inicializando tablas con arquitectura limpia..."
    python init_db_clean.py
    log_success "Base de datos inicializada correctamente"
else
    log_info "Base de datos mantenida sin cambios"
fi

echo ""

###############################################################################
# PASO 8: Iniciar servicios
###############################################################################
log_info "PASO 8: Iniciando servicios..."

# Iniciar PostgreSQL
log_info "Iniciando PostgreSQL..."
sudo service postgresql start && log_success "PostgreSQL iniciado" || log_warning "Error iniciando PostgreSQL"

# Iniciar Redis
log_info "Iniciando Redis..."
sudo service redis-server start && log_success "Redis iniciado" || log_warning "Error iniciando Redis"

echo ""

###############################################################################
# RESUMEN FINAL
###############################################################################
echo "═════════════════════════════════════════════════════════════"
echo "                  ✅ ACTUALIZACIÓN COMPLETADA                 "
echo "═════════════════════════════════════════════════════════════"
echo ""
log_success "Sistema actualizado correctamente"
echo ""
log_info "📋 INFORMACIÓN DEL SISTEMA:"
echo "   • Commit actual: $CURRENT_COMMIT"
echo "   • Python: $VENV_PYTHON_VERSION"
echo "   • Base de datos: apolo_billing"
echo "   • Usuario: apolo_user"
echo ""
log_info "🚀 PARA INICIAR EL SERVIDOR:"
echo "   cd /home/jbazan/ApoloBilling/backend"
echo "   source venv/bin/activate"
echo "   uvicorn main:app --host 0.0.0.0 --port 8000 --reload"
echo ""
log_info "🌐 ACCESO AL SISTEMA:"
echo "   • URL: http://localhost:8000"
echo "   • Rate Cards: http://localhost:8000/dashboard/rate-cards"
echo "   • Usuario: admin"
echo "   • Password: admin123"
echo ""
log_info "📖 DOCUMENTACIÓN:"
echo "   • ACTUALIZACION_LOCAL.md - Guía detallada"
echo "   • DESPLIEGUE_RESUMEN.txt - Resumen de despliegue"
echo "   • LEGACY_CLEANUP_COMPLETED.md - Cambios de arquitectura"
echo ""
echo "═════════════════════════════════════════════════════════════"
