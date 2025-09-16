use thiserror::Error as ThisError;

use std::{borrow::Cow, convert::Infallible, io, time::Duration};

/// Custom result type for VISCA operations.
pub type Result<T, E = Error> = std::result::Result<T, E>;

/// VISCA protocol error type.
///
/// Provides comprehensive error handling for all VISCA operations.
/// The VISCA protocol has a well-defined set of error conditions
/// that map directly to camera responses and communication failures.
///
/// # Error Categories
///
/// Errors are categorized into two main types:
///
/// ## Retryable Errors
/// These errors indicate temporary conditions that may succeed on retry:
/// - `CameraBusy` - Camera is processing another command
/// - `CommandPending` - Command acknowledged but not yet complete
/// - `CameraMoving` - Camera is still moving to a position
/// - `CommandTimeout` - Operation exceeded timeout (may succeed with longer timeout)
/// - `CommandBufferFull` - Camera's command buffer is full (always retry)
/// - `Timeout` - General timeout condition
///
/// Use [`Error::is_retryable()`] to check if an error can be retried, and
/// [`Error::suggested_retry_delay()`] to get the recommended delay before retrying.
///
/// ## Non-Retryable Errors
/// These errors indicate permanent failures or invalid operations:
/// - `SyntaxError` - Invalid VISCA command format
/// - `CommandNotExecutable` - Command invalid in current state
/// - `InvalidParameter` - Parameter value is invalid
/// - `FeatureNotSupported` - Camera model doesn't support this feature
/// - `PresetNotFound` - Requested preset doesn't exist
///
/// # VISCA Error Codes
///
/// The VISCA protocol defines specific error codes that are mapped to Error variants:
/// - `0x01` → `MessageLengthError` - Message length incorrect
/// - `0x02` → `SyntaxError` - Command format invalid
/// - `0x03` → `CommandBufferFull` - Camera busy (always retryable)
/// - `0x04` → `CommandCanceled` - Command was canceled
/// - `0x05` → `NoSocket` - No socket available
/// - `0x41` → `CommandNotExecutable` - Command invalid in current state
///
/// Use [`Error::from_code()`] to convert VISCA error codes to Error variants.
///
/// # Example
///
/// ```rust
/// use grafton_visca::Error;
/// use std::time::Duration;
///
/// fn handle_camera_error(error: Error) -> Result<(), Error> {
///     if error.is_retryable() {
///         if let Some(delay) = error.suggested_retry_delay() {
///             println!("Retrying after {:?}", delay);
///             std::thread::sleep(delay);
///             // Retry the operation...
///         }
///     } else {
///         // Handle permanent error
///         return Err(error);
///     }
///     Ok(())
/// }
/// ```
#[derive(ThisError, Debug)]
pub enum Error {
    /// Failed to establish connection to the camera.
    #[error("Connection failed to {addr}: {source}")]
    ConnectionFailed {
        /// The address that failed to connect.
        addr: Cow<'static, str>,
        /// The underlying IO error.
        source: io::Error,
    },

    /// Connection to the camera was closed.
    #[error("Connection closed{}", reason.as_ref().map(|r| format!(": {r}")).unwrap_or_default())]
    ConnectionClosed {
        /// Optional reason for the connection closure.
        reason: Option<Cow<'static, str>>,
    },

    /// Command execution exceeded the configured timeout.
    #[error("Command timeout after {duration:?} for command: {command}")]
    CommandTimeout {
        /// Duration of the timeout.
        duration: Duration,
        /// Description of the command that timed out.
        command: Cow<'static, str>,
    },

    /// Camera is busy executing another command and cannot accept new commands.
    #[error("Camera is busy executing another command")]
    CameraBusy,

    /// Command has been acknowledged but is still pending completion.
    /// This is returned when an ACK is received, indicating the command
    /// was queued but not yet executed.
    #[error("Command acknowledged and pending completion")]
    CommandPending,

    /// Camera is still performing a mechanical movement operation.
    #[error("Camera is still moving, position: pan={pan}, tilt={tilt}")]
    CameraMoving {
        /// Current pan position.
        pan: i16,
        /// Current tilt position.
        tilt: i16,
    },

    /// Camera has not been properly initialized or powered on.
    #[error("Camera not initialized")]
    CameraNotReady,

    /// Response from camera doesn't match the expected format.
    #[error("Invalid response: expected {expected}, got {actual:?}")]
    InvalidResponse {
        /// Description of expected response.
        expected: Cow<'static, str>,
        /// Actual bytes received.
        actual: Vec<u8>,
    },

