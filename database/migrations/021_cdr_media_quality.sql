-- 021: per-call media quality measured by FreeSWITCH
--
-- RTPEngine forwards RTP *and* RTCP in its kernel module (table = 0), so its
-- userspace never parses RTCP and its MOS/jitter/loss counters stopped
-- advancing. FreeSWITCH is in the media path of every non-direct call and
-- reports, at hangup, what it received on each leg (rtp_audio_in_mos,
-- rtp_audio_in_skip_packet_count / media_packet_count). The engine keeps
-- the worst leg per call. NULL: no media (unanswered) or direct Kamailio CDR.

SET lock_timeout = '5s';

ALTER TABLE cdrs ADD COLUMN IF NOT EXISTS mos REAL;
ALTER TABLE cdrs ADD COLUMN IF NOT EXISTS rtp_loss_pct REAL;

COMMENT ON COLUMN cdrs.mos IS 'Lowest FreeSWITCH-estimated MOS (1-4.5) among the call legs, NULL without media';
COMMENT ON COLUMN cdrs.rtp_loss_pct IS 'Highest received-RTP loss percent among the call legs (FreeSWITCH skip/expected packets)';
