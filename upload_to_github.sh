#!/bin/bash

# =================================================================
# 🚀 SCRIPT AUTOMATIZADO: SUBIR CÓDIGO A GITHUB
# =================================================================
# Autor: ApoloBilling Deployment Script
# Fecha: 2024-01-12
# Repositorio: https://github.com/jesus-bazan-entel/ApoloBilling
# =================================================================

set -e  # Salir en caso de error

# Colores para output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m' # No Color

# Función para imprimir con colores
print_status() {
    echo -e "${BLUE}[INFO]${NC} $1"
}

print_success() {
    echo -e "${GREEN}[ÉXITO]${NC} $1"
}

print_warning() {
    echo -e "${YELLOW}[ADVERTENCIA]${NC} $1"
}

print_error() {
    echo -e "${RED}[ERROR]${NC} $1"
}

# Banner
echo -e "${BLUE}"
echo "=================================================="
echo "🚀 APOLOBILLING - SUBIDA A GITHUB AUTOMÁTICA"
echo "=================================================="
echo -e "${NC}"

# Verificar si estamos en el directorio correcto
print_status "Verificando directorio actual..."
if [ ! -f "main.py" ] && [ ! -f "backend/main.py" ]; then
    print_error "No se encontraron archivos del proyecto ApoloBilling."
    print_error "Asegúrate de ejecutar este script desde el directorio raíz del proyecto."
    exit 1
fi
print_success "Directorio correcto encontrado."

# Paso 1: Configurar Git
print_status "Configurando Git..."

# Solicitar información del usuario
read -p "Ingresa tu nombre para Git: " GIT_NAME
read -p "Ingresa tu email para Git: " GIT_EMAIL

# Configurar Git localmente para este proyecto
git config user.name "$GIT_NAME"
git config user.email "$GIT_EMAIL"

print_success "Git configurado."

# Paso 2: Crear .gitignore
print_status "Creando archivo .gitignore..."

cat > .gitignore << 'EOF'
# =================================================================
# APOLOBILLING - ARCHIVOS EXCLUIDOS DE GITHUB
# =================================================================

# Python
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
pip-delete-this-directory.txt
.DS_Store
*.db
*.sqlite3
*.sqlite
tarificador.db

# Java
target/
*.class
*.jar
*.war
*.ear
*.zip
*.tar.gz
*.rar

# Node.js
node_modules/
npm-debug.log*
yarn-debug.log*
yarn-error.log*

# Rust
target/
Cargo.lock

# Configuración local
.env.local
.env.production
.env.development
backend/.env
rust-billing-engine/.env

# Logs
logs/
*.log
npm-debug.log*
yarn-debug.log*
yarn-error.log*

# Archivos temporales
*.tmp
*.temp
*.swp
*.swo
*~
.DS_Store
Thumbs.db

# Archivos de backup
*.backup
*.bak
*.old

# IDEs
.vscode/
.idea/
*.swp
*.swo

# Archivos específicos del proyecto
password_reset.txt
com.tarificador
*.whl
*.tar.gz

# Archivos del sistema
*.pid
*.seed
*.pid.lock

# Certificados SSL (si existen)
*.pem
*.key
*.crt
ssl/

# Base de datos de desarrollo
*.db
*.sqlite
*.sqlite3
EOF

print_success "Archivo .gitignore creado."

# Paso 3: Verificar estado de Git
print_status "Verificando estado de Git..."

if [ ! -d ".git" ]; then
    print_status "Inicializando repositorio Git..."
    git init
    print_success "Repositorio Git inicializado."
else
    print_success "Repositorio Git ya existe."
fi

# Paso 4: Verificar si ya existe el remote
print_status "Verificando repositorio remoto..."

if git remote get-url origin > /dev/null 2>&1; then
    print_warning "El remote 'origin' ya existe."
    read -p "¿Deseas actualizar la URL del remote? (y/n): " UPDATE_REMOTE
    if [ "$UPDATE_REMOTE" = "y" ] || [ "$UPDATE_REMOTE" = "Y" ]; then
        git remote set-url origin https://github.com/jesus-bazan-entel/ApoloBilling.git
        print_success "URL del remote actualizada."
    fi
else
    print_status "Agregando repositorio remoto..."
    git remote add origin https://github.com/jesus-bazan-entel/ApoloBilling.git
    print_success "Repositorio remoto agregado."
fi

# Paso 5: Mostrar archivos que se van a commitear
print_status "Archivos que se incluirán en el commit:"
git status --porcelain | head -20

