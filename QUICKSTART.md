# 🚀 Apolo Billing - Despliegue Rápido en WSL

## Método Rápido (Recomendado)

### 1️⃣ Clonar el repositorio en tu WSL Debian

```bash
cd ~
git clone https://github.com/jesus-bazan-entel/ApoloBilling.git
cd ApoloBilling
git checkout genspark_ai_developer
```

### 2️⃣ Ejecutar script de instalación automática

```bash
./quick_start_wsl.sh
```

El script automáticamente:
- ✅ Verifica e instala dependencias (Python, PostgreSQL, Redis)
- ✅ Crea base de datos y usuario
- ✅ Instala dependencias Python
- ✅ Configura variables de entorno
- ✅ Inicializa la base de datos
- ✅ Crea usuario admin
- ✅ Inicia el servidor

### 3️⃣ Acceder al Dashboard

Abrir en tu navegador Windows:
```
http://localhost:8000
```

**Credenciales por defecto:**
- Username: `admin`
- Password: `admin123`

### 4️⃣ Acceder a Rate Cards UI

```
http://localhost:8000/dashboard/rate-cards
```

---

## Método Manual (Paso a Paso)

Si prefieres instalar manualmente, consulta: [`DEPLOYMENT_GUIDE_WSL.md`](./DEPLOYMENT_GUIDE_WSL.md)

---

## 🛑 Detener el Servidor

Presionar `Ctrl + C` en la terminal donde corre el servidor.

---

## 🔄 Reiniciar Servicios

```bash
# Reiniciar PostgreSQL
sudo service postgresql restart

# Reiniciar Redis
sudo service redis-server restart

# Reiniciar Backend
cd backend
source venv/bin/activate
uvicorn main:app --host 0.0.0.0 --port 8000 --reload
```

---

## 📊 Verificar Estado

```bash
# Verificar PostgreSQL
sudo service postgresql status

# Verificar Redis
redis-cli ping

# Verificar Backend
curl http://localhost:8000/docs
```

---

## 🐛 Solución de Problemas

### Error: "Port 8000 already in use"

```bash
# Encontrar proceso
sudo lsof -i :8000

# Matar proceso
sudo kill -9 <PID>
```

### Error: "Connection to PostgreSQL refused"

```bash
# Iniciar PostgreSQL
sudo service postgresql start

# Verificar estado
sudo service postgresql status
```

### Error: "Redis connection refused"

```bash
# Iniciar Redis
sudo service redis-server start

# Verificar
redis-cli ping
```

---

## 📝 Notas Importantes

1. **Primera vez**: El script `quick_start_wsl.sh` configurará todo automáticamente
2. **Acceso desde Windows**: Usar `http://localhost:8000` (WSL2 automáticamente hace port forwarding)
3. **Cambiar password**: Después del primer login, cambiar la contraseña del admin
4. **Datos de prueba**: Importar rate cards usando el botón "Import CSV" en el dashboard

---

## 📚 Documentación Adicional

- [`DEPLOYMENT_GUIDE_WSL.md`](./DEPLOYMENT_GUIDE_WSL.md) - Guía completa de despliegue
- [`UI_MIGRATION_COMPLETED.md`](./UI_MIGRATION_COMPLETED.md) - Documentación de la UI
- [`MIGRATION_PLAN_RATE_CARDS.md`](./MIGRATION_PLAN_RATE_CARDS.md) - Plan de migración
- [`DATABASE_ANALYSIS.md`](./DATABASE_ANALYSIS.md) - Análisis del modelo de datos

---

## 🎯 Próximos Pasos

1. ✅ Desplegar sistema (este documento)
2. 📥 Importar rate cards existentes
3. 👥 Crear usuarios adicionales
4. 🧪 Probar funcionalidades CRUD
5. 📊 Configurar monitoreo

---

## 🆘 Soporte

Para más ayuda, consultar:
- Documentación completa: `DEPLOYMENT_GUIDE_WSL.md`
- Pull Request: https://github.com/jesus-bazan-entel/ApoloBilling/pull/1
- Logs del sistema: `backend/logs/`

---

**✨ ¡Listo para usar Apolo Billing en WSL!**
