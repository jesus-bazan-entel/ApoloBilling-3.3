# KPIs and formulas

How to turn the collector's raw numbers into capacity figures. Every figure in a
report should say which unit it is in and where it came from (command + time),
because "calls" and "channels" get mixed up easily and every projection depends
on them.

## Contents
1. Units
2. Load KPIs
3. Headroom KPIs
4. Projection (ceiling estimate)
5. Health KPIs that are NOT capacity
6. Worked example (Apolo SBC, 2026-10-25)

## 1. Units

| Term | Meaning | Source |
|---|---|---|
| session = channel | one call leg inside FreeSWITCH | `status` ("N session(s)"), `show channels count` |
| call | bridged pair of legs (A + B) | `show calls count` |
| sessions per call | usually ~2; **3 while a parallel ring is in progress** (A + 2 B-legs) | channels / calls |
| CPS | new sessions per second (FreeSWITCH counts sessions, not calls) | `status` ("session(s) per Sec … peak …") |

`max-sessions` and `sessions-per-second` limit **sessions**, so convert:
`call ceiling = max-sessions / sessions_per_call`. With parallel ring on a share
`f` of calls: `sessions_per_call ≈ 2 + f`. Measure `f` (e.g. share of calls to
numbers with an app registered) instead of assuming every call forks — the third
leg only exists when the forked target is reachable (a `sofia_contact` that
returns `error/...` creates no session).

## 2. Load KPIs

| KPI | Formula | Read it as |
|---|---|---|
| Session utilisation | sessions_now / max-sessions | <60% comfortable, 60–80% plan, >80% act |
| Peak session utilisation | session peak / max-sessions | the one that matters for sizing |
| CPS utilisation | CPS peak / sessions-per-second | >80% → 503s at bursts |
| CPU per call | fs_cpu_pct_one_core / calls | e.g. 18.7% / 46 = 0.41% of a core per call |
| Threads per session | threads / sessions | ~1–1.5 normal (one thread per session + fixed pool) |
| RSS per session | rss_mb / sessions | memory is rarely the limit; flag only if mem_avail < 20% |

Take CPU over ≥10 s (`pidstat -u -p PID 10 1`); one-shot `top` values are noisy.

## 3. Headroom KPIs

| KPI | Formula |
|---|---|
| RTP port capacity (legs) | (rtp-end-port − rtp-start-port) / 2 — each leg takes an even port for RTP and the next for RTCP |
| RTP port capacity (calls) | RTP legs / sessions_per_call (only legs with media in FS; 0 in bypass) |
| Session headroom | max-sessions − session peak |
| CPS headroom | sessions-per-second − CPS peak |

The binding limit is the **smallest** of: session ceiling, RTP port ceiling, CPU
ceiling (section 4), CPS ceiling.

## 4. Projection (ceiling estimate)

```
cpu_ceiling_calls ≈ (cpus × 0.70) / (cpu_per_call_fraction_of_one_core)
```
Example: 8 cores × 0.70 = 5.6 cores; 0.0041 core/call → ~1,360 calls.

Caveats to state every time:
- Linear extrapolation. Real limits show up earlier as **timer jitter** (audio
  stutter, rising `skip_packet_count`), especially on VMs (check
  `virtualization=` and `steal`).
- Transcoding changes everything: a G.729/Opus↔G.711 call can cost 5–20× a
  pass-through call. Recompute per codec mix.
- Media mode changes it: in `bypass_media` the per-call media cost goes to ~0
  and the ceiling becomes sessions and CPS.

## 5. Health KPIs that are NOT capacity

Do not use these as capacity evidence without the caveats:

| Metric | Trap |
|---|---|
| `rtp_audio_in_mos` | `R = int((recv − flaws)/recv × 100)`; `MOS = 1 + 0.035R + 7e-6·R(R−60)(100−R)` (switch_rtp.c `do_mos`). Exactly 4.50 whenever no sequence gaps; drops under 3.6 only at ~30% flawed packets. No codec, delay or jitter term. |
| `rtp_audio_in_skip_packet_count` | Counts reads where no packet was ready and FS generated CNG (switch_rtp.c ~7826). Includes silence and timer phase; **not network loss**. Real seq-gap loss is `in_flaw_total`. |
| PDD from `progress_uepoch − start_uepoch` | Quantised to FreeSWITCH's clock tick (0/19/20 ms). Measures the next hop's 18x, not the destination network (ITU PDD). |
| RTPEngine MOS/jitter in kernel mode (`table = 0`) | RTCP is forwarded in kernel and never parsed: counters freeze. |
| CDR durations | inflated by dead calls when nothing times out (no `rtp-timeout-sec`, no session timers). |

## 6. Worked example (Apolo SBC, 2026-10-25 14:13)

```
calls=46 channels=72 → 1.57 sessions/call (some legs still ringing)
peak sessions=158, max-sessions=1600 → 9.9% peak utilisation
CPS peak=12 / 100 → 12%
fs_cpu=18.7% of one core → 0.41%/call; 8 vCPU KVM
RTP ports 20002–32768 → 6,383 legs
ceilings: sessions 1600/2.06 ≈ 780 calls · RTP 6383/2.06 ≈ 3,100 · CPU ≈ 1,360 · CPS 100
binding limit: max-sessions (~780 calls) → ~10× today's peak
```
