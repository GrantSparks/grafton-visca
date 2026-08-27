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

/// One surface disposition per semantic row, counted from the macro calls.
fn surface_dispositions(surface: &str) -> usize {
    surface.matches("noun_entry!(").count()
        + surface.matches("broadcast_entry!(").count()
        + surface.matches("internal_entry!(").count()
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
    let semantics = declarations(&source("src/command/semantics.rs"));
    let surface = declarations(&source("src/command/surface.rs"));

    assert!(semantics.contains("pub enum BuiltinCommand"));
    assert!(semantics.contains("pub const ALL: &[Self]"));
    assert!(surface.contains("BuiltinCommand::ALL"));
    assert!(surface.contains("match command"));

    // Both sides of every equality below are counted from source, so the
    // ledger size is written down nowhere and reformatting cannot silently
    // satisfy the gate.  The closed `ALL` slice must list every declared
    // variant, and the surface match must give every listed row exactly one
    // disposition.
    let rows = builtin_command_rows(&semantics);
    assert_eq!(
        rows,
        ledger_variants(&semantics),
        "BuiltinCommand::ALL must list every declared semantic row"
    );
    assert_eq!(
        surface_dispositions(&surface),
        rows,
        "every semantic row needs exactly one surface disposition"
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

    // Derived, not string-matched: the dynamic projection must declare one
    // command method per target-facing ledger row and one inquiry method per
    // projected inquiry, whatever those totals happen to be.
    assert_eq!(
        declared_usize(&dynamic_nouns, "DYN_NOUN_TARGET_METHOD_COUNT"),
        surface.matches("noun_entry!(").count(),
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
