-- Drop dSIPRouter's demo carriers from a fresh install.
-- Date: 2026-10-26
-- Applied by scripts/install_server.sh (configure_kamailio). Idempotent.
--
-- `dsiprouter.sh install` seeds ~50 public carriers (Twilio, Skyetel, Voxbone,
-- VoIP Innovations, ...), MS Teams SBC addresses and a "Default Outbound Route"
-- (groupid 8000, no prefix) to the Skyetel group. The `address` rows make
-- Kamailio trust those IPs as carriers, and the route would send every
-- outbound call to Skyetel. The reference SBC had them removed by hand.
-- Only rows with the seed names are touched, so re-running the installer on a
-- configured node keeps its real carriers.

SET @seed := '^name[:=](Twilio|Skyetel|Flowroute|Voxbone|VoIP Innovations|Thinq|Voxtelesys|Les\\.net|ThinkTel)';

DELETE FROM dr_gateways WHERE description REGEXP @seed;
DELETE FROM dr_gw_lists WHERE description REGEXP @seed;
DELETE FROM address     WHERE tag REGEXP @seed OR tag LIKE 'name:msteams-sbc%';
DELETE FROM dispatcher  WHERE description REGEXP @seed;

-- Mappings left pointing at nothing
DELETE FROM dsip_gw2gwgroup
 WHERE gwid NOT IN (SELECT CAST(gwid AS CHAR) FROM dr_gateways);
DELETE FROM dsip_gwgroup2lb
 WHERE gwgroupid NOT IN (SELECT CAST(id AS CHAR) FROM dr_gw_lists);

-- The seeded catch-all outbound route, once its target group is gone
DELETE FROM dr_rules
 WHERE description = 'name:Default Outbound Route'
   AND gwlist REGEXP '^#[0-9]+$'
   AND CAST(SUBSTRING(gwlist, 2) AS UNSIGNED) NOT IN (SELECT id FROM dr_gw_lists);
