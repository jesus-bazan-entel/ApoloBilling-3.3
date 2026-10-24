#!/usr/bin/env bash
#===============================================================================
# target-run.sh — runs ON THE NEW SERVER, launched by sbc-clone.sh.
# Applies every agent in order, starts the services, validates them and writes
# REPORTE.txt with the access credentials.
#===============================================================================
set -uo pipefail

BUNDLE=$(cd "$(dirname "$(readlink -f "$0")")" && pwd)
RUN_DIR=$(dirname "$BUNDLE")
export LOG_FILE=$RUN_DIR/clone.log
umask 022

# shellcheck source=lib/common.sh
source "$BUNDLE/lib/common.sh"
# shellcheck source=/dev/null
source "$BUNDLE/clone.env"
build_ip_map
export DEBIAN_FRONTEND=noninteractive

AGENTS=(system packages database kamailio rtpengine freeswitch backend frontend monitoring nginx firewall)
# an error in these leaves nothing worth configuring afterwards
CRITICAL=(system packages database)

for a in "${AGENTS[@]}"; do
    # shellcheck source=/dev/null
    source "$BUNDLE/agents/$a.sh"
done

# lookup "user" in "user:pass user2:pass2"
cred_of() { tr ' ' '\n' <<<"$2" | awk -F: -v u="$1" '$1==u {print substr($0, length(u)+2)}'; }
export -f cred_of

declare -a FAILED_AGENTS=()

echo "${C_BLD}Instalando SBC clonado en $(hostname) — registro: $LOG_FILE${C_RST}"

for a in "${AGENTS[@]}"; do
    log_section "Agente $a"
    # subshell + errexit: any failing command inside apply_* aborts that agent
    ( set -e; "apply_$a" "$BUNDLE/$a" )
    rc=$?
    if (( rc != 0 )); then
        log_err "El agente $a falló (código $rc); detalle en $LOG_FILE"
        FAILED_AGENTS+=("$a")
        for c in "${CRITICAL[@]}"; do
            [[ $a == "$c" ]] && { log_err "Agente crítico: se detiene la clonación."; exit 2; }
        done
    fi
done

log_section "Arranque de servicios"
systemctl daemon-reload
for svc in mariadb postgresql redis-server rtpengine freeswitch kamailio apolo-backend apolo-billing-engine \
           apolo-frontend apolo-monitoring nginx snmpd; do
    systemctl list-unit-files "$svc.service" &>/dev/null || continue
    systemctl enable "$svc" &>/dev/null || true
    if systemctl restart "$svc" &>/dev/null; then log_ok "$svc"; else log_err "$svc no arrancó"; fi
done
sleep 15   # FreeSWITCH profiles and the Rust services need a moment

log_section "Validaciones"
for a in "${AGENTS[@]}"; do
    declare -F "validate_$a" >/dev/null && "validate_$a" "$BUNDLE/$a"
done

#-------------------------------------------------------------------------------
REPORT=$RUN_DIR/REPORTE.txt
FIDELITY=$(awk -v ok="$CHECKS_OK" -v t="$CHECKS_TOTAL" -v w="$CHECKS_WARN" 'BEGIN { if (t==0) print "0.000"; else printf "%.3f", (ok-w)*100/t }')
{
cat <<EOF
===============================================================================
 SBC CLONADO — $(hostname) — $(date '+%F %T %Z')
 Origen: $SRC_HOSTNAME   ·   Ejecución: $TS
===============================================================================

ACCESO AL PORTAL WEB
  URL ............... https://$NEW_MGMT/
  Usuario ........... $ADMIN_USER
  Contraseña ........ $ADMIN_PASS
  (Los demás usuarios del portal conservan la contraseña que tenían en el origen.)

BASES DE DATOS
  PostgreSQL apolo_billing (127.0.0.1:5432)
$(for p in $PG_ROLE_PASSWORDS; do printf '    usuario %-14s contraseña %s\n' "${p%%:*}" "${p#*:}"; done)
  MariaDB kamailio (127.0.0.1:3306)
$(for p in $MY_USER_PASSWORDS; do printf '    usuario %-14s contraseña %s\n' "${p%%:*}" "${p#*:}"; done)
    root              acceso por socket local (sudo mysql)
  Redis (127.0.0.1:6379) sin contraseña, solo escucha en localhost (igual que el origen)

OTROS SECRETOS
  FreeSWITCH ESL (8021) ... $ESL_PASS
  JWT_SECRET .............. $JWT_SECRET
  SECRET_KEY (Python) ..... $APP_SECRET_KEY
  Kamailio api_token ...... $KAM_API_TOKEN
  SIP_ENCRYPTION_KEY ...... igual al origen (cifra las claves SIP guardadas en la base)

RED
  Gestión ............... $NEW_MGMT
  Señalización privada .. $NEW_SIG_INT   (Kamailio ↔ FreeSWITCH, puerto 5080)
  Señalización pública .. $NEW_SIG_EXT   (carriers, 5060 UDP/TCP, 5061 TLS)
  RTP privada ........... $NEW_RTP_INT
  RTP pública ........... $NEW_RTP_EXT   (puertos $NEW_RTP_MIN-$NEW_RTP_MAX)
  Rutas estáticas ....... ${ROUTES:-ninguna}

VALIDACIONES ($CHECKS_OK/$CHECKS_TOTAL correctas, $CHECKS_WARN avisos — fidelidad $FIDELITY%)
$(printf '  %s\n' "${CHECK_LINES[@]}")

AGENTES CON ERRORES: ${FAILED_AGENTS[*]:-ninguno}

PENDIENTES MANUALES
  - Carriers y PBXs: autoriza las nuevas IPs ($NEW_SIG_EXT señalización, $NEW_RTP_EXT RTP)
    en sus ACL antes de mover tráfico.
  - El histórico (CDRs, kamailio.acc, métricas de monitoreo) no se copió.
  - falcon-sensor (CrowdStrike) no se clona: instálalo con la licencia de tu organización.
  - Certificados TLS nuevos autofirmados (portal y Kamailio); reemplázalos si usas certificados de una CA.

Registro completo: $LOG_FILE
===============================================================================
EOF
} > "$REPORT"
chmod 600 "$REPORT"

echo
cat "$REPORT"

(( ${#FAILED_AGENTS[@]} == 0 )) && (( CHECKS_OK == CHECKS_TOTAL )) && exit 0
exit 1
