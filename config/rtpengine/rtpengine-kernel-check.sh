#!/bin/sh
# Apolo self-check (and self-heal) for rtpengine kernel forwarding.
# Called from rtpengine.service.d/override.conf (ExecStartPost, ice-prefixed
# `+-` so it runs as root and can never fail the service) and every 5 min by
# rtpengine-kernel-check.timer with --quiet (logs only when something is wrong).
# The packaged ngcp-rtpengine-iptables-setup hides all iptables errors
# (2>/dev/null) and calls iptables without -w, so a missing kernel rule would
# otherwise go unnoticed while the daemon silently falls back to userspace
# (no-fallback = false). That happened on the 2026-11-26 18:55 boot (firewalld
# and docker start at the same time as rtpengine): the rule was missing and
# all media ran in userspace until it was re-added by hand.
# When the iptables chain/rule is missing it re-runs the setup (idempotent,
# does not touch the daemon or live calls; up to 3 tries), then re-checks.
# Watch with:
#   journalctl -t rtpengine-kernel-check --since "1 hour ago"
# Always exits 0.
CONFIG_FILE="${CONFIG_FILE:-/etc/rtpengine/rtpengine.conf}"
QUIET=0
[ "$1" = "--quiet" ] && QUIET=1

T="$(/usr/libexec/rtpengine/rtpengine-get-table --config-file="$CONFIG_FILE" 2>/dev/null)"

rule_ok() {
  iptables-legacy -w 5 -C rtpengine -p udp -j RTPENGINE --id "$T" 2>/dev/null \
    && iptables-legacy -w 5 -C INPUT -p udp -j rtpengine 2>/dev/null
}

case "$T" in
  ''|*[!0-9]*)
    [ "$QUIET" = 1 ] || echo "rtpengine post-start: table no numerica ('$T'), modo userspace asumido"
    ;;
  *)
    if [ "$T" -ge 0 ]; then
      F=0
      if ! lsmod 2>/dev/null | grep -q xt_RTPENGINE; then
        echo "WARNING rtpengine: table=$T pero xt_RTPENGINE no cargado (forwarding en kernel OFF, fallback a userspace)"
        F=1
      fi
      if [ ! -d "/proc/rtpengine/$T" ]; then
        echo "WARNING rtpengine: falta /proc/rtpengine/$T"
        F=1
      fi
      if ! rule_ok; then
        echo "WARNING rtpengine: falta regla legacy 'rtpengine -p udp -j RTPENGINE --id $T' o el salto INPUT -> rtpengine; reaplicando ngcp-rtpengine-iptables-setup"
        for _ in 1 2 3; do
          /usr/sbin/ngcp-rtpengine-iptables-setup start >/dev/null 2>&1
          rule_ok && break
          sleep 2
        done
        if rule_ok; then
          echo "rtpengine: regla de kernel restaurada (table $T)"
        else
          echo "WARNING rtpengine: no se pudo restaurar la regla (revisar backend iptables-legacy / xtables lock)"
          F=1
        fi
      fi
      if [ "$F" = 0 ] && [ "$QUIET" = 0 ]; then
        echo "rtpengine post-start: kernel forwarding OK (table $T)"
      fi
    else
      [ "$QUIET" = 1 ] || echo "rtpengine post-start: modo userspace (table=$T)"
    fi
    ;;
esac

exit 0
