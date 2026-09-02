# Issue 542: v2 design and implementation review

This review compares the 1.2.0 `main` implementation at `6c7a9d37` with the
2.0 release-candidate branch based on `1a561b6c`, including the release-candidate
corrections made during this review. Hardware evidence is deliberately outside
the software-complete conclusion below.

## Conclusion

The v2 architecture is the stronger long-term design for protocol ownership:
one owner serializes admission, wire I/O, correlation, retry, cancellation,
settlement, state-cache mutation, and observations. Its pure engine is easier
to replay and reason about than distributing lifecycle authority among client
objects. Typed targeted/applied-only operations and recoverable cancellation
also make more invalid lifecycle states impossible.

It is nevertheless a substantial rewrite, not a thin 1.x facade evolution.
That choice raised the parity and maturity risk, and the sequence of restored
1.x conveniences and controls confirms that the risk was real. The right
release-candidate posture is therefore:

- keep the new owner and typed-lifecycle architecture;
- keep blocking and async as genuinely mode-native drivers over shared pure
  protocol/preparation logic;
- reuse the established wire encoders, decoders, framing, transports, value
  types, and profile evidence;
- generate facade projections from one semantic/noun registry; and
- reject duplicate domain implementations where the canonical type now covers
  the same domain.

## Size

This is a source-only audit snapshot, measured on 2026-09-01 against the 1.2.0
base `6c7a9d3783861189745536372c4d21de24d4252d` and the then-current review
tree at `HEAD` `888aa07a66ecec0b493b84e402915c636bdeeed1` plus its pending
changes. The selected current Rust-file content digest was
`718b9df45eef5d59f96d2a78cdfe51af7d4f37e41c0220339e7c575263caa196`; it
includes the then-untracked `grafton-visca-macros/src/range_type.rs`. These are
not claims about a later worktree or release commit: remeasure after any source
change.

For each checkout/tree, obtain the three line rows from its root with
`find src grafton-visca-macros/src -type f -name '*.rs' -print0 | xargs -0 wc
-l`, `find src/runtime -type f -name '*.rs' -print0 | xargs -0 wc -l`, and
`find tests -type f -name '*.rs' -print0 | xargs -0 wc -l`, using each
command's trailing `total` row. The current-tree source delta was measured as
`git diff --numstat 6c7a9d3783861189745536372c4d21de24d4252d -- src
grafton-visca-macros/src`, plus every untracked Rust file reported by `comm
-13 <(git ls-files -- src grafton-visca-macros/src | sort) <(find src
grafton-visca-macros/src -type f -name '*.rs' -print | sort) | xargs -r wc
-l`. At this snapshot that contribution was the 293-line `range_type.rs`; a
future clean checkout needs only the `git diff` command.

| Measure                                           |   1.2.0 base | 2026-09-01 review snapshot |  Change |
| ------------------------------------------------- | -----------: | -------------------------: | ------: |
| Rust lines under `src/` plus macro `src/`         |       84,146 |                    127,370 |  +51.4% |
| Rust lines under `src/runtime/`                   |       17,848 |                     46,614 | +161.2% |
| Rust lines under `tests/` (integration-test tree) |       19,563 |                     31,162 |  +59.3% |
| Optimized library `.rlib` (blocking-only)         | not measured |               not measured |       — |
| Stripped minimal TCP/power/zoom blocking consumer | not measured |               not measured |       — |

No fresh binary artifacts were retained for this source-only snapshot, so the
table deliberately makes no `.rlib` or executable-size claim. When a release
measurement is needed, use Rust 1.98 (`cargo +1.98.0`) with fresh separate
target directories and blocking-only features: `cargo +1.98.0 build --release
--no-default-features --locked` for 1.2.0, the same command with `--features
blocking` for v2, and `cargo +1.98.0 build --manifest-path <fixture>/Cargo.toml
--release --locked` for the preserved consumer fixture. Record the exact
commit, features, linker settings, and resulting artifact sizes alongside that
measurement; monomorphization and called API paths make it illustrative rather
than a universal application-size promise.

The snapshot establishes substantial source growth, including 28,766 additional
runtime lines. Its main additions include the deterministic engine, separate
mode-native owner drivers, typed preparation and operation state, bounded
admission/cancellation boundaries, multi-target state, settlement polling,
state cache, diagnostics/metrics, and parity/release gates. The proc-macro
crate also gained `proc-macro-crate` so derives work when a downstream user
renames the dependency; its TOML parser stack is a compile-time cost, not an
async runtime linked into blocking applications.

