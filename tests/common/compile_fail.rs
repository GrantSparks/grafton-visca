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
//! * Put the declaration block at the end of the fixture. Several fixtures
//!   also have trybuild `.stderr` snapshots whose line numbers point into the
//!   fixture source, and a declaration above the code would shift every one of
//!   them.
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

use std::{
    collections::BTreeSet,
    env,
    ffi::OsString,
    fmt::Write as _,
    fs,
    path::{Path, PathBuf},
    process::Command,
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
    features
}

pub fn assert_compile_fail_fixtures(fixture_dirs: &[&str], crate_features: &[&str]) {
    let crate_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let fixtures = collect_fixtures(&crate_root, fixture_dirs);
    assert!(
        !fixtures.is_empty(),
        "no compile-fail fixtures found in {:?}",
        fixture_dirs
    );

    let work_root = crate_root
        .join("target")
        .join("contract-compile-fail")
        .join(format!(
            "{}-{}",
            std::process::id(),
            sanitize(&fixture_dirs.join("-"))
        ));
    if work_root.exists() {
        fs::remove_dir_all(&work_root).unwrap_or_else(|error| {
            panic!(
                "failed to remove old compile-fail work dir {}: {error}",
                work_root.display()
            )
        });
    }
    fs::create_dir_all(&work_root).unwrap_or_else(|error| {
        panic!(
            "failed to create compile-fail work dir {}: {error}",
            work_root.display()
        )
    });

    prefetch_contract_dependencies(&crate_root, &work_root, crate_features);

    let mut failures = Vec::new();
    for (index, fixture) in fixtures.iter().enumerate() {
        if let Err(failure) = check_fixture(&crate_root, &work_root, index, fixture, crate_features)
        {
            failures.push(failure);
        }
    }

    assert!(
        failures.is_empty(),
        "{} of {} compile-fail fixture(s) did not match their declared diagnostics \
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

fn prefetch_contract_dependencies(crate_root: &Path, work_root: &Path, crate_features: &[&str]) {
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
        case_manifest(crate_root, 0, "prefetch-dependencies", crate_features),
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

    let mut command = Command::new(cargo_executable());
    command
        .arg("fetch")
        .arg("--quiet")
        .arg("--manifest-path")
        .arg(prefetch_dir.join("Cargo.toml"))
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

/// Compiles one fixture and matches the result against its declarations.
///
/// Returns the report for a fixture that did not meet them; harness-level
/// problems (unreadable files, an unspawnable cargo) still panic outright.
fn check_fixture(
    crate_root: &Path,
    work_root: &Path,
    index: usize,
    fixture: &Path,
    crate_features: &[&str],
) -> Result<(), String> {
    let fixture_name = fixture
        .file_stem()
        .and_then(|stem| stem.to_str())
        .unwrap_or("fixture");
    let fixture_label = fixture
        .strip_prefix(crate_root)
        .unwrap_or(fixture)
        .display()
        .to_string();

    let source = fs::read_to_string(fixture)
        .unwrap_or_else(|error| panic!("failed to read fixture {}: {error}", fixture.display()));

    let expectations = match parse_expectations(&source) {
        Ok(expectations) => expectations,
        Err(problem) => {
            return Err(format!("{fixture_label}: {problem}"));
        }
    };
    if let Some(problem) = declaration_problem(&expectations) {
        return Err(format!("{fixture_label}: {problem}"));
    }

    let case_dir = work_root.join(format!("{index:02}-{}", sanitize(fixture_name)));
    let src_dir = case_dir.join("src");
    fs::create_dir_all(&src_dir).unwrap_or_else(|error| {
        panic!(
            "failed to create compile-fail case dir {}: {error}",
            src_dir.display()
        )
    });
    fs::write(src_dir.join("main.rs"), &source).unwrap_or_else(|error| {
        panic!(
            "failed to write compile-fail case source for {}: {error}",
            fixture.display()
        )
    });
    fs::write(
        case_dir.join("Cargo.toml"),
        case_manifest(crate_root, index, fixture_name, crate_features),
    )
    .unwrap_or_else(|error| {
        panic!(
            "failed to write compile-fail case manifest for {}: {error}",
            fixture.display()
        )
    });

    let output = Command::new(cargo_executable())
        .arg("check")
        .arg("--offline")
        .arg("--quiet")
        .arg("--manifest-path")
        .arg(case_dir.join("Cargo.toml"))
        .env(
            "CARGO_TARGET_DIR",
            crate_root
                .join("target")
                .join("contract-compile-fail-target"),
        )
        .output()
        .unwrap_or_else(|error| {
            panic!(
                "failed to run cargo check for {}: {error}",
                fixture.display()
            )
        });

    if output.status.success() {
        return Err(format!(
            "{fixture_label}: compiled successfully; the contract it pins is no longer enforced \
             (expected {})",
            describe(&expectations),
        ));
    }

    let stderr = String::from_utf8_lossy(&output.stderr);
    let problems = unmet_expectations(&expectations, &stderr);
    if problems.is_empty() {
        return Ok(());
    }

    let mut report = format!("{fixture_label}:");
    for problem in &problems {
        let _ = write!(report, "\n  - {problem}");
    }
    let _ = write!(report, "\n  declared: {}", describe(&expectations));
    let _ = write!(report, "\n  stderr:\n{}", indent(&stderr));
    Err(report)
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

fn case_manifest(
    crate_root: &Path,
    index: usize,
    fixture_name: &str,
    crate_features: &[&str],
) -> String {
    format!(
        r#"[package]
name = "grafton-visca-contract-{index:02}-{fixture_name}"
version = "0.0.0"
edition = "2021"
publish = false

[workspace]

[dependencies]
grafton-visca = {{ path = {}, default-features = false, features = {} }}
"#,
        toml_string(&crate_root.to_string_lossy()),
        toml_array(crate_features),
        fixture_name = sanitize(fixture_name),
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
}
