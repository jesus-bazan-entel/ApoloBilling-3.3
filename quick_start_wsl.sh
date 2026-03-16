#!/bin/bash

# 🚀 Apolo Billing - Quick Start Script for WSL Debian
# This script automates the deployment process

set -e  # Exit on error

echo "╔══════════════════════════════════════════════════════════════╗"
echo "║      🚀 Apolo Billing - Quick Deployment Script             ║"
echo "║          WSL Debian Environment Setup                        ║"
echo "╚══════════════════════════════════════════════════════════════╝"
echo ""

# Colors
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

# Functions
print_success() {
    echo -e "${GREEN}✅ $1${NC}"
}

print_error() {
    echo -e "${RED}❌ $1${NC}"
}

print_info() {
    echo -e "${YELLOW}ℹ️  $1${NC}"
}

# Check if running in WSL
if ! grep -qEi "(Microsoft|WSL)" /proc/version &> /dev/null ; then
    print_error "This script must be run in WSL (Windows Subsystem for Linux)"
    exit 1
fi

print_success "Running in WSL environment"
echo ""

# Step 1: Check System Requirements
echo "📋 Step 1/8: Checking system requirements..."

# Check Python
if command -v python3 &> /dev/null; then
    PYTHON_VERSION=$(python3 --version | awk '{print $2}')
    print_success "Python $PYTHON_VERSION installed"
else
    print_error "Python 3 not found. Installing..."
    sudo apt update
    sudo apt install -y python3 python3-pip python3-venv
fi

# Check PostgreSQL
if command -v psql &> /dev/null; then
    print_success "PostgreSQL installed"
else
    print_info "PostgreSQL not found. Installing..."
    sudo apt install -y postgresql postgresql-contrib
    sudo service postgresql start
    print_success "PostgreSQL installed and started"
fi

# Check Redis
if command -v redis-cli &> /dev/null; then
    print_success "Redis installed"
else
    print_info "Redis not found. Installing..."
    sudo apt install -y redis-server
    sudo service redis-server start
    print_success "Redis installed and started"
fi

echo ""

# Step 2: Start Services
echo "📦 Step 2/8: Starting services..."

sudo service postgresql start
print_success "PostgreSQL started"

sudo service redis-server start
print_success "Redis started"

# Verify Redis
if redis-cli ping &> /dev/null; then
    print_success "Redis is responding"
else
    print_error "Redis is not responding"
    exit 1
fi

echo ""

# Step 3: Configure Database
echo "🗄️  Step 3/8: Configuring database..."

DB_EXISTS=$(sudo -u postgres psql -lqt | cut -d \| -f 1 | grep -w apolo_billing | wc -l)

if [ $DB_EXISTS -eq 0 ]; then
    print_info "Creating database and user..."
    sudo -u postgres psql << EOF
CREATE USER apolo_user WITH PASSWORD 'apolo_password_2024';
CREATE DATABASE apolo_billing OWNER apolo_user;
GRANT ALL PRIVILEGES ON DATABASE apolo_billing TO apolo_user;
\q
EOF
    print_success "Database 'apolo_billing' created"
else
    print_success "Database 'apolo_billing' already exists"
fi

echo ""

# Step 4: Setup Backend Environment
echo "🐍 Step 4/8: Setting up Python backend..."

cd backend

# Create virtual environment
if [ ! -d "venv" ]; then
    print_info "Creating Python virtual environment..."
    python3 -m venv venv
    print_success "Virtual environment created"
else
    print_success "Virtual environment already exists"
fi

# Activate virtual environment
source venv/bin/activate

# Upgrade pip
pip install --upgrade pip --quiet

# Install dependencies
if [ -f "requirements.txt" ]; then
    print_info "Installing Python dependencies..."
    pip install -r requirements.txt --quiet
    print_success "Python dependencies installed"
else
    print_error "requirements.txt not found"
    exit 1
fi

echo ""

# Step 5: Configure Environment Variables
echo "⚙️  Step 5/8: Configuring environment variables..."

if [ ! -f ".env" ]; then
    cat > .env << 'EOF'
DATABASE_URL=postgresql://apolo_user:apolo_password_2024@localhost/apolo_billing
REDIS_URL=redis://localhost:6379/0
SECRET_KEY=apolo-secret-key-change-in-production-2024
DEBUG=True
ALLOWED_HOSTS=localhost,127.0.0.1
JWT_SECRET_KEY=apolo-jwt-secret-key-2024
JWT_ALGORITHM=HS256
ACCESS_TOKEN_EXPIRE_MINUTES=30
CORS_ORIGINS=http://localhost:8000,http://127.0.0.1:8000
LOG_LEVEL=INFO
EOF
    print_success "Backend .env file created"
else
    print_success "Backend .env file already exists"
fi

echo ""

# Step 6: Initialize Database
echo "🔧 Step 6/8: Initializing database schema..."

if [ -f "init_database.py" ]; then
    python init_database.py
    print_success "Database initialized"
else
    print_info "init_database.py not found, skipping..."
fi

echo ""

# Step 7: Create Admin User (if needed)
echo "👤 Step 7/8: Creating admin user..."

python << 'PYEOF'
try:
    from app.db.database import SessionLocal
    from app.models.user import User
    from passlib.context import CryptContext
    
    pwd_context = CryptContext(schemes=["bcrypt"], deprecated="auto")
    db = SessionLocal()
    
    existing = db.query(User).filter(User.username == "admin").first()
    if not existing:
        admin = User(
            username="admin",
            email="admin@apolobilling.com",
            hashed_password=pwd_context.hash("admin123"),
            role="superadmin",
            is_active=True
        )
        db.add(admin)
        db.commit()
        print("✅ Admin user created")
        print("   Username: admin")
        print("   Password: admin123")
    else:
        print("✅ Admin user already exists")
    
    db.close()
except Exception as e:
    print(f"⚠️  Could not create admin user: {e}")
PYEOF

echo ""

# Step 8: Start Backend Server
echo "🚀 Step 8/8: Starting backend server..."

print_success "Backend setup complete!"
echo ""
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
echo ""
echo "🎉 Apolo Billing is ready to start!"
echo ""
echo "📊 Access Points:"
echo "   Dashboard:       http://localhost:8000"
echo "   Login:           http://localhost:8000/login"
echo "   Rate Cards UI:   http://localhost:8000/dashboard/rate-cards"
echo "   API Docs:        http://localhost:8000/docs"
echo ""
echo "🔐 Default Credentials:"
echo "   Username: admin"
echo "   Password: admin123"
echo "   ⚠️  Change password in production!"
echo ""
echo "━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━"
echo ""
echo "🚀 Starting server..."
echo ""

# Start the server
uvicorn main:app --host 0.0.0.0 --port 8000 --reload
