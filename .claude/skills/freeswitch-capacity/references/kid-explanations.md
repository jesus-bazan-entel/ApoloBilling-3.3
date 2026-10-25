# Explaining it to an 11-year-old

Each technical section of a capacity report ends with a short box "Explicado
fácil" (or "In plain words") that a curious 11-year-old could follow. Rules
that make these work:

- One idea per box, 2–5 sentences, everyday objects, no acronyms (or explain
  them in the same sentence).
- Keep the analogy faithful: if it would lead a reader to a wrong conclusion,
  drop it. Say where the analogy stops working when it matters.
- Use the real number from the report inside the analogy ("today 46 of 780
  seats are taken").
- Never talk down; a child is smart, just new to the topic.

## Analogy library

| Concept | Analogy |
|---|---|
| FreeSWITCH node | A telephone **exchange building** where every call has to pass |
| SIP signalling | The **letters** people send to agree "can we talk?", "yes!", "bye" — a few per call |
| RTP media | The **conversation itself**, sent as 50 tiny voice envelopes per second in each direction |
| CPS limit | How many **new visitors the door guard can let in per second** |
| Concurrent calls / max-sessions | How many **seats** the building has; every call takes 2 seats (3 while two phones ring at once) |
| Thread per session | Each seat has its own **helper** who passes the voice envelopes along; a sleeping helper costs nothing |
| Timer | The **metronome** the helpers follow to send an envelope every 20 ms; if it stutters, everyone's voice stutters |
| Transcoding | A helper who must **translate** every envelope into another language — slow and tiring |
| No transcoding | Both people speak the same language, the helper just **passes the envelope** |
| Proxy media | The helper passes envelopes **without opening them and without the metronome**, as soon as they arrive |
| Bypass media | The building only introduces the two people; then they **talk through a window directly** (RTPEngine) and the building doesn't carry the conversation |
| Bypass after bridge | The building carries the conversation **while the phone rings**, then opens the window once someone answers |
| Core DB on /dev/shm | Writing the visitor list on a **whiteboard** instead of carving it in stone: much faster, erased when the lights go off (that's fine, it's only today's list) |
| mod_cdr_csv | A clerk who copies every call into a **paper notebook** nobody reads, because the computer already saves it |
| loglevel debug | A reporter writing down **every word** of every conversation — tons of paper |
| rtp-timeout | If a line has been **silent for 5 minutes**, someone checks and hangs it up so the seat is free again |
| Session timers | Every 30 minutes the building asks "**are you still there?**" |
| RTPEngine latching | The window remembers the **first face** that looked through it; if the wrong person looked first, the voice goes to the wrong place |
| Single node | **Only one building**: if it closes for repairs, every call inside is cut |
| Horizontal scaling | **Several buildings** with a receptionist (Kamailio) sending each visitor to one that is open |
| MOS ceiling | A thermometer that **only goes up to 4.5** and almost always shows 4.5 — it can't tell you if someone has a slight fever |
| Skip count ≠ loss | Counting the times the helper **found no envelope waiting** — sometimes the person was simply quiet |
| Canary | Sending **one small group** through the new road first, to see if it's safe before sending everyone |
| Rollback | Keeping the **old map** in your pocket so you can go back if the new road is closed |

## Example box

> **Explicado fácil.** El edificio tiene 1.600 asientos y cada llamada usa 2,
> así que caben unas 780 llamadas. Hoy, en el momento más lleno, hubo 158
> asientos ocupados: el edificio está casi vacío. Antes de construir más,
> conviene que los ayudantes trabajen mejor (menos papeles inútiles), no
> agrandar el edificio.
