//! Source-level guards for the closed 2.0 semantic and noun ledgers.

use std::{fs, path::PathBuf};

fn source(relative: &str) -> String {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    fs::read_to_string(root.join(relative))
        .unwrap_or_else(|error| panic!("read {relative}: {error}"))
}

/// Rows listed in the closed `BuiltinCommand::ALL` slice.
fn ledger_rows(semantics: &str) -> usize {
    semantics
        .rsplit_once("pub const ALL: &[Self] = &[")
        .and_then(|(_, rest)| rest.split_once("    ];"))
        .map(|(slice, _)| slice.matches("Self::").count())
        .expect("BuiltinCommand::ALL slice")
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

/// Entries declared inside the `inquiry_methods!` projection blocks.
fn dynamic_inquiry_methods(nouns: &str) -> usize {
    let mut total = 0;
    for block in nouns.split("inquiry_methods! {").skip(1) {
        let body = block.split_once("\n    }").map_or(block, |(body, _)| body);
        total += body
            .lines()
            .filter(|line| line.trim_start().starts_with("fn "))
            .count();
    }
    total
}

#[test]
fn semantic_ledger_is_single_source_and_class_balanced() {
    let semantics = source("src/command/semantics.rs");
    let surface = source("src/command/surface.rs");

    assert!(semantics.contains("pub enum BuiltinCommand"));
    assert!(semantics.contains("pub const ALL: &[Self]"));
    assert!(surface.contains("BuiltinCommand::ALL"));
    assert!(surface.contains("match command"));

    // Both sides of every equality below are counted from source, so the
    // ledger size is written down nowhere and reformatting cannot silently
    // satisfy the gate.  The closed `ALL` slice must list every declared
    // variant, and the surface match must give every listed row exactly one
    // disposition.
    let rows = ledger_rows(&semantics);
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
    let surface = source("src/command/surface.rs");
    let async_nouns = source("src/async_nouns.rs");
    let blocking_nouns = source("src/blocking_nouns.rs");
    let dynamic_nouns = source("src/dynapi/nouns.rs");

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
    assert_eq!(
        declared_usize(&dynamic_nouns, "DYN_NOUN_INQUIRY_METHOD_COUNT"),
        dynamic_inquiry_methods(&dynamic_nouns),
        "dynamic inquiry-method count drifted from the projected inquiries"
    );
}
