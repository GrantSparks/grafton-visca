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

The answer is yes: v2 is larger in source and in a small linked blocking
consumer. Source and integration-test figures are physical `wc -l` totals over
the stated Rust files. Both optimized library builds and both preserved
consumer fixtures use Rust 1.98 (`cargo +1.98.0`), fresh separate target
directories, and blocking-only features; the consumer manifests retain their
existing release `strip = "symbols"` setting.

| Measure | 1.2.0 | v2 RC | Change |
| --- | ---: | ---: | ---: |
| Rust lines under `src/` plus macro `src/` | 84,146 | 97,682 | +16.1% |
| Rust lines under `src/runtime/` | 17,848 | 26,243 | +47.0% |
| Integration-test Rust lines | 19,563 | 24,342 | +24.4% |
| Optimized library `.rlib` (blocking-only) | 8,887,736 B | 11,319,258 B | +27.4% |
| Stripped minimal TCP/power/zoom blocking consumer | 767,680 B | 1,018,696 B | +32.7% |

The final executable measurement is illustrative rather than a universal
application-size promise: monomorphization, enabled features, linker settings,
and which API paths a program calls all affect the result. It does establish
that the increase is not merely comments or tests.

Growth is concentrated in the new lifecycle/owner implementation, not in a
second set of camera opcodes. The command, protocol, and transport trees grew
only modestly; the runtime tree grew by roughly 8.4k lines. The main additions
are the deterministic engine, separate mode-native owner drivers, typed
preparation and operation state, bounded admission/cancellation boundaries,
multi-target state, settlement polling, state cache, diagnostics/metrics, and
the parity/release gates around them. The proc-macro crate also gained
`proc-macro-crate` so derives work when a downstream user renames the
dependency; its TOML parser stack is a compile-time cost, not an async runtime
linked into blocking applications.

Crucially, the size increase is not Tokio hidden behind the blocking facade.
`blocking` calls `BlockingTransport` synchronously and its normal dependency
graph contains no Tokio, smol, async executor, or pollster. A CI dependency
gate now enforces that for blocking network and serial configurations.

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

The concern about rewriting battle-tested functionality is justified. Across
the production source comparison against main at `6c7a9d37`, including the
current working-tree corrections, 54,039 lines were added and 40,503 deleted
(`git diff --numstat` over the package and macro `src/` trees). Git cannot
recognize most of the new facade/runtime files as renames.
Several capabilities then had to be restored after parity review, including
direction helpers, normalized zoom and ND-filter conveniences, menu toggling,
horizontal-flip disable, 2D/3D noise-reduction disable, serial single-camera
constructors, live tuning, and submission priority.

The high-quality boundary is:

- **Reuse unchanged protocol knowledge.** Built-in command encoders, inquiry
  parsers, value validation, framing, envelopes, transport adapters, and
  profile evidence remain authoritative. Noun methods construct those request
  types; they do not reproduce byte vectors.
- **Share new semantics once.** `prepared` performs profile-aware lowering for
  both modes, the engine owns lifecycle transitions for both modes, and the
  noun table generates all public facade variants.
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
   User}`. Neither route can create or demote urgent work.
2. Valid non-empty async receive batches retain strict priority over
   simultaneously ready control sources. Only a non-progressing receive yields
   to shutdown, cancellation, admission, control, and timer, in that fixed
   order. This removes the arbitrary eight-read fairness flip without
   reintroducing the always-failing-transport livelock.
3. The Rust 1.98 compile-pass and compile-fail gates use the repository's
   declaration-based harness rather than toolchain-sensitive rendered
   diagnostics. The pinned job checks the declared compile contract directly.
4. `AffectedAxes` stores `NonZeroU8`; zero has no public or internal value,
   deserialization revalidates, schema bounds are 1 through 31, and a possibly
   empty intersection returns `Option<AffectedAxes>`.
5. Recoverable cancellation refusal is approved and retained.
6. Hardware verification remains a stable-release gate, not a prerequisite to
   calling the software work release-candidate complete.
7. Raw VISCA does not use universal FIFO pre-ACK attribution. It keeps one
   unacknowledged command candidate per target across `Sending`, `AwaitingAck`,
   and `AwaitingLateAck`, then reopens socket-level concurrency as soon as ACK
   establishes ownership. Raw ACK and error routing never uses command FIFO or
   temporal recency. A socketless error routes that unique candidate only with
   no inquiry owner; with no unacknowledged command it may route the legitimate
   per-target inquiry FIFO, while a command-plus-inquiry collision is ignored.
   An explicit socket routes only its exact target/socket owner, and a
   socketless error never targets `Executing`. A named ACK socket is equally
   exact: if another request owns that socket, the ACK is inert with
   `SocketConflict` rather than being remapped to a different free socket;
   only a socketless ACK may select the first free registered socket. Sony
   sequencing retains pre-ACK pipelining and exact sequence correlation.
8. A successfully sent raw command is never automatically replayed after an
   ACK, completion, or cancellation ambiguity timeout, or after a raw receive
   fault while awaiting ACK. An active retry-budget expiry in `Sending`,
   `AwaitingAck`, or `Executing` has the same poison result. The session becomes
   unusable because both physical outcome and future reply correlation are
   uncertain; callers must replace it and reconcile camera state. Same-sequence
   Sony recovery and retries after conclusive camera rejection remain
   supported.
9. Broadcast setup commands (Address Set and I/F Clear) and socket
   cancellation are owner-coordinated lifecycle controls, not public generic
   plain commands. Public cancellation is available only through the exact
   operation handle.
10. Intrinsically urgent cancellation bypasses queued ordinary work but never
    bypasses physical command pacing.
11. Fixed-format ACK, completion, error, and network-change frames require
    their exact protocol lengths; a known prefix with trailing bytes is
    malformed, not a valid response or an unknown extension. Fixed ACK,
    completion, and error socket nibbles are likewise strict: `0` is the
    documented socketless compatibility form, `1` and `2` are the numbered
    sockets, and `3..=15` are malformed rather than another socketless
    response. This distinction must remain explicit because
    `ViscaSocket::from_protocol_byte` alone returns `None` for both wire
    forms.
12. Private runtime identifiers do not wrap into reuse. The practically
    unreachable 64-bit exhaustion boundary fails closed so stale cancellation
    or transmission inputs cannot alias a later request.
13. One admission-to-terminal retry budget remains active through every later
    noncancelled attempt phase, including backoff, ready, send, ACK, execution,
    and reply; expiry preserves the prior retry cause instead of replacing it
    with an incidental later phase timeout. Cancellation quarantine is separate
    and is never shortened by budget expiry. Empty UDP datagrams are discarded
    without resetting that receive deadline.

## Release-candidate boundary

Development-complete issue closure should require implementation, contract
tests, documentation/migration notes, public-API snapshot review, MSRV/lint,
and the supported feature matrix. It should not claim physical-camera evidence.
The stable 2.0 release remains blocked on the hardware checklist, while an RC
tag is explicitly allowed to carry pending hardware rows.
