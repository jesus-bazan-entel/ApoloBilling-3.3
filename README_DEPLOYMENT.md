# 🚀 ApoloBilling - Guía Rápida de Despliegue

## 📋 **RESUMEN EJECUTIVO**

Este proyecto incluye scripts automatizados para subir el código a GitHub y desplegar en producción de forma sencilla.

### 📁 **Archivos Disponibles:**
- `upload_to_github.sh` - Script para subir código a GitHub
- `deploy_to_production.sh` - Script para desplegar en producción  
- `GITHUB_DEPLOYMENT_GUIDE.md` - Guía detallada completa
- `README.md` - Este archivo con instrucciones rápidas

---

## 🔥 **PROCESO RÁPIDO (3 PASOS)**

### **PASO 1: Subir a GitHub** ⬆️
```bash
# Ejecutar script automatizado
./upload_to_github.sh

# El script solicitará:
# - Tu nombre y email para Git
# - Tu token personal de GitHub
# - Confirmación para proceder
```

### **PASO 2: Preparar Servidor** 🖥️
```bash
# Conectar a tu servidor de producción
ssh usuario@servidor-produccion

# Instalar Git (si no está instalado)
sudo apt install git -y
```

### **PASO 3: Desplegar** 🚀
```bash
# Descargar script de despliegue
wget https://raw.githubusercontent.com/jesus-bazan-entel/ApoloBilling/main/deploy_to_production.sh
chmod +x deploy_to_production.sh

# Ejecutar despliegue (elegir docker o manual)
./deploy_to_production.sh docker    # Recomendado
# o
./deploy_to_production.sh manual     # Sin Docker
```

---

## 🔧 **DETALLES DE CONFIGURACIÓN**

### **🔑 Token de GitHub**
Para crear tu token personal:
1. Ve a [GitHub.com → Settings → Developer settings → Personal access tokens](https://github.com/settings/tokens)
2. Click "Generate new token (classic)"
3. Selecciona permisos: `repo`, `workflow`, `write:packages`
4. Copia el token para usar en el script

### **💾 Credenciales de Base de Datos**
El script generará automáticamente:
- Contraseña segura para PostgreSQL
- Usuario: `apolo_user`
- Base de datos: `apolo_billing`

### **🌐 Puertos del Sistema**
- **80/443**: Web Interface
- **8000**: API Backend
- **8080**: Billing Engine  
- **8021**: ESL (FreeSWITCH)
- **5432**: PostgreSQL
- **6379**: Redis

---

## 📊 **VERIFICACIÓN DEL DESPLIEGUE**

### **✅ Checklist Post-Despliegue:**
- [ ] Repositorio creado en GitHub
- [ ] Código subido correctamente
- [ ] Servidor accesible vía web
- [ ] API respondiendo en puerto 8000
- [ ] Base de datos funcionando
- [ ] Servicios iniciados correctamente

### **🔍 Comandos de Verificación:**
```bash
# Verificar servicios
sudo systemctl status apolo-api-backend
sudo systemctl status apolo-billing-engine

# Verificar puertos
netstat -tuln | grep :8000
netstat -tuln | grep :8080

# Verificar logs
tail -f /opt/logs/health_check.log
```

---

## 🛠️ **COMANDOS ÚTILES**

### **🔄 Actualizaciones Futuras:**
```bash
# En el servidor de producción
cd /opt/ApoloBilling
git pull origin main

# Reiniciar servicios
sudo systemctl restart apolo-api-backend apolo-billing-engine
```

### **💾 Backup Manual:**
```bash
# Ejecutar backup
/opt/backup.sh

# Ver backups disponibles
ls -la /opt/backups/
```

### **📈 Monitoreo:**
```bash
# Ver estado de salud
/opt/health_check.sh

# Ver logs en tiempo real
tail -f /var/log/syslog
```

---

## 🆘 **SOLUCIÓN DE PROBLEMAS**

### **❌ Error de Conexión a Base de Datos:**
```bash
# Verificar PostgreSQL
sudo systemctl status postgresql

# Reiniciar PostgreSQL
sudo systemctl restart postgresql
```

### **❌ Servicios No Inician:**
```bash
# Ver logs detallados
sudo journalctl -u apolo-api-backend -f

# Verificar variables de entorno
cat /opt/ApoloBilling/.env.production
```

### **❌ Problemas de Memoria:**
```bash
# Ver uso de memoria
free -h

# Ver procesos que consumen más memoria
ps aux --sort=-%mem | head -10
```

---

## 📞 **SOPORTE**

### **📋 Información del Sistema:**
- **Repositorio**: https://github.com/jesus-bazan-entel/ApoloBilling
- **Versión**: v1.0.0
- **Documentación**: Ver `GITHUB_DEPLOYMENT_GUIDE.md`

### **🔧 Logs Importantes:**
- **Sistema**: `/var/log/syslog`
- **Aplicación**: `/opt/logs/`
- **Nginx**: `/var/log/nginx/`
- **PostgreSQL**: `/var/log/postgresql/`

### **📧 Contacto:**
- Desarrollador: Jesús Bazán Entel
- Email: jesus-bazan-entel@entel.pe

---

## 🎯 **PRÓXIMOS PASOS**

1. **✅ Ejecutar** `upload_to_github.sh` para subir código
2. **✅ Ejecutar** `deploy_to_production.sh` en servidor
3. **✅ Verificar** que todos los servicios estén funcionando
4. **✅ Configurar** SSL/HTTPS para producción
5. **✅ Programar** monitoreo externo (opcional)
6. **✅ Configurar** alertas por email (opcional)

---

## 🏆 **¡LISTO PARA PRODUCCIÓN!**

Tu sistema ApoloBilling estará completamente desplegado y funcionando en producción con:
- ✅ Código en GitHub
- ✅ Servicios automatizados
- ✅ Base de datos configurada  
- ✅ Backup automático
- ✅ Monitoreo de salud
- ✅ Firewall configurado
- ✅ Logs centralizados

**¡Disfruta tu sistema de facturación en producción!** 🚀