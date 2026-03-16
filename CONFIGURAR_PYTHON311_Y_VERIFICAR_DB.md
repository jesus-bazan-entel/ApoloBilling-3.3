# 🐍 CONFIGURAR PYTHON 3.11 Y VERIFICAR COMUNICACIÓN CON BASE DE DATOS

**Fecha:** 2025-12-23  
**Sistema:** Ubuntu/Debian  
**Objetivo:** Configurar Python 3.11 como predeterminado y verificar comunicación con `apolo_billing`

---

## 📋 PARTE 1: CONFIGURAR PYTHON 3.11 COMO PREDETERMINADO

### **MÉTODO 1: update-alternatives (Sistema completo)**

```bash
# PASO 1: Verificar versiones instaladas
ls -la /usr/bin/python3*
```

**Salida esperada:**
```
lrwxrwxrwx 1 root root 10 ... /usr/bin/python3 -> python3.13
-rwxr-xr-x 1 root root ... /usr/bin/python3.11
-rwxr-xr-x 1 root root ... /usr/bin/python3.13
```

---

```bash
# PASO 2: Registrar python3.11 con prioridad 1
sudo update-alternatives --install /usr/bin/python3 python3 /usr/bin/python3.11 1
```

**Salida esperada:**
```
update-alternatives: using /usr/bin/python3.11 to provide /usr/bin/python3 (python3) in auto mode
```

---

```bash
# PASO 3: Registrar python3.13 con prioridad 2
sudo update-alternatives --install /usr/bin/python3 python3 /usr/bin/python3.13 2
```

**Salida esperada:**
```
update-alternatives: using /usr/bin/python3.13 to provide /usr/bin/python3 (python3) in auto mode
```

---

```bash
# PASO 4: Seleccionar python3.11 como predeterminado
sudo update-alternatives --config python3
```

**Menú interactivo:**
```
There are 2 choices for the alternative python3 (providing /usr/bin/python3).

  Selection    Path                 Priority   Status
------------------------------------------------------------
  0            /usr/bin/python3.13   2         auto mode
  1            /usr/bin/python3.11   1         manual mode
* 2            /usr/bin/python3.13   2         manual mode

Press <enter> to keep the current choice[*], or type selection number:
```

**👉 ESCRIBE:** `1`  
**👉 PRESIONA:** `ENTER`

---

```bash
# PASO 5: Verificar el cambio
python3 --version
which python3
ls -la /etc/alternatives/python3
```

**Salida esperada:**
```
Python 3.11.7
/usr/bin/python3
lrwxrwxrwx 1 root root 18 ... /etc/alternatives/python3 -> /usr/bin/python3.11
```

✅ **Python 3.11 está ahora configurado como predeterminado**

---

## 📋 PARTE 2: RESOLVER CONFLICTO DE GIT Y ACTUALIZAR CÓDIGO

```bash
# PASO 1: Ir al directorio del proyecto
cd /home/jbazan/ApoloBilling

# PASO 2: Ver cambios locales (opcional)
git diff backend/.env

# PASO 3: Sobrescribir con la versión del repositorio
git checkout origin/genspark_ai_developer -- backend/.env

# PASO 4: Hacer pull limpio
git pull origin genspark_ai_developer

# PASO 5: Verificar contenido del .env
cat backend/.env
```

**Contenido esperado de `backend/.env`:**
```env
PROJECT_NAME="Apolo Billing"
API_V1_STR="/api"
DATABASE_URL="postgresql://apolo_user:apolo_password_2024@127.0.0.1:5432/apolo_billing"
SECRET_KEY="secreto-super-importante"
SUPERADMIN_PASSWORD="ApoloNext$Sam$"
```

✅ **Código actualizado correctamente**

---

## 📋 PARTE 3: VERIFICAR COMUNICACIÓN CON BASE DE DATOS

### **A. Test Backend Python**

