#!/usr/bin/env bash
# Agente: firewall — firewalld con las mismas zonas, puertos, servicios y
# reglas que el origen (IPs propias traducidas; IPs de terceros intactas).
# Se aplica al final y con reversión automática: si sbc-clone.sh no logra abrir
# una conexión SSH nueva al terminar, en 15 minutos se restaura la
# configuración anterior y se detiene firewalld, para no perder el acceso.

FW_ROLLBACK_UNIT=sbc-firewall-rollback

capture_firewall() {
    local d=$1
    tar -C / -czpf "$d/etc-firewalld.tgz" --exclude='*.old' etc/firewalld
    firewall-cmd --get-default-zone > "$d/default-zone"
    local z
    for z in $(firewall-cmd --get-zones); do
        firewall-cmd --permanent --zone="$z" --list-ports 2>/dev/null | tr ' ' '\n' | sed "s/^/$z /"
    done | grep -v ' $' > "$d/ports.txt" || true
    firewall-cmd --permanent --list-rich-rules --zone="$(cat "$d/default-zone")" > "$d/rich-rules.txt" 2>/dev/null || true
    return 0
}

apply_firewall() {
    local d=$1 zone backup
    command -v firewall-cmd >/dev/null || { log_err "firewalld no está instalado"; return 1; }
    backup=/root/sbc-clone/$TS/firewalld.pre-clone
    rm -rf "$backup"; cp -a /etc/firewalld "$backup"

    rm -rf /etc/firewalld/zones /etc/firewalld/ipsets /etc/firewalld/services /etc/firewalld/policies
    tar -C / -xzpf "$d/etc-firewalld.tgz"
    rewrite_ips_tree /etc/firewalld
    zone=$(cat "$d/default-zone")

    # never lock ourselves out: keep the SSH port used by this clone open
    systemctl unmask firewalld &>/dev/null || true
    firewall-offline-cmd --zone="$zone" --add-service=ssh >> "$LOG_FILE" 2>&1 || true
    [[ -n ${TARGET_SSH_PORT:-} ]] && firewall-offline-cmd --zone="$zone" --add-port="${TARGET_SSH_PORT}/tcp" >> "$LOG_FILE" 2>&1 || true

    # automatic rollback unless sbc-clone.sh confirms a fresh SSH login works
    systemctl stop "$FW_ROLLBACK_UNIT.timer" &>/dev/null || true
    systemd-run --quiet --unit="$FW_ROLLBACK_UNIT" --on-active=900 /bin/bash -c \
        "rm -rf /etc/firewalld && cp -a '$backup' /etc/firewalld && systemctl disable --now firewalld"
    systemctl enable firewalld >> "$LOG_FILE" 2>&1
    if systemctl is-active --quiet firewalld; then firewall-cmd --reload >> "$LOG_FILE" 2>&1
    else systemctl start firewalld; fi
    log_ok "firewalld aplicado (zona $zone). reversión automática en 15 min salvo confirmación de SSH"
    return 0
}

fw_rich_rule() { firewall-cmd --zone="$1" --query-rich-rule="$2" >/dev/null; }
fw_port_open() { firewall-cmd --zone="$1" --query-port="$2" >/dev/null; }

validate_firewall() {
    local d=$1 z p r
    check "firewalld activo" systemctl is-active --quiet firewalld
    while read -r z p; do
        [[ -n $p ]] && check "Firewall: puerto $p abierto en zona $z" fw_port_open "$z" "$p"
    done < "$d/ports.txt"
    local rules=/tmp/sbc-clone-rich-rules.txt
    cp "$d/rich-rules.txt" "$rules"; rewrite_ips "$rules"
    while read -r r; do
        [[ -n $r ]] && check "Firewall: regla «$r»" fw_rich_rule "$(cat "$d/default-zone")" "$r"
    done < "$rules"
    rm -f "$rules"
    return 0
}
