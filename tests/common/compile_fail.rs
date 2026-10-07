//! Compile-fail harness with per-fixture expected-diagnostic declarations.
//!
//! Every fixture states, in its own source, exactly which diagnostics rustc
//! must produce for it. The harness asserts *that* declaration rather than a
//! shared list of plausible error codes.
//!
//! # Why
//!
//! The previous harness accepted any of nine error codes or four loose
//! substrings, so a fixture naming a path that had simply rotted away kept
//! passing: the `E0432` from the bogus path satisfied the same list the
//! genuine `E0603` privacy contract satisfied, and the contract the fixture
//! claimed to pin was gone without a single test turning red. Thirty-nine of
//! the forty-five fail fixtures ran through that path.
//!
//! # The declaration syntax
//!
//! A fixture declares one expectation per line, in a comment beginning `//~`:
//!
//! ```text
//! //~ E0603
//! //~ "module `camera_id` is private"
//! ```
//!
//! * `//~ E0603` requires rustc to emit `error[E0603]`.
//! * `//~ "text"` requires `text` to appear somewhere in rustc's stderr. `\"`
//!   and `\\` are the two escapes. This is how a lint-only fixture (which has
//!   no error code at all) pins its diagnostic.
//!
//! A fixture whose contract is a post-monomorphization error, such as a
//! failing `const { assert!(..) }` inside a generic function, adds the line
//! `//@ build`. `cargo check` never instantiates generic code, so those
//! fixtures are compiled with `cargo build` instead.
//!
//! The rules the harness enforces:
//!
//! 1. Every fixture declares at least one expectation. A fixture with none is
//!    a harness failure, not a silent pass.
//! 2. Every declared code must appear in stderr, **and every code in stderr
//!    must be declared**. The set is exact, so a fixture that starts failing
//!    for an extra or different reason fails loudly instead of coasting on the
//!    reason it still shares with its declaration.
//! 3. Every declared message must appear in stderr.
//! 4. A fixture declaring an *absence* code — [`ABSENCE_CODES`], the codes a
//!    typo produces just as readily as a real removal — must also declare a
//!    message naming the path that has to be missing. "This name is gone" is
//!    only a contract if the fixture says which name.
//!
//! Three conventions keep the declarations honest:
//!
//! * Put the declaration block at the end of the fixture so the contract under
//!   test remains visually primary and diagnostic anchors stay easy to audit.
//! * Anchor a message on rustc's own prose ("unresolved import `x`", "module
//!   `y` is private"), never on a bare identifier. rustc echoes the offending
//!   source line into stderr, so `"grafton_visca::CameraBuilder"` on its own
//!   would be matched by the fixture's own `use` statement no matter what
//!   rustc concluded about it.
//! * Keep the message to the part that is stable across toolchains and feature
//!   sets. 1.98 renamed "no function or associated item named `new` found for
//!   struct" to "no associated function or constant named `new` found for
//!   struct", and rustc abbreviates a type path whenever it is unambiguous
//!   (`grafton_visca::blocking::Session` under `--all-features`, plain
//!   `Session` in a blocking-only build). "named `new` found for struct" is
//!   specific, is prose, and survives both.
//!
//! Failures are collected across the whole run and reported together, so one
//! run tells you every fixture that needs attention.

#![allow(dead_code)]

use std::{
    collections::BTreeSet,
    env,
    ffi::{OsStr, OsString},
    fmt::Write as _,
    fs,
    path::{Path, PathBuf},
    process::{Command, Stdio},
};

/// Error codes that mean "this name did not resolve".
///
/// Each is emitted for a genuine removal and for a plain typo alike, so a
/// fixture declaring one must also pin the message naming the absent path.
const ABSENCE_CODES: &[&str] = &[
    "E0405", "E0412", "E0422", "E0425", "E0432", "E0433", "E0599",
];

/// One diagnostic a fixture requires of rustc.
#[derive(Debug, Clone, PartialEq, Eq)]
enum Expectation {
    /// `//~ E0603`: rustc must emit `error[E0603]`.
    Code(String),
    /// `//~ "..."`: the text must appear in rustc's stderr.
    Message(String),
}

pub fn active_grafton_visca_features() -> Vec<&'static str> {
    let mut features = Vec::new();

    if cfg!(feature = "blocking") {
        features.push("blocking");
    }
    if cfg!(feature = "async") {
        features.push("async");
    }
    if cfg!(feature = "runtime-tokio") {
        features.push("runtime-tokio");
    }
    if cfg!(feature = "runtime-smol") {
        features.push("runtime-smol");
    }
    if cfg!(feature = "transport-serial") {
        features.push("transport-serial");
    }
    if cfg!(feature = "transport-serial-tokio") {
        features.push("transport-serial-tokio");
    }
    if cfg!(feature = "dyn-api") {
        features.push("dyn-api");
    }
    if cfg!(feature = "test-utils") {
        features.push("test-utils");
    }
    if cfg!(feature = "serde") {
        features.push("serde");
    }
    if cfg!(feature = "schemars") {
        features.push("schemars");
    }
    if cfg!(feature = "ts-rs") {
        features.push("ts-rs");
    }
    features
}

/// Resolves the target namespace used by nested compile-fail Cargo projects.
///
/// Cargo exposes `CARGO_TARGET_DIR` to test processes using the same path
/// semantics as its own command-line configuration: an absolute path is
/// preserved, while a relative path is relative to the invoking process's
/// current directory. Keeping that namespace here prevents concurrent matrix
/// legs from sharing generated sources or nested Cargo artifacts by accident.
fn compile_fail_target_root(
    crate_root: &Path,
    caller_target_dir: Option<&Path>,
    current_dir: &Path,
) -> PathBuf {
    match caller_target_dir {
        Some(path) if path.is_absolute() => path.to_path_buf(),
        Some(path) => current_dir.join(path),
        None => crate_root.join("target"),
    }
}

