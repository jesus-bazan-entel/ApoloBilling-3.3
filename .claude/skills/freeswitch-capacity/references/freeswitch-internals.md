# How FreeSWITCH spends its resources

What actually limits one FreeSWITCH node, with the source locations to cite
(FreeSWITCH v1.10.12 — `https://github.com/signalwire/freeswitch/tree/v1.10.12`).
Read this when explaining *why* a limit exists or when a recommendation needs a
source.

## Contents
1. The two bottlenecks: SIP and RTP
2. Threading model
3. Timers
4. Core database
5. Events (ESL) and logging
6. Single process = availability limit
7. Official sources

## 1. The two bottlenecks: SIP and RTP

The official performance guide reduces capacity to two bottlenecks, SIP and RTP,
which translate into calls per second and concurrent calls respectively.

Per bridged call:
- **SIP**: ~10 messages, once (INVITE, 100, 180/183, 200, ACK … BYE, 200).
- **RTP**: one packet every ptime (20 ms) per direction per leg → 50 × 2 × 2 =
  **200 packets/s per call**, for the whole call. 1,000 calls ≈ 200,000 pps.

So CPS is limited by signalling work, concurrency by media work.

## 2. Threading model

- **One thread per session.** `switch_core_session_thread_launch()`
  (`src/switch_core_session.c` ~1942) starts a thread (or takes one from the
  session thread pool) per channel. A bridged call = 2 threads, each looping:
  wait for the timer tick → read a frame → write it to the other leg.
  Sleeping threads cost almost nothing; the cost is the work per packet.
- **One event loop per SIP profile.** `sofia_profile_thread_run()`
  (`src/mod/endpoints/mod_sofia/sofia.c` ~3130) drives the sofia-sip stack with
  `su_root_step()` (~3471–3524). Parsing and transactions for a profile go
  through that loop; work is handed to message-queue threads
  (`sofia_msg_thread_run`, ~2273) but the per-profile loop caps CPS. More
  profiles (other IP/port) = more loops.

## 3. Timers

RTP **transmit** is clocked by a timer (`rtp-timer-name`, default `soft`); RX is
socket driven. If the scheduler delays the timer (VM, disk I/O stalls, CPU
contention) all channels stutter at once.

- `mod_timerfd` (kernel timerfd) is *supposed* to be steadier, but on
  2026-10-25 it multiplied the share of calls with >1% skipped reads from 3% to
  30% on a KVM guest and was rolled back. Treat any timer change as a
  measured experiment with rollback ready.
- In **proxy media** the RTP session is created without `USE_TIMER`
  (`src/switch_core_media.c` ~8714–8717: flags reset to `DATAWAIT` +
  `PROXY_MEDIA`; `src/switch_rtp.c` ~3073 destroys the timer in UDPTL/proxy
  mode): packets are forwarded as they arrive. That removes the timer
  dependency entirely.

## 4. Core database

Every channel create/answer/hangup is written to the core DB (SQLite by
default, `/var/lib/freeswitch/db/core.db`). The vanilla `switch.conf.xml`
suggests `core-db-name=/dev/shm/core.db` "for better performance" (lost on
reboot, which is fine: it is runtime state), or `core-db-dsn` for
ODBC/PostgreSQL. On a host with disk pressure this matters.

## 5. Events (ESL) and logging

- Each ESL consumer subscribed to `all` events multiplies event-dispatch work.
  Subscribe only to what is needed (e.g. `CHANNEL_CREATE CHANNEL_ANSWER
  CHANNEL_HANGUP_COMPLETE`).
- `loglevel debug` writes every SIP and media detail (≈1 GB/day observed).
  Production: `info` or `notice`.
- `mod_cdr_csv` writes one CSV line per call (1.3 GB observed) — redundant when
  CDRs already go to a database through ESL.

## 6. Single process = availability limit

All calls live in one process with no state replication. A crash or restart
drops every call (observed: ~50 calls per restart). No tuning fixes this; only
more nodes behind a SIP load balancer (Kamailio `dispatcher`/`drouting`) do.

## 7. Official sources

- Performance guide: https://developer.signalwire.com/freeswitch/FreeSWITCH-Explained/Configuration/Performance-Testing-and-Configurations
- Switch configuration: https://developer.signalwire.com/freeswitch/FreeSWITCH-Explained/Configuration/XML-Switch-Configuration_13173223
- Media handling (official docs repo): https://github.com/signalwire/freeswitch-docs/blob/main/docs/media/media-handling.mdx
- Vanilla config: https://github.com/signalwire/freeswitch/blob/v1.10.12/conf/vanilla/autoload_configs/switch.conf.xml
- Source: `src/switch_core_session.c`, `src/switch_rtp.c`, `src/switch_core_media.c`,
  `src/switch_ivr_originate.c`, `src/mod/endpoints/mod_sofia/{sofia.c,sofia_glue.c}`

The SignalWire documentation site sometimes serves only its landing page to
fetchers; the same text lives in `github.com/signalwire/freeswitch-docs`
(`docs/media/*.mdx`, `docs/users-endpoints/sip-profiles-sofia.mdx`).
