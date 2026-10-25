-- 025: where each call left the SBC
--
-- caller_number / called_number are the a-leg's numbers exactly as received
-- in the INVITE (From user / R-URI user, %XX decoded). These columns record
-- the outgoing side of the bridge, taken by the engine from the b-leg:
--   * egress_number:  user part of the To sent to the next hop (sip_to_user),
--                     e.g. 312217473046 in → 17473046 out to FS Legado
--   * egress_ip:      next hop's IP (sip_network_ip)
--   * egress_gateway: FreeSWITCH gateway used (sip_gateway_name)
-- With parallel ring the answered b-leg wins, else the first one. Written
-- from 2026-10-25 ~19:00.
--
-- History: for October inbound Integratel DIDs the number is rebuilt from the
-- dialplan rule that produced it (did-312216800137: bridge
-- freeswitch-legado/${destination_number:4}); egress_ip stays NULL on those
-- rows, which is how the UI tells "per dialplan" from "observed".

ALTER TABLE cdrs ADD COLUMN IF NOT EXISTS egress_number varchar(50);
ALTER TABLE cdrs ADD COLUMN IF NOT EXISTS egress_ip varchar(45);
ALTER TABLE cdrs ADD COLUMN IF NOT EXISTS egress_gateway varchar(64);

COMMENT ON COLUMN cdrs.egress_number IS 'Number sent to the next hop (b-leg sip_to_user); NULL ip = rebuilt from the dialplan';
COMMENT ON COLUMN cdrs.egress_ip IS 'Next hop IP of the egress b-leg (sip_network_ip)';
COMMENT ON COLUMN cdrs.egress_gateway IS 'FreeSWITCH gateway of the egress b-leg (sip_gateway_name)';

SET statement_timeout = 0;

UPDATE cdrs
SET egress_number = substr(called_number, 5)
WHERE start_time >= '2026-10-01 00:00:00-05'
  AND egress_number IS NULL
  AND direction = 'inbound'
  AND freeswitch_server_id <> 'kamailio'
  AND called_number ~ '^31[0-9]{10}$';