fn configured_compile_fail_target_root(crate_root: &Path) -> PathBuf {
    let current_dir = env::current_dir().unwrap_or_else(|error| {
        panic!("failed to determine compile-fail current directory: {error}")
    });
    let caller_target_dir = env::var_os("CARGO_TARGET_DIR").map(PathBuf::from);
    compile_fail_target_root(crate_root, caller_target_dir.as_deref(), &current_dir)
}

fn compile_fail_work_root(target_root: &Path, process_id: u32, fixture_dirs: &[&str]) -> PathBuf {
    // Explicit fixture lists can contain many long paths. A fixed-width
    // identity avoids the per-component filename limit while keeping separate
    // harness invocations isolated.
    let mut identity = 0xcbf2_9ce4_8422_2325_u64;
    for item in fixture_dirs {
        for byte in item.bytes().chain(std::iter::once(0xff)) {
            identity ^= u64::from(byte);
            identity = identity.wrapping_mul(0x0000_0100_0000_01b3);
        }
    }
    target_root
        .join("contract-compile-fail")
        .join(format!("{process_id}-{identity:016x}"))
}

fn compile_fail_nested_target_root(target_root: &Path) -> PathBuf {
    target_root.join("contract-compile-fail-target")
}

/// Removes only the generated source/manifests for one harness process.
///
/// Nested Cargo artifacts deliberately live beside this directory, under the
/// stable `contract-compile-fail-target` child of the caller's target root,
/// so dropping this guard can never remove a reusable target cache.
struct CompileFailWorkGuard {
    path: PathBuf,
}

impl CompileFailWorkGuard {
    fn new(path: PathBuf) -> Self {
        Self { path }
    }
}

impl Drop for CompileFailWorkGuard {
    fn drop(&mut self) {
        match fs::remove_dir_all(&self.path) {
            Ok(()) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => eprintln!(
                "failed to clean up compile-fail work dir {}: {error}",
                self.path.display()
            ),
        }
    }
}

pub fn assert_compile_fail_fixtures(fixture_dirs: &[&str], crate_features: &[&str]) {
    let crate_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let fixtures = collect_fixtures(&crate_root, fixture_dirs);
    assert!(
        !fixtures.is_empty(),
        "no compile-fail fixtures found in {:?}",
        fixture_dirs
    );

    assert_compile_fixture_set(
        &crate_root,
        fixtures,
        fixture_dirs,
        crate_features,
        ExpectedOutcome::Failure,
    );
}

/// Compiles every Rust fixture in the supplied directories and requires it to
/// succeed with the active public feature surface.
pub fn assert_compile_pass_fixtures(fixture_dirs: &[&str], crate_features: &[&str]) {
    let crate_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let fixtures = collect_fixtures(&crate_root, fixture_dirs);
    assert!(
        !fixtures.is_empty(),
        "no compile-pass fixtures found in {:?}",
        fixture_dirs
    );

    assert_compile_fixture_set(
        &crate_root,
        fixtures,
        fixture_dirs,
        crate_features,
        ExpectedOutcome::Success,
    );
}

/// Compiles an explicit fixture list through the declaration-based harness.
///
/// Issue-specific suites use this when they own only a subset of a shared
/// fixture directory. Keeping those suites on this harness avoids relying on
/// toolchain-sensitive rendered stderr snapshots.
pub fn assert_compile_fail_fixture_paths(fixture_paths: &[&str], crate_features: &[&str]) {
    let crate_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    assert!(
        !fixture_paths.is_empty(),
        "no compile-fail fixture paths supplied"
    );
    let mut fixtures = fixture_paths
        .iter()
        .map(|path| crate_root.join(path))
        .collect::<Vec<_>>();
    for fixture in &fixtures {
        assert!(
            fixture.is_file(),
            "compile-fail fixture does not exist: {}",
            fixture.display()
        );
    }
    fixtures.sort();

    assert_compile_fixture_set(
        &crate_root,
        fixtures,
        fixture_paths,
        crate_features,
        ExpectedOutcome::Failure,
    );
}

/// Compiles an explicit list of fixtures and requires each one to succeed.
pub fn assert_compile_pass_fixture_paths(fixture_paths: &[&str], crate_features: &[&str]) {
    let crate_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    assert!(
        !fixture_paths.is_empty(),
        "no compile-pass fixture paths supplied"
    );
    let mut fixtures = fixture_paths
        .iter()
        .map(|path| crate_root.join(path))
        .collect::<Vec<_>>();
    for fixture in &fixtures {
        assert!(
            fixture.is_file(),
            "compile-pass fixture does not exist: {}",
            fixture.display()
        );
    }
    fixtures.sort();

    assert_compile_fixture_set(
        &crate_root,
        fixtures,
        fixture_paths,
        crate_features,
        ExpectedOutcome::Success,
    );
}

#[derive(Clone, Copy)]
enum ExpectedOutcome {
    Success,
    Failure,
}