# Paso 6: Confirmar antes del commit
print_status "Preparando para el commit..."
echo ""
read -p "¿Continuar con el commit y subida a GitHub? (y/n): " CONTINUE
if [ "$CONTINUE" != "y" ] && [ "$CONTINUE" != "Y" ]; then
    print_warning "Operación cancelada por el usuario."
    exit 0
fi

# Paso 7: Agregar archivos
print_status "Agregando archivos al staging..."
git add .
print_success "Archivos agregados al staging."

# Paso 8: Crear commit
print_status "Creando commit inicial..."

COMMIT_MESSAGE="🚀 Initial commit: ApoloBilling Complete System

✨ Features:
- Complete billing engine with FreeSWITCH ESL integration
- Multi-language support (Python, Rust, Java, JavaScript)  
- Database models and API endpoints
- FreeSWITCH simulation and testing tools
- Docker containerization support
- PostgreSQL and Redis integration
- ESL (Event Socket Library) communication
- Complete deployment documentation
- Web interface with Nginx
- System monitoring and logging

🛠️ Technologies:
- Backend: Python FastAPI + Rust Billing Engine
- Database: PostgreSQL with async support
- Cache: Redis for session management
- Frontend: Web interface
- Containerization: Docker & Docker Compose
- Communication: FreeSWITCH ESL protocol
- Web Server: Nginx reverse proxy

📋 Documentation:
- Complete technical specifications
- Deployment guides (Docker & Manual)
- API documentation
- Testing procedures
- Maintenance guides"

git commit -m "$COMMIT_MESSAGE"
print_success "Commit creado exitosamente."

# Paso 9: Configurar rama principal
print_status "Configurando rama principal..."
git branch -M main
print_success "Rama principal configurada."

# Paso 10: Solicitar credenciales y subir
print_status "Preparando para subir a GitHub..."
echo ""
print_warning "IMPORTANTE: Se te pedirá tu token personal de GitHub."
echo ""
echo "📋 Instrucciones para crear tu token:"
echo "1. Ve a GitHub.com → Settings → Developer settings → Personal access tokens"
echo "2. Click 'Generate new token (classic)'"
echo "3. Selecciona los permisos: 'repo', 'workflow', 'write:packages'"
echo "4. Copia el token y pégalo cuando se solicite"
echo ""

# Leer token de forma segura
read -s -p "Ingresa tu token personal de GitHub: " GITHUB_TOKEN
echo ""

# Configurar URL del remote con token
REMOTE_URL="https://$GITHUB_TOKEN@github.com/jesus-bazan-entel/ApoloBilling.git"
git remote set-url origin "$REMOTE_URL"

# Paso 11: Subir a GitHub
print_status "Subiendo código a GitHub..."
git push -u origin main

print_success "✅ Código subido exitosamente a GitHub!"

# Paso 12: Crear tag de versión
print_status "Creando tag de versión..."
git tag -a v1.0.0 -m "🎉 Release v1.0.0: ApoloBilling Production Ready

🚀 Initial production release with:
- Complete billing system functionality
- FreeSWITCH integration ready
- Docker deployment support
- Comprehensive documentation
- Multi-environment configuration"

git push origin v1.0.0
print_success "Tag v1.0.0 creado y subido."

# Limpiar token del remote URL por seguridad
git remote set-url origin https://github.com/jesus-bazan-entel/ApoloBilling.git

# Paso 13: Verificar subida
print_status "Verificando subida a GitHub..."
sleep 3

echo ""
echo -e "${GREEN}=================================================="
echo "🎉 ¡SUBIDA A GITHUB COMPLETADA EXITOSAMENTE!"
echo "==================================================${NC}"
echo ""
echo "📋 Resumen de la operación:"
echo "✅ Repositorio inicializado"
echo "✅ Archivos agregados y commit realizados"
echo "✅ Código subido a GitHub"
echo "✅ Tag v1.0.0 creado"
echo ""
echo "🔗 Enlaces útiles:"
echo "   📁 Repositorio: https://github.com/jesus-bazan-entel/ApoloBilling"
echo "   📊 Commits: https://github.com/jesus-bazan-entel/ApoloBilling/commits/main"
echo "   🏷️  Releases: https://github.com/jesus-bazan-entel/ApoloBilling/releases"
echo ""
echo "📖 Próximos pasos:"
echo "   1. Revisar el repositorio en GitHub"
echo "   2. Seguir la guía de despliegue en GITHUB_DEPLOYMENT_GUIDE.md"
echo "   3. Configurar tu servidor de producción"
echo "   4. Ejecutar el despliegue con Docker o manual"
echo ""
echo "🔧 Para actualizaciones futuras, usa:"
echo "   git add ."
echo "   git commit -m 'Descripción del cambio'"
echo "   git push origin main"
echo ""
print_success "¡Listo para producción! 🚀"