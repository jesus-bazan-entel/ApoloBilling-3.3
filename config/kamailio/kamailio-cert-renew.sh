#!/usr/bin/env bash
# Renew the self-signed TLS cert Kamailio uses (tls.cfg, http_async_client).
# Replaces dSIPRouter's daily `dsiprouter renewsslcert` cron now that Kamailio
# runs standalone. Only regenerates when the cert expires within 30 days.
set -euo pipefail

CERT=/etc/dsiprouter/certs/dsiprouter-cert.pem
KEY=/etc/dsiprouter/certs/dsiprouter-key.pem
DAYS_LEFT_MIN=30

if openssl x509 -in "$CERT" -noout -checkend $((DAYS_LEFT_MIN * 86400)) &>/dev/null; then
    exit 0
fi

ISSUER_O=$(openssl x509 -in "$CERT" -noout -nameopt multiline -issuer 2>/dev/null |
    awk -F' = ' '/organizationName/ {print $2}')
if [[ "$ISSUER_O" != "dSIPRouter" ]]; then
    logger -t kamailio-cert-renew "cert issuer is '$ISSUER_O', not self-signed; renew it manually"
    exit 1
fi

CN=$(openssl x509 -in "$CERT" -noout -nameopt multiline -subject |
    awk -F' = ' '/commonName/ {print $2}')

cp -a "$CERT" "$CERT.bak-$(date +%Y%m%d)"
cp -a "$KEY" "$KEY.bak-$(date +%Y%m%d)"

openssl req -new -newkey rsa:4096 -x509 -sha256 -days 365 -nodes \
    -out "$CERT" -keyout "$KEY" \
    -subj "/C=US/ST=MI/L=Detroit/O=dSIPRouter/CN=${CN:-$(hostname)}" &>/dev/null

chown dsiprouter:kamailio "$CERT" "$KEY"
chmod 640 "$CERT" "$KEY"

kamcmd tls.reload
logger -t kamailio-cert-renew "renewed self-signed cert for CN=${CN:-$(hostname)}"