fn assert_compile_fixture_set(
    crate_root: &Path,
    fixtures: Vec<PathBuf>,
    work_identity: &[&str],
    crate_features: &[&str],
    expected: ExpectedOutcome,
) {
    let target_root = configured_compile_fail_target_root(crate_root);
    let nested_target_root = compile_fail_nested_target_root(&target_root);
    let work_root = compile_fail_work_root(&target_root, std::process::id(), work_identity);
    if work_root.exists() {
        fs::remove_dir_all(&work_root).unwrap_or_else(|error| {
            panic!(
                "failed to remove old compile-fail work dir {}: {error}",
                work_root.display()
            )
        });
    }
    let _work_cleanup = CompileFailWorkGuard::new(work_root.clone());
    fs::create_dir_all(&work_root).unwrap_or_else(|error| {
        panic!(
            "failed to create compile-fail work dir {}: {error}",
            work_root.display()
        )
    });

    prefetch_contract_dependencies(crate_root, &work_root, &nested_target_root, crate_features);

    let cases = write_case_workspace(crate_root, &work_root, &fixtures, crate_features);
    let run = compile_case_workspace(&work_root, &nested_target_root, &cases);

    let mut failures = Vec::new();
    for (case, compilation) in cases.iter().zip(&run.compilations) {
        let fixture_label = case
            .fixture
            .strip_prefix(crate_root)
            .unwrap_or(&case.fixture)
            .display()
            .to_string();
        let result = match expected {
            ExpectedOutcome::Success => {
                check_pass_fixture(&fixture_label, compilation, &run.unattributed)
            }
            ExpectedOutcome::Failure => {
                check_fail_fixture(&fixture_label, &case.source, compilation, &run.unattributed)
            }
        };
        if let Err(failure) = result {
            failures.push(failure);
        }
    }

    let contract_kind = match expected {
        ExpectedOutcome::Success => "compile-pass",
        ExpectedOutcome::Failure => "compile-fail",
    };

    assert!(
        failures.is_empty(),
        "{} of {} {contract_kind} fixture(s) violated their contract \
         (features {crate_features:?})\n\n{}",
        failures.len(),
        fixtures.len(),
        failures.join("\n\n"),
    );
}

fn collect_fixtures(crate_root: &Path, fixture_dirs: &[&str]) -> Vec<PathBuf> {
    let mut fixtures = Vec::new();

    for fixture_dir in fixture_dirs {
        let absolute_dir = crate_root.join(fixture_dir);
        let entries = fs::read_dir(&absolute_dir).unwrap_or_else(|error| {
            panic!(
                "failed to read compile-fail fixture dir {}: {error}",
                absolute_dir.display()
            )
        });

        for entry in entries {
            let entry = entry.unwrap_or_else(|error| {
                panic!(
                    "failed to read an entry in fixture dir {}: {error}",
                    absolute_dir.display()
                )
            });
            let path = entry.path();
            if path.extension().is_some_and(|extension| extension == "rs") {
                fixtures.push(path);
            }
        }
    }

    fixtures.sort();
    fixtures
}

fn prefetch_contract_dependencies(
    crate_root: &Path,
    work_root: &Path,
    nested_target_root: &Path,
    crate_features: &[&str],
) {
    let prefetch_dir = work_root.join("prefetch-dependencies");
    let prefetch_src_dir = prefetch_dir.join("src");
    fs::create_dir_all(&prefetch_src_dir).unwrap_or_else(|error| {
        panic!(
            "failed to create compile-fail dependency prefetch dir {}: {error}",
            prefetch_src_dir.display()
        )
    });
    fs::write(
        prefetch_dir.join("Cargo.toml"),
        case_manifest(
            crate_root,
            0,
            "prefetch-dependencies",
            crate_features,
            ManifestRoot::Standalone,
        ),
    )
    .unwrap_or_else(|error| {
        panic!(
            "failed to write compile-fail dependency prefetch manifest {}: {error}",
            prefetch_dir.display()
        )
    });
    fs::write(prefetch_src_dir.join("lib.rs"), "").unwrap_or_else(|error| {
        panic!(
            "failed to write compile-fail dependency prefetch source {}: {error}",
            prefetch_src_dir.display()
        )
    });

    let mut command = nested_cargo_command();
    command
        .arg("fetch")
        .arg("--quiet")
        .arg("--manifest-path")
        .arg(prefetch_dir.join("Cargo.toml"))
        .env("CARGO_TARGET_DIR", nested_target_root)
        .env("CARGO_HTTP_MULTIPLEXING", "false")
        .env("CARGO_NET_RETRY", "10");

    let output = command.output().unwrap_or_else(|error| {
        panic!("failed to prefetch dependencies for compile-fail fixtures: {error}")
    });

    if !output.status.success() {
        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);
        panic!(
            "failed to prefetch dependencies for compile-fail fixtures\nstdout:\n{stdout}\nstderr:\n{stderr}"
        );
    }
}

/// One fixture's package inside the generated case workspace.
struct CasePackage {
    fixture: PathBuf,
    source: String,
    /// Directory of the member, relative to the workspace root.
    dir_name: String,
    /// Package name, which is also the name of its only (bin) target.
    package_name: String,
    /// `build` or `check`; see [`compile_command`].
    command: &'static str,
}

/// What the nested Cargo run reported for one fixture package.
#[derive(Debug, Default, Clone, PartialEq, Eq)]
struct FixtureCompilation {
    /// Cargo emitted a `compiler-artifact` record for the fixture's bin
    /// target, i.e. rustc finished the crate without an error.
    compiled: bool,
    /// The fixture's rustc diagnostics, rendered exactly as `cargo check`
    /// prints them to stderr (see [`attribute_cargo_messages`]).
    diagnostics: String,
}

struct WorkspaceRun {
    /// One entry per [`CasePackage`], in the same order.
    compilations: Vec<FixtureCompilation>,
    /// Cargo's own stderr plus every diagnostic not belonging to a fixture
    /// package (for example the library under test). Shown only for a
    /// fixture that failed without a diagnostic of its own.
    unattributed: String,
}

