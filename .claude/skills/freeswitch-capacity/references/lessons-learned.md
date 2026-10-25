# Lessons learned (Apolo SBC, October 2026)

Real traps found while analysing a production FreeSWITCH SBC. Check for each of
them on any node: they are cheap to verify and expensive to discover in
production.

| # | Trap | How it showed up | How to check / avoid |
|---|---|---|---|
| 1 | Health check regex vs tabs | `sofia status` separates columns with **tabs**; a check grepping `"^ *internal +profile"` saw every profile as down, rolled back a healthy restart and restarted FS twice | Use `[[:space:]]+`; dry-run every verification against live output before an unattended window |
| 2 | Rollback restores the modified config | The restart script backed up the config *after* the changes had been written, so its rollback restored the same (possibly broken) config | Back up the last known-good config **before** editing; the unattended wrapper falls back to that file only if FS was actually restarted |
| 3 | Timer change regression | `mod_timerfd` on KVM: calls with >1% skipped reads 3% → 30%; rolled back after 10 min | Baseline + after measurement in 5–10 min buckets; rollback ready |
| 4 | MOS ceiling | 88% of calls exactly 4.50; never below 4.13 | Do not present FS MOS as quality; see `kpis-and-formulas.md` §5 |
| 5 | "Loss" that isn't loss | `rtp_loss_pct` was `skip_packet_count` (CNG reads), tripled by the timer change | Real loss = `in_flaw_total`; label skip counts honestly |
| 6 | PDD that isn't PDD | 0/19/20 ms only — one clock tick to the next hop's 18x | Rename or drop; ITU PDD needs the destination's ringing |
| 7 | Calls vs channels | Capacity tables mixed 63 calls and 67 channels | Always state the unit; compute sessions/call |
| 8 | Parallel ring legs | 3 sessions per forked call | Ceiling = max-sessions / (2 + share of forked calls) |
| 9 | Dead calls | No `rtp-timeout-sec` on the main profiles; proxy dialog timeout 6 h with `send_bye=0` | Add rtp timeouts; keep SIP session timers; RTPEngine timeout sends no BYE |
| 10 | Disk writers per call | `mod_cdr_csv` 1.3 GB, `loglevel debug` ~1 GB/day, core.db on disk | Level 1 hygiene |
| 11 | DTMF payload types differ per leg | 97 (carrier/RTPEngine side) vs 101 (PBX side); FS translates today | Mandatory test before proxy/bypass |
| 12 | "Same LAN" assumption | The app's media relay was a **public** IP (45.5.68.242), not the LAN SBC | Read SDP `c=` lines / `remote_media_ip`, don't infer from topology diagrams |
| 13 | Deprecated but working params | `rtp-timeout-sec` logs "deprecated, use media_timeout" yet still applies per channel in 1.10.12 (`sofia_glue.c`) | Verify in source for the running version before switching syntax |
| 14 | Configured ≠ in use | `core-db-name` set but FS still on the old file until restart | Collector prints `core_db_note`; re-run after the restart |
| 15 | Docs site serves a landing page | developer.signalwire.com pages returned only the manual's front page to fetchers | Use `github.com/signalwire/freeswitch-docs` and the source tree at the running tag |
