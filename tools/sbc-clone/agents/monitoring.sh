#!/usr/bin/env bash
# Agente: monitoreo y operación — apolo-monitoring (puerto 8080), scripts de
# /usr/local/bin, timers (freeswitch-clock-check), crontab de root (purga de
# acc, renovación de certificado), SNMP, nullmailer y Redis.

capture_monitoring() {
    local d=$1 f u
    mkdir -p "$d/units" "$d/bin" "$d/snmp" "$d/nullmailer"
    cp -a /etc/systemd/system/apolo-monitoring.service "$d/units/"
    [[ -d /etc/systemd/system/apolo-monitoring.service.d ]] && cp -a /etc/systemd/system/apolo-monitoring.service.d "$d/units/"
    # custom timers and the services they trigger
    for f in /etc/systemd/system/*.timer; do
        [[ -f $f ]] || continue
        u=$(basename "$f" .timer)
        cp -a "$f" "$d/units/"
        [[ -f /etc/systemd/system/$u.service ]] && cp -a "/etc/systemd/system/$u.service" "$d/units/"
    done
    cp -a /usr/local/bin/. "$d/bin/"
    crontab -l -u root > "$d/crontab-root" 2>/dev/null || true
    cp -a /etc/snmp/snmpd.conf "$d/snmp/" 2>/dev/null || true
    cp -a /etc/default/snmpd "$d/snmp/default-snmpd" 2>/dev/null || true
    cp -a /etc/nullmailer/. "$d/nullmailer/" 2>/dev/null || true
    cp -a /etc/redis/redis.conf "$d/redis.conf" 2>/dev/null || true
    return 0
}

apply_monitoring() {
    local d=$1 f u
    install_tree "$d/bin" /usr/local/bin
    local -a bins=()
    for f in "$d"/bin/*; do [[ -f $f ]] && bins+=("/usr/local/bin/$(basename "$f")"); done
    rewrite_ips "${bins[@]}"
    replace_old_secrets "${bins[@]}"
    log_ok "Scripts de /usr/local/bin ($(ls "$d/bin" | paste -sd' '))"

    for f in "$d"/units/*.service "$d"/units/*.timer; do
        [[ -f $f ]] || continue
        cp "$f" /etc/systemd/system/
        rewrite_app_secrets "/etc/systemd/system/$(basename "$f")"
    done
    [[ -d $d/units/apolo-monitoring.service.d ]] && install_tree "$d/units/apolo-monitoring.service.d" /etc/systemd/system/apolo-monitoring.service.d
    systemctl daemon-reload
    for f in "$d"/units/*.timer; do
        [[ -f $f ]] || continue
        systemctl enable --now "$(basename "$f")" >> "$LOG_FILE" 2>&1 || log_warn "No se pudo activar $(basename "$f")"
    done
    log_ok "apolo-monitoring y timers"

    if [[ -s $d/crontab-root ]]; then
        cp "$d/crontab-root" /tmp/sbc-clone-crontab
        rewrite_ips /tmp/sbc-clone-crontab
        crontab -u root /tmp/sbc-clone-crontab
        rm -f /tmp/sbc-clone-crontab
        log_ok "Crontab de root ($(grep -cvE '^\s*(#|$)' "$d/crontab-root") tareas)"
    fi

    if [[ -f $d/snmp/snmpd.conf ]]; then
        cp "$d/snmp/snmpd.conf" /etc/snmp/snmpd.conf
        [[ -f $d/snmp/default-snmpd ]] && cp "$d/snmp/default-snmpd" /etc/default/snmpd
        rewrite_ips /etc/snmp/snmpd.conf
        sed -i -E "s/^(sysLocation\s+).*/\1${NEW_HOSTNAME}/" /etc/snmp/snmpd.conf
        log_ok "snmpd"
    fi
    if compgen -G "$d/nullmailer/*" >/dev/null && [[ -d /etc/nullmailer ]]; then
        install_tree "$d/nullmailer" /etc/nullmailer
        echo "$NEW_HOSTNAME" > /etc/mailname
        log_ok "nullmailer"
    fi
    if [[ -f $d/redis.conf ]]; then
        install -o redis -g redis -m 640 "$d/redis.conf" /etc/redis/redis.conf
        log_ok "Redis (misma configuración: localhost, 512 MB, allkeys-lru)"
    fi
    return 0
}

validate_monitoring() {
    local d=$1 f
    check "apolo-monitoring activo" systemctl is-active --quiet apolo-monitoring
    check "Monitoreo escucha en el puerto 8080" bash -c 'ss -Hltn "sport = :8080" | grep -q .'
    check "Redis responde (PING)" bash -c 'redis-cli ping | grep -q PONG'
    for f in "$d"/units/*.timer; do
        [[ -f $f ]] && check "Timer $(basename "$f") activo" systemctl is-active --quiet "$(basename "$f")"
    done
    check "Crontab de root con las mismas tareas que el origen" \
        test "$(crontab -l -u root 2>/dev/null | grep -cvE '^\s*(#|$)')" = "$(grep -cvE '^\s*(#|$)' "$d/crontab-root")"
    [[ -f $d/snmp/snmpd.conf ]] && check "snmpd activo" systemctl is-active --quiet snmpd
    return 0
}
