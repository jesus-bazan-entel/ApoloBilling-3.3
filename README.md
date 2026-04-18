# ApoloBilling
# ApoloBilling-3.3

● Resumen final — arquitectura nueva activa y persistida en el repo
  
Nomenclatura canónica de IPs en el installer (6 parámetros)

<img width="987" height="320" alt="image" src="https://github.com/user-attachments/assets/0027a8c2-a5c4-4342-b46f-794dbe831eb7" />

    Servicios — todos active

  apolo-backend / apolo-billing-engine / apolo-frontend   active
  kamailio / freeswitch / rtpengine                       active
  dsiprouter                                              masked

  Gateway FS kamailio reporta Status UP con ping activo hacia 10.118.0.6:5080.

  Archivos modificados (repo)

  - config/freeswitch/sip_profiles/internal.xml → puerto 5060 + force-register-domain
  - config/freeswitch/sip_profiles/external.xml → context from-kamailio, ACL kamailio-acl
  - config/freeswitch/sip_profiles/external/gateways/kamailio.xml → proxy a 10.118.0.6:5080
  - config/freeswitch/dialplan/from-pbx.xml → reescrito (cliente → gateway kamailio)
  - config/freeswitch/dialplan/from-kamailio.xml → nuevo (inbound de Kamailio → extensión)
  - config/freeswitch/dialplan/to-kamailio.xml → eliminado
  - config/freeswitch/autoload_configs/acl.conf.xml → ACL limpio LAN only
  - config/freeswitch/sip_profiles/external/gateways/{fibertel,pbx-fibertel,kamailio-internal}.xml → eliminados
  - config/freeswitch/vars.xml → internal_sip_port=5060, external_sip_port=5062
  - config/kamailio/kamailio.cfg → bloque listen custom, WITH_DMZ/WITH_SIGNAL_SERVERNAT off, substdefs reapuntados
  - config/rtpengine/rtpengine.conf → internal/10.118.0.6;external/10.124.193.169
  - scripts/install_server.sh:
    - Nuevas vars/flags: CARRIER_SIP_IP, PUBLIC_MEDIA_IP (--carrier-sip-ip=, --public-media-ip=)
    - Nueva función detach_from_dsiprouter() (stop+disable+mask)
    - deploy_apolo_configs parchea IPs FS (10.118.0.6 → $INTERNAL_IP_ADDR), Kamailio con $CARRIER_SIP_IP, RTPEngine con $PUBLIC_MEDIA_IP
    - configure_freeswitch reducido (el repo ya trae todo)
    - configure_kamailio inserta Integratel y FS como peers en MySQL (dr_gateways + address grupos 1/2)

  Cambio semántico clave: CARRIER_* ahora identifica al peer (Integratel), no a nuestra cara pública. Las viejas --internal-ip, --external-ip,
  --public-media-ip fueron eliminadas.

  Cómo lo consume cada componente

  - FreeSWITCH: sip-ip → INTERNAL_SIP_IP; rtp-ip → INTERNAL_MEDIA_IP (sed separados en deploy_apolo_configs).
  - Kamailio substdefs: INTERNAL_IP_ADDR ← INTERNAL_SIP_IP; EXTERNAL_IP_ADDR ← EXTERNAL_SIP_IP (custom listen block usa ambos para bind directo).
  - RTPEngine: interface = internal/$INTERNAL_MEDIA_IP;external/$EXTERNAL_MEDIA_IP.
  - Kamailio MySQL (configure_kamailio):
    - dr_gateways gwid=100: sip:$INTERNAL_SIP_IP:5062 (FS)
    - dr_gateways gwid=8: sip:$CARRIER_SIP_IP:5060 (carrier)
    - address grp=1: $INTERNAL_SIP_IP:5062 (tag FreeSWITCH)
    - address grp=2: $CARRIER_SIP_IP:5060 (tag Carrier-SIP) + $CARRIER_MEDIA_IP (tag Carrier-RTP)

  Aplicado en caliente

  Tags actualizados en MySQL Integratel-* → Carrier-*. Los tres servicios siguen active y los listeners son exactamente los esperados:

  10.118.0.6:5060    freeswitch (internal)
  10.118.0.6:5062    freeswitch (external)
  10.118.0.6:5080    kamailio (FS-face)
  10.124.193.41:5060 kamailio (carrier-face)

  Comando para re-instalar con los nombres nuevos

  sudo ./scripts/install_server.sh \
    --internal-sip-ip=10.118.0.6 \
    --external-sip-ip=10.124.193.41 \
    --internal-media-ip=10.118.0.6 \
    --external-media-ip=10.124.193.169 \
    --carrier-sip-ip=10.35.12.68 \
    --carrier-media-ip=10.35.12.84 \
    --reset-db \
    --non-interactive

  Memoria project_fs_kamailio_split_architecture.md actualizada con la tabla canónica y el cambio semántico de CARRIER_*.

