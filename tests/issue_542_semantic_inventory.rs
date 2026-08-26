//! Source-level guards for the closed 2.0 semantic and noun ledgers.

use std::{fs, path::PathBuf};

fn source(relative: &str) -> String {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    fs::read_to_string(root.join(relative))
        .unwrap_or_else(|error| panic!("read {relative}: {error}"))
}

#[test]
fn semantic_ledger_is_single_source_and_class_balanced() {
    let semantics = source("src/command/semantics.rs");
    let surface = source("src/command/surface.rs");

    assert!(semantics.contains("pub enum BuiltinCommand"));
    assert!(semantics.contains("pub const ALL: &[Self]"));
    assert!(surface.contains("BuiltinCommand::ALL"));
    assert!(surface.contains("match command"));
    assert!(surface.contains("(plain, applied_only, targeted), (115, 16, 15)"));
    assert!(surface.contains("assert_eq!(noun_count, 143)"));

    // The source-level checks intentionally count only the closed ALL slice;
    // method matches below are allowed to contain exhaustive implementation
    // arms without becoming a second semantic inventory.
    let all = semantics
        .rsplit_once("pub const ALL: &[Self] = &[")
        .and_then(|(_, rest)| rest.split_once("    ];"))
        .map(|(slice, _)| slice.matches("Self::").count())
        .expect("BuiltinCommand::ALL slice");
    assert_eq!(all, 146, "semantic ALL inventory drifted");
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
    assert!(dynamic_nouns.contains("DYN_NOUN_TARGET_METHOD_COUNT: usize = 143"));
    assert!(dynamic_nouns.contains("DYN_NOUN_INQUIRY_METHOD_COUNT: usize = 66"));
}
