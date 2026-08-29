# 1.x behavioral parity gate

2.0 is a clean API and architecture break. That means callers do not receive
1.x compatibility shims, deprecated aliases, or the old scheduler surface. It
does not mean that a battle-tested protocol behavior can be silently
re-authored. The parity gate separates those two ideas:

- *Source-code reuse* means retaining an implementation or helper from 1.x.
  The 2.0 design deliberately does not require this; the owner and engine have
  new boundaries.
- *Behavior reuse* means retaining an observable contract such as retry budget,
  response attribution, wire bytes, or drop/detach semantics. This is what the
  gate records and exercises.

## The pinned oracle

The versioned corpus is [`tests/fixtures/1x_oracle/manifest.json`](../tests/fixtures/1x_oracle/manifest.json).
Every row pins the complete 1.x commit
`6c7a9d3783861189745536372c4d21de24d4252d`, names the old source file and test
symbol, cites the applicable [issue #542](https://github.com/GrantSparks/grafton-visca/issues/542)
contract, and points to the v2 production-path test(s) or fixture(s) that
exercise the behavior. Each v2 row also records the transport `envelope`
(`sony`, `raw`, or `neutral`), the camera `profile` it drives (a concrete type
such as `SonyFR7`, or `n/a` for engine-level and pure encode/decode tests), and
the terminal `receipt_class` it asserts.

The validator is enforcing, not advisory. It checks that every referenced
symbol still exists; that the recorded envelope/profile match the mapped test's
own body (a `Sony*` profile must appear in the body and pair with envelope
`sony`; a row declared `raw` must not run a Sony-only test), so an
evidence-laundering raw→Sony swap under an unchanged name is a visible, checked
diff instead of a silent re-baseline; and — when it runs the mapped `cargo test`
commands — that each mapped symbol *actually executed*. A libtest filter that
matches nothing exits `0` after printing `running 0 tests`, and an `#[ignore]`d
test prints `... ignored` while the binary still exits `0`, so the gate parses
the libtest output and fails unless every mapped symbol appears as an executed
(`ok`) test. Renaming an enclosing `mod tests`, filtering to zero of the mapped
symbols, or ignoring one all fail loudly instead of reducing coverage silently.

Run it with:

```text
bash .github/scripts/validate-behavioral-parity.sh
```

The dedicated CI job checks out full Git history because `git show` must be able
to read the pinned oracle object. A shallow checkout produces a deterministic
failure explaining the required `fetch-depth: 0`. The gate is repository/CI
machinery and is not required by a crates.io consumer.

The current corpus covers timeout category defaults and selection; retry counts,
bounded exhaustion, and evidence-aware recovery; exact/unique/colliding
lower-16 sequence handling; stale sequenced completion, error, and inquiry
inertness; raw exact-socket and unique-candidate routing; compatible inquiry
FIFO; datagram isolation; stream poison; byte-stream malformed-frame tolerance;
envelope-specific receive faults; command wire bytes; inquiry decoding and
golden replies; blocking out-of-order receipt retention; and cancellation,
detach, and late observer delivery.

The `malformed-frame-tolerance` family (#672) pins that a delimited-but-
unclassifiable VISCA response is discarded and logged rather than fatal. 1.x
reported such a frame as `ReceiveDisposition::Malformed` and logged-and-continued
in both runners without disturbing any pending command; the 2.0 rewrite briefly
regressed this on byte streams, where the strict decoder's rejection became a
framing poison that killed the whole session. The v2 production replay drives the
real owner over a stream and asserts every quirky-but-delimited shape (padded
ACKs, vendor socket nibbles, RS-485 echoes, controller/broadcast lead bytes,
address-set replies, truncated frames, stray terminator bytes) is discarded as
`Ignored(MalformedFrame)` while the session stays `Running` and the command is
still settled by the next well-formed reply. A genuine loss of the framing
position (buffer overflow, or a boundary-free read past `max_buffer_size`) still
poisons, and that terminal case is preserved.

## Built-in request policy audit

The request-policy audit is exhaustive but does not maintain a second semantic
registry. It reads the 149 rows from `BuiltinCommand::ALL`, resolves each row
through the mechanically checked `BUILTIN_TYPED_REQUEST_INVENTORY`, and reads
the `TimeoutClass`/`RetryClass` constants from the concrete `Request` macro
invocations. The generated inquiry table contains 68 queryable request rows
and 11 decode-only response rows; every queryable row uses the generated
`Inquiry`/`Inquiry` policy. The pinned 1.x command declarations are the
comparison source (`src/command/*.rs` `TIMEOUT_CATEGORY`/`category` entries),
and the pinned inquiry table has no per-row timeout override, so every old
built-in inquiry used the 1.x quick-category default.

The rows with an intentional timeout-category decision are:

| v2 rows | 1.x category | v2 policy | Decision |
| --- | --- | --- | --- |
| `PanTiltStop`, `ZoomStop`, `FocusStop` | Movement | Quick / Movement | Urgent stop deadline; retain movement retry/error semantics. |
| `PanTiltLimitSet`, `PanTiltLimitClear` | Movement | Quick / Standard | Plain limit-state edits are not actuation. |
| `FocusAuto`, `FocusManual`, `FocusToggle` | Movement | Quick / Standard | Focus-mode settings are plain configuration. |
| `FocusOnePush`, `FocusSnap` | Movement | Quick / Movement | Applied-only focus triggers retain movement retry/error semantics with an urgent deadline. |
| `IrisReset`, `IrisUp`, `IrisDown`, `IrisDirect` | Quick | Movement / Movement | Targeted physical aperture operations now have exact iris settlement inquiries. |
| `NdFilterDirect`, `NdFilterStepUp`, `NdFilterStepDown` | Quick | Movement / Movement | Targeted physical filter operations now have exact ND settlement inquiries. |
| `Sharpness*`, `Gamma`, `NoiseReduction2d*`, `NoiseReduction3d*`, `ImageFlipBoth`, `ImageFlipCombined` | Custom | Quick / Standard | `Custom` was only the uncategorized 60-second fallback; these are explicit quick configuration writes in v2. `ImageFlipBoth` carries both axes in one opcode and shares the combined-flip treatment with `ImageFlipCombined`. |
| 68 queryable built-in inquiries | Quick | Inquiry / Inquiry | Inquiry response timing is a separate profile fact: v2 gives inquiries their own deadline, an interim 1 s on every built-in profile, down from the 5 s `Quick` budget 1.x inquiries used (parity waiver `inquiry-deadline-interim-default`). The inquiry retry *budget* still uses the old quick budget. |

The remaining 120 command rows retain their 1.x timeout category, and every
command row has an explicit retry class. The `BuiltinCommand` universe size
(149) and this 29-changed / 120-preserved split are pinned by
`intentional_timeout_category_changes_account_for_the_preserved_remainder` in
`src/command/semantics.rs`, so adding or reclassifying a command fails that test
until this table and the count above are updated to match. `CommandCancel` is
deliberately
`Quick`/`Never` because replaying a cancellation is not a safe generic retry;
`PushAfPress` and `PushAfRelease` are `Quick`/`Movement` because they are
focus actuation and may receive the movement-specific transient `0x41` retry.
`PresetSet` and `PresetReset` are `Preset`/`Preset`: their timeout category
and movement/preset-specific `0x41` retry behavior are both preserved even
though the v2 semantic class is plain. Retry counts are derived from the
timeout category, while the retry class selects replay and contextual error
arms; consequently a `Quick`/`Movement` stop intentionally receives the
quick retry count while retaining movement error handling.

No v2 request resurrects `Custom`. The old fallback is compared as behavior,
not as a public API compatibility requirement.

## Adding or waiving a row

Add a stable lowercase behavior ID to `required_families` **and to
`EXPECTED_REQUIRED_FAMILIES` in `validate.py`** — the required set is pinned in
the validator, so a family cannot be dropped from coverage in a lone manifest
edit. Then add one verified row with:

1. the 1.x path and one or more concrete function symbols;
2. one or more v2 test definitions, each with a current source symbol, its
   `envelope`/`profile`/`receipt_class`, and a direct `cargo test` command; and
3. a `#542` clause plus a short rationale.

The validator rejects missing, duplicate, placeholder, unverified, or
unreferenced IDs, and a family set that does not match the pinned constant. It
runs each unique command once after grouping rows by command, and asserts every
mapped symbol executed (not filtered, not ignored). Keep mappings pointed at
existing production replay tests and fixtures. A new integration test should
drive the public owner/facade/testkit; do not add a second scheduler or a
hand-written simulator just to satisfy this corpus.

An intentional 2.0 behavior change is not a waiver by omission, and it is not a
self-approval either. Mark the row `intentional-change` only with an explicit
`approved_change` listed in `approved_intentional_changes`, document the
rationale in the row, **and add the same id verbatim to `CHANGELOG.md`** — the
gate requires every approved waiver to appear in the changelog, so a waiver
forces a reviewed, user-visible entry naming what it supersedes rather than a
manifest line that blesses itself. An unapproved classification, or an approved
id with no changelog entry, fails the gate. If the behavior is still required by
#542, keep it `preserved` and repair the production path or its test.

The three current waivers are `deterministic-equal-jitter` and
`evidence-based-raw-correlation` (both maintainer-ratified per #692) and
`evidence-bounded-retry`. **TODO(#671):** `evidence-bounded-retry` is being
rewritten to describe the new per-request (not whole-session) failure model;
its rationale below is provisional and will be ratified once #671 lands. Its
changelog entry says the same, so the coupling is satisfied without implying it
is settled.

Retry-count categories and bounded exponential scheduling retain their 1.x
provenance, but replay is no longer inferred merely from elapsed time. Sony
ambiguity can retry only with the same sequence; an ambiguous successfully sent
raw command is never replayed. By default (issue #671) it fails only that one
command with `UnsequencedCommandUnconfirmed` and quarantines its correlation
while the session survives; the opt-in `strict_unconfirmed_poison` mode restores
the whole-session poison. The default backoff starts at 50 ms and uses
deterministic equal jitter within a bounded ceiling. One admission-to-terminal budget stays active through every later
noncancelled phase and is the largest of ten seconds, twice the request's
governing deadline, and the profile busy timeout.

The raw-correlation row is likewise an explicit v2 safety change, not a waiver
by omission. Raw commands keep one unacknowledged candidate per target and
never use FIFO or temporal recency for ACK/error attribution. A named socket
that is free is exact evidence; an occupied named socket falls back to the
target's other free socket (issues #620/#682) because the ACK's candidate is
already uniquely identified, and is inert only when no socket is free. The only
remaining FIFO rule belongs to route/content-compatible raw inquiries when no
command candidate creates ambiguous ownership.

Hardware evidence remains separate. This gate proves deterministic software
behavior against scripted transports, production owner/engine replays, and
absolute wire/decode goldens. It does not claim that a particular camera,
network, serial adapter, or firmware behaves correctly; those observations stay
in the hardware release checklist.
