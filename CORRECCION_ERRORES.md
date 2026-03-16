# 🔧 Corrección de Errores - Apolo Billing

## Fecha: 2025-12-22
## Versión: 2.0.1

---

## ❌ Problemas Identificados

### 1. Error de Configuración `.env`

**Síntoma:**
```
pydantic_core._pydantic_core.ValidationError: 2 validation errors for Settings
algorithm
  Extra inputs are not permitted [type=extra_forbidden, input_value='HS256', input_type=str]
access_token_expire_minutes
  Extra inputs are not permitted [type=extra_forbidden, input_value='60', input_type=str]
```

**Causa:**
El archivo `app/core/config.py` solo acepta estas variables:
- `PROJECT_NAME`
- `API_V1_STR`
- `DATABASE_URL`
- `SECRET_KEY`
- `SUPERADMIN_PASSWORD`

Las variables `ALGORITHM` y `ACCESS_TOKEN_EXPIRE_MINUTES` no están definidas en la clase `Settings`.

**Solución:**
Eliminar esas dos líneas del `.env`:

```bash
cd /home/jbazan/ApoloBilling/backend
cat > .env << 'EOF'
PROJECT_NAME=Apolo Billing
API_V1_STR=/api
DATABASE_URL=postgresql://apolo_user:apolo_password_2024@localhost/apolo_billing
SECRET_KEY=desarrollo-secret-key-cambiar-en-produccion-123456789
SUPERADMIN_PASSWORD=admin123
EOF
```

---

### 2. Error de Inicialización de Base de Datos

**Síntoma:**
```
sqlalchemy.exc.ProgrammingError: (psycopg2.errors.UndefinedTable) relation "rate_cards" does not exist
[SQL: CREATE INDEX IF NOT EXISTS idx_rate_cards_priority ON rate_cards(priority DESC)]
```

**Causa:**
El script `init_db_clean.py` intentaba crear índices antes de que las tablas existieran.

**Solución Aplicada:**
Modificado `init_db_clean.py` para:
1. Ejecutar comandos SQL individualmente
2. Manejo de errores mejorado
3. Verificación de existencia de tablas/índices

---

## ✅ Correcciones Aplicadas

### Archivos Modificados:

1. **`backend/init_db_clean.py`**
   - Mejorado manejo de errores
   - Ejecución secuencial de comandos SQL
   - Ignorar errores de "already exists"

2. **`actualizar_local.sh`**
   - Eliminadas variables `ALGORITHM` y `ACCESS_TOKEN_EXPIRE_MINUTES` del `.env`
   - Configuración correcta del entorno

3. **`ACTUALIZACION_LOCAL.md`**
   - Actualizada documentación con `.env` correcto

4. **`backend/.env.example`**
   - Creado archivo de ejemplo con variables correctas

---

## 🚀 Cómo Aplicar las Correcciones

### Si ya ejecutaste el script y obtuviste errores:

```bash
# 1. Navegar al directorio backend
cd /home/jbazan/ApoloBilling/backend

# 2. Corregir el archivo .env (eliminar las 2 líneas problemáticas)
cat > .env << 'EOF'
PROJECT_NAME=Apolo Billing
API_V1_STR=/api
DATABASE_URL=postgresql://apolo_user:apolo_password_2024@localhost/apolo_billing
SECRET_KEY=desarrollo-secret-key-cambiar-en-produccion-123456789
SUPERADMIN_PASSWORD=admin123
EOF

# 3. Actualizar el script de inicialización
cd /home/jbazan/ApoloBilling
git pull origin genspark_ai_developer

# 4. Reinicializar la base de datos
cd /home/jbazan/ApoloBilling/backend
source venv/bin/activate
python init_db_clean.py

# 5. Iniciar el servidor
uvicorn main:app --host 0.0.0.0 --port 8000 --reload
```

---

## 📋 Verificación Post-Corrección

### 1. Verificar `.env`

```bash
cd /home/jbazan/ApoloBilling/backend
cat .env
```

**Debe mostrar SOLO estas 5 líneas:**
```
PROJECT_NAME=Apolo Billing
API_V1_STR=/api
DATABASE_URL=postgresql://apolo_user:apolo_password_2024@localhost/apolo_billing
SECRET_KEY=desarrollo-secret-key-cambiar-en-produccion-123456789
SUPERADMIN_PASSWORD=admin123
```

