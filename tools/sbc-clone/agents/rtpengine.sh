#!/usr/bin/env bash
# Agente: RTPEngine — rtpengine.conf, unidad systemd, verificación del módulo
# de kernel (reenvío en kernel), backend iptables, rsyslog y logrotate.

RTP_CONF=/etc/rtpengine/rtpengine.conf

capture_rtpengine() {
    local d=$1
    mkdir -p "$d/systemd" "$d/misc"
    cp -a "$RTP_CONF" "$d/rtpengine.conf"
    cp -a /etc/default/rtpengine.conf "$d/default-rtpengine.conf"
    [[ -d /etc/systemd/system/rtpengine.service.d ]] && cp -a /etc/systemd/system/rtpengine.service.d/. "$d/systemd/"
    [[ -f /usr/local/sbin/rtpengine-kernel-check ]] && cp -a /usr/local/sbin/rtpengine-kernel-check "$d/misc/"
    cp -a /etc/rsyslog.d/rtpengine.conf "$d/misc/rsyslog-rtpengine.conf" 2>/dev/null || true
    cp -a /etc/logrotate.d/rtpengine "$d/misc/logrotate-rtpengine" 2>/dev/null || true
    update-alternatives --query iptables 2>/dev/null | awk '/^Value:/ {print $2}' > "$d/iptables-alt"
    return 0
}

apply_rtpengine() {
    local d=$1
    command -v rtpengine >/dev/null || { log_err "RTPEngine no está instalado"; return 1; }

    mkdir -p /etc/rtpengine
    cp "$d/rtpengine.conf" "$RTP_CONF"
    rewrite_ips "$RTP_CONF"
    # explicit, because the source may share one IP between SIP and RTP roles
    sed -i -E "s|^interface\s*=.*|interface = internal/${NEW_RTP_INT};external/${NEW_RTP_EXT}|" "$RTP_CONF"
    sed -i -E "s|^port-min\s*=.*|port-min = ${NEW_RTP_MIN}|; s|^port-max\s*=.*|port-max = ${NEW_RTP_MAX}|" "$RTP_CONF"
    log_ok "rtpengine.conf: internal/$NEW_RTP_INT external/$NEW_RTP_EXT puertos $NEW_RTP_MIN-$NEW_RTP_MAX"

    # CONFIG_FILE, user and iptables options of the packaged unit
    cp "$d/default-rtpengine.conf" /etc/default/rtpengine.conf
    mkdir -p /etc/systemd/system/rtpengine.service.d
    install_tree "$d/systemd" /etc/systemd/system/rtpengine.service.d
    [[ -f $d/misc/rtpengine-kernel-check ]] && install -m 755 "$d/misc/rtpengine-kernel-check" /usr/local/sbin/rtpengine-kernel-check
    [[ -f $d/misc/rsyslog-rtpengine.conf ]] && cp "$d/misc/rsyslog-rtpengine.conf" /etc/rsyslog.d/rtpengine.conf
    [[ -f $d/misc/logrotate-rtpengine ]] && cp "$d/misc/logrotate-rtpengine" /etc/logrotate.d/rtpengine
    systemctl restart rsyslog
    systemctl daemon-reload

    local alt
    alt=$(cat "$d/iptables-alt")
    if [[ -n $alt && -x $alt ]]; then
        update-alternatives --set iptables "$alt" >> "$LOG_FILE" 2>&1
        [[ -x ${alt/iptables/ip6tables} ]] && update-alternatives --set ip6tables "${alt/iptables/ip6tables}" >> "$LOG_FILE" 2>&1
        log_ok "Backend iptables: $alt"
    fi

    if modprobe xt_RTPENGINE >> "$LOG_FILE" 2>&1; then
        log_ok "Módulo de kernel xt_RTPENGINE cargado"
    else
        log_warn "No se pudo cargar xt_RTPENGINE; RTPEngine funcionará en modo usuario (revisa dkms status)"
    fi
    return 0
}

validate_rtpengine() {
    check "RTPEngine activo" systemctl is-active --quiet rtpengine
    check "RTPEngine control NG en 127.0.0.1:7722" bash -c 'ss -Hlun "src 127.0.0.1:7722" | grep -q .'
    check "RTPEngine responde a rtpengine-ctl" bash -c 'rtpengine-ctl list numsessions >/dev/null'
    check "RTPEngine interfaces internal/$NEW_RTP_INT external/$NEW_RTP_EXT" \
        grep -qE "^interface\s*=\s*internal/${NEW_RTP_INT};external/${NEW_RTP_EXT}\s*$" "$RTP_CONF"
    check_warn "Reenvío RTP en kernel (xt_RTPENGINE cargado)" bash -c 'lsmod | grep -q xt_RTPENGINE'
    return 0
}