/// Writes one nested Cargo workspace whose members are the fixture packages.
///
/// Every member has the same dependency and feature set, so resolver-2
/// feature unification cannot change what any single fixture sees compared
/// with compiling it as its own workspace.
fn write_case_workspace(
    crate_root: &Path,
    work_root: &Path,
    fixtures: &[PathBuf],
    crate_features: &[&str],
) -> Vec<CasePackage> {
    let mut cases = Vec::with_capacity(fixtures.len());
    for (index, fixture) in fixtures.iter().enumerate() {
        let fixture_name = fixture
            .file_stem()
            .and_then(|stem| stem.to_str())
            .unwrap_or("fixture");
        let source = fs::read_to_string(fixture).unwrap_or_else(|error| {
            panic!("failed to read fixture {}: {error}", fixture.display())
        });
        let dir_name = format!("{index:02}-{}", sanitize(fixture_name));
        let case_dir = work_root.join(&dir_name);
        let src_dir = case_dir.join("src");
        fs::create_dir_all(&src_dir).unwrap_or_else(|error| {
            panic!(
                "failed to create compile-contract case dir {}: {error}",
                src_dir.display()
            )
        });
        fs::write(src_dir.join("main.rs"), &source).unwrap_or_else(|error| {
            panic!(
                "failed to write compile-contract case source for {}: {error}",
                fixture.display()
            )
        });
        fs::write(
            case_dir.join("Cargo.toml"),
            case_manifest(
                crate_root,
                index,
                fixture_name,
                crate_features,
                ManifestRoot::Member,
            ),
        )
        .unwrap_or_else(|error| {
            panic!(
                "failed to write compile-contract case manifest for {}: {error}",
                fixture.display()
            )
        });
        cases.push(CasePackage {
            fixture: fixture.clone(),
            command: compile_command(&source),
            package_name: case_package_name(index, fixture_name),
            dir_name,
            source,
        });
    }

    let members = cases
        .iter()
        .map(|case| case.dir_name.as_str())
        .collect::<Vec<_>>();
    let manifest = format!(
        "[workspace]\nmembers = {}\nresolver = \"2\"\n",
        toml_array(&members)
    );
    fs::write(work_root.join("Cargo.toml"), manifest).unwrap_or_else(|error| {
        panic!(
            "failed to write compile-contract workspace manifest in {}: {error}",
            work_root.display()
        )
    });
    cases
}

/// Compiles every fixture package with one Cargo invocation per command kind
/// (`check`, and `build` only when some fixture declares `//@ build`).
///
/// `--keep-going` makes Cargo compile every member even after one fails, so
/// each fixture gets its own verdict exactly as a separate invocation would.
fn compile_case_workspace(
    work_root: &Path,
    nested_target_root: &Path,
    cases: &[CasePackage],
) -> WorkspaceRun {
    let mut compilations = vec![FixtureCompilation::default(); cases.len()];
    let mut unattributed = String::new();

    for command_kind in ["check", "build"] {
        let selected = cases
            .iter()
            .enumerate()
            .filter(|(_, case)| case.command == command_kind)
            .collect::<Vec<_>>();
        if selected.is_empty() {
            continue;
        }

        let mut command = nested_cargo_command();
        command
            .arg(command_kind)
            .arg("--keep-going")
            .arg("--offline")
            .arg("--quiet")
            .arg("--message-format=json")
            .arg("--manifest-path")
            .arg(work_root.join("Cargo.toml"))
            .env("CARGO_TARGET_DIR", nested_target_root);
        for (_, case) in &selected {
            command.arg("-p").arg(&case.package_name);
        }
        let output = command.output().unwrap_or_else(|error| {
            panic!(
                "failed to run cargo {command_kind} for compile-contract fixtures in {}: {error}",
                work_root.display()
            )
        });

        let stdout = String::from_utf8_lossy(&output.stdout);
        let packages = selected
            .iter()
            .map(|(_, case)| (case.package_name.as_str(), case.dir_name.as_str()))
            .collect::<Vec<_>>();
        let attributed = attribute_cargo_messages(&stdout, &packages).unwrap_or_else(|problem| {
            panic!(
                "cargo {command_kind} produced an unreadable JSON message stream: {problem}\n\
                 stderr:\n{}",
                String::from_utf8_lossy(&output.stderr)
            )
        });
        for ((slot, _), compilation) in selected.iter().zip(attributed.compilations) {
            compilations[*slot] = compilation;
        }
        unattributed.push_str(&attributed.unattributed);
        unattributed.push_str(&String::from_utf8_lossy(&output.stderr));
    }

    WorkspaceRun {
        compilations,
        unattributed,
    }
}

/// Splits Cargo's `--message-format=json` stream into per-fixture results.
///
/// `packages` lists `(package name, member directory)` pairs; each fixture
/// package has exactly one target, a bin of the same name, so a record's
/// `target.name` identifies its fixture.
///
/// Equivalence with the per-fixture `cargo check --quiet` stderr this replaces:
///
/// * Cargo renders human diagnostics by printing rustc's JSON `rendered`
///   field verbatim, so concatenating a fixture's `compiler-message` records
///   in stream order reproduces its rustc output byte for byte.
/// * The only stderr lines Cargo added itself were the summaries
///   "error: could not compile `..`" and "warning: `..` generated N
///   warnings". Neither contains an `error[E....]` code, and no fixture
///   anchors a message on them, so dropping them changes no verdict.
/// * The old per-fixture verdict was the process exit status. A crate that
///   rustc finished produces a `compiler-artifact` record (including when it
///   is fresh from the cache), and a crate that failed produces none, so the
///   artifact record is the per-package equivalent of that status.
/// * Inside a workspace rustc reports `NN-name/src/main.rs` instead of
///   `src/main.rs`; the member prefix is removed so reports read as before.
///   No fixture anchors a message on a path either way.
fn attribute_cargo_messages(
    json_stream: &str,
    packages: &[(&str, &str)],
) -> Result<WorkspaceRun, String> {
    let mut compilations = vec![FixtureCompilation::default(); packages.len()];
    let mut unattributed = String::new();

    for (number, line) in json_stream.lines().enumerate() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        let record: serde_json::Value = serde_json::from_str(line)
            .map_err(|problem| format!("line {}: {problem}", number + 1))?;
        let reason = record.get("reason").and_then(serde_json::Value::as_str);
        let target_name = record
            .get("target")
            .and_then(|target| target.get("name"))
            .and_then(serde_json::Value::as_str);
        let slot =
            target_name.and_then(|name| packages.iter().position(|(package, _)| *package == name));

        match reason {
            Some("compiler-message") => {
                let rendered = record
                    .get("message")
                    .and_then(|message| message.get("rendered"))
                    .and_then(serde_json::Value::as_str)
                    .unwrap_or_default();
                match slot {
                    Some(slot) => {
                        let member_source = format!("{}/src/", packages[slot].1);
                        compilations[slot]
                            .diagnostics
                            .push_str(&rendered.replace(&member_source, "src/"));
                    }
                    None => unattributed.push_str(rendered),
                }
            }
            Some("compiler-artifact") => {
                if let Some(slot) = slot {
                    compilations[slot].compiled = true;
                }
            }
            _ => {}
        }
    }

    Ok(WorkspaceRun {
        compilations,
        unattributed,
    })
}

