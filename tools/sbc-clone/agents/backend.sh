#!/usr/bin/env bash
# Agente: backend Apolo — /opt/ApoloBilling (código, binarios Rust ya
# compilados para Debian 12, .env), API (apolo-backend) y motor de tarifación
# (apolo-billing-engine). Reescribe IPs y todas las credenciales en los .env.

APOLO_DIR=/opt/ApoloBilling
BACKEND_UNITS=(apolo-backend apolo-billing-engine)
RUST_BINS=(rust-backend/target/release/apolo-billing rust-billing-engine/target/release/apolo-billing-engine)

capture_backend() {
    local d=$1 u
    mkdir -p "$d/units"
    # code + config without build trees, dependencies or logs (≈100 MB)
    tar -C / -czpf "$d/apolo.tgz" \
        --exclude="${APOLO_DIR#/}/rust-backend/target" --exclude="${APOLO_DIR#/}/rust-billing-engine/target" \
        --exclude="${APOLO_DIR#/}/frontend/node_modules" --exclude="${APOLO_DIR#/}/venv" \
        --exclude="${APOLO_DIR#/}/logs/*" --exclude='__pycache__' --exclude='*.pyc' \
        "${APOLO_DIR#/}"
    # compiled release binaries, in their own archive: tar's --exclude above
    # also drops members named explicitly on the command line
    tar -C / -czpf "$d/bins.tgz" "${RUST_BINS[@]/#/${APOLO_DIR#/}/}"
    for u in "${BACKEND_UNITS[@]}"; do
        cp -a "/etc/systemd/system/$u.service" "$d/units/"
        [[ -d /etc/systemd/system/$u.service.d ]] && cp -a "/etc/systemd/system/$u.service.d" "$d/units/"
    done
    return 0
}

# every credential and IP in a dotenv / unit file
rewrite_app_secrets() {
    local f=$1
    rewrite_ips "$f"
    replace_old_secrets "$f"
    grep -q '^JWT_SECRET=' "$f" && set_env_var "$f" JWT_SECRET "$JWT_SECRET"
    grep -q '^SECRET_KEY=' "$f" && set_env_var "$f" SECRET_KEY "$APP_SECRET_KEY"
    grep -q '^SUPERADMIN_PASSWORD=' "$f" && set_env_var "$f" SUPERADMIN_PASSWORD "$ADMIN_PASS"
    # FREESWITCH_SERVERS=host:port:password[,host:port:password]
    ESL="$ESL_PASS" perl -i -pe 's/(:\d+:)[^,\s]+/$1$ENV{ESL}/g if /^FREESWITCH_SERVERS=/' "$f"
    return 0
}

apply_backend() {
    local d=$1 f u
    systemctl stop "${BACKEND_UNITS[@]}" &>/dev/null || true
    [[ -d $APOLO_DIR ]] && mv "$APOLO_DIR" "$APOLO_DIR.pre-clone-$TS"
    tar -C / -xzpf "$d/apolo.tgz"
    tar -C / -xzpf "$d/bins.tgz"
    usermod -aG kamailio,freeswitch apolo 2>/dev/null || true
    mkdir -p "$APOLO_DIR/logs"; chown apolo:apolo "$APOLO_DIR/logs"
    log_ok "Código y binarios en $APOLO_DIR"

    while IFS= read -r -d '' f; do
        rewrite_app_secrets "$f"
    done < <(find "$APOLO_DIR" -maxdepth 3 -name '.env' -type f -not -path '*/node_modules/*' -print0)
    log_ok "Archivos .env: IPs y credenciales nuevas (SIP_ENCRYPTION_KEY se conserva)"

    # IPs and passwords hardcoded in runtime code (e.g. the monitoring
    # collector logs into MariaDB with a literal password)
    local -a code=()
    mapfile -d '' code < <(find "$APOLO_DIR" \( -path '*/.git' -o -path '*/node_modules' -o -path '*/target' \
        -o -path '*/.claude' -o -path "$APOLO_DIR/frontend/dist" \) -prune -o -type f \( -name '*.py' -o -name '*.sh' \
        -o -name '*.js' -o -name '*.json' -o -name '*.toml' -o -name '*.yml' -o -name '*.yaml' -o -name '*.conf' \
        -o -name '*.ini' -o -name '*.xml' \) -print0)
    rewrite_ips "${code[@]}"
    replace_old_secrets "${code[@]}"
    log_ok "Código del proyecto: IPs y contraseñas escritas a mano actualizadas (${#code[@]} archivos revisados)"

    for u in "${BACKEND_UNITS[@]}"; do
        cp "$d/units/$u.service" /etc/systemd/system/
        [[ -d $d/units/$u.service.d ]] && install_tree "$d/units/$u.service.d" "/etc/systemd/system/$u.service.d"
        rewrite_app_secrets "/etc/systemd/system/$u.service"
    done
    systemctl daemon-reload

    for f in "${RUST_BINS[@]}"; do
        if ldd "$APOLO_DIR/$f" | grep -q 'not found'; then
            log_err "$f: faltan librerías: $(ldd "$APOLO_DIR/$f" | grep 'not found' | awk '{print $1}' | paste -sd' ')"
            return 1
        fi
    done
    log_ok "Binarios Rust con todas sus librerías"
    return 0
}

http_code_is() { [[ $(curl -sk -o /dev/null -w '%{http_code}' --max-time 10 "$1") == "$2" ]]; }
api_login() {
    local body
    body=$(U="$ADMIN_USER" P="$ADMIN_PASS" python3 -c \
        'import json,os; print(json.dumps({"username": os.environ["U"], "password": os.environ["P"]}))')
    curl -s --max-time 10 -H 'Content-Type: application/json' -d "$body" http://127.0.0.1:8000/api/v1/auth/login |
        grep -q '"access_token"'
}

validate_backend() {
    check "apolo-backend activo" systemctl is-active --quiet apolo-backend
    check "apolo-billing-engine activo" systemctl is-active --quiet apolo-billing-engine
    check "API responde /api/v1/health (HTTP 200)" http_code_is http://127.0.0.1:8000/api/v1/health 200
    check "Motor de tarifación responde /api/v1/health (HTTP 200)" http_code_is http://127.0.0.1:9000/api/v1/health 200
    check "Login del portal con $ADMIN_USER y la contraseña nueva" api_login
    return 0
}
