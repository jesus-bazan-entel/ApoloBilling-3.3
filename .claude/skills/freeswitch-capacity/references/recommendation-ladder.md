# Recommendation ladder

Recommend changes as a ladder: lowest risk and highest certainty first, each
rung with a measurable gate before the next. Skip rungs that are already done
(the collector shows most of them) and stop climbing when the measured headroom
covers the expected growth — more change is more risk, not more quality.

## Contents
- Level 0 — Measure and fix the numbers
- Level 1 — Hygiene (no restart or one planned restart)
- Level 2 — Limits sized to the traffic
- Level 3 — Host and timer
- Level 4 — Media path (proxy → bypass-after-bridge → bypass)
- Level 5 — Horizontal scaling and availability
- What NOT to recommend first
- Gate template

## Level 0 — Measure and fix the numbers (zero risk)

- Run the collector at peak and off-peak; store the output with a timestamp.
- Fix units (sessions vs calls), compute sessions/call, CPU/call, binding limit
  (`kpis-and-formulas.md`).
- Identify misleading indicators (MOS ceiling, skip≠loss, PDD quantised) and
  remove or relabel them before using them as evidence.
- Gate: a one-line statement "binding limit = X at Y% of peak, unit = Z".

## Level 1 — Hygiene

| Change | Why | Restart? | Risk |
|---|---|---|---|
| `loglevel` debug → info | log I/O per packet/message | `fsctl loglevel info` live + config | none |
| Disable `mod_cdr_csv` if CDRs go to a DB | one disk write per call | `unload mod_cdr_csv` live + comment in modules.conf | none |
| `core-db-name=/dev/shm/core.db` | core DB written per channel event | yes (planned) | low |
| `rtp-timeout-sec` / `rtp-hold-timeout-sec` (300/1800) | dead calls hold sessions and inflate CDRs | profile restart / FS restart | low |
| SIP session timers (`enable-timer`, `session-timeout`) | dead-dialog detection when the peer supports them | profile restart | low |
| ESL subscriptions limited to needed events | event dispatch cost | consumer change | none |
| Unload unused modules | memory, threads, attack surface — **not** per-call CPU | restart | low, but grep the dialplan first |

Gate: no new errors in `journalctl`/freeswitch.log; CDRs still written; registrations back to the pre-restart count.

## Level 2 — Limits sized to the traffic

- `max-sessions` ≥ 2 × expected peak sessions (count parallel-ring legs).
- `sessions-per-second` ≥ 2 × measured CPS peak.
- RTP port range ≥ 2 × peak legs with media; never overlapping another relay's
  range (e.g. RTPEngine).
- systemd `LimitNOFILE`, `LimitNPROC`, `LimitSTACK` sized for threads/sockets.
Gate: no "Throttle"/"Max Sessions" lines in the log at peak.

## Level 3 — Host and timer

- Prefer bare metal or dedicated vCPUs; check `steal` and `virtualization=`.
- CPU governor `performance`; FreeSWITCH realtime scheduling (`-rp` /
  `CPUSchedulingPolicy`).
- Remove disk I/O stalls on the host (other daemons reading/writing heavily).
- Timer changes (`mod_timerfd`, `enable-clock-nanosleep`) **only as a measured
  experiment**: baseline `skip_packet_count` share and test calls before, same
  after, rollback ready. Observed regression: timerfd raised calls with >1%
  skipped reads from 3% to 30% on KVM.
Gate: the per-call loss/skip distribution is not worse than the baseline.

## Level 4 — Media path

Only when Levels 0–3 leave too little headroom, or the timer is the proven
weak spot. Order by risk (details in `media-modes.md`):
1. `proxy_media` on one canary route — removes the timer, keeps FS in the path.
2. `bypass_media_after_bridge` canary — keeps forking/early media as today;
   needs re-INVITE support on every far end and in the SBC.
3. Pure `bypass_media` — max capacity; solve RTPEngine latching with parallel
   ring and PT mismatches first.
Gate: full test matrix green on the canary for ≥1 day of real traffic, ASR and
complaint rate unchanged.

## Level 5 — Horizontal scaling and availability

- N FreeSWITCH nodes behind Kamailio (`dispatcher` with OPTIONS health checks,
  or drouting gateway lists with weights). A restart then drops only that
  node's calls.
- Shared state to solve first: registrations (move the registrar to Kamailio
  with `Path`, or share the sofia registration DB), CDR collection per node,
  dialplan/config distribution.
Gate: drain + restart one node with zero impact on new calls.

## What NOT to recommend first

- Patching FreeSWITCH source: the limits are architectural (thread per session,
  one sofia loop per profile); a fork loses upgrades and security fixes.
- Unloading modules "for performance": it does not change per-call CPU.
- Raising limits without measuring: hides the real bottleneck until it fails.
- Any timer or media-mode change without a baseline and a rollback.

## Gate template

```
Change: …            Level: …           Restart: yes/no (window …)
Baseline (before): metric = value @ time
Success if:          metric within …    and no new errors in …
Rollback:            exact command / backup file
Result (after):      metric = value @ time → keep / roll back
```