```bash
cd /home/jbazan/ApoloBilling

python3 << 'PYEOF'
import psycopg2
print("🔧 Probando conexión del Backend Python a PostgreSQL...")
print()

DATABASE_URL = "postgresql://apolo_user:apolo_password_2024@127.0.0.1:5432/apolo_billing"

try:
    # Conectar
    conn = psycopg2.connect(DATABASE_URL)
    print("✅ Conexión exitosa a PostgreSQL")
    
    # Verificar base de datos y usuario
    cur = conn.cursor()
    cur.execute("SELECT current_database(), current_user;")
    db, user = cur.fetchone()
    print(f"📊 Base de datos: {db}")
    print(f"👤 Usuario: {user}")
    print()
    
    # Contar registros
    cur.execute("SELECT COUNT(*) FROM accounts;")
    count = cur.fetchone()[0]
    print(f"📋 Cuentas en tabla 'accounts': {count}")
    
    cur.execute("SELECT COUNT(*) FROM rate_cards;")
    count = cur.fetchone()[0]
    print(f"💰 Tarjetas en tabla 'rate_cards': {count}")
    
    cur.execute("SELECT COUNT(*) FROM cdrs;")
    count = cur.fetchone()[0]
    print(f"📞 CDRs en tabla 'cdrs': {count}")
    print()
    
    # Verificar cuenta de prueba
    cur.execute("SELECT account_number, balance, status FROM accounts WHERE account_number = '100001';")
    result = cur.fetchone()
    if result:
        acc, bal, status = result
        print(f"✅ Cuenta de prueba encontrada:")
        print(f"   Account: {acc}")
        print(f"   Balance: ${bal}")
        print(f"   Status: {status}")
    else:
        print("⚠️  Cuenta 100001 no encontrada")
    
    conn.close()
    print()
    print("╔═══════════════════════════════════════════════════════════╗")
    print("║  ✅ BACKEND PYTHON COMUNICA CORRECTAMENTE CON apolo_billing ║")
    print("╚═══════════════════════════════════════════════════════════╝")

except Exception as e:
    print(f"❌ Error: {e}")
    print()
    print("╔═══════════════════════════════════════════════════════════╗")
    print("║  ❌ BACKEND PYTHON NO PUEDE CONECTAR A apolo_billing      ║")
    print("╚═══════════════════════════════════════════════════════════╝")
PYEOF
```

**Salida esperada:**
```
🔧 Probando conexión del Backend Python a PostgreSQL...

✅ Conexión exitosa a PostgreSQL
📊 Base de datos: apolo_billing
👤 Usuario: apolo_user

📋 Cuentas en tabla 'accounts': 1
💰 Tarjetas en tabla 'rate_cards': 11
📞 CDRs en tabla 'cdrs': 5

✅ Cuenta de prueba encontrada:
   Account: 100001
   Balance: $9.991
   Status: ACTIVE

╔═══════════════════════════════════════════════════════════╗
║  ✅ BACKEND PYTHON COMUNICA CORRECTAMENTE CON apolo_billing ║
╚═══════════════════════════════════════════════════════════╝
```

---

### **B. Test Motor Rust**

```bash
cd /home/jbazan/ApoloBilling/rust-billing-engine
RUST_LOG=info cargo run
```

**Salida esperada (primeras líneas):**
```
🚀 Starting Apolo Billing Engine (Rust) - v2.0.5
✅ Database connection test successful
Database pool created
Connected to database: apolo_billing as user: apolo_user
✅ Rate card loaded: Perú - Nacional ($0.015/min, 6 sec increment, priority 100)
✅ Rate card loaded: Perú Móvil ($0.018/min, 6 sec increment, priority 150)
...
🎧 ESL Server listening on 0.0.0.0:8021
```

✅ **Motor Rust conectado correctamente a `apolo_billing`**

---

## 📊 RESUMEN FINAL

| Componente | Base de Datos | Usuario | Python | Estado |
|------------|---------------|---------|--------|--------|
| **Backend Python** | `apolo_billing` | `apolo_user` | 3.11.7 | ✅ CORRECTO |
| **Motor Rust** | `apolo_billing` | `apolo_user` | N/A | ✅ CORRECTO |

---

## 🎯 PRÓXIMOS PASOS

Una vez verificada la comunicación:

```bash
# Terminal 1: Iniciar motor Rust
cd /home/jbazan/ApoloBilling/rust-billing-engine
RUST_LOG=info cargo run

# Terminal 2: Ejecutar simulador ESL
cd /home/jbazan/ApoloBilling
./tools/esl_simulator.py --duration 30
```

---

## 🔗 ENLACES IMPORTANTES

- **Repository:** https://github.com/jesus-bazan-entel/ApoloBilling
- **Pull Request:** https://github.com/jesus-bazan-entel/ApoloBilling/pull/1
- **Latest Commit:** https://github.com/jesus-bazan-entel/ApoloBilling/commit/fc096359

---

## 📝 NOTAS

- ✅ Python 3.11 configurado con `update-alternatives`
- ✅ Backend `.env` corregido para usar `apolo_billing`
- ✅ Ambos componentes apuntan a la misma base de datos
- ✅ Credenciales unificadas: `apolo_user:apolo_password_2024`

