#!/usr/bin/env bash
# Agente: paquetes — repositorios APT, mismas versiones de paquetes que el
# origen, .deb compilados localmente (RTPEngine), librerías sin paquete
# (libks, libjwt, libstirshaken, módulo stirshaken de Kamailio), pip y npm.

# not cloned: hardware/boot specific, licensed agents, unused stacks
PKG_EXCLUDE_RE='^(linux-image|linux-headers|grub|shim|efibootmgr|os-prober|intel-microcode|amd64-microcode|firmware-|discover|laptop-detect|installation-report|tasksel|falcon-sensor|docker|containerd|opensips|cloud-|qemu-guest-agent|open-vm-tools)'
# components whose exact version is checked after install
PKG_KEY_RE='^(kamailio|ngcp-rtpengine-daemon|freeswitch|freeswitch-meta-all|postgresql-15|mariadb-server|redis-server|nginx|nodejs|firewalld)$'
LIB_GLOBS=(/usr/lib/libks* /usr/lib/libjwt* /usr/lib/libstirshaken*)

capture_packages() {
    local d=$1 f p v
    mkdir -p "$d/apt/sources.list.d" "$d/apt/keyrings" "$d/apt/usr-keyrings" "$d/apt/preferences.d" "$d/debs" "$d/libs"

    cp -a /etc/apt/sources.list "$d/apt/" 2>/dev/null || true
    for f in /etc/apt/sources.list.d/*; do
        [[ -e $f ]] || continue
        grep -qiE 'docker|opensips' "$f" && continue
        cp -a "$f" "$d/apt/sources.list.d/"
    done
    cp -a /etc/apt/keyrings/. "$d/apt/keyrings/" 2>/dev/null || true
    cp -a /usr/share/keyrings/. "$d/apt/usr-keyrings/" 2>/dev/null || true
    cp -a /etc/apt/preferences.d/. "$d/apt/preferences.d/" 2>/dev/null || true
    # legacy keyring (Kamailio repo has no signed-by); -L: the PGDG key is a
    # symlink into postgresql-common, which the target does not have yet
    mkdir -p "$d/apt/trusted.gpg.d"
    [[ -f /etc/apt/trusted.gpg ]] && cp -a /etc/apt/trusted.gpg "$d/apt/"
    cp -aL /etc/apt/trusted.gpg.d/. "$d/apt/trusted.gpg.d/" 2>/dev/null || true
    # FreeSWITCH (SignalWire) repo credentials
    [[ -f /etc/apt/auth.conf ]] && cp -a /etc/apt/auth.conf "$d/apt/"
    [[ -d /etc/apt/auth.conf.d ]] && cp -a /etc/apt/auth.conf.d "$d/apt/"

    # manually installed packages with their exact versions
    apt-mark showmanual | grep -vE "$PKG_EXCLUDE_RE" | while read -r p; do
        v=$(dpkg-query -W -f='${Version}' "$p" 2>/dev/null) && echo "$p=$v"
    done > "$d/manual.txt"
    dpkg-query -W -f='${db:Status-Abbrev}|${Package}=${Version}\n' |
        awk -F'|' -v re="$PKG_KEY_RE" '$1 ~ /^ii/ { split($2, a, "="); if (a[1] ~ re) print $2 }' > "$d/key-versions.txt"

    # packages that exist only locally (built from source): ship their .deb
    while IFS='=' read -r p v; do
        apt-cache policy "$p" 2>/dev/null | grep -A1 -F " $v " | tail -1 | grep -q '/var/lib/dpkg/status' || continue
        f=$(find /usr/local/src /var/cache/apt/archives -maxdepth 2 -name "${p}_${v//+/?}_*.deb" 2>/dev/null | head -1)
        if [[ -n $f ]]; then cp -a "$f" "$d/debs/"; else echo "$p=$v" >> "$d/missing-debs.txt"; fi
    done < "$d/manual.txt"

    # libraries and Kamailio modules installed by hand (no owning package)
    local kmods
    kmods=$(dirname "$(find /usr/lib -path '*kamailio/modules/tm.so' | head -1)")
    for f in "${LIB_GLOBS[@]}" "$kmods"/*.so; do
        [[ -e $f ]] || continue
        dpkg -S "$f" &>/dev/null && continue
        echo "$f" >> "$d/libs.txt"
    done
    [[ -s $d/libs.txt ]] && tar -C / -czf "$d/libs/unpackaged.tgz" $(sed 's#^/##' "$d/libs.txt")

    pip3 freeze --path /usr/local/lib/python3.*/dist-packages 2>/dev/null > "$d/pip.txt" || true
    npm ls -g --depth=0 --parseable 2>/dev/null | tail -n +2 | while read -r f; do
        p=$(basename "$f"); [[ $p == npm || $p == corepack ]] && continue
        v=$(node -p "require('$f/package.json').version" 2>/dev/null) && echo "$p@$v"
    done > "$d/npm.txt" || true
    return 0
}

