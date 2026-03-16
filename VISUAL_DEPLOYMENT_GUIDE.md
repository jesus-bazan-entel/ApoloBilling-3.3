# 📱 Guía Visual de Despliegue - Windows con WSL

## 🎯 Objetivo
Desplegar Apolo Billing en tu máquina Windows usando WSL (Debian).

---

## 📋 Antes de Empezar

### ✅ Requisitos Previos
- [ ] Windows 10/11
- [ ] WSL2 instalado con Debian
- [ ] Al menos 4GB RAM disponible
- [ ] 10GB espacio en disco

---

## 🚀 Opción 1: Instalación Automática (Recomendada)

### Paso 1: Abrir WSL Debian

**Desde Windows:**

1. Presiona `Windows + R`
2. Escribe: `wsl`
3. Presiona Enter

O simplemente busca "Debian" en el menú Inicio.

```
┌─────────────────────────────────────────┐
│  🪟 Windows                             │
│    ↓ WSL ejecutándose                  │
│  🐧 Debian (Terminal)                  │
└─────────────────────────────────────────┘
```

---

### Paso 2: Clonar el Repositorio

En la terminal de WSL, ejecuta:

```bash
cd ~
git clone https://github.com/jesus-bazan-entel/ApoloBilling.git
cd ApoloBilling
git checkout genspark_ai_developer
```

**Salida esperada:**
```
Cloning into 'ApoloBilling'...
remote: Enumerating objects: 1234, done.
✅ Branch 'genspark_ai_developer' set up to track remote branch
```

---

### Paso 3: Ejecutar Script Automático

```bash
./quick_start_wsl.sh
```

**El script hará:**

```
╔══════════════════════════════════════════════════════════════╗
║      🚀 Apolo Billing - Quick Deployment Script             ║
║          WSL Debian Environment Setup                        ║
╚══════════════════════════════════════════════════════════════╝

✅ Running in WSL environment

📋 Step 1/8: Checking system requirements...
✅ Python 3.11.2 installed
✅ PostgreSQL installed
✅ Redis installed

📦 Step 2/8: Starting services...
✅ PostgreSQL started
✅ Redis started
✅ Redis is responding

🗄️  Step 3/8: Configuring database...
✅ Database 'apolo_billing' created

🐍 Step 4/8: Setting up Python backend...
✅ Virtual environment created
✅ Python dependencies installed

⚙️  Step 5/8: Configuring environment variables...
✅ Backend .env file created

🔧 Step 6/8: Initializing database schema...
✅ Database initialized

👤 Step 7/8: Creating admin user...
✅ Admin user created
   Username: admin
   Password: admin123

🚀 Step 8/8: Starting backend server...
━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━
🎉 Apolo Billing is ready!

📊 Access Points:
   Dashboard:       http://localhost:8000
   Login:           http://localhost:8000/login
   Rate Cards UI:   http://localhost:8000/dashboard/rate-cards
   API Docs:        http://localhost:8000/docs

🔐 Default Credentials:
   Username: admin
   Password: admin123
━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━━

INFO:     Started server process
INFO:     Waiting for application startup.
INFO:     Application startup complete.
INFO:     Uvicorn running on http://0.0.0.0:8000
```

---

### Paso 4: Abrir el Dashboard

**En tu navegador Windows (Chrome, Edge, Firefox):**

1. Abre una nueva pestaña
2. Ve a: `http://localhost:8000`
3. Verás la pantalla de login

```
┌──────────────────────────────────────────────┐
│                                              │
│          🚀 Apolo Billing System            │
│                                              │
│   ┌────────────────────────────────────┐   │
│   │  Username: [admin____________]     │   │
│   │  Password: [●●●●●●●●●_______]     │   │
│   │                                    │   │
│   │         [ Login ]                  │   │
│   └────────────────────────────────────┘   │
│                                              │
└──────────────────────────────────────────────┘
```

**Credenciales:**
- Username: `admin`
- Password: `admin123`

---

### Paso 5: Acceder a Rate Cards

Después del login, en el menú lateral:

```
┌─ Apolo Billing ─────────────────────┐
│                                      │
│ 📊 Gestión Comercial                │
│   └─ Gestión de Abonados            │
│                                      │
│ 📡 Ingeniería de Tráfico            │
│   ├─ Tráfico & CDRs                 │
│   └─ Rutas & Tarifas ▼              │
│       ├─ ⭐ Rate Cards (Nuevo) ✓    │ ← Clic aquí
│       ├─ Zonas (Legacy)              │
│       ├─ Prefijos (Legacy)           │
│       └─ Tarifas (Legacy)            │
│                                      │
│ 💰 Facturación                       │
│   └─ Facturación & Cobros           │
│                                      │
└──────────────────────────────────────┘
```

