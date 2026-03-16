# GitHub Actions Workflows

Este directorio contiene los workflows de CI/CD para ApoloBilling.

## Workflows Disponibles

### 1. CI (Continuous Integration) - `ci.yml`

**Trigger:** Push y Pull Requests a `main` y `develop`

**Jobs:**
- **rust-backend**: Tests, linting y build del backend Rust
- **rust-billing-engine**: Tests, linting y build del billing engine
- **frontend**: Linting y build del frontend React

**Servicios:**
- PostgreSQL 15 (para tests)
- Redis 7 (para tests)

**Checks:**
- ✅ Rust formatting (`cargo fmt`)
- ✅ Clippy linting (`cargo clippy`)
- ✅ Tests unitarios e integración
- ✅ Build en modo release
- ✅ ESLint (frontend)
- ✅ TypeScript type checking

### 2. Deploy (Deployment) - `deploy.yml`

**Trigger:** Push a `main` o ejecución manual

**Requisitos:**
Debes configurar estos secrets en GitHub:

```
Settings → Secrets and variables → Actions → New repository secret
```

| Secret | Descripción | Ejemplo |
|--------|-------------|---------|
| `DEPLOY_HOST` | IP o dominio del servidor | `190.105.250.73` |
| `DEPLOY_USER` | Usuario SSH | `apolo` |
| `DEPLOY_SSH_KEY` | Clave privada SSH | `-----BEGIN RSA PRIVATE KEY-----...` |
| `DEPLOY_PORT` | Puerto SSH (opcional) | `22` |
| `SLACK_WEBHOOK` | Webhook de Slack (opcional) | `https://hooks.slack.com/...` |

**Proceso:**
1. Conecta al servidor via SSH
2. Pull de últimos cambios
3. Compila Rust backend
4. Compila Rust billing engine
5. Compila frontend
6. Reinicia servicios systemd
7. Verifica health check

### 3. Security Audit - `security.yml`

**Trigger:**
- Push y Pull Requests
- Cron: Cada lunes a las 9 AM UTC
- Ejecución manual

**Checks:**
- 🔒 `cargo audit` para vulnerabilidades Rust
- 🔒 `npm audit` para vulnerabilidades frontend
- 🔒 CodeQL analysis
- 📦 Dependencias desactualizadas

## Configuración Inicial

### Paso 1: Generar Clave SSH para Deployment

En tu servidor de producción:

```bash
# Generar clave SSH (sin passphrase para automatización)
ssh-keygen -t rsa -b 4096 -f ~/.ssh/github_actions -N ""

# Agregar clave pública a authorized_keys
cat ~/.ssh/github_actions.pub >> ~/.ssh/authorized_keys

# Copiar clave privada (para agregar a GitHub Secrets)
cat ~/.ssh/github_actions
```

### Paso 2: Configurar Secrets en GitHub

1. Ve a tu repositorio en GitHub
2. Settings → Secrets and variables → Actions
3. Click "New repository secret"
4. Agrega cada secret:

```
DEPLOY_HOST = 190.105.250.73
DEPLOY_USER = apolo
DEPLOY_SSH_KEY = (pega el contenido de ~/.ssh/github_actions)
DEPLOY_PORT = 22
```

### Paso 3: Configurar Permisos en el Servidor

Asegúrate de que el usuario `apolo` puede reiniciar servicios sin contraseña:

```bash
# Editar sudoers
sudo visudo

# Agregar esta línea:
apolo ALL=(ALL) NOPASSWD: /bin/systemctl restart apolo-backend, /bin/systemctl restart apolo-billing-engine, /bin/systemctl restart apolo-frontend, /bin/systemctl status apolo-*
```

### Paso 4: Verificar Configuración

```bash
# Probar conexión SSH desde tu máquina local
ssh -i ~/.ssh/github_actions apolo@190.105.250.73 "echo 'SSH OK'"

# Probar restart de servicios
ssh -i ~/.ssh/github_actions apolo@190.105.250.73 "sudo systemctl status apolo-backend"
```

## Uso de los Workflows

### Ejecución Automática

Los workflows se ejecutan automáticamente cuando:
- Haces push a `main` o `develop`
- Abres un Pull Request
- Cada lunes (security audit)

### Ejecución Manual

Para ejecutar el deployment manualmente:

1. Ve a tu repositorio en GitHub
2. Actions → Deploy → Run workflow
3. Selecciona la rama y click "Run workflow"

### Ver Resultados

1. Ve a la pestaña "Actions" en GitHub
2. Selecciona el workflow
3. Click en el run específico para ver logs

## Badges de Estado

Agrega estos badges a tu README.md:

```markdown
![CI](https://github.com/jesus-bazan-entel/ApoloBilling/workflows/CI/badge.svg)
![Security](https://github.com/jesus-bazan-entel/ApoloBilling/workflows/Security%20Audit/badge.svg)
```

## Troubleshooting

### Error: "Permission denied (publickey)"

**Solución:**
- Verifica que la clave SSH privada esté correctamente configurada en GitHub Secrets
- Verifica que la clave pública esté en `~/.ssh/authorized_keys` del servidor
- Verifica permisos: `chmod 600 ~/.ssh/authorized_keys`

### Error: "cargo: command not found"

**Solución:**
- Asegúrate de que `source ~/.cargo/env` esté en el script de deployment
- O agrega cargo al PATH del usuario

### Error: "Failed to connect to localhost:8000"

**Solución:**
- Aumenta el tiempo de sleep después de restart
- Verifica que los servicios estén configurados correctamente
- Revisa logs: `journalctl -u apolo-backend -n 50`

### Tests fallan en CI pero pasan localmente

**Solución:**
- Verifica que las variables de entorno estén configuradas en el workflow
- Asegúrate de que los servicios (PostgreSQL, Redis) estén disponibles
- Revisa los logs del workflow en GitHub Actions

## Mejoras Futuras

- [ ] Agregar tests de integración end-to-end
- [ ] Configurar deployment a staging antes de producción
- [ ] Agregar notificaciones a Slack/Discord
- [ ] Configurar rollback automático si health check falla
- [ ] Agregar workflow para release/tagging
- [ ] Configurar cache de Docker para builds más rápidos

## Recursos

- [GitHub Actions Documentation](https://docs.github.com/en/actions)
- [Workflow syntax](https://docs.github.com/en/actions/using-workflows/workflow-syntax-for-github-actions)
- [Encrypted secrets](https://docs.github.com/en/actions/security-guides/encrypted-secrets)
