# Site Inputs

Every operator interaction that changes something at the site — tripping the
E-stop, opening a line switch, closing a feeder breaker — is a **request to
send a signal**, not a state change. The click records an intent; something
else carries it to the RTAC; the site decides what to do with it and reports
the result on its own schedule, through the read-only points documented in
[`alarms/README.md`](alarms/README.md).

This document describes that category of signal and enumerates its instances —
one per interactable element on the SLD.

> **Status: the request path is built; the write path is not.** An operator's
> click is recorded, run through the lifecycle below, and reported — and then
> fails, because no control has anywhere to be written to. No write addresses
> are given anywhere in this document because we do not have any: the client's
> workbook has an `Outputs` sheet, but it carries no tabular data yet. When it
> arrives it settles the addressing, and probably the shape, of the write path
> (neems-core#111); nothing above that layer should need to change.
>
> A demo deployment is the exception, and only in who resolves the request —
> see [Demo mode](#demo-mode) below.

## The category

The alarm spec models points in two categories, `digital` and `analog`, both
`access: read_only`. Site inputs are the third: **writable points**, one per
thing an operator can act on. The spec's `access` field is already per entry
rather than a per-category flag for exactly this reason — see "source vs.
derived fields" in [`alarms/README.md`](alarms/README.md).

> **"Site input" is the client's phrase; the code says "control".** `SiteInput`
> was already taken — `models::site::SiteInput` is the set of fields for
> creating a site, and both would sit side by side in the published types
> package meaning entirely different things. So the endpoints are `/Controls`,
> the registry is `SITE_CONTROLS`, and the requests are `control_requests`. This
> document keeps the client's word because it is the word they will use when
> they send the `Outputs` sheet.

Two things separate a site input from an alarm point, and both drive the design:

- **It is a request, not a value.** An alarm point has a current reading; a site
  input has a *history of attempts*, each of which either got out or didn't.
  The unit of state is the request, and it is durable and auditable.
- **What we owe an operator ends at the RTAC.** Getting the signal to the site
  is this system's entire undertaking. What the site then does — whether the
  breaker actually opens — is reported separately and continuously by the
  read-only points, and is deliberately not folded into the request's outcome.

## The request lifecycle

```
  pending ──► sent
     │
     └──────► failed
```

| State | Meaning | Set by |
|-------|---------|--------|
| `pending` | Recorded from an operator; not yet written to the RTAC. | The API, on the operator's request. |
| `sent` | Written to the RTAC, and the write succeeded. **Terminal success.** | The collector, once its write returns. |
| `failed` | Nothing wrote it to the RTAC. **Terminal failure.** | Three ways in, below. |

`failed` is ours rather than the client's, and it is the state the SLD exists to
show: an operator who clicks a breaker and gets a spinner forever has been told
nothing. It is reached three ways, in descending order of how much they can tell
the operator:

- **At the request**, when the control has no write register — there is nowhere
  for the signal to go, and the API says so before leaving anything pending.
- **From the collector**, which knows why its write failed (unreachable RTAC,
  refused write) and reports it through the `Failed` endpoint.
- **On a timeout**, after 60s, as the backstop for a collector that is not
  running at all. Applied on read, so a request nothing ever picked up resolves
  without needing a browser open. Matches the E-stop's
  `DISPATCH_TIMEOUT_SECONDS` (`neems-api/src/api/estop.rs:45`) for the same
  reason.

**`sent` is terminal, deliberately.** The client's clarification also described
an `acknowledged` state — the RTAC signaling receipt — which is **out of scope
at this stage**; see "A third state, deferred" below for what it would take and
why nothing is lost by waiting.

### Request state and site state are separate axes

The same rule the alarm system follows for acknowledgement: an acknowledged
alarm may still be firing, and a cleared one may still be waiting to be
acknowledged. Here: a `sent` request whose breaker is still closed means the
signal got to the RTAC and the breaker has not moved — information about the
site, not a failure of the request. The SLD must render both axes and must never
collapse them into one indicator.

This is the whole reason `sent` can be terminal without leaving the operator
uninformed. "Did my click get out?" is answered by the request, finally and
quickly; "did the breaker move?" is answered by the readback, continuously and
for as long as anyone looks. Neither question is waiting on a state between
them.

Concretely, a switch on the diagram has:

- a **position**, from the read-only feedback point (the "Position readback"
  column below), which is the only thing that may drive the drawn contact; and
- a **request overlay**, from the lifecycle above, which is what tells the
  operator their click went somewhere.

The diagram has the second of those (neems-react#112): a click records a
request and the element carries a badge saying what became of it, drawn as a
separate mark rather than as a change to the symbol, so neither axis can be
mistaken for the other.

It does not yet have the first. The drawn position is still local — it moves
when someone clicks and at no other time — which makes it stale rather than
invented, and unreliable in exactly one direction: it will show a movement the
site never made. neems-react#113 replaces it with the readback points, and until
then the badge is the only feedback on the diagram that is actually sourced from
the site.

### A third state, deferred

`sent` already rests on a receipt. A Modbus write returns a response — the
RTAC's Modbus server confirming the registers were written (`write_command`,
`neems-data/src/rtac/modbus_client.rs:345`) — so "the write succeeded" is the
RTAC answering, not us assuming. That is what makes `sent` a defensible terminal
state rather than a hopeful one.

What it does not tell us is whether the RTAC's *logic* picked the command up, as
opposed to the value merely landing in a register. Distinguishing those is what a
separate `acknowledged` state would buy, and it needs a point we do not have —
plausibly a **command echo** register the RTAC writes back, or a **handshake
bit** per control. Both would come from the `Outputs` sheet, which does not exist
yet, so there is nothing to build against today regardless.

Deferring costs little, because the failure `acknowledged` would catch — an RTAC
accepting writes into a register nothing acts on — is still *visible*, just
indirectly: the request reads `sent` and the position readback never changes. An
operator sees that. What they would lose is being told which of the two halves
broke, and that is worth revisiting once the `Outputs` sheet lands and we know
whether a handshake point is even on offer.

Design consequence in the meantime: **do not model the lifecycle as a boolean.**
Keep it an enum on both sides of the wire so a fourth variant is an addition
rather than a rewrite, and keep the request's own outcome separate from the
readback (above) — the two-axis rule is what makes a later `acknowledged` slot
in without disturbing anything the SLD draws.

## What exists today

### The E-stop

The E-stop came first and is the working model for everything else, with two
deliberate differences: it is engage-only (there is no endpoint to clear one; a
latched E-stop is cleared on site), and it is site-level rather than
per-element.

| Endpoint | Method | Purpose |
|----------|--------|---------|
| `/api/1/Sites/<site_id>/EmergencyStop` | POST | Record a request. Coalesces onto one already in flight. |
| `/api/1/Sites/<site_id>/EmergencyStop` | GET | Observed state + latest request, read together. |
| `/api/1/Sites/<site_id>/EmergencyStop/Pending` | GET | What the collector should act on. |
| `/api/1/Sites/<site_id>/EmergencyStop/<request_id>/Dispatch` | POST | The collector reporting its write succeeded. |

`EstopRequestStatus` (`neems-api/src/models/estop.rs`) is
`Pending -> Dispatched | Failed` — exactly the lifecycle above, with `sent`
spelled `dispatched`. The E-stop keeps its own name and its own table: it is
engage-only and site-level, and rewriting a working safety path to share a
vocabulary is not worth the risk. The collector side is
`neems-data/src/rtac/estop_http.rs`, which polls the pending endpoint and
reports dispatch back over HTTP because `neems-data` has no connection to the
API database.

The command registers the E-stop is written through (`CMD_COMMAND` at 1000,
`neems-data/src/rtac/protocol.rs:230`) are **our own framing**, not the
client's. They were moved to 1000–1004 to clear the client's 101–102 / 601–780
range once point numbers became addresses. Per-element controls should expect to
use whatever the `Outputs` sheet specifies instead, not to extend this block.

### Every other control

The controls in the table below share one path, added for neems-core#110:

| Endpoint | Method | Purpose |
|----------|--------|---------|
| `/api/1/Sites/<site_id>/Controls` | GET | The controls and the latest request against each. |
| `/api/1/Sites/<site_id>/Controls/<control_id>/Requests` | POST | Request an action. Coalesces onto one already in flight. |
| `/api/1/Sites/<site_id>/Controls/Pending` | GET | What the collector should act on. |
| `/api/1/Sites/<site_id>/Controls/Requests/<request_id>/Sent` | POST | The collector reporting its write succeeded. |
| `/api/1/Sites/<site_id>/Controls/Requests/<request_id>/Failed` | POST | The collector reporting why it could not. |

The table of instances is compiled in, at
`neems-data/src/rtac/site_controls.rs` — it describes the site's equipment and the
code that knows how to write it, not data an operator can edit, and serving it
keeps the diagram from carrying its own copy of which elements are interactable.
Requests live in `control_requests` (`neems-api/src/models/control_request.rs`).

**Every request off demo mode fails immediately**, because no entry in that
table has a write register. The API checks before leaving a request pending and resolves it with a
reason an operator can read, rather than handing the collector work it cannot do
and letting the operator watch a minute of nothing. That check disappears by
itself when the `Outputs` sheet fills the column in.

### Demo mode

A demo deployment has no RTAC and runs no collector, so the paragraph above
leaves it with a diagram on which every click fails — which is no demo of the
controls at all. With `NEEMS_DEMO_MODE` on, the API stands in for the collector:
it moves the control's **readback point**, through the same `alarm_state` and
snapshot path `POST /1/Demo/AlarmState` writes, and then reports the request
`sent`.

Three things about that are deliberate:

- **It writes the readback, not the position.** The two axes stay separate here
  as everywhere else. A demo breaker moves because the site says it moved, and
  the diagram reaches that conclusion by reading the same points it would read
  against real hardware — which is what makes the demo worth showing.
- **It writes before it reports.** Same order as the real collector, so a
  failure to move the readback fails the request with a reason rather than
  claiming a signal that went nowhere.
- **It gives nothing a `write_register`.** Demo mode routes around the write
  path; it does not pretend one exists. `writable` is still false on every
  control, `/Controls/Pending` is still empty, and a deployment that has not set
  `NEEMS_DEMO_MODE` still fails every request honestly.

The readback's *sense* is what makes this possible, and it had to be added to
`SITE_CONTROLS` to do it: 101/102 report open and the feeder points report
closed, so the same action drives the two halves of the diagram in opposite
directions. `SiteControl::readback` carries the point and its `active_means`
together for that reason.

**The E-stop takes the same route, through its own path.** A demo E-STOP press
raises alarm 104 and reports the request `dispatched`, so `observed_active` and
the diagram's lockout follow from the alarm feed as they would against hardware.
It stays engage-only: there is still no request that clears a trip. On a demo
the "panel on site" is the Demo Controls drawer, which lowers 104 through
`POST /1/Demo/AlarmState`.

Every one of these writes — drawer alarms, control readbacks, E-stop trips —
goes through one helper that sets the alarm and appends a reading in a single
transaction. The reading is built from the newest bitfield with `alarm_state`
laid over it, because seeded history (`/1/Demo/InjectHistory`) lives only in
readings: a snapshot from `alarm_state` alone would read, in `/Alarms/History`,
as every seeded alarm clearing the moment anyone clicked.

Demo mode does not keep the feed fresh between clicks — nothing writes readings
on a cadence, by decision. The frontend treats stale data as an emergency
(neems-react#120), and a demo bypasses that from the drawer (neems-react#119).

One pending request per control is a database constraint, not merely something
the ORM is careful about: two concurrent clicks that both landed would be two
movements for one ask. A second click while one is in flight coalesces onto it
and does *not* redirect it — an operator who clicks open then close before
either goes out has said two contradictory things, and sending the first is the
only answer that cannot surprise them.

## Instances

One row per interactable element. Ids follow the SLD component id, which is the
key everything else in the system already routes on (`ZONE_TO_COMPONENT`,
`neems-react/src/components/SingleLineDiagram/layouts/NewtownLayout.tsx:25`).

"Position readback" is the existing read-only point that reports the resulting
state — the closest thing to an acknowledgement we have today, and the only
thing entitled to drive the drawn position. "SLD token" is the spreadsheet's
"Related SLD Object" name, which differs from our component ids.

| Control id | SLD component | Label | SLD token | Actions | Position readback | Today |
|------------|---------------|-------|-----------|---------|-------------------|-------|
| `estop` | *(site-level)* | E-STOP | `Estop` | trip *(engage only)* | digital 104 `Estop` | Own endpoints |
| `switch-89l-1` | `switch-89l-1` | 89L-1 | `52-MAIN-1` | open, close | digital 101 `BPS 89L1 Open` | Requestable |
| `switch-89l-2` | `switch-89l-2` | 89L-2 | `52-MAIN-2` | open, close | digital 102 `BPS 89L2 Open` | Requestable |
| `feeder-1a` | `feeder-1a` | 52-MP-1A | `MP-1A` | open, close | digital 607 `AC_breaker_closed` | Requestable |
| `feeder-1b` | `feeder-1b` | 52-MP-1B | `MP-1B` | open, close | digital 637 `AC_breaker_closed` | Requestable |
| `feeder-1c` | `feeder-1c` | 52-MP-1C | `MP-1C` | open, close | digital 667 `AC_breaker_closed` | Requestable |
| `feeder-2a` | `feeder-2a` | 52-MP-2A | `MP-2A` | open, close | digital 697 `AC_breaker_closed` | Requestable |
| `feeder-2b` | `feeder-2b` | 52-MP-2B | `MP-2B` | open, close | digital 727 `AC_breaker_closed` | Requestable |
| `feeder-2c` | `feeder-2c` | 52-MP-2C | `MP-2C` | open, close | digital 757 `AC_breaker_closed` | Requestable |
| `lockout-relay` | `lockout-relay` | 86-M1 | `LOR` | *(see note)* | digital 103 `86-M1 Set` | Requestable, UI-gated |

Notes on the rows:

- **The feeder breakers are the Megapacks' own AC breakers.** Each pack also
  reports `Breaker_ready_to_close` (616, 646, 676, 706, 736, 766) and
  `Breaker_irrational` (615, 645, 675, 705, 735, 765). `ready_to_close` is a
  precondition worth reading *before* enabling a close request, and
  `irrational` — position feedback that contradicts itself — is a reason to
  refuse to draw a position at all. Neither is used today.
- **The line switch readbacks are one-sided.** 101/102 report *open*, not
  position, so "not open" is being inferred rather than reported. That
  distinction matters when the feed is stale: unknown must not render as closed.
- **The lockout relay is gated off.** `siteConfig.lockout.remoteTriggerEnabled`
  is `false` (`neems-react/src/config/siteConfig.ts:34`) and the element takes no
  click while it is. Remotely resetting a lockout relay is a safety-significant
  action and should stay disabled until the client asks for it specifically;
  whether the request is a trip, a reset, or both is not decided.
- **One request per element, carrying the requested position** — rather than
  separate open and close points — matching the client's "a request that is sent
  for both open and close". If the `Outputs` sheet turns out to define paired
  momentary open/close points, this splits, and the table gains a row per point
  while the UI keeps one control per element.

Not interactable, and not proposed as such: the main breaker 52-M1
(`siteConfig.sld.showMainBreaker52M1` exists but nothing reads it), the
Megapacks, transformers, meter, SEL-451 relay and fire alarm panel. They are
rendered from read-only points only.

## Open questions for the client

1. **The `Outputs` sheet.** Write addresses, value encoding, and whether a
   control is a level (requested position) or a momentary pulse per direction.
   Nothing can be written to the site until this lands; until then every request
   on a real deployment resolves as `failed` with "no RTAC point defined", which
   is the truth.
2. **Are all ten controls in scope?** The line switches and the lockout relay
   are the safety-significant ones; the client may want some of them read-only
   on the diagram regardless of what the RTAC accepts.
3. **What happens to a request the site ignores?** A `sent` request whose
   breaker never moves stays that way indefinitely. Whether the UI should
   escalate that to the operator after some interval, and after how long, is a
   client decision — the E-stop's answer today is to stop watching after 60s and
   say "sent, not tripped".
