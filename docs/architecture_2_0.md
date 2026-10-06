# 2.0 architecture

This page describes the public 2.0 vocabulary and the ownership rules behind
it. The implementation has one protocol authority per session. A typed,
blocking, async, or dynamic camera value is a view onto that authority; it is
not another connection.

## Public vocabulary

| Concept | 2.0 meaning |
| --- | --- |
| `SessionConfig` | Mode-independent, reusable profile/target registry and operational tuning. |
| `blocking::Session` | Caller-driven owner for a blocking transport. |
| `Session` | Owner handle for an async transport and selected executor/runtime. |
| `Camera<P>` | A statically checked view for one registered target and compile-time profile `P`. |
| `BlockingDynSessionCamera` | A native blocking runtime-profile view. Its typed request methods return synchronous results and blocking lifecycle handles. |
| `DynSessionCamera` | An async runtime-profile view. It exposes `DynSessionCameraControl`, the 14 object-safe dynamic noun traits, and `DynMotion`. |
| `StateCache` | A read-only, target-local projection of exact applied write-only state. |
| `MetricsSnapshot` and `DiagnosticEvent` | Bounded scalar metrics and sanitized owner observations. |

The final noun entry points are `power`, `zoom`, `system`, `pan_tilt`, `focus`,
`exposure`, `white_balance`, `image`, `presets`, `tally`, `nd_filter`,
`motion_sync`, `menu`, and `advanced`. Motion safety and observation are a
separate `motion()` view. Dynamic code uses the corresponding `Dyn*` noun
traits. Names such as `DynCameraControl`, `DynPanTiltControl`, and
`IntoDynCamera` belong to the historical 1.x vocabulary and are not 2.0 API
names.

## Ownership and lifetime

Construction moves one transport into one serialized owner. The owner is the
only component allowed to perform protocol scheduling, pacing, correlation,
retry, cancellation, transport I/O, state-cache mutation, and diagnostic
delivery.