The blocking facade remains synchronous through `BlockingTransport`; its normal
dependency graph contains no Tokio, smol, async executor, or pollster. The CI
dependency gate covers blocking network and serial configurations.

## Design comparison

| Area | 1.x | v2 RC |
| --- | --- | --- |
| Protocol authority | Mature client/runtime design with fewer layers | One explicit owner and pure deterministic lifecycle engine |
| Blocking | Native blocking and smaller | Native blocking retained; independent feature and no async runtime dependency |
| Async | Native async, selected as the crate mode | Native async with caller-selected executor; Tokio and smol can coexist with blocking |
| Lifecycle API | Mature but more caller-visible lifecycle vocabulary | Typed plain/inquiry/targeted/applied-only requests and owner-derived identity |
| Cancellation | Familiar 1.x behavior | Refusal returns `CancelRejected<H>`, preserving the still-live handle |
| Scheduling | Caller priority applied broadly | Intrinsic `ControlClass` plus non-urgent caller `SubmissionClass`; STOP cannot be demoted |
| State/diagnostics | Smaller and simpler surface | Target-local applied-state cache, bounded diagnostics, metrics, and explicit recovery state |
| Multi-target/profile validation | Less centralized | Immutable registry and pre-I/O topology/profile/envelope validation |
| Surface consistency | Battle-tested hand-authored API | One noun table generates blocking, async, dynamic, and semantic projections |
| Maturity | Released and field-used | Software release-candidate evidence; hardware evidence still pending |

The principal v2 cost is complexity. There are more states, queues, receipts,
and boundary error paths to validate. Maintaining separate blocking and async
drivers is intentional because forcing blocking clients through an async
runtime would violate a core product requirement, but only the I/O-driving
shells should differ: correlation, retry, preparation, classification, and
terminal-state rules belong in shared pure code.

## Reuse audit

The concern about rewriting battle-tested functionality is justified. In the
2026-09-01 source snapshot defined above, the production-tree comparison with
the 1.2.0 base had 87,141 added and 43,917 deleted lines (the documented
`git diff --numstat` command plus the then-untracked 293-line range macro).
Git cannot recognize most of the new facade/runtime files as renames. Those
counts are snapshot evidence, not a claim about subsequent working-tree
corrections.
Several capabilities then had to be restored after parity review, including
direction helpers, normalized zoom and ND-filter conveniences, menu toggling,
horizontal-flip disable, independently gated 2D/3D noise-reduction inquiries
and controls, serial single-camera constructors, live tuning, and submission
priority.

The high-quality boundary is:

- **Reuse unchanged protocol knowledge.** Built-in command encoders, inquiry
  parsers, value validation, framing, envelopes, transport adapters, and
  profile evidence remain authoritative. Noun methods construct those request
  types; they do not reproduce byte vectors.
