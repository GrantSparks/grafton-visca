# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### 2.0.0-rc.1 release candidate

- **Every normative issue-542 trace fixture is now replayed through the
  production engine and owner** (#634). Four lifecycle fixtures — observer late
  delivery, deadline classes, capacity and failures, and PTZOptics
  cancellation — previously had no production replay at all, and the two that
  did were weak: the engine replay rebuilt its expected reply route, reply
  payload and camera error code from the fixture's *input* columns, so an
  engine that corrupted every inquiry reply or collapsed every camera error
  code onto one still passed; the owner replay consumed 17 of 70 records and
  ignored the expectation columns entirely. All five lifecycle fixtures now
  replay record for record against the real `OwnerState` — the real protocol
  engine, admission permit pool, terminal observers and applied-state
  subscribers — and the protocol replay reads its reply route, payload bytes,
  attributed socket and error code out of the engine's own effects. The
  1,357-line hand-written simulator the fixtures used to be checked against is
  deleted; only the fixture-integrity checks a production replay cannot make
  (versioned trace format, scoped scenario coverage, one recognized normative
  #542 clause per expectation) survive, in
  `tests/issue_542_trace_fixture_contract.rs`. The blocking out-of-order
  fixture's first ACK block was corrected: it asserted that a raw VISCA ACK —
  which carries no request identity — could be attributed to the *second* of
  two outstanding commands. Raw ACKs are attributed in transmission order and
  the socket the ACK carries becomes that request's socket, so out-of-order
  settlement is expressed at completion, which is keyed by target and socket
  together. No library behavior changed.

- **Pinned the movement wire encodings to golden byte vectors, and closed two
  validation gaps the suite could not see** (#633). Transposing the pan and
  tilt fields of the absolute pan/tilt encoder, or swapping two rows of the
  direction table, previously passed the entire test suite: the directional
  coverage compared a helper frame against an explicit frame that routed
  through the same table, and the only fixtures naming the bytes outright had
  been orphaned out of the build. Every pan/tilt direction, the absolute and
  relative position frames, and the zoom, focus, preset, and power encodings
  now have absolute byte-vector assertions that no encoder recomputes. The
  raw frame-size bound gets a fixture that is otherwise valid — legal address
  byte, legal terminator — so deleting the `raw::MAX_BYTES` branch fails a
  test instead of being covered by the terminator rule, and
  `raw::validate_axes` is now exercised through every public axis-carrying
  constructor.
- **An operation that names no affected axis is rejected at preparation**
  (#633). `AffectedAxes::NONE` and the `BitAnd` of two disjoint sets have been
  constructible since #624, and `raw::Targeted` / `raw::AppliedOnly` already
  refused them at construction — but a targeted operation that reached
  preparation with an empty set lowered to a settlement plan that issued zero
  position inquiries and reported "settled" without observing the camera.
  Preparation now rejects an empty set for both completion kinds with the
  same `Error::InvalidRequest` the raw path uses. This can change behaviour
  for a downstream `OperationCommand` implementation that returns an empty
  `affected_axes()`, which the trait has always documented as a non-empty set:
  such an operation now fails admission instead of silently completing
  unobserved. No public signature changed. Motion observation is deliberately
  unaffected — `motion().is_moving(MotionQuery::new(AffectedAxes::NONE))`
  still returns `Ok(false)` for 1.x parity, because a query may legitimately
  select nothing even though an operation must name what it moves.
- Restored `image().disable_horizontal_flip()` on all three noun surfaces
  (#635). The rewrite ledgered `BuiltinCommand::ImageFlipHorizontal` under the
  single method spelling `enable_horizontal_flip`, so the noun surfaces only
  ever emitted `ImageMirrorCommand::new(true)`. On a profile with
  `HasImageMirror` but no `HasCombinedImageFlip` — SonyFR7, SonyBRCH900,
  SonyEVIH100 — no noun method could return the mirror to off: `set_flip_mode`
  is gated on the combined marker and rewrites the vertical axis too. 1.x
  paired the two directions. The mirror-off opcode value (`81 01 04 61 03 FF`)
  now has its own ledger row, `BuiltinCommand::ImageFlipHorizontalOff`, so the
  per-row gates and the cross-surface parity test enforce both directions the
  way they already do for the vertical flip and multicast pairs. Like every
  single-axis flip opcode, it invalidates `StateKey::Flip` rather than
  half-setting the pair.
- **Packaging and release-machinery polish** (#642). `grafton-visca-macros`
  now ships `LICENSE-MIT` and `LICENSE-APACHE` in its published tarball: it
  declares `MIT OR Apache-2.0` and both licences require their text to
  accompany the distribution, and its README linked two files the package did
  not contain. `deny.toml` is excluded from the published main crate — it is
  CI-only configuration, in the same class as the already-excluded `api/`
  baselines — and the `exclude` list now states why `CHANGELOG.md` and
  `CONTRIBUTING.md` are deliberately kept. The publication workflow pins its
  toolchain and `actions/checkout` to the versions `ci.yml` pins, and refuses
  to publish a tag whose commit has no successful `CI success` check run,
  replacing RELEASING.md's honour-system instruction with a gate. CI gains an
  advisory job that runs the release validator against the real repository
  with the intended next tag, so manifest and version drift surfaces on the
  pull request that introduces it rather than at publish time. The fuzz target
  gains a committed seed corpus of well-formed and malformed frames, so each
  bounded run starts warm and a crash can be pinned as a permanent regression
  seed. Two orphaned `.github/scripts` setup scripts that referenced a v0.x
  milestone and a nonexistent issue template — and that would have created
  real GitHub issues if run — are deleted. Documentation fixes: the retry
  budgets in `docs/observability_and_recovery.md` are now stated as retries
  rather than attempts and carry the missing `Movement`/`Preset` row, the
  busy-timeout term in the wall-clock budget and the backoff ceiling, the
  half-open jitter band, and the built-in-inquiry `0x02` retry path; the
  Windows CI job says that its serial tests drive a mock and that the Win32
  backend is compiled but never executed; and CONTRIBUTING's nightly install
  one-liner names the `rustfmt` and `miri` components that `--profile minimal`
  omits.
- **Replaced the fake terminator tests with real encode-path coverage** (#627).
  `tests/no_hardcoded_terminator_test.rs` had regressed to its pre-#586
  revision: one test asserted that a `const` it declared itself equalled `0xFF`
  and never touched the crate, another only `println!`ed and could not fail,
  and the surviving source scanner matched uppercase `0xFF` only — structurally
  blind to the lowercase literals in `src/` — while exempting any line
  containing `// `. The file now drives the production encode path
  (`Request::write_into`) for real typed commands across the power, pan/tilt,
  zoom, focus, iris, preset and system families, for built-in inquiries, for
  raw-frame admission, and for both transport envelopes, asserting that every
  frame ends with the exported `VISCA_TERMINATOR` and carries it exactly once.
  Because the frames are compared against the imported constant rather than a
  literal, the constant and the encoders can no longer drift apart. The scanner
  and `tests/terminator_validation_test.rs` (three file-existence and
  string-presence assertions) are deleted: a text scan cannot tell a hardcoded
  terminator from the legitimate `0xff` in response fixtures, simulator
  scripts, framer comparisons, error codes and the constant's own definition,
  so it can only be noisy or vacuous.
- Restored the 1.x convenience helpers the rewrite dropped, on all three noun
  surfaces (#569). `pan_tilt().up()/down()/left()/right()` are back as thin
  wrappers over `move_direction`; `zoom().set_normalized(UnitInterval)` and
  `set_normalized_in_domain(UnitInterval, ZoomDomain)` make `UnitInterval` a
  real noun input instead of an inquiry-conversion-only type;
  `nd_filter().set_stops(f32)` restores photographic-stop input;
  `motion_sync().set_speed(MotionSyncSpeed)` restores the range-checked speed
  argument; and `menu().toggle_display()` restores the vendor open/close
  control. `AffectedAxes` gains `ALL`, `MOVEMENT`, `NONE`, and the `BitOr` /
  `BitOrAssign` / `BitAnd` operators, so "wait for everything that moves" is
  one expression again; `IdleWait` gains `Default`, `From<Duration>`,
  `with_axes`, `with_timeout`, and the named `for_preset_recall` /
  `for_pan_tilt` / `for_zoom` / `for_focus` presets, and `MotionQuery` gains
  `Default` and `From<AffectedAxes>`. `motion().is_moving()` is a
  no-argument query again; the axis-selecting form keeps its 1.x spelling
  `is_moving_axes(MotionQuery)`. `Camera<P>::capabilities()` is back on the
  typed cameras, matching the dynamic projection. `StateCache` grows typed
  getters for all fifteen keys — `auto_slow_shutter()`, `spotlight()`,
  `flip_state()`, `pan_tilt_limits()`, and the rest — each documenting the
  per-key decoding of the previously untyped `[i64; 4]` payload, and a new
  `StateKey::Flip` records the combined horizontal/vertical flip pair (the
  single-axis flip and mirror opcodes invalidate it rather than half-setting
  it). The dynamic noun projection no longer carries
  `#![allow(missing_docs)]`: every method is documented from its async
  counterpart, and the non-ledger convenience wrappers are declared in
  `DYN_NOUN_CONVENIENCE_METHODS` so the closed-projection inventory gate
  still rejects anything undeclared.
- **Restored the owner metrics counters the rewrite dropped, and made the
  retry decision observable** (#571). `MetricsSnapshot` carries `ack_timeouts`,
  `completion_timeouts`, `inquiry_timeouts`, `busy_errors`, `protocol_errors`,
  `retries_scheduled` and `ignored_unmatched_sequenced_replies` again — the
  first numbers a field debugging session asks for in a crate whose failure
  modes are timing- and hardware-dependent. 1.x carried a single `timeouts`
  counter; the 2.0 engine distinguishes the ACK, completion and inquiry
  deadlines, so each is counted separately. `busy_errors` covers the codes the
  scheduler itself treats as transient camera-side backpressure — command
  buffer full (`0x03`), no socket (`0x05`), and not executable in the current
  state (`0x41`) — and `protocol_errors` covers every other error frame.
  1.x bucketed `0x03 | 0x04` as busy instead; `0x04` is this engine's
  cancellation reply, so counting it would make every successful cancellation
  look like camera backpressure, and it is now counted as neither. Both error
  counters count frames as they are decoded, including frames that no longer
  correlate to an active request. `retries_scheduled` counts wherever the
  engine emits a retry, so a transient receive-fault retry (#565) counts like
  any other.
- Added `DiagnosticEvent::DeadlineExpired` and `DiagnosticDeadline` (#571). A
  subscriber that needs to know whether an expired deadline led to another
  attempt reads `will_retry` off the event, instead of inferring it from a
  `Transition` plus the absence of a following `RetryScheduled` — which is what
  1.x's `SchedulerAction::Timeout { will_retry }` carried directly. The flag is
  the decision the engine actually took, not the policy that motivated it: a
  policy that permits retrying a deadline still fails the request once its
  attempt or duration budget is spent, and the event reports that honestly.
- **Single-camera constructors return a camera, with the profile bound at
  compile time** (#568). Every other entry point returns a `Session`, so a
  one-camera program had to name its profile a second time through
  `session.camera::<P>()?` — and that second naming was a runtime check, so
  opening with `PtzOpticsG2` and asking for `PtzOpticsG3` compiled and failed
  on the device. `Connect::open_tcp_camera::<P>` / `open_udp_camera::<P>` (and
  the blocking equivalents) now return a `CameraSession<P>` that owns its
  session and hands out the `P` camera view with no turbofish and no fallible
  projection; a mismatch is not expressible, as in 1.x. The configured forms
  are `CameraConfig::<P>::open_camera` / `open_camera_async`, and
  `CameraSession::open` takes a caller-owned transport with the same bind.
  `close` mirrors `Session::close`, and dropping the value tears the session
  down. The multi-camera `Session`/`camera_for` path is unchanged.
- **Restored the 1.x retry coverage the rewrite narrowed** (#566). A post-ACK
  completion timeout retries again — the rewrite hard-coded it off for every
  request class, so a camera that acknowledged a command and then went silent
  failed on its first deadline with no second attempt. A lost ACK retries for
  every retry class except `Never`, instead of only `Standard`, which had left
  all 32 movement requests and every preset dying on a single dropped ACK
  frame. Per-category retry budgets are back in 1.x's shape — quick and inquiry
  work gets two attempts more than the base, network work one fewer, and a
  long-running command exactly one — replacing three flat numbers keyed on the
  retry class. `OperationalTuning::retry_limit` overrides that *base*, which is
  the knob 1.x exposed, so a request's effective count is derived from its
  timeout category rather than taken literally.
- Sized the retry budget against the request's own deadline (#566). It was two
  seconds, shorter than every profile's completion deadline, so re-enabling
  completion retries alone would have changed nothing. The budget is now
  1.x's ten seconds or twice the request's governing deadline, whichever is
  larger, which admits exactly one further full-length attempt.
- Restored the ACK backoff exponent cap and added deterministic backoff jitter
  (#566). 1.x capped the ACK backoff exponent at five — 32x the initial delay —
  and left completion, inquiry, protocol-error and transport-fault retries
  uncapped; that distinction is back. **1.x had no jitter at all**, so the
  jitter here is new rather than restored: the rewrite's `maximum_backoff`
  ceiling makes concurrent retries converge on the same instant and stay there,
  which is the collision a backoff exists to break up. Each wait is now drawn
  from the equal-jitter band `[ceiling / 2, ceiling]`, so no request waits
  longer than 1.x would have. The draw is a pure function of the engine's seed,
  the request identity and the attempt number — never of wall-clock time or
  process entropy — so the engine remains a total function of its inputs and
  the exact sequence is pinned by test.
- **Restored the 1.x transport fault tolerance the rewrite dropped** (#565).
  A transient receive failure no longer destroys the session: the owner
  classifies the read error, and a transient one — the classic case is a UDP
  `recv` reporting ECONNREFUSED after an ICMP port-unreachable — retries every
  command still awaiting its ACK under that command's own bounded retry policy
  and keeps the session running, exactly as 1.x's `SchedulerEvent::NetworkError`
  did. Only a read that proves the connection is gone still ends the session,
  and it now ends it as a close rather than a byte-stream poison, because a
  failed read consumes nothing and cannot desynchronize framing.
- Socketless VISCA ACKs and completions work again (#565). A camera answering
  `90 40 FF` / `90 50 FF` carries no socket nibble; the transport adapter turned
  that into a hard `Error::InvalidResponse` that killed the whole session. The
  socket is now optional all the way to the scheduler, which assigns the first
  free command socket to a socketless ACK — 1.x behavior — and attributes a
  socketless completion by envelope sequence, or by sole socket ownership on
  raw VISCA. Genuinely malformed frames are still rejected.
- Restored socket reassignment on ACK (#565). A camera that names a command
  socket another request already holds no longer costs that command its ACK
  deadline: the ACK falls back to the target's other free socket, as 1.x did.
  A one-socket target has nothing to fall back to and the frame stays inert.
- Closed the #297 ACK race at the engine level (#565). An ACK that reaches the
  engine before the write result for the frame it answers is now latched on
  that request and applied the instant the write is confirmed, instead of being
  dropped. The defense no longer depends on owner call ordering, so a future
  concurrent reader and writer cannot silently reopen the race.
- Documented the stream write-failure policy explicitly (#565). A failed
  datagram write fails exactly one request, with its own transport error, and
  the session keeps running. A failed stream write poisons the session on
  purpose: no transport trait in this crate reports how many bytes of a frame
  reached the wire, so a partial write must be assumed and the byte-stream
  position treated as unknowable. `Error::StreamPoisoned` carries the exact
  transport cause in its reason and is the one terminal error that answers
  `Error::requires_new_session()` correctly (#564). 1.x drew the same line.
- A blocking pump that ends the session now reports the session's boundary
  error, not the raw transport cause (#629). A fatal read closed the session
  and then returned the underlying `Error::Io` to whoever was pumping. `Io`
  classifies as survivable, so a caller driving an auto-reconnect loop off
  `Error::requires_new_session()` was told to keep using a session the owner
  had already closed, and only learned the truth from the *next* call. The
  observation paths that wait on a receipt hid this — a closed session fails
  the request they are waiting on, and that terminal outcome carries the right
  error — but the paths with no receipt to consult did not: settlement polling
  between two position samples, above all. The verdict is now translated once,
  where every pump caller shares it, so a settlement wait, a cancellation
  observation, or any future pump caller sees `ConnectionClosed` (or
  `StreamPoisoned` for a framing failure on a stream), with the transport cause
  preserved in the reason. The async owner already reported boundary errors
  this way.
- Gated the async noun surface and added a cross-surface parity test (#570).
  Only `blocking_nouns` carried a per-row ledger gate, so deleting or
  misclassifying an async noun method failed no test: the published API
  snapshots diff each facade only against its own baseline and never against
  each other. `async_nouns` now carries the same gate against the closed
  `command::surface` ledger, and a new crate-private parity test reads all
  three noun facades — async, blocking, and the dynamic projection — and
  asserts they agree method for method on name, semantic return class, and
  capability bound, all derived from the ledger rather than from a
  hand-maintained expected table. That gate found eleven exposure rows
  (`brightness_*` and `compensation_*`) recorded in the ledger with only the
  noun's own marker while all three facades gate them on
  `HasBrightnessControl` / `HasExposureCompensation`; the ledger now records
  those typed markers. The hardcoded ledger-size assertions
  (146/143/115/16/15/66) are derived from `BuiltinCommand::ALL` and the
  generated inquiry table instead of written down, and the assertions that
  matched another file's formatted source text — which rustfmt could break
  with correct code — are replaced by derivations that survive reformatting.
  Test-only: the public API is unchanged.
- Operation-handle drop semantics match 1.x exactly: drop is `detach` and
  never stops hardware (#567). Dropping a handle relinquishes the observer and
  nothing else, so an early `?`, a panic unwinding past it, or a forgotten
  binding leaves physical movement running until an explicit stop ends it.
  This is documented rather than changed — 1.x behaved identically — so it is
  not a migration item, and the migration-table row that implied otherwise is
  corrected. Callers who want motion bounded by a scope write a stop-on-exit
  guard; the pattern is documented in `docs/migration_2_0.md` and demonstrated
  in `examples/operation_handles.rs` (and in its async wrapper form in
  `examples/operation_handles_async.rs`). No new public API.
- Added owner-backed `Session`/`SessionConfig` construction and typed
  `Camera<P>` views for heterogeneous multi-camera sessions.
- Added the final blocking, async, and dynamic noun surfaces with typed
  completion classes, profile capability gates, and shared operation
  lifecycle semantics.
- Standardized endpoint defaults, transport configuration, preflight
  validation, cancellation, retry, and observability behavior across the
  supported runtime and transport combinations.
- Removed the 1.x compatibility feature aliases and documented the 2.0
  request, construction, dynamic API, and release contracts.
- Added `Error::requires_new_session()`, the spec-normative classifier that
  separates transport-level session death (`ConnectionClosed`, `StreamPoisoned`,
  and the transport/channel-unavailable errors) from the deliberate
  `RuntimeShutdown` (#564). All of these share `ErrorKind::IoClosed`, so
  applications could not tell a field disconnect apart from a shutdown they
  requested without matching implementation details. `true` is positive proof
  that the session is finished and must be rebuilt from the retained
  `SessionConfig`; `false` only means the error alone does not prove it.
- Added a hardware release evidence checklist. Hardware, registry, and
  Synemantic validation remain `Pending (Not run)` until release owners record
  bench evidence.
- Ported the README and `prelude` quick starts to the 2.0 API and made them
  compile (#563). The README used the async-only `camera::Connect` path and
  called noun accessors and `submit` on `Session`; it now uses
  `blocking::Connect`, selects a typed view with `session.camera::<P>()`, and
  submits `request::builtin` commands. The `prelude` snippets now observe the
  `#[must_use]` operation handles with `applied()`/`settled()`, and the stale
  claim that `UnitInterval` carries normalized control values is corrected to
  the inquiry-conversion helpers that actually use it. A `cfg(doctest)`
  `include_str!` gate in `src/lib.rs` compiles every README snippet under
  `cargo test --doc`, so the front page can no longer drift from the API.
- Fixed a blocking submission that loses the dispatch race being terminalized
  as `Error::TransportBusy` (#561). A blocking caller holding more un-awaited
  operation handles than the target has command sockets now queues the excess
  work, which the owner writes as sockets free — matching both the async facade
  and 1.x. Admission capacity (`max_pending_queue_depth`) still bounds the
  queue and still rejects genuinely over-capacity submissions with
  `Error::RuntimeQueueFull`.
- Pinned every CI toolchain to an exact version (stable 1.98.0, nightly
  2026-08-26, MSRV 1.88.0) so the byte-compared gates stop breaking on
  unrelated pull requests whenever rustc releases, re-blessed the three
  `trybuild` contracts whose diagnostic wording drifted, regenerated the
  `api/2.0.0-rc.1` public API snapshots against the pinned toolchain, and
  installed that nightly explicitly in the snapshot job so `cargo public-api`
  can build rustdoc JSON instead of failing on a missing `nightly` toolchain
  (#562).
- **Removed the `tcp` feature** (#573). It was in the default set but gated no
  code — there was never a `cfg(feature = "tcp")` anywhere in the crate — so
  enabling or omitting it built exactly the same library while implying that
  standard TCP was optional. Standard TCP and UDP are not features: they are
  built from `std` and the runtime adapters and compile with whichever facade
  is enabled. Manifests that name `tcp` explicitly (`features = ["tcp"]`, or
  `default-features = false` plus `tcp`) must drop it; nothing else changes,
  because the default feature set is now `["blocking"]` and builds the same
  code it did before. This is a feature-list change made while 2.0.0-rc.1 is
  unpublished.
- Removed dead CI machinery and restored the coverage 2.0 had silently dropped
  (#573). Deleted `.github/workflows/test-features.yml`, a `workflow_call`-only
  workflow nothing invoked, and `.github/actions/setup-sccache/`, referenced by
  no workflow. `deny.toml` is now executed by a pinned `cargo-deny` job instead
  of sitting in the tree unenforced, and `cargo audit` runs again. Added a
  Windows job (the blocking serial transport is a different `serialport`
  implementation there) and a macOS check job. The `removed-features` job,
  which re-proved against a live compiler what the manifest inventory test
  already pins, was folded into `tests/issue_548_supported_surface_inventory.rs`.
  The README feature-union table promised automated validation of
  `runtime-tokio,transport-serial`, which no matrix leg covered; that leg is
  back, and the table now names the CI job that checks each union.
- Added the CI leg that runs the shipped test toolkit, and made a filtered test
  run fail when its filter matches nothing (#628). `test-utils` was never
  unioned with a facade in any job — the one `test-utils` leg selects neither
  `blocking` nor a runtime, and the all-features job is a `cargo check` — so 37
  tests existed in the tree and executed nowhere: all eight of
  `tests/issue_566_scripted_error_recovery.rs` (whose header claimed the
  opposite), five in `tests/inquiry_simulator_test.rs`, three in
  `tests/timeout_category_tests.rs`, the twenty `testkit`
  `deterministic_executor` and `scripted_transport` library tests that
  `CONTRIBUTING.md` tells contributors to build on, and one in
  `src/blocking.rs`. A `test-utils,blocking,runtime-tokio` leg in
  `.github/workflows/ci.yml` and `.github/scripts/test-all-features.sh` takes
  the count of never-executed tests from 37 to 0. Separately, every
  name-filtered run in `.github/scripts/miri-tests.sh` and the property-test
  entry in `.github/scripts/test-all-features.sh` now assert that the filter
  selected at least one test: libtest exits 0 on a filter that matches nothing,
  so a renamed module would have turned the whole Miri job green and vacuous.
- Stopped shipping the public API snapshots to crates.io and shrank them
  (#572). `api/` was 71% of the published tarball — 1.8 MiB of CI baseline text
  with no use to consumers — and is now in the `exclude` list, taking the
  package from 355 files / 2.5 MiB compressed to 347 files / 737 KiB. The seven
  byte-compared surfaces are reduced to three (`blocking`, `tokio-dyn`,
  `all-features`), which measurably cover the same items: `blocking` is a
  strict superset of the retired no-default surface, `tokio-dyn` of the retired
  async surface, and `all-features` of the rest. `--simplified` drops the
  compiler-emitted blanket impls that were ~42% of every file and identical for
  every public type; auto-trait and auto-derived impls are still tracked. The
  snapshots and their regeneration procedure are now documented in
  `CONTRIBUTING.md`, which also drops its stale instruction not to tag without
  "both semver surfaces" — a gate that no longer exists.
- Fixed the async owner terminating a session when a byte-stream read carried
  bytes without finishing a VISCA frame (#560). The actor treated the resulting
  empty decoded batch as end of stream and failed every in-flight request, so a
  reply split across two TCP or serial reads — routine on stream transports —
  closed the session. Only a zero-length transport read now signals a close; a
  short read that only advances a partial frame keeps the owner pumping, which
  matches the blocking owner.
- Closed the release version-gate bypasses in
  `.github/scripts/validate-release.sh` (#632). The hardware-evidence
  requirement keyed off a literal `2.x.y` match, so `v2.0.0+meta` — identical in
  semver precedence to `v2.0.0` — and every later major (`v3.0.0`, `v12.0.0`)
  published a stable release with an all-`Pending` hardware checklist. Tags
  carrying build metadata are now refused outright, and the evidence gate
  applies to every stable release with major version 2 or higher while
  pre-releases keep their candidate exemption. The checklist parser no longer
  loses the `Status` column to Markdown emphasis or letter case, rejects a
  checklist with no `Status` rows instead of passing it by omission, and treats
  `pending`/`TBD`/`TODO` sign-off records as placeholders. The tag shape check
  also rejects leading zeroes. `test-validate-release.sh` gains fixtures for
  both bypasses, later-major stable and pre-release controls, and the adjacent
  checklist and tag-shape holes.

## [1.2.0] - 2026-08-27

### Added

- `runtime::Priority` is public again in every build configuration, and all four
  documented levels (`Low`, `Normal`, `High`, `Critical`) now exist in production
  builds (#578). `High` and `Critical` were previously compile-gated behind
  `all(feature = "mode-async", feature = "test-utils")` or `cfg(test)`, so a
  released build exposed only `Low` and `Normal` while the type documented an
  emergency/safety lane. The scheduler orders queued work by `Ord` alone, so the
  upper two levels need no new scheduling code. `runtime::testing::Priority`
  keeps working and is now available in blocking `test-utils` builds too.
- Cameras can now submit at a chosen `runtime::Priority` (#589). `Camera` gained
  `set_command_priority` / `command_priority` (mirrored on `BlockingClient`),
  which set the priority every command from that handle is queued at, and
  `execute_with_priority`, which raises a single command without changing the
  handle default — the form an emergency stop needs on a camera shared behind an
  `Arc`. Previously every camera call site submitted at `Normal` and the blocking
  runner hard-coded `Normal`, so `High` and `Critical` were unreachable from the
  public API. The default is unchanged (`Normal`), inquiries keep the scheduler's
  own polling priority, and priority only decides which **queued** command is
  dispatched next: it never interrupts or reorders a command already sent.
- The async `Camera` now implements `Clone` (#597). The type-level docs, the
  connection-pooling example in the `runtime` module, and the per-handle caveats
  added by #584 and #589 all described cloning a camera, but no `Clone` impl
  existed in either mode. Cloning is cheap and shares the connection: the
  runtime task, its transport, the VISCA socket allocator, the command queue,
  and the write-only state cache are all reference counted, so both handles
  drive the same camera and commands from either are sequenced against each
  other. Teardown happens when the **last** handle drops — dropping a clone
  leaves the original working — while an explicit `shutdown()` still terminates
  the runtime for every handle. Per-handle state (camera ID, `timeout_config`
  view, `command_priority`) is copied at clone time and diverges afterwards,
  which is what makes the #589 emergency-stop pattern work on a camera shared
  behind an `Arc`: clone off a private handle, raise it to `Critical`, submit.
  The blocking `Camera` owns its transport and remains non-`Clone`.
- `CameraSession::into_inner()` on async sessions. `Connect::open_tcp_async()`
  and its siblings return a session, while `IntoDynCamera` is implemented for the
  owned `Camera`, so users of the convenience helpers could not reach
  `into_dyn()` at all — the only route was the verbose `Runtime::connect_tcp` +
  `CameraBuilder` path. Extraction is a handoff, not a close: the transport and
  the runtime task move with the returned camera, which then owns the teardown
  the session would have performed on drop (#588).

### Deprecated

- The blocking `CameraSession` surface is deprecated and will be removed in 2.0
  (#594). `CameraSession::new` is gated behind `mode-async` and is only reached
  through the async entry points, so no blocking caller could ever construct a
  `CameraSession<Blocking, ...>`; every blocking entry point returns
  `BlockingClient`, which remains the single blocking handle. The deprecation is
  applied per method on the blocking-only impl blocks — `camera`, `camera_mut`,
  `into_inner`, `close`, `raw`, the movement-detection methods (`await_idle`,
  `await_pan_tilt_idle`, `await_zoom_idle`, `await_focus_idle`, `is_moving`,
  `await_with_config`, `await_axes_idle`), the noun accessors (`power`, `zoom`,
  `pan_tilt`, `focus`, `exposure`, `white_balance`, `menu`, `presets`, `tally`,
  `system`, `image`), and the blocking `RawSender` methods (`send_bytes`,
  `execute`, `send_command`). The `CameraSession` type itself and the async
  session surface are unaffected and are not deprecated.

### Fixed

- The `Camera` documentation no longer claims a `Clone` that does not apply
  (#597). Both cfg blocks of the type-level docs stated "The `Camera` type is
  `Clone`" over a `camera.clone()` example, and the `runtime` module built its
  connection-pooling example on `Camera::clone()`. The async block now states
  the real contract — shared runtime and connection, per-handle timeout view and
  command priority, teardown on the last handle — and its example is a compiled
  `rust,no_run` doctest. The blocking block now says what is true: the blocking
  `Camera` owns its transport, is neither `Clone` nor `Sync`, and is shared by
  keeping the one `BlockingClient` behind an `Rc<RefCell<_>>` or `Arc<Mutex<_>>`
  (or by enabling `mode-async`). The connection-pool example is likewise a
  compiled doctest built on the real `Connect` + `into_inner` path.
- The `CameraBuilder` documentation no longer shows APIs that do not exist
  (#587). The `from_transport` example documented
  `Transport::tcp(...).build_async_with(runtime)`, but `Transport` and
  `NetTransportBuilder` are `cfg(not(feature = "mode-async"))` blocking-mode
  types and `build_async_with` exists nowhere in the crate. The module-level
  example used an undeclared `custom_transport` and called `shutdown()` on the
  `CameraSession` returned by `CameraConfig::open_async`, which only offers
  `close()`. All three `CameraBuilder` examples were wrapped in `ignore` fences,
  so `cargo test --doc` never compiled them. They are now `rust,no_run`
  doctests written against the real construction paths — `Runtime::connect_tcp`
  plus `TransportHandle` for async, and the blocking `Transport` builder for
  `from_transport_handle` — and are compiled on every run.
- The `mode-async,test-utils` feature combination (async mode with no runtime
  feature) now builds and tests cleanly. `issue_377_async_detection_test.rs`
  imported `transport::protocol_detection`, a module deleted when runtime
  protocol detection was replaced by compile-time envelope selection, and its
  `#![cfg(...)]` gate meant only this one uncovered combination ever built it.
  The dead test is retired; the `executor_selection` timeout-executor panic is
  allowed explicitly; `runtime_parity_test.rs` is gated on a real runtime so it
  no longer leaves unused items behind; and the never-executed
  `DeterministicExecutor` variants in `issue_339_timeout_behavior_test.rs` and
  `issue_371_cancel_camera_id_test.rs` are marked `#[ignore]` with an
  explanation, since they hang. A bare `mode-async,test-utils` cell was added to
  the CI feature matrix so runtime-free async test files cannot rot unnoticed
  again (#591).
- Deferred cancels in the async runtime no longer abort an unrelated command.
  The cancel outbox now records which command each queued cancel targets, and a
  cancel is dropped if the command has completed or the camera has reassigned
  its VISCA socket before the frame is sent (#574).
- Deferred cancels on a multi-camera transport are no longer dropped as stale
  (#602). VISCA sockets are camera-local, but the async cancel outbox
  re-validated each queued cancel with the camera-blind
  `find_command_on_socket`, which returns whichever camera has held that socket
  *number* longest. On a daisy chain an unrelated command on camera 1's socket 1
  shadowed camera 2's own socket 1, so camera 2's perfectly valid cancel was
  judged stale and silently discarded and its command ran on to completion or
  timeout. The re-validation is now camera-scoped. The staleness guard from #574
  is unchanged; it is simply evaluated against the right camera's state. The
  direction was always fail-safe — a mismatch could only drop a cancel, never
  emit one against another camera's command.
- Async cameras can control the on-screen menu again: `Camera::menu()` was
  defined only in the blocking `impl` block, so enabling `mode-async` removed
  the OSD menu accessor from the camera surface entirely (#575).
- Added `Camera::set_timeout_config()` to the async camera. Timeouts were fixed
  at construction time for async users; the new setter hands the configuration
  to the runtime loop, which re-evaluates deadlines against it on every
  housekeeping pass, so it also covers work already in flight.
- `DeterministicExecutor` can no longer silently trap timeout tests. Its
  `Executor::block_on` never advanced the virtual clock, so any future awaiting a
  timer parked forever; it now panics immediately and names the working
  alternatives. A new `DeterministicExecutor::run_until` drives a borrowing,
  non-`Send` future while running background tasks and advancing virtual time,
  and is the drop-in replacement used by the in-crate callers that previously
  went through `block_on`. The five never-executed `DeterministicExecutor`
  variants in `issue_339_timeout_behavior_test.rs` and
  `issue_371_cancel_camera_id_test.rs` are deleted rather than left `#[ignore]`d:
  per the #394 decision, timeout and cancel behaviour is tested on real runtimes,
  and the equivalent tokio tests in the same files already assert those
  guarantees. The one assertion without a tokio counterpart - a second command
  still succeeding after an intervening recv timeout - was ported to
  `runtime_resilient_to_timeout_errors_between_commands`. The orphan
  `src/testing/testkit/deterministic_executor_test.rs`, never declared as a
  module, is removed (#600).
- Replies from serial-chain addresses 2-7 are no longer rejected by the response
  decoder (#590). A VISCA device at address *n* answers with the high nibble
  `8 + n`, so camera 1 replies `0x90` and camera 7 replies `0xF0`; the decoder
  accepted only `0x9y`, and every ACK, completion, error, and data reply from
  the rest of a daisy chain was dropped as malformed, leaving those commands to
  time out. The full `0x90`-`0xFy` reply range is now decoded, the source
  address is carried through as a `CameraId`, and reply correlation uses it: on
  a transport carrying more than one camera a reply resolves against the sending
  camera's own sockets, ACK queue, and inquiry FIFO instead of matching by
  socket number across cameras. Single-camera behavior is unchanged - when at
  most one camera has work outstanding the address is not discriminating (IP
  cameras commonly answer `0x90` whatever address they were given) and the
  existing attribution runs as before. Lead bytes that are not reply addresses,
  including the `0x88` address-set traffic handled by the serial handshake, are
  still rejected as malformed.

## [1.1.0] - 2026-08-25

### Added

- Dynamic operation handles now expose `await_applied`, `await_settled`, and
  `detach` alongside ID-based cancellation. Public `OperationMetadata` and
  command-derived metadata keep applied-only versus targeted behavior aligned
  across blocking, async, and dynamic handles.
- Maintained blocking and Tokio operation-handle examples now demonstrate exact
  per-command deadlines, physical settling, and explicit detach/stop behavior.

#### Mode-Honest Operation Handles for Movement Commands (#539)

- Added a single `submit(command)` primitive on both async and blocking cameras
  that returns an operation handle which is genuine in either mode. This replaces
  the async-only `_op` handle surface with one unified path.
  - Async cameras return `InFlight`; blocking cameras return the new
    `BlockingInFlight`. `BlockingInFlight` drives completion **synchronously on
    the caller's thread** with no async executor involved. Both handle types
    expose the same lifecycle vocabulary, with signatures appropriate to their
    blocking or async mode.
  - `Camera::submit(command)` takes a prepared `ViscaCommand`, submits it as a
    command-derived targeted or applied-only operation, and returns its handle
    without waiting. Custom commands without metadata retain the conservative
    targeted/all-axes 1.x fallback.
    `Camera::submit_continuous(command)` does the same but marks a custom command
    as applied-only, with no well-defined settled state.
- Added handle completion methods with explicit *applied* vs *settled* semantics:
  - `await_applied(timeout)` resolves when the camera has accepted and
    protocol-completed the command. It is available on every handle — including
    applied-only operations — and is the method to use for a bounded deadman
    `STOP`.
  - `await_settled(timeout)` resolves when physical motion has ended. Both modes
    use exact operation completion when the profile supports it and bounded
    position polling otherwise. It is meaningful only for targeted moves;
    applied-only handles return `Error::NotSupported`.
  - `cancel()` requests scheduler-owned, ID/socket-safe cancellation. Queued
    commands can be removed before sending; sent commands use the protocol cancel
    path once their socket is known and the profile supports it. Success does not
    prove physical motion has stopped. `detach()` is the explicit fire-and-forget
    escape hatch.
- Added `OpKind` (`Targeted` / `Continuous`, where `Continuous` is the 1.x name
  for any applied-only operation) to describe whether an operation has a settled
  state, and re-exported it from `grafton_visca::camera`.
- Blocking movement detection (`is_moving_axes_with_deadline`) can now be called
  through a shared `&self` reference so it composes with the new handle waits.

### Fixed

- PTZOptics G2 sent-command cancellation now returns `Error::NotSupported`
  instead of transmitting a socket-cancel frame that tested G2 cameras reject.
  Queued commands remain locally cancellable; bounded continuous movement should
  use an explicit STOP command.
- Blocking deadline scheduling no longer passes a zero-duration receive timeout
  to platform sockets when command spacing has elapsed but inquiry spacing has
  not, avoiding a tight warning/error loop on real TCP transports.
- Operation-handle examples now snapshot and restore the complete pan/tilt/zoom
  pose, run cleanup after operation failures, and verify restoration before
  closing the session.
- Async runtime handles now retain unexpected terminal transport errors so work
  submitted after termination receives the original cause instead of a generic
  channel-closed error.

#### Complete Operation-Handle Semantics and Mode Parity (#539)

- Built-in command metadata is now the single source of truth for operation kind
  and affected axes across blocking, async, and dynamic handles.
- Async `await_settled` now matches blocking mode: it uses the exact command
  completion on profiles with operation-complete support and position polling
  otherwise, under one timeout budget spanning exact protocol completion and
  fallback settling.
- Blocking submission performs its initial synchronous dispatch before returning
  a handle, retains out-of-order results for multiple live handles, and uses the
  scheduler's protocol-aware cancellation path.
- The dynamic facade retains operation metadata and shares applied, settled,
  cancel, and detach semantics with the static handle surface.
- CI gates the declared Rust 1.88 MSRV and checks semver compatibility for the
  cfg-exclusive blocking and async/dynamic public surfaces separately.
- Release and API documentation now distinguishes lifecycle management from
  profile-aware command conversion, async queue acceptance from blocking initial
  dispatch, and cancellation requests from proof that physical motion stopped.

### Changed

- Clarified that TCP keepalive is an OS-level liveness mechanism rather than
  application-level VISCA traffic, and documented safe recovery from a closed
  connection.

#### Bounded Contextual Retry for Transient Inquiry Syntax Errors (#536)

- A VISCA `0x02` Syntax Error attributed to an **active built-in inquiry** is now
  treated as a transient camera-overload signal and retried by the scheduler with
  bounded backoff, reusing the existing per-category retry budget, backoff, and
  `RetryConfig::max_retry_duration`. Previously such inquiries failed immediately.
  This is a **narrow, contextual** retry — not general syntax-error retryability:
  - Command-side `0x02` remains terminal and fails immediately with
    `Error::SyntaxError`, with no retry queued.
  - Raw custom inquiries (`InquiryResponseSpec::Raw`) remain terminal by default,
    so malformed custom bytes are not hidden by silent retries.
  - `Error::SyntaxError.is_retryable()` is unchanged (still `false`); the decision
    lives in a single scheduler-owned classifier keyed on the raw error code plus
    live entry context (entry kind, response spec, lifecycle phase).
  - When the retry budget or max retry duration is exhausted, the operation fails
    with `SyntaxError` (wrapped in context), never a generic `Timeout`.
- Retry dispatch now re-enters the scheduler's normal send path instead of being
  sent directly by the runtime loops, so ready retries honor profile send pacing
  (`MIN_COMMAND_SPACING`, `MIN_INQUIRY_SPACING`), the max in-flight inquiry limit,
  and command socket capacity — for **all** retries, not just syntax-error retries.
- A retryable inquiry `0x02` also applies an inquiry-class cooldown, delaying the
  failed inquiry's resend and any other queued inquiry for the backoff window
  without pausing command traffic.

#### Operation-Handle Lifecycle (#539)

- Operation handles (`InFlight` and `BlockingInFlight`) are now `#[must_use]`, so
  directly ignoring a returned handle raises a lint. Rust does not enforce linear
  use after a handle is bound to a variable. **Dropping a handle never stops the
  command** — drop is equivalent to `detach`. Use `cancel()` for explicit
  cancellation.
- The deprecated concrete async `_op` handle methods and the new `submit`
  primitive now share a single internal submission implementation, so the
  handle-producing lifecycle paths cannot drift. The similarly named dyn trait
  methods remain the object-safe 1.x handle surface because `submit` is not
  object-safe.

### Deprecated

- The concrete async `_op` movement handle methods are deprecated ahead of the
  2.0 operation redesign:
  `pan_tilt_absolute_op`, `pan_tilt_relative_op`, `pan_tilt_home_op`,
  `pan_tilt_reset_op`, `set_zoom_op`, `set_zoom_normalized_op`,
  `set_zoom_normalized_in_domain_op`, `set_focus_op`, and `preset_recall_op`.
  They remain profile-aware, behavior-preserving shims throughout 1.x. When the
  equivalent command has already been constructed and profile-validated, prefer
  `submit(command)` plus `await_applied` / `await_settled`.
- `InFlight::await_completion` is deprecated in favor of `await_applied`, which
  makes the applied-versus-settled completion distinction explicit. The old name
  remains as a delegating shim and will be removed in 2.0.

##### Migration Notes

This release is source-compatible: existing code — including the `_op` methods
and `await_completion` — continues to compile and retains its documented
behavior, emitting only deprecation warnings. To migrate:

- Ordinary command-completion callers need no change: the ergonomic methods and
  noun accessors (`camera.pan_tilt().absolute(...)`, `camera.zoom().stop()`, …)
  are unchanged.
- Replace `handle.await_completion(timeout)` with `handle.await_applied(timeout)`.
- To obtain a handle for a command you can construct and validate directly —
  such as `PanTilt::Home` or `Zoom::Stop` — use
  `camera.submit(&command)` and drive it with `await_applied` / `await_settled`.
  Blocking callers gain a real handle here for the first time
  (`camera.submit(&command)?.await_applied(timeout)?`), which previously required
  a separate idle-polling wait.
- `submit` manages lifecycle but does not add the profile-aware conversion or
  validation performed by typed noun controls. For handles created from
  ergonomic inputs (degrees, normalized units, or preset IDs), the `_op` methods
  remain profile-aware compatibility shims throughout 1.x. Their final
  replacement is part of the coherent 2.0 operation API design.

Intended follow-up changes, targeted for the next major release (2.0) unless
noted:

- The deprecated concrete-camera `_op` methods and the static and dynamic
  `await_completion` compatibility aliases will then be removed. The dyn handle
  entry points will be redesigned with the rest of the operation API rather than
  removed without a replacement.
- The coarse operation markers will be split into targeted and applied-only
  variants so that calling `await_settled` on an applied-only handle becomes a
  compile error instead of a runtime `Error::NotSupported`.
- The public surface will move to typed consuming handles with compile-time
  targeted/applied-only completion constraints.

## [1.0.0] - 2026-06-13

### Changed
- Refocused the README, crate root documentation, and examples index around the
  stable camera-first 1.0 adoption path, with exhaustive support and protocol
  details routed to dedicated reference docs.
- Clarified that author hardware validation for the 1.0 built-in profiles is
  limited to PTZOptics brand cameras; other built-in profiles are source-backed
  and should be validated against target hardware and firmware.
- Expanded the release documentation checklist to include rustdoc and doctest
  verification across default and common optional feature sets.

### Fixed
- Classified syntax errors on inquiry responses as transient camera responses
  and logged them at warning level, while preserving error-level logging for
  command-side syntax errors.

## [0.13.0] - 2026-05-18

### Breaking Changes

#### Unambiguous Network Endpoint Parsing (#534)
- **BREAKING**: Bare IPv6 literals are no longer interpreted as host-plus-port endpoints. `2001:db8::1:5678` is treated as the IPv6 host `2001:db8::1:5678`; with a profile/default port it canonicalizes to `[2001:db8::1:5678]:<default>`.
- Explicit IPv6 ports now require brackets, for example `[2001:db8::1]:5678`. Missing ports without a profile/default port now return `Error::InvalidAddress` before DNS or socket work.
- Low-level TCP/UDP transport constructors and runtime adapter connectors now use the same endpoint grammar as camera-first connection paths. IPv6 zone identifiers are rejected explicitly.

#### Profile-Aware Direct Zoom Positioning (#529)
- **BREAKING**: Direct zoom setters now distinguish raw and normalized command paths. `set_zoom` accepts only a checked raw `ZoomPosition`; use `set_zoom_normalized(UnitInterval)` for optical normalized zoom and `set_zoom_normalized_in_domain(UnitInterval, ZoomDomain)` for documented optical-plus-digital ranges.
- **BREAKING**: Profile-independent `ZoomPosition` conversions from `f32`, `UnitInterval`, `Percentage`, and `Magnification` were removed. `Raw<u16>` conversion is now checked through `TryFrom<Raw<u16>>` and invalid raw values return an error instead of falling back to minimum zoom.
- **BREAKING**: `Capabilities::magnification_to_zoom_units` now returns `Result<u16, Error>` and rejects non-finite, below-1.0x, and out-of-range magnifications instead of silently clamping invalid values to wide zoom.
- Normalized zoom command conversion is now profile-aware, uses each profile's optical or digital maximum explicitly, and never falls back from `OpticalPlusDigital` to optical-only when no digital range is documented.
- Blocking, async, accessor, `_op`, and dyn-api zoom surfaces now share the same raw, optical-normalized, and domain-normalized semantics.

#### 1.0 Low-Level API Hardening (#528)
- **BREAKING**: Camera implementation submodules are no longer public extension points. Import the supported camera surface from `grafton_visca::camera` (`Connect`, `Camera`, `CameraBuilder`, `CameraConfig`, `TransportKind`, `TransportOptions`, `CommandId`, and operation/in-flight handle types) and import static control traits from the crate root, for example `grafton_visca::{PowerControl, ZoomControl}`.
- **BREAKING**: `Normalized` was removed in favor of the checked `UnitInterval` value type. Use `UnitInterval::new(value)?`, `UnitInterval::try_from(value)?`, `UnitInterval::ZERO`, or `UnitInterval::ONE`; invalid, NaN, and infinite values are rejected instead of being constructible through a public tuple field.
- **BREAKING**: `ViscaCommand::Response` was removed. Custom commands now implement only command encoding and kind metadata through `ViscaCommand`; typed custom inquiries implement `ResponseParser` for response typing and parsing.
- **BREAKING**: `CameraConfig::camera_id` and `CameraBuilder::camera_id` now accept `CameraId`. Use `try_camera_id(u8)` when converting a raw VISCA camera number from configuration or user input.
- **BREAKING**: Public extension enums that may grow after 1.0 are marked `#[non_exhaustive]`, including `Error`, command `Response`, transport policy enums, timeout categories, and dyn-api operation categories. Downstream exhaustive matches need a wildcard arm.
- API contract tests now cover the intentional public camera exports, hidden low-level camera modules, removal of `Normalized`, removal of `ViscaCommand::Response`, checked camera ID builders, and root-level control trait import paths.

#### Image Quality Profile Gates (#526)
- **BREAKING**: Removed the broad `ImageProcessingControl` typed bucket. Contrast and sharpness now use `ContrastControl` and `SharpnessControl`; unsupported profiles no longer compile for those setters or matching inquiries.
- **BREAKING**: Exposure brightness moved out of image-processing metadata. Use `BrightnessControl` / `BrightnessInquiryControl` and `Capabilities::exposure_brightness_range`; image luminance remains on `LuminanceControl` / `HasLuminanceControl`.
- **BREAKING**: `ImageProcessing::CONTRAST_RANGE` and `ImageProcessing::SHARPNESS_RANGE` are now `Option<Range<u8>>`; downstream profile impls should use `Some(range)` for source-backed controls and `None` when unsupported.
- **BREAKING**: `set_sharpness_mode`, typed image-freeze helpers, and ungated brightness/contrast/sharpness inquiry methods were removed from broad public traits. Raw `command::ImageFreeze` remains available for custom integrations.
- Runtime discovery now reports unsupported brightness, contrast, and sharpness ranges as `None` and computes `has_image_processing` from source-backed image-processing metadata instead of hardcoding `true`.
- Built-in profiles no longer use empty `0..0` range sentinels. `SonyEVIH100`, `SonyBRC300`, `NearusBRC300`, and `GenericVisca` do not expose typed contrast/sharpness APIs unless source-backed metadata is added.
- PTZOptics sharpness command encoding now accepts the profile-supported `0x00..=0x0F` range instead of rejecting values above `0x0B`.

#### Exclusive API Mode Selection (#525)
- **BREAKING**: Removed the misleading public `mode-blocking` feature. Blocking is now documented as the baseline build when `mode-async` is not enabled.
- Builds that request `mode-blocking` now fail with Cargo's unknown-feature error, including combinations such as `mode-async,mode-blocking` or `runtime-tokio,mode-blocking`.
- The supported feature matrix now validates blocking with `cargo test --no-default-features`; async remains selected through `mode-async`, `runtime-tokio`, or `runtime-smol`.
- API contract tests now cover the intentional exclusive public surfaces: blocking types and construction methods are unavailable in async builds, and async types and construction methods are unavailable in blocking builds.

#### Public API Freeze and Camera-First Convergence (#509, #510)
- **BREAKING**: Blocking camera construction now returns the ergonomic `BlockingClient<P, Tr>` surface consistently; `CameraConfig::open_blocking()`, `CameraConfig::open_serial_blocking()`, blocking `CameraBuilder::open()`, and `CameraBuilder::build_blocking()` no longer expose the raw mode-generic camera type
- **BREAKING**: `BlockingCamera<P, Tr>` now aliases `BlockingClient<P, Tr>`, making noun accessors such as `camera.power().on()?` and `camera.pan_tilt().home()?` the canonical blocking API
- **BREAKING**: Low-level camera implementation modules are no longer public extension points; camera construction/session types remain available through `grafton_visca::camera`, and static control traits remain available through root-level re-exports such as `grafton_visca::PowerControl` and `grafton_visca::ZoomControl`
- **BREAKING**: Runtime scheduler internals are no longer part of the production public API; `runtime::RuntimeHandle`, `runtime::Priority`, and scheduler submodules are hidden, with test-only hooks available under `runtime::testing` when `test-utils` is enabled
- **BREAKING**: Transport implementation modules such as `transport::builder`, `transport::buffer`, `transport::blocking_transport`, `transport::address`, and `transport::envelope` are hidden; supported transport configuration and extension types are re-exported from `grafton_visca::transport`
- **BREAKING**: The `command::encode` implementation module is hidden; raw command extensions should import `ViscaCommand`, `CommandKind`, and `InquiryKind` from `grafton_visca::command`
- **BREAKING**: Command category modules such as `command::zoom`, `command::preset`, `command::resolution`, `command::response`, `command::typed`, and `command::inquiry` are no longer public API; supported low-level command, response, parser, and inquiry types are re-exported from `grafton_visca::command`
- **BREAKING**: `grafton_visca::testing` is exported only with `test-utils` enabled; `runtime-tokio` alone no longer exposes the camera simulator or deterministic test helpers
- **BREAKING**: The unused `MotionGuard` helper and low-level `runtime_demo_lowlevel` example were removed; use explicit camera accessors such as `camera.zoom().stop()` and the high-level `runtime_demo` example instead
- **BREAKING**: The direct blocking `nd_filter()` inquiry method was removed to reserve `camera.nd_filter()` for the camera-first ND filter accessor; use `camera.nd_filter().position()?`

#### Optional Vendor Control Capability Gates (#518)
- **BREAKING**: Optional vendor feature traits were split into runtime metadata traits (`NdFilterMetadata`, `MotionSyncMetadata`, `VariableSpeedMetadata`) and explicit typed API support markers (`HasNdFilter`, `HasMotionSync`, `HasVariableSpeed`)
- **BREAKING**: Unsupported typed vendor controls no longer compile for profiles that only carry unsupported metadata defaults; for example, `PtzOpticsG2.nd_filter()`, `PtzOpticsG2.motion_sync()`, `GenericVisca.motion_sync()`, and `SonyFR7.motion_sync()` are no longer available
- **BREAKING**: ND filter inquiries were removed from broad all-profile inquiry paths and are now gated with the ND filter support marker through `NdFilterInquiryControl`
- `Profile` still requires optional-feature metadata so `Capabilities::from_profile::<P>()` can report `has_nd_filter`, `has_motion_sync`, `has_variable_speed`, `nd_filter_type`, and `max_motion_sync_speed` for every built-in profile
- Built-in typed support markers now follow the documented model capabilities: `SonyFR7` supports typed ND filter and variable speed controls; built-in PTZOptics profiles remain unmarked for typed Motion Sync because the current model capability specs do not establish that support; `GenericVisca` and other built-ins remain unmarked for these optional vendor controls
- Built-in profiles now report generic `SUPPORTS_ONE_PUSH_FOCUS = false` unless the reference docs establish that exact capability; FR7 still exposes its separate Push AF support through `HasPushAutoFocus`
- Added contributor guidance for source-backed camera profile capability changes, including metadata/support-marker decisions and required test coverage
- Raw/custom VISCA command extension APIs remain profile-agnostic escape hatches for advanced integrations

##### Migration Notes

The main semantic change is that optional vendor metadata no longer implies typed API support. Before this change, blanket implementations made support markers follow optional feature traits automatically. After this change, metadata traits (`NdFilterMetadata`, `MotionSyncMetadata`, `VariableSpeedMetadata`) feed runtime discovery, and explicit support markers (`HasNdFilter`, `HasMotionSync`, `HasVariableSpeed`, `HasPushAutoFocus`) are the source of truth for typed controls. The removed blanket implementations had this shape:

```rust
impl<T: ProfileMetadata + NdFilter> HasNdFilter for T {}
impl<T: ProfileMetadata + MotionSync> HasMotionSync for T {}
impl<T: ProfileMetadata + VariableSpeed> HasVariableSpeed for T {}
```

Profile authors must now opt into typed surfaces deliberately:

```rust
impl NdFilterMetadata for MyProfile {
    const ND_MODE: NdFilterMode = NdFilterMode::Variable;
}
impl HasNdFilter for MyProfile {}
```

Method and bound migrations:

```diff
-use grafton_visca::InquiryControl;
+use grafton_visca::{
+    capabilities::HasNdFilter,
+    NdFilterInquiryControl,
+};

-where P: Profile + Default, Camera<M, P, Tr, Exec>: InquiryControl<Mode = M>
+where P: Profile + HasNdFilter + Default, Camera<M, P, Tr, Exec>: NdFilterInquiryControl<Mode = M>
 let position = camera.nd_filter_position().await?;
```

```diff
-use grafton_visca::InquiryControl;
+use grafton_visca::{
+    capabilities::HasMotionSync,
+    MotionSyncControl,
+};

-where P: Profile + Default, Camera<M, P, Tr, Exec>: InquiryControl<Mode = M>
+where P: Profile + HasMotionSync + Default, Camera<M, P, Tr, Exec>: MotionSyncControl<Mode = M>
 let mode = camera.motion_sync_mode().await?;
```

```diff
-use grafton_visca::VariableSpeedControl;
+use grafton_visca::{
+    capabilities::HasVariableSpeed,
+    VariableSpeedControl,
+};

-where P: Profile + Default, Camera<M, P, Tr, Exec>: VariableSpeedControl<Mode = M>
+where P: Profile + HasVariableSpeed + Default, Camera<M, P, Tr, Exec>: VariableSpeedControl<Mode = M>
 camera.set_variable_speed_mode(VariableSpeedMode::Fine50).await?;
```

Generic wrappers that forwarded every camera operation through one `P: Profile + Default` bound need feature-specific impl blocks or bounds. For example:

```diff
 impl<P, Tr> CameraWrapper<P, Tr>
-where P: Profile + Default
+where P: Profile + HasNdFilter + Default
 {
     pub async fn nd_position(&self) -> Result<NdFilterPosition, Error> {
         self.camera.nd_filter().position().await
     }
 }
```

If the old wrapper is left as `P: Profile + Default`, errors will look like:

```text
error[E0277]: the trait bound `P: HasNdFilter` is not satisfied
help: consider further restricting type parameter `P` with trait `HasNdFilter`
```

Downstream `Arc<dyn ...>` facades are the architectural migration point. If a single trait object currently exposes ND filter, Motion Sync, and variable-speed methods for every profile, split the facade into a base trait plus optional capability objects:

```rust
use grafton_visca::mode::BoxFuture;

pub trait CameraOps: Send + Sync {
    fn capabilities(&self) -> &Capabilities;
    fn nd_filter(&self) -> Option<Arc<dyn NdFilterOps>>;
    fn motion_sync(&self) -> Option<Arc<dyn MotionSyncOps>>;
    fn variable_speed(&self) -> Option<Arc<dyn VariableSpeedOps>>;
}

pub trait NdFilterOps: Send + Sync {
    fn position(&self) -> BoxFuture<'_, Result<NdFilterPosition, Error>>;
}
```

The empty support markers are dyn-compatible but are compile-time profile markers, not operation facades. The static control traits are usable with dyn dispatch only when the associated `Mode` is fixed, for example `dyn NdFilterInquiryControl<Mode = Async>`. They are not a replacement for a runtime capability fan-out. For public object-safe facades, use the `dyn-api` feature where it covers the operation, or mirror its boxed-future style. Downstream traits that use `async fn` or RPITIT return types are not dyn-compatible; return `BoxFuture` instead.

Built-in typed support markers after this change:

| Profile | `HasNdFilter` | `HasMotionSync` | `HasVariableSpeed` | `HasPushAutoFocus` |
| ------- | ------------- | --------------- | ------------------ | ------------------ |
| `GenericVisca` | no | no | no | no |
| `PtzOpticsG2` | no | no | no | no |
| `PtzOpticsG3` | no | no | no | no |
| `PtzOptics30X` | no | no | no | no |
| `SonyFR7` | yes | no | yes | yes |
| `SonyBRCH900` | no | no | no | no |
| `SonyEVIH100` | no | no | no | no |
| `SonyBRC300` | no | no | no | no |
| `NearusBRC300` | no | no | no | no |

Downstream projects can catch this class of breakage with compile-fail fixtures that mirror the crate's `tests/api_contract/fail/*optional*` cases: assert that unsupported built-in profiles cannot call optional typed accessors, and assert that wrappers requiring typed ND or variable speed include `HasNdFilter` or `HasVariableSpeed` bounds. When auditing facade changes, `cargo +nightly rustc -- -Z print-type-sizes` can help confirm that boxed-future or trait-object changes did not accidentally grow hot-path wrapper types.

If this gating model ships in a pre-1.0 release before the final cutover, prefer a staged path with deprecated forwarding aliases or a temporary `--cfg unstable_gates`-style opt-in so downstreams can land generic-bound changes and dyn-facade splits separately.

#### Profile Sub-Capability Gates (#521)
- **BREAKING**: Broad static control traits were decomposed so profile-specific unsupported sub-capabilities no longer compile for built-in profiles that lack source-backed support.
- **BREAKING**: `ZoomControl` now contains baseline tele/wide/stop only. Direct absolute zoom moved to `DirectZoomControl`, VISCA digital zoom toggle moved to `DigitalZoomControl`, and optical-plus-digital normalized positioning moved to `DigitalZoomRangeControl`.
- **BREAKING**: Iris-priority mode and iris set/reset/up/down moved from `ExposureControl` to `IrisControl`; iris value inquiry moved from `InquiryControl` to `IrisInquiryControl`.
- **BREAKING**: One-push focus, PTZOptics snap focus, focus zone, AF sensitivity, and focus near-limit inquiry are now exposed through marker-gated traits instead of the broad focus/inquiry traits.
- **BREAKING**: Backlight, WDR/dynamic range, exposure-compensation inquiries, one-push white balance, ATW, AWB sensitivity, color temperature, RGB gain/tuning, flip/mirror, saturation, hue, luminance, gamma, noise reduction, and picture effects moved out of broad exposure/color/white-balance/image/inquiry traits into marker-gated subcontrol traits.
- `PtzOpticsG2`, `PtzOpticsG3`, and `PtzOptics30X` no longer expose typed VISCA digital zoom toggle, optical-plus-digital zoom positioning, one-push focus, or snap focus. Their raw command escape hatches remain available.
- `GenericVisca` no longer exposes typed backlight/WDR, color-temperature, RGB gain/tuning, image flip/mirror, noise-reduction, or picture-effect APIs; those command bytes remain reachable through raw VISCA escape hatches.
- `PtzOpticsG2`, `PtzOpticsG3`, `PtzOptics30X`, `SonyFR7`, and `SonyBRCH900` expose typed iris APIs through explicit support markers; `GenericVisca`, Sony BRC/EVI, and Nearus profiles retain iris support where their metadata has an iris range.
- `PtzOpticsG2`, `PtzOpticsG3`, and `PtzOptics30X` now expose typed picture-effect controls for the validated VISCA picture-effect command family.
- `PictureEffectCommand` now sends only source-backed named modes (`Off` and `BlackAndWhite`); use `PictureEffectMode::Unknown(value)` for model-specific raw picture-effect values.
- Sony-encapsulated profiles now report `supports_tcp() == false`; the consolidated reference validates Sony VISCA-over-IP over UDP `52381`.
- Color-temperature support now follows the checked-in VISCA reference: PTZOptics G2/G3/30X, Sony BRC-H900, and Sony EVI-H100 expose typed color-temperature APIs; Sony FR7 does not.
- Direct zoom positioning now validates raw positions against the selected profile range before encoding, so a profile without digital zoom support cannot send an out-of-profile digital-range direct zoom command through the typed API.
- PTZOptics G2/G3/30X profiles now expose the validated AF zone command and inquiry, while PTZOptics G3/30X no longer expose focus-near-limit inquiry support that is not validated for PTZOptics in the consolidated reference.
- PTZOptics raw VISCA preset validation now uses the documented `0-127` command-table range for G2/G3/30X; higher serial/IP preset capacity remains an out-of-band device capability until raw VISCA values above `0x7F` are target-tested.
- PTZOptics30X direct zoom validation now uses the standard `0x4000` optical endpoint instead of the Axis-only `0x7AC0` digital endpoint.
- `dyn-api` remains profile-erased and now rejects unsupported digital zoom, one-push focus, focus zone, and AF sensitivity requests before command construction.

#### Built-In Tally Capability Gate (#524)
- **BREAKING**: Built-in profiles now expose typed tally accessors only when the centralized profile registry grants `HasTally`; `GenericVisca` and PTZOptics profiles no longer compile with `camera.tally()` or direct typed tally inquiry helpers.
- **BREAKING**: Tally control traits now require `P: HasTally`; generic callers must add `grafton_visca::capabilities::HasTally` when they intentionally target tally-capable profiles.
- **BREAKING**: The former `InquiryControl::tally_light_status` path was removed; use `TallyControl::tally_status` or the camera-first `camera.tally().status()` accessor, both gated by `HasTally`.
- Sony FR7 and BRC-H900 retain typed tally controls and inquiries through explicit registry-backed `HasTally` support.
- Raw/custom VISCA command extension APIs remain available for integrations that need to send vendor-specific tally bytes outside the typed support matrix.
- Added a profile-first capability marker matrix to `docs/camera_profile_support.md` so downstreams can see which built-in profiles implement `HasTally` and other typed support markers.

##### Migration Notes

Before:

```rust
use grafton_visca::InquiryControl;

where
    P: Profile + Default,
    Camera<M, P, Tr, Exec>: InquiryControl<Mode = M>,
{
    let status = camera.tally_light_status().await?;
}
```

After:

```rust
use grafton_visca::capabilities::HasTally;
use grafton_visca::TallyControl;

where
    P: Profile + HasTally + Default,
    Camera<M, P, Tr, Exec>: TallyControl<Mode = M>,
{
    let status = camera.tally_status().await?;
    // or: let status = camera.tally().status().await?;
}
```

Capability-gated control traits cannot be required by a heterogeneous dyn-erased
aggregate unless every profile behind that aggregate implements the marker. Use
optional accessors, runtime feature detection through `Capabilities`, or split
the aggregate into a base trait plus capability-specific extension traits.

#### Granular Iris Capability Modelling
- **BREAKING**: `Exposure::IRIS_RANGE` changed from `Range<u16>` to `Option<Range<u16>>`; downstream `impl Exposure` blocks must wrap their range in `Some(...)` or use `None` for cameras that lack iris control
- **BREAKING**: `Capabilities::iris_range` changed from `RangeInclusive<u16>` to `Option<RangeInclusive<u16>>`
- **BREAKING**: `Capabilities` struct is now `#[non_exhaustive]`; use `Capabilities::from_profile::<P>()` instead of struct literals

#### Removal of `runtime-async-std` Compatibility Alias
- **BREAKING**: The `runtime-async-std` feature, `AsyncStdRuntime`, `AsyncStdExecutor`, `AsyncStdCamera`, and `runtime_adapters::async_std` module have been removed
- **Migration**: Replace `runtime-async-std` with `runtime-smol` in `Cargo.toml`; replace `AsyncStdRuntime::new()` with `SmolRuntime::new()`
- CI, pre-commit hooks, and documentation no longer reference async-std

#### First-Class `dyn-api` Contract (#514)
- **BREAKING**: `DynCameraControl` now exposes direct control-family accessors (`pan_tilt()`, `zoom()`, `focus()`, `presets()`, and `motion()`) plus `capabilities()` for runtime feature discovery; the old optional `as_*` accessor shape was removed
- **BREAKING**: `InFlightDyn::await_completion()` is a one-shot wait on the command's own VISCA response future, matching the static `InFlight` contract instead of approximating completion through category-level idle polling
- Dynamic movement timeout parameters now apply to command completion responses consistently; physical idle waits are available explicitly through `DynMotionControl::await_idle()` and the axis-specific idle wait methods
- Dynamic in-flight handles now preserve command errors, cancellation results, and timeout behavior from the static response future

#### Executor Timeout Surface Slimming
- **BREAKING**: `Executor::timeout_owned()` was removed; executor implementors now provide only the borrowed `Executor::timeout()` timeout primitive, eliminating the duplicate owned/object-safe timeout path

#### Runtime Transport Bounds
- **BREAKING**: `Runtime::{TcpTransport,UdpTransport}` and `RuntimeSerial::SerialTransport` now require `Sync` in addition to `Send`, matching the camera control surface returned by `Connect` and `CameraConfig`

### Added

#### Blocking Camera-First Accessors (#510)
- Added blocking noun accessors for power, zoom, pan/tilt, focus, exposure, white balance, image processing, presets, tally, system, menu, ND filter, motion sync, and advanced inquiries
- Blocking accessors return `Result<T, Error>` directly, eliminating `.block()` from the canonical blocking flow while preserving async accessor parity
- Added missing white-balance accessor operations for one-push mode, ATW mode, and color-temperature mode
- Added compile-time API contract coverage for the frozen camera-first construction path, blocking accessor surface, root-level control trait re-exports, command-root extension API, hidden implementation modules, and `test-utils` gating

#### 1.0 API Contract Tests (#512)
- Expanded the API stability suite from compile-presence checks into explicit 1.0 contract assertions for public value wrappers, profile capabilities, serialization/schema/type-generation features, and runtime/transport-gated public entry points
- Added feature-specific compile contracts for async camera-first sessions, blocking serial, Tokio serial, `dyn-api`, and `test-utils` so supported feature surfaces fail loudly when their public API drifts
- Added compile-fail contracts proving unsupported optional vendor controls and support-marker bounds stay unavailable for unsupported profiles

#### Per-Profile Iris and Exposure-Mode Support
- New `Exposure::EXPOSURE_MODES` associated constant lets each profile declare which exposure modes the hardware accepts (defaults to all five standard modes)
- New `ExposureExt::supports_exposure_mode()` and `ExposureExt::supports_iris_control()` helper methods
- New `Capabilities::has_iris_control` field and `Capabilities::supports_exposure_mode()` convenience method for runtime feature gating
- Runtime guards in `set_exposure_mode()`, `set_iris()`, `reset_iris()`, and `iris_inquiry()` now return `Error::FeatureNotSupported` for unsupported profiles instead of sending a command the camera will reject

### Changed

#### Centralized Built-In Profile Registry (#524)
- Built-in profile definitions now come from one crate-private typed registry that generates profile structs, capability metadata, typed support marker impls, `ProfileId`, `ProfileGroup`, and registry invariant tests.
- `Capabilities::from_profile::<P>()`, profile IDs/groups, typed support markers, and README support matrices are mechanically checked against the same registry for every built-in profile.
- The profile support documentation no longer claims PTZOptics digital zoom support or PTZOptics Motion Sync support that is not established by the checked-in model references.
- Built-in profile capability changes now require registry evidence for intentionally unsupported optional typed surfaces, reducing the risk of stale hand-written support tables.

#### Built-In Inquiry Metadata Registry Cleanup (#522)
- Built-in inquiry commands now use one crate-local metadata table as the source of truth for response discriminants, decoded response data, dispatch, zero-sized query command structs, canonical bytes, typed response conversions, and accessor metadata.
- The built-in inquiry generator now classifies queryable, decode-only, aliased, and alternate-interpretation inquiries explicitly so intentional command-sharing and non-queryable responses are documented in code.
- Generated inquiry accessors now carry their profile capability gates in metadata, keeping broad inquiry cleanup and optional subcontrol gates aligned with API contract tests.
- The public `ViscaInquiry` derive macro remains available for downstream extension commands, while built-in inquiries use the internal registry instead of duplicating metadata through the public derive path.

#### Protocol Reference Consolidation
- Consolidated the PTZOptics, Axis, Sony protocol-resolution, and previous unified VISCA notes into `docs/visca_reference.md`.
- README, contributor, example, and profile-support documentation now point contributors to the consolidated VISCA reference as the source of protocol evidence for built-in profile changes.

#### Scheduler Command/Inquiry State Split (#508)
- Runtime scheduler state is now split into distinct command and inquiry entries instead of a shared mixed state bag
- Commands own ACK, socket, retry, and cancellation state; inquiries own reply correlation, response typing, and inquiry retry state
- Inquiry response type is required when an inquiry enters active scheduler state, and inquiry handling no longer depends on debug-only kind assertions
- Tests now cover legal command/inquiry invariants only, including that inquiry entries cannot carry cancellation state
- Removed the stale scheduler send-failure rollback helper and compile feature-specific scheduler internals only where they are used

#### Documentation and Examples Aligned for v1.0 (#509, #510)
- README, crate docs, prelude docs, and examples now present `Connect` plus camera-first accessors as the primary API for both blocking and async users
- Installation snippets now target the v1 line and remove stale pre-release/0.x caveats
- Blocking and async examples were updated away from direct trait-method calls toward the long-term `camera.power()`, `camera.zoom()`, `camera.pan_tilt()`, and related accessor style
- The examples guide no longer presents direct runtime-handle usage as application-facing API

#### 1.0 Support Matrix (#513, #514)
- README and contributor docs now declare the supported runtime, transport, profile, and optional-feature matrix for the 1.0 contract
- Compatibility-only feature unions are documented separately from supported contract rows, so CI coverage for dependency-graph combinations does not imply additional public API promises
- CI feature coverage now includes explicit blocking detection, blocking serial, Tokio serial, serde/schemars/ts-rs, dyn-api, and Tokio/smol runtime coexistence entries
- `dyn-api` is treated as a first-class 1.0 feature with public API contract coverage plus Tokio and smol runtime integration tests
- README, crate docs, and examples now document the profile-gated vendor control matrix separately from runtime capability metadata

#### PTZOptics Profiles Follow Reference-Backed Iris Support
- PTZOptics G2, G3, and 30X profiles now set `IRIS_RANGE: Some(0x00..0x0D)` and include `ExposureMode::Iris`, matching the consolidated reference's iris-priority, direct iris, and iris inquiry rows
- Sony and generic VISCA profiles retain their existing iris support unchanged

#### Dependency Refresh
- Updated dependency requirements for `bytes`, `smallvec`, `serde_with`, `serialport`, `tokio`, and `async-executor`.
- Removed unused `chrono` and `hex` dependencies from the crate manifest.

### Fixed

#### Runtime Boundary Shutdown Semantics (#523)
- Async runtime handles now fail new command, inquiry, completion-subscription, and metrics requests immediately with `Error::RuntimeShutdown` after shutdown begins.
- Explicit runtime shutdown now fails accepted and queued work, replies to pending control requests, drops completion subscribers, and exits the runtime loop without relying on data-plane channel capacity.
- Cancellation and shutdown requests now use an urgent control path selected ahead of normal control traffic, preserving liveness even when the bounded submission queue is full.
- Regression coverage now exercises runtime shutdown, termination cleanup, cancellation ID validity after bounded submission, and runtime-boundary behavior across the async control surface.

## [0.12.0] - 2026-03-22

### Breaking Changes

#### `CameraConfig` Now Owns Full `TransportConfig`
- **BREAKING**: `CameraConfig` now stores and applies a full `TransportConfig` across TCP, UDP, blocking serial, and Tokio serial transports
- `retry_config()` replaces `retries()`
- Transport-specific defaults are now derived from the active transport instead of being split across separate configuration paths

### Deprecated

#### `runtime-async-std` Compatibility Alias
- `runtime-async-std` is now a deprecated compatibility alias to `runtime-smol` and will be removed in `0.13.0`
- `AsyncStdRuntime`, `AsyncStdExecutor`, and `runtime_adapters::async_std` remain available in this release to ease migration
- Builds that enable `runtime-async-std` now emit a cargo warning directing new code to `runtime-smol`

### Added

#### Full Cross-Runtime TCP Keepalive Support
- TCP keepalive is now fully applied for Tokio, smol, the deprecated `runtime-async-std` compatibility alias, and blocking TCP transports
- New `TcpKeepaliveConfig` policy type with builder support via `tcp_keepalive(TcpKeepaliveConfig)` and `disable_tcp_keepalive()`
- Default TCP transport configuration now enables keepalive consistently across all supported runtimes

### Changed

#### Transport Socket Option Unification
- Shared TCP and UDP socket option handling now lives in a common transport module instead of being duplicated per runtime
- Runtime-specific TCP connectors now flow through the same socket-option application path, removing unsupported keepalive fallbacks
- Serial transport defaults now use consistent buffer sizing across blocking and Tokio implementations

#### Structured TCP Keepalive Policy
- TCP keepalive configuration now uses `Option<TcpKeepaliveConfig>` instead of a raw duration, allowing the idle period and probe interval to be tuned separately
- `TcpKeepaliveConfig::default()` now provides the long-lived VISCA TCP policy used by default transport configurations

#### Maintained Socket Options Backend
- Updated the internal TCP socket option plumbing to the maintained `socket2` 0.6 API line

### Fixed

#### Documentation Alignment
- Refreshed README, examples guide, contributor guide, and crate/module docs to match the current feature names and explicit runtime-selection model
- Updated transport-configuration guidance to use `CameraConfig`, `TransportConfig`, and the current `CameraBuilder::with_executor(...).from_transport(...)` flow

#### PTZOptics Sharpness Range Validation
- PTZOptics G2, G3, and 30X sharpness validation now consistently accepts `0x00..=0x0F` in the model-aware constructors and validators
- Added regression coverage for accepted `0x0F` and rejected `0x10` sharpness levels

#### PTZOptics Digital Zoom and Runtime Error Visibility
- PTZOptics G2, G3, and 30X profiles no longer advertise VISCA digital zoom support because the cameras reject the command and may close the TCP connection afterward
- Capability discovery and regression tests now reflect PTZOptics digital zoom as unsupported
- Background runtime loop exits and connection-closed transport failures are now surfaced through error-level logging instead of being silently discarded

## [0.11.0] - 2026-02-28

### Breaking Changes

#### `InquirySupport` Enum Replaces `SUPPORTS_INQUIRY: bool` (#498)
- **BREAKING**: `ProfileMetadata::SUPPORTS_INQUIRY: bool` replaced with `ProfileMetadata::INQUIRY_SUPPORT: InquirySupport`
- New `InquirySupport` enum with three variants: `Full`, `Partial`, `None`
- Derives `Debug, Clone, Copy, PartialEq, Eq, Hash` plus `serde`, `schemars`, `ts-rs` when feature-gated
- Default is `InquirySupport::Full` (most VISCA cameras support all inquiries)
- Profile assignments:
  - `Full`: PtzOpticsG2, PtzOpticsG3, PtzOptics30X, SonyFR7, SonyBRCH900 (hardware-confirmed)
  - `Partial`: GenericVisca, SonyEVIH100, SonyBRC300, NearusBRC300
- Added `ProfileId::inquiry_support()` and `ProfileGroup::inquiry_support()` runtime query methods
- `Capabilities` struct: `supports_inquiry: bool` replaced with `inquiry_support: InquirySupport`
- Added `Capabilities::has_full_inquiry_support()` convenience method

#### Extended `Profile` Supertrait (#498)
- **BREAKING**: `Profile` supertrait now requires `MotionSync + NdFilter + VariableSpeed`
- All three traits have sensible defaults, so existing `impl Profile for T {}` blocks continue to work
- `NdFilter` trait now has default associated constants (`ND_MODE = NdFilterMode::None`, `ND_STEPS = None`)
- `Capabilities::from_profile` now reads trait data instead of hardcoded false values:
  - `has_nd_filter` — from `P::ND_MODE`
  - `nd_filter_type` — from `P::ND_MODE` variant
  - `has_motion_sync` / `max_motion_sync_speed` — from `P::SUPPORTS_MOTION_SYNC` / `P::MAX_MOTION_SYNC_SPEED`
  - `has_variable_speed` — from `P::SUPPORTS_VARIABLE_SPEED`
  - `has_direct_menu_control` — from `P::SUPPORTS_DIRECT_CONTROL`
  - `supports_wake_on_lan` — from `P::SUPPORTS_WAKE_ON_LAN`

#### Zoom Position Type Range Expanded (#494)
- `ZoomPosition` max changed from `0x7000` to `0x7FFF` to support all profile-declared zoom ranges (e.g., `PtzOptics30X` at `0x7AC0`, digital zoom at `0x7FFF`)
- Removed global `ZoomPosition::MAX_OPTICAL` and `ZoomPosition::MAX_DIGITAL` constants — use profile capability constants (`P::OPTICAL_ZOOM_MAX`, `P::DIGITAL_ZOOM_MAX`) instead
- Zoom normalization (`from_normalized`, `to_normalized`) now requires profile max parameters instead of using global constants
- Removed `push_visca_u14` builder method (only caller was zoom, which now uses `push_visca_u16`)

#### `InquiryData` Contrast/Luminance Variants (#500)
- **BREAKING**: `InquiryData::Contrast(u8)` and `InquiryData::Luminance(u8)` tuple variants changed to struct variants with named `level: u8` field
- Pattern matching must use `InquiryData::Contrast { level }` / `InquiryData::Luminance { level }` instead of `InquiryData::Contrast(val)` / `InquiryData::Luminance(val)`

#### Inquiry Byte Corrections
- `RED_GAIN` inquiry corrected from `0x0A, 0x12` to `0x04, 0x43` — responses may differ from previous (incorrect) queries
- `BLUE_GAIN` inquiry corrected from `0x0A, 0x13` to `0x04, 0x44`
- `AUTO_WB_SENSITIVITY` inquiry corrected from `0x04, 0x59` to `0x04, 0xA9`
- `RED_TUNING` / `BLUE_TUNING` inquiry constants are now aliases for `RED_GAIN` / `BLUE_GAIN` (same register)

#### Color Inquiry Decoder Format Changes
- **BREAKING**: `ColorTemperature` inquiry response decoder changed from 4-nibble to 1-byte raw parsing to match PTZOptics G2 wire format
- **BREAKING**: `RedChannel`/`BlueChannel` inquiry decoders changed from 1-byte signed offset (`i8`) to 4-nibble absolute gain (`u8`) — parsed values will differ
- `RedTuning`/`BlueTuning` inquiry decoders changed from 1-byte raw to 4-nibble with offset
- `RedGainInquiry` and `BlueGainInquiry` converted from derive macro to manual `ResponseParser` impls

#### Removed `has_exposure_mode_inquiry` from `Capabilities`
- **BREAKING**: `has_exposure_mode_inquiry` field removed from `Capabilities` struct — the flag was universally true and never checked before sending the inquiry
- `SUPPORTS_EXPOSURE_MODE_INQUIRY` constant removed from the `Exposure` trait
- `HasAutoExposure` marker trait removed (defined for 5 profiles but never used as a trait bound)

#### Dead Marker Traits Removed
- **BREAKING**: 11 marker traits removed that were defined and implemented but never used as trait bounds (~40 manual `impl` blocks)
- `SUPPORTS_AUTO_EXPOSURE` flag removed from `Exposure` trait (was universally `true`, never checked)

#### Dependency Major Version Bumps
- `ts-rs` upgraded from 11 to 12 — downstream code using the `ts-rs` feature may need updates for the new `Config` parameter in test APIs
- `flume` upgraded from 0.11 to 0.12
- `schemars` upgraded from 1.1 to 1.2

### Added

#### Hardware Integration Test (#498)
- New `tests/hardware_inquiry_test.rs` for validating VISCA inquiries against a real PTZOptics G2 camera
- Tests are `#[ignore]` by default — run with `VISCA_CAMERA_IP=192.168.0.110 cargo test --test hardware_inquiry_test -- --ignored --nocapture --test-threads=1`
- Comprehensive `test_all_inquiries_succeed` test categorizes results as OK, KNOWN_MISMATCH (parser bug), or FAIL (unexpected)
- Hardware validation confirmed 24/28 inquiries parse correctly; 4 have known response format mismatches (red_gain, blue_gain, color_temperature, version)
- These parser mismatches are library-side bugs — the camera responds to all commands

#### New PTZOptics Commands
- Anti-flicker mode control (`AntiFlickerMode`: Off, 50Hz, 60Hz) with `set_anti_flicker_mode()` on `ExposureControl`
- Preset recall speed control (`PresetRecallSpeed`: 1-24) with `set_preset_recall_speed()` on `PresetsControl`
- USB audio on/off control with `set_usb_audio()` on `StreamingControl`
- Focus toggle (AF/MF switch) with `focus_toggle()` on `FocusControl`
- Focus snap (one-push AF in manual mode) with `focus_snap()` on `FocusControl`

#### New Inquiry Commands
- Contrast inquiry (`81 09 04 A2 FF`) with `contrast()` on `InquiryControl` — returns `ContrastLevel` (#500)
- Luminance inquiry (`81 09 04 A1 FF`) with `luminance()` on `InquiryControl` — returns `LuminanceLevel` (#500)
- `ContrastInquiry` and `LuminanceInquiry` derive-macro structs with `last_nibble` parser (#500)
- `ImageAccessor::contrast()` and `ImageAccessor::luminance()` accessor methods (via `camera.image().contrast()`) (#500)
- Blocking API `contrast()` and `luminance()` methods on `BlockingClient` (#500)
- Camera simulator support for contrast (opcode `0xA2`) and luminance (opcode `0xA1`) inquiry responses (#500)
- Integration tests for contrast and luminance inquiries (#500)
- Flicker mode inquiry (`81 09 04 55 FF`) with `flicker_mode()` on `InquiryControl`
- `FlickerModeInquiry` struct with `ResponseParser` support

#### Gamma Curve Control (#499)
- `GammaCommand` — sets gamma curve via VISCA command `81 01 04 5B 0p FF` (p=0 Standard, 1-4 different gamma curves)
- `set_gamma(level: GammaLevel)` on `GammaControl` trait with full doc comments cross-referencing `GammaInquiryControl::gamma`
- Blocking API `set_gamma()` method on `BlockingClient`
- `SUPPORTS_GAMMA: bool` and `GAMMA_RANGE: Option<Range<u8>>` on `ImageProcessing` capability trait (defaults to unsupported)
- `validate_gamma()` on `ImageProcessingExt` for profile-aware validation
- `image::GAMMA_PREFIX` byte constant
- Gamma support enabled for PtzOpticsG2, PtzOpticsG3, PtzOptics30X, SonyFR7, SonyBRCH900, SonyEVIH100 (range 0-4)
- Completes the get/set pair: `GammaInquiryControl::gamma()` (inquiry) + `GammaControl::set_gamma()` (control)
- **Hardware-validated**: Gamma inquiry and set commands confirmed working on PTZOptics G2 (undocumented in official PTZOptics VISCA reference)

#### Hardware Control Tests
- New `tests/hardware_control_test.rs` for validating VISCA SET commands against a real PTZOptics G2 camera
- Round-trip tests for gamma, contrast, and luminance: read original value, set to a different value, verify readback, restore original
- Each test safely restores original camera settings after validation
- Tests are `#[ignore]` by default — run with `VISCA_CAMERA_IP=192.168.0.110 cargo test --test hardware_control_test -- --ignored --nocapture --test-threads=1`

#### Hardware Inquiry Test Additions
- Added gamma, contrast, and luminance to `hardware_inquiry_test.rs` individual and comprehensive tests
- All three inquiries pass on PTZOptics G2 (gamma=2, contrast=9, luminance=6 at time of testing)

#### Documentation: Image Processing Commands
- Added Section 8.4 "Image Processing Parameters" to unified VISCA reference covering Brightness, Luminance, Contrast, Gamma, and Sharpness
- Expanded Section 7 capability matrix with "Image Processing" row listing supported features per camera model
- Updated Section 9.1 (PTZOptics) and Section 9.4 (EVI-H100) with image processing capability details
- Added gamma command/inquiry to PTZOptics G2 command list with errata note #5 documenting undocumented-but-functional status
- Added Image Processing entries to Appendix A opcode table (Sharpness, Brightness, Luminance, Contrast, Gamma, NR)

#### Transport-Level Command Pacing (#496)
- New `MIN_COMMAND_SPACING` constant on `ProfileMetadata` trait, enforced at the scheduler layer for all sends (commands and inquiries)
- Prevents spurious `0x02 Syntax Error` responses and dropped completions on cameras with small internal command buffers
- PTZOptics G2/G3/30X: 100ms spacing; Sony FR7/BRC-H900: 35ms spacing; GenericVisca and others: 0ms (no artificial spacing)

#### Zoom Magnification Helpers (#495)
- `zoom_magnification_to_units` field on `Capabilities` struct — exposes profile's magnification-to-VISCA-units lookup table at runtime
- `max_optical_zoom()` — returns maximum optical magnification (e.g., 20.0 for a 20x camera)
- `max_combined_zoom()` — returns maximum magnification including digital zoom if available
- `zoom_units_to_magnification()` — convert VISCA position units to magnification ratio
- `magnification_to_zoom_units()` — convert magnification ratio to VISCA position units
- Enhanced `Capabilities::summary()` to display human-readable zoom values (e.g., "Optical Zoom: 20x (0x4000 units)")

#### Fine-Grained Capability Flags
- `has_focus_zone`, `has_af_sensitivity`, `has_focus_near_limit_inquiry`, `has_rgb_gain` fields on `Capabilities` struct
- `SUPPORTS_FOCUS_NEAR_LIMIT_INQUIRY` on `Focus` trait (default `true`; overridden to `false` for PTZOptics G2)
- `has_gamma` and `has_luminance` fields on `Capabilities` struct — wired from `SUPPORTS_GAMMA` and `SUPPORTS_LUMINANCE` profile traits

#### Wire-Level Debug Logging
- Trace-level logging at target `grafton_visca::wire` showing every TX and RX byte in hex format
- Enable with `RUST_LOG=grafton_visca::wire=trace`; zero overhead when disabled

#### Inquiry Registry Unification (#503, #504)
- New `define_inquiries!` declarative macro in `inquiry_registry.rs` replaces 4 independently-maintained copies of inquiry metadata
- Single source of truth generates `InquiryKind` (76 variants), `InquiryData` (76 variants), and `dispatch()` (76 match arms) from one definition table
- `generate_typed_impl()` proc-macro refactored from 72-arm string matching to attribute-driven (`typed_response`, `typed_field`, `typed_constructor`)
- All 10 manual `ViscaCommand+ResponseParser` impls converted to `#[derive(ViscaInquiry)]`
- Adding a new inquiry type now requires editing exactly one registry entry + one struct; unknown cases produce compile errors instead of silent no-ops
- Net reduction: ~2,100 lines removed

### Changed

#### Dependency Updates
- Updated 79 semver-compatible lockfile dependencies
- Removed unused `rand` dev-dependency

### Fixed

#### PTZOptics Luminance Profile Support
- PtzOpticsG2, PtzOpticsG3, PtzOptics30X profiles now correctly declare `SUPPORTS_LUMINANCE = true` and `LUMINANCE_RANGE = Some(0..15)` (previously defaulted to unsupported despite luminance being documented and hardware-validated)

#### Contrast/Luminance Response Decoder Format (#500)
- Contrast and luminance response decoders corrected from 1-byte direct parsing to proper 4-nibble `Nibbles::<4>` format with `last_nibble()` extraction
- Hardware-validated against a real PTZOptics G2 camera: contrast inquiry returns `90 50 00 00 00 09 FF`, luminance returns `90 50 00 00 00 06 FF`
- Removed "write-only" notes from `set_contrast` and `set_luminance` doc comments; added cross-references to the new inquiry methods
- Camera simulator contrast/luminance handlers were previously commented out — now fully implemented with `encode_position()` encoding
- Documentation errata note #3 in PTZOptics G2 command list resolved: confirmed `0p 0q` (2-nibble) parameter form and updated curated command list to match

#### Stale Inquiry Comments (#498)
- Fixed comments in `examples/inquiry_quickstart.rs` that incorrectly claimed PTZOptics cameras support only a "subset" of VISCA inquiries — they support the full set

#### Hardcoded `false` Values in `Capabilities::from_profile` (#498)
- `has_nd_filter`, `has_motion_sync`, `has_variable_speed`, `has_direct_menu_control`, and `supports_wake_on_lan` were all hardcoded to `false` — now correctly read from profile trait constants

#### Zoom Position Truncation (#494)
- **Critical:** `Zoom::Position` encoding no longer silently truncates values above `0x3FFF` — previously, any zoom value with bit 14 set was masked to zero (e.g., `0x4000` encoded as `0x0000`)
- Profile-aware zoom normalization now correctly maps to each camera's actual optical/digital zoom range instead of hardcoded global constants

#### PTZOptics G2/G3/30X Capability Range Corrections (hardware-validated)
- Gain range corrected from `0..9` to `0..8` (hardware max is 7; values above are silently clamped)
- Brightness (Bright Direct) range corrected to `0..18` (hardware max is 17)
- Sharpness range corrected to `0..16` (hardware accepts 0-15; PTZOptics docs say 0-11 but camera accepts full range)
- RG/BG tuning range corrected from `-7..8` to `-10..11`
- Added missing ColorTemperature WB mode and color temp range (2500–8000K)
- Added missing exposure compensation, RGB gain, and hue support declarations
- G2 `MAX_PRESETS` corrected from 89 to 127

#### Flip Inquiry Opcode Correction
- `ImageFlipInquiry` corrected from opcode `0x66` (`CAM_PictureFlipInq`, vertical-only boolean) to `0xA4` (`CAM_FlipInq`, combined bitfield)
- `FlipStateInquiry` corrected from undocumented opcode `0x65` to `0xA4`
- Previously, `0x66` returned `0x03` (flip OFF) which the flags parser misinterpreted as `{horizontal: true, vertical: true}`
- `0xA4` returns a `0x00`–`0x03` bitfield matching the `CAM_Flip` write command, ensuring consistent read/write behavior

#### Scheduler ACK-Timeout Retry (#497)
- Commands that timed out waiting for ACK now transition to `Queued` phase before retry, preventing late ACKs from matching timed-out commands and eliminating duplicate command sends
- Added phase guard in `get_ready_retries` as defense-in-depth to reject retries for commands not in `Queued` phase

#### `Error::kind()` Exhaustive Match (#501, #502)
- Replaced wildcard `_ => ErrorKind::Other` in `Error::kind()` with an exhaustive match covering all ~55 `Error` variants
- `TransportBusy` now maps to `Busy` (retryable with 50ms delay) instead of `Other` (non-retryable)
- `NoSocket` now maps to `BufferFull` (retryable with 200ms delay) instead of `Other`
- `CommandPending` now maps to `Busy`; `MaxRetriesExceeded` maps to `Timeout` with `is_retryable() = false`
- Adding a new `Error` variant without a `kind()` arm now causes a compile error

#### `execute()` Error Masking
- `ViscaClient::execute()` had wildcard match arms that silently returned `Ok(())` for non-Completion responses (`CmdAck`, `Unknown`)
- Now uses `Response::into_result()` which correctly maps `CmdAck` to `Err(CommandPending)` and `Unknown` to `Err(InvalidResponse)`
- Cache updates in `execute_updating_cache` now only occur after confirmed success

#### One-Push Focus Removed from PtzOpticsG2
- `SUPPORTS_ONE_PUSH_FOCUS` set to `false` for PtzOpticsG2 — the camera ACKs the one-push focus command (`81 01 04 18 01 FF`) but never sends a Completion response, causing a 30-second timeout

#### PtzOpticsG3/30X `SUPPORTS_OPERATION_COMPLETE` Fix
- `SUPPORTS_OPERATION_COMPLETE` now correctly set to `true` for PtzOpticsG3 and PtzOptics30X (same firmware family as G2), enabling event-driven movement detection

## [0.10.0] - 2025-12-22

### Breaking Changes

#### Movement-Wait API Consolidation (#468)
- `await_pan_tilt_idle`, `await_zoom_idle`, `await_focus_idle` consolidated into unified `AwaitConfig` pattern

#### Type-Safe CommandId (#467)
- Command IDs now use opaque `CommandId` newtype instead of raw `u8`

#### PanTiltLimitCorner Narrowed (#463)
- `PanTiltLimitCorner` now only represents the two valid corners (was previously over-general)

### Added

#### StateCache for Write-Only Properties (#451)
- New `StateCache` type for tracking write-only VISCA properties that cannot be queried
- Enables application-level state tracking without camera round-trips

#### TypeScript Export Support (#444)
- New `ts-rs` feature for generating TypeScript type definitions
- Complete exports for all command enums

#### Dynamic API Feature (#443)
- New `dyn-api` feature with object-safe camera traits
- Motion control trait with timeout/cancellation behavior documentation

#### Inquiry Rate-Limiting (#449)
- Optional inquiry rate-limiting with profile-aware spacing defaults
- Prevents overwhelming cameras with rapid inquiry sequences

#### Validated Inquiry Methods (#446)
- Added validated inquiry methods for sharpness and AF sensitivity
- Fixed MenuOpenClose and added dynamic_range inquiry

#### Bounded Command Queues (#464)
- Pending-command queue now bounded to prevent unbounded memory growth

### Fixed

#### Runtime Reliability
- SmolExecutor: Replace busy-polling with waker-driven timeouts
- BlockingRunner: Make deadline-driven to eliminate 10ms polling (#459)
- Runtime loop: Wake on all control channels (#442)
- Runtime loop: Exit gracefully on connection close

#### Memory Safety
- subscribe_completions() buffers now bounded (#462)
- Retry queue made lifecycle-aware to prevent stale retries (#460)
- 16-bit sequence correlation made collision-safe and allocation-free (#457)

#### Protocol Correctness
- ProtocolFramer: Resync on max-buffer overflow to prevent stalls (#465)
- Transport send failures: Preserve root cause + timeout semantics (#469)
- Transport send failures: Unify handling with SchedulerAction pipeline (#458)
- TCP/UDP endpoint normalization canonicalized (#461)

#### Camera-Specific Fixes
- PTZOptics: Route flip commands through combined 0xA4 register (#440)
- Decoders: Add missing SharpnessPosition decoder (#453)

#### Serial Transport
- Remove Arc<Mutex> to prevent self-deadlock in blocking transport

#### Diagnostics
- Add InquiryKind context to decoder errors and timeout warnings (#452)
- Improve inquiry timeout logging with elapsed time and duplicate tracking
- Remove dead code and add inflight count to inquiry tracing (#454)

#### API/Compile-Time
- Enforce compile-time mode selection
- Remove broken intra-doc links from CommandId
- Re-export CompletionEvent to fix rustdoc warnings
- Remove conditional RuntimeHandle doc link that broke no-feature builds

### Internal

#### Refactoring
- Eliminate panic-prone parameter encoding in visca_command! macro
- Narrow PanTiltLimitCorner to only two representable corners (#463)

#### Documentation
- Document connection sharing patterns for multi-client applications (#455)

#### Testing
- Add regression tests for runtime loop liveness fix (#442)

#### Chores
- Remove bindings/ directory (ts-rs auto-export handles downstream)
- Remove unused templates

## [0.9.0] - 2025-11-27

### Breaking Changes

#### Generic Zoom and Focus API (#102)

The zoom and focus APIs have been simplified by removing unit-specific method names in favor of generic methods that accept multiple input types via `TryInto` trait bounds.

**What Changed:**

- `zoom_absolute()` and `set_zoom_position()` → `set_zoom<T>()` in 0.9.0; this zoom API was later replaced by the profile-aware 1.0 direct zoom contract.
- `set_focus()` updated to accept generic types via `TryInto<FocusPosition>`
- `set_focus_near_limit()` updated to accept generic types
- `set_zoom_op()` and `set_focus_op()` async operation methods updated similarly

**Migration Guide:**

```rust
// Before (0.8.x)
camera.zoom_absolute(ZoomPosition::new(0x4000)?)?;
camera.set_zoom_position(0x4000)?;

// After the 1.0 zoom contract
camera.set_zoom(ZoomPosition::new(0x4000)?)?; // Raw VISCA position
camera.set_zoom_normalized(UnitInterval::new(0.5)?)?; // Optical normalized position

// Focus works similarly
camera.set_focus(Percentage(75.0))?;
camera.set_focus(FocusPosition::new(0x1000)?)?;
```

**Impact:**

- Cleaner, more intuitive API for setting zoom and focus
- Single method supports multiple unit types
- Compile-time type safety via `TryInto` bounds

#### Async Camera Timeout and Retry Configuration (#430)

Async cameras now properly use configured `TimeoutConfig` and `RetryConfig` values instead of silently ignoring them. This aligns async camera behavior with blocking cameras, where configuration is the single source of truth for runtime behavior.

**What Changed:**

1. **`Camera::set_timeout_config()` removed for async mode**: This method previously updated only a local field without affecting the running async runtime. Now, timeout and retry configuration must be set at construction time.

2. **New `Camera::new_async_with_config()` constructor**: Accepts explicit `TimeoutConfig` and `RetryConfig` parameters that are actually applied to the runtime.

3. **`CameraConfig` now passes configs to runtime**: `CameraConfig.timeouts` and `CameraConfig.retries` are now correctly wired into the async runtime at construction time.

4. **`CameraBuilder` gains `retry_config()` method**: Builder now allows setting both timeout and retry configuration for async cameras.

**Migration Guide:**

If you were calling `camera.set_timeout_config(...)` on async cameras:

```rust
// Before (0.8.x) - this didn't actually work!
let mut camera = Camera::<Async, P, _, _>::new_async(transport, executor).await?;
camera.set_timeout_config(custom_timeouts);

// After - use constructor with config or CameraConfig/CameraBuilder
let camera = Camera::<Async, P, _, _>::new_async_with_config(
    transport,
    executor,
    custom_timeouts,
    RetryConfig::default(),
).await?;

// Or use CameraConfig (recommended):
let session = CameraConfig::<PtzOpticsG2>::new()
    .timeouts(custom_timeouts)
    .retries(custom_retries)
    .open_async(runtime).await?;

// Or use CameraBuilder:
let camera = CameraBuilder::with_executor(executor)
    .timeout_config(custom_timeouts)
    .retry_config(custom_retries)
    .open_async::<PtzOpticsG2, _>(transport)
    .await?;
```

**Impact:**

- Async cameras now actually respect user-configured timeouts and retries
- Behavior is now consistent between blocking and async modes
- Configuration is type-driven and validated at construction time

### Added

#### Capability Flags for Picture Effects and Tally Lights

Fine-grained capability detection for picture effects and tally lights prevents cameras from advertising unsupported commands.

**New Capabilities:**

- `SUPPORTS_PICTURE_EFFECT` flag in `ImageProcessing` trait (default: false)
- `Tally` capability trait with `SUPPORTS_TALLY` constant
- `has_picture_effect` and `has_tally` fields in `Capabilities` discovery struct

**Profile Updates:**

- `PtzOpticsG2` explicitly sets both flags to `false`
- Sony professional cameras (`FR7`, `BRC-H900`) enable both capabilities

This prevents cameras from receiving syntax errors when the poller attempts to query `TallyStatus` or `PictureEffect` on cameras that don't support these commands.

### Fixed

#### TransportOptions Serde Representation

Changed `TransportOptions` enum to use serde's internally-tagged representation for better compatibility with TypeScript/JavaScript frontends.

**Before:**
```json
{"Tcp": {"address": "192.168.0.110:5678"}}
```

**After:**
```json
{"type": "TCP", "address": "192.168.0.110:5678"}
```

This matches common JSON API conventions and simplifies frontend integration.

### Internal

- Reduced inquiry matching verbosity: TRACE for detailed flow, DEBUG for interesting situations
- Simplified error logging: only log syntax errors (0x02) at ERROR level, not executable errors (0x41)
- Changed response decoding from DEBUG to TRACE level
- Use lazy evaluation for expensive hex formatting

## [0.8.0] - 2025-10-16

### 📋 What's New in 0.8.0 - Quick Overview

**TL;DR**: This release adds powerful ergonomic features, fixes protocol compliance issues, and eliminates the need for downstream wrapper code—all while maintaining backward compatibility for most use cases. **Most users can upgrade with minimal to no code changes.**

**Key Highlights:**
- ✨ **Optional Serialization**: Add `features = ["serde", "schemars"]` to serialize all types (eliminates ~150 lines of wrapper code)
- 🎯 **Better Ergonomics**: Direct f64 usage, built-in speed mapping, rich inquiry conversions, 26+ new convenience methods
- 🐛 **Protocol Fixes**: Corrected AWB sensitivity inquiry (was inverted), normalized tally APIs to baseline VISCA
- 🚀 **Performance**: Zero-allocation send path (2 allocations → 0 for standard commands)
- 🔧 **API Enhancements**: Type-safe operation handles, profile dispatch helpers, error context with retry intelligence
- 🛡️ **Type Safety**: Inquiry type fixes (red/blue tuning now correctly i8, new typed wrappers for gamma/noise reduction)

### ⚡ Do I Need to Migrate?

**Quick Decision Tree:**

1. **Are you using `auto_wb_sensitivity()` inquiry or `tally_status()` methods?**
   - → **YES**: Required changes (see Breaking Changes below)
   - → **NO**: Continue to #2

2. **Are you pattern-matching on `TransportHandle` or `ErrorKind`?**
   - → **YES**: Minor changes needed (add wildcard patterns)
   - → **NO**: Continue to #3

3. **Do you want to use new features (serialization, diagnostics, better ergonomics)?**
   - → **YES**: Opt-in via features and new APIs (see New Features below)
   - → **NO**: **You're done! No migration needed.**

### 🎯 Philosophy: Runtime-Agnostic Modernization & Protocol Correctness

This release focuses on eliminating downstream boilerplate, completing runtime-agnostic modernization, enhancing protocol correctness, and improving the ergonomics of camera control. The library now provides feature-gated serialization, intuitive unit conversions, comprehensive diagnostic utilities, uniform transport handling, and spec-validated protocol fixes—all without forcing users into a specific async runtime.

The central achievement is adding powerful ergonomic features that previously required custom downstream wrappers while ensuring strict VISCA specification compliance. The library now ships with feature-gated serialization, intuitive unit conversions, diagnostic utilities, uniform transport handling (including serial), enhanced error types, and spec-validated protocol corrections—all without forcing users into a specific async runtime.

### 🚀 Major Features & Improvements

#### Inquiry Type Safety Enhancements
Enhanced type safety for inquiry responses, preventing type confusion and ensuring correct value interpretation:

- **Red/Blue tuning inquiry type fixes**: Fixed type mismatches for `red_tuning()` and `blue_tuning()` inquiries from `u8` to `i8` to correctly represent -10 to +10 offset range. Updated all inquiry trait signatures, response decoders, blocking API, and accessor methods.
- **New type wrappers for image control**: Added `GammaLevel` (0-4 range) and `NoiseReductionLevel` (0-5 range) type wrappers to complement existing typed inquiry responses.
- **Consistent type safety across inquiries**: All tuning-related inquiry methods now return properly typed values that match the VISCA protocol semantics (gain offsets vs absolute values).

These improvements ensure that inquiry responses are type-safe throughout the API, eliminating potential bugs from incorrect type assumptions and providing better compile-time validation.

#### Protocol Correctness Fixes (#418)
Spec-validated VISCA protocol corrections ensure accurate camera control and telemetry:

- **Auto White Balance (AWB) Sensitivity inquiry fixed**: Corrected inverted mapping in inquiry decoder to match VISCA spec and command encoding (High=0x00, Normal=0x01, Low=0x02)
- **Tally APIs normalized to baseline VISCA**: Red and green tally inquiries now follow baseline VISCA specification; vendor-specific extensions properly documented
- **16-bit Zoom/PT position semantics codified**: ZoomPosition and PanTilt position types explicitly document 16-bit semantics per VISCA spec; profile-aware signed/unsigned coordinate handling maintained
- **Transaction model clarified**: Documentation and tests confirm inquiries do not ACK, commands follow ACK/Completion model with two-socket management

These fixes ensure accurate camera state reporting and proper protocol compliance across all VISCA-compatible cameras.

#### Serialization & Schema Support
All public value types now support optional serialization through feature-gated `serde` and `schemars` derives:

```toml
[dependencies]
grafton-visca = { version = "0.8", features = ["serde", "schemars"] }
```

With these features enabled, you can serialize/deserialize all value types directly and generate JSON schemas for API documentation, eliminating the need for downstream wrapper types.

**Dependency Update**: Upgraded `schemars` from 0.8 to 1.0 for compatibility with latest ecosystem tools.

#### Ergonomic Type Conversions
- **From<f64> for numeric types**: `Degrees`, `Normalized`, and other numeric types accept `f64` directly, eliminating manual casts
- **Published MIN/MAX constants**: All range types expose validation bounds (e.g., `PanSpeed::MIN`, `PanSpeed::MAX`)
- **Validated constructors**: All parameter types provide `new()` methods with clear error messages including valid ranges
- **Model-aware validation**: New `new_for_model()` constructors validate against specific camera capabilities

```rust
// Old (0.7.1): Manual casting
camera.pan_tilt_absolute(Degrees(45.0 as f32), Degrees(15.0 as f32), SpeedLevel::Fast)?;

// New (0.8.0): Natural f64 usage
camera.pan_tilt_absolute(Degrees(45.0), Degrees(15.0), SpeedLevel::Fast)?;

// Model-aware validation
let speed = PanSpeed::new_for_model(20, CameraVariant::PtzOpticsG2)?;
```

#### Inquiry Conversions
New `inquiry_conversions` module provides helpers for converting raw VISCA values to user-friendly formats:

```rust
use grafton_visca::{
    inquiry_conversions::{PanTiltPositionRaw, PanTiltPositionDeg, ZoomDomain},
    UnitInterval, ZoomPositionExt,
};

// Convert raw pan/tilt to degrees
let raw = PanTiltPositionRaw::new(1224, 648);
let deg = raw.as_degrees();  // ~85° pan, ~45° tilt

// Domain-aware zoom normalization
let zoom_pos = camera.inquiry().zoom_position().await?;
let optical_norm = zoom_pos.normalize_with_max(ZoomDomain::Optical, 0x4000, Some(0x7000))?;
let full_norm = zoom_pos.normalize_with_max(ZoomDomain::OpticalPlusDigital, 0x4000, Some(0x7000))?;

// Create zoom position from normalized value
let zoom = zoom_from_normalized(UnitInterval::new(0.5)?, ZoomDomain::Optical, 0x4000, Some(0x7000))?;
camera.set_zoom(zoom)?;
```

Types added:
- `PanTiltPositionRaw` / `PanTiltPositionDeg` - Raw and degree-based position representations
- `ZoomDomain` - Enum for Optical vs OpticalPlusDigital normalization
- `Normalized` - Type-safe wrapper for 0.0-1.0 values
- `ZoomPositionExt` trait - Domain-aware normalization methods

#### Coarse Speed Mapping
Canonical mapping from user-friendly speed levels to device-specific values:

```rust
use grafton_visca::types::Coarse;

// Old (0.7.1): Custom mapping tables in application code
let zoom_speed = match user_speed {
    UserSpeed::Slow => ZoomSpeed::new(2)?,
    UserSpeed::Medium => ZoomSpeed::new(4)?,
    UserSpeed::Fast => ZoomSpeed::new(6)?,
    // ...
};

// New (0.8.0): Built-in canonical mapping
let zoom_speed = ZoomSpeed::from_coarse(Coarse::Fast);  // → 6
let pan_speed = PanSpeed::from_coarse(Coarse::Medium);  // → 12
let tilt_speed = TiltSpeed::from_coarse(Coarse::Slow);  // → 5
```

`Coarse` is a type alias for `SpeedLevel` with five intuitive levels: Slowest, Slow, Medium, Fast, Fastest.

#### Color Temperature Control
Direct color temperature control for precise white balance adjustment:

```rust
// Set color temperature in Kelvin (2800K - 8000K range)
camera.color_temperature_direct(5600)?;  // Daylight

// Color temperature is now accessible via white balance inquiry
let temp_k = camera.color_temperature()?;
println!("Current color temperature: {}K", temp_k);
```

This provides fine-grained control over white balance beyond the standard presets (Indoor/Outdoor/OnePush), enabling precise color matching for professional workflows.

#### API Ergonomics & Runtime Polymorphism (#427)
Comprehensive API improvements to eliminate downstream boilerplate, delivering on the promise to reduce downstream code by ~600 lines while improving type safety, ergonomics, and API discoverability. These improvements directly address pain points discovered during real-world integration in downstream projects.

##### 1. Profile Dispatch & Runtime Polymorphism
Runtime profile selection without boilerplate dispatch logic:

```rust
use grafton_visca::camera::profiles::{ProfileId, ProfileGroup};

// Get profile group for runtime dispatch
let profile = ProfileId::PtzOpticsG2;
let group = profile.profile_group();  // → ProfileGroup::PtzOpticsG2

match group {
    ProfileGroup::GenericVisca => { /* ... */ }
    ProfileGroup::PtzOpticsG2 => { /* ... */ }
    ProfileGroup::SonyProfessional => { /* ... */ }
}
```

**What's New:**
- `ProfileId` enum with all camera models (PtzOpticsG2, SonyFr7, GenericVisca, etc.)
- `ProfileGroup` enum categorizing profiles into three groups
- `ProfileId::profile_group()` method for runtime dispatch
- Single source of truth for profile groupings

**Migration:**
```rust
// Old (0.7.x): Custom profile dispatch (218 lines of boilerplate)
pub trait ProfileGroup: Profile + Default + Send + Sync + 'static {
    fn matches(profile: ProfileId) -> bool;
}

impl ProfileGroup for GenericVisca {
    fn matches(profile: ProfileId) -> bool {
        matches!(profile, ProfileId::SonyFr7 | ProfileId::GenericVisca | ...)
    }
}

pub async fn dispatch_udp(profile: ProfileId, addr: &str, runtime: TokioRuntime)
    -> Result<Camera, Error>
{
    if GenericVisca::matches(profile) {
        let cam = Connect::open_udp_async::<GenericVisca, _>(addr, runtime).await?;
        Ok(Arc::new(cam))
    } else if PtzOpticsG2::matches(profile) {
        // ... repeat for each profile
    }
}

// New (0.8.0): Built-in profile dispatch
use grafton_visca::camera::profiles::ProfileGroup;

let group = profile.profile_group();
match group {
    ProfileGroup::GenericVisca => {
        Connect::open_udp_async::<GenericVisca, _>(addr, runtime).await?
    }
    ProfileGroup::PtzOpticsG2 => {
        Connect::open_udp_async::<PtzOpticsG2, _>(addr, runtime).await?
    }
    ProfileGroup::SonyProfessional => {
        Connect::open_udp_async::<SonyFR7, _>(addr, runtime).await?
    }
}
```

##### 2. Command Construction Ergonomics
Comprehensive trait methods eliminate manual command construction:

```rust
// Image processing operations
camera.set_image_flip(ImageFlipMode::Both).await?;
camera.enable_freeze().await?;
camera.disable_freeze().await?;
camera.set_contrast(ContrastLevel::new(5)?).await?;
camera.set_sharpness(SharpnessLevel::new(7)?).await?;
camera.set_luminance(LuminanceLevel::new(10)?).await?;
camera.set_saturation(SaturationLevel::new(8)?).await?;
camera.set_noise_reduction_2d(NoiseReduction2DLevel::new(3)?).await?;
camera.set_noise_reduction_3d(NoiseReduction3DLevel::new(2)?).await?;
camera.set_picture_effect(PictureEffectMode::Negative).await?;

// Color control operations
camera.set_color_temperature(5600).await?;
camera.set_red_gain(GainLevel::new(5)?).await?;
camera.set_blue_gain(GainLevel::new(6)?).await?;

// White balance operations
camera.set_white_balance_mode(WhiteBalanceMode::Auto).await?;
camera.set_awb_sensitivity(AutoWhiteBalanceSensitivity::High).await?;
camera.white_balance_one_push().await?;

// ND filter operations
camera.set_nd_filter_mode(NdFilterMode::Clear).await?;

// Tally light operations
camera.set_tally_red(true).await?;
camera.set_tally_green(false).await?;
```

**What's New:**
- 26+ new trait methods across control domains
- Consistent API across 22 control traits
- Mode-generic implementation (works for blocking and async)
- All commonly-used commands now have dedicated trait methods

**Migration:**
```rust
// Old (0.7.x): Manual command construction (26+ instances)
use grafton_visca::command::image::ImageFlipCombinedCommand;
let cmd = ImageFlipCombinedCommand::new(mode);
camera.execute(cmd).await?;

use grafton_visca::command::flip::ImageFreeze;
let cmd = ImageFreeze { on: enabled };
camera.execute(cmd).await?;

use grafton_visca::command::tally::{TallyRedOn, TallyRedOff};
if enabled {
    camera.execute(TallyRedOn::new()).await?
} else {
    camera.execute(TallyRedOff::new()).await?
}

// New (0.8.0): Direct trait methods
camera.set_image_flip(mode).await?;
camera.enable_freeze().await?;
camera.set_tally_red(enabled).await?;
```

##### 3. Enhanced Position Normalization
Convenient methods on value types for common conversions:

```rust
use grafton_visca::types::ZoomPosition;

// Zoom position normalization (built-in methods on the type)
let zoom_pos = camera.zoom_position().await?;

// Get normalized position in optical zoom range [0.0, 1.0]
let optical = zoom_pos.normalized_optical();  // Direct method!

// Get normalized position in combined range [0.0, 1.0]
let combined = zoom_pos.normalized_combined();  // Direct method!

// Get raw value
let raw = zoom_pos.value();

// Pan/tilt position conversions
let pt_pos = camera.pan_tilt_position().await?;
let (pan_deg, tilt_deg) = pt_pos.as_degrees();  // Direct conversion!
let (pan_raw, tilt_raw) = pt_pos.raw_values();
```

**What's New:**
- `normalized_optical()` and `normalized_combined()` methods on `ZoomPosition`
- `as_degrees()` method on `PanTiltPosition`
- `raw_values()` methods for direct access
- Extension traits available for advanced use cases

**Migration:**
```rust
// Old (0.7.x): Extension traits and intermediate types
use grafton_visca::inquiry_conversions::{ZoomDomain, ZoomPositionExt, PanTiltPositionRaw};

let pos = camera.zoom_position().await?;
let normalized = pos.normalize(ZoomDomain::Optical).0.into();  // Returns Normalized<f32>

let pos = camera.pan_tilt_position().await?;
let raw = PanTiltPositionRaw::from(pos);
let deg = raw.as_degrees();
let (pan_deg, tilt_deg) = (deg.pan.0.into(), deg.tilt.0.into());

// New (0.8.0): Direct methods on types
let pos = camera.zoom_position().await?;
let normalized = pos.normalized_optical();  // Direct f64

let pos = camera.pan_tilt_position().await?;
let (pan_deg, tilt_deg) = pos.as_degrees();  // Direct tuple
```

##### 4. Extended InFlight Operation Coverage
`_op` variants now available for all long-running operations:

```rust
use std::time::Duration;

// All major long-running operations now have _op variants:

// Pan/tilt operations
let handle = camera.pan_tilt_home_op().await?;
handle.await_completion(Duration::from_secs(30)).await?;

let handle = camera.pan_tilt_reset_op().await?;
handle.await_completion(Duration::from_secs(30)).await?;

let handle = camera.pan_tilt_absolute_op(pan, tilt, speed).await?;
handle.await_completion(Duration::from_secs(20)).await?;

// Preset operations
let handle = camera.preset_recall_op(PresetNumber::new(1)?).await?;
handle.await_completion(Duration::from_secs(60)).await?;

// Zoom operations
let handle = camera.zoom_absolute_op(position).await?;
handle.await_completion(Duration::from_secs(10)).await?;

// Focus operations
let handle = camera.set_focus_op(position).await?;
handle.await_completion(Duration::from_secs(5)).await?;
```

**What's New:**
- `pan_tilt_home_op()` - Returns `InFlight<PanTilt>`
- `pan_tilt_reset_op()` - Returns `InFlight<PanTilt>`
- `preset_recall_op()` - Returns `InFlight<Preset>`
- Plus existing: `pan_tilt_absolute_op()`, `zoom_absolute_op()`, `set_focus_op()`

**Migration:**
```rust
// Old (0.7.x): Manual timeout wrapping
tokio::time::timeout(
    Duration::from_secs(30),
    camera.pan_tilt_home()
).await??;

tokio::time::timeout(
    Duration::from_secs(60),
    camera.preset_recall(preset)
).await??;

// New (0.8.0): Typed InFlight handles
let handle = camera.pan_tilt_home_op().await?;
handle.await_completion(Duration::from_secs(30)).await?;

let handle = camera.preset_recall_op(preset).await?;
handle.await_completion(Duration::from_secs(60)).await?;
```

##### 5. Inquiry Availability Documentation
Clear documentation of write-only operations:

```rust
// Write-only operations are now clearly documented with **Note:** sections

/// Set contrast level.
///
/// **Note:** Contrast is write-only on most cameras. There is no corresponding
/// inquiry command to read back the current contrast level.
fn set_contrast(&self, level: ContrastLevel) -> Result<(), Error>;

/// Set sharpness level.
///
/// **Note:** The sharpness level itself is write-only on most cameras. While you can
/// query the sharpness mode (auto/manual) via [`InquiryControl::sharpness_mode`],
/// there is no inquiry to read back the specific sharpness level value.
fn set_sharpness(&self, level: SharpnessLevel) -> Result<(), Error>;

/// Set luminance (brightness) level.
///
/// **Note:** Luminance is write-only on most cameras. There is no corresponding
/// inquiry command to read back the current luminance level.
fn set_luminance(&self, level: LuminanceLevel) -> Result<(), Error>;

/// Set auto white balance sensitivity.
///
/// **Note:** AWB sensitivity is write-only. There is no corresponding inquiry
/// command to read back the current sensitivity setting.
fn set_awb_sensitivity(&self, sensitivity: AutoWhiteBalanceSensitivity) -> Result<(), Error>;
```

**What's New:**
- Consistent `**Note:**` documentation for all write-only operations
- Explains VISCA protocol limitations
- Suggests alternatives when available
- Prevents users from expecting unavailable inquiries

**Migration:**
```rust
// Old (0.7.x): Unclear inquiry availability, runtime errors
fn wb_get_awb_sensitivity(&self) -> Result<AutoWhiteBalanceSensitivity, Error> {
    Err(Error::FeatureNotSupported {
        feature: "AWB sensitivity inquiry"
    })
}

// New (0.8.0): Clear compile-time documentation
// Users know upfront that these are write-only operations
// No unexpected runtime errors from missing inquiries
```

##### 6. Profile Capabilities Metadata
Rich metadata for runtime introspection and validation:

```rust
use grafton_visca::capabilities::{ProfileMetadata, PanTilt, Zoom, Exposure};
use grafton_visca::camera::profiles::PtzOpticsG2;

// Profile metadata constants
assert_eq!(PtzOpticsG2::MODEL_NAME, "PtzOptics G2");
assert_eq!(PtzOpticsG2::DEFAULT_TCP_PORT, 5678);
assert_eq!(PtzOpticsG2::DEFAULT_UDP_PORT, 1259);
assert_eq!(PtzOpticsG2::ACK_TIMEOUT, Duration::from_millis(100));
assert_eq!(PtzOpticsG2::COMPLETION_TIMEOUT, Duration::from_millis(5000));

// Pan/tilt capabilities
assert_eq!(PtzOpticsG2::PAN_RANGE, -2448..2449);
assert_eq!(PtzOpticsG2::TILT_RANGE, -432..1297);
assert_eq!(PtzOpticsG2::MAX_PAN_SPEED, 24);
assert_eq!(PtzOpticsG2::MAX_TILT_SPEED, 20);

// Zoom capabilities
assert_eq!(PtzOpticsG2::OPTICAL_ZOOM_MAX, 0x4000);
assert_eq!(PtzOpticsG2::DIGITAL_ZOOM_MAX, Some(0x7000));

// Exposure capabilities
assert_eq!(PtzOpticsG2::IRIS_RANGE, 0x00..0x1D);
assert_eq!(PtzOpticsG2::GAIN_RANGE, 0..9);
assert_eq!(PtzOpticsG2::SUPPORTS_AUTO_EXPOSURE, true);
assert_eq!(PtzOpticsG2::SUPPORTS_BACKLIGHT_COMP, true);
assert_eq!(PtzOpticsG2::SUPPORTS_WDR, true);

// Compile-time capability markers
fn requires_nd_filter<P: NdFilter>(camera: &Camera<P>) {
    // Only profiles with ND filter support can call this
}
```

**What's New:**
- `ProfileMetadata` trait with rich constants
- Capability traits: `PanTilt`, `Zoom`, `Focus`, `Exposure`, `WhiteBalance`, `ImageProcessing`, `Presets`, `Power`, `MotionSync`, `NdFilter`
- Marker traits for compile-time checking: `HasAutoExposure`, `HasBacklightCompensation`, `HasWDR`, `HasAutoFocus`, `HasOnePushFocus`, etc.
- Parameter limits as const values (ranges, speed limits, conversion factors)

**Migration:**
```rust
// Old (0.7.x): Hard-coded limits in application code
const PTZ_OPTICS_PAN_MAX: i16 = 2448;
const PTZ_OPTICS_TILT_MAX: i16 = 1296;

// New (0.8.0): Use profile constants
use grafton_visca::camera::profiles::PtzOpticsG2;

if pan > PtzOpticsG2::PAN_RANGE.end {
    return Err(Error::OutOfRange { /* ... */ });
}
```

##### 7. Error Context Enhancement
Composable error context while preserving retry intelligence:

```rust
use grafton_visca::Error;

// Add context to errors
camera.power_on()
    .await
    .context("Failed to power on camera for preset recall")?;

// Retry intelligence preserved
match camera.send_command(cmd).await {
    Err(e) => {
        println!("Error: {}", e);  // Includes context
        println!("Retryable: {}", e.is_retryable());  // Still works!
        if let Some(delay) = e.suggested_retry_delay() {
            sleep(delay).await;
            // retry...
        }
    }
    Ok(_) => {}
}

// WithContext error variant
match error {
    Error::WithContext { context, source } => {
        println!("Context: {}", context);
        println!("Source: {}", source);
        // Retry intelligence delegated to source error
    }
    _ => {}
}
```

**What's New:**
- `with_context()` method on `Error` type
- `context()` method for Display types
- `WithContext` error variant that preserves inner error
- Retry intelligence (`is_retryable()`, `suggested_retry_delay()`) preserved through context chain

**Migration:**
```rust
// Old (0.7.x): Lost error context in complex flows
camera.power_on().await?;  // Generic error message

// New (0.8.0): Rich error context
camera.power_on()
    .await
    .context("Failed to power on camera for preset recall")?;

// Error message: "Failed to power on camera for preset recall: CommandTimeout"
```

**Benefits Summary:**
- **Profile Dispatch**: Eliminates ~218 lines of boilerplate
- **Command Ergonomics**: Reduces command construction by ~50%
- **Type Serialization**: Eliminates ~150 lines of type wrappers (covered in Serialization section)
- **Position Conversions**: Simpler, more discoverable API
- **InFlight Coverage**: Consistent async operation handling
- **Documentation**: Clear API contracts prevent surprises
- **Capabilities Metadata**: Better validation and introspection
- **Error Context**: Richer error messages without losing retry metadata

**Total Impact**: ~400+ lines of downstream boilerplate eliminated across all improvements.

#### Diagnostics & Health Checks
New `diagnostics` module provides tools for camera health monitoring:

```rust
use grafton_visca::diagnostics::Diagnostics;

// Probe camera for connectivity and latency
let report = camera.probe().await?;
if report.is_healthy() {
    println!("Camera responsive, RTT: {:?}", report.rtt);
}

// Simple ping check
if camera.ping().await? {
    println!("Camera is online");
}

// Measure average latency
let latency = camera.measure_latency(5).await?;
println!("Average RTT: {:?}", latency);
```

Types added:
- `ProbeReport` - Health check results with RTT and transport status
- `Diagnostics` trait - Methods for `probe()`, `ping()`, and `measure_latency()`
- **Prelude export**: `Diagnostics` trait now exported in prelude for easier access without explicit imports

#### Typed Operation Handles for Long-Running Commands
New `InFlight<C>` handles provide type-safe lifecycle management for camera operations:

```rust
use std::time::Duration;

// Start operation and get typed handle
let handle = camera.pan_tilt_absolute_op(
    Degrees(45.0),
    Degrees(15.0),
    SpeedLevel::Fast
).await?;

// Wait for completion (automatically selects correct waiter)
handle.await_completion(Duration::from_secs(5)).await?;

// Or cancel the operation (socket-safe, ID-based)
handle.cancel().await?;
```

**Key features:**
- **Socket-safe cancellation**: Cancel commands by ID without tracking sockets manually
- **Type-directed completion waits**: Compile-time selection of correct waiter (`await_pan_tilt_idle`, `await_zoom_idle`, etc.)
- **Zero-cost abstraction**: Uses ZST markers with no runtime overhead
- **Available `_op` variants**:
  - `pan_tilt_absolute_op()` / `pan_tilt_relative_op()` → `InFlight<PanTilt>`
  - `zoom_absolute_op()` → `InFlight<Zoom>`
  - `set_focus_op()` → `InFlight<Focus>`
  - `preset_recall_op()` → `InFlight<Preset>`

This eliminates common correctness pitfalls when managing long-running operations and removes the need for downstream wrappers to track command IDs and socket mappings.

**Note**: Available in async mode only (`#[cfg(feature = "mode-async")]`). Fire-and-forget methods remain unchanged for simple use cases.

#### Motion Control Enhancements
Convenient motion control improvements for better ergonomics:

```rust
// New stop_all_motion() method for single-call motion stopping
camera.stop_all_motion()?;  // Internally calls pan_tilt_stop()

// Available for all camera types through the MotionControl trait
// Clear documentation and consistent API across control methods
```

This provides a more intuitive API for emergency stops and motion control without needing to know which specific motion axis to stop.

#### Command Cancellation
Built-in support for canceling in-flight commands without application-level wrappers:

```rust
use grafton_visca::ViscaSocket;

// Start a command and get its ID for later cancellation
let (command_id, future) = camera.start_command_with_id(&zoom_cmd).await?;

// Cancel by command ID
camera.cancel(command_id).await?;
// The future will resolve with Err(Error::CommandCanceled)

// Or cancel all commands on a specific socket
camera.cancel_socket(ViscaSocket::S1).await?;
```

This provides first-class cancellation support for long-running operations like preset recalls or movements, enabling responsive UIs and timeout handling without runtime-specific wrappers.

#### Connection Timeout Enforcement (#417)
Async connectors now properly enforce `connect_timeout` and use non-blocking DNS resolution:

```rust
use grafton_visca::transport::TransportConfig;
use std::time::Duration;

let config = TransportConfig::default()
    .with_connect_timeout(Duration::from_millis(500));

// Old (0.7.1): connect_timeout was ignored in async, blocking DNS
// New (0.8.0): Timeout enforced, async DNS used
let transport = Transport::tcp()
    .address("192.168.0.110:5678")
    .config(config)
    .connect().await?;  // Times out after 500ms if unreachable
```

Changes per runtime:
- **Tokio**: Uses `tokio::time::timeout()` and `tokio::net::lookup_host()`
- **async-std**: Uses `async_std::future::timeout()` and `async_std::net::ToSocketAddrs`
- **smol**: Uses `async_io::Timer` with `futures_lite::future::race()` and `smol::unblock()` for DNS

#### Uniform Transport Support (#415)
Serial transport now integrated into `TransportHandle` enum, enabling uniform trait implementations:

```rust
// Old (0.7.1): Serial used separate type, preventing uniform trait implementations
pub enum TransportHandle<R: Runtime> {
    Udp(UdpTransport<R>),
    Tcp(TcpTransport<R>),
}

// New (0.8.0): All transports unified
pub enum TransportHandle<R: Runtime> {
    Udp(UdpTransport<R>),
    Tcp(TcpTransport<R>),
    #[cfg(feature = "transport-serial-*")]
    Serial(<R as RuntimeSerial>::SerialTransport),
}
```

This enables downstream libraries to implement traits uniformly across all transport types without trait coherence conflicts.

#### Enhanced Error Types (#414)
Richer error information with retry hints and improved ergonomics:

```rust
match camera.send_command(cmd).await {
    Err(e) => {
        println!("Error kind: {:?}", e.kind());
        if e.is_retryable() {
            if let Some(delay) = e.suggested_retry_delay() {
                sleep(delay).await;
                // retry...
            }
        }
    }
    Ok(_) => {}
}
```

**New capabilities:**
- **Clone implementation**: `Error` type now implements `Clone`, enabling better error handling patterns in multi-threaded contexts
- **ErrorKind** variants and methods provide machine-actionable error classification for robust retry logic
- Improved error propagation and composition in complex control flows
- Better compatibility with error handling libraries and patterns

This makes error handling more flexible, especially when errors need to be stored, passed across threads, or used in retry logic.

#### Cancel Command Error Handling Clarification
Improved error handling and documentation for command cancellation:

- **Clear error semantics**: Cancel command failures now properly distinguished from successful cancellations
- **Documentation**: Clarified that cancel commands may receive error responses when no command is in progress
- **Error handling patterns**: Examples show proper handling of "command not executable" errors for cancel operations

This helps developers write more robust cancellation logic without false positives from expected error conditions.

### 📊 Impact on Downstream Projects

The serialization improvements in 0.8.0 enable significant boilerplate reduction in downstream projects:

**Real-world example from visca-mcp:**
- **Wrapper type elimination**: Eliminated ~139 lines of wrapper types by using grafton-visca types directly
  - Removed `Normalized01` wrapper (~71 lines) - now uses `Normalized<f32>` directly with serde support
  - Removed `PresetId` wrapper (~68 lines) - now uses `PresetNumber` directly with serde support
- **Cleaner API**: No more manual serde implementations or bridge TryFrom implementations needed
- **Type safety**: Maintained compile-time validation while reducing code complexity
- **JSON schema generation**: Automatic schema generation for all parameter types via schemars feature

**Migration example:**
```rust
// Before (0.7.1): Custom wrapper types required
pub struct Normalized01(f32);
impl Serialize for Normalized01 {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where S: Serializer {
        // ~20 lines of validation and serialization
    }
}
impl<'de> Deserialize<'de> for Normalized01 {
    // ~25 lines of deserialization
}
impl TryFrom<Normalized01> for Normalized<f32> {
    // ~15 lines of conversion
}
// Total: ~71 lines

// After (0.8.0): Direct usage with serde feature
use grafton_visca::units::Normalized;
// 1 line - full serialization, validation, and schema support included
```

This demonstrates the library's maturity as a foundational component that reduces, rather than increases, downstream complexity.

### 📝 API Changes & Migration Guide

#### Import Changes

Most user-facing APIs remain unchanged. The primary additions are new modules:

```rust
// New modules (0.8.0)
use grafton_visca::{
    inquiry_conversions::{Normalized, PanTiltPositionRaw, PanTiltPositionDeg, ZoomDomain},
    diagnostics::{Diagnostics, ProbeReport},
    types::Coarse,
    ViscaSocket,
};
```

#### Type Construction

```rust
// Old (0.7.1): Manual casting from f64
camera.pan_tilt_absolute(
    Degrees(45.0 as f32),
    Degrees(15.0 as f32),
    SpeedLevel::Fast
)?;

// New (0.8.0): Direct f64 usage
camera.pan_tilt_absolute(
    Degrees(45.0),
    Degrees(15.0),
    SpeedLevel::Fast
)?;

// New (0.8.0): Model-aware validation
let pan = PanPosition::new_for_model(2000, CameraVariant::PtzOpticsG2)?;
```

#### Speed Mapping

```rust
// Old (0.7.1): Custom mapping in application
fn map_to_zoom_speed(level: UISpeed) -> ZoomSpeed {
    match level {
        UISpeed::Slow => ZoomSpeed::new(2).unwrap(),
        UISpeed::Medium => ZoomSpeed::new(4).unwrap(),
        UISpeed::Fast => ZoomSpeed::new(6).unwrap(),
    }
}

// New (0.8.0): Use built-in Coarse mapping
let speed = ZoomSpeed::from_coarse(Coarse::Medium);  // → 4
```

#### Inquiry Result Handling

```rust
// Old (0.7.1): Manual conversion
let raw_pos = camera.inquiry().pan_tilt_position().await?;
let deg_pan = (raw_pos.pan as f32) * 170.0 / 2448.0;
let deg_tilt = /* complex asymmetric formula */;

// New (0.8.0): Built-in conversions
let raw_pos = camera.inquiry().pan_tilt_position().await?;
let deg_pos = raw_pos.as_degrees();
println!("Pan: {}°, Tilt: {}°", deg_pos.pan.0, deg_pos.tilt.0);

// Or with profile-specific adjustments
let deg_pos = raw_pos.as_degrees_with_profile(&profile);
```

#### Zoom Normalization

```rust
// Old (0.7.1): Manual normalization with hard-coded constants
let zoom_pos = camera.inquiry().zoom_position().await?;
let normalized_optical = (zoom_pos.value() as f32) / 0x4000 as f32;
let normalized_full = (zoom_pos.value() as f32) / 0x7000 as f32;

// New (0.8.0): Domain-aware normalization
use grafton_visca::ZoomPositionExt;

let zoom_pos = camera.inquiry().zoom_position().await?;
let optical_norm = zoom_pos.normalize_with_max(ZoomDomain::Optical, 0x4000, Some(0x7000))?;
let full_norm = zoom_pos.normalize_with_max(ZoomDomain::OpticalPlusDigital, 0x4000, Some(0x7000))?;

// Set zoom from normalized value
camera.set_zoom_normalized(UnitInterval::new(0.5)?).await?;
```

#### Health Checks

```rust
// Old (0.7.1): Custom probe using arbitrary inquiry
async fn check_camera(camera: &Camera) -> bool {
    tokio::time::timeout(
        Duration::from_millis(100),
        camera.inquiry().zoom_position()
    ).await.is_ok()
}

// New (0.8.0): Dedicated diagnostics
use grafton_visca::diagnostics::Diagnostics;

let report = camera.probe().await?;
if report.is_healthy() {
    println!("RTT: {:?}", report.rtt);
}
```

#### Color Temperature Control (New Feature)

```rust
// Old (0.7.1): Limited to preset white balance modes
camera.white_balance_indoor().await?;  // ~3200K
camera.white_balance_outdoor().await?; // ~5600K

// New (0.8.0): Direct Kelvin control for precise matching
camera.color_temperature_direct(4500).await?;  // Exact 4500K
camera.color_temperature_direct(6500).await?;  // D65 standard

// Query current color temperature
let temp_k = camera.color_temperature().await?;
println!("Current: {}K", temp_k);  // e.g., "Current: 4500K"
```

#### Typed Operation Handles (New Feature)

```rust
// Old (0.7.1): Manual tracking of command IDs and sockets for cancellation
let cmd_id = camera.send_command_with_id(&cmd).await?.0;
// ... later, need to track which socket the command is on
camera.cancel_socket(ViscaSocket::S1).await?;

// New (0.8.0): Type-safe operation handles (async mode only)
let handle = camera.pan_tilt_absolute_op(
    Degrees(45.0),
    Degrees(15.0),
    SpeedLevel::Fast
).await?;

// Socket-safe cancellation (runtime resolves socket automatically)
handle.cancel().await?;

// Type-directed completion wait
handle.await_completion(Duration::from_secs(5)).await?;

// Available for: pan_tilt_absolute_op, pan_tilt_relative_op,
// zoom_absolute_op, set_focus_op, preset_recall_op
```

#### Motion Control (New Feature)

```rust
// Old (0.7.1): Need to know which motion control to stop
camera.pan_tilt_stop().await?;
camera.zoom_stop().await?;
camera.focus_stop().await?;

// New (0.8.0): Single method for emergency stop
camera.stop_all_motion().await?;  // Stops all motion

// Still available for granular control
camera.pan_tilt_stop().await?;  // Stop only pan/tilt
```

#### Diagnostics (New Feature)

```rust
// Old (0.7.1): Custom health check implementations
async fn check_camera(camera: &Camera) -> bool {
    timeout(Duration::from_millis(100), camera.inquiry().power_state())
        .await
        .is_ok()
}

// New (0.8.0): Built-in diagnostics
use grafton_visca::diagnostics::Diagnostics;  // Now in prelude

// Quick ping check
if camera.ping().await? {
    println!("Camera online");
}

// Detailed health report
let report = camera.probe().await?;
if report.is_healthy() {
    println!("Camera healthy, RTT: {:?}", report.rtt);
}

// Measure average latency
let latency = camera.measure_latency(5).await?;
println!("Avg RTT: {:?}", latency);
```

#### Protocol Fixes Migration

```rust
// AWB Sensitivity Inquiry (Fixed in 0.8.0)
// Old (0.7.1): Returned inverted values
let sensitivity = camera.auto_wb_sensitivity().await?;
// High was reported as Low, Low as High ❌

// New (0.8.0): Returns correct values per VISCA spec
let sensitivity = camera.auto_wb_sensitivity().await?;
// High correctly reports as High, Low as Low ✓
assert_eq!(sensitivity, AutoWhiteBalanceSensitivity::High);

// Tally APIs (Normalized in 0.8.0)
// Old (0.7.1): Mixed baseline and vendor-specific APIs
camera.tally_status().await?;  // Vendor-specific ❌

// New (0.8.0): Baseline VISCA only in main API
camera.tally_red().await?;     // Baseline VISCA ✓
camera.tally_green().await?;   // FR7 extension ✓
// Vendor extensions available via feature flags/profiles
```

#### Serialization (New Feature)

```rust
// Enable in Cargo.toml
// grafton-visca = { version = "0.8", features = ["serde", "schemars"] }

use grafton_visca::types::PanSpeed;

// Serialize to JSON
let speed = PanSpeed::new(12)?;
let json = serde_json::to_string(&speed)?;
assert_eq!(json, "12");

// Deserialize from JSON
let speed: PanSpeed = serde_json::from_str("15")?;
assert_eq!(speed.value(), 15);

// Generate JSON schema with schemars
let schema = schemars::schema_for!(PanSpeed);
```

### 🔥 Breaking Changes (Action Required)

> **Note**: These are the ONLY breaking changes in 0.8.0. If you're not using these specific APIs, you can upgrade without any code changes.

#### Protocol Correctness Fixes (Behavioral)
These fixes correct protocol violations and may change observed behavior:

1. **AWB Sensitivity Inquiry**: Returns correct VISCA-spec values
   ```rust
   // Old (0.7.1): Inverted mapping
   let sens = camera.auto_wb_sensitivity()?;
   // High=0x00 was decoded as Low ❌

   // New (0.8.0): Correct mapping
   let sens = camera.auto_wb_sensitivity()?;
   // High=0x00 correctly decoded as High ✓
   ```
   **Impact**: If you were compensating for the inverted values in your code, remove the compensation.

2. **Tally API Normalization**: Vendor-specific inquiry removed from baseline API
   ```rust
   // Old (0.7.1): Non-standard inquiry available
   camera.tally_status()?;  // PTZOptics-specific

   // New (0.8.0): Baseline VISCA only
   camera.tally_red()?;     // Standard VISCA
   camera.tally_green()?;   // FR7 extension
   ```
   **Impact**: If using `tally_status()`, migrate to `tally_red()` and `tally_green()` for portable code.

#### Transport Handle (Minor)
If you were pattern matching on `TransportHandle`, add the new `Serial` variant:

```rust
// Old (0.7.1)
match transport {
    TransportHandle::Udp(t) => { /* ... */ }
    TransportHandle::Tcp(t) => { /* ... */ }
}

// New (0.8.0)
match transport {
    TransportHandle::Udp(t) => { /* ... */ }
    TransportHandle::Tcp(t) => { /* ... */ }
    #[cfg(any(feature = "transport-serial", feature = "transport-serial-tokio"))]
    TransportHandle::Serial(t) => { /* ... */ }
}
```

#### Error Matching (Minor)
`ErrorKind` is now `#[non_exhaustive]`, so wildcard patterns are required:

```rust
// Old (0.7.1): Could exhaustively match
match error.kind() {
    ErrorKind::Timeout => { /* ... */ }
    ErrorKind::Cancelled => { /* ... */ }
    // Could list all variants
}

// New (0.8.0): Must include wildcard
match error.kind() {
    ErrorKind::Timeout => { /* ... */ }
    ErrorKind::Cancelled => { /* ... */ }
    _ => { /* ... */ }  // Required
}
```

#### Import Organization (Minor)
Module imports have been reorganized for consistency:

```rust
// Some internal module structures changed
// Public API exports remain stable
// If using deep imports, verify import paths
```

### ✅ Quick Migration Checklist

**Follow these steps to upgrade from 0.7.1 to 0.8.0:**

#### Step 1: Update Dependencies (Required)
```toml
[dependencies]
# Update version
grafton-visca = "0.8"

# Optional: Enable new serialization features
grafton-visca = { version = "0.8", features = ["serde", "schemars"] }
```

#### Step 2: Fix Breaking Changes (If Applicable)

**Only if you use AWB sensitivity inquiry:**
```rust
// Old (0.7.1): Values were inverted
let sens = camera.auto_wb_sensitivity()?;
// If you had compensation logic, REMOVE it

// New (0.8.0): Values are correct
let sens = camera.auto_wb_sensitivity()?;
// Now works correctly without compensation
```

**Only if you use tally status methods:**
```rust
// Old (0.7.1): Vendor-specific
camera.tally_status()?;

// New (0.8.0): Baseline VISCA
camera.tally_red()?;     // Standard
camera.tally_green()?;   // FR7 extension
```

**Only if you pattern-match TransportHandle:**
```rust
// Add Serial variant to your match:
match transport {
    TransportHandle::Udp(t) => { /* ... */ }
    TransportHandle::Tcp(t) => { /* ... */ }
    #[cfg(feature = "transport-serial")]
    TransportHandle::Serial(t) => { /* ... */ }  // ADD THIS
}
```

**Only if you exhaustively match ErrorKind:**
```rust
// Add wildcard to your match:
match error.kind() {
    ErrorKind::Timeout => { /* ... */ }
    _ => { /* ... */ }  // ADD THIS (ErrorKind is now #[non_exhaustive])
}
```

#### Step 3: Test Your Application
```bash
# Run your tests
cargo test

# Run clippy to catch any issues
cargo clippy
```

#### Step 4: Adopt New Features (Optional)

**Enable serialization for your types:**
```toml
[dependencies]
grafton-visca = { version = "0.8", features = ["serde"] }
```

**Use new convenience methods:**
```rust
// Direct f64 usage (no more casts!)
camera.pan_tilt_absolute(Degrees(45.0), Degrees(15.0), SpeedLevel::Fast)?;

// Coarse speed mapping
let speed = ZoomSpeed::from_coarse(Coarse::Medium);

// Direct inquiry conversions
let pos = camera.pan_tilt_position()?;
let (pan_deg, tilt_deg) = pos.as_degrees();  // Built-in conversion!

// Diagnostics
use grafton_visca::diagnostics::Diagnostics;
let healthy = camera.ping().await?;
```

**Use profile dispatch (eliminates boilerplate):**
```rust
use grafton_visca::camera::profiles::ProfileGroup;

// Old (0.7.1): ~218 lines of custom dispatch code
// ...custom ProfileGroup trait and implementations...

// New (0.8.0): Built-in!
let group = profile_id.profile_group();
match group {
    ProfileGroup::GenericVisca => { /* ... */ }
    ProfileGroup::PtzOpticsG2 => { /* ... */ }
    ProfileGroup::SonyProfessional => { /* ... */ }
}
```

### 🔍 Real-World Migration Examples

These examples show actual code changes (or lack thereof) when migrating from 0.7.1 to 0.8.0:

#### Example 1: Basic Camera Control (NO CHANGES NEEDED ✅)

```rust
// This code works in BOTH 0.7.1 and 0.8.0 without changes!

use grafton_visca::{
    camera::{profiles::PtzOpticsG2, Connect},
    types::SpeedLevel,
    units::{Degrees, Normalized},
    Error,
};

fn main() -> Result<(), Error> {
    let camera = Connect::open_tcp_blocking::<PtzOpticsG2>("192.168.0.110")?;

    camera.pan_tilt_home()?;
    camera.zoom_absolute(Normalized(0.5))?;
    camera.preset_recall(PresetNumber::new(1)?)?;

    Ok(())
}
// ✅ Works in 0.7.1 and 0.8.0 identically
```

#### Example 2: Better Ergonomics in 0.8.0 (OPTIONAL IMPROVEMENTS 🎯)

```rust
// 0.7.1: Manual casting required
camera.pan_tilt_absolute(
    Degrees(45.0 as f32),  // 👈 Manual cast needed
    Degrees(15.0 as f32),  // 👈 Manual cast needed
    SpeedLevel::Fast
)?;

// 0.8.0: Direct f64 usage (still supports old code too!)
camera.pan_tilt_absolute(
    Degrees(45.0),  // ✨ No cast needed!
    Degrees(15.0),  // ✨ No cast needed!
    SpeedLevel::Fast
)?;
```

```rust
// 0.7.1: Custom speed mapping in your code
let zoom_speed = match user_pref {
    UserSpeed::Slow => ZoomSpeed::new(2)?,
    UserSpeed::Medium => ZoomSpeed::new(4)?,
    UserSpeed::Fast => ZoomSpeed::new(6)?,
};

// 0.8.0: Built-in canonical mapping
let zoom_speed = ZoomSpeed::from_coarse(Coarse::Medium);  // → 4
```

#### Example 3: Inquiry with Conversions (NEW BUILT-IN HELPERS 🚀)

```rust
// 0.7.1: Manual conversion math
let pos = camera.pan_tilt_position()?;
let pan_degrees = (pos.pan as f32) * 170.0 / 2448.0;
let tilt_degrees = /* complex asymmetric calculation */;

// 0.8.0: Built-in conversion methods
let pos = camera.pan_tilt_position()?;
let (pan_deg, tilt_deg) = pos.as_degrees();  // ✨ Direct conversion!
println!("Pan: {:.1}°, Tilt: {:.1}°", pan_deg.0, tilt_deg.0);
```

```rust
// 0.7.1: Manual normalization with constants
let zoom = camera.zoom_position()?;
let normalized = (zoom.value() as f32) / 0x4000 as f32;

// 0.8.0: Domain-aware normalization
let zoom = camera.zoom_position()?;
let optical_norm = zoom.normalized_optical();      // ✨ 0.0-1.0 in optical range
let combined_norm = zoom.normalized_combined();    // ✨ 0.0-1.0 in full range
```

#### Example 4: Serialization (NEW OPTIONAL FEATURE ✨)

```rust
// 0.7.1: Had to create wrapper types for serialization
#[derive(Serialize, Deserialize)]
pub struct CameraPosition {
    pan: f64,      // Custom wrapper
    tilt: f64,     // Custom wrapper
    zoom: f64,     // Custom wrapper
}

impl From<ViscaPosition> for CameraPosition {
    fn from(pos: ViscaPosition) -> Self {
        // ~50 lines of conversion code...
    }
}

// 0.8.0: Direct serialization with serde feature
use grafton_visca::types::{PanPosition, TiltPosition, ZoomPosition};

#[derive(Serialize, Deserialize)]  // ✨ Works directly!
pub struct CameraPosition {
    pan: PanPosition,     // Serializes directly
    tilt: TiltPosition,   // Serializes directly
    zoom: ZoomPosition,   // Serializes directly
}
// No conversion code needed!
```

#### Example 5: Error Handling (IMPROVED IN 0.8.0 🛡️)

```rust
// 0.7.1: Basic error handling
camera.power_on()?;  // Generic error message

// 0.8.0: Rich error context (new feature)
camera.power_on()
    .context("Failed to power on camera for recording session")?;
// Error: "Failed to power on camera for recording session: CommandTimeout"

// Retry intelligence still preserved!
match result {
    Err(e) if e.is_retryable() => {
        sleep(e.suggested_retry_delay());
        retry()?;
    }
    _ => {}
}
```

#### Example 6: Async Mode (ENHANCED IN 0.8.0 ⚡)

```rust
// 0.7.1: Basic async operations
#[tokio::main]
async fn main() -> Result<(), Error> {
    use grafton_visca::runtime::TokioRuntime;

    let runtime = TokioRuntime::from_current()?;
    let camera = Connect::open_tcp_async::<PtzOpticsG2, _>(
        "192.168.0.110",
        runtime
    ).await?;

    camera.power().on().await?;
    camera.zoom().tele().await?;

    Ok(())
}
// ✅ Still works the same in 0.8.0!

// 0.8.0: NEW - Typed operation handles for better control
let handle = camera.pan_tilt_absolute_op(
    Degrees(45.0),
    Degrees(15.0),
    SpeedLevel::Fast
).await?;

// Can cancel with type safety
handle.cancel().await?;

// Or wait for completion
handle.await_completion(Duration::from_secs(5)).await?;
```

### 🎓 Detailed Migration Strategy

**To adopt new features:**

1. **Opt into serialization**:
   ```toml
   grafton-visca = { version = "0.8", features = ["serde", "schemars"] }
   ```

2. **Update protocol-dependent code**:
   - Verify AWB sensitivity inquiry handling (fix removes value inversion)
   - Replace `tally_status()` with `tally_red()` and `tally_green()` for VISCA compliance
   - Review any code that pattern matches on `TransportHandle`

3. **Adopt new convenience features**:
   - Replace custom conversion code with built-in helpers from `inquiry_conversions`
   - Replace custom speed mapping with `Coarse` and `from_coarse()` methods
   - Replace custom health checks with the `Diagnostics` trait (now in prelude)
   - Use `stop_all_motion()` for emergency stops instead of multiple stop calls
   - Use `_op` variants for type-safe operation lifecycle management (async mode only)

4. **Improve code ergonomics**:
   - Remove f32 casts when constructing `Degrees` and similar types (now accepts f64 directly)
   - Use `color_temperature_direct()` for precise white balance control
   - Leverage `Error::clone()` in multi-threaded error handling

5. **Enable serial transport** (if needed):
   ```toml
   # Blocking mode
   grafton-visca = { version = "0.8", features = ["transport-serial"] }

   # Async with Tokio
   grafton-visca = { version = "0.8", features = ["transport-serial-tokio"] }
   ```

6. **Use built-in cancellation** via `start_command_with_id()` and `cancel()` for long-running operations

### 📚 Technical Improvements

- **Runtime-agnostic modernization complete** (#416): Full async runtime support for tokio, async-std, and smol with runtime-neutral abstractions
- **Typed operation handles** (#425): Zero-cost `InFlight<C>` handles for socket-safe cancellation and type-directed completion waits
- **Protocol correctness validated** (#418): All protocol implementations verified against VISCA specification with comprehensive test coverage
- **Uniform transport architecture** (#415): Serial transport integrated into `TransportHandle` for consistent trait implementations across all transport types
- **Connection timeout enforcement** (#417): Async connectors properly enforce `connect_timeout` with non-blocking DNS resolution across all runtimes
- **Zero-cost abstractions**: Type-safe wrappers with no runtime overhead; ZST markers for operation categories
- **Consistent validation**: All types expose MIN/MAX constants and validated constructors
- **Non-exhaustive enums**: Future-proof API with `#[non_exhaustive]` on key enums
- **Enhanced error ergonomics**: `Error` type now implements `Clone` for better composability (#414)
- **Comprehensive testing**: Golden vectors for conversions, domain normalization, protocol correctness, and edge cases
- **Improved documentation**: All new types include examples and usage notes; examples updated to demonstrate 0.8.0 features
- **Import organization**: Standardized import structure and removed extraneous whitespace for consistency
- **Dependency updates**: Upgraded schemars to 1.0 for ecosystem compatibility

### Performance

#### Zero-Allocation Send Path (#421)
Eliminated all heap allocations in the hot send path for VISCA commands, achieving true zero-cost abstractions:

**Changes:**
- **Inline command storage**: Replaced `PreparedCommand` with `EncodedCommand` using `SmallVec<[u8; 24]>` for stack-allocated command bytes (heap-free for 99%+ of commands)
- **In-place framing**: Added `Envelope::frame_into()` method that writes directly into a reusable buffer, replacing `frame_bytes()` that returned owned `Bytes`
- **Reusable send buffer**: Runtime loops now allocate a single send buffer once at startup and reuse it across all send operations

**Impact:**
- **Before**: 2 heap allocations per command (encode + frame) + 1 extra copy
- **After**: 0 heap allocations for commands ≤24 bytes (covers all standard VISCA commands)
- Applies to all send paths: normal commands, retries, and cancel operations

**Breaking changes:**
- `PreparedCommand` renamed to `EncodedCommand` (type alias provided for migration)
- `PreparedCommand::payload` changed from `Bytes` to `SmallVec<[u8; 24]>` (internal field, not public API)
- Added `EncodedCommand::as_slice()` method for zero-copy access to encoded bytes
- `Envelope::frame_bytes()` and `Envelope::frame_bytes_with_meta()` deprecated in favor of `frame_into()`

**Migration:**
```rust
// Old (deprecated):
let payload = cmd.to_bytes(camera_id)?;
let framed = envelope.frame_bytes(&payload, kind);
transport.send(&framed).await?;

// New (zero-allocation):
let mut send_buf = BytesMut::with_capacity(buffer_size);
let encoded = EncodedCommand::new(cmd, camera_id)?;
envelope.frame_into(encoded.as_slice(), kind, &mut send_buf);
transport.send(&send_buf[..]).await?;
```

### 🔮 Future Direction

Version 0.8.0 represents a major API evolution before 1.0. The focus has shifted from architectural changes to stability, robustness, and ergonomics. The runtime-agnostic foundation is complete, serialization support is in place, and the API surface is clean and minimal. Upcoming releases will focus on:

- Stability and bug fixes toward 1.0

## [0.7.1] - 2025-10-10

### Added
- Serial transport support with `TransportHandle::Serial` variant for uniform trait implementation
- Warning messages for unsupported hardware configurations

### Changed
- Improved `Error` type with `Clone` implementation for better error handling
- Enhanced runtime-agnostic spawn background abstraction with `spawn_with_detach` function
- Standardized import organization and code formatting across the codebase
- Improved layout consistency

### Fixed
- Runtime-agnostic background task spawning now properly handles detached tasks

## [0.7.0] - 2025-09-18

This release represents a complete architectural transformation of the library, fundamentally reimagining how VISCA camera control should work in Rust. After hundreds of iterations and refinements since 0.6.0, we've achieved a design that prioritizes simplicity, type safety, and zero-cost abstractions.

### 🎯 Philosophy: Camera-First API Design

The central breakthrough in 0.7.0 is the **camera-first** approach. Instead of exposing protocol details, transport layers, or complex builders, the API now starts with what matters: the camera itself. This seemingly simple change cascades through the entire architecture, eliminating complexity at every level.

```rust
// The entire connection story in one line
let mut camera = Connect::open_tcp_blocking::<PtzOpticsG2>("192.168.0.110")?;

// Direct, intuitive control
camera.power_on()?;
camera.zoom_in()?;
camera.pan_tilt_home()?;
```

### 🔧 Core Architectural Changes

#### Unified Type System
- Single `Camera<Profile>` type replaces complex generic hierarchies
- `CameraSession` provides the actual connection and communication
- Profile system enables compile-time validation without runtime overhead
- Transport details completely hidden from public API

#### Zero-Allocation Command Pipeline
- Commands encoded directly into fixed-size arrays
- No heap allocations in the hot path
- Compile-time size calculation for all VISCA messages
- Protocol framing happens at the last possible moment

#### Runtime-Agnostic Architecture
- Blocking is the baseline no-async build; async mode is selected with `mode-async`
- No runtime required for blocking mode
- Multiple async runtimes supported (tokio, async-std, smol) with automatic selection
- Executor abstraction allows runtime switching without code changes

#### Type-State Session Management
- `CameraSession` uses type states (Open/Closed) to prevent use-after-close bugs
- RAII pattern ensures proper resource cleanup
- Connection lifecycle managed automatically
- Compile-time guarantees for session validity

### 📝 API Surface Consolidation

The public API has been dramatically simplified while maintaining full VISCA protocol support:

#### Before (0.6.0)
- Multiple client types (`Client`, `AsyncClient`, `ViscaClient`)
- Exposed transport traits and implementations
- Complex builder patterns with many configuration options
- Protocol details leaked into user code

#### After (0.7.0)
- Single `Camera` type with profile parameter
- One-line connection methods via `Connect` trait
- Transport and protocol completely abstracted
- Clean separation between camera control and infrastructure

### 🚀 Major Features & Improvements

#### Connection Simplicity (#403, #404)
- `Connect` trait provides simple `open_tcp_*` and `open_udp_*` methods
- Auto-detection of VISCA protocol variant (Sony vs Generic)
- IPv6 support with automatic address normalization (#402)
- DNS resolution handled transparently

#### Command Architecture (#400, #405)
- Exact-size VISCA encoding with zero allocations
- Compile-time protocol envelope construction
- Sony IP sequence number allocation fused into framing
- Separate inquiry pipeline to prevent head-of-line blocking (#393)

#### Profile System Enhancement
- Camera profiles now define all model-specific constants
- Compile-time validation of parameters against camera capabilities
- Automatic unit conversions based on camera model
- Profile-aware timeout configurations

#### Unified Scheduler (#392)
- Deadline-driven scheduler replaces fixed tick loop
- Commands and inquiries share same scheduling infrastructure
- Automatic retry handling with exponential backoff
- Per-category timeout configuration

### 🔄 Breaking Changes from 0.6.0

Due to the complete architectural overhaul, this release includes extensive breaking changes. Rather than listing each change individually (there are hundreds), here's how to think about migrating:

#### Connection & Setup
```rust
// Old (0.6.0)
let transport = TcpTransport::new("192.168.0.110:52381")?;
let camera = CameraBuilder::new()
    .with_transport(transport)
    .profile::<PtzOpticsG2>()
    .build()?;

// New (0.7.0)
let mut camera = Connect::open_tcp_blocking::<PtzOpticsG2>("192.168.0.110")?;
```

#### Feature Flags
```rust
// Old: Complex feature matrix
[features]
default = ["async", "tokio", "tcp", "udp"]

// New: Simple mode selection
[features]
default = []  # blocking baseline; enable async/runtime features for async builds
```

#### API Access
```rust
// Old: Traits scattered across modules
use grafton_visca::{ViscaZoomExt, ViscaPanTiltExt, ViscaFocusExt};

// New: Everything through Camera methods
use grafton_visca::{Camera, camera::profiles::PtzOpticsG2, camera::Connect};
// All methods available directly on camera instance
```

#### Command Execution
```rust
// Old: Complex command building
let cmd = ZoomCommand::Direct(ZoomPosition::new(0x4000)?);
camera.send(&cmd)?;

// New: Direct methods
camera.zoom_to(0x4000)?;
// Or with units
camera.zoom_to(Normalized::new(0.5)?)?;
```

### 🎓 Migration Strategy

Given the extensive changes, we recommend:

1. **Start Fresh**: Rather than trying to update existing code incrementally, consider rewriting camera control logic using the new API
2. **Use Examples**: The examples in `/examples` demonstrate all common patterns
3. **Leverage Type Safety**: Let the compiler guide you - most old patterns simply won't compile
4. **Simplify**: The new API requires significantly less code - embrace the simplicity

### 📚 Technical Improvements

- **Macro System** (#397, #399): Complete consolidation of internal macros, cleaner organization
- **Naming Consistency** (#396, #398): All types and methods follow consistent naming patterns
- **Protocol Correctness** (#389, #395): Strict VISCA compliance with proper terminator handling
- **Test Infrastructure** (#394): Deterministic executor for reliable async testing
- **Transport Unification** (#381, #387): Blocking and async transports share core logic
- **Error Handling** (#382): Consistent error semantics across all transport types

### 🔮 Future Direction

This release establishes a stable foundation for the 1.0 release. The camera-first API design, combined with zero-cost abstractions and compile-time safety, provides the ideal balance of simplicity and power for VISCA camera control in Rust.

## [0.6.1] - 2025-01-12

### Internal
- Pre-release version with initial architectural improvements
- Foundation for v0.7.0 release

## [0.6.0] - 2025-01-07

### Added

#### 🛡️ Type-Safe VISCA Terminator Pattern (#203)
- Implemented type-state pattern for VISCA terminator safety
- Added `CommandBuilder` for safe command construction with automatic terminator handling
- Consolidated all command constants to use `VISCA_TERMINATOR` constant
- Prevents protocol violations at compile time

#### ⏱️ Timeout System Enhancements (#202, #204)
- Moved timeout categories to type system constants
- Profile-specific timeout configurations for different camera models
- Compile-time timeout validation

#### 🎯 Event-Driven Movement Detection
- New event-driven system for camera movement detection
- Eliminates polling delays in movement completion detection
- More responsive and efficient movement tracking

#### 🔧 Code Quality Improvements (#193)
- Resolved all Clippy warnings including uninlined format args
- Fixed async feature compilation without tokio
- Eliminated sleep anti-patterns in examples (#191)
- Improved code formatting and documentation

### Fixed
- Corrected README path for crates.io publishing
- Fixed Clippy warnings for async feature without tokio dependency
- Resolved all CI workflow quality check failures

### Internal
- Applied comprehensive code formatting improvements
- Enhanced test coverage for new type-safe patterns
- Improved example code quality and best practices

## [0.5.0] - 2025-01-06

### Breaking Changes

#### 🔥 New CameraBuilder API (Issue #185)
- **BREAKING**: Removed all `connect_*` functions (`connect_tcp`, `connect_udp`, `connect_tokio_tcp`, `connect_tokio_udp`)
- **NEW**: Introduced `CameraBuilder` for a cleaner, more idiomatic API:
  ```rust
  // Before (removed):
  let camera = Camera::<PTZOpticsG2, _>::connect_tcp("192.168.0.110")?;

  // After (new):
  let camera = CameraBuilder::tcp("192.168.0.110")
      .profile::<PTZOpticsG2>()
      .build()?;
  ```
- Benefits of the new API:
  - Runtime parameters (address, protocol) come first
  - Compile-time profile selection comes second
  - Single entry point (`CameraBuilder`) for all transport types
  - More extensible for future transport options

### Changed
- Updated all examples to use the new `CameraBuilder` API
- Updated README and documentation with new builder pattern examples

## [0.4.0] - 2024-12-27

This release represents a major evolution of the library from a low-level VISCA protocol implementation to a high-level camera control solution with a unified, ergonomic API.

### Changed

#### 📝 Documentation Updates
- Toned down overstated claims in README to better reflect development status
- Added development status warning to README
- Adjusted feature claims to be more accurate and modest
- Clarified that test coverage is being expanded rather than complete
- Removed performance optimization claims pending benchmarking

### Added

#### 🎯 Unified Client Architecture
- New unified `Client` that works seamlessly in both sync and async contexts
- Thread-safe and `Clone`able client - share it freely across your application
- Automatic context detection - the client adapts to your code style
- Connection pooling built-in
- Automatic reconnection with configurable retry strategies
- Camera model configuration via `Client::builder()` for automatic command validation

#### 🔄 Resilience Features
- `ReconnectingTransport` with exponential backoff and configurable retries
- `ConnectionPool` for managing multiple cameras efficiently
- Health check system with automatic recovery
- Detailed error types that indicate retry-ability

#### 🎮 Camera Model Validation
- Optional camera model configuration to prevent invalid commands before sending
- Model-specific validation for zoom ranges (20X vs 30X cameras)
- Model-specific validation for pan/tilt absolute positions
- New `ModelValidation` error type for clear validation failure messages
- Maintains backward compatibility - validation is opt-in via `Client::builder()`
- Timeout management with per-command category timeouts

#### 🎨 High-Level Extension Traits
- `ViscaPowerExt`, `ViscaZoomExt`, `ViscaFocusExt`, etc. for domain-specific operations
- `PTZBuilder` for complex camera movements with intuitive units:
  - Degrees for pan/tilt (`pan_to_degrees(45.0)`)
  - Magnification for zoom (`zoom_to_magnification(10.0)`)
  - Percentages for positioning (`set_pan_tilt_percentage(0.5, -0.25)`)
- White balance fine-tuning methods (`white_balance_red_tuning()`, `white_balance_blue_tuning()`)
- Anti-flicker control (`set_anti_flicker()`)
- Sequential and concurrent command execution modes

#### 🚦 Enhanced Error Handling
- Specific error types for different failure modes
- `is_retryable()` method on errors
- `suggested_retry_delay()` for intelligent retry logic
- Context-aware errors with parameter ranges
- Removed `#[non_exhaustive]` for exhaustive error matching

#### 📚 Developer Experience
- Comprehensive prelude module (`use grafton_visca::prelude::*`)
- 30+ real-world examples demonstrating common use cases
- Complete API documentation with examples
- Zero clippy warnings (even on pedantic level)
- Simplified feature flags - just works out of the box

#### 🔧 Procedural Macros (grafton-visca-macros)
- Implemented `#[visca_command_variants]` for generating multiple method variants accepting different input types (raw values, typed wrappers, percentages, etc.)
- Implemented `#[visca_inquiry]` for automatic response parsing based on command type
- Added `#[visca_position_command]` for position-based commands with automatic validation and unit conversions (degrees, normalized values)
- Added `#[visca_speed_command]` for speed-based commands with SpeedLevel enum support
- Added `#[visca_bounded_command]` for bounded value commands with percentage variants and named level enums

### Changed

#### **BREAKING**: Complete API Overhaul
- **Core Type Renames** (cleaner, more idiomatic):
  - `ViscaClient` → `Client`
  - `ViscaError` → `Error`
  - `ViscaSession` → `Session`
  - `ViscaResponse` → `Response`
  - `ViscaCommand` → `Command` trait
  - `ViscaTransport` → `ViscaProtocol` struct

- **Command Naming Improvements**:
  - **Zoom**: `Tele/Wide` terminology → `ZoomIn/ZoomOut` throughout
    - `ZoomCommand::TeleStandard` → `ZoomCommand::ZoomInStandard`
    - `ZoomCommand::WideStandard` → `ZoomCommand::ZoomOutStandard`
    - Extension methods: `zoom_in_variable()` → `zoom_in_speed()`
  - **Focus**: Added clarity with prefixes
    - `FarStandard` → `FocusFarStandard`
    - `AFSensitivity` → `AutoFocusSensitivity`
  - **Presets**: Aligned with VISCA specification
    - `save_preset()` → `set_preset()`
    - `goto_preset()` → `recall_preset()`
  - **White Balance**: Simplified method names
    - `set_color_temperature_direct()` → `set_color_temperature()`

- **Transport Layer Evolution**:
  - Async-first design with blocking adapters
  - Unified transport abstraction across TCP/UDP
  - Built-in connection pooling and reconnection (no more feature flags)

- **Feature Flag Simplification**:
  - Removed complex feature matrix
  - Default is blocking client
  - Single `async` feature for async runtime
  - Connection pooling and reconnection are now standard

### Removed
- All deprecated type aliases from previous versions
- Duplicate convenience methods that didn't match VISCA spec
- Old split client implementations (`ViscaClient` vs `AsyncViscaClient`)
- Complex feature flag requirements for basic functionality
- Direct `power_on()`/`power_off()` methods (use `PowerCommand` instead)
- Orphaned async extension files from earlier refactoring

### Internal
- **Macro Consolidation** (Issue #201): Consolidated all macros into a single module hierarchy at `src/macros/` with clear separation between public API macros, internal implementation macros, and test utilities. Removed `#[macro_export]` from internal macros to prevent namespace pollution.

### Fixed
- Thread safety issues - client is now truly thread-safe without `RefCell`
- Feature gating problems with `no-default-features` builds
- All clippy warnings including pedantic lints
- Protocol edge cases in response handling
- Missing functionality in unified client (`try_send()`, `send_with_timeout()`)
- Example files now have proper feature requirements

### Performance Improvements
- Const functions used where possible
- Reduced allocations in command building
- More efficient response parsing
- Better memory usage patterns

## Migration Guide from v0.3.0

### Step 1: Update Your Cargo.toml
```toml
# Old
[dependencies]
grafton-visca = { version = "0.3", features = ["async", "sync", "reconnect", "pool"] }

# New - Blocking by default
grafton-visca = "0.4"

# OR for async
grafton-visca = { version = "0.4", default-features = false, features = ["async"] }
```

### Step 2: Update Imports
```rust
// Old
use grafton_visca::{ViscaClient, ViscaError, ViscaResponse, ViscaSession};
use grafton_visca::command::ViscaCommand;

// New
use grafton_visca::{Client, Error, Response, Session};
use grafton_visca::command::Command;

// Or use the prelude for common imports
use grafton_visca::prelude::*;
```

### Step 3: Update Client Creation
```rust
// Old - had to choose between sync and async
let client = ViscaClient::new(transport);
let client = AsyncViscaClient::new(async_transport);

// New - unified client works everywhere
let client = Client::connect_tcp("192.168.0.110")?;
// Use the same client in both sync and async code!
```

### Step 4: Update Method Calls
```rust
// Old power control
client.power_on()?;
client.power_off()?;

// New - use PowerCommand directly
use grafton_visca::command::{PowerCommand, power::Power};
client.send(&PowerCommand::new(Power::On))?;
client.send(&PowerCommand::new(Power::Standby))?;

// Or use extension trait
use grafton_visca::ViscaPowerExt;
client.set_power(Power::On)?;

// Old zoom methods (removed)
client.zoom_in()?;
client.zoom_out()?;

// New - use extension trait methods
use grafton_visca::ViscaZoomExt;
client.zoom_in()?;  // Standard speed
client.zoom_in_speed(Some(ZoomSpeed::new(5)?))?;  // Variable speed

// Old preset names
client.save_preset(1)?;
client.goto_preset(1)?;

// New - matches VISCA specification
client.set_preset(1)?;
client.recall_preset(1)?;
```

### Step 5: Update Type Names in Your Code
```rust
// Old
fn connect_camera(addr: &str) -> Result<ViscaClient, ViscaError> {
    ViscaClient::connect_udp(addr)
}

fn handle_response(resp: ViscaResponse) -> Result<(), ViscaError> {
    // ...
}

// New
fn connect_camera(addr: &str) -> Result<Client, Error> {
    Client::connect_udp(addr)
}

fn handle_response(resp: Response) -> Result<(), Error> {
    // ...
}
```

### Step 6: Use High-Level APIs
```rust
// Old - manual VISCA units
let pan_pos = 0x0800;  // What does this mean?
let tilt_pos = 0x0000;
client.send_command(PanTiltAbsolute::new(pan_pos, tilt_pos))?;

// New - intuitive units with PTZ builder
client.ptz()
    .pan_tilt_to(45.0, -15.0)  // Degrees!
    .zoom_to_magnification(5.0)  // 5x zoom
    .wait()  // Execute sequentially
    .execute()?;
```

### Step 7: Update Error Handling
```rust
// Old - generic errors
match result {
    Err(e) => eprintln!("Error: {}", e),
    Ok(_) => {}
}

// New - specific, actionable errors
match result {
    Err(e) if e.is_retryable() => {
        sleep(e.suggested_retry_delay());
        retry()?;
    }
    Err(Error::CameraMoving) => {
        client.wait_for_completion()?;
    }
    Err(Error::OutOfRange { param, min, max }) => {
        println!("{} must be between {} and {}", param, min, max);
    }
    _ => {}
}
```

## [0.3.0] - 2025-01-06

### Added
- **Production-Ready Features**: The library is now production-ready with >90% test coverage
- **Complete Documentation**: All public APIs now have comprehensive documentation with examples
- **Unit Tests**: Added extensive unit tests for core modules including:
  - Error handling (ViscaError, AppError)
  - Session management (ViscaSession)
  - Command encoding (pan/tilt commands with validation)
- **Demo Application**: Added `demo.rs` example showcasing all library features
- **Documentation Improvements**:
  - Added trait-level documentation for `ViscaProtocol` and `ViscaCommand`
  - Added comprehensive function documentation for `send_command_and_wait`
  - Added struct-level documentation for transport types
  - Added enum documentation for response types

### Changed
- **README Updates**: Updated to reflect production readiness, removed WIP warnings
- **Documentation Examples**: Fixed async example code to prevent lifetime issues

### Fixed
- **Test Compilation**: Fixed duplicate test modules and missing methods
- **Clippy Warnings**: Resolved redundant closure warnings in async modules
- **Example Code**: Fixed command usage in async_concurrent example

### Sprint 4 Completion
This release completes Sprint 4 of the production readiness roadmap:
- ✅ Comprehensive unit test coverage
- ✅ Complete Rustdoc documentation
- ✅ All code quality checks passing (fmt, clippy)
- ✅ Demo application showcasing features
- ✅ README updated for production use

## [0.2.2] - Previous Release

### Sprint 3 - Async Support
- Added full async/await support with `AsyncViscaClient`
- Implemented concurrent command execution with automatic socket management
- Added background response handling with proper state machine
- Thread-safe design allowing client to be cloned and shared

### Sprint 2 - Protocol Improvements
- Correct ACK/Completion response handling
- Proper socket management for VISCA's two-socket limitation
- Error response classification with specific error types

### Sprint 1 - Command Coverage
- Implemented all missing VISCA commands for PTZOptics G2
- Added exposure control commands (iris, shutter, gain, etc.)
- Added color adjustment commands (saturation, hue, white balance tuning)
- Added advanced PTZ commands (absolute/relative positioning)
- Added inquiry commands for all new features

## [0.1.0] - Initial Release

- Basic VISCA over IP implementation
- Core PTZ control commands
- UDP and TCP transport support
