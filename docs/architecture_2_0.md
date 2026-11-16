# 2.0 architecture

This page describes the public 2.0 vocabulary and the ownership rules behind
it. The implementation has one protocol authority per session. A compile-time or runtime-selected
blocking or async camera value is a view onto that authority; it is
not another connection.

## Public vocabulary

| Concept | 2.0 meaning |
| --- | --- |
| `config::SessionConfig` | Mode-independent, reusable profile/target registry and operational tuning. |
| `blocking::Session` | Owner handle for one native worker and blocking transport. |
| `r#async::Session` | Owner handle for an async transport and selected executor/runtime. |
| `Camera<P>` | A view for one registered target: compile-time profile `P`, or runtime `profiles::AnyProfile`. |
| `StateCache` | A read-only, target-local projection of exact applied write-only state. |
| `MetricsSnapshot` and `DiagnosticEvent` | Bounded scalar metrics and sanitized owner observations. |

<!-- BEGIN NOUN ENTRY POINTS -->
| Both facades | Accessor | Methods |
| --- | --- | --- |
| `power()` | `PowerAccessor` | 3 |
| `zoom()` | `ZoomAccessor` | 14 |
| `system()` | `SystemAccessor` | 2 |
| `pan_tilt()` | `PanTiltAccessor` | 10 |
| `focus()` | `FocusAccessor` | 22 |
| `presets()` | `PresetsAccessor` | 13 |
| `exposure()` | `ExposureAccessor` | 49 |
| `white_balance()` | `WhiteBalanceAccessor` | 52 |
| `image()` | `ImageAccessor` | 54 |
| `tally()` | `TallyAccessor` | 6 |
| `nd_filter()` | `NdFilterAccessor` | 7 |
| `motion_sync()` | `MotionSyncAccessor` | 3 |
| `menu()` | `MenuAccessor` | 7 |
| `streaming()` | `StreamingAccessor` | 4 |
| `motion()` | `MotionAccessor` | 3 |
<!-- END NOUN ENTRY POINTS -->

