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
//! A fixture places each diagnostic expectation beside the line it covers:
//!
//! ```text
//! use grafton_visca::types::camera_id;
//! //~^ E0603 "module `camera_id` is private"
//! ```
//!
//! `//~^` covers the preceding line. Use more carets to move further back or
//! use `//~ L4` to name line 4 explicitly. The declaration requires the exact
//! error code (or no code for a lint or macro error) and can include a stable
//! substring of the diagnostic's own message. The quoted message supports
//! escaped quotes and backslashes.
//!
//! Every error on a primary line in the fixture needs a matching declaration.
//! The message anchor must not occur verbatim in the source outside its
//! declarations. Absence codes (see `ABSENCE_CODES`) require a message to
//! distinguish the intended missing path from a typo.
//!
//! Choose the diagnostic prose that stays stable across supported feature
//! shapes. Rustc may abbreviate a type path whenever it is unambiguous.
//!
//! A fixture whose contract is a post-monomorphization error adds the line
//! `//@ build`. Cargo checks do not instantiate generic code. For a const
//! evaluation error originating in a dependency, rustc's instantiation note
//! points to the triggering line in the fixture.
//!
//! Failures are collected across the whole run and reported together, so one
//! run tells you every fixture that needs attention.

use std::{
    collections::BTreeSet,
    env,
    ffi::OsString,
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
struct Expectation {
    line: usize,
    code: Option<String>,
    message: Option<String>,
}

impl Expectation {
    fn matches(&self, diagnostic: &ErrorDiagnostic, line: usize) -> bool {
        self.line == line
            && self.code == diagnostic.code
            && self
                .message
                .as_ref()
                .is_none_or(|text| diagnostic.message.contains(text))
    }
}

/// The grafton-visca features enabled in the test crate that expands it, as a
/// `Vec<&'static str>` to pass to the fixture harness.
///
/// A macro rather than a function so its `cfg!` checks read the features of
/// the integration test binary (grafton-visca's own) instead of this crate's.
#[macro_export]
macro_rules! active_grafton_visca_features {
    () => {{
        let mut features: ::std::vec::Vec<&'static str> = ::std::vec::Vec::new();
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
    }};
}

/// The grafton-visca package root: fixture paths are relative to it, and the
/// generated fixture crates depend on it by path.
///
/// This crate lives one directory below that root (`test-support/`).
fn grafton_visca_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .parent()
        .expect("the test-support crate lives inside the grafton-visca package root")
        .to_path_buf()
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
    let crate_root = grafton_visca_root();
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
    let crate_root = grafton_visca_root();
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
    let crate_root = grafton_visca_root();
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
    let crate_root = grafton_visca_root();
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
    errors: Vec<ErrorDiagnostic>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct ErrorDiagnostic {
    primary_lines: BTreeSet<usize>,
    code: Option<String>,
    message: String,
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
                let message = &record["message"];
                let rendered = message["rendered"].as_str().unwrap_or_default();
                match slot {
                    Some(slot) => {
                        let member_source = format!("{}/src/", packages[slot].1);
                        let compilation = &mut compilations[slot];
                        compilation
                            .diagnostics
                            .push_str(&rendered.replace(&member_source, "src/"));
                        let source_path = format!("{member_source}main.rs");
                        match message["level"].as_str() {
                            Some("error") => compilation.errors.push(ErrorDiagnostic {
                                primary_lines: primary_fixture_lines(message, &source_path),
                                code: message["code"]["code"]
                                    .as_str()
                                    .filter(|code| is_error_code(code))
                                    .map(str::to_owned),
                                message: message["message"].as_str().unwrap_or_default().to_owned(),
                            }),
                            Some("note") => {
                                bind_instantiation_note(compilation, message, &source_path)
                            }
                            _ => {}
                        }
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

fn primary_fixture_lines(diagnostic: &serde_json::Value, source_path: &str) -> BTreeSet<usize> {
    diagnostic["spans"]
        .as_array()
        .into_iter()
        .flatten()
        .filter(|span| span["is_primary"] == true)
        .filter_map(|span| {
            // A primary span inside an expansion belongs to the fixture
            // through its invocation, even if the macro lives elsewhere.
            let span = std::iter::successors(Some(span), |span| span.get("expansion")?.get("span"))
                .find(|span| {
                    span["file_name"].as_str().is_some_and(|file| {
                        let file = file.replace('\\', "/");
                        file == source_path
                            || file
                                .strip_suffix(source_path)
                                .is_some_and(|prefix| prefix.ends_with('/'))
                    })
                })?;
            usize::try_from(span["line_start"].as_u64()?).ok()
        })
        .collect()
}

fn bind_instantiation_note(
    compilation: &mut FixtureCompilation,
    note: &serde_json::Value,
    source_path: &str,
) {
    // For an inline const assertion in a dependency, rustc emits a separate
    // note identifying the call that instantiated the invalid expression.
    let Some(error) = compilation.errors.last_mut() else {
        return;
    };
    if error.code.as_deref() == Some("E0080")
        && error.primary_lines.is_empty()
        && note["message"].as_str().is_some_and(|message| {
            message.starts_with("the above error was encountered while instantiating ")
        })
    {
        error.primary_lines = primary_fixture_lines(note, source_path);
    }
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
    if compilation.compiled {
        return Err(format!(
            "{fixture_label}: compiled successfully; the contract it pins is no longer enforced \
             (expected {})",
            describe(&expectations),
        ));
    }

    let stderr = compilation.diagnostics.as_str();
    let problems = unmet_expectations(&expectations, &compilation.errors);
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
    // Internal contract fixtures include their actual production trait source
    // without exposing a public test-only API or depending on target layout.
    command.env("GRAFTON_VISCA_SOURCE_ROOT", grafton_visca_root());
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

/// Reads and validates the fixture's line-bound diagnostic declarations.
fn parse_expectations(source: &str) -> Result<Vec<Expectation>, String> {
    let lines: Vec<_> = source.lines().map(str::trim).collect();
    let mut expectations = Vec::new();

    for (offset, content) in lines.iter().enumerate() {
        let Some(rest) = content.strip_prefix("//~") else {
            continue;
        };
        let number = offset + 1;
        let rest = rest.trim_start();
        let relative = rest.bytes().take_while(|&byte| byte == b'^').count();
        let (line, payload) = if relative > 0 {
            (number.checked_sub(relative), rest[relative..].trim_start())
        } else if let Some(explicit) = rest.strip_prefix('L') {
            let (digits, payload) = explicit
                .split_once(char::is_whitespace)
                .unwrap_or((explicit, ""));
            (digits.parse().ok(), payload.trim_start())
        } else {
            return Err(format!(
                "line {number}: expected a `//~^` or `//~ L4` location"
            ));
        };
        let line = line
            .filter(|line| {
                line.checked_sub(1)
                    .and_then(|offset| lines.get(offset))
                    .is_some_and(|target| !target.is_empty() && !target.starts_with("//~"))
            })
            .ok_or_else(|| format!("line {number}: expectation must point to a source line"))?;

        let (code, quoted) = if payload.starts_with('"') {
            (None, payload)
        } else {
            let (code, quoted) = payload
                .split_once(char::is_whitespace)
                .unwrap_or((payload, ""));
            if !is_error_code(code) {
                return Err(format!(
                    "line {number}: expected E1234 or a quoted diagnostic message"
                ));
            }
            (Some(code.to_owned()), quoted.trim_start())
        };
        let message = if quoted.is_empty() {
            None
        } else {
            let message = unquote(quoted)
                .filter(|text| !text.is_empty())
                .ok_or_else(|| {
                    format!("line {number}: expected a nonempty quoted diagnostic message")
                })?;
            if lines
                .iter()
                .any(|line| !line.starts_with("//~") && line.contains(&message))
            {
                return Err(format!(
                    "line {number}: diagnostic anchor {message:?} occurs in fixture source"
                ));
            }
            Some(message)
        };
        if message.is_none()
            && code
                .as_deref()
                .is_some_and(|code| ABSENCE_CODES.contains(&code))
        {
            return Err(format!(
                "line {number}: absence code requires a message naming the missing path"
            ));
        }

        expectations.push(Expectation {
            line,
            code,
            message,
        });
    }

    if expectations.is_empty() {
        return Err("declares no expected diagnostic. Add `//~^ E1234` and/or a quoted message beside the error.".to_owned());
    }
    Ok(expectations)
}

/// Requires matching evidence in both directions, at every primary fixture line.
fn unmet_expectations(expectations: &[Expectation], errors: &[ErrorDiagnostic]) -> Vec<String> {
    let mut problems = Vec::new();

    for expectation in expectations {
        if !errors.iter().any(|error| {
            error.primary_lines.contains(&expectation.line)
                && expectation.matches(error, expectation.line)
        }) {
            problems.push(format!(
                "rustc did not report {}",
                describe_expectation(expectation)
            ));
        }
    }
    for error in errors {
        let code = error.code.as_deref().unwrap_or("error");
        if error.primary_lines.is_empty() {
            problems.push(format!(
                "{code} has no primary fixture line: {}",
                error.message
            ));
        }
        for &line in &error.primary_lines {
            if !expectations
                .iter()
                .any(|expectation| expectation.matches(error, line))
            {
                problems.push(format!("line {line}: undeclared {code}: {}", error.message));
            }
        }
    }

    problems
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

fn describe_expectation(expectation: &Expectation) -> String {
    let code = expectation.code.as_deref().unwrap_or("uncoded error");
    match &expectation.message {
        Some(message) => format!("line {}: {code} containing {message:?}", expectation.line),
        None => format!("line {}: {code}", expectation.line),
    }
}

fn describe(expectations: &[Expectation]) -> String {
    expectations
        .iter()
        .map(describe_expectation)
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
    use std::ffi::OsStr;

    use super::*;

    fn expectations(source: &str) -> Vec<Expectation> {
        parse_expectations(source).expect("fixture declaration parses")
    }

    #[test]
    fn reads_relative_and_explicit_declarations() {
        let declared = expectations(
            "use grafton_visca::types::camera_id;\n\n//~^^ E0603 \"module `camera_id` is private\"\n",
        );
        assert_eq!(
            declared,
            expectations(
                "use grafton_visca::types::camera_id;\n//~ L1 E0603 \"module `camera_id` is private\"\n"
            )
        );
        assert_eq!(
            declared,
            vec![Expectation {
                line: 1,
                code: Some("E0603".to_owned()),
                message: Some("module `camera_id` is private".to_owned()),
            }]
        );
    }

    #[test]
    fn build_directive_selects_a_full_build() {
        assert_eq!(compile_command("fn main() {}\n//~^ E0080\n"), "check");
        assert_eq!(
            compile_command("//@ build\nfn main() {}\n//~^ E0080\n"),
            "build"
        );
    }

    #[test]
    fn rejects_malformed_declarations() {
        for malformed in [
            "//~",
            "//~^",
            "//~^ E603",
            "//~^ privacy error",
            "//~^ \"unterminated",
            "//~^ \"trailing\" junk",
            "//~^^ E0603",
            "//~ L3 E0603",
        ] {
            let source = format!("fn main() {{}}\n{malformed}\n");
            assert!(parse_expectations(&source).is_err(), "accepted {malformed}");
        }
    }

    #[test]
    fn undeclared_fixture_has_no_contract() {
        assert!(parse_expectations("fn main() {}").is_err());
    }

    #[test]
    fn absence_code_without_a_message_anchor_is_rejected() {
        assert!(parse_expectations("use example::missing;\n//~^ E0432\n").is_err());
        assert!(parse_expectations(
            "use example::missing;\n//~^ E0432 \"unresolved import `example::missing`\"\n"
        )
        .is_ok());
        assert!(parse_expectations("use example::private;\n//~^ E0603\n").is_ok());
    }

    #[test]
    fn an_anchor_copied_from_the_source_is_rejected() {
        let problem =
            parse_expectations("use example::missing;\n//~^ E0432 \"example::missing\"\n")
                .unwrap_err();
        assert!(problem.contains("occurs in fixture source"));
    }

    fn error(lines: &[usize], code: Option<&str>, message: &str) -> ErrorDiagnostic {
        ErrorDiagnostic {
            primary_lines: lines.iter().copied().collect(),
            code: code.map(str::to_owned),
            message: message.to_owned(),
        }
    }

    #[test]
    fn message_and_code_must_match_the_same_diagnostic_on_the_declared_line() {
        let declared =
            expectations("use example::private;\n//~^ E0603 \"module `private` is private\"\n");
        let correct = error(&[1], Some("E0603"), "module `private` is private");
        assert!(unmet_expectations(&declared, std::slice::from_ref(&correct)).is_empty());
        for mismatch in [
            error(&[2], Some("E0603"), &correct.message),
            error(&[1], Some("E0432"), &correct.message),
            error(&[1], Some("E0603"), "module `wrong` is private"),
        ] {
            let problems = unmet_expectations(&declared, &[mismatch]);
            assert_eq!(problems.len(), 2, "{problems:?}");
            assert!(problems[0].contains("rustc did not report line 1: E0603"));
            assert!(problems[1].contains("undeclared"));
        }
    }

    #[test]
    fn every_primary_fixture_line_must_be_declared() {
        let declared = expectations("first();\n//~^ E0308\nsecond();\n");
        let single = error(&[1], Some("E0308"), "mismatched types");
        let extra = error(&[3], Some("E0308"), "mismatched types");
        let problems = unmet_expectations(&declared, &[single, extra]);
        assert_eq!(problems, ["line 3: undeclared E0308: mismatched types"]);

        let multi_span = error(&[1, 3], Some("E0308"), "mismatched types");
        assert_eq!(
            unmet_expectations(&declared, std::slice::from_ref(&multi_span)),
            ["line 3: undeclared E0308: mismatched types"]
        );
        let both_lines = expectations("first();\n//~^ E0308\nsecond();\n//~^ E0308\n");
        assert!(unmet_expectations(&both_lines, &[multi_span]).is_empty());
    }

    #[test]
    fn a_lint_only_fixture_matches_on_its_message_alone() {
        let declared = expectations("operation();\n//~^ \"unused operation\"\n");
        let lint = error(&[1], None, "unused operation");
        assert!(unmet_expectations(&declared, &[lint]).is_empty());
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
                    errors: vec![error(&[], None, "")],
                },
                FixtureCompilation {
                    compiled: true,
                    diagnostics: "warning: unused\n --> src/main.rs:2:1\n\n".to_owned(),
                    errors: Vec::new(),
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
        let source = "use example::private;\n//~^ E0603\n";
        let compiled = FixtureCompilation {
            compiled: true,
            diagnostics: String::new(),
            errors: Vec::new(),
        };
        let failed = FixtureCompilation {
            compiled: false,
            diagnostics: "error[E0603]: module `x` is private\n".to_owned(),
            errors: vec![error(&[1], Some("E0603"), "module `x` is private")],
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
