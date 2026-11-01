#!/usr/bin/env bash
# Agente: Kamailio — kamailio.cfg en producción, includes de /etc/dsiprouter,
# certificados, kamctlrc, unidad systemd, rsyslog y logrotate.
# Kamailio funciona de forma independiente (sin dSIPRouter), igual que el origen.

KAM_CFG=/etc/kamailio/kamailio.cfg

capture_kamailio() {
    local d=$1 f t
    mkdir -p "$d/etc/kamailio" "$d/dsip" "$d/default" "$d/systemd" "$d/misc"
    cp -a "$KAM_CFG" /etc/kamailio/kamctlrc "$d/etc/kamailio/"
    # MySQL credentials included by kamailio.cfg (since 2026-11-01)
    [[ -f /etc/kamailio/kamailio-db.cfg ]] && cp -a /etc/kamailio/kamailio-db.cfg "$d/etc/kamailio/"
    # symlinked includes (tls.cfg, stir-shaken.cfg, transnexus.cfg → /etc/dsiprouter)
    : > "$d/links.txt"
    for f in /etc/kamailio/*; do
        [[ -L $f ]] || continue
        t=$(readlink -f "$f")
        echo "$f|$t" >> "$d/links.txt"
        mkdir -p "$d/dsip$(dirname "$t")"; cp -a "$t" "$d/dsip$t"
    done
    # certificates except the self-signed TLS pair, which is regenerated
    tar -C / -czf "$d/dsip/certs.tgz" --exclude='etc/dsiprouter/certs/dsiprouter-cert.pem' \
        --exclude='etc/dsiprouter/certs/dsiprouter-key.pem' etc/dsiprouter/certs
    cp -a /etc/default/kamailio.conf "$d/default/"
    [[ -d /etc/default/kamailio.d ]] && cp -a /etc/default/kamailio.d "$d/default/"
    [[ -d /etc/systemd/system/kamailio.service.d ]] && cp -a /etc/systemd/system/kamailio.service.d/. "$d/systemd/"
    cp -a /etc/rsyslog.d/kamailio.conf /etc/logrotate.d/kamailio "$d/misc/" 2>/dev/null || true
    [[ -x /usr/local/sbin/kamailio-cert-renew ]] && cp -a /usr/local/sbin/kamailio-cert-renew "$d/misc/"
    return 0
}

# network (CIDR) of the interface that holds an IP on this host
iface_net_of() {
    python3 - "$1" <<'EOF'
import ipaddress, subprocess, sys
for line in subprocess.check_output(["ip", "-4", "-o", "addr", "show"], text=True).splitlines():
    cidr = line.split()[3]
    if cidr.split("/")[0] == sys.argv[1]:
        print(ipaddress.ip_interface(cidr).network); break
EOF
}
iface_of() { ip -4 -o addr show | awk -v ip="$1" '{split($4,a,"/"); if (a[1]==ip) print $2}' | head -1; }

set_substdef() {
    NAME="$1" VAL="$2" perl -i -pe 's/^(#!substdef "!\Q$ENV{NAME}\E!)[^!]*(!g")/$1$ENV{VAL}$2/' "$KAM_CFG"
}

apply_kamailio() {
    local d=$1 line link target pass
    command -v kamailio >/dev/null || { log_err "Kamailio no está instalado"; return 1; }

    cp "$d/etc/kamailio/kamailio.cfg" "$d/etc/kamailio/kamctlrc" /etc/kamailio/
    if [[ -f $d/etc/kamailio/kamailio-db.cfg ]]; then
        install -m 0640 -o root -g kamailio "$d/etc/kamailio/kamailio-db.cfg" /etc/kamailio/kamailio-db.cfg
    fi
    install_tree "$d/dsip/etc" /etc
    tar -C / -xzf "$d/dsip/certs.tgz"
    while IFS='|' read -r link target; do
        [[ -n $link ]] && ln -sfn "$target" "$link"
    done < "$d/links.txt"

    # IPs: generic rewrite, then the values that depend on the new host
    rewrite_ips "$KAM_CFG" /etc/kamailio/kamctlrc /etc/dsiprouter/kamailio/*.cfg
    local net ip6 ifc
    net=$(iface_net_of "$NEW_SIG_INT")
    [[ -n $net ]] && set_substdef INTERNAL_IP_NET "$net"
    ifc=$(iface_of "$NEW_SIG_INT")
    ip6=$(ip -6 -o addr show dev "$ifc" scope link 2>/dev/null | awk '{split($4,a,"/"); print a[1]; exit}')
    if [[ -n $ip6 ]]; then set_substdef INTERNAL_IP6_ADDR "$ip6"; set_substdef EXTERNAL_IP6_ADDR "$ip6"; fi
    set_substdef UAC_REG_ADDR "$NEW_SIG_EXT"
    log_ok "IPs: privada $NEW_SIG_INT ($net), pública $NEW_SIG_EXT"

    # credentials
    replace_old_secrets "$KAM_CFG" /etc/kamailio/kamctlrc /etc/kamailio/kamailio-db.cfg
    pass=$(cred_of kamailio "$MY_USER_PASSWORDS")
    [[ -n $pass ]] && sed -i -E "s/^(DBRWPW|DBROPW)=.*/\1=\"$pass\"/" /etc/kamailio/kamctlrc
    TOKEN="$KAM_API_TOKEN" perl -i -pe 's/^(server\.api_token\s*=\s*")[^"]*/$1$ENV{TOKEN}/' "$KAM_CFG"
    log_ok "Credenciales de base de datos y api_token nuevos en kamailio.cfg y kamctlrc"

    # new self-signed TLS pair (same recipe as kamailio-cert-renew)
    openssl req -new -newkey rsa:4096 -x509 -sha256 -days 365 -nodes \
        -out /etc/dsiprouter/certs/dsiprouter-cert.pem -keyout /etc/dsiprouter/certs/dsiprouter-key.pem \
        -subj "/C=US/ST=MI/L=Detroit/O=dSIPRouter/CN=$NEW_HOSTNAME" >> "$LOG_FILE" 2>&1
    chown -R dsiprouter:kamailio /etc/dsiprouter/certs /etc/dsiprouter/kamailio
    chmod 640 /etc/dsiprouter/certs/dsiprouter-*.pem /etc/dsiprouter/kamailio/*.cfg
    log_ok "Certificado TLS nuevo (CN=$NEW_HOSTNAME)"

    cp "$d/default/kamailio.conf" /etc/default/kamailio.conf
    [[ -d $d/default/kamailio.d ]] && cp -a "$d/default/kamailio.d" /etc/default/
    mkdir -p /etc/systemd/system/kamailio.service.d
    install_tree "$d/systemd" /etc/systemd/system/kamailio.service.d
    [[ -f $d/misc/kamailio.conf ]] && cp "$d/misc/kamailio.conf" /etc/rsyslog.d/kamailio.conf
    [[ -f $d/misc/kamailio ]] && cp "$d/misc/kamailio" /etc/logrotate.d/kamailio
    [[ -f $d/misc/kamailio-cert-renew ]] && install -m 755 "$d/misc/kamailio-cert-renew" /usr/local/sbin/kamailio-cert-renew
    systemctl restart rsyslog
    systemctl daemon-reload

    if ! kamailio -c -f "$KAM_CFG" >> "$LOG_FILE" 2>&1; then
        log_err "kamailio.cfg no pasa la verificación de sintaxis (kamailio -c)"; return 1
    fi
    log_ok "kamailio.cfg verificado (kamailio -c)"
    return 0
}

listening() {  # listening proto ip port
    local opt=-ltn; [[ $1 == udp ]] && opt=-lun
    ss -H $opt "src $2:$3" | grep -q .
}
sip_options_reply() {
    command -v sipsak >/dev/null || return 1
    timeout 10 sipsak -vv -s "sip:sbc-clone@$1:5060" 2>&1 | grep -q 'SIP/2.0 [1-6][0-9][0-9]'
}

validate_kamailio() {
    check "Kamailio activo" systemctl is-active --quiet kamailio
    check "Kamailio escucha UDP $NEW_SIG_EXT:5060 (carriers)" listening udp "$NEW_SIG_EXT" 5060
    check "Kamailio escucha TCP $NEW_SIG_EXT:5060" listening tcp "$NEW_SIG_EXT" 5060
    check "Kamailio escucha UDP $NEW_SIG_INT:5080 (FreeSWITCH)" listening udp "$NEW_SIG_INT" 5080
    check "Kamailio responde por RPC (kamcmd core.uptime)" kamcmd core.uptime
    check "Kamailio cargó los destinos del dispatcher desde MariaDB" bash -c 'kamcmd dispatcher.list | grep -q NRSETS'
    check "Kamailio recarga drouting desde MariaDB" kamcmd drouting.reload
    check_warn "Kamailio responde SIP OPTIONS en $NEW_SIG_EXT:5060" sip_options_reply "$NEW_SIG_EXT"
    return 0
}
