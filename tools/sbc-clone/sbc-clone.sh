#!/usr/bin/env bash
#===============================================================================
# sbc-clone.sh — clone this SBC (Kamailio + RTPEngine + FreeSWITCH + Apolo)
# onto a fresh Debian 12 server over SSH.
#
# Runs on the SOURCE server as root. It asks for the new server's IPs and SSH
# access, captures everything each component needs (one "agent" per component
# under agents/), ships the bundle to the target, installs and configures it
# there with freshly generated credentials, and validates every service.
#
# What is cloned: packages (same versions), Kamailio, RTPEngine (+kernel
# module), FreeSWITCH, Apolo backend/billing engine/frontend/monitoring,
# PostgreSQL + MariaDB schema and configuration data (no CDR/acc/monitoring
# history), Redis, nginx, firewalld, crons, static routes, system tuning.
#
# Usage:
#   sbc-clone.sh                     interactive
#   sbc-clone.sh --params FILE       reuse answers saved by a previous run
#   sbc-clone.sh --capture-only      build the bundle locally, no SSH
#   sbc-clone.sh --yes               do not ask for final confirmation
#===============================================================================
set -euo pipefail

TOOL_DIR=$(cd "$(dirname "$(readlink -f "$0")")" && pwd)
# shellcheck source=lib/common.sh
source "$TOOL_DIR/lib/common.sh"

AGENTS=(system packages database kamailio rtpengine freeswitch backend frontend monitoring nginx firewall)
for a in "${AGENTS[@]}"; do
    # shellcheck source=/dev/null
    source "$TOOL_DIR/agents/$a.sh"
done

TS=$(date +%Y%m%d-%H%M%S)
WORK_DIR=/var/tmp/sbc-clone/$TS
BUNDLE=$WORK_DIR/bundle
LOG_FILE=$WORK_DIR/capture.log
PARAMS_FILE=""
CAPTURE_ONLY=0
ASSUME_YES=0

while (( $# )); do
    case $1 in
        --params) PARAMS_FILE=$2; shift 2 ;;
        --capture-only) CAPTURE_ONLY=1; shift ;;
        --yes|-y) ASSUME_YES=1; shift ;;
        -h|--help) sed -n '2,25p' "$0"; exit 0 ;;
        *) echo "Opción desconocida: $1" >&2; exit 1 ;;
    esac
done

(( EUID == 0 )) || { echo "Ejecuta este script como root en el SBC de origen." >&2; exit 1; }
umask 077
mkdir -p "$BUNDLE"
chmod 700 /var/tmp/sbc-clone "$WORK_DIR"
# WORK_DIR (700) protects the bundle; inside it keep normal modes, because
# directories are later copied onto the target's system paths
umask 022

#-------------------------------------------------------------------------------
# Source detection: read the roles of each IP from the live configs instead of
# hardcoding them, so the tool keeps working after the source changes.
#-------------------------------------------------------------------------------
kam_substdef() { grep -oP "^#!substdef \"!$1!\K[^!]*" /etc/kamailio/kamailio.cfg | head -1; }

