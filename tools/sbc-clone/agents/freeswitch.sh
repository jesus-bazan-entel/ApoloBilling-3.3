#!/usr/bin/env bash
# Agente: FreeSWITCH — /etc/freeswitch completo (perfiles SIP, gateways,
# dialplan, módulos), contraseña ESL nueva y perfiles/gateways en ejecución.

capture_freeswitch() {
    local d=$1
    mkdir -p "$d/systemd"
    tar -C / -czpf "$d/etc-freeswitch.tgz" etc/freeswitch
    [[ -d /etc/systemd/system/freeswitch.service.d ]] && cp -a /etc/systemd/system/freeswitch.service.d/. "$d/systemd/"
    [[ -f /etc/default/freeswitch ]] && cp -a /etc/default/freeswitch "$d/default-freeswitch"
    # profiles running on the source, to compare after the clone
    fs_cli -x 'sofia status' 2>/dev/null | awk '$2=="profile" && $4=="RUNNING" {print $1}' > "$d/profiles-running.txt" || true
    fs_cli -x 'sofia status' 2>/dev/null | awk '$2=="gateway" {print $1}' > "$d/gateways.txt" || true
    return 0
}

apply_freeswitch() {
    local d=$1
    command -v freeswitch >/dev/null || { log_err "FreeSWITCH no está instalado"; return 1; }
    systemctl stop freeswitch &>/dev/null || true

    # replace the packaged default config entirely, keeping it aside
    [[ -d /etc/freeswitch && ! -d /etc/freeswitch.pkg-default ]] && mv /etc/freeswitch /etc/freeswitch.pkg-default
    rm -rf /etc/freeswitch
    tar -C / -xzpf "$d/etc-freeswitch.tgz"

    # media IP first (only where the source used its RTP-internal IP), then
    # the generic per-role rewrite for everything else
    local f
    while IFS= read -r -d '' f; do
        OLD="$SRC_RTP_INT" NEW="$NEW_RTP_INT" perl -i -pe \
            's/(name="rtp-ip"\s+value=")\Q$ENV{OLD}\E(")/${1}__SBC_RTP_INT__$2/g' "$f"
    done < <(find /etc/freeswitch/sip_profiles -type f -name '*.xml' -print0)
    rewrite_ips_tree /etc/freeswitch
    grep -rl '__SBC_RTP_INT__' /etc/freeswitch | xargs -r sed -i "s/__SBC_RTP_INT__/${NEW_RTP_INT}/g"
    log_ok "Perfiles SIP: señalización $NEW_SIG_INT, media $NEW_RTP_INT"

    ESL="$ESL_PASS" perl -i -pe 's/(<param name="password" value=")[^"]*(")/$1$ENV{ESL}$2/' \
        /etc/freeswitch/autoload_configs/event_socket.conf.xml
    log_ok "Contraseña ESL nueva"

    if compgen -G "$d/systemd/*" >/dev/null; then
        mkdir -p /etc/systemd/system/freeswitch.service.d
        install_tree "$d/systemd" /etc/systemd/system/freeswitch.service.d
    fi
    [[ -f $d/default-freeswitch ]] && cp "$d/default-freeswitch" /etc/default/freeswitch
    systemctl daemon-reload
    return 0
}

fs_running_profile() { fs_cli -p "$ESL_PASS" -x 'sofia status' | awk -v p="$1" '$1==p && $2=="profile" && $4=="RUNNING" {f=1} END {exit !f}'; }
fs_has_gateway() { fs_cli -p "$ESL_PASS" -x 'sofia status' | awk -v g="$1" '$1==g && $2=="gateway" {f=1} END {exit !f}'; }

validate_freeswitch() {
    local d=$1 p
    check "FreeSWITCH activo" systemctl is-active --quiet freeswitch
    check "FreeSWITCH ESL acepta la contraseña nueva" bash -c "fs_cli -p '$ESL_PASS' -x status | grep -q UP"
    while read -r p; do
        [[ -n $p ]] && check "FreeSWITCH perfil $p RUNNING" fs_running_profile "$p"
    done < "$d/profiles-running.txt"
    while read -r p; do
        [[ -n $p ]] && check_warn "FreeSWITCH gateway $p cargado" fs_has_gateway "$p"
    done < "$d/gateways.txt"
    return 0
}
