# ApoloBilling
# ApoloBilling-3.3

● Resumen final — arquitectura nueva activa y persistida en el repo

  Puertos/IPs en producción

  ┌─────────────────────┬───────────────────────────────────────────────┬───────────────────────────────────────────────────────────┐
  │      Servicio       │                   IP:Puerto                   │                            Rol                            │
  ├─────────────────────┼───────────────────────────────────────────────┼───────────────────────────────────────────────────────────┤
  │ FreeSWITCH internal │ 10.118.0.6:5060                               │ Clientes SIP registran aquí (context from-pbx)            │
  ├─────────────────────┼───────────────────────────────────────────────┼───────────────────────────────────────────────────────────┤
  │ FreeSWITCH external │ 10.118.0.6:5062                               │ Bus FS↔Kamailio (context from-kamailio, ACL kamailio-acl) │
  ├─────────────────────┼───────────────────────────────────────────────┼───────────────────────────────────────────────────────────┤
  │ Kamailio FS-face    │ 10.118.0.6:5080                               │ Recibe outbound de FS, gateway de FS apunta aquí          │
  ├─────────────────────┼───────────────────────────────────────────────┼───────────────────────────────────────────────────────────┤
  │ Kamailio carrier    │ 10.124.193.41:5060                            │ Cara a Integratel                                         │
  ├─────────────────────┼───────────────────────────────────────────────┼───────────────────────────────────────────────────────────┤
  │ RTPEngine           │ internal=10.118.0.6 ; external=10.124.193.169 │ Relay con media pública independiente                     │
  ├─────────────────────┼───────────────────────────────────────────────┼───────────────────────────────────────────────────────────┤
  │ dSIPRouter          │ masked                                        │ No puede modificar kamailio.cfg/rtpengine.conf            │
  └─────────────────────┴───────────────────────────────────────────────┴───────────────────────────────────────────────────────────┘

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

  Uso del installer

  sudo ./scripts/install_server.sh \
    --internal-ip=10.118.0.6 \
    --external-ip=10.124.193.41 \
    --carrier-sip-ip=10.124.193.41 \
    --public-media-ip=10.124.193.169 \
    --reset-db \
    --non-interactive

  Memoria nueva: project_fs_kamailio_split_architecture.md documenta la arquitectura activa y sus invariantes.

