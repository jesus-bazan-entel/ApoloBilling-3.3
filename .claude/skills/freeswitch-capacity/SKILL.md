---
name: freeswitch-capacity
description: Capacity analysis of a FreeSWITCH node (SBC, B2BUA, PBX) from its configuration, live monitoring and KPIs, ending in a graded, low-risk-first recommendation plan with technical explanations plus plain-language boxes an 11-year-old could follow. Use this whenever someone asks how many calls a FreeSWITCH can handle, whether it will scale, why it drops calls or stutters under load, how to size max-sessions / sessions-per-second / RTP ports, whether to use proxy_media or bypass_media, whether to add another FreeSWITCH node, or wants a capacity/health report of a FreeSWITCH server — even if they only say "SBC", "fs_cli", "concurrent calls" or "CPS" without the word capacity.
---

# FreeSWITCH capacity analysis

Goal: tell the user, with evidence, **what limits this FreeSWITCH node today, how
much headroom it has, and what to change in what order** — each change with its
risk, gate and rollback. Then explain it twice: technically, and in a short
"Explicado fácil" box anyone can follow.

The analysis is only as good as its numbers, so measure first, compute in the
right units, distrust misleading indicators, and recommend the smallest change
that buys the needed headroom.

## Workflow

### 1. Collect (read-only)

Run the collector on the FreeSWITCH host (root or a user with fs_cli access):

```bash
.claude/skills/freeswitch-capacity/scripts/collect_fs_capacity.sh -s 10 -o /tmp/fs-capacity-$(date +%F-%H%M).txt
```

It changes nothing. It prints node/host facts, `status`, calls vs channels,
CPU of the process, vmstat (iowait/steal/disk), `switch.conf.xml` limits, core
DB location (and whether the configured one is in use), systemd limits, every
SIP profile's media/timer/timeout params, codecs and DTMF payload type per leg,
loaded modules and timers, per-call disk writers, recent crash/limit lines, and
neighbours (Kamailio dialogs, RTPEngine kernel table).

Also gather what the collector cannot see:
- **Peak data**: ask for (or query) the busiest hour — CDR counts per minute,
  the `status` peak line, monitoring history. An off-peak snapshot alone
  understates the need.
- **Dialplan features in use**: grep the dialplan for `bridge` patterns
  (comma = parallel ring), `playback`, `record_session`, `conference`,
  `pre_answer`, `ignore_early_media`, `bypass_media`, `proxy_media`.
- **What sits around FS**: SBC/proxy in front (Kamailio/OpenSIPS), media relay
  (RTPEngine), far ends (carrier, PBX, push SBC / mobile app).
- **Expected growth**: target calls/CPS, and whether the concern is capacity or
  availability (they have different fixes).

If you cannot run commands on the host, ask the user to run the collector and
paste the output.

### 2. Compute KPIs in explicit units

Follow `references/kpis-and-formulas.md`: sessions vs calls, sessions per call
(parallel ring adds a leg), peak utilisation of max-sessions and CPS, CPU per
call, RTP port ceiling, CPU ceiling estimate, and the **binding limit** (the
smallest ceiling). State unit, source command and timestamp for every number.

### 3. Diagnose the bottleneck

Classify findings into: SIP/CPS · media/concurrency · timer precision ·
disk/core DB/logging · limits/config · availability (single node). Read
`references/freeswitch-internals.md` for the why (thread per session, one sofia
loop per profile, timer-clocked TX, core DB writes) and cite source/doc
locations from it.

Before using any quality metric as evidence, check `kpis-and-formulas.md` §5:
FreeSWITCH MOS sits at its 4.5 ceiling, `skip_packet_count` is not network
loss, PDD from FS timestamps is quantised to the 20 ms tick, RTPEngine kernel
mode freezes RTCP stats. Run through `references/lessons-learned.md` — every
trap there has hit a real production node.

### 4. Recommend as a ladder

Use `references/recommendation-ladder.md`: Level 0 measure → 1 hygiene → 2
limits → 3 host/timer → 4 media path → 5 horizontal. Skip rungs already done,
and stop when the headroom covers the expected growth. Each recommendation
needs: why (evidence), expected effect, risk, restart needed, gate
(before/after metric), rollback.

Media-mode changes (proxy, bypass-after-bridge, bypass) carry the most hidden
risk: go through the compatibility checklist and test matrix in
`references/media-modes.md` and say which items are verified and which still
need a test.

When availability is the real problem (restarts drop every call), say so
plainly: tuning does not fix a single process; more nodes do.

### 5. Write the report

Write it in the user's language. Use this structure:

```markdown
# Capacidad de FreeSWITCH — <host> (<fecha>)

## Resumen
3–5 bullets: binding limit and headroom (with unit), main risk, top
recommendation. Then an "Explicado fácil" box.

## Datos medidos
Table: metric · value · unit · source/time. Flag off-peak samples.

## Límites y techo
Table of ceilings (sessions, RTP ports, CPU, CPS) and which one binds.
"Explicado fácil" box.

## Hallazgos
One subsection per finding: evidence → why it matters (cite source/doc) →
"Explicado fácil".

## Plan gradual
Table: nivel · cambio · por qué · efecto esperado · riesgo · reinicio · gate ·
rollback. Then per level a short "Explicado fácil".

## Qué falta verificar
Items not measurable from here (peak data, tests on far ends).

## Fuentes
Official docs and source files used, as links.
```

For the plain-language boxes, use `references/kid-explanations.md` (analogy
library and rules). Keep each box short, faithful and with the real numbers.

## Guardrails

- The collector is read-only; **any change** to a production node (reload,
  unload, restart, config edit) needs the user's go-ahead, a backup of the
  known-good config taken *before* editing, and a low-traffic window if calls
  drop. Restarting FreeSWITCH drops every call in progress.
- Never present a timer or media-mode change as safe without a measured
  baseline and a rollback; one such change multiplied skipped reads 10× on a VM.
- Do not recommend patching FreeSWITCH source to "scale": the limits are
  architectural and a fork loses upgrades and security fixes.
- When a number is an extrapolation, say so and name what would break first
  (usually timer jitter on VMs before CPU saturation).
