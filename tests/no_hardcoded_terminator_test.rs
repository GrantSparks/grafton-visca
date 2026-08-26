//! Test to ensure no hardcoded 0xFF values exist in source code.
//!
//! This test scans the source code to verify that all VISCA terminators
//! use the VISCA_TERMINATOR constant rather than hardcoded 0xFF values.

use std::{fs, path::Path};

/// Patterns that are allowed to have 0xFF
const ALLOWED_PATTERNS: &[&str] = &[
    // The constant definition itself
    "const VISCA_TERMINATOR: u8 = 0xFF",
    // Error handling for unknown codes
    "Error::Unknown(0xFF)",
    // Documentation examples
    "/// ",
    "//! ",
    // Test value ranges that happen to include 0xFF
    "0x00, 0x55, 0xAA, 0xFF",
    // Protocol validation checks
    "!= 0xFF",
    "== 0xFF",
    // Comments
    "// ",
    // Error messages and debug output describing the terminator
    "missing 0xFF terminator",
    "terminator 0xFF",
    "VISCA command missing 0xFF",
    // Test data that includes terminator at end
    "0x08, 0xFF,",
];

/// Files that are allowed to have 0xFF for specific reasons
const ALLOWED_FILES: &[&str] = &[
    // Test files that define local constants
    "tests/terminator_validation_test.rs",
    "tests/no_hardcoded_terminator_test.rs",
    // Protocol validator that checks for terminators
    "tests/common/protocol_validator.rs",
    // Documentation examples
    "src/macros/test_utils.rs",
    // Protocol-aware frame reading tests need literal test data
    "src/transport/async_io.rs",
];

fn check_file_for_hardcoded_terminator(path: &Path) -> Vec<(usize, String)> {
    let content = fs::read_to_string(path).unwrap_or_default();
    let mut violations = Vec::new();

    // Skip allowed files
    let path_str = path.to_str().unwrap_or("");
    if ALLOWED_FILES
        .iter()
        .any(|&allowed| path_str.ends_with(allowed))
    {
        return violations;
    }

    for (line_num, line) in content.lines().enumerate() {
        if line.contains("0xFF") {
            // Check if this is an allowed pattern
            let is_allowed = ALLOWED_PATTERNS
                .iter()
                .any(|pattern| line.contains(pattern));

            if !is_allowed {
                // Check for specific patterns that indicate hardcoded terminators
                if line.contains(", 0xFF]")
                    || line.contains(", 0xFF,")
                    || line.contains("[0xFF")
                    || (line.contains("0xFF") && line.contains("termin"))
                {
                    violations.push((line_num + 1, line.to_string()));
                }
            }
        }
    }

    violations
}

fn check_directory_recursively(dir: &Path) -> Vec<(String, Vec<(usize, String)>)> {
    let mut all_violations = Vec::new();

    if let Ok(entries) = fs::read_dir(dir) {
        for entry in entries.flatten() {
            let path = entry.path();

            if path.is_dir() {
                // Skip target and .git directories
                if let Some(name) = path.file_name() {
                    if name == "target" || name == ".git" || name == ".github" {
                        continue;
                    }
                }
                all_violations.extend(check_directory_recursively(&path));
            } else if path.extension().and_then(|s| s.to_str()) == Some("rs") {
                let violations = check_file_for_hardcoded_terminator(&path);
                if !violations.is_empty() {
                    all_violations.push((path.to_string_lossy().to_string(), violations));
                }
            }
        }
    }

    all_violations
}

#[test]
fn test_no_hardcoded_terminators_in_source() {
    let workspace_root = Path::new(env!("CARGO_MANIFEST_DIR"));
    let src_dir = workspace_root.join("src");
    let macro_src_dir = workspace_root.join("grafton-visca-macros").join("src");

    let mut violations = check_directory_recursively(&src_dir);
    violations.extend(check_directory_recursively(&macro_src_dir));

    if !violations.is_empty() {
        let mut error_message = String::from(
            "\n\nFound hardcoded 0xFF values that should use VISCA_TERMINATOR constant:\n\n",
        );

        for (file, lines) in violations {
            error_message.push_str(&format!("File: {file}\n"));
            for (line_num, line) in lines {
                error_message.push_str(&format!("  Line {line_num}: {}\n", line.trim()));
            }
            error_message.push('\n');
        }

        error_message.push_str(
            "Please replace hardcoded 0xFF with VISCA_TERMINATOR constant.\n\
             Add the appropriate VISCA_TERMINATOR import or public command path if needed.\n",
        );

        panic!("{}", error_message);
    }
}

/// The terminator the library publishes must be the protocol's `0xFF`, and every
/// frame the library encodes must actually end with it exactly once.
#[test]
fn exported_terminator_matches_the_protocol_and_the_encoder() {
    use grafton_visca::{
        command::{PowerOn, ViscaCommand, Zoom, VISCA_TERMINATOR},
        CameraId,
    };

    assert_eq!(
        VISCA_TERMINATOR, 0xFF,
        "VISCA protocol requires the terminator to be 0xFF"
    );

    let mut buffer = [0u8; 32];

    let len = PowerOn::new()
        .write_into(CameraId::CAMERA_1, &mut buffer)
        .expect("PowerOn should encode");
    assert_eq!(
        buffer[len - 1],
        VISCA_TERMINATOR,
        "PowerOn must end with the exported terminator"
    );
    assert_eq!(
        buffer[..len]
            .iter()
            .filter(|&&b| b == VISCA_TERMINATOR)
            .count(),
        1,
        "PowerOn must contain exactly one terminator"
    );

    let len = Zoom::TeleStd
        .write_into(CameraId::CAMERA_1, &mut buffer)
        .expect("Zoom should encode");
    assert_eq!(
        buffer[len - 1],
        VISCA_TERMINATOR,
        "Zoom must end with the exported terminator"
    );
    assert_eq!(
        buffer[..len]
            .iter()
            .filter(|&&b| b == VISCA_TERMINATOR)
            .count(),
        1,
        "Zoom must contain exactly one terminator"
    );
}