detect_source() {
    log_section "Detectando la configuración del SBC de origen"
    SRC_HOSTNAME=$(hostname)
    SRC_SIG_INT=$(kam_substdef INTERNAL_IP_ADDR)
    SRC_SIG_INT_NET=$(kam_substdef INTERNAL_IP_NET)
    SRC_SIG_EXT=$(kam_substdef EXTERNAL_IP_ADDR)
    SRC_FQDN=$(grep -oP '^SIP_DOMAIN=\K\S+' /etc/kamailio/kamctlrc 2>/dev/null || true)
    local iface
    iface=$(grep -oP '^interface\s*=\s*\K.*' /etc/rtpengine/rtpengine.conf | tr -d ' ')
    SRC_RTP_INT=$(grep -oP 'internal/\K[0-9.]+' <<<"$iface" || true)
    SRC_RTP_EXT=$(grep -oP 'external/\K[0-9.]+' <<<"$iface" || true)
    SRC_RTP_MIN=$(grep -oP '^port-min\s*=\s*\K\d+' /etc/rtpengine/rtpengine.conf)
    SRC_RTP_MAX=$(grep -oP '^port-max\s*=\s*\K\d+' /etc/rtpengine/rtpengine.conf)
    SRC_MGMT=$(openssl x509 -in /etc/nginx/ssl/apolo.crt -noout -subject 2>/dev/null | grep -oP 'CN\s*=\s*\K[0-9.]+' || true)
    if ! is_ipv4 "${SRC_MGMT:-}"; then
        # fallback: the only local IPv4 that has no SIP/RTP role
        SRC_MGMT=$(ip -4 -o addr show scope global | awk '{split($4,a,"/"); print a[1]}' |
            grep -vxF -e "$SRC_SIG_INT" -e "$SRC_SIG_EXT" -e "$SRC_RTP_INT" -e "$SRC_RTP_EXT" |
            grep -v '^172\.17\.' | head -1)
    fi
    SRC_ADMIN_USER=$(cd /tmp && sudo -u postgres psql -d apolo_billing -Atc \
        "select username from usuarios where role='superadmin' and activo order by id limit 1" 2>/dev/null || true)

    log_info "Hostname ................ $SRC_HOSTNAME"
    log_info "Dominio SIP / FQDN ...... ${SRC_FQDN:-—}"
    log_info "Gestión / portal web .... ${SRC_MGMT:-—}"
    log_info "Señalización privada .... $SRC_SIG_INT ($SRC_SIG_INT_NET)"
    log_info "Señalización pública .... $SRC_SIG_EXT"
    log_info "RTP privada ............. $SRC_RTP_INT"
    log_info "RTP pública ............. $SRC_RTP_EXT"
    log_info "Rango RTP ............... $SRC_RTP_MIN-$SRC_RTP_MAX"
    for v in SRC_SIG_INT SRC_SIG_EXT SRC_RTP_INT SRC_RTP_EXT; do
        is_ipv4 "${!v}" || die "No pude detectar $v desde la configuración de origen"
    done
}

#-------------------------------------------------------------------------------
# Prompts
#-------------------------------------------------------------------------------
ask() {
    # ask VAR "pregunta" [default] [validator]
    local var=$1 q=$2 def=${3:-} validator=${4:-} ans
    if [[ -n "${!var:-}" ]]; then return 0; fi   # preloaded from --params
    while true; do
        if [[ -n "$def" ]]; then read -r -p "   $q [$def]: " ans; ans=${ans:-$def}
        else read -r -p "   $q: " ans; fi
        if [[ -z "$ans" ]]; then echo "   (obligatorio)"; continue; fi
        if [[ -n "$validator" ]] && ! "$validator" "$ans"; then echo "   valor inválido"; continue; fi
        printf -v "$var" '%s' "$ans"; return 0
    done
}
ask_secret() {
    local var=$1 q=$2 ans
    [[ -n "${!var:-}" ]] && return 0
    while true; do
        read -r -s -p "   $q: " ans; echo
        [[ -n "$ans" ]] && { printf -v "$var" '%s' "$ans"; return 0; }
    done
}
is_port() { [[ $1 =~ ^[0-9]+$ ]] && (( $1 > 0 && $1 < 65536 )); }
is_hostname() { [[ $1 =~ ^[A-Za-z0-9]([A-Za-z0-9.-]*[A-Za-z0-9])?$ ]]; }
is_ip_or_skip() { [[ $1 == "-" ]] || is_ipv4 "$1"; }

