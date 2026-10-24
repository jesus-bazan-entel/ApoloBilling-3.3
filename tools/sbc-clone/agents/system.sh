#!/usr/bin/env bash
# Agente: sistema base — hostname, /etc/hosts, zona horaria, locale, sysctl,
# límites, usuarios de servicio y rutas estáticas persistentes.

capture_system() {
    local d=$1
    mkdir -p "$d/etc"
    timedatectl show -p Timezone --value > "$d/timezone"
    cp -a /etc/default/locale "$d/etc/" 2>/dev/null || true
    grep -vE '^\s*#|^\s*$' /etc/locale.gen > "$d/locale.gen" 2>/dev/null || true
    cp -a /etc/hosts "$d/etc/hosts"
    cp -a /etc/sysctl.conf "$d/etc/" 2>/dev/null || true
    mkdir -p "$d/etc/sysctl.d" "$d/etc/modules-load.d" "$d/etc/security/limits.d"
    cp -a /etc/sysctl.d/. "$d/etc/sysctl.d/" 2>/dev/null || true
    cp -a /etc/modules-load.d/. "$d/etc/modules-load.d/" 2>/dev/null || true
    cp -a /etc/security/limits.conf "$d/etc/security/" 2>/dev/null || true
    cp -a /etc/security/limits.d/. "$d/etc/security/limits.d/" 2>/dev/null || true
    return 0
}

apply_system() {
    local d=$1

    hostnamectl set-hostname "$NEW_HOSTNAME"
    log_ok "Hostname: $NEW_HOSTNAME"

    # source /etc/hosts with IPs and names translated, plus the new hostname
    cp "$d/etc/hosts" /etc/hosts.sbc-clone.tmp
    rewrite_ips /etc/hosts.sbc-clone.tmp
    sed -i -E "/^127\.0\.1\.1\s/d; s/(\s)${SRC_HOSTNAME}(\s|$)/\1${NEW_HOSTNAME}\2/g" /etc/hosts.sbc-clone.tmp
    sed -i "1a 127.0.1.1\t${NEW_HOSTNAME}" /etc/hosts.sbc-clone.tmp
    cp -a /etc/hosts "/etc/hosts.bak-sbc-clone" 2>/dev/null || true
    mv /etc/hosts.sbc-clone.tmp /etc/hosts
    log_ok "/etc/hosts"

    timedatectl set-timezone "$(cat "$d/timezone")"
    log_ok "Zona horaria: $(cat "$d/timezone")"

    apt-get update -qq >> "$LOG_FILE" 2>&1
    apt-get install -y -qq locales >> "$LOG_FILE" 2>&1
    if [[ -s $d/locale.gen ]]; then
        while read -r line; do
            grep -qxF "$line" /etc/locale.gen || echo "$line" >> /etc/locale.gen
        done < "$d/locale.gen"
        locale-gen >> "$LOG_FILE" 2>&1
    fi
    [[ -f $d/etc/locale ]] && cp "$d/etc/locale" /etc/default/locale
    log_ok "Locale"

    [[ -f $d/etc/sysctl.conf ]] && cp "$d/etc/sysctl.conf" /etc/sysctl.conf
    install_tree "$d/etc/sysctl.d" /etc/sysctl.d
    install_tree "$d/etc/modules-load.d" /etc/modules-load.d
    [[ -f $d/etc/security/limits.conf ]] && cp "$d/etc/security/limits.conf" /etc/security/limits.conf
    install_tree "$d/etc/security/limits.d" /etc/security/limits.d
    sysctl --system >> "$LOG_FILE" 2>&1 || true
    log_ok "sysctl y límites"

    # service accounts the configs expect (packages create their own later)
    getent group apolo >/dev/null || groupadd --system apolo
    getent passwd apolo >/dev/null || useradd --system -g apolo -d /opt/ApoloBilling -s /bin/bash apolo
    getent group dsiprouter >/dev/null || groupadd --system dsiprouter
    getent passwd dsiprouter >/dev/null || useradd --system -g dsiprouter -d /home/dsiprouter -s /bin/false dsiprouter
    log_ok "Usuarios apolo y dsiprouter"

    apply_static_routes
    return 0
}

apply_static_routes() {
    local script=/usr/local/sbin/sbc-static-routes
    {
        echo '#!/usr/bin/env bash'
        echo '# Rutas estáticas del SBC (generado por sbc-clone). Lo ejecuta sbc-static-routes.service.'
        local r
        for r in $ROUTES; do
            echo "ip route replace ${r%%>*} via ${r#*>} || echo 'no se pudo crear la ruta ${r%%>*} via ${r#*>}' >&2"
        done
        echo 'exit 0'
    } > "$script"
    chmod 755 "$script"
    cat > /etc/systemd/system/sbc-static-routes.service <<'EOF'
[Unit]
Description=SBC static routes to carriers and remote networks
Wants=network-online.target
After=network-online.target

[Service]
Type=oneshot
RemainAfterExit=yes
ExecStart=/usr/local/sbin/sbc-static-routes

[Install]
WantedBy=multi-user.target
EOF
    systemctl daemon-reload
    systemctl enable sbc-static-routes.service >> "$LOG_FILE" 2>&1
    systemctl restart sbc-static-routes.service
    log_ok "Rutas estáticas persistentes: ${ROUTES:-ninguna}"
    return 0
}

route_present() { ip -4 route show "${1%%>*}" | grep -q "via ${1#*>}"; }

validate_system() {
    check "Hostname es $NEW_HOSTNAME" test "$(hostname)" = "$NEW_HOSTNAME"
    check "Zona horaria $(cat "$1/timezone")" test "$(timedatectl show -p Timezone --value)" = "$(cat "$1/timezone")"
    local r
    for r in $ROUTES; do
        check "Ruta ${r%%>*} vía ${r#*>}" route_present "$r"
    done
    return 0
}
