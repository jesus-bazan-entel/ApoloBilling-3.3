-- Kamailio (MySQL/MariaDB) schema additions for direct inbound routing.
-- Date: 2026-10-24
-- Applied by scripts/install_server.sh (apply_kamailio_migrations). Idempotent.
-- See database/migrations/017_inbound_direct_routing.sql for the design.

-- htable `inbound_fs_numbers` (kamailio.cfg): numbers that must go through
-- FreeSWITCH even when their inbound route is direct. Written by rust-backend.
CREATE TABLE IF NOT EXISTS apolo_inbound_fs_numbers (
    number   VARCHAR(64) NOT NULL PRIMARY KEY,
    route_id VARCHAR(64) NOT NULL,
    KEY route_id_idx (route_id)
) ENGINE=InnoDB DEFAULT CHARSET=utf8mb3;

-- Dialog-based CDRs (acc module cdr_enable=1, cdr_extra). Only calls that
-- skip FreeSWITCH need them for billing; the others are marked via=fs (kept
-- for cross-checking) or skipped entirely with $dlg_var(nocdr).
ALTER TABLE acc_cdrs
    ADD COLUMN IF NOT EXISTS callid      VARCHAR(255) NOT NULL DEFAULT '',
    ADD COLUMN IF NOT EXISTS src_user    VARCHAR(64)  NOT NULL DEFAULT '',
    ADD COLUMN IF NOT EXISTS dst_user    VARCHAR(64)  NOT NULL DEFAULT '',
    ADD COLUMN IF NOT EXISTS src_ip      VARCHAR(64)  NOT NULL DEFAULT '',
    ADD COLUMN IF NOT EXISTS dst_gwid    VARCHAR(16)  NOT NULL DEFAULT '',
    ADD COLUMN IF NOT EXISTS via         VARCHAR(16)  NOT NULL DEFAULT '',
    -- text: acc writes '' when the dlg_var is unset (unanswered call)
    ADD COLUMN IF NOT EXISTS answer_ts   VARCHAR(20)  NOT NULL DEFAULT '',
    -- unix ts of the final failure reply (acc keeps end_time = start_time then)
    ADD COLUMN IF NOT EXISTS final_ts    VARCHAR(20)  NOT NULL DEFAULT '',
    ADD COLUMN IF NOT EXISTS sip_code    VARCHAR(3)   NOT NULL DEFAULT '',
    ADD COLUMN IF NOT EXISTS sip_reason  VARCHAR(128) NOT NULL DEFAULT '',
    ADD INDEX IF NOT EXISTS via_id_idx (via, id);

-- answer_ts was first shipped as INT UNSIGNED: strict mode rejected the '' acc
-- writes for unanswered calls, dropping their CDR.
ALTER TABLE acc_cdrs MODIFY COLUMN answer_ts VARCHAR(20) NOT NULL DEFAULT '';
