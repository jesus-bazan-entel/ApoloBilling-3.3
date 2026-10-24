#!/bin/sh
# Apolo post-start self-check for rtpengine kernel forwarding.
# Called from rtpengine.service.d/override.conf (ExecStartPost, ice-prefixed
# `+-` so it runs as root and can never fail the service).
# The packaged ngcp-rtpengine-iptables-setup hides all iptables errors
# (2>/dev/null), so a missing kernel rule would otherwise go unnoticed while
# the daemon silently falls back to userspace (no-fallback = false).
# This only logs to stdout (-> journal). Watch with:
#   journalctl -u rtpengine --since "5 min ago" | grep -i "WARNING\|kernel forwarding\|userspace"
# Always exits 0.
CONFIG_FILE="${CONFIG_FILE:-/etc/rtpengine/rtpengine.conf}"

T="$(/usr/libexec/rtpengine/rtpengine-get-table --config-file="$CONFIG_FILE" 2>/dev/null)"

case "$T" in
  ''|*[!0-9]*)
    echo "rtpengine post-start: table no numerica ('$T'), modo userspace asumido"
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
      if ! iptables-legacy -C rtpengine -p udp -j RTPENGINE --id "$T" 2>/dev/null; then
        echo "WARNING rtpengine: falta regla legacy 'rtpengine -p udp -j RTPENGINE --id $T' (revisar backend iptables-legacy)"
        F=1
      fi
      if [ "$F" = 0 ]; then
        echo "rtpengine post-start: kernel forwarding OK (table $T)"
      fi
    else
      echo "rtpengine post-start: modo userspace (table=$T)"
    fi
    ;;
esac

exit 0
