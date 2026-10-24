#!/usr/bin/env bash
# Agente: nginx — sitio del portal (HTTPS 443 → frontend 3000, /api → 8000,
# /api/v1/monitoring → 8080, /ws), con certificado autofirmado nuevo.

capture_nginx() {
    local d=$1
    # everything except the TLS key pair (regenerated) and the dSIPRouter GUI site
    tar -C / -czpf "$d/etc-nginx.tgz" --exclude='etc/nginx/ssl' \
        --exclude='etc/nginx/sites-enabled/dsiprouter.conf' --exclude='etc/nginx/sites-available/dsiprouter.conf' etc/nginx
    return 0
}

apply_nginx() {
    local d=$1
    command -v nginx >/dev/null || { log_err "nginx no está instalado"; return 1; }
    [[ -d /etc/nginx && ! -d /etc/nginx.pkg-default ]] && cp -a /etc/nginx /etc/nginx.pkg-default
    rm -f /etc/nginx/sites-enabled/*
    tar -C / -xzpf "$d/etc-nginx.tgz"
    rewrite_ips_tree /etc/nginx

    mkdir -p /etc/nginx/ssl
    openssl req -x509 -nodes -newkey rsa:2048 -days 1095 -sha256 \
        -keyout /etc/nginx/ssl/apolo.key -out /etc/nginx/ssl/apolo.crt \
        -subj "/CN=$NEW_MGMT" -addext "subjectAltName=IP:$NEW_MGMT,DNS:$NEW_FQDN,DNS:$NEW_HOSTNAME" >> "$LOG_FILE" 2>&1
    chmod 600 /etc/nginx/ssl/apolo.key
    log_ok "Certificado HTTPS autofirmado nuevo (CN=$NEW_MGMT, 3 años)"

    nginx -t >> "$LOG_FILE" 2>&1 || { log_err "nginx -t falló"; return 1; }
    log_ok "nginx configurado y verificado (nginx -t)"
    return 0
}

validate_nginx() {
    check "nginx activo" systemctl is-active --quiet nginx
    check "Portal https://$NEW_MGMT/ (HTTP 200)" http_code_is "https://$NEW_MGMT/" 200
    check "API vía nginx https://$NEW_MGMT/api/v1/health (HTTP 200)" http_code_is "https://$NEW_MGMT/api/v1/health" 200
    return 0
}
