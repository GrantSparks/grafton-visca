//! Source-level guards for the closed 2.0 semantic and noun ledgers.

use std::{fs, path::PathBuf};

#[path = "common/source_scan.rs"]
mod source_scan;

use source_scan::{builtin_command_rows, declarations, inquiry_accessor_rows};

fn source(relative: &str) -> String {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    fs::read_to_string(root.join(relative))
        .unwrap_or_else(|error| panic!("read {relative}: {error}"))
}

/// Variants declared by the `BuiltinCommand` enum body.
fn ledger_variants(semantics: &str) -> usize {
    semantics
        .split_once("pub enum BuiltinCommand {")
        .and_then(|(_, rest)| rest.split_once("\n}"))
        .map(|(body, _)| {
            body.lines()
                .filter_map(|line| line.trim().strip_suffix(','))
                .filter(|name| {
                    name.starts_with(|ch: char| ch.is_ascii_uppercase())
                        && name.chars().all(char::is_alphanumeric)
                })
                .count()
        })
        .expect("BuiltinCommand enum body")
}

/// Reads `pub const NAME: usize = N;` without depending on its formatting.
fn declared_usize(source: &str, name: &str) -> usize {
    let needle = format!("{name}: usize");
    source
        .split_once(needle.as_str())
        .and_then(|(_, rest)| rest.split_once('='))
        .and_then(|(_, rest)| rest.split_once(';'))
        .and_then(|(value, _)| value.trim().parse().ok())
        .unwrap_or_else(|| panic!("no usize constant named {name}"))
}

#[test]
fn semantic_ledger_is_single_source_and_class_balanced() {
    let semantics_source = source("src/command/semantics.rs");
    let semantics = declarations(&semantics_source);
    let surface = declarations(&source("src/command/surface.rs"));

    assert!(semantics.contains("pub enum BuiltinCommand"));
    assert!(surface.contains("BuiltinCommand::ALL"));
    assert!(surface.contains("pub(crate) const fn surface_entry"));

    // The surface projection is generated from the shared noun table.  Keep
    // this check whitespace-tolerant: the consumer name and the `All` scope
    // are the contract, not the formatter's choice of spacing.
    let compact_surface: String = surface.split_whitespace().collect();
    assert_eq!(
        compact_surface
            .matches("noun_table!(All=>surface_rows_for_entry)")
            .count(),
        1,
        "surface_entry must consume the complete noun table"
    );

    // `surface_rows!` emits the command match, so the compiler—not a source
    // count—owns exhaustiveness.  There is no wildcard arm to hide a newly
    // added BuiltinCommand row, and duplicate generated arms are rejected by
    // the explicit unreachable-pattern lint on `surface_entry`.
    let surface_rows = surface
        .split_once("macro_rules! surface_rows")
        .and_then(|(_, rest)| rest.split_once("/// Derive the one surface row"))
        .map(|(body, _)| body)
        .expect("surface_rows macro body");
    let compact_surface_rows: String = surface_rows.split_whitespace().collect();
    assert!(compact_surface_rows.contains("match$input"));
    assert!(!compact_surface_rows.contains("_=>"));
    assert!(surface.contains("#[deny(unreachable_patterns)]"));

    // The semantic inventory remains the independent source of truth.  The
    // surface constants make the split readable without reproducing the noun
    // table here: every command is either target-facing or a protocol
    // exception.
    // `BuiltinCommand::ALL` is test-only audit data, so the declaration scan
    // intentionally removes it. Read the raw source only for this
    // source-level count; production declarations remain checked through the
    // enum and exhaustive classification below.
    let rows = builtin_command_rows(&semantics_source);
    let variants = ledger_variants(&semantics);
    assert_eq!(
        rows, variants,
        "BuiltinCommand::ALL must list every declared semantic row"
    );
    assert_eq!(
        declared_usize(&surface, "BUILTIN_COMMAND_COUNT"),
        rows,
        "readable surface total drifted from BuiltinCommand::ALL"
    );
    assert_eq!(
        declared_usize(&surface, "TARGET_FACING_COMMAND_COUNT")
            + declared_usize(&surface, "NON_NOUN_COMMAND_COUNT"),
        rows,
        "target-facing and protocol-exception totals must cover every semantic row"
    );
}

#[test]
fn removed_legacy_semantic_vocabulary_does_not_return() {
    for path in [
        "src/command/semantics.rs",
        "src/command/surface.rs",
        "src/lib.rs",
    ] {
        let text = source(path);
        for forbidden in [
            "PhysicalAxis",
            "PhysicalAxes",
            "PhysicalAxisSelection",
            "BUILTIN_CLASSIFICATION_LEDGER",
            "mode-async",
            "async-core",
        ] {
            assert!(
                !text.contains(forbidden),
                "removed vocabulary returned in {path}: {forbidden}"
            );
        }
    }
}

#[test]
fn static_and_dynamic_ledgers_reference_the_same_command_rows() {
    // Declaration regions only: each of these files names its own methods as
    // string literals inside its in-file inventory tests, so scanning the whole
    // file would let the test data satisfy the gate.
    let surface = declarations(&source("src/command/surface.rs"));
    let async_nouns = declarations(&source("src/async_nouns.rs"));
    let blocking_nouns = declarations(&source("src/blocking_nouns.rs"));
    let dynamic_nouns = declarations(&source("src/dynapi/nouns.rs"));
    let inquiry_structs = declarations(&source("src/command/inquiry_structs.rs"));

    for text in [&async_nouns, &blocking_nouns, &dynamic_nouns] {
        assert!(text.contains("execute"));
        assert!(text.contains("inquire"));
        assert!(text.contains("submit"));
    }
    assert!(surface.contains("StaticSurfaceDisposition::Noun"));

    // Both projections expose the same readable target-facing total.  The
    // semantic test above independently anchors that total to BuiltinCommand;
    // this assertion only checks that the dynamic projection did not drift.
    assert_eq!(
        declared_usize(&dynamic_nouns, "DYN_NOUN_TARGET_METHOD_COUNT"),
        declared_usize(&surface, "TARGET_FACING_COMMAND_COUNT"),
        "dynamic target-method count drifted from the static noun ledger"
    );
    // Both sides of the inquiry count used to come out of `dynapi/nouns.rs`
    // itself, which made the assertion self-referential.  The independent
    // source is the generated accessor table in `command/inquiry_structs.rs`
    // that `BUILTIN_INQUIRY_ACCESSORS` is built from; the crate's own
    // `dynamic_inquiry_count_follows_generated_typed_accessor_ledger` test in
    // `src/dynapi/nouns.rs` anchors the same constant against that slice at
    // run time, and this gate is the source-level half of it.
    assert_eq!(
        declared_usize(&dynamic_nouns, "DYN_NOUN_INQUIRY_METHOD_COUNT"),
        inquiry_accessor_rows(&inquiry_structs),
        "dynamic inquiry-method count drifted from the generated inquiry ledger"
    );
}
