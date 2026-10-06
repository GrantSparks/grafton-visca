//! Source-level guards for the closed 2.0 semantic and noun ledgers.

use std::{fs, path::PathBuf};

fn source(relative: &str) -> String {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    fs::read_to_string(root.join(relative))
        .unwrap_or_else(|error| panic!("read {relative}: {error}"))
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