collect_params() {
    if [[ -n "$PARAMS_FILE" ]]; then
        # shellcheck source=/dev/null
        source "$PARAMS_FILE"
        log_info "Respuestas cargadas desde $PARAMS_FILE"
    fi

    log_section "Servidor destino (Debian 12) — acceso SSH"
    if (( ! CAPTURE_ONLY )); then
        ask TARGET_HOST "IP o hostname SSH del nuevo servidor" "" is_hostname
        ask TARGET_PORT "Puerto SSH" "22" is_port
        ask TARGET_USER "Usuario SSH (root o usuario con sudo)" "root"
        ask SSH_AUTH "Autenticación: 'clave' (contraseña) o 'llave' (archivo de llave privada)" "clave"
        if [[ $SSH_AUTH == llave ]]; then
            ask SSH_KEY "Ruta de la llave privada" "/root/.ssh/id_rsa"
            [[ -r $SSH_KEY ]] || die "No puedo leer $SSH_KEY"
        else
            ask_secret SSH_PASS "Contraseña SSH de $TARGET_USER@$TARGET_HOST"
        fi
        if [[ $TARGET_USER != root ]]; then
            ask_secret SUDO_PASS "Contraseña sudo de $TARGET_USER (Enter = la misma de SSH)" || true
            SUDO_PASS=${SUDO_PASS:-${SSH_PASS:-}}
        fi
    fi

    log_section "Identidad del nuevo SBC"
    ask NEW_HOSTNAME "Hostname" "${SRC_HOSTNAME}-clone" is_hostname
    ask NEW_FQDN "Dominio SIP / FQDN" "${SRC_FQDN:-$NEW_HOSTNAME}" is_hostname

    log_section "Direcciones IP del nuevo SBC (deben existir ya en sus interfaces)"
    ask NEW_MGMT "IP de gestión / portal web (origen: ${SRC_MGMT:-—})" "" is_ipv4
    ask NEW_SIG_INT "IP de señalización PRIVADA, lado FreeSWITCH/PBX (origen: $SRC_SIG_INT)" "" is_ipv4
    ask NEW_SIG_EXT "IP de señalización PÚBLICA, lado carrier (origen: $SRC_SIG_EXT)" "" is_ipv4
    ask NEW_RTP_INT "IP de RTP PRIVADA (origen: $SRC_RTP_INT)" "$NEW_SIG_INT" is_ipv4
    ask NEW_RTP_EXT "IP de RTP PÚBLICA (origen: $SRC_RTP_EXT)" "" is_ipv4
    ask NEW_RTP_MIN "Puerto RTP mínimo" "$SRC_RTP_MIN" is_port
    ask NEW_RTP_MAX "Puerto RTP máximo" "$SRC_RTP_MAX" is_port

    log_section "Rutas estáticas hacia carriers y redes remotas"
    collect_routes

    log_section "Portal web"
    ask ADMIN_USER "Usuario administrador del portal (se le asignará una contraseña nueva)" "${SRC_ADMIN_USER:-admin}"
}

