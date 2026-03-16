# 🔧 FIX: Frontend mostrando CDRs incorrectos

**Fecha:** 2025-12-23  
**Problema:** El frontend muestra datos dummy en lugar de los CDRs reales del motor Rust

---

## 🔍 PROBLEMA IDENTIFICADO

El backend FastAPI está mostrando datos **hardcodeados** (falsos) en lugar de consultar la tabla `cdrs` real de `apolo_billing`.

### **Archivos a Modificar:**

1. **`backend/app/models/cdr.py`** - Modelo con campos incorrectos
2. **`backend/app/web/views.py`** (línea 765) - Endpoint con datos dummy

---

## ✅ SOLUCIÓN PASO A PASO

### **1. Corregir Modelo CDR**

**Archivo:** `backend/app/models/cdr.py`

**Cambios a realizar:**

```python
# ANTES (INCORRECTO):
uuid = Column(String, unique=True, index=True, nullable=False)
caller = Column(String, nullable=False)
callee = Column(String, nullable=False)
rate_applied = Column(Numeric(10, 4), nullable=True)

# DESPUÉS (CORRECTO):
call_uuid = Column(String, unique=True, index=True, nullable=False)
caller_number = Column(String, nullable=False)
called_number = Column(String, nullable=False)
rate_id = Column(Integer, nullable=True)
```

**Status:** ✅ YA APLICADO AUTOMÁTICAMENTE

---

### **2. Reemplazar Endpoint `/dashboard/cdr`**

**Archivo:** `backend/app/web/views.py`  
**Línea:** 765

**Buscar esta función:**
```python
@router.get("/dashboard/cdr", response_class=HTMLResponse)
async def dashboard_cdr_view(request: Request, user: Usuario = Depends(get_current_active_user)):
    from datetime import datetime
    # Dummy stats for CDR
    stats = {
        "total_calls": 120,
        ...
```

**Reemplazar TODA la función (líneas 765-802) con:**

Ver archivo: `/tmp/fix_cdr_endpoint.py` para el código completo de reemplazo.

---

## 🚀 APLICAR FIX MANUALMENTE

En tu servidor ejecuta:

```bash
cd /home/jbazan/ApoloBilling

# 1. Hacer backup del archivo original
cp backend/app/web/views.py backend/app/web/views.py.backup

# 2. Editar el archivo
nano backend/app/web/views.py

# 3. Buscar la línea 765:
#    @router.get("/dashboard/cdr", response_class=HTMLResponse)

# 4. ELIMINAR las líneas 765-802 (toda la función dashboard_cdr_view)

# 5. PEGAR el contenido de /tmp/fix_cdr_endpoint.py (ver arriba)

# 6. Guardar: Ctrl+O, Enter, Ctrl+X

# 7. Reiniciar backend
cd /home/jbazan/ApoloBilling/backend
source venv/bin/activate
pkill -f "uvicorn"  # Detener proceso viejo
uvicorn app.main:app --host 0.0.0.0 --port 8000 --reload &
```

---

## 🧪 VERIFICAR FIX

```bash
# 1. Verificar que el backend inició correctamente
curl http://localhost:8000/docs

# 2. Acceder al dashboard CDR en el navegador
# http://TU_IP:8000/dashboard/cdr

# 3. Deberías ver los CDRs reales:
#    - Origen: 100001
#    - Destino: 51987654321
#    - Duración: 30s
#    - Costo: ~$0.009
#    - Fecha: 2025-12-23
```

---

## 📊 RESULTADO ESPERADO

### **ANTES (Datos Falsos):**
- Costo: $0.5000
- Fecha: 1970-01-01
- Números random: 1001, 987654321

### **DESPUÉS (Datos Reales):**
- Costo: $0.009 (o el calculado real)
- Fecha: 2025-12-23 14:11:05
- Números reales: 100001 → 51987654321

---

## 📝 ALTERNATIVA: Script Automático

Si prefieres, puedo crear un script Python que haga el reemplazo automático:

```bash
cd /home/jbazan/ApoloBilling
python3 << 'PYEOF'
import re

# Leer archivo
with open('backend/app/web/views.py', 'r') as f:
    content = f.read()

# Encontrar y reemplazar la función dashboard_cdr_view
# (Patrón regex complejo - ver código completo)

# Guardar
with open('backend/app/web/views.py', 'w') as f:
    f.write(content)

print("✅ Endpoint /dashboard/cdr actualizado")
PYEOF
```

---

## 🔗 ARCHIVOS INVOLUCRADOS

- `backend/app/models/cdr.py` ✅ CORREGIDO
- `backend/app/web/views.py` ⏳ PENDIENTE (líneas 765-802)
- `tools/setup_apolo_billing_complete.sql` ✅ OK
- Motor Rust: ✅ FUNCIONAL (no tocar)

---

## 🎯 RESUMEN

**Problema:** Backend muestra datos dummy  
**Causa:** Función `dashboard_cdr_view` con valores hardcodeados  
**Solución:** Reemplazar con consulta real a tabla `cdrs` de `apolo_billing`  
**Archivos:** `views.py` (pendiente modificar manualmente)

**Próximo paso:** Aplicar el fix en el servidor real.