apply_packages() {
    local d=$1 line

    cp -a "$d/apt/sources.list" /etc/apt/sources.list 2>/dev/null || true
    install_tree "$d/apt/sources.list.d" /etc/apt/sources.list.d
    mkdir -p /etc/apt/keyrings
    install_tree "$d/apt/keyrings" /etc/apt/keyrings
    install_tree "$d/apt/usr-keyrings" /usr/share/keyrings
    install_tree "$d/apt/preferences.d" /etc/apt/preferences.d 2>/dev/null || true
    [[ -f $d/apt/trusted.gpg ]] && cp "$d/apt/trusted.gpg" /etc/apt/trusted.gpg
    install_tree "$d/apt/trusted.gpg.d" /etc/apt/trusted.gpg.d
    [[ -f $d/apt/auth.conf ]] && install -m 600 "$d/apt/auth.conf" /etc/apt/auth.conf
    [[ -d $d/apt/auth.conf.d ]] && cp -a "$d/apt/auth.conf.d" /etc/apt/
    apt-get update >> "$LOG_FILE" 2>&1
    log_ok "Repositorios APT"

    apt-get install -y -qq ca-certificates curl gnupg rsync perl python3-argon2 dkms >> "$LOG_FILE" 2>&1
    log_ok "Herramientas base"
    # needed to build the RTPEngine kernel module; without them RTPEngine
    # still runs, forwarding media in userspace
    if apt-get install -y -qq "linux-headers-$(uname -r)" >> "$LOG_FILE" 2>&1; then
        log_ok "Headers del kernel $(uname -r)"
    else
        log_warn "No hay headers para el kernel $(uname -r): actualiza el kernel del destino y reinicia para tener reenvío RTP en kernel"
    fi

    # local-only packages are installed from their .deb further down
    local local_re=""
    for line in "$d"/debs/*.deb; do
        [[ -e $line ]] && local_re+="|$(dpkg-deb -f "$line" Package)"
    done
    local -a want=()
    while read -r line; do
        [[ -n $local_re && ${line%%=*} =~ ^(${local_re#|})$ ]] && continue
        want+=("$line")
    done < "$d/manual.txt"

    # firewalld would start with stock rules mid-install; the firewall agent
    # enables it at the end with the cloned rules
    systemctl mask firewalld >> "$LOG_FILE" 2>&1 || true

    log_info "Instalando ${#want[@]} paquetes con las versiones del origen (tarda varios minutos)…"
    if ! apt-get install -y -q --allow-downgrades -o Dpkg::Options::=--force-confdef -o Dpkg::Options::=--force-confold \
            "${want[@]}" >> "$LOG_FILE" 2>&1; then
        log_warn "La instalación en bloque falló; reintentando paquete por paquete"
        local p fails=0
        for line in "${want[@]}"; do
            p=${line%%=*}
            dpkg-query -W -f='${Status}' "$p" 2>/dev/null | grep -q 'install ok installed' && continue
            if apt-get install -y -q --allow-downgrades -o Dpkg::Options::=--force-confold "$line" >> "$LOG_FILE" 2>&1; then continue; fi
            if apt-get install -y -q -o Dpkg::Options::=--force-confold "$p" >> "$LOG_FILE" 2>&1; then
                log_warn "$p: versión del origen no disponible, instalada $(dpkg-query -W -f='${Version}' "$p")"
            else
                log_err "$p: no se pudo instalar"; fails=$((fails + 1))
            fi
        done
        (( fails == 0 )) || log_warn "$fails paquetes no se instalaron (ver $LOG_FILE)"
    fi
    log_ok "Paquetes del sistema"

    # .deb built on the source: the daemon is required, the kernel module
    # (DKMS) is best-effort because it depends on this host's kernel headers
    local -a debs=() dkms_debs=()
    for line in "$d"/debs/*.deb; do
        [[ -e $line ]] || continue
        if [[ $line == *-dkms_* ]]; then dkms_debs+=("$line"); else debs+=("$line"); fi
    done
    if (( ${#debs[@]} )); then
        apt-get install -y -q -o Dpkg::Options::=--force-confold "${debs[@]}" >> "$LOG_FILE" 2>&1
        log_ok "Paquetes compilados en el origen: $(printf '%s\n' "${debs[@]##*/}" | sed 's/_.*//' | paste -sd' ')"
    fi
    if (( ${#dkms_debs[@]} )); then
        if apt-get install -y -q -o Dpkg::Options::=--force-confold "${dkms_debs[@]}" >> "$LOG_FILE" 2>&1; then
            log_ok "Módulo de kernel RTPEngine (DKMS) compilado"
        else
            log_warn "No se pudo compilar el módulo de kernel de RTPEngine (DKMS); funcionará en modo usuario"
            dpkg --configure -a >> "$LOG_FILE" 2>&1 || true
        fi
    fi
    [[ -s $d/missing-debs.txt ]] && log_warn "Sin .deb en el origen para: $(paste -sd' ' "$d/missing-debs.txt")"

    if [[ -f $d/libs/unpackaged.tgz ]]; then
        tar -C / -xzf "$d/libs/unpackaged.tgz"
        ldconfig
        log_ok "Librerías y módulos sin paquete: $(wc -l < "$d/libs.txt") archivos"
    fi

    if [[ -s $d/pip.txt ]]; then
        if ! pip3 install -q --break-system-packages -r "$d/pip.txt" >> "$LOG_FILE" 2>&1; then
            log_warn "pip en bloque falló; reintentando uno por uno"
            while read -r line; do
                pip3 install -q --break-system-packages "$line" >> "$LOG_FILE" 2>&1 ||
                    pip3 install -q --break-system-packages "${line%%==*}" >> "$LOG_FILE" 2>&1 ||
                    log_warn "pip: no se pudo instalar $line"
            done < "$d/pip.txt"
        fi
        log_ok "Paquetes Python ($(wc -l < "$d/pip.txt"))"
    fi
    if [[ -s $d/npm.txt ]] && command -v npm >/dev/null; then
        # shellcheck disable=SC2046
        if npm install -g --silent $(cat "$d/npm.txt") >> "$LOG_FILE" 2>&1; then
            log_ok "npm global: $(paste -sd' ' "$d/npm.txt")"
        else
            log_warn "npm global falló ($(paste -sd' ' "$d/npm.txt")); el portal necesita 'serve'"
        fi
    fi

    # packages start their services with stock configs (FreeSWITCH would grab
    # 5060 on every IP); stop them until their agents configure them
    local s
    for s in kamailio freeswitch rtpengine nginx; do
        systemctl stop "$s" &>/dev/null || true
    done
    return 0
}

pkg_version_matches() { [[ $(dpkg-query -W -f='${Version}' "$1" 2>/dev/null) == "$2" ]]; }

validate_packages() {
    local d=$1 line
    while IFS='=' read -r p v; do
        check "Paquete $p versión $v" pkg_version_matches "$p" "$v"
    done < "$d/key-versions.txt"
    local total ok=0
    total=$(wc -l < "$d/manual.txt")
    while IFS='=' read -r p v; do
        dpkg-query -W -f='${Status}' "$p" 2>/dev/null | grep -q 'install ok installed' && ok=$((ok + 1))
    done < "$d/manual.txt"
    check_warn "Paquetes del origen instalados: $ok de $total" test "$ok" -eq "$total"
    return 0
}
