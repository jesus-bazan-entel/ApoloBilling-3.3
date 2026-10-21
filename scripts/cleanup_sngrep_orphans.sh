#!/usr/bin/env bash
#
# cleanup_sngrep_orphans.sh
#
# Mata procesos sngrep huerfanos que quedaron sin terminal de control
# (p.ej. por una sesion SSH cortada abruptamente mientras capturaba),
# sin tocar sesiones sngrep activas o capturas en background recien
# iniciadas intencionalmente.
#
# Criterios para considerar un proceso "huerfano" (deben cumplirse TODOS):
#   1. tty == "?"          -> sin terminal de control asociada
#   2. ppid == 1           -> reparentado a init (su padre original ya no existe)
#   3. edad >= MIN_AGE_SECONDS -> no es una captura recien lanzada en background
#
# Uso en crontab (cada 15 minutos, log a archivo propio):
#   */15 * * * * /opt/ApoloBilling/scripts/cleanup_sngrep_orphans.sh >> /var/log/sngrep_cleanup.log 2>&1
#
# Variables de entorno opcionales:
#   MIN_AGE_SECONDS   Periodo de gracia antes de matar un huerfano (default 600 = 10 min)

set -uo pipefail

MIN_AGE_SECONDS="${MIN_AGE_SECONDS:-600}"
LOG_TAG="cleanup_sngrep_orphans"

log() {
    echo "$(date '+%Y-%m-%d %H:%M:%S') [$LOG_TAG] $*"
}

ps -o pid,ppid,tty,etimes,user,comm -C sngrep --no-headers 2>/dev/null | \
while read -r pid ppid tty etimes user comm; do
    [[ -z "$pid" ]] && continue

    # Criterio 1 y 2: sin tty y reparentado a init
    if [[ "$tty" != "?" || "$ppid" != "1" ]]; then
        continue
    fi

    # Criterio 3: periodo de gracia por edad
    if (( etimes < MIN_AGE_SECONDS )); then
        log "Omitido PID=$pid user=$user: huerfano pero con solo ${etimes}s de antiguedad (< ${MIN_AGE_SECONDS}s de gracia)"
        continue
    fi

    log "Terminando sngrep huerfano: PID=$pid user=$user ppid=$ppid tty=$tty edad=${etimes}s"
    kill -TERM "$pid" 2>/dev/null
    sleep 1
    if kill -0 "$pid" 2>/dev/null; then
        log "PID=$pid no respondio a SIGTERM, forzando SIGKILL"
        kill -KILL "$pid" 2>/dev/null
    fi
done

exit 0