The async and blocking camera cores expand from one `camera_core!` definition
(#865). Both retain one owner handle, immutable target registration and
submission-class default, and prepare each request using live validated
tuning. The native blocking expansion calls its owner directly; the async
expansion awaits its corresponding owner methods. Facade parity checks inspect
both expanded cores' private and crate-visible methods, bounds and forwarding
bodies, as well as the public facade signatures.

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
* `Camera<AnyProfile>` erases only the compile-time profile parameter. Requests,
  operation kinds, ownership, transport, runtime, timeouts and cache policy stay concrete.
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
and retry policy into an inert request context. For an ACK-bearing command,
one governing attempt includes ACK plus completion. The admission-to-terminal
retry budget keeps the existing ten-second/twice-response/busy-timeout floor
and grows when necessary to cover that full attempt; untuned built-in budgets
are unchanged. Completion-only commands and inquiries keep their one response
window. Default observers cover the resulting retry budget (#842).
The owner/engine consumes that context without reselecting or inventing a
deadline. `TransportConfig` carries I/O timeouts, buffer bounds, and socket
options. Transport kind, send semantics, and VISCA addressing come from the
required `HasTransportConfig::identity()` descriptor; the configuration has no
addressing field and does not own protocol retry or completion policy.

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

The native shells expand `run`, `handle_event` and `drive` from one
`owner_turn_driver!` definition. Clock reads, cooperative handoff, event
selection and bounded writes are the four native I/O hooks. Every staged
write reports its outcome and drives the resulting engine effects before the
owner selects more input or starts another write. The blocking expansion
contains no future. A recording-core test compares the complete call order
of both expansions, including transport release before the liveness barrier.

Internally, both shells complete engine input through the same executor-free
`EngineTurn` policy. A complete turn runs due work, pending cancellation, and
one ordinary dispatch; an input-only turn preserves retained wire evidence
ahead of deadlines at the same sampled instant. These are
options on one engine boundary, not feature-gated owner entry points. A
correlated frame sampled exactly at its response deadline therefore wins.
Every strictly late correlated frame first retires its owner through the due
lifecycle, then accounts its evidence against the resulting retained Sony
identity, raw debt, or socket hold. Stream candidates retire before any
side-effecting ledger resolution. A late ACK can prove acceptance and cancel
a pending Sony retry; a late raw completion can release the socket hold that
retirement just installed. Deadlines never erase that evidence (#858). A
write result is applied before any frame, cancellation or due work: neither
owner interleaves inputs while its immediate write is outstanding. A successful
raw stream write sampled after its budget still owes its unanswered first
answer (#837, #860). A frame for an unrelated request keeps that request's own
deadline.

<!-- BEGIN PROFILE TIMINGS -->
| Profile | ACK (ms) | Inquiry (ms) | Command spacing (ms) | Inquiry spacing (ms) | Quick / movement / preset / long-running / network (ms) |
| --- | --- | --- | --- | --- | --- |
| `PtzOpticsG2` | 500 | 1000 | 100 | 150 | 5000 / 30000 / 60000 / 300000 / 5000 |
| `PtzOpticsG3` | 500 | 1000 | 100 | 150 | 5000 / 30000 / 60000 / 300000 / 5000 |
| `PtzOptics30x` | 500 | 1000 | 100 | 150 | 5000 / 30000 / 60000 / 300000 / 5000 |
| `SonyFr7` | 500 | 1000 | 35 | 35 | 8000 / 30000 / 60000 / 300000 / 8000 |
| `SonyBrcH900` | 500 | 1000 | 35 | 35 | 6000 / 30000 / 60000 / 300000 / 6000 |
| `SonyEviH100` | 500 | 1000 | 0 | 0 | 5000 / 30000 / 60000 / 300000 / 5000 |
| `SonyBrc300` | 500 | 1000 | 0 | 0 | 5000 / 30000 / 60000 / 300000 / 5000 |
| `NearusBrc300` | 500 | 1000 | 0 | 0 | 5000 / 30000 / 60000 / 300000 / 5000 |
| `GenericVisca` | 500 | 1000 | 0 | 0 | 10000 / 30000 / 60000 / 300000 / 10000 |
<!-- END PROFILE TIMINGS -->

## Target registry and preflight

`SessionConfig` has exactly seven individual target slots, VISCA IDs 1 through
7. Broadcast is never a session target. Registration is immutable once the
owner starts, and duplicate IDs, an eighth target, an empty registry, and
profile/configuration conflicts are rejected before transport I/O.

Public `CameraId` values cross individual-camera boundaries through the one
private `TargetSlot::try_from` rule. A `TargetSlot` indexes `PerTarget<T>`, a
fixed seven-slot table used by session registration, grouped engine state,
stream-ledger state, owner registries and caches. Neither controller address 0
nor broadcast occupies an inert slot. Request removal scrubs only its own
target; broadcast Address Set and interface-clear maintenance remain explicit
transport-lifecycle operations outside these tables. The public API continues
to use `CameraId` (#853).

`CameraSession<P>::camera()` returns its single bound target. A `Session`
selects every target explicitly with `camera::<P>(target)`, checking the complete
registered `ProfileSpec`: capability and reply-domain facts, profile-selected
encodings, coordinate codec, transports, envelope, timing, command sockets,
completion and cancellation support, preset axes and position inquiries.
A custom profile cannot claim a built-in identity after changing any of those
facts. `camera::<AnyProfile>(target)` retains runtime validation instead of
making compile-time capability claims.

Multi-target registration is rejected before socket creation on any
IP-addressed transport, which is every standard TCP and UDP configuration and
every Sony-encapsulated one. Heterogeneous *envelopes* are never accepted:
admission requires every registered profile to report the same wire envelope
as the first, and any mismatch is an error. What a multi-target session does
accept is heterogeneous *profiles* that share one envelope — in practice
several raw-VISCA profiles — carried by a serial-addressed transport. The
built-in serial transports qualify: both `transport-serial` and
`transport-serial-tokio` report `TransportIdentity::Serial { bus }`.

Every transport provides the required
`HasTransportConfig::identity() -> TransportIdentity<'_>` alongside its I/O
configuration. The `Tcp`, `Udp`, and `Serial { bus }` variants fix the standard
kind, stream/datagram semantics, and addressing together. A truly custom
transport declares `Custom { semantics, addressing }`, the explicit escape
hatch from the standard profile/transport pair matrix. It still undergoes
target-registry, envelope, addressing/topology, pacing, framing, buffer, and
bounded-owner validation. The adapter reads this descriptor before transport
configuration and performs its topology check once, deriving wire addressing
and write-failure policy from the same identity. Wrappers, including both
transport-handle enums, forward the inner identity with its serial Address Set
bus report.

The owner supplies complete framed packets to transport writes.
`BlockingTransport::send_with_timeout(&mut self, bytes, timeout)` takes no
command-kind argument; `AsyncTransport::send` likewise sends the supplied
bytes without selecting their protocol framing.

The static `Has*` profile-marker gates on noun/accessor methods are the
compile-time permission surface. Direct generic `Camera::execute` and
`Camera::inquire` intentionally carry no request-specific `P` bounds: they are
the uniform typed/downstream extension boundary. A runtime `ProfileSpec` is
validated in full before owner admission, and dynamic callers use
`capabilities()`/`supports(...)` before selecting optional controls.
Metadata is discovery information; it is not a fallback that exposes an
unsupported typed operation.
For example, broad `HasExposure` admits the exposure domain, while the shared
`04 39` mode command and inquiry require `HasExposureMode`; erased callers
recheck both the typed surface and its source-backed mode inventory.

## Engine transition and ordering

The construction and request path has a fixed order:

1. Build `SessionConfig` from validated `ProfileSpec` facts and operational tuning and validate target IDs, registry
   cardinality, profiles, transport envelope, addressing, tuning, and bounded
   owner policy.
2. Create the one owner and its fixed target-local state registry. No socket,
   serial device, or protocol frame is opened before step 1 succeeds.
3. Start one async owner actor or one native blocking owner worker.
4. Project `Camera<P>` or `Camera<AnyProfile>` views
   without creating a second owner, runtime, transport, or cache.
5. Prepare each request against the selected target/profile and admit it to the
   owner. Preparation invokes `Request::validate_for_target` before encoding;
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
   or merely prepared command cannot create a cache value. A write that may
   have taken effect without confirmation invalidates its key before the
   terminal outcome resolves; a conclusive rejection leaves it intact. Known
   values occupy one typed inline slot per declared `StateKey`.
8. Record terminal outcome, release admission capacity, and retain only the
   bounded diagnostic/history state promised by the public API.

### Correlation state ownership

`ProtocolEngine` contains shared `EngineCore` state and one closed
`CorrelationMode`, selected at construction from the wire envelope and transport
kind. `EngineCore` owns the entry table, target policies, identities, pacing,
and session state. Correlation-specific state belongs to its mode:

| Mode | Owned correlation state |
| --- | --- |
| `Sequenced` | No raw hold table or stream state; applies to Sony on either transport kind. |
| `RawDatagram` | `HoldTable<RawHoldScope>`, including the `PreAck` scope. |
| `RawStream` | `StreamCorrelation`: `HoldTable<StreamScope>`, `StreamLedger`, per-target `CompletionLearning`, and the retained-input release gate. |

Mode dispatch borrows the shared core and the actual variant state together.
`StreamContext` receives `&mut EngineCore` and `&mut StreamCorrelation`;
`DatagramContext` receives the core and its datagram hold table. Stream-only
processing methods are implemented on `StreamContext` and require its actual
stream state. Completion learning and its due-pass work belong only to the
stream state.

The hold table exposes closed mutations: conservative extension with owner
collapse, exact socket release, paid-reply skew reset, expiry, and clear. A
`StreamScope` can name only `Socket`, `Inquiry`, or `AllResponses`; it cannot
install `PreAck`. Stream hold expiry requires `&StreamLedger`, preserving exact
socket holds until evidence releases them and inquiry-shape holds while their
reply is still owed. One evidence-release predicate serves expiry, wake
planning, and retained-input release. These mode-placement rules follow from
the types; ledger ownership, phase legality, disputes, and collection bounds
retain their semantic safety audits.

The raw stream variant boxes `StreamCorrelation` once at engine construction.
This keeps its larger learning and ledger state out of every engine's inline
size and preserves the existing representation-size bound. The box introduces
no allocation per request or transition.

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
coexist with independently expiring `Socket`, `PreAck`, and exact inquiry-shape
holds, and overlapping owners of one key erase identity rather than choosing by
recency. The hold table has one bounded entry per target/scope pair, expires
through the ordinary next-wake path, and never grows with command history.
Each target has at most four command keys (two sockets, pre-ACK and all
responses), plus its 16 reserved inquiry declarations: at most 20 hold entries
per target and 140 across seven targets. A
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
route the sole live inquiry only when no unacknowledged command exists.
Multiple disjoint live inquiries have no error-shape evidence, so that collision,
like a command-plus-inquiry collision, is ignored. A socketless
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
  an inquiry-shape hold, or an `AllResponses` hold has already discarded such a
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
| Inquiry data | the oldest inquiry whose decoder-declared payload shape admits the data; the wire carries no inquiry identity |
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
for the longer of the two candidates' completion deadlines. Both candidates
have finished their immediate write: neither owner processes a frame while a
write result is outstanding (#860).
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
owed inquiry reply also retains the declaration's inquiry hold through its
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
least the reply skew). Inside it, the debt's overlap class waits: same-target
inquiries whose declarations overlap an owed reply, ordinary commands for an owed command answer
(an `Urgent` STOP is still written under the ordinary `PreAck` bypass rule;
a `NoReply` command waits too, its possible rejection being owed in turn).
If the window ends with the debt unpaid, only that inquiry overlap class or command lane *latches*: it no
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
one would let its late answer reach a later request. At 32 total debt members,
ordinary commands latch until debts are paid; urgent ACK-bearing STOPs and
cancellations continue, and a disjoint inquiry can still cross the debt. Raw
inquiries have a separate limit: at most **16 exact declarations per target**
can be simultaneously admitted, owed, or retained in reply-skew holds. Admission
derives their union from those authoritative records, including queued requests;
there is no reservation mirror. A new seventeenth declaration is refused before
admission with `InquiryShapeCapacityExhausted` (`PreAdmission` / `NotAccepted`,
`Recovery::Abandon`). Existing declarations, command recovery, other targets and
the healthy session remain available. Releasing all evidence for a declaration
releases its reservation. This bounded resource policy is the explicit exception
to disjoint-class availability. `Bytes(4)`, `Nibbles(4)` and `Unknown` are three
exact declarations, even though they overlap: overlap never merges legal sets.

Memory stays bounded without dropping anything. Consecutive debts owing the
same exact answer form one *run* with a member count, paid oldest first.
"Consecutive" is judged among entries an answer could confuse: cancellations
for one socket among themselves, every non-cancellation among the others,
because a socketless error may answer any of them. Disjoint inquiry data
therefore still separates command runs.

Let `D = 32`, `A` be the requests that can be admitted at once (ordinary capacity
plus the target's control reserve), `C = 16`, and `P = 5` pinned dispute entries.
Choose the last non-inquiry, non-cancellation write that was not an urgent ACK
command, or the empty origin. Its prefix contained at most `D - 1` debt members
and `A` live entries. Retiring and coalescing never increases that prefix, so
it retains at most `D - 1 + A` rows. The suffix contains at most `A` live
non-cancellation entries, `C` inquiry debts, and `P` pinned entries. Pairwise
overlap exclusion proves each inquiry debt has exactly one member. These
separators split urgent ACK debt into at most `A + C + P + 1` runs, making the
suffix at most `2A + 2C + 2P + 1` rows. Socket-specific cancellations add at
most `2A + 2`. Thus each target is bounded by
`D + 5A + 2C + 2P + 2 = 5A + 76` ledger rows. This is the previous bound plus
`2C`: one inquiry row and at most one additional ACK-run separator per retained
declaration. There are seven individual targets; seven times the per-target
bound is a conservative session bound. The engine audits the row bound, exact
reservation cap, and absence of overlapping staged/live/owed inquiry rows.

The ledger rests on these camera facts, and guarantees under them that every
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
- An inquiry reply has the structural shape declared by its decoder. Shape means
  data length and, for nibble data, every byte at most `0x0F`; response values
  and firmware-specific ranges remain the decoder's responsibility.
- Every written request receives exactly one first answer (only a
  `CompletionOnly` command's completion may never come).
- An accepted socketed command reports one terminal completion or execution
  error. In particular it does not duplicate a socketless completion: a
  complete count of such frames can prove that all retained socket candidates
  ended, even when it cannot identify each frame separately.
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

- An undocumented inquiry shape is discarded with `ReplyShapeMismatch` and
  increments `ignored_reply_shape_mismatches`. The inquiry times out instead of
  completing with a decoder length error; on a stream its first answer remains
  owed. If the undocumented payload fits another live inquiry's declaration,
  it can reach that inquiry: documented shapes are an explicit camera assumption.

- A socketless execution error, or a second answer to one request, binds to
  the oldest legal entry: it pays a debt (discarded), or reaches a live
  request as its answer — a false rejection, or an ACK for a command the
  camera has not read.
- A duplicate socketless completion can falsely exhaust the retained
  candidates' terminal count and complete a different live candidate.
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

A camera that omits the socket in its completions costs availability when
two commands execute or a predecessor's socket is held. The engine retains
an ambiguous completion with the identities of both possible owners. When
the number of retained terminals proves both ended, it retires any overdue
owners, releases their holds and completes the owners still within their
deadlines. The buffer is fixed at two socket identities and never applies an
old frame to a successor. An ACK reusing a held socket proves that predecessor
ended, but does not prove whose earlier completion arrived; reuse therefore
discards that ambiguous attribution instead of completing the other owner
by guess. A missing terminal, an intervening execution error, or socket reuse
can still leave a socket ambiguity unconfirmed. A
`CompletionOnly` completion can still be lost to that ambiguity and latch the
command lane. A camera that names its sockets (the PTZOptics G2 sends
`90 51 FF` and `90 52 FF`) does not pay these availability costs (#858).

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
command. An oversized raw frame ending in `FF` is discarded and counted too: its
terminator preserves the next frame boundary. Cumulative input beyond the hard
retention bound `max_buffer_size + recv_buffer_size`, or a Sony header declaring
a payload the framer cannot retain, is a genuine framing failure that still
poisons a stream (#848). For the serial preset the bound is 8192 + 256 = 8448
bytes, and overflow reports that configured bound. Raw and Sony framers are
separate types with one strict, nonmutating overflow policy. The adapter pairs
each framer with its matching envelope and visits frame bytes by reference;
it does not copy them into a second framing scratch buffer.

One immutable `WireProtocol` authority is created for the session and shared
with the deterministic engine. Engine mode predicates derive from that authority;
the policy stores no second envelope discriminator. Its typed raw/Sony encoder
projections mint opaque write plans: raw takes no sequence argument, while Sony
binds an explicit fresh-allocation or replay action to the same allocator used by
sequence reset. The owner executes the bound plan into its reusable frame buffer;
it never compares a write mode with a separately selected adapter mode. Minting a
plan does not allocate a Sony sequence. Execution validates the payload before
allocating a fresh sequence, and replay leaves the allocator unchanged. The
adapter's receive framer is constructed from that same authority, once.

Receive batches reuse the owner's bounded storage after the shell consumes them. The read
boundary produces one closed verdict (frames, no progress, transient failure,
or fatal failure), retaining original fatal causes. The shell executes that
verdict without classifying OS errors again. Retained stream inspection is an
infallible nonzero length and its evidence, including explicit non-raw input;
only discarding can fail. By the same decoupling, a
single stream read that decodes more than `frames_per_receive` frames stops at the limit and
leaves the remaining complete frames buffered for the next receive (the framer
retains bytes across reads, bounded by `max_buffer_size + recv_buffer_size`), rather than failing
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
for an external caller to join. Other calls that would wait for the same worker
fail immediately with `InvalidState` before enqueuing work, so a transport
callback cannot deadlock the owner or prevent STOP delivery. An idle read is reported only when the
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

The engine keeps exactly one authoritative entry per admitted request. Its
single stored `Lifecycle` value owns the phase-local attempt facts and legal
cancellation substate
(`src/runtime/engine/lifecycle.rs`, #836). `Phase` and `CancelState` are
flattened read-only diagnostic/test projections, never independently stored
entry fields. The public diagnostic phase vocabulary is unchanged.
Production code classifies lifecycles with lifecycle predicates; `Phase` is
diagnostic only.
The tables below follow the exhaustive lifecycle transitions in
`src/runtime/engine/mod.rs`.

Raw inquiry replies have no wire identity. Each prepared inquiry carries its
decoder-declared `PayloadShape`: exact unrestricted `Bytes(n)`, exact nibble
`Nibbles(n)`, or conservative `Unknown`. Built-in declarations and canonical
derived inquiries come from the decoder table. Pan/tilt preparation selects
shape and decoder from the same profile framing once and retains both through
reusable polling templates. No adapter invents an inquiry identifier.

Raw targets permit one staged or live inquiry per pairwise overlapping shape.
Equal lengths overlap, and `Unknown` overlaps everything; overlap is not
transitive, so a queued `Unknown` does not merge unrelated widths. Distinct
classes may be live together, under the same minimum inquiry spacing. Each
reply can fit at most one live inquiry. Datagram replies bind only when that
inquiry's declaration admits the data; stale cross-shape replies and malformed
nibble data are discarded and counted. Same-shape delayed datagrams and
undeclared `Unknown` retain positional ambiguity. Socketless errors carry no
shape evidence: on datagrams, multiple live inquiries make them ambiguous;
on streams, the ledger's oldest-legal first-answer order remains authoritative.

A matched reply exhausts its response and installs no hold. An uncertain raw
datagram release retains the profile's `raw_inquiry_reply_skew` only for
overlapping declarations. Stream inquiries are never resent for a missing
reply: lossless transport leaves that first answer owed, through its ambiguity
window and then a class-specific latch. A disjoint position reply can cross an
unanswered one-byte mode inquiry and retire its older terminal debt by write
order. Thus `is_moving`, `wait_until_idle` and observed-stable settlement polling
continue when their declaration is disjoint. Overlapping inquiries still wait
and latch with `InquiryCorrelationLost`. No inquiry timeout poisons a session;
every failed engine inquiry terminal has the source-preserving `InquiryFailed`
wrapper and `Terminal` / `FailedConclusively` context. The 16-declaration
resource bound above applies independently to each raw target.

### Protocol phases

| `Lifecycle` variant | Phase-local facts | Legal cancellation |
| --- | --- | --- |
| `Ready` | On retry, the previous written attempt. | None; cancelling removes the entry. |
| `Sending` | Transmission, framing expectation, start time, attempt facts, and the closed `SendingReply` family. Its result precedes every unrelated input or due pass. | None; commands refuse cancellation with `CancellationNotSupported`. |
| `AwaitingAck` | Send time, ACK deadline and attempt facts. | None or requested. |
| `AwaitingCompletion` | Send time, completion deadline and attempt facts. No socket. | None; cancellation is refused with `CancellationNotSupported` (#835). |
| `Executing` | Socket, start time, original completion deadline and attempt facts. | `ExecutingCancellation`: none, requested, sending, or observation-failed. |
| `AwaitingReply` | Send order, send time, inquiry deadline and attempt facts. | None; inquiries are not cancellable. |
| `Backoff` | Ready-at time and the previous written attempt retained for exact Sony acceptance. | None. |
| `AwaitingCancellationResolution` | Socket, original deadline and attempt facts. | `ResolvingCancellation`: requested, sending, awaiting-terminal, or observation-failed; never none. |

`AttemptFacts` holds dispatch-time socket capacity and stream write order.
Retry waiting retains the last written attempt so a late exact-sequence ACK
honors that attempt's capacity. `SendingReply` carries only its response
family: it contains no reply latch or cancellation intent (#858, #860).
The original completion deadline and cancellation ambiguity deadline each have
one home; lifecycle methods derive an effective maximum where the protocol
requires it. A transition accepts one lifecycle value, and closed due actions
preserve response/cancellation-before-retry ordering (#836).

The lifecycle selects one closed due action from its phase and the entry's
retry budget. Each action removes its request or moves its next due instant
strictly past the sampled clock; zero-delay retries become ready in that same
action. A late cancellation observer and its already elapsed terminal window
finish together. Due work carries no generation ticket, and a debug iteration
bound and per-action progress check turn a future regression into a test
failure. Context timeout intervals are nonzero proofs, retry budgets are
optional positive intervals, and admission validates completed timing windows
against its already sampled owner clock. Overflow saturates at the latest
representable instant; an extension with no future instant retires instead of
reinstalling an immediate deadline (#861).

### Cancellation substates

Cancellation is nested in the lifecycle states that permit it. Requested and
sending substates carry their ambiguity deadline and observer-open fact; the
sending substate also carries its transmission and framing expectation. Awaiting-terminal additionally
carries the cancellation observation deadline. Observation-failed records the
remaining ambiguity lifecycle after its observer has resolved. None of these
substates repeats the socket: only the enclosing socketed lifecycle owns it.
Whether cancellation was attempted is derived from the substate, rather than a
separate latch (#836).

These types admit 15 flattened `(phase, cancellation)` combinations. The
transition/property tests independently check that table. The closed state
representation replaces the former phase-local latch and cancellation checks.
Only command requests carry `ReplyShape`; inquiry contexts carry none. Active
phases derive their response family from the lifecycle. Queued and retry work
consult the immutable request when selecting the next lifecycle. `EntryTable`
owns live entries, eager ready queues, Sony sequence indexes, and a two-slot
socket table indexed by `ViscaSocket`. Only admission, closed transitions, and
removal mutate those indexes; there is no mutable entry or index view. A
camera-proven socket displacement has a closed placement carrying its one
socketed lifecycle until same-frame cleanup, so removing the predecessor cannot
release a successor's socket. Capacity, raw holds, and stream-ledger safety
remain audited, and both halt entry points run the debug invariant hook on every
return path (#836, #856, #859, #860).

Write receipts name the request and match its active sending transmission.
Transmission identities never repeat; no parallel write-ownership map exists.
Raw inquiry order derives from `AwaitingReply`'s required send order. Ready
queues contain request ids only and eagerly remove ids on every phase change;
there are no queue generations, stale tickets, or dispatch-time pruning passes.
The [entry-table audit record](https://github.com/GrantSparks/grafton-visca/issues/859#issuecomment-6901336955) explains the
cross-index checks replaced by construction.

Terminal results retain their request lane: `CommandOutcome` contains written,
applied, cancelled or failed commands, while `InquiryOutcome` contains a reply
or failure. Generic failures are lifted using the entry's request variant;
inquiry failures keep their `InquiryFailed` source chain. Owner observer slots
use the same lane, and operation application and cancellation read one shared
terminal verdict. Delivery moves the result into that slot (#903).

### Transition table

| Input / state | Transition or result |
| --- | --- |
| Admit while `Running`, within capacity, target registered, policy compatible | Allocate a non-colliding `RequestId`, create one `Ready` lifecycle with no cancellation substate, enqueue it, and return its `RequestId` directly. No write happens here. |
| Admit over ordinary capacity | Reject with `Error::RuntimeQueueFull`; no id, entry, observer, or queue slot is created. |
| Admit an urgent typed STOP into its target's control reserve | Admit while that target's reserved entries are below its reserve (one per supported typed STOP); otherwise reject with `Error::ControlReserveExhausted`. Only an urgent request may hold a reserved slot (D26, #778). |
| Admit with the id/generation space exhausted | Reject with `Error::RuntimeIdentityExhausted`. |
| Admit to a non-`Running` session | Reject with the session's terminal error (or `Error::RuntimeShutdown`). |
| Select a `Ready` request | Transition to `Sending`, allocate one `TransmissionId`, emit exactly one request `Transmit` (Sony carries its retained sequence; raw carries none). |
| Ordinary dispatch behind a live raw `AwaitingAck` candidate or inert `PreAck` hold | Queue to the evidence boundary: the candidate's ACK or the hold's deadline; the old terminal request cannot be rescued (#714/#723/#724). |
| `Urgent` raw command behind one positional candidate | Cross the single-candidate gate after command pacing when socket capacity remains. This is the sole two-candidate exception (#714). |
| Successful command send (`AckThenCompletion`, the default) | Record any Sony sequence and transition to `AwaitingAck`. |
| Successful command send (`CompletionOnly`, issue #700) | Transition straight to `AwaitingCompletion` (no ACK phase, no socket). |
| Successful command send (`NoReply`, plain raw only) | Finish the plain `execute()` after the local write succeeds; this is not protocol application and cannot create an operation handle. Retain the bounded broad `AllResponses` hold before same-target raw response-bearing command or inquiry work may start; a STOP is never held by it (#795). Another `NoReply` may write and extend that key. |
| Successful inquiry send | Record any Sony sequence and transition to `AwaitingReply`. Raw ownership derives from entries and decoder-declared payload legality: one staged/live inquiry per overlapping class, with disjoint inquiries and socket-owned commands allowed to coexist. |
| Write refused before I/O (`NotSent`) | Fail only this logical request with its preparation cause in `TransmissionNotSent`. Certainty is `NotAccepted` unless an earlier attempt may have taken effect, when it remains `Unconfirmed`; retain that attempt's owed answers and holds. No write-failure counter increments and the stream remains usable. |
| Failed command send, datagram transport | Terminally fail that one request. OS send failures retain their typed cause under `TransportError`; an explicit write timeout keeps its timeout context. Every other entry keeps running (#869). |
| Cancellation write refused before I/O | Drop its unwritten ledger entry and fail the cancellation observation. Keep the acknowledged operation, socket and original deadline live; a later completion still applies it (#869). |
| Failed inquiry terminal | Wrap the exact source in `InquiryFailed` with `Terminal` / `FailedConclusively`; the source still determines whether the session must be reopened. This covers arbitrary custom transport verdicts without reporting an uncertain inquiry effect (#868). |
| Failed command send, stream transport | Poison the session (`Error::StreamPoisoned`) and resolve every active entry. |
| Compatible failed stream write sampled after the request's total budget | Poison before applying per-request late-result policy: the stream seam cannot prove that no partial frame reached the wire (#724). |
| Request write result sampled strictly after its total retry budget | Retire the expired request before the result can register correlation, report `Written`, or replace a retained retry cause. A successful raw-stream write retains its unanswered first answer as a ledger debt. Results at equality remain input-first; cancellation writes use their separate ambiguity lifecycle (#837, #860). |
| ACK in `AwaitingAck` with a free socket | Assign the socket and transition to `Executing`; if cancellation was requested on a supported target, emit one socket cancellation. ACK evidence passes an `AllResponses` hold; a `PreAck` hold without a live predecessor permits the unique live candidate. Neither can resurrect a terminal owner (#723/#744; `urgent_ack_binds_when_its_preack_hold_has_no_live_predecessor`). |
| Strictly late correlated frame, either envelope | Retire its overdue owner before accounting the frame against retained identity, debt or hold. Input at equality still wins; unrelated requests keep their own deadlines. A raw datagram hold continues to discard the late frame (#858). |
| Exact-sequence Sony ACK/completion after an ACK deadline or during `Backoff`/`Ready` with a retained written attempt | The due lifecycle runs first if needed. An ACK cancels the pending retry and enters `Executing`, preserving the written attempt's socket capacity; a completion finishes `Applied`. A duplicate camera error cannot spend another retry. Expired total budgets remain terminal (#858). |
| Raw datagram ACK/error while two positional candidates are open | Ignore it as ambiguous and bind it to neither candidate; never use admission order or recency (#714). |
| Raw ACK naming a busy or quarantined socket | Treat the camera's named socket as authoritative: immediately fail a stale live owner, or downgrade an inert exact-socket hold, to its original-deadline unkeyed `PreAck` hold; assign the named socket to the uniquely resolved successor (#721/#723/#750). |
| Raw datagram error naming a free, unheld socket | A rejection code (`0x01`/`0x02`/`0x03`/`0x41`) routes the target's unique unacknowledged ACK-bearing command (the camera's rejection in the socket it allocated, e.g. G2 `90 6y 41 FF`; an inquiry, answered on socket 0, never names one); ignore it while the socket is quarantined or two candidates are open, and ignore `0x04`/`0x05`, which answer only a cancellation (#795). |
| Raw datagram error naming a socket owned by a live request | `0x04`/`0x05`: route the owner only if its emitted cancellation targets that socket. Otherwise route the exact owner unless another command on the target is unacknowledged: then bind to neither (#714). A live inquiry never competes: its error is socketless (#795). |
| Unsequenced frame on a raw byte stream | Resolve it through the correlation ledger: the oldest entry that can legally produce it (see "The raw byte-stream correlation ledger"); a live entry's request receives it, a debt discards it. A completion or error naming a live owner's socket is that owner's; one naming a held socket releases the hold (#795). |
| Raw stream socketless completion for a sole socket holder | Retire a strictly overdue holder first, then release its resulting socket hold. With two possible socket identities, retain the frame until a complete terminal count proves both ended; socket reuse never assigns it to the successor (#858). |
| Written raw byte-stream request ends without its first answer | Its ledger entry becomes a debt for its ambiguity window (an inquiry's is at least the reply skew); a staged write that never left leaves no entry. An owed inquiry reply retains the exact inquiry-shape hold; a pre-ACK command installs no `PreAck` hold (#795). |
| Ledger debt paid | Before side-effecting raw-stream ledger resolution, retire candidates whose response deadline is strictly earlier than the sampled frame; input at equality still wins. Resolve the frame against the resulting debt and discard it. An owed ACK displaces a stale owner of its named socket and holds that socket (or the first free eligible one) through its command's completion window; an owed reply leaves the ordinary reply skew from now (#795/#837; `raw_evidence::late_first_answers_are_order_independent`; `raw_evidence::late_ack_preserves_its_socket_against_successor_errors_and_socketless_completion`). |
| Ordinary command or inquiry dispatch while its lane owes an answer inside its window | Queue; an `Urgent` ACK-bearing STOP may still write when no command is unacknowledged (#795). |
| Ledger debt unpaid when the due pass reaches its window end | Latch only overlapping inquiry declarations, or the command lane: fail affected queued `Ready`/`Backoff` work and reject matching new work, unwritten, with `InquiryCorrelationLost` (overlapping inquiries, `CompletionOnly`) or `CommandCorrelationLost` (ordinary ACK-bearing, `CompletionOnly`, `NoReply`), `Terminal`/`NotAccepted`. The latch neither expires nor wakes the owner; STOPs, other targets, and the session are unaffected; the debt's answer, or a later first answer that settles it, reopens the lane (#795). |
| Disputed raw stream frame (a socketless error behind a `CompletionOnly` entry that may still be rejected or a `NoReply` command's owed entry, or any frame behind a disputed waiting request, with another candidate behind it) | Bind to neither; keep the union of the debts either explanation leaves: the `CompletionOnly` command owes only its completion, the other candidate keeps owing and is disputed (never crossed; settled by its own answer or by write order) (#795). |
| Sequenced Sony ACK naming a busy socket | Use the target's other free socket when available for #620/#682 compatibility; otherwise report `Ignored(SocketConflict)` and finish `CommandFailedAfterAck`. The exact sequence proved acceptance even when local ownership cannot safely track its socket, so replay is forbidden (#858). |
| Unrelated input or due work while a request is `Sending` | Forbidden by the immediate-write contract. Both owners apply the identified result first; the input-boundary auditor rejects a violation in debug and test builds (#860). |
| Completion in `Executing`, including after cancellation ambiguity elapsed | `finish` with `CommandOutcome::Applied`; exact socket ownership keeps it attributable through the completion deadline, and a retained cancellation observer maps this to `Completed` (#724). |
| Exact completion in `AwaitingCancellationResolution` before its phase deadline | `finish` with `CommandOutcome::Applied`; the request is still open while its owned socket is retained. |
| Completion in `AwaitingCompletion` (issue #700) | `finish` with `CommandOutcome::Applied`, regardless of a vendor socket nibble on a datagram; on a raw byte stream only a socketless completion resolves to this request (#795). A raw datagram retains the bounded `AllResponses` hold before ordinary same-target response-bearing work starts. A raw stream accounts for the answer in its ledger instead of installing that broad hold (`completion_only_terminal_quarantine_blocks_successor_then_restores_liveness`; `raw_evidence::stream_owed_completion_only_answer_holds_back_ordinary_work`). |
| Inquiry reply in `AwaitingReply` | Attribute only when its declaration admits the data, otherwise discard with `ReplyShapeMismatch` and increment `ignored_reply_shape_mismatches`; `finish` with the attributed payload. A matched Raw reply installs no hold and releases its overlap class immediately (#712, #892). |
| Uncertain Raw inquiry response-correlation release | A terminal error/timeout or retry release/requeue retains the profile's short `raw_inquiry_reply_skew` before another overlapping same-target inquiry may send. ACK-bearing commands, including `Urgent`, are never gated by this inquiry-only hold. It filters stale unkeyed inquiry data/reply and socketless-error evidence, while live attributable ACK/completion (including a uniquely attributable socketless completion) and exact named socket terminals remain eligible (#712). |
| Raw inquiry reply deadline or total budget in `AwaitingReply`, or a successful write sampled after the budget; stream transport | Never resend. Fail the inquiry with `Timeout` (`Terminal`, `FailedConclusively`) retain the declaration's inquiry hold, and record that its overlap class owes one reply (its window: the request's ambiguity window, at least the reply skew). Overlapping same-target inquiries wait; disjoint inquiries and ACK-bearing commands are not gated beyond the ordinary inquiry hold. Datagram keeps the skew hold and the inquiry retry policy (#795). |
| Raw inquiry frame at exact reply-skew expiry | Complete input wins over the due pass. Stale unkeyed inquiry data/reply and socketless-error evidence remain filtered, while live attributable ACK/completion (including a uniquely attributable socketless completion) and exact named socket terminals remain eligible. Retained partial-byte ambiguity follows the engine-owned time grace before an orphan is discarded (#713). |
| Retryable conclusive rejection before ACK (buffer-full `0x03`/`0x05`, movement `0x41` for a non-STOP movement request), no cancel intent | Increment the bounded attempt and enter `Backoff`. A typed STOP refused with `0x41` is terminal with `Error::CommandNotExecutable`: the refusal has a standing cause (a focus STOP under auto-focus) and the caller needs the verdict at once. |
| Camera error after ACK (`Executing` or `AwaitingCancellationResolution`), either envelope | Never retried: `finish` with `Error::CommandFailedAfterAck` wrapping the camera's exact error (`Terminal`/`Unconfirmed`, not retryable). The acknowledged command may already have moved (#795). |
| Completion deadline in `Executing`, Sony envelope, no cancel intent | `finish` with `Timeout` (`Terminal`/`Unconfirmed`); never resend. The same-sequence retransmission in docs/visca_reference.md §5.3 recovers a lost message, and an ACK already proved acceptance (#795). |
| Retryable rejection with cancel intent | Suppress retry and `finish` with `Cancelled`, because no executing attempt exists. |
| `0x04` command-cancelled terminal | `finish` with `Cancelled`. |
| ACK timeout, Sony envelope, retryable | Retry within policy on ACK-capped backoff, replaying the exact sequence. |
| Transient receive fault while a Sony command is `AwaitingAck` | Retry each eligible command independently using uncapped backoff and its retained sequence. Commands whose retry count or budget is exhausted fail with the exact receive error, without an added `FailureContext`; unrelated requests and the session remain live (`receive_fault_fails_only_unretryable_work_and_never_the_session`). |
| Transient raw receive fault in `AwaitingAck` with no recorded cancel intent | Default: leave the unacknowledged command in `AwaitingAck`; it is neither replayed nor failed on the fault, and a later ACK may still assign its socket. If its ACK deadline subsequently expires without an ACK, apply the raw unconfirmed-recovery row below. A command with recorded cancel intent instead remains live through its cancellation-resolution bound. |
| Raw ACK deadline expiry without an ACK, completion loss, a completion-only command's completion deadline in `AwaitingCompletion` (issue #700), or retry-budget expiry in an active raw phase | Default: immediately `finish` with `Error::UnsequencedCommandUnconfirmed`, remove the request, and install a keyed `PreAck`, `Socket`, or `AllResponses` hold through the ambiguity interval. Late frames covered by that hold are inert (#671/#723/#724). On a raw byte stream the ledger owes a pre-ACK answer instead of a `PreAck` hold, and a `Socket` hold lasts until evidence releases it (#795). |
| Keyed raw hold expiry | Release only the target/scope key whose deadline elapsed and reconsider queued work. No terminal event occurs here because the request was already resolved; independent simultaneous holds remain in force. The one exception is an exact inquiry-shape hold retained for its owed raw stream reply, which latches with its ledger debt instead of releasing. |
| Cancellation-resolution deadline expiry | Finish with `UnsequencedCommandUnconfirmed` for raw or `CancellationUnconfirmed` for Sony. Socketed raw terminals retain `Socket`; pre-ACK cancellation expiry retains `PreAck` on a datagram or a ledger debt on a stream. A datagram hold lasts until `max(now, phase deadline, cancellation ambiguity deadline) + ambiguity`, so it is never created already expired; a raw stream retains exact socket evidence until its answer releases it (#835; `cancellation_holds::unconfirmed_cancel_write_failure_and_resolution_expiry_retain_future_socket_hold`). |
| A transient raw receive fault in `AwaitingAck` with no recorded cancel intent, or any raw unconfirmed-recovery trigger above, under the `strict_unconfirmed_poison` opt-in | Poison the session and report `Error::StreamPoisoned`, restoring the pre-#671 behavior. A recorded cancel instead follows its live cancellation-resolution path and poisons only if that deadline remains unconfirmed. |
| Retry becomes eligible (`Backoff` → ready) | Return to `Ready` and dispatch through the ordinary capacity and pacing gates. |
| Owner halt fence over a `Ready` or `Backoff` request, a retry decision for a fenced request, or admission of a fenced submission | `finish` (or reject admission) with `Error::MotionSuperseded`. Its context is `PreAdmission`/`NotAccepted` for a rejected submission; otherwise `Terminal`, with `NotAccepted` when no written attempt can have taken effect (never written, or every written attempt rejected before ACK) and `Unconfirmed` when an earlier attempt lost its ACK, completion, or receive path (#795). |
| Cancel in `Ready` or `Backoff` | Remove without I/O; emit `CancellationRecorded`, then `finish` with `Cancelled`. |
| Cancellation decision for a `Sending` command | Refuse with `CancellationNotSupported`; this state cannot record intent. A public cancellation arriving during a transport write is processed only after its result, under the resulting lifecycle (#860). |
| Cancel in `AwaitingAck` (supported target) | Record requested intent, suppress retries, and wait for socket assignment or the later of the ACK and cancellation ambiguity deadlines (#835). |
| Cancel in `AwaitingCompletion` | Refuse the cancellation observation with `CancellationNotSupported`, record no intent, and preserve the completion-only command's original completion deadline. Its own completion can still apply it; an unconfirmed raw datagram expiry retains `AllResponses` (#835; `cancellation_holds::completion_only_cancel_preserves_window_and_late_completion_cannot_apply_successor`). |
| Cancel after a raw request has terminalized into a keyed hold | Return `Ignored(UnknownRequest)`. The hold is inert correlation state, not a cancellable request, and its deadline is unchanged. |
| Cancel in `Executing` (supported target) | Record intent, emit one socket cancellation, and retain the original completion correlation through the command's completion deadline; cancellation ambiguity does not shorten exact socket correlation (#724). |
| Cancel after transmission on a target without socket cancellation | The cancellation observation fails with `Error::CancellationNotSupported`; record no intent, send no frame, and leave the original request active. |
| Cancel of an inquiry | The cancellation observation fails with `Error::InquiryNotCancelable`. |
| Repeated cancel from the same handle | The owner observes the handle's one existing intent and gives the engine no second input (#777). |
| Duplicate cancel reaching the engine | `Ignored(DuplicateCancellation)`. |
| Successful cancel send | Transition to `AwaitingCancellationResolution` with `AwaitingTerminal`; wait for the original completion (`Completed`) or the protocol-cancel terminal (`Cancelled`) through the later of the retained completion and ambiguity deadlines. |
| Failed datagram cancel send | Resolve the cancellation observation with its error, retain the original request and exact routing, and never retry that cancellation for the same socket assignment. An `Executing` request remains live through `max(completion deadline, cancellation ambiguity deadline)` (#724; `failed_cancel_write_does_not_shorten_an_executing_commands_completion_window`). A later unconfirmed terminal retains its future phase-scoped hold (#835). |
| Failed stream cancel send | Poison the session and resolve every entry and observer. |
| Observer detach or timeout | No engine input and no protocol transition. The expired wait reports `ObservationTimeout` (#783). |
| Close / shutdown / poison | Resolve every active entry once, in admission order, with the distinct terminal error; clear every index and transmission; reject or drain boundary admissions deliberately. |

Non-retryable camera and transport errors stay exact even when cancel intent
exists. `Cancelled` is never used merely because a caller stopped waiting or the
engine lost certainty. When one input emits both a cancellation acknowledgement
and an immediate terminal result, `CancellationRecorded` precedes `Terminal`.

The terminal conditions are distinct errors, and `Error::requires_new_session()`
separates a dead session from a recoverable one:

| Terminal condition | Error | `requires_new_session()` |
| --- | --- | --- |
| The peer closed the connection | `ConnectionClosed` | `true` |
| The stream position became unknowable, or the strict opt-in poisoned an unconfirmable raw command | `StreamPoisoned` | `true` |
| A sent raw command's outcome cannot be correlated (default per-request mode) | `UnsequencedCommandUnconfirmed` | `false` |
| The application shut the session down | `RuntimeShutdown` | `false` |
| The owner task or worker unwound before publishing a normal terminal | `OwnerTerminated` | `true` |

## Operational invariants

The engine and its serialized owner uphold the following invariants in every
supported configuration. They are the properties the deterministic engine, the
bounded owner boundaries, and the `#![forbid(unsafe_code)]` crate attribute
exist to guarantee. Enforcement labels identify the authority: **audit** is
`ProtocolEngine::assert_invariants` in debug/test builds; **types** is the
closed representation; **owner** is serialized engine/owner transitions,
receipt delivery and bounded boundary code; **unsafe** is the crate attribute.
A historical/temporal property is identified with its owner enforcement rather
than claimed as a snapshot-auditor check.

- Runtime mutation is serialized per transport. (Enforcement: owner: the one serialized
  coordinator.)
- Every admitted request has exactly one authoritative entry until safe terminal removal.
  (Enforcement: types: private entry table and atomic index changes; owner: sole
  admission path and safe terminal removal.)
- Every request and cancellation transmission has one active identity and one result. (Enforcement:
  types: identity and framing expectation belong to the active sending lifecycle; owner: named
  receipts match that identity, and `duplicate_write_receipts_do_not_requeue_the_request` pins
  stale receipt rejection.)
- Every request reaches at most one terminal engine transition. (Enforcement: owner: sole-entry
  terminal removal and non-reused request identity.)
- Every retained operation or cancellation observer resolves at most once. (Enforcement: owner:
  one-shot terminal and cancellation slots.)
- Every target/socket pair has at most one active owner. (Enforcement: types: atomic entry-table
  transitions and the closed active/displaced placement.)
- No retry path writes a command again after its ACK. (Enforcement: type: retry transitions require
  an eligibility value issued only by unacknowledged lifecycles.)
- Every live raw byte-stream ledger entry belongs to a request that still awaits that answer; the
  ledger exists only on a raw stream session, a frame resolves only against an entry that can
  legally produce it, and a debt never blocks a STOP or poisons the session. (Enforcement: types:
  only `RawStream` owns a ledger; audit: ledger ownership, phase, dispute and capacity rules;
  owner: STOP dispatch.)
- Every `AwaitingCompletion` entry is completion-only, and every raw completion-only/no-reply
  positional candidate owns its target lane exclusively for ordinary work. On a raw byte stream an
  urgent STOP may cross that exclusion while the ledger keeps its evidence distinct (#795;
  `raw_evidence::stream_owed_completion_only_answer_holds_back_ordinary_work`). (Enforcement: owner: request-shape transition selection; types: phase-local
  representation; audit: target-local dispatch candidates.)
- Every keyed raw hold is inert: it can filter late input and gate successors, but it cannot consume
  a response, accept cancellation, or recreate a request. (Enforcement: types: holds have no request
  lifecycle; owner: resolver/dispatch gates.)
- Raw holds exist only in raw correlation modes, and a raw stream cannot install a `PreAck` hold.
  (Enforcement: types: `Sequenced` owns no hold table, and `HoldTable<StreamScope>` excludes
  `PreAck`.) Stream hold expiry requires ledger evidence; exact socket holds do not expire by
  time, and an owed inquiry reply retains its shape. (Enforcement: types: the stream expiry API
  requires its ledger; owner: exact evidence-release transitions.)
- Every `AwaitingCancellationResolution` entry has non-`None` cancellation state and owns its exact
  target/socket correlation. (Enforcement: types: resolving cancellation has no none state;
  entry-table transitions own the exact socket.)
- Every sequence owner is an active, target-compatible, and phase-compatible entry. (Enforcement:
  types: entry-table transitions update full-width/lower-16 indexes with bounded retry history.)
- Stale external admission, cancellation, or transmission identities cannot send or resolve work.
  (Enforcement: owner: checked non-repeating identities and active lifecycle lookup.) Ready queues
  and due actions have no stale ticket representation.
- A cancel transmission is emitted at most once for one socket assignment. (Enforcement: owner: emit
  only from the requested substate, then transition to sending.)
- Observer removal never mutates protocol state or releases protocol capacity early. (Enforcement:
  owner: receipt wait/detach boundaries.)
- Malformed frames cannot panic. Duplicate, stale, reordered and unsolicited input cannot mutate
  unrelated state within the documented camera-ordering and raw correlation-window assumptions.
  (Enforcement: owner: strict decoding/correlation; types: bounded target/socket keys.)
- No user decoder, callback, subscriber, transport I/O, or sleep runs while mutable engine state is
  borrowed. (Enforcement: owner: effect/I/O boundary after the engine borrow ends.)
- The owner copies or moves each effect out of the engine and ends the mutable engine/observer
  borrow before transport I/O, borrowing the engine again only to apply the identified result.
  (Enforcement: owner: EngineTurn and copied/moved effects.)
- No slow observer or subscriber can block protocol progress. (Enforcement: owner: bounded
  nonblocking observer/subscriber delivery.)
- Request-identity wraparound cannot alias active or quarantined work. (Enforcement: owner: checked
  fail-closed allocators; types: private entry-table index updates.)
- All boundary, engine, observer, diagnostics, framing, and subscription memory has a documented
  bound. (Enforcement: audit: engine collection bounds; types/owner: bounded lanes, buffers and
  registries.)
- Stream poison, close, and shutdown cannot leave a retained waiter blocked indefinitely.
  (Enforcement: audit: terminated-session emptiness; owner: terminal publication and boundary
  drain.)
- A fresh session contains no state from a poisoned session. (Enforcement: owner: new registries and
  fresh protocol authority.)
- The runtime code implementing this architecture contains no `unsafe`. (Enforcement: unsafe:
  #![forbid(unsafe_code)].)
- A returned blocking operation handle names an admitted request. Its initial write may still be
  pending or may fail through its outcome (D24;
  `runtime::owner::blocking::tests::submission_succeeds_at_admission_and_a_failed_write_is_the_outcome`).
  (Enforcement: owner: admission reply; write failures stay on the operation outcome.)
- Target-local state changes occur only from exact `AppliedStateEffect` delivery and never depend on
  an observer. (Enforcement: owner: exact effect applied before observer delivery; types: target
  key.)
- Built-in encoding and per-transmission framing allocate nothing; lifecycle allocations remain
  fixed and admission-bounded. (Enforcement: types: bounded inline frame storage; owner:
  admission-bounded lifecycle storage.)

Sony sequences may remain owned through `Backoff`, `Ready` and `Sending` only
with evidence of a retained retry attempt. Private full-width and lower-16
indexes change with phase-compatible history. Raw inquiry correlation derives
from live phases and decoder-declared payload legality. Streams retain their
first-answer write order in the ledger; datagram ownership is unique by
pairwise overlap exclusion, without a duplicate inquiry send-order field.

A ledger `accept` decision returns at most `1 + 2 * MAX_DISPUTE_HANDOFFS` = 7
retroactive frames: its opening error, then each of the three handoffs' ACK and
optional named terminal. The opening error cannot carry a named follow-up.
`Answered` stores all seven inline. Deferred evidence belongs to one local
engine decision, never engine state; that decision can collect one such result
per target plus two socketless candidates (7 * 7 + 2 = 51 frames inline), and
at most two distinct displaced owners per target (14 inline). It applies
displaced owners before retroactive frames at the existing drain points and is
consumed before effects return to the owner. The largest real ledger decision
and the all-target inline capacity have explicit tests.

A terminated engine retains no live entry, sequence, socket, hold, ledger debt
or queue (#856, #859).

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
fault and spending no retry budget). Reads that already waited for their full
budget do not receive an extra pause or spend the receive-flood fairness streak.
When a pause ends at a response or grace deadline, one receive-first probe
preserves complete input before the due pass. Fault pauses are armed before
write effects; staging the next transmission clears its receive pause, while
a successful write resets idle escalation (#838).
The coordinator paces only the receive
source, so boundaries, timers, and control stay selectable during the pause in
both shells, and an eager custom driver cannot hot-spin either owner (#723).

<!-- BEGIN WRITE-ONLY STATE KEYS -->
`PanTiltLimit`, `PresetsRecallSpeed`, `FocusLock`, `ExposureSpotlight`, `ExposureAutoSlowShutter`, `NdFilterMode`, `NdFilterAuto`, `ImageFreeze`, `ZoomDigitalZoom`, `StreamingMulticast`, `StreamingNdiQuality`, `TallyBrightness`, `PanTiltSpeedMode`, `TallyMode`, `ImageFlipMode`, `MotionSyncSpeed`.
<!-- END WRITE-ONLY STATE KEYS -->

## Request and motion semantics

Plain commands normally complete when the owner has protocol-applied them; a raw
`NoReply` plain command instead reports only a successful local transport write.
Typed inquiries return their decoded response. Targeted operations have a meaningful
target state and expose applied completion plus `Settlement` evidence. `settled`
returns exact profile completion or stable samples with axes, window, and
tolerance; stable samples do not prove arrival at the endpoint. Applied-only operations represent actuation without a
meaningful target state and expose
applied completion only. `Operation<operation::Targeted>` has `applied`,
`settled`, `cancel`, and `detach`; `Operation<operation::AppliedOnly>` has no
settled operation. Both profile selections use these same concrete handles.

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

The owner's cancellation intent is a closed requested/recorded state. A
refusal is returned synchronously by the same engine cancellation input, rather
than extracted from a later unrelated turn. Admission claims likewise use
unbounded/pending/claimed states, so a claim cannot be both missing and pending
(#854).

Cancellation is one intent per handle. The first `cancel` creates the
cancellation slot and ships it with the request; a later `cancel` — after a
timeout or a dropped call — finds the owner already holding that intent and
observes it, so the wire carries at most one cancellation. A refusal
(`CancellationNotSupported`, `InquiryNotCancelable`) installs no intent and leaves the
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
explicit operation lifecycle. `motion().stop_all()`,
`motion().is_moving(...)`, and `motion().wait_until_idle(...)` are the only
camera-level motion safety/observation entry points. A dropped handle is not
an automatic STOP; emergency stopping is an explicit STOP or motion operation.

`stop_all()` is one bounded owner transaction returning `HaltReport`.
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
[issue #542](https://github.com/GrantSparks/grafton-visca/issues/542#issuecomment-6901336636).
