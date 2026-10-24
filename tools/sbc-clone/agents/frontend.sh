#!/usr/bin/env bash
# Agente: frontend — portal web ya compilado (frontend/dist, copiado por el
# agente backend) servido por `serve` en el puerto 3000 (apolo-frontend).

capture_frontend() {
    local d=$1
    cp -a /etc/systemd/system/apolo-frontend.service "$d/"
    [[ -d /etc/systemd/system/apolo-frontend.service.d ]] && cp -a /etc/systemd/system/apolo-frontend.service.d "$d/"
    return 0
}

apply_frontend() {
    local d=$1
    [[ -d /opt/ApoloBilling/frontend/dist ]] || { log_err "No existe frontend/dist (¿falló el agente backend?)"; return 1; }
    # the API URL is baked into the build and the .env
    rewrite_ips /opt/ApoloBilling/frontend/.env
    rewrite_ips_tree /opt/ApoloBilling/frontend/dist
    cp "$d/apolo-frontend.service" /etc/systemd/system/
    [[ -d $d/apolo-frontend.service.d ]] && install_tree "$d/apolo-frontend.service.d" /etc/systemd/system/apolo-frontend.service.d
    command -v serve >/dev/null || { log_err "Falta 'serve' (npm global)"; return 1; }
    systemctl daemon-reload
    log_ok "Portal compilado con las IPs nuevas"
    return 0
}

validate_frontend() {
    check "apolo-frontend activo" systemctl is-active --quiet apolo-frontend
    check "Portal responde en 127.0.0.1:3000 (HTTP 200)" http_code_is http://127.0.0.1:3000/ 200
    return 0
}
