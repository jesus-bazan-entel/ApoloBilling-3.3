-- Migration 035: Default caller ID ("cabecera") for IP trunks
-- Algunas PBX (ej. Asterisk sin CallerID configurado en el canal) mandan
-- From: "Anonymous" <sip:anonymous@anonymous.invalid> en sus salientes.
-- Sin un numero valido: (a) el carrier recibe un caller ID inutilizable,
-- y (b) el billing engine no puede resolver la cuenta por ANI (el fallback
-- por source_ip tampoco aplica aqui para el From anonimo - ver nota en
-- event.rs). Este campo, cuando esta seteado, se sincroniza a Kamailio
-- (address.tag) y se usa para reemplazar el From antes de relayar a
-- FreeSWITCH (ver kamailio.cfg REQINIT, clasificacion FLT_PBX).

BEGIN;

ALTER TABLE sip_endpoints ADD COLUMN IF NOT EXISTS default_caller_id VARCHAR(20);

ALTER TABLE sip_endpoints
    DROP CONSTRAINT IF EXISTS chk_sip_endpoints_default_caller_id_digits;

ALTER TABLE sip_endpoints
    ADD CONSTRAINT chk_sip_endpoints_default_caller_id_digits CHECK (
        default_caller_id IS NULL OR default_caller_id ~ '^[0-9]{1,20}$'
    );

COMMENT ON COLUMN sip_endpoints.default_caller_id IS
'Numero principal/cabecera de la troncal: reemplaza el From cuando la PBX manda Anonymous o nada. Solo aplica a auth_mode=ip (sincronizado a Kamailio address.tag).';

COMMIT;