/// Matches one compiled fail fixture against its declarations.
///
/// Returns the report for a fixture that did not meet them.
fn check_fail_fixture(
    fixture_label: &str,
    source: &str,
    compilation: &FixtureCompilation,
    unattributed: &str,
) -> Result<(), String> {
    let expectations = match parse_expectations(source) {
        Ok(expectations) => expectations,
        Err(problem) => {
            return Err(format!("{fixture_label}: {problem}"));
        }
    };
    if let Some(problem) = declaration_problem(&expectations) {
        return Err(format!("{fixture_label}: {problem}"));
    }

    if compilation.compiled {
        return Err(format!(
            "{fixture_label}: compiled successfully; the contract it pins is no longer enforced \
             (expected {})",
            describe(&expectations),
        ));
    }

    let stderr = compilation.diagnostics.as_str();
    let problems = unmet_expectations(&expectations, stderr);
    if problems.is_empty() {
        return Ok(());
    }

    let mut report = format!("{fixture_label}:");
    for problem in &problems {
        let _ = write!(report, "\n  - {problem}");
    }
    let _ = write!(report, "\n  declared: {}", describe(&expectations));
    let _ = write!(report, "\n  stderr:\n{}", indent(stderr));
    append_unattributed(&mut report, compilation, unattributed);
    Err(report)
}

fn check_pass_fixture(
    fixture_label: &str,
    compilation: &FixtureCompilation,
    unattributed: &str,
) -> Result<(), String> {
    if compilation.compiled {
        return Ok(());
    }

    let mut report = format!(
        "{fixture_label}: did not compile successfully\n  stderr:\n{}",
        indent(&compilation.diagnostics),
    );
    append_unattributed(&mut report, compilation, unattributed);
    Err(report)
}

/// Adds Cargo's own output to a report when the fixture failed without a
/// diagnostic of its own (a dependency or manifest failure), so the cause
/// is never hidden.
fn append_unattributed(report: &mut String, compilation: &FixtureCompilation, unattributed: &str) {
    if compilation.diagnostics.trim().is_empty() && !unattributed.trim().is_empty() {
        let _ = write!(
            report,
            "\n  cargo output not attributable to a fixture:\n{}",
            indent(unattributed)
        );
    }
}

/// Builds a Cargo command for the nested contract projects.
///
/// The outer test process may set compiler flags globally (CI does this with
/// `RUSTFLAGS=-D warnings`). Those flags are not part of a fixture's public
/// contract and can turn warnings from dependencies or generated metadata
/// into unrelated errors. Keep the nested invocation's environment explicit:
/// fixture-local `#![deny(...)]` attributes still apply because they are in
/// the source being checked.
fn nested_cargo_command() -> Command {
    let mut command = Command::new(cargo_executable());
    // Do not inherit the host's `/dev/null` for nested Cargo/rustc probes.
    // Some CI/container environments expose a regular file there, and its
    // contents can be consumed by rustc's `-` target-info probe instead of
    // EOF. `output()` closes a piped stdin before waiting for the child,
    // giving every nested process a real, deterministic empty stream.
    command.stdin(Stdio::piped());
    for variable in [
        "RUSTFLAGS",
        "CARGO_ENCODED_RUSTFLAGS",
        "RUSTDOCFLAGS",
        "CARGO_ENCODED_RUSTDOCFLAGS",
    ] {
        command.env_remove(variable);
    }
    command
}

/// The Cargo command a fixture is compiled with: `build` when it declares
/// `//@ build` (its contract is a post-monomorphization error), else `check`.
fn compile_command(source: &str) -> &'static str {
    if source.lines().any(|line| line.trim() == "//@ build") {
        "build"
    } else {
        "check"
    }
}

/// Reads the `//~` declaration block out of a fixture's source.
fn parse_expectations(source: &str) -> Result<Vec<Expectation>, String> {
    let mut expectations = Vec::new();

    for (offset, line) in source.lines().enumerate() {
        let Some(rest) = line.trim_start().strip_prefix("//~") else {
            continue;
        };
        let number = offset + 1;
        let rest = rest.trim();

        if rest.is_empty() {
            return Err(format!("line {number}: empty `//~` expectation"));
        }
        if rest.starts_with('"') {
            let text = unquote(rest).ok_or_else(|| {
                format!("line {number}: `//~ \"...\"` expectation is not a closed quoted string")
            })?;
            if text.is_empty() {
                return Err(format!("line {number}: empty `//~ \"...\"` expectation"));
            }
            expectations.push(Expectation::Message(text));
        } else if is_error_code(rest) {
            expectations.push(Expectation::Code(rest.to_owned()));
        } else {
            return Err(format!(
                "line {number}: expected `//~ E1234` or `//~ \"message text\"`, found `//~ {rest}`"
            ));
        }
    }

    Ok(expectations)
}