---

## 🎨 Interfaz de Rate Cards

### Vista Principal

```
╔═══════════════════════════════════════════════════════════════╗
║  🎯 Rate Cards Management                                     ║
╚═══════════════════════════════════════════════════════════════╝

┌─── Estadísticas ─────────────────────────────────────────────┐
│  Total Cards: 10  │  Avg Rate: $0.08/min  │  Min: $0.08/min │
└──────────────────────────────────────────────────────────────┘

┌─── Búsqueda LPM ─────────────────────────────────────────────┐
│  🔍 Buscar por destino: [519839876543______] [Buscar]       │
└──────────────────────────────────────────────────────────────┘

┌─── Tabla de Rate Cards ──────────────────────────────────────┐
│ Actions │ Prefix │ Name         │ Rate/min │ Incr │ Priority│
├─────────┼────────┼──────────────┼──────────┼──────┼─────────┤
│ [✏️][🗑️]│ 51983  │ Perú Móvil   │ $0.0850  │  6s  │   150   │
│ [✏️][🗑️]│ 51982  │ Perú Móvil   │ $0.0850  │  6s  │   150   │
│ [✏️][🗑️]│ 51999  │ Perú Movistar│ $0.0800  │  6s  │   150   │
└──────────────────────────────────────────────────────────────┘

[➕ Nueva Rate Card]  [📤 Importar CSV]  [📥 Exportar CSV]
```

---

## 🔧 Operaciones Comunes

### ➕ Crear Nueva Rate Card

1. Clic en **"➕ Nueva Rate Card"**
2. Llenar formulario:
   ```
   Prefijo de Destino: 51980
   Nombre de Destino:  Perú Móvil Claro
   Tarifa por Minuto:  0.0850
   Incremento Factur:  6
   Cargo de Conexión:  0.0000
   Prioridad:          150
   ```
3. Clic en **"Crear"**
4. ✅ Toast: "Rate Card creada exitosamente"

---

### ✏️ Editar Rate Card

1. Clic en botón **✏️** de la fila
2. Modal se abre con datos pre-cargados
3. Modificar campos necesarios
4. Clic en **"Guardar Cambios"**
5. ✅ Toast: "Rate Card actualizada exitosamente"

---

### 🗑️ Eliminar Rate Card

1. Clic en botón **🗑️** de la fila
2. Diálogo de confirmación:
   ```
   ⚠️  ¿Está seguro de eliminar esta Rate Card?
       Esta acción no se puede deshacer.
   
       [Cancelar]  [Eliminar]
   ```
3. Clic en **"Eliminar"**
4. ✅ Toast: "Rate Card eliminada exitosamente"

---

### 🔍 Buscar Rate Card (LPM)

1. Ingresar número: `519839876543`
2. Clic en **"Buscar"**
3. Resultado:
   ```
   ✅ Rate Card Encontrada
   
   Prefijo:          51983
   Destino:          Perú Móvil Claro
   Tarifa/min:       $0.0850
   Tarifa/seg:       $0.001417
   Incremento:       6s
   Cargo Conexión:   $0.0000
   Prioridad:        150
   Match Length:     5 dígitos
   ```

---

### 📤 Importar CSV Masivo

1. Clic en **"📤 Importar CSV"**
2. Preparar CSV con este formato:
   ```csv
   destination_prefix,destination_name,rate_per_minute,billing_increment,connection_fee,priority
   51980,Perú Móvil Claro,0.0850,6,0.0000,150
   51981,Perú Móvil Claro,0.0850,6,0.0000,150
   51982,Perú Móvil Claro,0.0850,6,0.0000,150
   ```
3. Seleccionar archivo
4. Clic en **"Importar"**
5. ✅ Toast: "Importación exitosa: 3 registros importados, 0 omitidos"

---

### 📥 Exportar a CSV

1. Clic en **"📥 Exportar CSV"**
2. Archivo descarga automáticamente: `rate_cards_2024-12-22.csv`
3. Abrir en Excel para revisar

---

## 🛑 Detener el Sistema

### Método 1: Desde WSL Terminal

