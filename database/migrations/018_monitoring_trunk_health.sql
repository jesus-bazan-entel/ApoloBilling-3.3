-- Migration: Trunk health KPI for the monitoring dashboard
-- Date: 2026-10-24
-- Description: The "Dispatcher Health" card measured kamcmd dispatcher.list, a
--              dSIPRouter leftover no call uses (routing is drouting, and the
--              dispatcher table was emptied on 2026-10-24). The monitoring
--              collector now sends SIP OPTIONS to every enabled routing_trunks
--              row through Kamailio's own sockets (tm.t_uac_wait_block) every
--              60s, updates routing_trunks.sip_status / last_options_* and logs
--              to routing_sip_status_log (check_source = 'kamailio').
--
--              These two columns keep the per-sample count so the card has a
--              history series. dispatcher_* columns are kept for old samples.

ALTER TABLE monitoring_kamailio_metrics
    ADD COLUMN IF NOT EXISTS trunks_online INTEGER DEFAULT 0,
    ADD COLUMN IF NOT EXISTS trunks_total  INTEGER DEFAULT 0;

COMMENT ON COLUMN monitoring_kamailio_metrics.trunks_online IS
    'Enabled routing_trunks answering SIP OPTIONS (any reply < 500 except 408)';
COMMENT ON COLUMN monitoring_kamailio_metrics.trunks_total IS
    'Enabled routing_trunks probed in this sample';
