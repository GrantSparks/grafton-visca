//! Tests to ensure error mapping consistency across all layers.

use grafton_visca::Error;

#[test]
fn test_error_code_consistency() {
    // Test that all VISCA error codes map correctly and consistently
    let test_cases = [
        (0x01, "MessageLengthError"),
        (0x02, "SyntaxError"),
        (0x03, "CommandBufferFull"),
        (0x04, "CommandCanceled"),
        (0x05, "NoSocket"),
        (0x41, "CommandNotExecutable"),
    ];

    for (code, expected_name) in test_cases {
        let error = Error::from_code(code);

        // Verify the error type is correct
        match code {
            0x01 => assert!(
                matches!(error, Error::MessageLengthError),
                "Code 0x{code:02X} should map to {expected_name}"
            ),
            0x02 => assert!(
                matches!(error, Error::SyntaxError),
                "Code 0x{code:02X} should map to {expected_name}"
            ),
            0x03 => assert!(
                matches!(error, Error::CommandBufferFull),
                "Code 0x{code:02X} should map to {expected_name}"
            ),
            0x04 => assert!(
                matches!(error, Error::CommandCanceled),
                "Code 0x{code:02X} should map to {expected_name}"
            ),
            0x05 => assert!(
                matches!(error, Error::NoSocket),
                "Code 0x{code:02X} should map to {expected_name}"
            ),
            0x41 => assert!(
                matches!(error, Error::CommandNotExecutable),
                "Code 0x{code:02X} should map to {expected_name}"
            ),
            _ => panic!("Unknown test case"),
        }
    }
}

#[test]
fn recovery_distinguishes_capacity_acceptance_and_uncertainty() {
    use grafton_visca::Recovery;
    use std::time::Duration;
    for (error, expected) in [
        (
            Error::transport_error(std::io::Error::other("send")),
            Recovery::Resubmit {
                after: Duration::from_millis(50),
            },
        ),
        (Error::CommandPending, Recovery::WaitAgain),
        (
            Error::CommandBufferFull,
            Recovery::Resubmit {
                after: Duration::from_millis(200),
            },
        ),
        (Error::io_timeout(), Recovery::Reconcile),
        (Error::SyntaxError, Recovery::Abandon),
        (Error::OwnerTerminated, Recovery::ReopenSession),
    ] {
        assert_eq!(error.recovery(), expected);
    }
}

#[test]
fn test_error_code_round_trip() {
    // Test that error codes can round-trip through from_code
    let test_codes = [0x01, 0x02, 0x03, 0x04, 0x05, 0x41, 0xFF];

    for code in test_codes {
        let error = Error::from_code(code);

        // For known error codes, verify they map to the correct variant
        match code {
            0x02 => assert!(matches!(error, Error::SyntaxError)),
            0x03 => assert!(matches!(error, Error::CommandBufferFull)),
            0x04 => assert!(matches!(error, Error::CommandCanceled)),
            0x05 => assert!(matches!(error, Error::NoSocket)),
            0x41 => assert!(matches!(error, Error::CommandNotExecutable)),
            _ => assert!(matches!(
                error,
                Error::Unknown(_) | Error::MessageLengthError
            )),
        }
    }
}

#[cfg(feature = "async")]
#[test]
fn test_internal_external_error_consistency() {
    // Test that the public Error type handles all VISCA error codes consistently
    // This ensures the runtime can convert error codes correctly
    let test_codes = [0x01, 0x02, 0x03, 0x04, 0x05, 0x41];

    for code in test_codes {
        let public_error = Error::from_code(code);

        // Verify that specific error codes have expected retryability
        match code {
            0x03 | 0x05 => {
                // Buffer full (0x03) and no socket (0x05) are transient capacity — retryable
                assert!(
                    matches!(
                        public_error.recovery(),
                        grafton_visca::Recovery::Resubmit { .. }
                    ),
                    "Error for 0x{code:02X} should be retryable"
                );
            }
            0x01 | 0x02 | 0x04 => {
                // These should not be retryable
                assert!(
                    !matches!(
                        public_error.recovery(),
                        grafton_visca::Recovery::Resubmit { .. }
                    ),
                    "Error for 0x{code:02X} should not be retryable"
                );
            }
            0x41 => {
                // CommandNotExecutable is context-dependent but base error is not retryable
                assert!(
                    !matches!(
                        public_error.recovery(),
                        grafton_visca::Recovery::Resubmit { .. }
                    ),
                    "Error for 0x{code:02X} (CommandNotExecutable) base should not be retryable"
                );
            }
            _ => {}
        }
    }
}

#[test]
fn test_unknown_error_handling() {
    // Test that unknown error codes are handled consistently
    let unknown_codes = [
        0x00, 0x06, 0x10, 0x20, 0x30, 0x40, 0x42, 0x50, 0x60, 0x70, 0x80, 0x90, 0xA0, 0xB0, 0xC0,
        0xD0, 0xE0, 0xF0, 0xFE, 0xFF,
    ];

    for code in unknown_codes {
        let error = Error::from_code(code);

        // Most codes should map to Unknown, except 0x01 which maps to MessageLengthError
        if code == 0x01 {
            assert!(
                matches!(error, Error::MessageLengthError),
                "Code 0x{code:02X} should map to MessageLengthError"
            );
        } else if code == 0x00 || code > 0x05 && code != 0x41 {
            assert!(
                matches!(error, Error::Unknown(c) if c == code),
                "Code 0x{code:02X} should map to Unknown(0x{code:02X})"
            );
        }
    }
}

#[test]
fn test_error_display_messages() {
    // Test that error messages are consistent and informative

    let test_cases = [
        (Error::SyntaxError, "Syntax error in VISCA command"),
        (Error::CommandBufferFull, "Command buffer is full"),
        (Error::CommandCanceled, "Command was canceled"),
        (Error::NoSocket, "No socket available"),
        (Error::CommandNotExecutable, "Command is not executable"),
        (Error::MessageLengthError, "Message length error"),
    ];

    for (error, expected_msg) in test_cases {
        let msg = error.to_string();
        assert_eq!(msg, expected_msg, "Error message mismatch for {error:?}");
    }
}