- **Share new semantics once.** `prepared` performs profile-aware lowering for
  both modes, the engine owns lifecycle transitions for both modes, and the
  noun table generates all public facade variants. Profile-dependent response
  facts follow the same seam: preparation selects an immutable owned decoder,
  so blocking, async, and dynamic facades cannot disagree. Numeric response
  domains that already have a validated public value type remain
  profile-neutral; in particular, every supporting PTZOptics profile decodes
  the full `NoiseReduction3DLevel` domain (#717).
- **Rewrite only where ownership semantics changed.** The sole-owner engine,
  bounded admission, typed settlement, multi-target registry, and recoverable
  cancellation could not be obtained safely by merely renaming the 1.x client
  facade.
- **Do not retain parallel domain models.** During this review the semantic
  ledger's hand-written `BuiltinAxes` bitset/iterator/validator was replaced by
  the canonical non-empty `AffectedAxes` implementation.

Future parity work should compare semantics and wire transcripts, not port
method bodies mechanically. When a 1.x helper is just a checked constructor or
delegation over an established request type, v2 should project that existing
implementation through the noun table rather than rewrite its validation or
encoding.

## Decisions from this review

1. `ControlClass::Urgent` remains request-owned safety metadata. Public
   per-handle and per-call QoS uses `SubmissionClass::{Background, Normal,
   User}`, and the raw escape hatch's `raw::Policy` rejects
   `ControlClass::Urgent` at construction (#679). No public route — QoS override
   or raw policy — can create or demote urgent work. The raw hatch likewise
   rejects the owner-only socket-cancel and interface-clear wire shapes (#678),
   so no target-scoped `execute` can emit an owner-only primitive.
   **Extended (issue #700):** the raw hatch no longer assumes every command
   follows the ACK-then-completion protocol. `raw::Policy` carries a
   `raw::RawReplyShape` axis (`AckThenCompletion` default, `CompletionOnly`,
   `NoReply`), lowered through `prepared.rs` into the engine's `RequestContext` as
   a protocol-policy fact — the engine never infers the shape from wire bytes. A
   `CompletionOnly` command skips `AwaitingAck`, owns no socket, holds its
   target's command channel exclusively (so its socketless completion cannot
   misbind), and has no ACK-timeout path; a `NoReply` command terminates on a
   successful send. This is what makes a *legitimate* completion-only vendor frame
   expressible without it failing at an ACK deadline it will never meet. It does
   **not** relax the owner-only rejection above: a socket cancel or interface
   clear stays refused at construction regardless of reply shape, and `#678`'s
   `validate_wire` guard is independent of it.
2. Valid non-empty async receive batches retain strict priority over
   simultaneously ready control sources. Only a non-progressing receive yields
   to shutdown, cancellation, admission, control, and timer, in that fixed
   order. This removes the arbitrary eight-read fairness flip without
   reintroducing the always-failing-transport livelock. **Revised (issue #675):**
   the progress-sensitive rule alone bounded only the *failing* arm; a peer that
   returned a valid frame on every poll (the *succeeding* arm) still won the
   left-biased selection forever and starved every boundary source, so
   `close()` never returned and an emergency stop could not be admitted (fatal
   on smol, latent on tokio). The decision now adds a bounded **fairness
   ceiling** — after a run of consecutive receive-first wins tied to
   `frames_per_receive`, one boundary-first turn is forced regardless of
   progress — which restores #625's own acceptance criterion (the boundary
   channels are always eventually polled) for the succeeding arm too. This is a
   bounded backstop, not the old open-ended history counter, and it does not fire
   for real reply bursts, so the settle-first ordering is preserved. The same
   revision makes the async owner enforce `read_timeout` and `write_timeout`
   around the transport (they were previously inert on the async surface) and
   paces a run of immediately-returning no-data reads so an idle transport cannot
   hot-spin.
3. The Rust 1.98 compile-pass and compile-fail gates use the repository's
   declaration-based harness rather than toolchain-sensitive rendered
   diagnostics. The pinned job checks the declared compile contract directly.
4. `AffectedAxes` stores `NonZeroU8`; zero has no public or internal value,
   deserialization revalidates, schema bounds are 1 through 31, and a possibly
   empty intersection returns `Option<AffectedAxes>`.
5. Recoverable cancellation refusal is approved and retained.
6. Hardware verification remains a stable-release gate, not a prerequisite to
   calling the software work release-candidate complete.
7. Raw VISCA does not use universal FIFO pre-ACK attribution. It normally keeps
   one unacknowledged command candidate per target across `Sending`,
   `AwaitingAck`, `AwaitingCompletion`, and `AwaitingLateAck`. One intrinsic
   `Urgent` command may cross one existing candidate as the #714 safety-lane
   exception; while both are open, unsequenced ACK/error evidence binds to
   neither. `AwaitingCompletion` is the
   completion-only shape: it holds the target channel exclusively and never
   earns a socket. For an ACK-bearing command, socket-level concurrency reopens
   as soon as ACK establishes ownership. Raw ACK and error routing never uses
   command FIFO or
   temporal recency. A socketless error routes that unique candidate only with
   no inquiry owner; with no unacknowledged command it may route the legitimate
   per-target inquiry FIFO, while a command-plus-inquiry collision is ignored.
   An explicit socket routes only its exact target/socket owner, and a
   socketless error never targets `Executing`. A named ACK socket is exact when
   that socket is free; if another request owns it — typically because a lost
   completion made the camera reuse the socket — the camera's new assignment
   supersedes the stale local claim (#721). The old request relinquishes the
   socket into an unkeyed ambiguity quarantine and the new request owns the
   named socket for completion and cancellation. A socketless ACK still selects
   the first free registered socket. Sony sequencing retains pre-ACK pipelining,
   exact sequence correlation, and the bounded #620/#682 other-free-socket
   compatibility fallback. Raw correlation holds follow the same evidence
   boundary: cancellation and unconfirmed-correlation holds for an owned S1/S2
   release only that exact target/socket, while pre-ACK or otherwise unowned
   ambiguity remains unkeyed. These scopes remain distinct when simultaneous.
   A terminal `NoReply`/`CompletionOnly` tombstone is instead a broad
   target-response hold. A matched raw inquiry reply creates no hold; an
   uncertain timeout/error/retry release retains only the profile's short
   reply-skew hold. That narrow scope blocks a new same-target inquiry and
   filters stale unkeyed inquiry data/reply and socketless-error evidence, while
   ACK-bearing commands (including `Urgent`), live attributable completion
   (including a uniquely attributable socketless completion), and exact named
   socket terminals remain eligible (#712).
8. A successfully sent raw command is never automatically replayed after an
   ACK, completion, or cancellation ambiguity timeout, or an active retry-budget
   expiry in `Sending`, `AwaitingAck`, `AwaitingCompletion`, or `Executing` — a
   raw command may already have reached the camera. A transient raw receive fault
   while the command awaits ACK likewise never authorizes a replay, but it does
   not itself terminalize the command by default: the command remains awaiting
   ACK, and only a later ACK deadline without an ACK enters the unconfirmed
   recovery below. The `strict_unconfirmed_poison` opt-in instead poisons the
   session on that receive fault only when no cancel intent is recorded; a
   recorded cancel follows cancellation-driven late-ACK resolution and poisons
   only if that deadline remains unconfirmed. It still never replays the
   command.
   **Ratified per-request default (issue #671, superseding the earlier
   whole-session poison rule and the #565/#566 narrowing):** each resulting
   unconfirmed-recovery trigger — an ACK, completion, or cancellation ambiguity
   timeout, or an active retry-budget expiry — fails only that one command with
   `UnsequencedCommandUnconfirmed` — a
   per-request outcome the session survives (its `requires_new_session()` is
   `false`) — and quarantines the correlation still at stake (the owned socket,
   or the command's place as the sole unacknowledged raw command) until the
   ambiguity deadline, so a late response in that hold cannot bind to a later
   command; the hold filters only the evidence within its own scope. The session
   and every unrelated request keep running. Whole-session
   poison is retained only behind the opt-in `strict_unconfirmed_poison`
   `OperationalTuning` mode (default off), which restores the pre-fix behavior
   and surfaces it as `StreamPoisoned` so a poisoned session still requires a
   replacement. Same-sequence Sony recovery and retries after conclusive camera
   rejection (buffer-full `0x03` / no-socket `0x05` / movement `0x41`) remain
   supported.
9. Broadcast setup commands (Address Set and I/F Clear) and socket
   cancellation are owner-coordinated lifecycle controls, not public generic
   plain commands. Public cancellation is available only through the exact
   operation handle.
10. Intrinsically urgent cancellation bypasses queued ordinary work but never
    bypasses physical command pacing.
   **Ratified clarification for #673/#714:** issue #542 §4's older blanket
   sentence that blocking submission “never waits for ACK” is superseded by two
   bounded decisions. An ordinary ACK-bearing operation may drain the sole live
   raw ACK-capable predecessor under its own ACK budget (#673). A lost-ACK
   `AwaitingLateAck` predecessor instead gives ordinary first dispatch a timed
   wait to its ambiguity deadline, never generic contention. An intrinsically
   `Urgent` stop bypasses that drain and the one-candidate gate, subject to
   command pacing and socket capacity; if two candidates are open, ACK/error
   evidence binds to neither (#714). `CompletionOnly` and `NoReply` are never
   draining successors, while a cancellation-driven late-ACK state remains
   eligible only when its ACK is still accepted. The maintainer decision is
   recorded on the [#673 issue trail](https://github.com/GrantSparks/grafton-visca/issues/673#issuecomment-5508129187).
11. Fixed-format ACK, completion, error, and network-change frames require
    their exact protocol lengths; a known prefix with trailing bytes is
    malformed, not a valid response or an unknown extension. Fixed ACK,
    completion, and error socket nibbles are likewise strict: `0` is the
    documented socketless compatibility form, `1` and `2` are the numbered
    sockets, and `3..=15` are malformed rather than another socketless
    response. This distinction must remain explicit because
    `ViscaSocket::from_protocol_byte` alone returns `None` for both wire
    forms. **The classifier verdict is decoupled from the session consequence
    (revised for #672/#674/#681):** a frame the framer already delimited at an
    `FF` boundary but that this strict classifier rejects is a *malformed frame
    to discard* (`Ignored(MalformedFrame)`, counted in
    `OwnerMetrics::ignored_malformed_frames`) on a byte stream, exactly as a
    datagram already discards it and as 1.x logged-and-continued — it is never a
    framing poison. Only the framer losing its position (cumulative buffer
    overflow, or a boundary-free read past `max_buffer_size`) still poisons a
    stream. By the same rule, a stream read that decodes more than
    `frames_per_receive` frames stops at the limit and leaves the remainder
    buffered for the next receive (the framer retains bytes across reads) rather
    than failing `ResponseTooLarge`, and a single-target IP session accepts any
    `0x9y..=0xFy` reply source and attributes it to the sole target, restoring
    the misaddressed-camera compatibility (#590/#598) 1.x had. The strict
    per-frame classification itself is unchanged; only the consequence of a
    rejected delimited frame is corrected.
12. Private runtime identifiers do not wrap into reuse. The practically
    unreachable 64-bit exhaustion boundary fails closed so stale cancellation
    or transmission inputs cannot alias a later request.
13. One admission-to-terminal retry budget remains active through every later
    noncancelled attempt phase, including backoff, ready, send, ACK, execution,
    and reply; expiry preserves the prior retry cause instead of replacing it
    with an incidental later phase timeout. A cancellation quarantine, and the
    per-request unconfirmed quarantine of item 8, are separate holds governed by
    their own ambiguity deadline and are never driven or shortened by budget
    expiry. Empty UDP datagrams are discarded without resetting that receive
    deadline.
14. Async `shutdown` is the idempotent, non-joining signal; consuming `close`
    is the deterministic transport-teardown barrier. `close` waits until the
    sole owner has finished its boundary drain and dropped its driver/transport,
    without promising an executor-specific task join; it maps an explicit
    `RuntimeShutdown` terminal result to success, and preserves a transport or
    poison cause that won the documented source ordering.
15. Timeout and admission ownership follow the lifecycle boundary rather than
    the transport boundary. A validated profile owns exact per-category
    `CommandTimeouts`; operational tuning may make those facts more
    conservative; `SessionConfig` owns bounded admission capacity. Socket,
    serial, connect, read, write, buffer, and keepalive settings remain transport
    concerns, but command retry and scheduler queue policy do not.
16. Sony receive metadata preserves whether a sequence is full-width or only a
    potentially truncated lower 16 bits. Full-width identity is exact;
    lower-16 identity is usable only when the target-compatible owner is unique,
    and a collision is inert rather than guessed.
17. The historical 1.x behavior guide and its retained direct v2 tests
    distinguish preservation from reviewed v2 safety changes. They must never
    force temporal raw command attribution, ambiguous raw replay, fixed retry
    timing, or another superseded behavior back into production merely because
    1.x once implemented it.
18. At a byte-stream correlation-expiry boundary, complete buffered frames are
    correlated before the due release; partial bytes are not decoded as frames.
    Unless the due release also includes the broad target-response tombstone
    from a `NoReply` or `CompletionOnly` terminal, a named completion/error
    prefix may survive another socket's release only when it names a still-live
    non-releasing target/socket owner. Matching stale socket evidence may be
    discarded and other-target serial input is retained. That broad tombstone
    instead discards every response-shaped partial prefix for its target,
    including source-only, ACK, socketless `0x50`/`0x60`, and named terminal
    prefixes; noncorrelating evidence and other-target serial input are
    preserved. Source-only, ACK (whose socket nibble is assignment preference
    rather than ownership), and socketless `0x50`/`0x60` prefixes remain
    ambiguous for narrower inquiry/pre-ACK/unkeyed or socket-scoped releases,
    so input continues to drain first. The former 64-turn fail-closed rule had
    no stable wall-clock duration and is superseded by #713: both owners use
    one engine-owned grace interval, bounded by the read timeout and 100 ms. A
    completing tail is processed under the old scope; an unresolved orphan
    prefix is discarded and reported malformed at expiry without poisoning the
    session. The historical decision and supersession are recorded on the
    [#713 issue trail](https://github.com/GrantSparks/grafton-visca/issues/713#issuecomment-5508129430).

## Release-candidate boundary

Development-complete issue closure should require implementation, contract
tests, documentation/migration notes, public-API snapshot review, MSRV/lint,
and the supported feature matrix. It should not claim physical-camera evidence.
The stable 2.0 release remains blocked on the hardware checklist, while an RC
tag is explicitly allowed to carry pending hardware rows.
