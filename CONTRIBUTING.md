# Contributing to grafton-visca

Thank you for your interest in contributing to grafton-visca! This guide will help you understand the project structure and best practices for contributions.

## Table of Contents

- [Getting Started](#getting-started)
- [Development Setup](#development-setup)
- [Code Style](#code-style)
- [Public API Conventions](#public-api-conventions)
- [Safety Guidelines](#safety-guidelines)
- [Command Development](#command-development)
- [Camera Profile Support](#camera-profile-support)
- [Testing](#testing)
- [Public API Snapshots](#public-api-snapshots)
- [Documentation](#documentation)
- [Pull Request Process](#pull-request-process)
- [Release Process](#release-process)

## Getting Started

Before contributing, please:

1. Read the [README.md](README.md) to understand the project architecture
2. Review existing issues and pull requests
3. Fork the repository and create a feature branch

## Development Setup

### Toolchains

CI pins every toolchain to an exact version in `.github/workflows/ci.yml`.
Compile-contract fixtures declare exact error codes and stable diagnostic
anchors beside the source lines they cover. For example, `//~^ E0603 "is private"`
matches the preceding line. Public API listings generated from the base and
proposed source are compared byte for byte. Reproducing a CI failure locally
means using the same versions:

```bash
rustup toolchain install 1.98.0          # stable jobs and compile contracts
# `--profile minimal` installs rustc/cargo/rust-std only, so the components the
# nightly jobs actually use must be named explicitly. `--component` takes one
# comma-separated list; a space-separated second name is parsed as another
# toolchain, not as a component.
rustup toolchain install nightly-2026-08-26 --profile minimal --component rustfmt  # fmt, fuzz, API snapshots
```

A newer stable can change diagnostic prose. Update a fixture's in-source
message anchor only when the workflow's pinned stable is bumped in the same
change and the new diagnostic still proves the intended contract. Regenerating
the public API listings is documented under
[Public API Snapshots](#public-api-snapshots).

```bash
# Clone your fork
git clone https://github.com/YOUR_USERNAME/grafton-visca.git
cd grafton-visca

# Run the canonical local/release validation matrix. CI builds one job per
# feature shape from this script's `list-json` mode, so both share one list.
bash .github/scripts/test-all-features.sh

# Inspect the canonical per-leg commands and run lint/doc matrix modes.
bash .github/scripts/test-all-features.sh list-json
bash .github/scripts/test-all-features.sh clippy
bash .github/scripts/test-all-features.sh doc

# Format with the pinned formatter.
cargo +nightly-2026-08-26 fmt --all
```

## Code Style

Commit hooks check formatting with the CI-pinned nightly formatter, lint the
default feature set once, and run text hygiene checks. The default feature set
already enables `blocking`; it does not need a second blocking-only lint pass.
CI owns broad feature testing, all-feature and no-default linting, and rustdoc.

For a feature-specific lint investigation, run an opt-in hook, for example
`pre-commit run clippy-runtime-smol --hook-stage manual --all-files`.
`pre-commit run --hook-stage manual --all-files` runs all the extra lint and
rustdoc hooks. Use these when the change warrants them, rather than stacking
the entire local matrix on top of a successful CI run for the same source.

We maintain high code quality standards:

- **Format**: Use `cargo fmt` before committing
- **Linting**: Fix all `cargo clippy` warnings (including pedantic)
- **Documentation**: All public APIs must have doc comments with examples
- **Tests**: Add tests for new functionality

## Public API Conventions

These conventions are normative for public changes. `src/api_conventions.rs`
and the public-path, facade-parity, value and compile-contract gates enforce
this grammar; update the declarations and their reviewed contracts together.

1. **One canonical path per item.** Facade items live in `blocking` or
   `r#async`. Other items have one topical home. The crate root re-exports only
   `Error`, `Result`, `value`, `value_enum` and `ViscaInquiry`;
   `prelude::{blocking, r#async}` are the only additional re-export points.
2. **Rust API Guidelines naming.** Treat acronyms as words (`SonyFr7`, `Atw`,
   `PushAutoFocus`), omit `get_`, keep constructors on the type they return and
   follow C-CONV for conversions.
3. **Noun grammar.** Read a property with `p() -> T` and write it with
   `set_p(T)`, using the same type, including booleans. The primary mode is
   `mode()` / `set_mode(Mode)`; a sub-feature uses `p_mode()` /
   `set_p_mode(PMode)`. Single steps are `p_up`, `p_down`, `p_reset`; counted
   steps are `p_up_by(steps)` and `p_down_by(steps)`; toggles are `toggle_p`.
   Other actions use the closed verb list in `src/api_conventions.rs`, such
   as `on`, `standby`, `home`, `stop`, `drive`, `recall`, `store`, `clear`,
   `one_push`, `navigate`, `enter`, `back` and `stop_all`. Do not introduce
   `enable_*`, `disable_*`, `*_on`, `*_off`, `increase_*`, `decrease_*`,
   `*_direct`, `*_status` or `x_set` methods.
4. **One meaning per name.** A closed verb keeps the same meaning on every
   noun.
5. **Round-trip types.** For every readable and writable property,
   `set_p(p()?)` type-checks. A write-only property's `StateCache` read returns
   the setter's type.
6. **Concept names.** Method, value, request, marker and typed-surface names
   contain no vendor or model tokens. Vendors belong in profile types, ids and
   encoders. The same capability contract reuses its marker with a
   profile-selected encoding; a different contract receives a different
   vendor-neutral concept name.
7. **One value generator.** Every bounded scalar uses fallible `new`,
   `value()`, `MIN`/`MAX`, `TryFrom`/`From`, `Debug`, `Clone`, `Copy`,
   `PartialEq`, `Eq`, `PartialOrd`, `Ord`, `Hash`, and serde, schemars and ts-rs. Construction validates the value's own
   domain; preparation validates the profile domain. Use `Degrees`, `Kelvin`,
   `UnitInterval`, `ExposureTime` and `std::time::Duration` for physical
   quantities. Value enums use the same generator family. Public wire
   conversion belongs only to `wire::WireValue`.
8. **Errors.** Every fallible public constructor returns `crate::Result`,
   never `Option` or another error type. Data-carrying `Error` variants use
   named fields.
9. **Two configuration shapes.** A builder has bare setter names and a
   terminal `build() -> Result<X>`, with no getters. A reusable config value
   has `with_p(self, value) -> Self` setters, `p(&self)` getters and private
   fields.
10. **Serialization.** Every serde-visible name is `snake_case`. Persisted
    profile ids are explicit and pinned by tests. Portable data types support
    serde, schemars and ts-rs, including values, configuration, policies and
    diagnostics. Runtime errors and error-bearing outcomes remain opaque,
    alongside sessions, views and handles.
11. **Complete noun docs.** Every noun method states its behavior, unit and
    range source, and completion shape. Placeholder text is rejected.
12. **Complete gates.** A row's static bound and runtime gate list the same
    markers.
13. **Evidence and callability.** A `Has*` marker is a source-backed claim,
    implemented only for profiles with that evidence. A compile-time profile's
    `DOMAINS`, `TRANSPORTS` and `TYPED_SUPPORT` match its specification.
    Domain and transport markers also require the corresponding entry in
    `DOMAINS` or `TRANSPORTS` at the first typed use. Facade methods use the
    sealed `Permits<gate::X>`, satisfied by evidenced compile-time profiles and
    `AnyProfile`. Generic code requiring a static guarantee uses
    `CompileTimeProfile + HasX`; code accepting runtime-checked cameras uses
    `CameraProfile + Permits<gate::X>`.

## Safety Guidelines

### Protocol Safety with VISCA_TERMINATOR

All VISCA commands MUST be properly terminated with the `VISCA_TERMINATOR` byte (0xFF). This is critical for protocol compliance and camera communication safety.

Built-in encoders use the crate's internal `FrameWriter`, catalogue bodies and
bounded output storage. Downstream `request::Request` implementations likewise
write a complete addressed, terminated frame. Check the boundary directly:

```rust
use grafton_visca::{
    command::zoom::Stop,
    request::Request,
    types::CameraId,
    wire::VISCA_TERMINATOR,
};

# fn main() -> grafton_visca::Result<()> {
let mut buffer = [0; Stop::MAX_SIZE];
let length = Stop.write_into(CameraId::CAMERA_1, &mut buffer)?;
assert_eq!(buffer[length - 1], VISCA_TERMINATOR);
# Ok(()) }
```

### Physical Safety Considerations

PTZ cameras involve physical movement that can cause damage or injury if not handled properly:

1. **Movement Commands**: Always document safety warnings for pan/tilt/zoom operations
2. **Speed Limits**: Respect maximum speed parameters to prevent mechanical damage
3. **Position Bounds**: Implement and respect position limits
4. **Emergency Stop**: Ensure stop commands are easily accessible

Example safety documentation:

```rust
/// Moves the camera to absolute pan/tilt position.
///
/// # Safety
///
/// This command physically moves the camera hardware. Ensure:
/// - Clear operating area around the camera
/// - No obstructions in the movement path
/// - Appropriate speed settings for your environment
fn absolute_move(
    camera: &grafton_visca::blocking::Camera<grafton_visca::profiles::PtzOpticsG2>,
    position: grafton_visca::types::PanTiltPosition,
) -> grafton_visca::Result<grafton_visca::blocking::Operation<grafton_visca::operation::Targeted>> {
    camera.pan_tilt().absolute(position, grafton_visca::types::SpeedLevel::Medium)
}
```

## Command Development

When implementing new VISCA commands:

The owner-facing API is built from the typed `Request`, `Inquiry`, and
`OperationCommand` contracts. Custom wire values should implement `Request`
directly; the protocol encoder is an internal implementation detail and does
not by itself select a completion class or lifecycle. Keep new semantic
classifications in the authoritative request/command ledger.

### Checked value types

Declare bounded scalar values with `#[grafton_visca::value(...)]` and value
enums with `#[grafton_visca::value_enum]`. Their generated comparison/hash,
display and ecosystem implementations should be shared by built-in and
contributed values. The same attribute supports finite `f32`/`f64` domains;
NaN and infinity are rejected and negative zero is normalized. Keep the scalar's union domain separate from the camera
profile's supported domain: construction checks the former and request
preparation checks the latter before I/O. The generator owns all derives on
a checked scalar. For `Default`, implement the trait using a constant
constructed with the checked `new`.

```rust
use grafton_visca::{value, wire::WireValue};

#[value(min = 0, max = 7, display = "Speed {}")]
pub struct AdjustmentLevel(u8);

# fn example() -> grafton_visca::Result<()> {
let speed = AdjustmentLevel::new(3)?;
assert_eq!(speed.value(), 3);
assert_eq!(AdjustmentLevel::MIN.value(), 0);
assert_eq!(AdjustmentLevel::from_wire(speed.to_wire())?, speed);
# Ok(()) }
```

Use `WireValue::to_wire` and `from_wire` for protocol representations. Declare
centered or biased integer mappings on the value attribute; perform conversions
that depend on profile facts at the target boundary. Physical quantities use
`Degrees`, `Kelvin`, `UnitInterval`, `ExposureTime` or `std::time::Duration`
rather than unlabelled numbers. Check value boundaries, codec round trips and
invalid serialized input when adding or changing a value.

### 1. Use a typed Request for Safe Encoding

Typed `Request` implementations make semantic admission explicit while keeping
wire encoding bounded and allocation-free. The sealed `Request::Class` chooses
the return type of `policy()`: `CommandPolicy` for `PlainClass`, `InquiryPolicy`
for `InquiryClass`, or `OperationPolicy` for `OperationClass<K>`. Import these
policies from `grafton_visca::request`; custom and raw requests share them.

The following custom extension illustrates encoding and classification only;
its illustrative vendor payload grants no camera-profile support:

```rust
use grafton_visca::{
    profiles::CommandTimeoutClass,
    request::{CommandPolicy, CommandRetryClass, PlainClass, Request, SubmissionClass},
    types::CameraId,
    Error,
};

#[grafton_visca::value(min = 0, max = 15)]
struct AuxiliaryLevel(u8);

struct SetAuxiliaryLevel(AuxiliaryLevel);

impl Request for SetAuxiliaryLevel {
    type Class = PlainClass;
    const MAX_SIZE: usize = 7;

    fn policy(&self) -> CommandPolicy {
        CommandPolicy::new(
            CommandTimeoutClass::Quick,
            CommandRetryClass::Never,
            SubmissionClass::Normal,
        )
    }

    fn write_into(&self, camera_id: CameraId, buffer: &mut [u8]) -> Result<usize, Error> {
        let frame = [
            0x80 | camera_id.value(), 0x01, 0x04, 0x7E, 0x01, self.0.value(), 0xFF,
        ];
        if buffer.len() < frame.len() {
            return Err(Error::buffer_too_small(frame.len(), buffer.len()));
        }
        buffer[..frame.len()].copy_from_slice(&frame);
        Ok(frame.len())
    }
}
```

### 2. Define Proper Buffer Sizes

Each request declares `Request::MAX_SIZE`, an upper bound including every byte
through the final `VISCA_TERMINATOR`. `write_into` returns the actual frame
length, which may be shorter than that bound.

### 3. Implement Inquiry Pattern

An inquiry implements `request::Request<Class = request::InquiryClass>` and
`request::Inquiry`, which returns a public `request::ResponseDecoder<T>`. Its
`Request::policy()` returns an `InquiryPolicy` built from `InquiryRetryClass`
and `SubmissionClass`; the lifecycle fixes inquiry timing and reply handling.
`ViscaInquiry` emits this policy method for derived inquiries. Its
`payload_shape` may declare the exact byte/nibble shape needed for correlation.
Use `ResponseDecoder::from_fn` for allocation-free decoding of data payloads;
use `wire::Payload` and its checked parsing helpers where appropriate.

The supported inquiry derive supplies this contract as well:

```rust
use grafton_visca::{request::Inquiry, types::ExposureMode, ViscaInquiry};

#[derive(Debug, Clone, Copy, ViscaInquiry)]
#[visca(opcode = 0x39, response = ExposureMode, typed_response = ExposureMode, typed_field = mode)]
struct ModeInquiry;

# fn main() -> grafton_visca::Result<()> {
assert_eq!(ModeInquiry.decoder().decode(&[0x00])?, ExposureMode::Auto);
# Ok(()) }
```

An existing profile contribution uses the existing inquiry and noun contracts.
New concepts and their decoder contracts receive separate review before any
profile first grants them.

### 4. Adding a Built-in Command

Declare every built-in command in the following five places.

1. **Ledger row** (`src/command/semantics.rs`): one `builtin_command_ledger!`
   row gives the command its `BuiltinCommand` identity and its class
   (`plain`, `applied(<AXES>)`, `applied_stop(<AXES>)` or `targeted(<AXES>)`), plus any
   write-only state effect. Its `packet:` column owns one `PacketPattern` built from
   `command::bytes::constants`, with explicit selector bytes or nibble masks
   where needed. Sourced rows require a nonempty `Some(pattern)`; use `None`
   only for unsourced rows. A `source!` reference contains the source ID and checked
   evidence key. Declare each source’s model set once in `source_ids!`.
2. **Wire encoder** (`src/command/<domain>.rs`): fixed-body and single-parameter
   encoders use the crate-internal `visca_command!` generator. Other
   `WireEncode` implementations write through `FrameWriter`. Add or reuse
   catalogue constants for fixed bytes, so the encoder and its ledger pattern
   share their register definition.
3. **Request entry** (`src/request/builtin.rs`): one `builtin_request!` entry
   names the request type's ledger row and its policy. Each entry still gives
   an explicit `size:`, the `MAX_SIZE` allocation bound for the request's
   longest frame, while the exact encoded length is measured from the encoder.
   Add the per-value checks to its `BuiltinValidation` implementation; a typed
   capability gate comes from the ledger row. A type whose values serve
   different ledger rows adds
   `rows: |value| match value[.field] { Variant => Row, ... }`, an exhaustive
   match whose patterns are variant paths (a wildcard is rejected), and its
   generated request hook gates each value on the row it selects,
   `self.ledger_row()`, before per-value validation.
4. **Noun wrapper** (`src/command/nouns/<noun>.rs`): use `forward_request!`
   to forward every request hook and derive its reviewed state projection.
   A value-selecting wrapper declares `rows = [Row, ...]`; a wrapper that
   selects different inner requests uses the generator's `select` form,
   which checks each arm's class, allocation bound and policy.
   Use `forward_inquiry!` for inquiry wrappers.
5. **Noun row** (`src/noun_table.rs`): one row under the owning `@noun` header
   gives the method name, arguments, rustdoc, capability `where` marker and
   request. The blocking and async methods, the semantic surface entry
   and the typed-request inventory are all expanded from it.

For a `Set` or `Clear` row, implement `BuiltinStateValue` on the request.
A write-only setter also needs a `state_key_ledger!` entry for its readback.
Declare new typed gates in `typed_support_registry`, and add an evidence-backed
grant in `profile_registry.rs` for each built-in profile that offers the gated
method. The tables generate the corresponding facade and capability projections.

The compiler rejects a ledger row without a noun row, a noun row whose kind
disagrees with the ledger class, and a request type without a `rows:` entry
whose noun rows disagree on their capability gate. For a value-selecting
type, declare every possible ledger row on its noun wrapper; generated preparation tests check the row selected by each
evaluated noun request. A typed built-in inquiry is added to
`define_builtin_inquiries!` in `src/command/inquiry_structs.rs` and gets its
runtime gate from its noun row in the same way. Its `bytes:` references a
catalogue-owned body, including for inquiry-only registers. Model-specific
bodies are selected by `body_for_profile` from named catalogue constants;
keep those alternatives out of reference tuples and inline byte arrays.

Then:

- extend the wire golden (`src/issue_715_wire_golden.rs` and
  `testdata/issue_715_wire_golden.txt`). Every successful case checks its
  exact selected command row or inquiry body. Every `rows:` arm must have an
  encoded witness for its own request type; duplicate arm-to-row identities
  require explicit branch witnesses before coverage can be claimed;
- add the checked evidence rows in `docs/visca_reference.md` and run the
  command, inquiry and grant evidence tests plus the orphan-row completeness
  check in `source_evidence`. Grant packet sets come from the noun-gated rows.
  Historical summary exceptions must name the exact reference key and check
  its complete expected packet set; they do not grant extra methods or packets;
- update the semantic class tripwire in `src/command/surface.rs`
  (`surface_ledger_is_exhaustive_unique_and_class_balanced`) and name the class
  the row landed in in the commit message;
- review the generated public API diff (see [Public API Snapshots](#public-api-snapshots)).

## Camera Profile Support

> A camera contribution adds a profile: one registry row with source-backed
> grants, its evidence entries, and, only when the camera speaks a different
> dialect for an existing concept, an encoding variant with golden bytes. It
> adds no public method, value type, request type, marker or typed surface. A
> genuinely new concept may add those, under a vendor-neutral name and with a
> reviewed contract, in a change separate from the profile that first uses it.

Follow the [Camera Profile Support Guide](docs/camera_profile_support.md) for
facts, grants, encoding families and the worked `acme_cam.json` promotion path.
Export a complete built-in spec with `examples/profile_spec.rs`, set its identity
to custom, validate the edited facts and use it immediately through
`Camera<profiles::AnyProfile>`. The read-only ignored probe produces query/decoder
observations for review; a successful decode does not grant writable controls.

Promote the reviewed facts into one registry row with the same fact keys and
nesting as serialized `ProfileSpec`. The registry emits compile-time markers and
runtime permissions from the same evidenced grants. Keep source/model/firmware
limits in [the VISCA reference](docs/visca_reference.md). Update facts and their
tests, then regenerate support blocks with `GRAFTON_VISCA_BLESS=1`; never
hand-edit a matrix. The wire golden uses the same variable. Blessing deliberately
fails after writing and is refused under `CI`: inspect the diff and rerun with
the variable unset.

The [PR template](.github/pull_request_template.md) holds the general and
camera-profile checklists.

## Testing

### Error constructor inventory

`bash .github/scripts/test-error-variant-reachability.sh` checks every public
`Error` variant against one reviewed constructor expression in
`.github/scripts/error-variant-constructors.tsv`. Expressions are token sequences
and must occur exactly once in production source. The shared source scanner
ignores comments, literal contents and test-only items; include construction-only
syntax, such as a field initializer, rather than a pattern spelling. Update the
inventory when moving a constructor or intentionally changing the error surface.
The checker compiles directly with the pinned stable `rustc`, without building
the library or resolving dependencies. Its mutation fixtures check missing and
duplicate production constructors and ignore copies inside tests.

### Unit Tests

Unit tests and their required fixtures ship with the library. Keep the fixtures
under `testdata/` and shared unit-test helpers under `src/test_support/`. A unit
test may supplement its shipped inputs with repository data when it handles
`NotFound` explicitly. Integration suites, compile contracts and repository
gates live under `tests/` and use the unpublished `test-support/` helpers.

Test the contract a change introduces: checked value boundaries, exact encoded
bytes, decoder results, profile admission or owner lifecycle as applicable.
Literal wire expectations must remain independent of the encoder:

```rust
use grafton_visca::{command::zoom::Stop, request::Request, types::CameraId};

#[test]
fn zoom_stop_is_a_complete_visca_frame() {
    let mut buffer = [0; Stop::MAX_SIZE];
    let length = Stop.write_into(CameraId::CAMERA_1, &mut buffer).unwrap();
    assert_eq!(&buffer[..length], &[0x81, 0x01, 0x04, 0x07, 0x00, 0xFF]);
}
```

### Integration Tests

For camera control features, use the public utilities under
`grafton_visca::testing::testkit` with the `test-utils` feature. Supply a `ScriptedTransportIdentity` that
matches the connection model. Prefer
`ScriptedBlockingTransport` or `ScriptedTransport` for protocol scripts and a
real Tokio/smol executor for wall-clock timeout behavior. See
`src/testing/testkit/README.md` and the existing operation-handle integration
tests for maintained patterns.

Helpers shared by the integration tests live in the unpublished
`grafton-visca-test-support` workspace crate (`test-support/`), a
dev-dependency whose `blocking` and `async` features follow the facade under
test. Tests that need a fake camera use its `fake_camera` module (`FakeCamera`
with `blocking_wire()`/`async_wire()`, the shared reply frames and wait
helpers); scenarios that run on several facades or runtimes use its
`facade_matrix!`/`runtime_matrix!` macros. A hand-written transport stays
local only with a `// Local fake:` comment.

Behavioral suites live as modules under `tests/integration/`, so each feature
leg links one behavioral test executable. `tests/allocation/` shares one
thread-local counting allocator and stays in a separate process to isolate
measurements. `tests/api_stability_test/` and `tests/no_default_smoke/` remain
separate because dedicated matrix legs select exactly those workloads. The
suite module name remains part of every libtest name:

```bash
cargo test --test integration issue_555_observability::
cargo test --features test-utils --test allocation issue_548_allocation_baselines::
```

Keep new suites under the appropriate target and declare them in its
`main.rs`; add a separate target only when its process state or matrix
selection requires it. `tests/common/` helpers are compiled once per target.
When moving suites, preserve each feature leg's executable test inventory:
run `.github/scripts/check-integration-test-equivalence.py capture OUTPUT`
on each checkout, then its `compare BEFORE/inventory.json AFTER/inventory.json`
mode. The `measure` mode records cold and incremental build timings and
integration executable bytes in an empty target directory for each checkout.

### Running Tests

```bash
# Default test suite
cargo test

# Inspect the same feature list CI uses; use test-leg NAME to run one entry.
bash .github/scripts/test-all-features.sh list-json
bash .github/scripts/test-all-features.sh

# With output for debugging
cargo test -- --nocapture
```

### Test time budget

The whole feature matrix (`test`, `clippy` and `doc` modes) should finish in
under ten minutes of wall time on a many-core machine when run in parallel:

```bash
# N jobs at a time, each in its own target directory under target/matrix;
# per-job logs and a summary land in target/matrix/logs/<mode>/.
bash .github/scripts/test-all-features.sh --jobs 12 test

# The 15 slowest tests of one shape (pinned nightly --report-time; timings only)
bash .github/scripts/test-all-features.sh slowest 15 --all-features
```

- No single test should need more than a few seconds of wall time; the
  parallel summary lists any test libtest reports as running for over 60 s.
  The known exception is `api_stability_test::canonical_compile_contracts`,
  which compiles the applicable authored fixtures for the leg's features and
  can exceed 60 s on a cold, fully loaded run.
- Each parallel job keeps its own target directory, so a full `test`,
  `clippy` and `doc` run can retain substantial build output under
  `target/matrix`. Delete it when you need the space; the next run rebuilds it.
- Wait on deadlines in virtual time where the owner allows it: write the
  scenario as `facade_matrix! { paused: ... }` so its Tokio case runs on paused
  time and its blocking and smol cases use `ManualClock` in `test-utils`
  legs. Those two cases are omitted without `test-utils`, while non-paused
  scenarios continue to test the production feature shapes.
- A heavy test whose behaviour cannot differ by feature configuration runs in
  one designated leg: list it in `.github/scripts/engine-model-tests.sh` only
  with evidence that no `cfg(feature = ...)` reaches its code path. The matrix
  fails if a listed test runs anywhere else, or not exactly once in its leg.

### Built-in wire golden

`testdata/issue_715_wire_golden.txt` pins the encoded bytes of every
built-in command. After a deliberate encoder change, regenerate it with:

```bash
GRAFTON_VISCA_BLESS=1 cargo +1.98.0 test --lib wire_ledger_matches_literal_golden_inventory
```

The bless run writes the fixture and then fails on purpose ("fixture blessed;
review the diff and rerun without the variable"), and it is refused when `CI`
is set. Review the fixture diff, then rerun the test without the variable.

## Public API Snapshots

Public API listings are generated from both the proposed source and its merge
base using the same pinned toolchain. CI uploads the two listings and their
diff as artifacts; generated listings are never committed. Release tags retain
the source needed to reproduce each release's API.

Three surfaces are compared: `blocking` includes the bare no-default-features
surface, `async` includes the async surface, and `all-features` covers the
serial transports, ecosystem derives and smol adapter. `--simplified` removes
blanket implementations while retaining auto traits such as `Send` and `Sync`.
Hidden macro adapters remain covered by downstream compile contracts.

To reproduce the comparison locally:

```bash
cargo +1.98.0 install cargo-public-api --version 0.52.0 --locked
base="$(git merge-base HEAD origin/main)"
for surface in blocking async all-features; do
  bash .github/scripts/check-public-api.sh "$base" "$surface"
done
```

The script writes `base.txt`, `head.txt` and `changes.diff` under
`target/public-api/<surface>/`. It builds the base from a temporary source
archive and removes that archive afterward. Feature arguments are selected from
each source tree: the async comparison retains `dyn-api` for older bases that
have it, while current heads use `runtime-tokio` alone. Both builds use
`nightly-2026-08-26`; when changing that pin, update the workflow and script
together. API drift without a `CHANGELOG.md` hunk fails the comparison. An
intended change needs a caller-visible changelog entry and review of the
uploaded diff alongside its source; fix any unintended change before merging.

## Documentation

For 2.0 work, update the Unreleased section of `CHANGELOG.md` in the same
change as the implementation. The changelog records what changed between
published versions, and Unreleased states the net change since the last
published version (the newest `v<version>` tag), not the history of how it was
reached: when a change revises an item that Unreleased already describes,
rewrite that entry to state the net result instead of adding another. If
behavior, setup, examples, or contributor workflow changes, update the matching
README, example, or contributor docs before closing the task. `submit` examples
must distinguish lifecycle management from profile-aware input validation and
applied completion from physical settling.

### Repository contents

Keep source, durable product and contributor documentation, and authored test
contracts in git. Put issue investigations, design reviews, decision records,
measurements and completion evidence on their GitHub issues and link them from
the relevant documentation.

Keep generated build and test output under ignored directories such as
`target/` or in CI artifacts. Commit a deterministic generator instead of its
output when fixtures or seed files can be recreated reliably. Fuzz starting
seeds are materialized by `python3 fuzz/seed_corpus.py`.

Retain independent regression inputs and expected results that cannot be
reconstructed from the implementation under test: authored compile contracts
and lifecycle traces, reviewed literal wire goldens, previous-release profile
JSON, and saved property-test failure seeds. Regenerating an oracle from the
current implementation would merely accept the behavior it is meant to check.
A new reproducer should become a focused source test or a seed recipe; attach
raw failure output to the issue or CI run.

### Changelog discipline for reversals and waivers

A decision must not reuse the issue number of the finding it reverses. When a
change reverts or supersedes behavior that a published version shipped — a
prior review verdict included — it gets its own `### Changed` or `### Removed`
entry under Unreleased that names the superseded finding by its issue number.
Reversing a decision made after the last published version needs no record of
its own: rewrite or drop the Unreleased entry so that it states only the net
change from the published version. An item added and removed again before the
next publication, or an internal fix of a regression that never shipped, gets
no entry.

`docs/migration_2_0.md` records the differences between 1.2 and 2.0 only. Add
or update a row there wherever a 1.x user would feel a change; do not add rows
or notes for differences between 2.0 prereleases, and do not describe an API
that did not exist in 1.2 as a migration source.

The `release-validation` CI job enforces three parts of this record mechanically:

- Every published release section is byte-for-byte immutable relative to the
  compared base. A release is published once its `v<version>` tag is reachable
  from that base. Sections below a published one also count as published.
  Only the top `## [Unreleased]` body and unpublished dated sections above the
  newest published one may change. An unpublished section may be folded back
  into Unreleased. A release
  cut that moves the prior Unreleased lines in order into one new strict dated
  section below an otherwise empty Unreleased heading is also accepted, but
  only once no unpublished section remains below it: fold those first. Each
  version has exactly one dated heading.
  Correct a published statement with an entry under Unreleased; do not edit the
  published entry. The validator needs the release tags, so run it from a
  full-history checkout with tags; it fails rather than guess when none are
  present.
- Every top-level Unreleased bullet carrying the exact `**BREAKING**` label must
  include an issue reference in the form `(#NNN)` or `(#NNN, #MMM)`.
- Every new commit that touches `src/` must have a non-empty explanatory body,
  not only a subject. The policy-boundary files under `.github/` bootstrap the
  repaired historical text and grandfather the already-audited PR #559 commit
  ledger; they must not be advanced to excuse later changes.

From a full-history checkout, run the validator and its deliberate-failure and
acceptance fixtures with:

```bash
python3 .github/scripts/validate-change-record.py "$(git merge-base HEAD origin/main)" HEAD
bash .github/scripts/test-validate-change-record.sh
```

When a change alters observable protocol behavior, add or update a direct
regression or wire/decode golden in the production owner, engine, or parser
path, and record the caller-visible consequence in `CHANGELOG.md`, and in
`docs/migration_2_0.md` when it differs from 1.2. Review the new behavior
against the implementation and its tests directly; do not preserve a behavior
merely because an earlier release had it if that would make correlation,
framing, cancellation, or physical safety less certain.

### Code Documentation

- All public items must have doc comments
- Include examples in doc comments
- Document safety considerations for movement commands
- Use `#[must_use]` where appropriate

### Example Documentation

````rust
/// Return a camera to home and wait for profile-selected settlement evidence.
///
/// # Examples
///
/// ```no_run
/// # use grafton_visca::{blocking::CameraSession, profiles::PtzOpticsG2};
/// let session = CameraSession::<PtzOpticsG2>::connect_tcp("192.168.0.110")?;
/// let camera = session.camera();
/// camera.pan_tilt().home()?.settled()?;
/// session.close()?;
/// # Ok::<(), grafton_visca::Error>(())
/// ```
///
/// # Errors
///
/// Returns an error if submission, protocol completion, fallback settling, or
/// shutdown fails.
pub fn return_home(
    camera: &grafton_visca::blocking::Camera<grafton_visca::profiles::PtzOpticsG2>,
) -> grafton_visca::Result<grafton_visca::operation::Settlement> {
    camera.pan_tilt().home()?.settled()
}
````

## Pull Request Process

1. **Branch Naming**: Use descriptive branch names (e.g., `feat/add-nd-filter`, `fix/timeout-handling`)

2. **Commit Messages**: Follow conventional commits:
   - `feat:` New features
   - `fix:` Bug fixes
   - `docs:` Documentation changes
   - `test:` Test additions/changes
   - `refactor:` Code refactoring

3. **PR Description**: Include:
   - Problem being solved
   - Implementation approach
   - Safety considerations (if applicable)
   - Testing performed

4. **Checklist**: Complete the [pull request template](.github/pull_request_template.md),
   including its camera-profile section when applicable. It is the single
   maintained checklist for validation, documentation and source evidence.

5. **Review Process**:
   - Address reviewer feedback promptly
   - Keep discussions focused and professional
   - Update PR description with any significant changes

## Release Process

Releases use a two-crate prerelease/final publish sequence because the main crate
depends on the same-version `grafton-visca-macros` package. Follow
[RELEASING.md](RELEASING.md) for `2.0.0` versioning, changelog
finalization, validation, tagging, crates.io index verification, and recovery if
the macro package publishes but the main package does not. Never create a
release tag from a commit that has not passed the complete 2.0 matrix — public
API comparisons included — on a pull request.

`cargo-semver-checks` is not part of that matrix while 2.0 is a release
candidate. Prerelease API drift is reviewed directly in the generated public API
diffs, and comparing with 1.x would only re-report the intended major
break. It returns once 2.0.0 is published.

## Feature Flags

When adding features that require new dependencies:

1. Make them optional via feature flags
2. Document the feature in Cargo.toml
3. Update README.md with feature documentation
4. Ensure tests work with and without the feature

## Performance Considerations

- Prefer const functions where possible
- Use zero-allocation patterns (see `FrameWriter`, which writes straight into the caller's buffer)
- Avoid unnecessary heap allocations
- Profile performance-critical code paths

## Questions?

If you have questions about contributing:

1. Check existing issues and discussions
2. Review the API documentation
3. Open an issue for clarification
4. Join discussions on implementation approaches

Thank you for helping make grafton-visca better and safer for everyone!