    /// Camera explicitly rejected the command.
    #[error("Command rejected by camera: {reason}")]
    CommandRejected {
        /// Reason for rejection.
        reason: Cow<'static, str>,
    },

    /// Requested preset position does not exist.
    #[error("Preset {id} not found")]
    PresetNotFound {
        /// ID of the missing preset.
        id: u8,
    },

    /// Camera model doesn't support the requested feature.
    #[error("Feature '{feature}' not supported by this camera model")]
    FeatureNotSupported {
        /// Name of the unsupported feature.
        feature: &'static str,
    },

    /// Underlying IO error from network operations.
    #[error("IO error: {0}")]
    Io(#[from] io::Error),

    /// VISCA protocol syntax error (0x02): Command format is incorrect or parameters are illegal.
    #[error("Syntax error in VISCA command")]
    SyntaxError,

    /// VISCA protocol command buffer full error (0x03): Two sockets are already in use.
    #[error("Command buffer is full")]
    CommandBufferFull,

    /// VISCA protocol command canceled (0x04): Command was canceled in the specified socket.
    #[error("Command was canceled")]
    CommandCanceled,

    /// VISCA protocol no socket error (0x05): No command is executing in the specified socket.
    #[error("No socket available")]
    NoSocket,

    /// VISCA protocol command not executable (0x41): Command cannot be executed due to current conditions.
    #[error("Command is not executable")]
    CommandNotExecutable,

    /// Response data doesn't conform to expected VISCA protocol format.
    #[error("Invalid response format")]
    InvalidResponseFormat,

    /// Response has an unexpected number of bytes.
    #[error("Invalid response length")]
    InvalidResponseLength,

    /// Response type doesn't match what the command should return.
    #[error("Unexpected response type")]
    UnexpectedResponseType,

    /// Received an unknown error code from the camera.
    #[error("Unknown error code: {0:#02X}")]
    Unknown(u8),

    /// Invalid request to socket manager.
    #[error("Invalid request: {0}")]
    InvalidRequest(Cow<'static, str>),

    /// Message length error (0x01): Message length is incorrect.
    #[error("Message length error")]
    MessageLengthError,

    /// Failed to parse response data.
    #[error("Parse error: {0}")]
    ParseError(Cow<'static, str>),

    /// Transport layer communication error.
    #[error("Transport error: {0}")]
    TransportError(Cow<'static, str>),

    /// Invalid parameter provided to a command.
    #[error("Invalid parameter '{parameter}': {reason} (value: {value})")]
    InvalidParameter {
        /// The parameter name that was invalid.
        parameter: &'static str,
        /// The value that was provided.
        value: Cow<'static, str>,
        /// The reason why it's invalid.
        reason: Cow<'static, str>,
    },

    /// Buffer provided is too small for encoding.
    #[error("Buffer too small: required {required} bytes, but only {actual} available")]
    BufferTooSmall {
        /// Required buffer size.
        required: usize,
        /// Actual buffer size provided.
        actual: usize,
    },

    /// Invalid preset number for the camera model.
    #[error("Invalid preset {preset}: must be <= {max}")]
    InvalidPreset {
        /// The requested preset number.
        preset: u8,
        /// Maximum allowed preset for this camera.
        max: u8,
    },

    /// Parameter value is out of the acceptable range.
    #[error("Parameter out of range: {parameter} = {value} (valid range: {min}..{max})")]
    ParameterOutOfRange {
        /// Name of the parameter.
        parameter: &'static str,
        /// Value that was provided.
        value: i32,
        /// Minimum valid value.
        min: i32,
        /// Maximum valid value.
        max: i32,
    },

    /// Operation exceeded timeout without response.
    #[error("Operation timed out")]
    Timeout,

    /// Maximum retry attempts exceeded.
    #[error("Maximum retries exceeded")]
    MaxRetriesExceeded,

    /// Operation is not supported by this implementation.
    #[error("Operation not supported")]
    NotSupported,

    /// Operation cannot be performed in current state.
    #[error("Invalid state: {0}")]
    InvalidState(Cow<'static, str>),

    /// Transport is busy and cannot be borrowed for a new operation.
    /// This occurs when multiple operations try to use the transport concurrently
    /// in blocking mode.
    #[error("Transport is busy with another operation")]
    TransportBusy,

    /// Command validation failed for the specified camera model.
    #[error("Command '{command}' not valid for {model:?}: {reason}")]
    ModelValidation {
        /// The camera model that failed validation.
        model: crate::constants::CameraVariant,
        /// The command that failed validation.
        command: Cow<'static, str>,
        /// Reason for the validation failure.
        reason: Cow<'static, str>,
    },

    /// No response received from camera.
    #[error("No response received from camera")]
    NoResponse,

    /// Channel has been closed.
    #[error("Channel closed")]
    ChannelClosed,

    /// Runtime has been shutdown.
    #[error("Runtime has been shutdown")]
    RuntimeShutdown,

    /// Validation error from capability traits.
    #[error("Validation error: {0}")]
    ValidationError(#[from] crate::capabilities::ValidationError),

    /// Unknown inquiry response type.
    #[error("Unknown inquiry response type '{response_type}' with data: {data:?}")]
    UnknownResponseKind {
        /// The response type that was not recognized.
        response_type: Cow<'static, str>,
        /// The raw response data.
        data: Vec<u8>,
    },

    /// Invalid camera ID provided.
    #[error("Invalid camera ID {id}: must be 1-7 for individual cameras or 8 for broadcast")]
    InvalidCameraId {
        /// The invalid camera ID that was provided.
        id: u8,
    },

    /// Lock was poisoned by a panic in another thread.
    #[error("Lock poisoned for {0}")]
    LockPoisoned(&'static str),

    /// Response exceeds maximum allowed size.
    #[error("Response too large: exceeds maximum of {max_size} bytes")]
    ResponseTooLarge {
        /// Maximum allowed size.
        max_size: usize,
    },

    /// Socket manager is unavailable or has been shut down.
    #[error("Socket manager unavailable")]
    SocketManagerUnavailable,

    /// Channel for socket manager communication has been closed.
    #[error("Socket manager channel closed")]
    SocketManagerChannelClosed,

    /// Response channel has been closed unexpectedly.
    #[error("Response channel closed")]
    ResponseChannelClosed,

    /// Invalid network address format.
    #[error("Invalid address: {reason}")]
    InvalidAddress {
        /// Reason why the address is invalid.
        reason: Cow<'static, str>,
    },

    /// Transport configuration mismatch.
    #[error("Transport configuration mismatch: {reason}")]
    TransportMismatch {
        /// Reason for the mismatch.
        reason: &'static str,
    },

    /// Runtime is required for async operations but was not provided.
    #[error("No runtime configured for async operations")]
    MissingRuntime,
}

impl Error {
    /// Create an `Error` from a VISCA error response code.
    ///
    /// ## Error Code Mapping
    ///
    /// - `0x01`: Message Length Error - message length incorrect
    /// - `0x02`: Syntax Error - command format invalid
    /// - `0x03`: Command Buffer Full - camera busy, always retry later
    /// - `0x04`: Command Canceled - command was canceled
    /// - `0x05`: No Socket - no socket available
    /// - `0x41`: Command Not Executable - command invalid in current state
    ///
    /// Note: 0x41 is context-dependent. Some cameras (e.g., FR7) use it to indicate
    /// "still settling after preset, retry" while others mean "invalid command".
    /// The runtime determines retry policy based on camera profile and command context.
    #[must_use]
    pub const fn from_code(code: u8) -> Self {
        match code {
            0x01 => Self::MessageLengthError,
            0x02 => Self::SyntaxError,
            0x03 => Self::CommandBufferFull, // Always retryable
            0x04 => Self::CommandCanceled,
            0x05 => Self::NoSocket,
            0x41 => Self::CommandNotExecutable, // Context-dependent retryability
            _ => Self::Unknown(code),
        }
    }

    /// Check if this error is potentially retryable.
    ///
    /// Returns `true` for errors that represent temporary conditions
    /// that may succeed if the operation is retried. This includes:
    /// - Camera busy states (`CameraBusy`, `CommandBufferFull`)
    /// - Pending operations (`CommandPending`, `CameraMoving`)
    /// - Timeout conditions (`CommandTimeout`, `Timeout`)
    ///
    /// # Example
    ///
    /// ```rust
    /// use grafton_visca::Error;
    ///
    /// let error = Error::CameraBusy;
    /// if error.is_retryable() {
    ///     println!("This error can be retried");
    /// }
    /// ```
    #[must_use]
    pub const fn is_retryable(&self) -> bool {
        matches!(
            self,
            Self::CameraBusy
                | Self::CommandPending
                | Self::CameraMoving { .. }
                | Self::CommandTimeout { .. }
                | Self::CommandBufferFull
                | Self::Timeout
        )
    }

    /// Get a suggested retry delay for retryable errors.
    ///
    /// Returns `Some(Duration)` with a recommended delay before retrying
    /// the operation, or `None` if the error is not retryable.
    ///
    /// The suggested delays are based on typical camera response times:
    /// - `CameraBusy`: 100ms (camera is processing)
    /// - `CommandPending`: 50ms (command acknowledged, waiting for completion)
    /// - `CameraMoving`: 500ms (mechanical movement in progress)
    /// - `CommandTimeout`: 1s (previous timeout, try with longer duration)
    /// - `CommandBufferFull`: 200ms (wait for buffer space)
    /// - `Timeout`: 2s (general timeout, allow more time)
    ///
    /// # Example
    ///
    /// ```rust
    /// use grafton_visca::Error;
    /// use std::time::Duration;
    ///
    /// let error = Error::CameraBusy;
    /// if let Some(delay) = error.suggested_retry_delay() {
    ///     assert_eq!(delay, Duration::from_millis(100));
    ///     std::thread::sleep(delay);
    ///     // Retry the operation...
    /// }
    /// ```
    #[must_use]
    pub const fn suggested_retry_delay(&self) -> Option<Duration> {
        match self {
            Self::CameraBusy => Some(Duration::from_millis(100)),
            Self::CommandPending => Some(Duration::from_millis(50)),
            Self::CameraMoving { .. } => Some(Duration::from_millis(500)),
            Self::CommandTimeout { .. } => Some(Duration::from_secs(1)),
            Self::CommandBufferFull => Some(Duration::from_millis(200)),
            Self::Timeout => Some(Duration::from_secs(2)),
            Self::MaxRetriesExceeded => None,
            _ => None,
        }
    }
}

impl From<nom::Err<nom::error::Error<&[u8]>>> for Error {
    fn from(err: nom::Err<nom::error::Error<&[u8]>>) -> Self {
        Self::ParseError(Cow::Owned(err.to_string()))
    }
}

impl From<Infallible> for Error {
    fn from(_: Infallible) -> Self {
        unreachable!("Infallible error should never occur")
    }
}

#[cfg(test)]
#[allow(clippy::panic)]
mod tests {
    use super::*;
    use crate::command::bytes::VISCA_TERMINATOR;

    #[test]
    fn test_visca_error_from_code() {
        assert!(matches!(Error::from_code(0x01), Error::MessageLengthError));
        assert!(matches!(Error::from_code(0x02), Error::SyntaxError));
        assert!(matches!(Error::from_code(0x03), Error::CommandBufferFull));
        assert!(matches!(Error::from_code(0x04), Error::CommandCanceled));
        assert!(matches!(Error::from_code(0x05), Error::NoSocket));
        assert!(matches!(
            Error::from_code(0x41),
            Error::CommandNotExecutable
        ));
        assert!(matches!(Error::from_code(0xFF), Error::Unknown(0xFF)));
    }

    #[test]
    fn test_error_code_mapping_table() {
        // Table-driven test for all known VISCA error codes
        // This ensures consistent mapping across all layers
        let cases = [
            (0x01, "MessageLengthError"),
            (0x02, "SyntaxError"),
            (0x03, "CommandBufferFull"),
            (0x04, "CommandCanceled"),
            (0x05, "NoSocket"),
            // 0x41 is "Command Not Executable" per VISCA spec - command invalid in current state
            (0x41, "CommandNotExecutable"),
        ];

        for (byte, expected_variant) in cases {
            let error = Error::from_code(byte);
            let variant_name = match error {
                Error::MessageLengthError => "MessageLengthError",
                Error::SyntaxError => "SyntaxError",
                Error::CommandBufferFull => "CommandBufferFull",
                Error::CommandCanceled => "CommandCanceled",
                Error::NoSocket => "NoSocket",
                Error::CameraBusy => "CameraBusy",
                Error::CommandNotExecutable => "CommandNotExecutable",
                _ => "Unknown",
            };
            assert_eq!(
                variant_name, expected_variant,
                "Byte {:#04x} should map to {}",
                byte, expected_variant
            );
        }
    }

    #[test]
    fn test_unknown_error_code_maps_to_unknown() {
        // Test that unrecognized error codes map to Unknown variant
        let unknown_codes = [0x00, 0x06, 0x10, 0x20, 0x30, 0x40, 0x42, VISCA_TERMINATOR];

        for code in unknown_codes {
            let error = Error::from_code(code);
            assert!(
                matches!(error, Error::Unknown(c) if c == code),
                "Code {:#04x} should map to Unknown({})",
                code,
                code
            );
        }
    }

    #[test]
    fn test_visca_error_display() {
        assert_eq!(
            Error::SyntaxError.to_string(),
            "Syntax error in VISCA command"
        );
        assert_eq!(
            Error::CommandBufferFull.to_string(),
            "Command buffer is full"
        );
        assert_eq!(Error::CommandCanceled.to_string(), "Command was canceled");
        assert_eq!(Error::NoSocket.to_string(), "No socket available");
        assert_eq!(
            Error::CommandNotExecutable.to_string(),
            "Command is not executable"
        );
        assert_eq!(Error::Unknown(0x99).to_string(), "Unknown error code: 0x99");
        assert_eq!(
            Error::InvalidParameter {
                parameter: "test",
                value: Cow::Borrowed("invalid"),
                reason: Cow::Borrowed("test reason"),
            }
            .to_string(),
            "Invalid parameter 'test': test reason (value: invalid)"
        );
        assert_eq!(Error::Timeout.to_string(), "Operation timed out");
    }

    #[test]
    fn test_visca_error_from_io_error() {
        let io_err = io::Error::other("test error");
        let visca_err = Error::from(io_err);
        assert!(matches!(visca_err, Error::Io(_)));
    }

    #[test]
    fn test_visca_error_from_nom_error() {
        use nom::error::{Error as NomError, ErrorKind};

        let nom_err = nom::Err::Error(NomError::new(&b"test"[..], ErrorKind::Tag));
        let visca_err = Error::from(nom_err);
        assert!(matches!(visca_err, Error::ParseError(_)));
    }

    #[test]
    fn test_is_retryable() {
        assert!(Error::CameraBusy.is_retryable());
        assert!(Error::CameraMoving { pan: 100, tilt: 50 }.is_retryable());
        assert!(Error::CommandTimeout {
            duration: Duration::from_secs(5),
            command: Cow::Borrowed("test")
        }
        .is_retryable());
        assert!(Error::CommandBufferFull.is_retryable());
        assert!(Error::Timeout.is_retryable());

        assert!(!Error::SyntaxError.is_retryable());
        assert!(!Error::CommandNotExecutable.is_retryable());
        assert!(!Error::InvalidParameter {
            parameter: "test",
            value: Cow::Borrowed("invalid"),
            reason: Cow::Borrowed("test reason"),
        }
        .is_retryable());
        assert!(!Error::PresetNotFound { id: 1 }.is_retryable());
    }

    #[test]
    fn test_suggested_retry_delay() {
        assert_eq!(
            Error::CameraBusy.suggested_retry_delay(),
            Some(Duration::from_millis(100))
        );
        assert_eq!(
            Error::CameraMoving { pan: 100, tilt: 50 }.suggested_retry_delay(),
            Some(Duration::from_millis(500))
        );
        assert_eq!(
            Error::CommandTimeout {
                duration: Duration::from_secs(5),
                command: Cow::Borrowed("test")
            }
            .suggested_retry_delay(),
            Some(Duration::from_secs(1))
        );
        assert_eq!(
            Error::CommandBufferFull.suggested_retry_delay(),
            Some(Duration::from_millis(200))
        );
        assert_eq!(
            Error::Timeout.suggested_retry_delay(),
            Some(Duration::from_secs(2))
        );

        assert_eq!(Error::SyntaxError.suggested_retry_delay(), None);
        assert_eq!(
            Error::InvalidParameter {
                parameter: "test",
                value: Cow::Borrowed("invalid"),
                reason: Cow::Borrowed("test reason"),
            }
            .suggested_retry_delay(),
            None
        );
    }

    #[test]
    fn test_error_classification_completeness() {
        let retryable_errors = vec![
            Error::CameraBusy,
            Error::CameraMoving { pan: 0, tilt: 0 },
            Error::CommandTimeout {
                duration: Duration::from_secs(1),
                command: Cow::Borrowed("test"),
            },
            Error::CommandBufferFull,
            Error::Timeout,
        ];

        for error in retryable_errors {
            assert!(error.is_retryable(), "Error should be retryable: {error}");
            assert!(
                error.suggested_retry_delay().is_some(),
                "Retryable error should have suggested delay: {error}"
            );
        }

        let non_retryable_errors = vec![
            Error::SyntaxError,
            Error::CommandNotExecutable,
            Error::PresetNotFound { id: 1 },
            Error::FeatureNotSupported { feature: "test" },
            Error::InvalidParameter {
                parameter: "test",
                value: Cow::Borrowed("invalid"),
                reason: Cow::Borrowed("test reason"),
            },
        ];

        for error in non_retryable_errors {
            assert!(
                !error.is_retryable(),
                "Error should not be retryable: {error}"
            );
            assert!(
                error.suggested_retry_delay().is_none(),
                "Non-retryable error should not have suggested delay: {error}"
            );
        }
    }
}