collect_routes() {
    # every non-default route with a next hop, as "dest|gw|dev"
    mapfile -t SRC_ROUTES < <(ip -4 route show | awk '$1!="default" && / via / {
        for (i=1;i<=NF;i++) { if ($i=="via") gw=$(i+1); if ($i=="dev") dev=$(i+1) }
        print $1 "|" gw "|" dev }')
    ROUTES=""
    if (( ${#SRC_ROUTES[@]} == 0 )); then log_info "El origen no tiene rutas estáticas."; return; fi

    declare -A own_map=(
        ["$SRC_SIG_INT"]=$NEW_SIG_INT ["$SRC_SIG_EXT"]=$NEW_SIG_EXT
        ["$SRC_RTP_INT"]=$NEW_RTP_INT ["$SRC_RTP_EXT"]=$NEW_RTP_EXT)
    [[ -n ${SRC_MGMT:-} ]] && own_map["$SRC_MGMT"]=$NEW_MGMT

    local -A gw_new=()
    local r dest gw dev var preset
    for r in "${SRC_ROUTES[@]}"; do
        IFS='|' read -r dest gw dev <<<"$r"
        if [[ -n ${own_map[$gw]:-} ]]; then gw_new[$gw]=${own_map[$gw]}; continue; fi
        [[ -n ${gw_new[$gw]:-} ]] && continue
        local dests
        dests=$(printf '%s\n' "${SRC_ROUTES[@]}" | awk -F'|' -v g="$gw" '$2==g {print $1}' | paste -sd' ')
        echo "   Gateway $gw ($dev) en origen → rutas: $dests"
        var="ROUTE_GW_${gw//./_}"
        preset=${!var:-}
        if [[ -n $preset ]]; then gw_new[$gw]=$preset; continue; fi
        ask "$var" "  Gateway equivalente en el nuevo servidor ('-' = no crear estas rutas)" "$gw" is_ip_or_skip
        gw_new[$gw]=${!var}
    done
    for r in "${SRC_ROUTES[@]}"; do
        IFS='|' read -r dest gw dev <<<"$r"
        [[ ${gw_new[$gw]} == "-" ]] && continue
        ROUTES+="$dest>${gw_new[$gw]} "
    done
    ROUTES=${ROUTES% }
    log_info "Rutas a crear: ${ROUTES:-ninguna}"
}

#-------------------------------------------------------------------------------
# Secrets: every credential on the clone is new, except SIP_ENCRYPTION_KEY,
# which encrypts SIP device passwords already stored in the copied database.
#-------------------------------------------------------------------------------
# Old passwords are read from the URLs in the configs (scheme://user:pass@)
# and kamctlrc. Each distinct old password gets ONE new password, so accounts
# that shared a password keep sharing it, and the old value can be replaced
# wherever it is hardcoded (e.g. the monitoring collector's MariaDB login).
discover_old_passwords() {
    declare -gA OLD_PG=() OLD_MY=()
    local -a files=(/etc/kamailio/kamailio.cfg /etc/systemd/system/apolo-*.service)
    mapfile -t -O "${#files[@]}" files < <(find /opt/ApoloBilling -maxdepth 3 -name .env -type f -not -path '*/node_modules/*')
    local scheme user pass
    while read -r scheme user pass; do
        if [[ $scheme == mysql ]]; then OLD_MY[$user]=${OLD_MY[$user]:-$pass}
        else OLD_PG[$user]=${OLD_PG[$user]:-$pass}; fi
    done < <(perl -ne 'while (m{(postgres(?:ql)?|mysql)://([^:/\s"\x27]+):([^@\s"\x27]+)@}g) { print "$1 $2 $3\n" }' "${files[@]}" 2>/dev/null)
    pass=$(grep -oP '^DBRWPW="?\K[^"]+' /etc/kamailio/kamctlrc 2>/dev/null || true)
    [[ -n $pass ]] && OLD_MY[kamailio]=${OLD_MY[kamailio]:-$pass}
    return 0
}

generate_secrets() {
    local r roles users old new
    local -A new_of_old=()
    discover_old_passwords
    # new password for an account, shared with any account that had the same old one
    new_for() {
        local old=$1
        if [[ -n $old && -n ${new_of_old[$old]:-} ]]; then printf '%s' "${new_of_old[$old]}"; return; fi
        local n; n=$(gen_secret 24)
        [[ -n $old ]] && new_of_old[$old]=$n
        printf '%s' "$n"
    }
    roles=$(cd /tmp && sudo -u postgres psql -Atc \
        "select rolname from pg_roles where rolcanlogin and rolname <> 'postgres' order by 1")
    users=$(mysql -N -e "select distinct user from mysql.user where user not in ('root','mariadb.sys','mysql','') order by 1")
    PG_ROLE_PASSWORDS=""; MY_USER_PASSWORDS=""
    for r in $roles; do new=$(new_for "${OLD_PG[$r]:-}"); [[ -n ${OLD_PG[$r]:-} ]] && new_of_old[${OLD_PG[$r]}]=$new; PG_ROLE_PASSWORDS+="$r:$new "; done
    for r in $users; do new=$(new_for "${OLD_MY[$r]:-}"); [[ -n ${OLD_MY[$r]:-} ]] && new_of_old[${OLD_MY[$r]}]=$new; MY_USER_PASSWORDS+="$r:$new "; done
    PG_ROLE_PASSWORDS=${PG_ROLE_PASSWORDS% }
    MY_USER_PASSWORDS=${MY_USER_PASSWORDS% }
    # old→new pairs, base64-encoded so any character survives; short old
    # values are skipped to avoid replacing unrelated text
    SECRET_MAP=""
    for old in "${!new_of_old[@]}"; do
        (( ${#old} >= 8 )) || { log_warn "Contraseña antigua demasiado corta para reemplazarla en código (se omite)"; continue; }
        SECRET_MAP+="$(printf '%s' "$old" | base64 -w0):$(printf '%s' "${new_of_old[$old]}" | base64 -w0) "
    done
    SECRET_MAP=${SECRET_MAP% }
    ADMIN_PASS=$(gen_secret 16)
    JWT_SECRET=$(gen_secret 64)
    APP_SECRET_KEY=$(gen_secret 64)
    ESL_PASS=$(gen_secret 20)
    KAM_API_TOKEN=$(gen_secret 64)
}

write_clone_env() {
    local f=$BUNDLE/clone.env
    {
        echo "# generado por sbc-clone.sh $TS en $SRC_HOSTNAME — contiene credenciales"
        for v in TS SRC_HOSTNAME SRC_FQDN SRC_MGMT SRC_SIG_INT SRC_SIG_INT_NET SRC_SIG_EXT SRC_RTP_INT SRC_RTP_EXT \
                 SRC_RTP_MIN SRC_RTP_MAX NEW_HOSTNAME NEW_FQDN NEW_MGMT NEW_SIG_INT NEW_SIG_EXT NEW_RTP_INT \
                 NEW_RTP_EXT NEW_RTP_MIN NEW_RTP_MAX ROUTES ADMIN_USER ADMIN_PASS PG_ROLE_PASSWORDS \
                 MY_USER_PASSWORDS SECRET_MAP JWT_SECRET APP_SECRET_KEY ESL_PASS KAM_API_TOKEN TARGET_SSH_PORT; do
            printf '%s=%q\n' "$v" "${!v:-}"
        done
    } > "$f"
    chmod 600 "$f"

    # answers without secrets, reusable with --params
    {
        for v in TARGET_HOST TARGET_PORT TARGET_USER SSH_AUTH SSH_KEY NEW_HOSTNAME NEW_FQDN NEW_MGMT NEW_SIG_INT \
                 NEW_SIG_EXT NEW_RTP_INT NEW_RTP_EXT NEW_RTP_MIN NEW_RTP_MAX ADMIN_USER; do
            [[ -n ${!v:-} ]] && printf '%s=%q\n' "$v" "${!v}"
        done
        compgen -v ROUTE_GW_ | while read -r v; do printf '%s=%q\n' "$v" "${!v}"; done
    } > "$WORK_DIR/params.env"
}


#-------------------------------------------------------------------------------
# SSH plumbing
#-------------------------------------------------------------------------------
SSH_CTL=""
ssh_setup() {
    SSH_CTL=$WORK_DIR/ssh-ctl
    # ssh keeps the FIRST value given for an option, so the multiplexing
    # options live apart from the base ones (see the firewall confirmation)
    SSH_BASE_OPTS=(-p "$TARGET_PORT" -o StrictHostKeyChecking=accept-new -o ServerAliveInterval=30)
    if [[ $SSH_AUTH == llave ]]; then
        SSH_BASE_OPTS+=(-i "$SSH_KEY" -o BatchMode=yes); SSH_WRAP=()
    else
        command -v sshpass >/dev/null || { log_info "Instalando sshpass (necesario para SSH con contraseña)"; apt-get install -y -qq sshpass >/dev/null; }
        export SSHPASS=$SSH_PASS
        SSH_WRAP=(sshpass -e)
        SSH_BASE_OPTS+=(-o PubkeyAuthentication=no)
    fi
    SSH_OPTS=("${SSH_BASE_OPTS[@]}" -o ControlMaster=auto -o "ControlPath=$SSH_CTL" -o ControlPersist=60m)
    SCP_OPTS=(-P "$TARGET_PORT" -o StrictHostKeyChecking=accept-new -o "ControlPath=$SSH_CTL")
}
rssh() { "${SSH_WRAP[@]}" ssh "${SSH_OPTS[@]}" "$TARGET_USER@$TARGET_HOST" "$@"; }
rscp() { "${SSH_WRAP[@]}" scp -q "${SCP_OPTS[@]}" "$@"; }
# run a command as root on the target
rroot() {
    if [[ $TARGET_USER == root ]]; then rssh "$@"
    else printf '%s\n' "$SUDO_PASS" | rssh "sudo -S -p '' bash -c $(printf '%q' "$*")"; fi
}

preflight_target() {
    log_section "Validando el servidor destino"
    rssh true || die "No pude conectarme por SSH a $TARGET_USER@$TARGET_HOST:$TARGET_PORT"
    log_ok "Conexión SSH"
    rroot true || die "El usuario $TARGET_USER no puede ejecutar comandos como root"
    log_ok "Privilegios de root"

    local os arch ips mem disk
    os=$(rssh '. /etc/os-release; echo "$ID $VERSION_ID"')
    [[ $os == "debian 12" ]] || die "El destino debe ser Debian 12 (encontré: $os)"
    log_ok "Sistema operativo: Debian 12"
    arch=$(rssh 'uname -m')
    [[ $arch == "$(uname -m)" ]] || die "Arquitectura distinta: origen $(uname -m), destino $arch"
    log_ok "Arquitectura: $arch"

    ips=$(rssh "ip -4 -o addr show | awk '{split(\$4,a,\"/\"); print a[1]}'")
    local v missing=0
    for v in NEW_MGMT NEW_SIG_INT NEW_SIG_EXT NEW_RTP_INT NEW_RTP_EXT; do
        if grep -qxF "${!v}" <<<"$ips"; then log_ok "IP ${!v} presente en el destino ($v)"
        else log_err "La IP ${!v} ($v) no está configurada en ninguna interfaz del destino"; missing=1; fi
    done
    (( missing == 0 )) || die "Configura las IPs en el servidor destino y vuelve a ejecutar (usa --params $WORK_DIR/params.env)"

    mem=$(rssh "awk '/MemTotal/ {print int(\$2/1024)}' /proc/meminfo")
    disk=$(rssh "df -Pm / | awk 'NR==2 {print \$4}'")
    (( mem >= 3500 )) && log_ok "Memoria: ${mem} MB" || log_warn "Memoria baja en destino: ${mem} MB (origen: $(awk '/MemTotal/ {print int($2/1024)}' /proc/meminfo) MB)"
    (( disk >= 15000 )) && log_ok "Disco libre: ${disk} MB" || die "Espacio insuficiente en / del destino: ${disk} MB (mínimo 15000 MB)"
    rssh 'timeout 10 bash -c "</dev/tcp/deb.debian.org/443"' 2>/dev/null && log_ok "Acceso a Internet (repositorios)" ||
        die "El destino no llega a deb.debian.org:443; necesita Internet para instalar paquetes"
    TARGET_SSH_PORT=$TARGET_PORT
}

#-------------------------------------------------------------------------------
main() {
    LOG_FILE=$WORK_DIR/capture.log
    echo "${C_BLD}Clonación de SBC — origen: $(hostname)${C_RST}"
    echo "Registro: $LOG_FILE"
    detect_source
    collect_params
    build_ip_map
    generate_secrets

    if (( ! CAPTURE_ONLY )); then
        ssh_setup
        preflight_target
    fi
    write_clone_env

    log_section "Resumen"
    cat <<EOF
   Destino ............ ${TARGET_USER:-—}@${TARGET_HOST:-—}:${TARGET_PORT:-—}
   Hostname / FQDN .... $NEW_HOSTNAME / $NEW_FQDN
   Portal web ......... https://$NEW_MGMT/
   Señalización ....... privada $NEW_SIG_INT · pública $NEW_SIG_EXT
   RTP ................ privada $NEW_RTP_INT · pública $NEW_RTP_EXT · puertos $NEW_RTP_MIN-$NEW_RTP_MAX
   Rutas .............. ${ROUTES:-ninguna}
   Datos .............. estructura completa + configuración (sin CDRs, acc ni métricas históricas)
   Credenciales ....... todas nuevas (se muestran al final)
EOF
    if (( ! ASSUME_YES )); then
        local ok; read -r -p "   ¿Continuar con la clonación? [s/N]: " ok
        [[ $ok =~ ^[sSyY]$ ]] || { echo "Cancelado. Respuestas guardadas en $WORK_DIR/params.env"; exit 0; }
    fi

    log_section "Captura del origen (agentes)"
    local a
    for a in "${AGENTS[@]}"; do
        mkdir -p "$BUNDLE/$a"
        log_info "agente $a"
        "capture_$a" "$BUNDLE/$a"
    done
    cp -a "$TOOL_DIR/lib" "$TOOL_DIR/agents" "$TOOL_DIR/target-run.sh" "$BUNDLE/"
    tar -C "$WORK_DIR" -czf "$WORK_DIR/bundle.tgz" bundle
    log_ok "Paquete listo: $WORK_DIR/bundle.tgz ($(du -h "$WORK_DIR/bundle.tgz" | cut -f1))"

    if (( CAPTURE_ONLY )); then
        log_info "--capture-only: no se conectó al destino."
        exit 0
    fi

    log_section "Transfiriendo al destino"
    rscp "$WORK_DIR/bundle.tgz" "$TARGET_USER@$TARGET_HOST:/tmp/sbc-clone-$TS.tgz"
    rroot "mkdir -p /root/sbc-clone/$TS && chmod 700 /root/sbc-clone && tar -C /root/sbc-clone/$TS -xzf /tmp/sbc-clone-$TS.tgz && rm -f /tmp/sbc-clone-$TS.tgz"
    log_ok "Copiado a /root/sbc-clone/$TS"

    log_section "Instalación en el destino (puede tardar 20-40 minutos)"
    set +e
    rroot "bash /root/sbc-clone/$TS/bundle/target-run.sh"
    local rc=$?
    set -e

    # firewall safety net: a NEW connection (not the multiplexed one) must work
    log_section "Confirmando acceso SSH tras aplicar el firewall"
    if "${SSH_WRAP[@]}" ssh "${SSH_BASE_OPTS[@]}" -o ControlPath=none -o ConnectTimeout=15 \
            "$TARGET_USER@$TARGET_HOST" true 2>/dev/null; then
        rroot "systemctl stop sbc-firewall-rollback.timer 2>/dev/null; true"
        log_ok "SSH accesible con el firewall nuevo; reversión cancelada"
    else
        log_err "No se pudo abrir una conexión SSH nueva: en 15 min el destino restaurará su firewall anterior y detendrá firewalld"
    fi

    rroot "cat /root/sbc-clone/$TS/REPORTE.txt" > "$WORK_DIR/REPORTE.txt" 2>/dev/null || true
    chmod 600 "$WORK_DIR/REPORTE.txt" 2>/dev/null || true
    [[ -n $SSH_CTL ]] && ssh -o "ControlPath=$SSH_CTL" -O exit "$TARGET_HOST" 2>/dev/null || true

    echo
    if (( rc == 0 )); then
        echo "${C_GRN}${C_BLD}Clonación completada.${C_RST} Reporte con credenciales: $WORK_DIR/REPORTE.txt (y /root/sbc-clone/$TS/REPORTE.txt en el destino)"
    else
        echo "${C_RED}${C_BLD}La clonación terminó con errores (código $rc).${C_RST} Revisa /root/sbc-clone/$TS/clone.log en el destino."
        echo "Puedes reintentar: $0 --params $WORK_DIR/params.env"
    fi
    exit $rc
}

main