/// Checks a fixture's declaration on its own, before anything is compiled.
fn declaration_problem(expectations: &[Expectation]) -> Option<String> {
    if expectations.is_empty() {
        return Some(
            "declares no expected diagnostic. Add `//~ E1234` and/or `//~ \"message text\"` \
             lines stating exactly what rustc must report."
                .to_owned(),
        );
    }

    let absent: Vec<String> = declared_codes(expectations)
        .into_iter()
        .filter(|code| ABSENCE_CODES.contains(&code.as_str()))
        .collect();
    let has_message = expectations
        .iter()
        .any(|expectation| matches!(expectation, Expectation::Message(_)));

    if !absent.is_empty() && !has_message {
        return Some(format!(
            "declares the absence code(s) {absent:?} but no `//~ \"...\"` message. A typo \
             produces those codes as readily as a real removal, so the fixture must also pin \
             the message naming the path that has to be missing."
        ));
    }

    None
}

/// Matches a fixture's declarations against what rustc actually reported.
fn unmet_expectations(expectations: &[Expectation], stderr: &str) -> Vec<String> {
    let mut problems = Vec::new();

    let declared = declared_codes(expectations);
    let observed = observed_codes(stderr);

    for code in declared.difference(&observed) {
        problems.push(format!("declared {code}, which rustc did not report"));
    }
    for code in observed.difference(&declared) {
        problems.push(format!(
            "rustc reported {code}, which the fixture does not declare"
        ));
    }
    for expectation in expectations {
        if let Expectation::Message(text) = expectation {
            if !stderr.contains(text.as_str()) {
                problems.push(format!(
                    "declared message {text:?}, which rustc did not report"
                ));
            }
        }
    }

    problems
}

fn declared_codes(expectations: &[Expectation]) -> BTreeSet<String> {
    expectations
        .iter()
        .filter_map(|expectation| match expectation {
            Expectation::Code(code) => Some(code.clone()),
            Expectation::Message(_) => None,
        })
        .collect()
}

/// Collects every `error[E1234]` code rustc printed.
fn observed_codes(stderr: &str) -> BTreeSet<String> {
    let mut codes = BTreeSet::new();
    for (offset, _) in stderr.match_indices("error[") {
        let rest = &stderr[offset + "error[".len()..];
        let Some(end) = rest.find(']') else { continue };
        let code = &rest[..end];
        if is_error_code(code) {
            codes.insert(code.to_owned());
        }
    }
    codes
}

fn is_error_code(value: &str) -> bool {
    let Some(digits) = value.strip_prefix('E') else {
        return false;
    };
    digits.len() == 4 && digits.bytes().all(|byte| byte.is_ascii_digit())
}

/// Reads a `"..."` literal, honouring `\"` and `\\`.
///
/// Returns `None` unless the literal is closed at the very end of the input.
fn unquote(value: &str) -> Option<String> {
    let mut characters = value.strip_prefix('"')?.chars();
    let mut text = String::new();
    loop {
        match characters.next()? {
            '\\' => text.push(characters.next()?),
            '"' => break,
            character => text.push(character),
        }
    }
    characters.next().is_none().then_some(text)
}

fn describe(expectations: &[Expectation]) -> String {
    expectations
        .iter()
        .map(|expectation| match expectation {
            Expectation::Code(code) => code.clone(),
            Expectation::Message(text) => format!("{text:?}"),
        })
        .collect::<Vec<_>>()
        .join(", ")
}

fn indent(text: &str) -> String {
    text.lines()
        .map(|line| format!("    {line}"))
        .collect::<Vec<_>>()
        .join("\n")
}

/// Whether a generated package is its own workspace or a member of the
/// generated case workspace.
#[derive(Clone, Copy, PartialEq, Eq)]
enum ManifestRoot {
    Standalone,
    Member,
}

fn case_package_name(index: usize, fixture_name: &str) -> String {
    format!(
        "grafton-visca-contract-{index:02}-{}",
        sanitize(fixture_name)
    )
}

fn case_manifest(
    crate_root: &Path,
    index: usize,
    fixture_name: &str,
    crate_features: &[&str],
    root: ManifestRoot,
) -> String {
    let workspace_table = match root {
        ManifestRoot::Standalone => "\n[workspace]\n",
        ManifestRoot::Member => "",
    };
    format!(
        r#"[package]
name = "{package_name}"
version = "0.0.0"
edition = "2021"
publish = false
{workspace_table}
[features]
default = {active_features}
blocking = []
async = []
runtime-tokio = []
runtime-smol = []
transport-serial = []
transport-serial-tokio = []
dyn-api = []
test-utils = []
serde = []
schemars = []
ts-rs = []

[dependencies]
grafton-visca = {{ path = {}, default-features = false, features = {} }}
"#,
        toml_string(&crate_root.to_string_lossy()),
        toml_array(crate_features),
        active_features = toml_array(crate_features),
        package_name = case_package_name(index, fixture_name),
    )
}

fn cargo_executable() -> OsString {
    env::var_os("CARGO").unwrap_or_else(|| OsString::from("cargo"))
}

fn toml_array(values: &[&str]) -> String {
    let values = values
        .iter()
        .map(|value| toml_string(value))
        .collect::<Vec<_>>();
    format!("[{}]", values.join(", "))
}

fn toml_string(value: &str) -> String {
    let mut output = String::from("\"");
    for character in value.chars() {
        match character {
            '\\' => output.push_str("\\\\"),
            '"' => output.push_str("\\\""),
            '\n' => output.push_str("\\n"),
            '\r' => output.push_str("\\r"),
            '\t' => output.push_str("\\t"),
            character => output.push(character),
        }
    }
    output.push('"');
    output
}