* A blocking session runs that owner on one native worker thread the session
  owns (D24, #780, revising the earlier caller-thread model). Callers enqueue
  work through the same bounded boundary lanes the async owner uses and wait
  on their own observation slots; the owner makes progress with no caller
  inside the session.
* An async session spawns one detached owner task on the caller-selected
  executor. Tokio and smol adapters may both be compiled, but a session picks
  one runtime at construction.
* Typed camera views clone or borrow the owner handle and retain only their
  target and validated profile facts.
* Dynamic views erase profile/request types, not ownership, transport, runtime,
  timeout, or state-cache policy.
* `shutdown` is an idempotent, non-joining signal accepted by the one owner.
  It returns once the signal is accepted and does not claim that actor or
  transport teardown has finished.
* `close` consumes the session, requests the same shutdown, and waits for the
  sole detached owner to finish its boundary drain and drop its
  driver/transport. It is the deterministic transport-teardown barrier for
  reopening the same endpoint, not an executor-specific task join. When
  shutdown is accepted, `close` returns success only for the explicit
  `RuntimeShutdown` terminal cause; a transport close or stream poison that
  wins the normative source ordering is returned exactly. An immediate error
  sending `close`'s shutdown signal is preserved. Neither API is a second
  protocol authority or an implicit command resubmission mechanism.

There are no public callbacks, user-supplied lifecycle IDs, or unbounded
queues. The only background thread is a blocking session's owner worker: one
per session, not per camera (D24, #780, revising the earlier "no per-camera
background workers" invariant). Drop is not a protocol motion action:
dropping an operation handle detaches its observation, and dropping a
non-final shared `Session`, camera view, or operation leaves the shared owner
and other views running. Dropping the final owner handle signals the owner to
stop; the async owner task or blocking worker then releases its transport,
without the drop joining it. Neither release path issues a protocol STOP. Use
`shutdown` for an explicit shutdown signal or consuming `close` for the
deterministic release barrier: blocking `close` joins the worker thread. An
operation handle must be explicitly cancelled or detached according to its
documented lifecycle.

### Timeout and retry ownership

Timeout policy has one ownership boundary. A validated profile owns its exact
`CommandTimeouts` category table and its inquiry, acknowledgement,
cancellation, ambiguity, raw inquiry-reply skew, busy, and pacing facts.
`OperationalTuning` contains only validated per-category or per-fact overrides;
it is not a second profile or transport policy. Pure request preparation
selects one response deadline — the inquiry deadline for an inquiry, otherwise
the request's exact command category — and lowers it with the acknowledgement,
cancellation, ambiguity,
and retry policy into an inert request context. The owner/engine consumes that
context without reselecting or inventing a deadline. `TransportConfig` carries
socket/serial behavior only and does not own protocol retry or completion
policy.

Private request, transmission, and correlation identifiers advance
monotonically within a session and are never reused. If the 64-bit identity
space is exhausted, admission or transmission fails closed with
`RuntimeIdentityExhausted`; an old owner input must never alias new work.

The two execution modes share one owner core, not an async implementation
hidden behind a blocking wrapper. Boundary handling, source arbitration, turn
logic, and receipt observation are executor-free shared code; each shell
supplies only its I/O. The blocking worker drives `BlockingTransport` directly
with bounded reads and has no Tokio, smol, futures executor, or pollster
dependency in its downstream graph. `async` drives `AsyncTransport`
through the caller-selected executor. CI checks the native blocking dependency
boundary for the network, serial, and `test-utils` feature sets; the async
testkit's executor rides on `async`, so enabling `test-utils` on a blocking-only
build links no executor.

Internally, both shells complete engine input through the same executor-free
`EngineTurn` policy. A complete turn runs due work, pending cancellation, and
one ordinary dispatch; an input-only turn preserves retained wire evidence
ahead of deadlines at the same sampled instant. These are
options on one engine boundary, not feature-gated owner entry points. A
correlated frame sampled exactly at its response deadline therefore wins; a
frame sampled strictly later is ignored as stale before the due transition,
without making a frame for an unrelated live request stale.

## Target registry and preflight

`SessionConfig` has exactly seven individual target slots, VISCA IDs 1 through
7. Broadcast is never a session target. Registration is immutable once the
owner starts, and duplicate IDs, an eighth target, an empty registry, and
profile/configuration conflicts are rejected before transport I/O.

`camera()` is valid only when exactly one target is registered. A multi-target
session must use `camera_for(target)`, which also verifies that the requested
compile-time profile matches every protocol identity fact in the registered
`ProfileSpec`: capability and reply-domain facts, pan/tilt coordinate codec,
transports, envelope, timing, maximum command sockets, operation-complete and
command-cancel support, preset-recall axes, and position-inquiry support. A
runtime profile that customizes any of those facts must not claim a built-in
profile ID and cannot project that built-in typed facade. Dynamic views use the
same sole-target rule and an explicit target selection for multi-target
sessions.

Multi-target registration is rejected before socket creation on any
IP-addressed transport, which is every standard TCP and UDP configuration and
every Sony-encapsulated one. Heterogeneous *envelopes* are never accepted:
admission requires every registered profile to report the same wire envelope
as the first, and any mismatch is an error. What a multi-target session does
accept is heterogeneous *profiles* that share one envelope — in practice
several raw-VISCA profiles — carried by a serial-addressed transport. The
built-in serial transports qualify: both `transport-serial` and
`transport-serial-tokio` declare `AddressingMode::Serial`, so a caller-owned
transport is not required. A caller-owned transport that reports a standard
kind is validated against the profile transport registry. A truly custom
transport that reports no standard kind (`None`) may bypass only the standard
profile/transport pair matrix as an intentional BYO escape hatch. It still
undergoes target-registry, envelope, addressing/topology, pacing, framing,
buffer, and bounded-owner validation, and must provide its stream/datagram
semantics and transport configuration.

The static `Has*` profile-marker gates on noun/accessor methods are the
compile-time permission surface. Direct generic `Camera::execute` and
`Camera::inquire` intentionally carry no request-specific `P` bounds: they are
the uniform typed/downstream extension boundary. A runtime `ProfileSpec` is
validated in full before owner admission, and dynamic callers use
`capabilities()`/`supports_typed(...)` before selecting optional controls.
Metadata is discovery information; it is not a fallback that exposes an
unsupported typed operation.
For example, broad `HasExposure` admits the exposure domain, while the shared
`04 39` mode command and inquiry require `HasExposureMode`; erased callers
recheck both the typed surface and its source-backed mode inventory.

## Engine transition and ordering

The construction and request path has a fixed order:

1. Build or deserialize `SessionConfig` and validate target IDs, registry
   cardinality, profiles, transport envelope, addressing, tuning, and bounded
   owner policy.
2. Create the one owner and its fixed target-local state registry. No socket,
   serial device, or protocol frame is opened before step 1 succeeds.
3. Start the owner (async) or retain the caller-driven owner (blocking).
4. Project `Camera<P>`, `BlockingDynSessionCamera`, or `DynSessionCamera` views
   without creating a second owner, runtime, transport, or cache.
5. Prepare each request against the selected target/profile and admit it to the
   owner. Preparation invokes `Request::validate_for_profile` before encoding;
   crate-provided direct requests therefore return `FeatureNotSupported` for
   an unsupported profile before owner admission or transport I/O. `write_into`
   is only the profile-free encoding hook. Raw and other downstream requests
   remain explicit low-level extensions and may implement their own profile
   validation; preparation failures do not update state.
6. Schedule writes, replies, retries, pacing, cancellation, and completion in
   the owner. The strictest applicable profile pacing and bounded retry policy
   always wins.
7. For a request with an exact `AppliedStateEffect`, update that target's cache
   before best-effort observer fanout. A failed write, timeout, pre-ACK failure,
   or merely prepared command cannot create a cache value.
8. Record terminal outcome, release admission capacity, and retain only the
   bounded diagnostic/history state promised by the public API.

### Raw response shapes and correlation ownership

Correlation before ACK is envelope-specific. A raw-VISCA target normally has
one unacknowledged command candidate across `Sending`, `AwaitingAck`,
and `AwaitingCompletion`. One intrinsically `Urgent` command may cross one
existing positional candidate as the #714 safety-lane exception.
While both are open, unsequenced ACK/error evidence binds to neither; the engine
never guesses by recency. Once an unambiguous ACK assigns a socket, the next
command may be written while the first executes, so a two-socket camera retains
its useful concurrency without asking FIFO order to identify an ACK. A completion-only
command (`RawReplyShape::CompletionOnly`, issue #700) never earns a socket, so
it can never be socket-correlated; it therefore holds the target's command
channel exclusively for its whole lifetime — `AwaitingCompletion` counts as an
uncorrelated command that blocks any other dispatch, and it may not itself start
until the target is idle — which keeps its completion attributable to it alone.
A stray ACK to such a command is ignored; it never assigns a socket. Raw ACK and error
routing never uses command FIFO or temporal recency. Sony-encapsulated requests
may pipeline before ACK because their envelope sequence provides an exact
correlation key. An unsequenced ACK is never attributed by a guess.

A raw `NoReply` command likewise has no response identity while its local
write is in flight. It starts only when its target has no live command or
inquiry, and its `Sending` phase excludes same-target work until the write
either fails or leaves the bounded terminal hold below. Sony sequence
correlation does not need this raw-only exclusion.

Successful raw terminals that never earned a response identity retain bounded
evidence rather than disappearing from correlation immediately. A `NoReply` or
`CompletionOnly` command holds the target's response/correlation lane through
its ambiguity deadline: ordinary commands and inquiries to that target wait,
and any late completion, inquiry reply, or error for it is ignored before it
can bind to later response-bearing work. A STOP never waits for this hold, on
any transport (#795): it is written at once, and since the commands behind the
hold never ACK and earn no socket, its ACK and the completion or error naming
the socket it executes on pass the hold and reach it. A socketless error inside
the hold could be either's, and binds to neither: on a datagram transport the
hold discards it (the STOP, lacking any other answer, honestly ends
`UnsequencedCommandUnconfirmed`); on a byte stream the ledger disputes it
(below). Another `NoReply`
can safely write during that hold because it expects no response and extends
the same fixed deadline. This is the target's keyed `AllResponses` hold. It may
coexist with independently expiring `Socket`, `PreAck`, and `InquiryUnkeyed`
holds, and overlapping owners of one key erase identity rather than choosing by
recency. The hold table has one bounded entry per target/scope pair, expires
through the ordinary next-wake path, and never grows with command history. A
normal ACK-then-completion command deliberately does
not retain a terminal socket hold: the camera may immediately reuse its freed
socket, and unsequenced raw traffic cannot distinguish that legitimate next
response from a duplicate predecessor response. This preserves established raw
throughput and immediate socket reuse; the target quarantine is reserved for
the uncorrelatable reply shapes.
When a raw ACK names a free socket, that socket is exact evidence. When the
named socket is instead held by another request *or only by that request's
expired exact-socket quarantine* — the classic cause is a lost completion
frame that made the camera reuse the socket — the camera's new assignment
supersedes the stale local evidence (#721/#750). A live owner immediately fails
as unconfirmed; an inert `Socket(S1|S2)` hold is downgraded. In both cases the
original ambiguity deadline remains as an unkeyed `PreAck` hold, while the
uniquely identified successor owns the named socket for its completion and any
cancellation packet.
A socketless ACK still selects the first free registered socket. The bounded
#620/#682 other-free-socket fallback remains only for sequence-correlated Sony
traffic, whose later terminals have an independent request identity.

For a raw socketless error, the evidence rule is equally strict: route the
unique unacknowledged command only when no inquiry owner is live; otherwise
route the legitimate per-target inquiry FIFO only when no unacknowledged
command exists. A command-plus-inquiry collision is ignored. A socketless
error never falls back to an `Executing` command.

An error that names a socket follows the same evidence standard as an ACK
naming a socket. A camera that cannot start a command answers `z0 6y ... FF`
without an ACK, where `y` is the socket it allocated for that command; the
PTZOptics G2 rejects a focus STOP in auto-focus mode with `90 6y 41 FF` naming
its next free rotation socket, which it then skips. On the PTZOptics G2 bench
(three firmware builds, 2026-10-04) that socket is free in the engine's index,
including while another command executes on the other socket: a rejection
never names a socket the camera is executing on. `0x04` (cancelled) and `0x05`
(no socket) naming a socket answer only a cancellation packet for it. On a
datagram transport therefore:

- `0x04`/`0x05` naming a socket route only to the request whose emitted
  cancellation targets it, and are otherwise ignored.
- A rejection code (`0x01`, `0x02`, `0x03`, `0x41`) naming a socket that no
  live request owns and no exact `Socket` hold covers routes the target's
  unique unacknowledged ACK-bearing command. A target-wide `PreAck`,
  `InquiryUnkeyed`, or `AllResponses` hold has already discarded such a
  frame; a quarantined socket or the #714 two-candidate state leaves it inert.
- An error naming a socket owned by a live request routes that exact owner
  only when no other command on the target is unacknowledged; otherwise it
  binds to neither (#714).
- A named error is never an inquiry's. An inquiry is never allocated a
  socket: it receives no ACK and is answered on socket 0
  (`docs/visca_reference.md` §6.2), so its rejection takes the socket-0 form
  (§6.3). A live inquiry therefore never makes a named error ambiguous; the
  byte-stream ledger below applies the same rule.

A byte stream instead resolves every unsequenced frame through its
correlation ledger (below).

A conclusive rejection proves that the command did not start only before its
ACK. Once a command is acknowledged, no retry path writes it again, on either
envelope: a later camera error ends the request with
`Error::CommandFailedAfterAck`, which keeps the camera's exact error as its
source but reports `Terminal`/`Unconfirmed` and is never retryable, and a Sony
completion deadline after the ACK ends it with an unconfirmed `Timeout`
instead of a same-sequence resend. The Sony same-sequence retransmission the
crate relies on (docs/visca_reference.md §5.3 and the §11.5 "Sony
encapsulated UDP" row) recovers a lost message and lets the controller infer
whether the camera accepted it; an ACK already proved acceptance, and no cited
source says a camera ignores a repeated sequence once it is executing. A
relative move or preset recall that the camera accepted and then reported as
not executable may already have moved, so the caller reconciles it rather
than the engine replaying it (#795).

### Raw unconfirmed outcomes and strict recovery

Retry follows the same evidence boundary. Sony timeout recovery resends the
same logical request with the same sequence while it is unacknowledged. A
conclusive camera rejection before ACK, such as buffer-full or no-socket,
proves that the command did not start and may be retried under policy. A raw command that was successfully written but then
loses its ACK, completion, or cancellation resolution is different: replay
could perform a relative move or preset twice, while a naive continuation could
let a late reply bind to later work. Once an uncancelled command crosses the
deadline that makes its result unconfirmable, the decision is final: by default
the engine immediately emits `Error::UnsequencedCommandUnconfirmed`, removes
the live request, and retains only an inert keyed correlation hold through the
ambiguity interval (issues #671/#723). Late input cannot resurrect that request.
The scope follows the evidence: a command that owned S1 or S2 leaves only that
exact target/socket held; a lost ACK leaves `PreAck`; and a completion-only or
physically uncertain write leaves `AllResponses`. Simultaneous scopes are
distinct and expire independently; overlaps on one key extend to the later
deadline. A transient raw
receive fault while an *uncancelled* command is `AwaitingAck` is not itself such
a resolution loss: it neither replays nor fails the command by default, leaving
it to receive an ACK or reach its ACK deadline. If that deadline expires without
an ACK, the same immediate per-request recovery then applies. A command with
recorded cancel intent before socket assignment instead remains live in
`AwaitingAck` through its cancellation-ambiguity bound: an ACK may still assign
a socket and emit the cancellation. This is cancellation progress, not an
unconfirmed-command hold. Once a socket cancellation is emitted,
`AwaitingCancellationResolution` remains live until the original completion or
cancel terminal arrives, or its final resolution deadline expires.
A completion-only command (issue #700) that never receives its completion
follows the same rule: at its completion deadline it immediately fails
`UnsequencedCommandUnconfirmed` and leaves an inert `AllResponses` hold through
the ambiguity interval, never poisoning by default.
The session and every unrelated request keep running; the caller reconciles that
one command's camera effect rather than replacing the session.

### The raw byte-stream correlation ledger

On a raw-VISCA byte stream (TCP or serial) one rule correlates every
unsequenced frame (#795). A stream delivers every byte it accepted, in order
(a failed stream write poisons instead), and a camera answers each request it
reads with exactly one *first answer*, immediately and in read order: an
ACK-bearing command an ACK or a rejection, an inquiry its reply or a
rejection, a socket cancellation `0x04` or `0x05` naming its socket, a
`CompletionOnly` command a rejection (or, once it has run, its completion).
Raw answers carry no request identity, so per camera the engine keeps every
written request whose first answer has not arrived, in write order — the
*ledger*. An entry is added when the request's write starts and leaves when
its answer arrives.

Each first-answer frame resolves against the oldest entry that can legally
produce it:

| Frame | Resolves against |
| --- | --- |
| ACK | the oldest ACK-bearing command |
| Inquiry data | the oldest inquiry whose route the content allows |
| `0x04`/`0x05` naming socket `y` | the oldest cancellation for `y` |
| Rejection code naming a free, unheld socket | the oldest command (a rejection names the socket the camera allocated; an inquiry is never allocated one) |
| Socketless error | the oldest entry of any kind but cancellation (an execution error names its socket) |
| Socketless completion | the oldest `CompletionOnly` command, on a camera whose completions name their sockets (until that is learned, or on a camera that omits the nibble, only with no live socket holder or held socket) |

A completion naming a socket is never a `CompletionOnly` command's, which
earns no socket: it is its socket's live owner's, or the late completion of
a command that ended there. Frames that are not first answers keep their
exact rules: a completion or an error naming a live owner's socket is that
owner's (its completion or execution error), a completion or error naming a
socket held for an ended command is that command's late answer and releases
the hold, and anything else is ignored. A `CompletionOnly` command's single
answer is accounted by the ledger, so on a stream it leaves no time-bounded
`AllResponses` hold. Stream order makes the #714
two-candidate state exact: each ACK binds to the oldest open candidate.

A `CompletionOnly` command's answer carries no identity, so it is exclusive
on its camera for ordinary work: it starts only when no other command or
inquiry there is live, and until its completion or rejection arrives — even
after its own deadline ended it unconfirmed, as a debt that holds both lanes
— no inquiry and no ordinary command to that camera is written (they queue
under their own deadlines, and latch with its window). A socketless frame
while it is outstanding is then its own. Datagram sessions need no new rule:
there it was already exclusive while live, and once it ends a time-bounded
`AllResponses` hold keeps the target closed to response-bearing work other
than STOPs.

Only a STOP, never held back, can be written behind it, and only one: no
second STOP crosses a STOP waiting behind a `CompletionOnly` command that may
still be rejected. An accepted `CompletionOnly` command answers nothing until
it has run, so a socketless error behind it could be either one's: the
ledger's *dispute*. No length of time resolves it — a stall delays the STOP's
ACK without bound — so the error binds to neither, and the ledger keeps the
union of the debts either explanation leaves: the `CompletionOnly` command
owes only its completion (it can no longer be rejected either way), and the
STOP keeps owing its answer, *disputed*. While the dispute is tracked
nothing crosses a waiting request and no inquiry is written to that camera,
so only decisive evidence settles it, and the error then goes to the request
that sent it:

- The STOP's own ACK, when no other entry could have sent it, proves the
  command was rejected: the command fails with the error, and its debt is
  retired.
- The command's completion, or write order (a later request answered while
  the STOP's answer had not come), proves it was accepted: the STOP fails
  with the error.
- A frame that pays the STOP's debt while another request could have sent
  it is *handed off*: it was the STOP's late answer, or the next waiting
  request's own, so that request is disputed in turn and the frame is kept
  for it (with any completion or error naming the socket its ACK named,
  which also frees that socket). At most three hand-offs are followed.
- A disputed command waits one ambiguity interval past its ACK deadline for
  the proof before it ends unconfirmed.
- Once every request the dispute names has ended, and one ambiguity interval
  with it, the dispute goes *dormant*: it falls to the latch below and no
  longer holds anything back — the camera's command lane latches through the
  debts, its inquiry lane too while the `CompletionOnly` command is not
  proven accepted, so queued and new inquiries fail at once with
  `InquiryCorrelationLost` rather than waiting out their deadlines — while a
  late answer can still settle it and reopen the lanes.

When a second dispute involves the same requests (a STOP written at its wait
bound behind the disputed one, below, or a named rejection either could have
sent), the dispute is kept *accept-only*: the frames since may belong to
either explanation, so neither a STOP's answer nor write order can prove the
command rejected any more, but its completion still proves it accepted and
assigns the disputed frames (a `NoReply` command never completes, so its
dispute is dropped). Past three hand-offs the ledger stops tracking which
explanation is true: the requests involved
keep their union of debts, any that ends unanswered ends unconfirmed, and if
the command was in fact rejected its debt latches the camera's command lane
until the session is reopened (the endgame stated with
`CommandCorrelationLost`), while inquiries and STOPs flow. An extra debt only
discards a future answer; no frame is bound to a request that did not send
it while the camera keeps the assumptions below.

A `NoReply` command's hold (#700) holds back ordinary work, never a STOP, so
STOPs are written behind it. On a byte stream the ledger owes a `NoReply`
command's possible rejection until write order settles it (any later first
answer from that camera): no window proves it will not come, since a stall
can delay it past any deadline, and a late `z0 60 41 FF` must never reach a
later command (a movement command retried on it would be written twice).
Any later first answer settles it, as does a later request's completion or
execution error, so in a mixed workload it is settled by the next answer
from that camera. That entry latches no lane. While it is owed a
`CompletionOnly` command to that camera waits (its rejection could not be
told apart); once the `NoReply` command's window has ended with it still
owed — a camera sent only `NoReply` and `CompletionOnly` work, or stalled —
`CompletionOnly` commands there fail unwritten with `CommandCorrelationLost`
instead of timing out, until an answer (an inquiry's reply, a STOP's ACK)
settles it. A `NoReply` command is itself held back by an owed command
answer and refused with `CommandCorrelationLost` once that lane latches (or
at the debt cap); it is no longer "never blocked". A
socketless error with a STOP behind that entry is a dispute under the rules
above: it binds to neither; the STOP's own ACK, when nothing else could have
sent it, proves the error was the `NoReply` command's (which already ended
`Written`, so the error is discarded); write order proves it was the STOP's;
the hand-off cap and the latch endgame apply. Failing proof, the STOP ends
unconfirmed. A frame that pays a disputed debt, or passes a `NoReply`
command's entry, may be the own answer of each waiting request behind it
through the first that certainly still owes it, so each of those is disputed.

No rule above holds a STOP back for long. On a byte stream a queued STOP
waits for them (the #714 crossing, sibling STOPs of a halt, a dispute,
socket capacity) at most one ACK interval plus one ambiguity interval after
admission, and never past half its dispatch budget; then it is written
anyway. The ledger still binds every frame by write order, so the only cost
is decisiveness: a frame it makes ambiguous binds to neither request, and
the STOP may end unconfirmed. Every built-in profile (500 ms ACK, 1 s
ambiguity) thus writes a STOP within 1.5 s, against a halt budget of at
least its 30 s movement completion deadline. Only a `NoReply` or
`CompletionOnly` command's own local write in flight, which has no write
order yet, still holds it back.

A cancellation written for a command that has ended, after another command
was written that the camera then put in the same socket, reaches that later
command: the camera reads in write order, so by then the socket was the
later command's. Its `0x04` therefore ends that command unconfirmed (it may
have partly run) and frees the socket, so a later rejection naming the
socket is never taken for that command's error.

A request that ends without its first answer — its ACK or reply deadline,
its retry budget, a cancellation ambiguity deadline, or any other terminal —
keeps its entry as a *debt* if its write left (a staged write the due pass
drops reached nothing and leaves no entry). When the debt's answer arrives it
is discarded instead of reaching a later request. An owed ACK's command is
then executing on the camera, so the socket the ACK names (or the first free
one, for a socketless ACK) is held; a socketless completion while that hold
exists binds to no live command. An ACK disputed between two candidates
holds the socket it names the same way, since one of them executes there,
for the longer of the two candidates' completion deadlines (a candidate whose
write result has not come yet counts: its answer may overtake that result).
Either kind of ACK naming a socket the engine still indexes to an executing
command proves that command's completion was lost: it ends unconfirmed and
the socket is released (#721).

A tracked dispute is also decided where the engine learns who sent nothing
or who completed, and its frames are then assigned at once:

- A request it names that ends as a staged write that never left sent
  nothing. If it is the `CompletionOnly` command, the disputed error was the
  STOP's and each handed-off frame its step's (accepted); if it is a step,
  that step never sent the frame acceptance would make its own, so the
  command was rejected: it fails with the error and owes no completion, and
  the command lane does not latch. An accept-only dispute is the exception:
  its steps were chosen through entries already disputed, so a step's
  identity is inferred rather than proven, and an unwritten step only ends
  its tracking.
- A completion dropped while it was unknown whether the camera names its
  sockets in completions is the `CompletionOnly` command's once the camera
  is known to name them (`take_completion`). Like any completion of the
  command, it proves the command accepted.

A dispute is left untracked (its entries keep their union of debts, the
endgame above) only where neither explanation is proven: past three
hand-offs; when its candidate's answer arrives while another request could
have sent it and nothing can be handed off; when a second dispute touches a
`NoReply` command's dispute; when an accept-only dispute's candidate is
paid; or when one of its steps ends as a staged write that never left. An
owed inquiry reply also retains the target's `InquiryUnkeyed` hold through its
window and, once paid, leaves only the ordinary reply skew from that moment.
On a stream a pre-ACK command needs no time-bounded `PreAck` hold — its debt
replaces it, entry by entry — so none is installed, including when a camera
ACK displaces a stale socket owner or a held socket.

A stream's `Socket` hold — for a command that ended while executing, whose
owed ACK arrived late, or whose cancellation is unanswered — is likewise
released by evidence, never by time: a frame naming the socket (the
command's late completion or execution error, or its cancellation's
`0x04`/`0x05`, which also pays the cancellation's debt) or a camera ACK
naming the socket for another command. A stall can outlast any deadline, and
until the camera has finished there, an error naming that socket is the held
command's. The hold takes no command capacity (a camera still busy there
allocates the other socket or answers buffer-full). One residual has no
evidence: if the held command's end produced no frame naming the socket (its
completion was socketless), the camera may allocate the socket again, and
the next rejection naming it is misread once as the held command's late
error — but it releases the hold, and the waiting command it may have
rejected is disputed rather than left owing an answer that came. Likewise,
until a camera is known to name its sockets in completions (below), an
executing command's socket may already be free: a rejection code naming it
that a waiting command could have earned disputes that command instead of
failing the owner. For a camera that omits the nibble, a `CompletionOnly`
command waits for a held socket's deadline, since the held command's
completion would be indistinguishable from its own.

Write order also *settles* debts. A camera answers in read order, so a first
answer resolved against the entry written at `o` proves that every request
written before `o` was answered already: a debt older than `o` will never be
paid and is retired at once. A later command's ACK thus retires the debt of an
inquiry the camera never answered, and an inquiry's reply ends a cascade of
STOP debts in which each STOP's ACK had paid the previous STOP's debt. A
`CompletionOnly` debt is the exception: an accepted one answers only once it
has run, not in write order, so only its completion retires it. Its
rejection would have come in write order, though, so from then on it owes
only that completion, and an error is no longer ambiguous with it. A
completion settles too, time-independently: a completion (or an execution
error naming its socket) of the request written at `o` follows that
request's own first answer, and so every first answer written before `o`.
A retired inquiry reply leaves the ordinary reply
skew, as a paid one does. A run that merged past `o` is kept whole: keeping a
debt only discards a future answer, never misattributes one.

A debt's window is the owing request's ambiguity interval (an inquiry's is at
least the reply skew). Inside it, the debt's lane waits: same-target
inquiries for an owed reply, ordinary commands for an owed command answer
(an `Urgent` STOP is still written under the ordinary `PreAck` bypass rule;
a `NoReply` command waits too, its possible rejection being owed in turn).
If the window ends with the debt unpaid, only that lane *latches*: it no
longer wakes the owner, and its queued and new work fails unwritten —
inquiries and `CompletionOnly` commands with `Error::InquiryCorrelationLost`,
ordinary ACK-bearing, `CompletionOnly`, and `NoReply` commands with
`Error::CommandCorrelationLost` (both `Terminal`/`NotAccepted`, not
retryable, `requires_new_session() == false`) — until the answer arrives, a
later first answer from that camera settles the debt, or a new session is
opened. A camera that never answers an inquiry thus keeps accepting commands,
and the first of them it answers reopens its inquiries; a latched command lane
reopens on the reply to any later inquiry or the ACK of a later STOP. STOPs
(including the owner halt), other cameras, and the session are never
blocked, and nothing here poisons the
session: a poison would end STOP delivery to every camera on it. A
cancellation's debt never latches anything; it only ensures that its late
`0x04`/`0x05` reaches no later request.

A debt is never dropped, and nothing owed is forgotten by time: forgetting
one would let its late answer reach a later request. Instead, at 32 debts
every lane of the camera latches, whatever the windows say, until debts are
paid; STOPs are still written (`NoReply` commands are refused, their
possible rejections being owed too), and nothing is poisoned. One stall leaves a handful of debts — ordinary commands wait while
one is owed, inquiries are single-flight, and a halt writes at most three
STOPs — so 32 is reached only by a camera that answers nothing at all.

Memory stays bounded without dropping anything. Debts are indistinguishable
once owed, so consecutive debts owing the same answer form one *run* with a
member count, at its oldest member's position, paid oldest first — exactly
the order separate entries would pay in. "Consecutive" is judged among
entries an answer could confuse: cancellations for one socket among
themselves, every other kind among the others (a socketless error may answer
any of them). Past the cap only urgent ACK-bearing commands (STOPs) and
cancellations are written; their debts join the run behind them, so a camera
that answers nothing keeps one run of STOP debts however many halts it is
sent. The queue of one camera therefore never exceeds `32 + 5A + 12` entries,
where `A` is the requests it can have admitted at once (the session's ordinary
capacity plus its control reserve): every write other than a STOP happens
below the cap, behind fewer than 32 runs, after which each request then live
splits at most one more run and live entries at most two each, and a
tracked dispute pins at most five entries that may split one run each. The
engine checks the bound as an invariant.

The ledger rests on four camera facts, and guarantees under them that every
frame binds only to the request that sent it:

- A socketless error (`z0 60 02 FF`, `z0 60 03 FF`, or any other socket-0
  error) is a first answer, given before a command is accepted. The VISCA
  reference lists socket 0 only for the syntax and buffer-full errors
  (`docs/visca_reference.md` §6.3), and an inquiry, which never takes a
  socket, answers on socket 0 (§6.2).
- An error for an executing command names its socket (`z0 6y 04 FF`,
  `z0 6y 41 FF`, §6.3), and a rejection of a command that earns a socket
  names the socket the camera allocated, never a busy one: the PTZOptics G2
  bench (2026-10-04) names the next free socket of its rotation in every
  rejection. A rejection of a shape that earns no socket (`CompletionOnly`,
  `NoReply`) is socketless.
- Every written request receives exactly one first answer (only a
  `CompletionOnly` command's completion may never come).
- A camera whose completions name their sockets (`z0 5y FF`, §6.1) names
  them in every completion of a socketed command. This is learned per camera
  from its completions: a completion naming a socket teaches it; a socketless
  completion while no `CompletionOnly` command is outstanding proves the
  camera omits the nibble, for the rest of the session (unless it is the late
  completion of a `CompletionOnly` command a dispute proved rejected). Until
  learned, a socketless completion takes the conservative path (only a sole
  socket holder or the `CompletionOnly` command with nothing executing or
  held may receive it); one dropped that way is applied to the oldest
  `CompletionOnly` command still owed once the camera is known to name its
  sockets.

What a violation costs:

- A socketless execution error, or a second answer to one request, binds to
  the oldest legal entry: it pays a debt (discarded), or reaches a live
  request as its answer — a false rejection, or an ACK for a command the
  camera has not read.
- A request the camera never answers stays owed until a later first answer
  settles it; meanwhile it pays itself with the next answer of its class,
  so requests of that class written behind it end unconfirmed, one step
  behind (and a waiting one can receive a later request's answer), until an
  answer of another class settles the debt, its lane latches, or the session
  is reopened. A `CompletionOnly` command whose completion never comes
  latches the command lane until the session is reopened.
- A camera that names its sockets in some completions and not others is
  treated as omitting them once a socketless one arrives with no
  `CompletionOnly` command outstanding; until then, a socketless completion
  of a socketed command arriving while a `CompletionOnly` command is
  outstanding completes that command instead.

A camera that omits the socket in its completions keeps the assumptions but
costs availability: whenever two commands execute at once, or a socket is
held, its socketless completions cannot be attributed and are dropped, so
those commands end unconfirmed and a `CompletionOnly` command's completion
can be lost to its debt (latching the command lane). In the randomized model
on a hostile link, about half of its STOPs end unconfirmed this way and most
sessions that send `CompletionOnly` commands end with that lane latched; a
camera that names its sockets (the PTZOptics G2 sends `90 51 FF` and
`90 52 FF`) has neither cost.

Datagram transports cannot rely on order (a datagram may be lost) and keep
the time-bounded holds; the Sony envelope correlates by sequence.

### Sony full-width and lower-16 sequence identity

Outgoing Sony frames always carry a complete 32-bit sequence, and a retry
reuses that exact logical sequence instead of allocating another one. On
receive, a nonzero upper half is unambiguously `FrameSequence::Full32` and must
match one live, target-compatible full-width owner exactly.

Some cameras return only the low 16 bits and zero the upper half. Because a
genuine small 32-bit sequence has the same wire spelling, the decoder calls
that case `FrameSequence::MaybeTruncated`; it does not guess which provenance
the camera intended. The engine searches the target-compatible live owners
whose low 16 bits match and accepts the response only when that set identifies
one request. No candidate is unmatched; two or more candidates are
`AmbiguousLower16Sequence` and the frame is inert. A later exact full-width
reply can retire one colliding owner, after which the same lower-16 value may
again be unique. Registering a retry of one request under the same sequence
does not manufacture a collision.

This full-width-first, unique-lower-16-only rule is the complete correlation
decision from review item 16. Socket, FIFO, admission order, and temporal
recency never break a sequence collision.

Fixed-format ACK, completion, error, and network-change frames are classified
only at their exact lengths. A known fixed prefix with trailing bytes is
malformed, not an `Unknown` response. For fixed ACK, completion, and error
forms, the socket nibble is also strict: `0` decodes as the valid socketless
compatibility form, `1` and `2` decode as S1/S2, and `3..=15` is malformed.
The variable data-reply form is reserved for socket 0 with more than three
bytes; the canonical three-byte `z0 50 FF` socket-0 completion remains valid.

That strict classifier verdict is not a session verdict. A frame the framer
already delimited at an `FF` boundary but that the classifier rejects is a
*malformed frame to discard* on a byte stream — recorded as
`Ignored(MalformedFrame)` and counted in
`OwnerMetrics::ignored_malformed_frames` — exactly as a datagram already
discards it and as 1.x logged-and-continued over the same padded ACKs, vendor
socket nibbles, RS-485 echoes, address-set replies, and truncated frames. The
session stays `Running` and the frame's real reply still settles the outstanding
command. Only the framer *losing its position* — cumulative buffer overflow, or
a boundary-free read growing past `max_buffer_size` — is a genuine framing
failure that still poisons a stream. By the same decoupling, a single stream
read that decodes more than `frames_per_receive` frames stops at the limit and
leaves the remaining complete frames buffered for the next receive (the framer
retains bytes across reads, bounded by `max_buffer_size`), rather than failing
`ResponseTooLarge` over a large-but-valid burst. A single-target IP session,
which carries no routable source, accepts any `0x9y..=0xFy` reply source and
attributes it to its sole target, so a camera configured with a non-default
chain address still settles its command (#590/#598); the strict `0x90` check is
kept only when more than one target is registered.

At an ambiguity-expiry boundary on a byte stream, already complete buffered
frames are classified and correlated before the due release. Retained partial
bytes are not frames and are not decoded speculatively. Unless the due release
also includes the broad target-response `AllResponses` hold from a `NoReply` or
`CompletionOnly` terminal, a completion or error prefix naming S1/S2 may remain
buffered across another socket's release only when it names a still-live,
non-releasing owner of that exact target/socket; matching stale-socket evidence
may be discarded, and serial input for another target is preserved. That broad
hold instead discards every response-shaped partial prefix for its target,
including source-only, ACK, socketless `0x50`/`0x60`, and named terminal
prefixes; noncorrelating evidence and another target's serial input are
preserved. A source-only prefix, an ACK prefix (whose nibble is an assignment
preference rather than ownership), and socketless `0x50`/`0x60` prefixes remain
ambiguous for narrower inquiry/pre-ACK/unkeyed or socket-scoped releases. They
keep the old correlation scope alive while the owner awaits input for one
engine-owned grace interval: the lesser of the configured read timeout and 100
ms (#713). A tail arriving inside that interval is decoded before release. If
the deadline expires first, the orphan prefix is discarded, one malformed-frame
diagnostic is recorded, and the session remains `Running`; only framer overflow
or a decoder that cannot perform the requested discard poisons the stream.

### Blocking worker and the urgent safety lane

A blocking session's worker runs the same coordinator, turn logic, and engine
policy as the async owner (D24, #780). Blocking submission therefore means
admission, exactly as on async: a returned operation handle names an admitted
request, and a transport write failure is reported through that operation's
outcome. Socket-capacity contention and an earlier scheduler winner queue the
request until it is eligible; there is no blocking-only first-write boundary,
pre-ACK drain, or busy-transport rejection.

The worker polls the boundary lanes in the coordinator's order and reads the
transport in bounded slices of at most 10 ms (1 ms when a later source is
already ready). This bounds the time one idle read keeps the worker away
from its boundary lanes. Queued boundaries and writes can add to a STOP or
control request's total latency; STOP dispatch also waits for protocol
eligibility. A custom transport calling `close` from its own worker requests
shutdown and receives `InvalidState` immediately, leaving the worker available
for an external caller to join. An idle read is reported only when the
configured `read_timeout` has elapsed
without data, and the coordinator paces an eagerly idle or faulting transport
while every other source stays selectable.

Measured cost and latency (`tests/blocking_worker_resources.rs`, release
build, UDP loopback, 4-core Linux VM, October 2026; the harness is ignored by
default because the figures depend on the host):

| Measure | Result |
| --- | --- |
| Threads | exactly one per session; `close` joins it |
| Idle CPU | about 0.1% of one core per session |
| Resident memory | about 94 KiB per idle session |
| `close` latency | 12–20 ms (one read slice plus the join) |
| STOP call to bytes at the camera, while another thread waits on a running zoom, with no admission backlog and pacing already elapsed | p50 5.6 ms, p99 10.7 ms |

An intrinsically `Urgent` stop may cross one raw positional candidate after
command pacing. The resulting explicit two-candidate state makes every
unsequenced ACK/error ambiguous, so it binds to neither request and either
handle may later report `UnsequencedCommandUnconfirmed`; that uncertainty does
not retract the stop bytes already sent to the camera. A `CompletionOnly`
successor still requires target idleness (#714).

This ordering is what permits a detached observer or a dropped subscription to
miss an event without losing an already-applied state update.

## Engine phases and the transition table

The engine keeps exactly one authoritative entry per admitted request. Each
entry carries a protocol `Phase` and a cancellation substate (`CancelState`);
both are `pub(crate)` in `src/runtime/engine/types.rs`, so they never reach
rustdoc and are documented here instead. The tables below are derived from the
transition functions in `src/runtime/engine/mod.rs` and describe the settled
post-#671 behavior, not an aspirational one.

Raw inquiry replies have no wire identity. The production Raw adapter therefore
uses one live inquiry per target. A matched reply exhausts that inquiry's
response and releases the lane immediately, with no hold. Only an
uncertain release — terminal error/timeout or retry release/requeue — retains a
target-local hold for the profile's `raw_inquiry_reply_skew`, which is validated
at no more than its minimum inquiry spacing. That narrow hold blocks only a
same-target inquiry; ACK-bearing commands, including `Urgent` stops, remain
eligible and their ACK/completion evidence is still routed normally. It filters
stale unkeyed inquiry data/reply and socketless-error evidence while live
attributable ACK/completion (including a uniquely attributable socketless
completion) and exact named socket terminals remain eligible. At the exact
skew deadline complete input is correlated first; the byte-stream ambiguity
rule above governs retained input before the due pass releases a successor.
Other targets and all command work remain independently eligible (#712).

On a raw byte stream an inquiry that ends *unanswered* after its write is
instead never resent on a reply timeout: a stream loses nothing it accepted,
so the reply is late, not lost (a multi-second stall on the PTZOptics G2 bench
on 2026-10-04 delivered it long after any skew hold, where it would have
bound to the next same-target inquiry as silently wrong data). It fails at
its reply deadline with the usual `Terminal`/`FailedConclusively` timeout (it
had no effect, and its reply can complete no request), and its reply becomes
a ledger debt (above). Everything that needs an inquiry to a camera whose
inquiry lane has latched fails with `InquiryCorrelationLost`: `settled()`
observed-stable settlement polling (as `SettlementObservationFailed` with
that source), `is_moving`, and `wait_until_idle`. Datagram behaviour is
unchanged, and #671's per-request raw deadline verdict holds: no inquiry
timeout poisons the session.

### Protocol phases

| `Phase` | Meaning |
| --- | --- |
| `Ready` | Admitted and queued; nothing written yet. Holds a lazy-deletion `QueueTicket`. |
| `Sending` | The request's transport write is in flight; carries its `TransmissionId`. |
| `AwaitingAck` | A command was written and awaits its ACK (deadline = sent + ack). |
| `AwaitingCompletion` | A completion-only raw command (`RawReplyShape::CompletionOnly`, issue #700) was written and awaits its completion with no ACK and no socket (deadline = sent + completion). It holds its target's command channel exclusively and has no ACK-timeout path. |
| `Executing` | The ACK assigned a socket; awaits completion (deadline = ack + completion). |
| `AwaitingReply` | An inquiry was written and awaits its reply (deadline = sent + inquiry). |
| `Backoff` | A retryable rejection or timeout scheduled a retry; re-enters `Ready` at `ready_at`. |
| `AwaitingCancellationResolution` | A socket-owned command with active cancellation state awaits the original completion or protocol-cancel terminal through the later of the retained completion and ambiguity deadlines. This phase is never used with `CancelState::None`; non-cancellation uncertainty lives in the keyed hold table after terminal removal. |

### Cancellation substates

| `CancelState` | Meaning |
| --- | --- |
| `None` | No cancel intent. It never marks a correlation hold. |
| `Requested` | Cancel intent recorded before a socket was assigned; suppresses every later retry. |
| `Sending` | A cancel frame's write is in flight. |
| `AwaitingTerminal` | A cancel frame was written; awaits the original completion (`Completed`) or the protocol-cancel terminal (`Cancelled`). |
| `ObservationFailed` | A datagram cancel write failed; the token was resolved with that error while the original request stays live. If it is `Executing`, its exact socket keeps the original completion routable through the command's completion deadline. |

### Transition table

| Input / state | Transition or result |
| --- | --- |
| Admit while `Running`, within capacity, target registered, policy compatible | Allocate a non-colliding `RequestId`, create one `Ready` entry with `CancelState::None`, enqueue it, and emit `Admitted`. No write happens here. |
| Admit over ordinary capacity | Reject with `Error::RuntimeQueueFull`; no id, entry, observer, or queue slot is created. |
| Admit an urgent typed STOP into its target's control reserve | Admit while that target's reserved entries are below its reserve (one per supported typed STOP); otherwise reject with `Error::ControlReserveExhausted`. Only an urgent request may hold a reserved slot (D26, #778). |
| Admit with the id/generation space exhausted | Reject with `Error::RuntimeIdentityExhausted`. |
| Admit to a non-`Running` session | Reject with the session's terminal error (or `Error::RuntimeShutdown`). |
| Select a `Ready` request | Transition to `Sending`, allocate one `TransmissionId`, emit exactly one request `Transmit` (Sony carries its retained sequence; raw carries none). |
| Ordinary dispatch behind a live raw `AwaitingAck` candidate or inert `PreAck` hold | Queue to the evidence boundary: the candidate's ACK or the hold's deadline; the old terminal request cannot be rescued (#714/#723/#724). |
| `Urgent` raw command behind one positional candidate | Cross the single-candidate gate after command pacing when socket capacity remains. This is the sole two-candidate exception (#714). |
| Successful command send (`AckThenCompletion`, the default) | Record any Sony sequence and transition to `AwaitingAck`. |
| Successful command send (`CompletionOnly`, issue #700) | Transition straight to `AwaitingCompletion` (no ACK phase, no socket); apply any completion that raced the write result and drop any spurious raced ACK. |
| Successful command send (`NoReply`, plain raw only) | Finish the plain `execute()` after the local write succeeds; this is not protocol application and cannot create an operation handle. Retain the bounded broad `AllResponses` hold before same-target raw response-bearing command or inquiry work may start; a STOP is never held by it (#795). Another `NoReply` may write and extend that key. |
| Successful inquiry send | Record any Sony sequence, transition to `AwaitingReply`, and take a per-target FIFO position for a raw inquiry. Production Raw policy has one live inquiry per target, the precondition for its unkeyed-reply hold; a socket-owned command may coexist. |
| Failed command send, datagram transport | Terminally fail that one request with the exact transport error; every other entry keeps running. |
| Failed command send, stream transport | Poison the session (`Error::StreamPoisoned`) and resolve every active entry. |
| Compatible failed stream write sampled after the request's total budget | Poison before applying per-request late-result policy: the stream seam cannot prove that no partial frame reached the wire (#724). |
| Request still `Sending` when its total retry budget expires | Raw active command: immediately fail unconfirmed and install the applicable keyed hold (or strict-poison); raw single-flight inquiry: install its target-only reply-skew hold and fail (nothing is owed even on a stream: no write is in progress during the due pass, so the staged transmission is dropped); Sony: fail without registering late sequence metadata. A write result sampled exactly at the deadline remains input-first. |
| Request write result sampled strictly after its total retry budget | Apply the same expired-`Sending` policy before the result can register correlation, consume a deferred frame, report `Written`, or replace a retained retry cause; except that on a raw stream a successful write makes the request's first answer a ledger debt (rows below). Cancellation writes use their separate ambiguity lifecycle. |
| ACK in `AwaitingAck` with a free socket | Assign the socket and transition to `Executing`; if cancel intent is `Requested` on a supported target, emit one socket cancellation. An ACK covered by an inert `PreAck`/`AllResponses` hold is ignored and cannot resurrect its terminal owner (#723). |
| Unsequenced ACK/error while two raw positional candidates are open | Ignore it as ambiguous and bind it to neither candidate; never use admission order or recency (#714). |
| Raw ACK naming a busy or quarantined socket | Treat the camera's named socket as authoritative: immediately fail a stale live owner, or downgrade an inert exact-socket hold, to its original-deadline unkeyed `PreAck` hold; assign the named socket to the uniquely resolved successor (#721/#723/#750). |
| Raw datagram error naming a free, unheld socket | A rejection code (`0x01`/`0x02`/`0x03`/`0x41`) routes the target's unique unacknowledged ACK-bearing command (the camera's rejection in the socket it allocated, e.g. G2 `90 6y 41 FF`; an inquiry, answered on socket 0, never names one); ignore it while the socket is quarantined or two candidates are open, and ignore `0x04`/`0x05`, which answer only a cancellation (#795). |
| Raw datagram error naming a socket owned by a live request | `0x04`/`0x05`: route the owner only if its emitted cancellation targets that socket. Otherwise route the exact owner unless another command on the target is unacknowledged: then bind to neither (#714). A live inquiry never competes: its error is socketless (#795). |
| Unsequenced frame on a raw byte stream | Resolve it through the correlation ledger: the oldest entry that can legally produce it (see "The raw byte-stream correlation ledger"); a live entry's request receives it, a debt discards it. A completion or error naming a live owner's socket is that owner's; one naming a held socket releases the hold (#795). |
| Written raw byte-stream request ends without its first answer | Its ledger entry becomes a debt for its ambiguity window (an inquiry's is at least the reply skew); a staged write that never left leaves no entry. An owed inquiry reply retains the `InquiryUnkeyed` hold; a pre-ACK command installs no `PreAck` hold (#795). |
| Ledger debt paid | Discard the frame. An owed ACK holds the socket it names (or the first free one) for its command's completion deadline; an owed reply leaves the ordinary reply skew from now (#795). |
| Ordinary command or inquiry dispatch while its lane owes an answer inside its window | Queue; an `Urgent` ACK-bearing STOP may still write when no command is unacknowledged (#795). |
| Ledger debt unpaid when the due pass reaches its window end | Latch that lane: fail its queued `Ready`/`Backoff` work and reject new work, unwritten, with `InquiryCorrelationLost` (inquiries, `CompletionOnly`) or `CommandCorrelationLost` (ordinary ACK-bearing, `CompletionOnly`, `NoReply`), `Terminal`/`NotAccepted`. The latch neither expires nor wakes the owner; STOPs, other targets, and the session are unaffected; the debt's answer, or a later first answer that settles it, reopens the lane (#795). |
| Disputed raw stream frame (a socketless error behind a `CompletionOnly` entry that may still be rejected or a `NoReply` command's owed entry, or any frame behind a disputed waiting request, with another candidate behind it) | Bind to neither; keep the union of the debts either explanation leaves: the `CompletionOnly` command owes only its completion, the other candidate keeps owing and is disputed (never crossed; settled by its own answer or by write order) (#795). |
| Sequenced Sony ACK naming a busy socket | Use the target's other free socket when available for #620/#682 compatibility; otherwise remain inert as `Ignored(SocketConflict)`. |
| ACK while still `Sending` | Latch it once as a deferred ACK, applied when the send result lands. |
| Completion in `Executing`, including after cancellation ambiguity elapsed | `finish` with `RuntimeOutcome::Applied`; exact socket ownership keeps it attributable through the completion deadline, and a retained cancellation observer maps this to `Completed` (#724). |
| Exact completion in `AwaitingCancellationResolution` before its phase deadline | `finish` with `RuntimeOutcome::Applied`; the request is still open while its owned socket is retained. |
| Completion in `AwaitingCompletion` (issue #700) | `finish` with `RuntimeOutcome::Applied`, regardless of any socket nibble the vendor frame echoes (on a raw byte stream only a socketless completion resolves to it, #795); the resolver already established it as the sole completion-only candidate on the target. Retain the bounded broad `AllResponses` hold before same-target raw response-bearing command or inquiry work starts; a later `NoReply` may only extend that key. |
| Inquiry reply in `AwaitingReply` | `finish` with the attributed payload. A matched Raw reply installs no hold and releases the single-flight lane immediately (#712). |
| Uncertain Raw inquiry response-correlation release | A terminal error/timeout or retry release/requeue retains the profile's short `raw_inquiry_reply_skew` before another same-target inquiry may send. ACK-bearing commands, including `Urgent`, are never gated by this inquiry-only hold. It filters stale unkeyed inquiry data/reply and socketless-error evidence, while live attributable ACK/completion (including a uniquely attributable socketless completion) and exact named socket terminals remain eligible (#712). |
| Raw inquiry reply deadline or total budget in `AwaitingReply`, or a successful write sampled after the budget; stream transport | Never resend. Fail the inquiry with `Timeout` (`Terminal`, `FailedConclusively`) retain the target's `InquiryUnkeyed` hold, and record that the target's inquiry lane owes one reply (its window: the request's ambiguity window, at least the reply skew). Same-target inquiries wait; ACK-bearing commands are not gated beyond the ordinary inquiry hold. Datagram keeps the skew hold and the inquiry retry policy (#795). |
| Raw inquiry frame at exact reply-skew expiry | Complete input wins over the due pass. Stale unkeyed inquiry data/reply and socketless-error evidence remain filtered, while live attributable ACK/completion (including a uniquely attributable socketless completion) and exact named socket terminals remain eligible. Retained partial-byte ambiguity follows the engine-owned time grace before an orphan is discarded (#713). |
| Retryable conclusive rejection before ACK (buffer-full `0x03`/`0x05`, movement `0x41` for a non-STOP movement request), no cancel intent | Increment the bounded attempt and enter `Backoff`. A typed STOP refused with `0x41` is terminal with `Error::CommandNotExecutable`: the refusal has a standing cause (a focus STOP under auto-focus) and the caller needs the verdict at once. |
| Camera error after ACK (`Executing` or `AwaitingCancellationResolution`), either envelope | Never retried: `finish` with `Error::CommandFailedAfterAck` wrapping the camera's exact error (`Terminal`/`Unconfirmed`, not retryable). The acknowledged command may already have moved (#795). |
| Completion deadline in `Executing`, Sony envelope, no cancel intent | `finish` with `Timeout` (`Terminal`/`Unconfirmed`); never resend. The same-sequence retransmission in docs/visca_reference.md §5.3 recovers a lost message, and an ACK already proved acceptance (#795). |
| Retryable rejection with cancel intent | Suppress retry and `finish` with `Cancelled`, because no executing attempt exists. |
| `0x04` command-cancelled terminal | `finish` with `Cancelled`. |
| ACK timeout, Sony envelope, retryable | Retry within policy on ACK-capped backoff, replaying the exact sequence. |
| Transient receive fault while a Sony command is `AwaitingAck` | Retry every such eligible command independently using uncapped backoff and the retained sequence; unrelated requests and the session remain live. |
| Transient raw receive fault in `AwaitingAck` with no recorded cancel intent | Default: leave the unacknowledged command in `AwaitingAck`; it is neither replayed nor failed on the fault, and a later ACK may still assign its socket. If its ACK deadline subsequently expires without an ACK, apply the raw unconfirmed-recovery row below. A command with recorded cancel intent instead remains live through its cancellation-resolution bound. |
| Raw ACK deadline expiry without an ACK, completion loss, a completion-only command's completion deadline in `AwaitingCompletion` (issue #700), or retry-budget expiry in an active raw phase | Default: immediately `finish` with `Error::UnsequencedCommandUnconfirmed`, remove the request, and install a keyed `PreAck`, `Socket`, or `AllResponses` hold through the ambiguity interval. Late frames covered by that hold are inert (#671/#723/#724). On a raw byte stream the ledger owes a pre-ACK answer instead of a `PreAck` hold, and a `Socket` hold lasts until evidence releases it (#795). |
| Keyed raw hold expiry | Release only the target/scope key whose deadline elapsed and reconsider queued work. No terminal event occurs here because the request was already resolved; independent simultaneous holds remain in force. The one exception is an `InquiryUnkeyed` hold retained for an owed raw stream inquiry reply, which latches with its ledger debt instead of releasing. |
| Cancellation-resolution deadline expiry | `finish` with `Error::UnsequencedCommandUnconfirmed` for a raw session or `Error::CancellationUnconfirmed` for a Sony session. This is the end of a still-live cancellation lifecycle, not release of a `CancelState::None` quarantine marker. |
| A transient raw receive fault in `AwaitingAck` with no recorded cancel intent, or any raw unconfirmed-recovery trigger above, under the `strict_unconfirmed_poison` opt-in | Poison the session and report `Error::StreamPoisoned`, restoring the pre-#671 behavior. A recorded cancel instead follows its live cancellation-resolution path and poisons only if that deadline remains unconfirmed. |
| Retry becomes eligible (`Backoff` → ready) | Return to `Ready` and dispatch through the ordinary capacity and pacing gates. |
| Owner halt fence over a `Ready` or `Backoff` request, a retry decision for a fenced request, or admission of a fenced submission | `finish` (or reject admission) with `Error::MotionSuperseded`. Its context is `PreAdmission`/`NotAccepted` for a rejected submission; otherwise `Terminal`, with `NotAccepted` when no written attempt can have taken effect (never written, or every written attempt rejected before ACK) and `Unconfirmed` when an earlier attempt lost its ACK, completion, or receive path (#795). |
| Cancel in `Ready` or `Backoff` | Remove without I/O; emit `CancellationRecorded`, then `finish` with `Cancelled`. |
| Cancel in `Sending` (supported target) | Record `Requested` intent; the send result drives the next state. |
| Cancel in `AwaitingAck` (supported target) | Record `Requested` intent, suppress retries, and wait for socket assignment or the ambiguity deadline. |
| Cancel after a raw request has terminalized into a keyed hold | Return `Ignored(UnknownRequest)`. The hold is inert correlation state, not a cancellable request, and its deadline is unchanged. |
| Cancel in `Executing` (supported target) | Record intent, emit one socket cancellation, and retain the original completion correlation through the command's completion deadline; cancellation ambiguity does not shorten exact socket correlation (#724). |
| Cancel after transmission on a target without socket cancellation | The cancellation observation fails with `Error::NotSupported`; record no intent, send no frame, and leave the original request active. |
| Cancel of an inquiry | The cancellation observation fails with `Error::InquiryNotCancelable`. |
| Repeated cancel from the same handle | The owner observes the handle's one existing intent and gives the engine no second input (#777). |
| Duplicate cancel reaching the engine | `Ignored(DuplicateCancellation)`. |
| Successful cancel send | Transition to `AwaitingCancellationResolution` with `AwaitingTerminal`; wait for the original completion (`Completed`) or the protocol-cancel terminal (`Cancelled`) through the later of the retained completion and ambiguity deadlines. |
| Failed datagram cancel send | Resolve the token with the error (`ObservationFailed`), retain the original request and routing, and never retry that cancel for the same socket assignment. An `Executing` original remains governed by its completion deadline, not the earlier cancellation ambiguity deadline (#724). |
| Failed stream cancel send | Poison the session and resolve every entry and observer. |
| Observer detach or timeout | No engine input and no protocol transition. The expired wait reports `ObservationTimeout` (#783). |
| Close / shutdown / poison | Resolve every active entry once, in admission order, with the distinct terminal error; clear every index and transmission; reject or drain boundary admissions deliberately. |

Non-retryable camera and transport errors stay exact even when cancel intent
exists. `Cancelled` is never used merely because a caller stopped waiting or the
engine lost certainty. When one input emits both a cancellation acknowledgement
and an immediate terminal result, `CancellationRecorded` precedes `Terminal`.

The four terminal conditions are distinct errors, and `Error::requires_new_session()`
separates a dead session from a recoverable one:

| Terminal condition | Error | `requires_new_session()` |
| --- | --- | --- |
| The peer closed the connection | `ConnectionClosed` | `true` |
| The stream position became unknowable, or the strict opt-in poisoned an unconfirmable raw command | `StreamPoisoned` | `true` |
| A sent raw command's outcome cannot be correlated (default per-request mode) | `UnsequencedCommandUnconfirmed` | `false` |
| The application shut the session down | `RuntimeShutdown` | `false` |

## Operational invariants

The engine and its serialized owner uphold the following invariants in every
supported configuration. They are the properties the deterministic engine, the
bounded owner boundaries, and the `#![forbid(unsafe_code)]` crate attribute
exist to guarantee.

- Runtime mutation is serialized per transport.
- Every admitted request has exactly one authoritative entry until safe terminal removal.
- Every request and cancellation transmission has one active identity and one result.
- Every request reaches at most one terminal engine transition.
- Every retained operation or cancellation observer resolves at most once.
- Every target/socket pair has at most one active owner.
- No retry path writes a command again after its ACK.
- Every live raw byte-stream ledger entry belongs to a request that still
  awaits that answer; the ledger exists only on a raw stream session, a frame
  resolves only against an entry that can legally produce it, and a debt never
  blocks a STOP or poisons the session.
- Every `AwaitingCompletion` entry is completion-only, and every raw
  completion-only/no-reply positional candidate owns its target lane
  exclusively; the urgent two-candidate exception cannot cross it.
- Every keyed raw hold is inert: it can filter late input and gate successors,
  but it cannot consume a response, accept cancellation, or recreate a request.
- Every `AwaitingCancellationResolution` entry has non-`None` cancellation
  state and owns its exact target/socket correlation.
- Every sequence owner is an active, target-compatible, and phase-compatible entry.
- Stale queue, retry, deadline, correlation, admission, or transmission tickets cannot send or resolve work.
- A cancel transmission is emitted at most once for one socket assignment.
- Observer removal never mutates protocol state or releases protocol capacity early.
- Malformed, duplicate, stale, reordered, and unsolicited frames cannot panic or mutate unrelated state.
- No user decoder, callback, subscriber, transport I/O, or sleep runs while mutable engine state is borrowed.
- The owner copies or moves each effect out of the engine and ends the mutable engine/observer borrow before transport I/O, borrowing the engine again only to apply the identified result.
- No slow observer or subscriber can block protocol progress.
- Request-identity wraparound cannot alias active or quarantined work.
- All boundary, engine, observer, diagnostics, framing, and subscription memory has a documented bound.
- Stream poison, close, and shutdown cannot leave a retained waiter blocked indefinitely.
- A fresh session contains no state from a poisoned session.
- The runtime code implementing this architecture contains no `unsafe`.
- A returned blocking operation handle names a request whose initial transport write succeeded.
- Target-local state changes occur only from exact `AppliedStateEffect` delivery and never depend on an observer.
- Built-in encoding and per-transmission framing allocate nothing; lifecycle allocations remain fixed and admission-bounded.

## Scheduling and async source arbitration

Each request owns an intrinsic `ControlClass`. Typed stops and protocol
cancellation are `Urgent`; that is safety metadata rather than caller QoS.
Camera handles and individual calls may choose a `SubmissionClass` for ordinary
traffic (`Background`, `Normal`, or `User`). The public QoS type has no urgent
variant, and both override forms preserve an intrinsically urgent request.
Within each effective class the owner retains admission order; class selection
only chooses the next eligible queued write and never interrupts in-flight I/O.
Urgency bypasses ordinary admission backlog, not command-to-command physical
pacing. A preceding inquiry does not start the urgent command clock (#712).
Owner-issued socket cancellation is selected before ordinary ready work when
eligible, but is transmitted only after the preceding command/cancellation's
command-spacing deadline and itself advances that deadline.

Every arbitration decision of an async owner turn belongs to one
executor-free coordinator (`OwnerCoordinator` in `runtime/owner/turn`, #776);
the blocking owner adopts the same coordinator with its worker thread (D24).
It owns the
source phase, the fairness ceiling, the timer's control allowance, the
raw-release input proof, and the retained-boundary slot. It has no channel,
transport, executor, or clock access. A shell samples the clock, asks for a
plan, polls its sources in the planned order, reports what it selected, and
runs that event's engine turn. The plan's `Selection::order` is the single
definition of priority: the async actor polls its six sources by walking it,
so simultaneous readiness is never left to an executor's wake order or a
randomized race.

The order is deterministic and progress-sensitive rather than history-based:

1. Receive is first while it produces a valid non-empty frame batch. Buffered
   ACKs, completions, and replies therefore update protocol state before a
   simultaneously ready control observer sees that state.
2. A receive that makes no protocol progress (idle/no-data, a partial frame,
   a transient fault, an empty UDP datagram discarded under the current overall
   deadline, or a receive whose only frames were discarded as malformed) makes
   the next selection poll boundary sources first in the fixed order shutdown,
   cancellation, admission, control, then timer. Discarding an empty datagram
   never starts a fresh deadline; the async UDP adapter also yields
   cooperatively before polling again.
3. A boundary win, or a later valid frame batch when no boundary was ready,
   returns the actor to receive-first.
4. A **fairness ceiling** bounds the succeeding arm as well. After a run of
   consecutive receive-first wins (`frames_per_receive`), one boundary-first
   turn is forced regardless of progress, then the run restarts. This keeps a
   babbling peer, one that returns a valid frame on every poll, from starving
   shutdown, cancellation, admission, control, and the timer (#625, #675).
   When no boundary is queued the receive still wins the forced turn, so a busy
   transport is never stalled. Every forced or yielded boundary-first turn
   begins with one cooperative executor handoff.
5. An already-due timer grants control one observation, then precedes control
   until it fires. The allowance is keyed to that exact deadline, so a replaced
   or newly due timer starts afresh.

Phase choice depends on the result of the current receive; the only count is
the bounded fairness ceiling, never an open-ended receive history. Transmission
effects produced by either phase are driven immediately before the next
selection.

### Raw-correlation release and retained boundaries

A raw release does not create an exception to that order; it adds two rules.

- **A newly due release first gets an ordered receive.** The timer is what
  completes a release, so until the latched release set has had one receive
  at or after its deadline, the plan polls receive first, even when an idle
  read before the deadline would otherwise have yielded to the boundaries:
  input that became ready while that read was parked must win. Only the read's
  completion instant, not a later executor resume, certifies the proof. The
  fairness ceiling stays authoritative: a forced turn puts shutdown,
  cancellation, admission, control, and the release timer ahead of a receive
  flood, unless the framer already holds a complete frame, which is decoded
  first. That forced timer turn is what resolves retained input and starts a
  grace budget, so a flood cannot stall the release. An exact no-input result
  fences the latched set; with no grace pending, the next plan then delivers
  the timer through the receive source without another read. A transient fault
  does not fence, and its input turn suppresses due work, so a transport that
  keeps failing holds the release, and with it the queued boundaries, until the
  fault run becomes permanent and ends the session. During a retained-prefix
  grace the timer arm is the engine-owned grace deadline (#713); once the grace
  has elapsed it leads, so an eager receive cannot delay it further. If the due
  set grows before the timer turn, the latch is replaced and the new set needs
  its own proof (#746).
- **A boundary selected while a release is due is retained, not run.** Its
  ordinary input turn would run due work behind unread or retained evidence.
  The coordinator holds it in a single slot until the release resolves, and
  while the slot is occupied cancellation and admission are not eligible, so a
  second boundary stays in its bounded channel rather than being dequeued with
  nowhere to keep it (#775).

Thus an Urgent stop is selected within the fairness bound under raw input
pressure, while no scope is released using evidence collected before its own
deadline.

The retained-boundary slot. Here *due* means the raw release projection at the
instant the selection completed is non-empty, and *latched* means a release was
pending when the turn was planned.

| Slot | Event | Condition | Result | Next slot |
| --- | --- | --- | --- | --- |
| empty | admission or cancellation selected | not due | ordinary engine input turn | empty |
| empty | admission selected | due, claim won | retained; the claim is not taken again | admission |
| empty | admission selected | due, claim expired | answered `Timeout` | empty |
| empty | cancellation selected | due, terminal result buffered | answered with that result | empty |
| empty | cancellation selected | due, live | retained | cancellation |
| held | any plan | latched | cancellation and admission ineligible; shutdown, receive, control and timer keep their order | held |
| held | plan | not latched | shutdown wins if already signalled; otherwise the retained boundary is delivered before any source is polled | see below |
| held | redelivered | not due | ordinary engine input turn | empty |
| held | redelivered | due again (a new hold matured) | retained again | held |
| held | session ends (shutdown, close, fault, or unwind) | | answered with the terminal error (a cancellation with a buffered terminal result gets that result), before the queued boundaries | none |

### Transport I/O deadlines and idle pacing

The actor also bounds each transport operation itself, because the
runtime-agnostic async transports hold no timer of their own. Every read is
raced against the session's `read_timeout`; if it elapses the read is treated as
an idle no-data receive (it consumed nothing), which makes the advertised knob
live on the async surface rather than inert. Every write is raced against
`write_timeout`; if it elapses the write is abandoned as a failure — a stream
write that can no longer be confirmed poisons the session, a datagram write
fails only its own request — so a stalled peer can never park the actor and
block `close()`. The blocking owner cannot preempt arbitrary synchronous code,
so `BlockingTransport` exposes only deadline-bearing reads and writes and
requires implementations to return within the supplied bound. Built-in TCP,
UDP, and serial transports install that bound at the OS/device layer, and
construction rejects a zero advertised read or write timeout. A serial device
receives at least one millisecond even when a derived owner budget is smaller:
Windows treats a zero-millisecond serial timeout as no timeout, while the owner
still rechecks its precise deadline after that I/O call. Serial command and
startup writes deliberately do not drain the device (`flush`/`tcdrain`), since
that kernel call is not bounded by the write timeout; a VISCA reply or the
required startup settle interval confirms queued output instead. Finally, both
owner shells use one executor-free idle-receive
run. Immediately-returning no-data reads are paced by the same escalating
10–250 ms, next-wake-clamped pause the transient-fault path uses (recording no
fault and spending no retry budget). The coordinator paces only the receive
source, so boundaries, timers, and control stay selectable during the pause in
both shells, and an eager custom driver cannot hot-spin either owner (#723).

## Request and motion semantics

Plain commands normally complete when the owner has protocol-applied them; a raw
`NoReply` plain command instead reports only a successful local transport write.
Typed inquiries return their decoded response. Targeted operations have a meaningful
target state and expose applied completion plus `Settlement` evidence. `settled`
returns exact profile completion or stable samples with axes, window, and
tolerance; stable samples do not prove arrival at the endpoint. Applied-only operations represent actuation without a
meaningful target state and expose
applied completion only. Dynamic handles preserve this distinction:
`DynTargetedOperation` has `applied`, `settled`, `cancel`, and `detach`, while
`DynAppliedOperation` has no settled operation.

### Operation observation

An operation handle observes its request through two one-shot slots the owner
resolves at most once each (#777):

- the **terminal slot** carries the request's authoritative outcome
  (`Applied`, `Cancelled`, or the exact failure);
- the **cancellation slot** exists only after the handle's first `cancel`, and
  carries an error when an accepted cancellation ends without ending the
  request: its write failed, or its observation deadline passed.

Every wait borrows the handle (`&mut self`) and records each value it receives
into a handle-side cache in the same poll, so dropping a wait — a timeout, or
the losing branch of a `select!` — loses nothing, and a later wait answers from
the cache before it consults the owner. Application is cached, so `settled`
after `applied` continues from it. Settlement proven by position polling is
cached once proven; polling state is local to one wait, and a wait abandoned
mid-proof restarts with a fresh two-sample proof. A fixed per-target/per-axis
generation bank orders successful engine admissions against establishment of
polled evidence under one lock. A later conflicting admission supersedes an
unfinished proof even if the new command is cancelled before writing. Rejected
admissions do not advance generations. Evidence already cached and exact
profile completion remain valid. Polling errors retain their source while
reporting observation uncertainty for the original move.

Cancellation is one intent per handle. The first `cancel` creates the
cancellation slot and ships it with the request; a later `cancel` — after a
timeout or a dropped call — finds the owner already holding that intent and
observes it, so the wire carries at most one cancellation. A refusal
(`NotSupported`, `InquiryNotCancelable`) installs no intent and leaves the
handle observing. The verdict is terminal-first among outcomes delivered on time: once the
terminal outcome is known at or before the observer deadline it decides (`Applied` is `Completed`, `Cancelled` is `Cancelled`, a
failure is that error), whichever order the owner emitted the two slots in;
only without an on-time terminal does an on-time cancellation-slot error answer.
Delivery timestamps use the injected owner clock, not inferred wire arrival.
Late outcomes remain cached for a subsequent borrowing wait. A `cancel` after the
owner stopped still answers from an outcome delivered before it stopped.

The default observer deadline for `cancel` is the larger of the operation's
application deadline and its ambiguity plus cancellation deadlines, so a
default `cancel` outlives the owner's own cancellation resolution and reports
its conclusion rather than a caller-side timeout.

Use the noun view for ordinary controls and `submit` when a caller needs an
explicit operation lifecycle. `motion().stop_all_motion()`,
`motion().is_moving(...)`, and `motion().wait_until_idle(...)` are the only
camera-level motion safety/observation entry points. A dropped handle is not an automatic STOP;
emergency stopping is an explicit STOP or motion operation.

`stop_all_motion()` is one bounded owner transaction returning `HaltReport`.
Acceptance atomically samples the ingress cutoff and publishes the supported-axis
fence against settlement establishment. Earlier declared queued motion is
suppressed, including work still in the ingress lane; already-written lifecycle
and correlation survive, with future retries suppressed. A suppressed request
fails with `Error::MotionSuperseded`, whose failure context is `NotAccepted`
when it never reached the camera (never written, or every written attempt
rejected before ACK) and `Unconfirmed` only when an earlier attempt may have
taken effect. STOPs are admitted
independently using the existing reserve, then dispatch under one absolute
deadline and normal protocol gates. Sibling raw STOPs wait for attributable ACKs,
not each other's completions. A final write guard discards expired or stale
staged STOPs without fabricating transport failures. Later admissions remain
eligible; raw/custom commands participate only when they declare motion axes.

Broadcast address assignment and interface clear are transport-lifecycle
controls, not camera requests; the serial handshake owns them before the target
registry starts. Address Set treats an oversized delimiter-framed input as
discarded bus noise after the framer has resynchronized, then continues the
current scan or a remaining bounded attempt. Socket cancellation is likewise owner-only because only the
owner knows which live operation owns a camera-assigned socket. None of those
three wire primitives is exposed as a generic `request::builtin` command.

## Preserved implementation boundaries

The authoritative built-in semantic ledger remains
`src/command/semantics.rs`; facade inventories are projections, not a second
wire or semantic registry. Profile marker implementations and runtime
discovery facts come from the profile registry. The owner is shared by static,
dynamic, and state/diagnostic projections. These boundaries keep blocking and
async behavior equivalent while allowing their calling conventions to remain
different.

For the preservation inventory and ecosystem list, see
[`architecture_inventory.md`](architecture_inventory.md). For construction and
operational examples, see [`usage_2_0.md`](usage_2_0.md). The critical 1.x/v2
trade-off, size, and reuse analysis is recorded in
[`issue_542_design_review.md`](issue_542_design_review.md).
