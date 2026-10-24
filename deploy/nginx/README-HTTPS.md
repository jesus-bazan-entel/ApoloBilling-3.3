# HTTPS Dashboard - https://10.11.3.36/

Configuracion activa: `/etc/nginx/sites-available/apolobilling.conf`
- Puerto 80 redirige a HTTPS: `return 301 https://$host$request_uri`
- Puerto 443 con SSL: cert autofirmado en `/etc/nginx/ssl/apolo.{crt,key}`
- SAN del cert: 10.11.3.36, 10.118.0.6, 10.124.193.41, 127.0.0.1, localhost
- Validez: 825 dias (ver con `openssl x509 -in /etc/nginx/ssl/apolo.crt -noout -dates`)

## Regenerar certificado
```
sudo openssl req -x509 -nodes -days 825 -newkey rsa:2048 \
 -keyout /etc/nginx/ssl/apolo.key -out /etc/nginx/ssl/apolo.crt \
 -subj "/CN=10.11.3.36" \
 -addext "subjectAltName=IP:10.11.3.36,IP:10.118.0.6,IP:10.124.193.41,IP:127.0.0.1,DNS:localhost"
sudo nginx -t && sudo systemctl reload nginx
```

## Verificar
```
curl -s -o /dev/null -w "%{http_code} -> %{redirect_url}\n" http://10.11.3.36/
curl -sk -o /dev/null -w "%{http_code}\n" https://10.11.3.36/
curl -sk https://10.11.3.36/api/v1/health
```

Nota: cert autofirmado -> el navegador pedira aceptar riesgo. Para produccion con dominio usar Let's Encrypt con certbot.