fn sanitize(value: &str) -> String {
    value
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() {
                character.to_ascii_lowercase()
            } else {
                '-'
            }
        })
        .collect()
}

/// Self-tests for the declaration reader and matcher.
///
/// These run in every test binary that includes this module, so the harness's
/// own break conditions — a fixture declaring nothing, a declared code that
/// stops matching, an undeclared code appearing, an absence code with no
/// message anchor — are checked without needing a deliberately broken fixture
/// on disk.
#[cfg(test)]
mod tests {
    use super::*;

    fn expectations(source: &str) -> Vec<Expectation> {
        parse_expectations(source).expect("fixture declaration parses")
    }

    #[test]
    fn reads_code_and_message_declarations() {
        let declared =
            expectations("fn main() {}\n\n//~ E0603\n//~ \"module `camera_id` is private\"\n");
        assert_eq!(
            declared,
            vec![
                Expectation::Code("E0603".to_owned()),
                Expectation::Message("module `camera_id` is private".to_owned()),
            ]
        );
    }

    #[test]
    fn build_directive_selects_a_full_build() {
        assert_eq!(compile_command("fn main() {}\n//~ E0080\n"), "check");
        assert_eq!(
            compile_command("//@ build\nfn main() {}\n//~ E0080\n"),
            "build"
        );
    }

    #[test]
    fn rejects_malformed_declarations() {
        assert!(parse_expectations("//~\n").is_err());
        assert!(parse_expectations("//~ E603\n").is_err());
        assert!(parse_expectations("//~ privacy error\n").is_err());
        assert!(parse_expectations("//~ \"unterminated\n").is_err());
        assert!(parse_expectations("//~ \"trailing\" junk\n").is_err());
    }

    #[test]
    fn undeclared_fixture_is_rejected_before_it_is_compiled() {
        assert!(declaration_problem(&[]).is_some());
    }

    #[test]
    fn absence_code_without_a_message_anchor_is_rejected() {
        assert!(declaration_problem(&[Expectation::Code("E0432".to_owned())]).is_some());
        assert!(declaration_problem(&[
            Expectation::Code("E0432".to_owned()),
            Expectation::Message("unresolved import `grafton_visca::CameraBuilder`".to_owned()),
        ])
        .is_none());
        // A privacy code is not an absence code: it cannot be produced by a
        // typo, so it stands on its own.
        assert!(declaration_problem(&[Expectation::Code("E0603".to_owned())]).is_none());
    }

    #[test]
    fn a_declared_code_that_stops_matching_fails() {
        let declared = [Expectation::Code("E0603".to_owned())];
        let rotted = "error[E0432]: unresolved import `grafton_visca::gone`\n";
        let problems = unmet_expectations(&declared, rotted);
        assert_eq!(problems.len(), 2, "{problems:?}");
        assert!(problems[0].contains("declared E0603"));
        assert!(problems[1].contains("rustc reported E0432"));
    }

    #[test]
    fn an_extra_undeclared_code_fails() {
        let declared = [Expectation::Code("E0603".to_owned())];
        let stderr = "error[E0603]: module `camera_id` is private\n\
                      error[E0433]: failed to resolve: could not find `gone`\n";
        let problems = unmet_expectations(&declared, stderr);
        assert_eq!(problems.len(), 1, "{problems:?}");
        assert!(problems[0].contains("rustc reported E0433"));
    }

    #[test]
    fn a_declared_message_that_stops_matching_fails() {
        let declared = [
            Expectation::Code("E0432".to_owned()),
            Expectation::Message("unresolved import `grafton_visca::CameraBuilder`".to_owned()),
        ];
        let renamed = "error[E0432]: unresolved import `grafton_visca::CameraFactory`\n";
        let problems = unmet_expectations(&declared, renamed);
        assert_eq!(problems.len(), 1, "{problems:?}");
        assert!(problems[0].contains("declared message"));
    }

    #[test]
    fn a_lint_only_fixture_matches_on_its_message_alone() {
        let declared = [Expectation::Message(
            "unused `Operation` that must be used".to_owned(),
        )];
        let stderr = "error: unused `Operation` that must be used\n";
        assert!(unmet_expectations(&declared, stderr).is_empty());
    }

    #[test]
    fn matching_declarations_pass() {
        let declared = [
            Expectation::Code("E0603".to_owned()),
            Expectation::Message("module `camera_id` is private".to_owned()),
        ];
        let stderr = "error[E0603]: module `camera_id` is private\n";
        assert!(unmet_expectations(&declared, stderr).is_empty());
    }

    #[test]
    fn target_root_unset_falls_back_to_crate_target() {
        let crate_root = Path::new("/workspace/grafton-visca");
        let current_dir = Path::new("/tmp/test-process");
        assert_eq!(
            compile_fail_target_root(crate_root, None, current_dir),
            crate_root.join("target")
        );
    }

    #[test]
    fn absolute_caller_target_dir_is_preserved() {
        let caller_target_dir = Path::new("/tmp/outer-target");
        assert_eq!(
            compile_fail_target_root(
                Path::new("/workspace/grafton-visca"),
                Some(caller_target_dir),
                Path::new("/tmp/test-process"),
            ),
            caller_target_dir
        );
    }

    #[test]
    fn relative_caller_target_dir_is_resolved_against_current_dir() {
        assert_eq!(
            compile_fail_target_root(
                Path::new("/workspace/grafton-visca"),
                Some(Path::new("outer-target")),
                Path::new("/tmp/test-process"),
            ),
            Path::new("/tmp/test-process/outer-target")
        );
    }