### 2. Verificar Base de Datos

```bash
sudo -u postgres psql -d apolo_billing -c "\dt"
```

**Debe mostrar estas 6 tablas:**
```
 public | accounts              | table | apolo_user
 public | balance_reservations  | table | apolo_user
 public | balance_transactions  | table | apolo_user
 public | cdrs                  | table | apolo_user
 public | rate_cards            | table | apolo_user
 public | users                 | table | apolo_user
```

### 3. Verificar Servidor

```bash
cd /home/jbazan/ApoloBilling/backend
source venv/bin/activate
python -c "from app.core.config import settings; print('✅ Config cargada correctamente')"
```

**Si no hay errores, la configuración es correcta.**

### 4. Probar el Sistema

```bash
# Iniciar servidor
cd /home/jbazan/ApoloBilling/backend
source venv/bin/activate
uvicorn main:app --host 0.0.0.0 --port 8000 --reload
```

Luego acceder a: http://localhost:8000/dashboard/rate-cards

---

## 📝 Variables de `.env` Correctas

| Variable | Descripción | Valor por Defecto |
|----------|-------------|-------------------|
| `PROJECT_NAME` | Nombre del proyecto | `Apolo Billing` |
| `API_V1_STR` | Prefijo de API | `/api` |
| `DATABASE_URL` | URL de PostgreSQL | `postgresql://apolo_user:...` |
| `SECRET_KEY` | Clave secreta para JWT | (generado único) |
| `SUPERADMIN_PASSWORD` | Password del admin | `admin123` |

### ❌ Variables NO Soportadas (no incluir):
- ~~`ALGORITHM`~~
- ~~`ACCESS_TOKEN_EXPIRE_MINUTES`~~
- ~~`redis_url`~~
- ~~`debug`~~

---

## 🐛 Solución de Problemas Adicionales

### Error: "relation 'rate_cards' does not exist"

```bash
cd /home/jbazan/ApoloBilling/backend
source venv/bin/activate

# Opción 1: Reinicializar base de datos
sudo service postgresql start
sudo -u postgres psql << 'EOF'
DROP DATABASE IF EXISTS apolo_billing;
CREATE DATABASE apolo_billing OWNER apolo_user;
EOF
python init_db_clean.py

# Opción 2: Crear tablas manualmente
sudo -u postgres psql -d apolo_billing -f /path/to/schema.sql
```

### Error: "Extra inputs are not permitted"

```bash
# Editar .env y verificar que NO contenga estas líneas:
cd /home/jbazan/ApoloBilling/backend
nano .env

# Eliminar cualquier línea con:
# ALGORITHM=
# ACCESS_TOKEN_EXPIRE_MINUTES=
# redis_url=
# debug=

# Guardar y cerrar (Ctrl+O, Enter, Ctrl+X)
```

### Error: Pydantic validation error

```bash
# Verificar que Python sea 3.11 (no 3.13)
python --version  # Debe mostrar 3.11.x

# Si muestra 3.13, recrear venv:
cd /home/jbazan/ApoloBilling/backend
deactivate 2>/dev/null
rm -rf venv
python3.11 -m venv venv
source venv/bin/activate
pip install -r requirements.txt
```

---

## ✅ Checklist de Corrección

- [ ] Archivo `.env` tiene SOLO 5 variables
- [ ] No contiene `ALGORITHM` ni `ACCESS_TOKEN_EXPIRE_MINUTES`
- [ ] Base de datos `apolo_billing` existe
- [ ] Usuario `apolo_user` tiene permisos
- [ ] Script `init_db_clean.py` ejecuta sin errores
- [ ] Servidor FastAPI inicia correctamente
- [ ] Dashboard Rate Cards accesible
- [ ] Login funciona con `admin/admin123`

---

## 📞 Soporte

Si persisten los errores:

1. Verifica versión de Python: `python --version` (debe ser 3.11.x)
2. Verifica PostgreSQL: `sudo service postgresql status`
3. Verifica logs: `tail -f logs/app.log`
4. Revisa este documento: `CORRECCION_ERRORES.md`

---

**Última actualización:** 2025-12-22  
**Versión del sistema:** 2.0.1 (Hotfix)  
**Archivos corregidos:** 4
