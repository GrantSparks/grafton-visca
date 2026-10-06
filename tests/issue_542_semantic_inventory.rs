//! Source-level guards for the closed 2.0 semantic and noun ledgers.

use std::{fs, path::PathBuf};

#[path = "common/source_scan.rs"]
mod source_scan;

use source_scan::declarations;

fn source(relative: &str) -> String {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    fs::read_to_string(root.join(relative))
        .unwrap_or_else(|error| panic!("read {relative}: {error}"))
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
    // One consumer generates both static facades (blocking and async), so
    // its expansion is where their owner hops are spelled.
    let static_nouns = declarations(&source("src/noun_facade.rs"));
    let dynamic_nouns = declarations(&source("src/dynapi/nouns.rs"));

    for text in [&static_nouns, &dynamic_nouns] {
        assert!(text.contains("execute"));
        assert!(text.contains("inquire"));
        assert!(text.contains("submit"));
    }
    assert!(surface.contains("StaticSurfaceDisposition::Noun"));

    // Both projections expose the same readable target-facing total; this
    // assertion only checks that the dynamic projection did not drift.
    assert_eq!(
        declared_usize(&dynamic_nouns, "DYN_NOUN_TARGET_METHOD_COUNT"),
        declared_usize(&surface, "TARGET_FACING_COMMAND_COUNT"),
        "dynamic target-method count drifted from the static noun ledger"
    );
}
