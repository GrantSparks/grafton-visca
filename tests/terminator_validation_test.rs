//! Tests for VISCA terminator validation.
//!
//! These tests ensure that:
//! 1. No hardcoded 0xFF values exist in the codebase
//! 2. The terminator safety mechanisms are in place

/// Test that the no-hardcoded-terminator test exists and works
#[test]
fn test_no_hardcoded_terminator_test_exists() {
    // This verifies that the no_hardcoded_terminator_test.rs file exists
    // and is part of the test suite. The actual scanning is done by that test.
    use std::path::Path;

    let test_file = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests")
        .join("no_hardcoded_terminator_test.rs");

    assert!(
        test_file.exists(),
        "no_hardcoded_terminator_test.rs should exist"
    );
}


/// Test that the type-safe example exists
#[test]
fn test_type_safe_example_exists() {
    use std::fs;
    use std::path::Path;

    let example = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("examples")
        .join("type_safe_commands.rs");

    assert!(
        example.exists(),
        "type_safe_commands.rs example should exist"
    );

    // Verify it demonstrates the type-state pattern
    let content = fs::read_to_string(example).expect("Failed to read example");
    assert!(
        content.contains("type-state"),
        "Example should mention type-state pattern"
    );
    assert!(
        content.contains("VISCA_TERMINATOR"),
        "Example should mention VISCA_TERMINATOR"
    );
}

/// Test that contributing guidelines include safety practices
#[test]
fn test_contributing_guidelines_include_safety() {
    use std::fs;
    use std::path::Path;

    let contributing = Path::new(env!("CARGO_MANIFEST_DIR")).join("CONTRIBUTING.md");

    assert!(contributing.exists(), "CONTRIBUTING.md should exist");

    // Verify it contains safety practices
    let content = fs::read_to_string(contributing).expect("Failed to read contributing guide");
    assert!(
        content.contains("VISCA_TERMINATOR"),
        "Contributing guide should mention VISCA_TERMINATOR"
    );
    assert!(
        content.contains("CommandBuilder"),
        "Contributing guide should mention CommandBuilder"
    );
    let lower_content = content.to_lowercase();
    assert!(
        lower_content.contains("safety"),
        "Contributing guide should mention safety practices"
    );
}
