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

* A blocking session drives that owner on the caller's thread.
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

There are no public callbacks, user-supplied lifecycle IDs, unbounded queues,
or per-camera background workers. Drop is not a protocol motion action:
dropping an operation handle detaches its observation, and dropping a
non-final shared async `Session` or camera view leaves the shared owner and
other views running. Dropping the final async owner handle may release the
detached owner and transport; a blocking `Session` owns its caller-thread host
and releases that host and transport by RAII when dropped. Neither release path
issues a protocol STOP. Use `shutdown` for an explicit shutdown signal or
consuming `close` for the async deterministic release barrier. An operation
handle must be explicitly cancelled or detached according to its documented
lifecycle.

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

The two execution modes share protocol state-machine semantics, not an async
implementation hidden behind a blocking wrapper. `blocking` drives
`BlockingTransport` directly and has no Tokio, smol, futures executor, or
pollster dependency in its downstream graph. `async` drives `AsyncTransport`
through the caller-selected executor. CI checks the native blocking dependency
boundary for the network, serial, and `test-utils` feature sets; the async
testkit's executor rides on `async`, so enabling `test-utils` on a blocking-only
build links no executor.

Internally, both shells complete engine input through the same executor-free
`EngineTurn` policy. A complete turn runs due work, pending cancellation, and
one ordinary dispatch; a deadline-only turn withholds ordinary dispatch while
an exact first write is reconsidered; an input-only turn preserves retained
wire evidence ahead of deadlines at the same sampled instant. These are
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
its ambiguity deadline; any late ACK, completion, or error for that target is
ignored before it can bind to later response-bearing work. Another `NoReply`
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
named socket is instead held by another request — the classic cause is a lost
completion frame that made the camera reuse the socket — the camera's new
assignment supersedes the stale local owner (#721). The older request releases
the socket, immediately fails as unconfirmed, and leaves an inert keyed
`PreAck` hold; the uniquely identified successor owns the named socket for its
completion and any cancellation packet.
A socketless ACK still selects the first free registered socket. The bounded
#620/#682 other-free-socket fallback remains only for sequence-correlated Sony
traffic, whose later terminals have an independent request identity.

For a raw socketless error, the evidence rule is equally strict: route the
unique unacknowledged command only when no inquiry owner is live; otherwise
route the legitimate per-target inquiry FIFO only when no unacknowledged
command exists. A command-plus-inquiry collision is ignored. An explicit
socket routes only the exact owner of that target/socket, and a socketless
error never falls back to an `Executing` command.

### Raw unconfirmed outcomes and strict recovery

Retry follows the same evidence boundary. Sony timeout recovery resends the
same logical request with the same sequence. A conclusive camera rejection
such as buffer-full or no-socket proves that the command did not start and may
be retried under policy. A raw command that was successfully written but then
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
one command's camera effect rather than replacing the session. An active
retry-budget expiry in `Sending`,
`AwaitingAck`, `AwaitingCompletion`, or `Executing` follows the same per-request
rule, while a retry still in a safe ready/backoff state finishes with its
retained last error when the total budget expires. The whole-session poison is
retained only behind the opt-in `strict_unconfirmed_poison` session policy (default off),
which restores the pre-fix behavior and surfaces it as `Error::StreamPoisoned`
so those callers still establish a fresh session. The budget applies to every
later noncancelled retry phase; an active cancellation lifecycle or an inert
keyed hold is separate and is never shortened by budget expiry.

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

### Blocking raw dispatch and the urgent safety lane

Blocking operation submission has one additional ownership boundary: a
returned operation handle always names a request whose initial transport write
already succeeded. An ordinary `AckThenCompletion` submission may drain the
sole live ACK-capable raw predecessor, bounded by its own ACK budget, when that
pre-ACK gate is the only obstacle (#673). Recorded cancellation may extend that
live `AwaitingAck` phase to its ambiguity deadline, and it remains drainable so
an ACK can assign a socket and trigger the cancel. Once an uncancelled ACK
deadline passes, however, the request is terminal and only an inert keyed
`PreAck` hold remains; ordinary work receives through that hold boundary and
then writes, without trying to rescue the old request. Neither case is reported
as generic contention; the async owner queues to the same deadline
(#714/#723/#724).

An intrinsically `Urgent` stop skips the #673 drain and may cross one raw
positional candidate after command pacing. The resulting explicit
two-candidate state makes every unsequenced ACK/error ambiguous, so it binds to
neither request and either handle may later report
`UnsequencedCommandUnconfirmed`; that uncertainty does not retract the stop
bytes already sent to the camera. A `CompletionOnly` successor still requires
target idleness and does not meet either exception. Genuine socket-capacity
contention (every command socket already occupied), re-entrancy, and losing the
global dispatch race remain fail-fast `Error::TransportBusy` boundaries.

A deadline-bound blocking submission that has not written by its caller
deadline returns no receipt, regardless of whether its class ordinarily allows
queueing. The owner terminalizes that exact ready entry before returning
`Timeout`, releasing its admission permit and preventing a later scheduler turn
from writing work the caller can no longer observe (#723).
Ordinary blocking commands, inquiries, and owner-internal requests retain
bounded queueing.

These are the ratified #673/#714 exceptions to issue #542 §4's older blanket
sentence that blocking submission “never waits for ACK.” This repository
records the superseding design decision; the GitHub issue bodies remain
historical and are not claimed to have changed.

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
| Admit over capacity | Reject with `Error::RuntimeQueueFull`; no id, entry, observer, or queue slot is created. |
| Admit with the id/generation space exhausted | Reject with `Error::RuntimeIdentityExhausted`. |
| Admit to a non-`Running` session | Reject with the session's terminal error (or `Error::RuntimeShutdown`). |
| Select a `Ready` request | Transition to `Sending`, allocate one `TransmissionId`, emit exactly one request `Transmit` (Sony carries its retained sequence; raw carries none). |
| Ordinary blocking first dispatch behind a live raw `AwaitingAck` candidate or inert `PreAck` hold | Drain a live ACK-capable candidate within its request bound. For an inert hold, return a correlation `WaitUntil`, receive through its boundary, and then write; the old terminal request cannot be rescued. Never classify either known bound as `TransportBusy` (#714/#723/#724). The async scheduler queues to the same evidence boundary. |
| `Urgent` raw command behind one positional candidate | Skip the blocking pre-ACK drain and cross the single-candidate gate after command pacing when socket capacity remains. This is the sole two-candidate exception (#714). |
| Successful command send (`AckThenCompletion`, the default) | Record any Sony sequence and transition to `AwaitingAck`. |
| Successful command send (`CompletionOnly`, issue #700) | Transition straight to `AwaitingCompletion` (no ACK phase, no socket); apply any completion that raced the write result and drop any spurious raced ACK. |
| Successful command send (`NoReply`, plain raw only) | Finish the plain `execute()` after the local write succeeds; this is not protocol application and cannot create an operation handle. Retain the bounded broad `AllResponses` hold before same-target raw response-bearing command or inquiry work may start. Another `NoReply` may write and extend that key. |
| Successful inquiry send | Record any Sony sequence, transition to `AwaitingReply`, and take a per-target FIFO position for a raw inquiry. Production Raw policy has one live inquiry per target, the precondition for its unkeyed-reply hold; a socket-owned command may coexist. |
| Failed command send, datagram transport | Terminally fail that one request with the exact transport error; every other entry keeps running. |
| Failed command send, stream transport | Poison the session (`Error::StreamPoisoned`) and resolve every active entry. |
| Compatible failed stream write sampled after the request's total budget | Poison before applying per-request late-result policy: the stream seam cannot prove that no partial frame reached the wire (#724). |
| Request still `Sending` when its total retry budget expires | Raw active command: immediately fail unconfirmed and install the applicable keyed hold (or strict-poison); raw single-flight inquiry: install its target-only reply-skew hold and fail; Sony: fail without registering late sequence metadata. A write result sampled exactly at the deadline remains input-first. |
| Request write result sampled strictly after its total retry budget | Apply the same expired-`Sending` policy before the result can register correlation, consume a deferred frame, report `Written`, or replace a retained retry cause. Cancellation writes use their separate ambiguity lifecycle. |
| ACK in `AwaitingAck` with a free socket | Assign the socket and transition to `Executing`; if cancel intent is `Requested` on a supported target, emit one socket cancellation. An ACK covered by an inert `PreAck`/`AllResponses` hold is ignored and cannot resurrect its terminal owner (#723). |
| Unsequenced ACK/error while two raw positional candidates are open | Ignore it as ambiguous and bind it to neither candidate; never use admission order or recency (#714). |
| Raw ACK naming a busy socket | Treat the camera's named socket as authoritative: immediately fail the stale local owner unconfirmed, leave its inert keyed `PreAck` hold, and assign the named socket to the uniquely resolved successor (#721/#723). |
| Sequenced Sony ACK naming a busy socket | Use the target's other free socket when available for #620/#682 compatibility; otherwise remain inert as `Ignored(SocketConflict)`. |
| ACK while still `Sending` | Latch it once as a deferred ACK, applied when the send result lands. |
| Completion in `Executing`, including after cancellation ambiguity elapsed | `finish` with `RuntimeOutcome::Applied`; exact socket ownership keeps it attributable through the completion deadline, and a retained cancellation observer maps this to `Completed` (#724). |
| Exact completion in `AwaitingCancellationResolution` before its phase deadline | `finish` with `RuntimeOutcome::Applied`; the request is still open while its owned socket is retained. |
| Completion in `AwaitingCompletion` (issue #700) | `finish` with `RuntimeOutcome::Applied`, regardless of any socket nibble the vendor frame echoes; the resolver already established it as the sole completion-only candidate on the target. Retain the bounded broad `AllResponses` hold before same-target raw response-bearing command or inquiry work starts; a later `NoReply` may only extend that key. |
| Inquiry reply in `AwaitingReply` | `finish` with the attributed payload. A matched Raw reply installs no hold and releases the single-flight lane immediately (#712). |
| Uncertain Raw inquiry response-correlation release | A terminal error/timeout or retry release/requeue retains the profile's short `raw_inquiry_reply_skew` before another same-target inquiry may send. ACK-bearing commands, including `Urgent`, are never gated by this inquiry-only hold. It filters stale unkeyed inquiry data/reply and socketless-error evidence, while live attributable ACK/completion (including a uniquely attributable socketless completion) and exact named socket terminals remain eligible (#712). |
| Raw inquiry frame at exact reply-skew expiry | Complete input wins over the due pass. Stale unkeyed inquiry data/reply and socketless-error evidence remain filtered, while live attributable ACK/completion (including a uniquely attributable socketless completion) and exact named socket terminals remain eligible. Retained partial-byte ambiguity follows the engine-owned time grace before an orphan is discarded (#713). |
| Retryable conclusive rejection (buffer-full `0x03`/`0x05`, movement `0x41`), no cancel intent | Increment the bounded attempt and enter `Backoff`. |
| Retryable rejection with cancel intent | Suppress retry and `finish` with `Cancelled`, because no executing attempt exists. |
| `0x04` command-cancelled terminal | `finish` with `Cancelled`. |
| ACK timeout, Sony envelope, retryable | Retry within policy on ACK-capped backoff, replaying the exact sequence. |
| Transient receive fault while a Sony command is `AwaitingAck` | Retry every such eligible command independently using uncapped backoff and the retained sequence; unrelated requests and the session remain live. |
| Transient raw receive fault in `AwaitingAck` with no recorded cancel intent | Default: leave the unacknowledged command in `AwaitingAck`; it is neither replayed nor failed on the fault, and a later ACK may still assign its socket. If its ACK deadline subsequently expires without an ACK, apply the raw unconfirmed-recovery row below. A command with recorded cancel intent instead remains live through its cancellation-resolution bound. |
| Raw ACK deadline expiry without an ACK, completion loss, a completion-only command's completion deadline in `AwaitingCompletion` (issue #700), or retry-budget expiry in an active raw phase | Default: immediately `finish` with `Error::UnsequencedCommandUnconfirmed`, remove the request, and install a keyed `PreAck`, `Socket`, or `AllResponses` hold through the ambiguity interval. Late frames covered by that hold are inert (#671/#723/#724). |
| Keyed raw hold expiry | Release only the target/scope key whose deadline elapsed and reconsider queued work. No terminal event occurs here because the request was already resolved; independent simultaneous holds remain in force. |
| Cancellation-resolution deadline expiry | `finish` with `Error::UnsequencedCommandUnconfirmed` for a raw session or `Error::CancellationUnconfirmed` for a Sony session. This is the end of a still-live cancellation lifecycle, not release of a `CancelState::None` quarantine marker. |
| A transient raw receive fault in `AwaitingAck` with no recorded cancel intent, or any raw unconfirmed-recovery trigger above, under the `strict_unconfirmed_poison` opt-in | Poison the session and report `Error::StreamPoisoned`, restoring the pre-#671 behavior. A recorded cancel instead follows its live cancellation-resolution path and poisons only if that deadline remains unconfirmed. |
| Retry becomes eligible (`Backoff` → ready) | Return to `Ready` and dispatch through the ordinary capacity and pacing gates. |
| Cancel in `Ready` or `Backoff` | Remove without I/O; emit `CancellationRecorded`, then `finish` with `Cancelled`. |
| Cancel in `Sending` (supported target) | Record `Requested` intent; the send result drives the next state. |
| Cancel in `AwaitingAck` (supported target) | Record `Requested` intent, suppress retries, and wait for socket assignment or the ambiguity deadline. |
| Cancel after a raw request has terminalized into a keyed hold | Return `Ignored(UnknownRequest)`. The hold is inert correlation state, not a cancellable request, and its deadline is unchanged. |
| Cancel in `Executing` (supported target) | Record intent, emit one socket cancellation, and retain the original completion correlation through the command's completion deadline; cancellation ambiguity does not shorten exact socket correlation (#724). |
| Cancel after transmission on a target without socket cancellation | The cancellation observation fails with `Error::NotSupported`; record no intent, send no frame, and leave the original request active. |
| Cancel of an inquiry | The cancellation observation fails with `Error::InquiryNotCancelable`. |
| Duplicate cancel of the same request | `Ignored(DuplicateCancellation)`. |
| Successful cancel send | Transition to `AwaitingCancellationResolution` with `AwaitingTerminal`; wait for the original completion (`Completed`) or the protocol-cancel terminal (`Cancelled`) through the later of the retained completion and ambiguity deadlines. |
| Failed datagram cancel send | Resolve the token with the error (`ObservationFailed`), retain the original request and routing, and never retry that cancel for the same socket assignment. An `Executing` original remains governed by its completion deadline, not the earlier cancellation ambiguity deadline (#724). |
| Failed stream cancel send | Poison the session and resolve every entry and observer. |
| Observer detach or timeout | No engine input and no protocol transition. |
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

The async actor expresses simultaneous readiness as deterministic,
progress-sensitive phases rather than a randomized race or a history
threshold:

1. Receive is first while it produces a valid non-empty frame batch. Buffered
   ACKs, completions, and replies therefore update protocol state before a
   simultaneously ready control observer sees that state.
2. A receive that makes no protocol progress — idle/no-data, a partial frame,
   a transient fault, an empty UDP datagram discarded under the current overall
   deadline, or a receive whose only frames were discarded as malformed (a whole
   bad datagram, or delimited-but-unclassifiable frames on a stream) — makes the
   next selection poll boundary sources first in the fixed order shutdown,
   cancellation, admission, control, then timer. Discarding an empty datagram
   never starts a fresh deadline; the async UDP adapter also yields cooperatively
   before polling again.
3. A boundary win, or a later valid frame batch when no boundary was ready,
   returns the actor to receive-first.
4. A **fairness ceiling** bounds the *succeeding* arm as well. After a run of
   consecutive receive-first wins (tied to the receive batch limit,
   `frames_per_receive`), one boundary-first turn is forced regardless of
   progress, then the run restarts. This is the case #625's acceptance criterion
   names directly — the boundary channels are always eventually polled — and it
   is what keeps a *babbling* peer (one that returns a valid frame on every
   poll) from winning the left-biased selection forever and starving shutdown,
   cancellation, admission, control, and the timer. When no boundary is queued
   the receive still wins the forced turn, so a genuinely busy transport is
   never stalled; only guaranteed to yield the front periodically. A burst large
   enough to reach the ceiling is adversarial rather than a real camera's reply
   stream, so the settle-first ordering of (1) still holds for real traffic.

This retains the protocol's strict source order for meaningful input while
preventing both an always-idle/always-failing transport **and** an
always-succeeding (babbling) one from starving shutdown and control. Phase
choice depends on the result of the current receive; the only count involved is
the bounded fairness ceiling of (4), never an open-ended receive history.
Transmission effects produced by either phase are driven immediately before the
next selection.

Raw-correlation release does not create an exception to that boundary order.
At a newly due raw hold the owner first obtains the mandatory ordered input
proof (so input which became ready after a pre-expiry idle read still wins); a
forced fairness turn still puts shutdown, cancellation, admission, control, and
the release timer first. During a retained-prefix grace, that same complete
boundary lane races the receive against the engine-owned grace deadline. An
exact no-input fence is scoped to the latched release set and remains reachable;
an immediately-idle custom transport is paced against the real grace deadline,
not the already-expired raw hold. If the due set grows before its Wake, the
shared release-turn coordinator replaces the latch and requires another
receive-first proof for the replacement. Thus an Urgent stop remains admissible
under raw input pressure, while no scope is released using evidence collected
before its own deadline (#746).

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
10–250 ms, next-wake/caller-deadline-clamped pause the transient-fault path uses
(recording no fault and spending no retry budget). Async maps that decision to
an executor sleep; blocking maps it to a caller-thread sleep, so an eager custom
driver cannot hot-spin either owner (#723).

## Request and motion semantics

Plain commands normally complete when the owner has protocol-applied them; a raw
`NoReply` plain command instead reports only a successful local transport write.
Typed inquiries return their decoded response. Targeted operations have a meaningful
target state and expose applied plus settled completion. `settled` observes the
profile-selected protocol settlement condition, not a bench-verified assertion
of physical rest. Applied-only operations represent actuation without a
meaningful target state and expose
applied completion only. Dynamic handles preserve this distinction:
`DynTargetedOperation` has `applied`, `settled`, `cancel`, and `detach`, while
`DynAppliedOperation` has no settled operation.

Use the noun view for ordinary controls and `submit` when a caller needs an
explicit operation lifecycle. `motion().stop_all_motion()`,
`motion().is_moving()`, `motion().is_moving_axes(...)`, and
`motion().wait_until_idle(...)` are the only camera-level motion
safety/observation entry points. A dropped handle is not an automatic STOP;
emergency stopping is an explicit STOP or motion operation.

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