    #[test]
    fn process_ids_produce_distinct_generated_source_paths() {
        let target_root = Path::new("/tmp/outer-target");
        let fixture_dirs = ["tests/api_contract/fail"];
        let first = compile_fail_work_root(target_root, 101, &fixture_dirs);
        let second = compile_fail_work_root(target_root, 202, &fixture_dirs);
        assert_ne!(first, second);
        assert!(first.starts_with(target_root.join("contract-compile-fail")));
        assert!(second.starts_with(target_root.join("contract-compile-fail")));
    }

    #[test]
    fn nested_target_cache_is_stable_under_target_root() {
        let target_root = Path::new("/tmp/outer-target");
        let nested = compile_fail_nested_target_root(target_root);
        assert_eq!(nested, target_root.join("contract-compile-fail-target"));
        assert_eq!(nested, compile_fail_nested_target_root(target_root));
    }

    #[test]
    fn nested_cargo_commands_remove_inherited_compiler_flags() {
        let command = nested_cargo_command();
        for variable in [
            "RUSTFLAGS",
            "CARGO_ENCODED_RUSTFLAGS",
            "RUSTDOCFLAGS",
            "CARGO_ENCODED_RUSTDOCFLAGS",
        ] {
            assert!(
                command
                    .get_envs()
                    .any(|(name, value)| name == OsStr::new(variable) && value.is_none()),
                "nested Cargo command must explicitly remove {variable}"
            );
        }
    }

    fn message_record(target: &str, level: &str, rendered: &str) -> String {
        format!(
            "{{\"reason\":\"compiler-message\",\"package_id\":\"path+file:///w/{target}#{target}@0.0.0\",\
             \"target\":{{\"kind\":[\"bin\"],\"name\":\"{target}\",\"test\":true}},\
             \"message\":{{\"level\":\"{level}\",\"code\":null,\"rendered\":{}}}}}",
            serde_json::Value::from(rendered)
        )
    }

    fn artifact_record(target: &str) -> String {
        format!(
            "{{\"reason\":\"compiler-artifact\",\"target\":{{\"kind\":[\"bin\"],\"name\":\"{target}\"}},\
             \"profile\":{{\"opt_level\":\"0\",\"debuginfo\":0}},\"fresh\":true}}"
        )
    }

    #[test]
    fn cargo_messages_are_attributed_to_their_fixture_in_order() {
        let stream = [
            message_record("grafton_visca", "warning", "warning: library warning\n"),
            message_record(
                "pkg-a",
                "error",
                "error[E0603]: module `x` is private\n --> 00-a/src/main.rs:1:5\n\n",
            ),
            message_record(
                "pkg-b",
                "warning",
                "warning: unused\n --> 01-b/src/main.rs:2:1\n\n",
            ),
            artifact_record("pkg-b"),
            message_record(
                "pkg-a",
                "failure-note",
                "For more information about this error, try `rustc --explain E0603`.\n",
            ),
            "{\"reason\":\"build-finished\",\"success\":false}".to_owned(),
        ]
        .join("\n");

        let run = attribute_cargo_messages(&stream, &[("pkg-a", "00-a"), ("pkg-b", "01-b")])
            .expect("stream parses");

        assert_eq!(
            run.compilations,
            vec![
                FixtureCompilation {
                    compiled: false,
                    diagnostics: "error[E0603]: module `x` is private\n --> src/main.rs:1:5\n\n\
                                  For more information about this error, try `rustc --explain E0603`.\n"
                        .to_owned(),
                },
                FixtureCompilation {
                    compiled: true,
                    diagnostics: "warning: unused\n --> src/main.rs:2:1\n\n".to_owned(),
                },
            ]
        );
        assert_eq!(run.unattributed, "warning: library warning\n");
    }

    #[test]
    fn a_fixture_without_records_has_not_compiled() {
        let run = attribute_cargo_messages("", &[("pkg-a", "00-a")]).expect("empty stream");
        assert_eq!(run.compilations, vec![FixtureCompilation::default()]);
        assert!(attribute_cargo_messages("not json\n", &[]).is_err());
    }

    #[test]
    fn compiled_fail_fixture_and_failed_pass_fixture_are_reported() {
        let source = "fn main() {}\n//~ E0603\n";
        let compiled = FixtureCompilation {
            compiled: true,
            diagnostics: String::new(),
        };
        let failed = FixtureCompilation {
            compiled: false,
            diagnostics: "error[E0603]: module `x` is private\n".to_owned(),
        };
        let silent_failure = FixtureCompilation::default();

        assert!(check_fail_fixture("f.rs", source, &compiled, "")
            .unwrap_err()
            .contains("compiled successfully"));
        assert!(check_fail_fixture("f.rs", source, &failed, "").is_ok());
        assert!(check_pass_fixture("p.rs", &compiled, "").is_ok());
        assert!(check_pass_fixture("p.rs", &failed, "")
            .unwrap_err()
            .contains("error[E0603]"));
        assert!(
            check_pass_fixture("p.rs", &silent_failure, "error: manifest problem")
                .unwrap_err()
                .contains("error: manifest problem")
        );
    }

    #[test]
    fn member_manifests_leave_the_workspace_table_to_the_root() {
        let crate_root = Path::new("/workspace/grafton-visca");
        let standalone = case_manifest(crate_root, 3, "x", &["blocking"], ManifestRoot::Standalone);
        let member = case_manifest(crate_root, 3, "x", &["blocking"], ManifestRoot::Member);
        assert!(standalone.contains("\n[workspace]\n"));
        assert!(!member.contains("[workspace]"));
        assert!(member.contains("name = \"grafton-visca-contract-03-x\"\n"));
        assert_eq!(standalone.replace("\n[workspace]\n", ""), member);
    }
}