En la terminal donde corre el servidor:
```
Ctrl + C
```

### Método 2: Desde PowerShell (Windows)

1. Abrir PowerShell
2. Ejecutar:
   ```powershell
   wsl pkill -f "uvicorn main:app"
   ```

---

## 🔄 Reiniciar el Sistema

### En WSL Terminal:

```bash
cd ~/ApoloBilling/backend
source venv/bin/activate
uvicorn main:app --host 0.0.0.0 --port 8000 --reload
```

O simplemente:
```bash
cd ~/ApoloBilling
./quick_start_wsl.sh
```

---

## 📊 Verificar Estado de Servicios

```bash
# PostgreSQL
sudo service postgresql status

# Redis
redis-cli ping

# Backend (verificar si está corriendo)
curl http://localhost:8000/docs
```

---

## 🐛 Problemas Comunes y Soluciones

### ❌ Error: "Port 8000 already in use"

**Solución:**
```bash
# Encontrar proceso
sudo lsof -i :8000

# Matar proceso (reemplazar <PID> con el número mostrado)
sudo kill -9 <PID>

# Reiniciar
uvicorn main:app --host 0.0.0.0 --port 8000 --reload
```

---

### ❌ Error: "Connection to PostgreSQL refused"

**Solución:**
```bash
# Iniciar PostgreSQL
sudo service postgresql start

# Verificar
sudo service postgresql status
```

---

### ❌ Error: "Redis connection refused"

**Solución:**
```bash
# Iniciar Redis
sudo service redis-server start

# Verificar
redis-cli ping
# Debe responder: PONG
```

---

### ❌ Error: "Module not found" al iniciar backend

**Solución:**
```bash
cd backend
source venv/bin/activate
pip install -r requirements.txt
```

---

## 🎯 Flujo de Trabajo Completo

```
┌─────────────────────────────────────────────────────────────┐
│  1. Abrir WSL Debian                                        │
│     └─ Windows + R → "wsl" → Enter                         │
│                                                             │
│  2. Navegar al proyecto                                     │
│     └─ cd ~/ApoloBilling                                    │
│                                                             │
│  3. Iniciar servicios                                       │
│     └─ ./quick_start_wsl.sh                                │
│                                                             │
│  4. Abrir navegador Windows                                 │
│     └─ http://localhost:8000                               │
│                                                             │
│  5. Login                                                   │
│     └─ Username: admin, Password: admin123                 │
│                                                             │
│  6. Ir a Rate Cards                                         │
│     └─ Menú: Rutas & Tarifas → Rate Cards (Nuevo)         │
│                                                             │
│  7. Trabajar con Rate Cards                                 │
│     └─ Crear, Editar, Eliminar, Buscar, Importar          │
│                                                             │
│  8. Detener (cuando termines)                              │
│     └─ Ctrl + C en terminal WSL                            │
└─────────────────────────────────────────────────────────────┘
```

---

## 📚 Documentación Adicional

- 📄 **QUICKSTART.md** - Inicio rápido (este archivo)
- 📄 **DEPLOYMENT_GUIDE_WSL.md** - Guía detallada de despliegue
- 📄 **UI_MIGRATION_COMPLETED.md** - Documentación técnica de la UI
- 📄 **MIGRATION_PLAN_RATE_CARDS.md** - Plan de migración completo

---

## ✅ Checklist de Verificación

Después de desplegar, verifica:

- [ ] PostgreSQL está corriendo
- [ ] Redis está corriendo
- [ ] Backend responde en http://localhost:8000
- [ ] Login funciona con admin/admin123
- [ ] Dashboard carga correctamente
- [ ] Rate Cards UI es accesible
- [ ] Puedes crear una nueva Rate Card
- [ ] Búsqueda LPM funciona
- [ ] Importar/Exportar CSV funciona

---

## 🎉 ¡Listo!

Tu sistema Apolo Billing está funcionando en WSL y listo para usar desde Windows.

**Próximos pasos sugeridos:**
1. Cambiar password del admin
2. Importar tus rate cards existentes
3. Crear usuarios adicionales
4. Explorar todas las funcionalidades

---

**¿Necesitas ayuda?**
- Consulta: `DEPLOYMENT_GUIDE_WSL.md` para detalles técnicos
- Revisa logs: `~/ApoloBilling/backend/logs/`
- GitHub: https://github.com/jesus-bazan-entel/ApoloBilling

---

**✨ ¡Disfruta usando Apolo Billing!**
