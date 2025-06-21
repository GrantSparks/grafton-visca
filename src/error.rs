use std::{fmt, io, time::Duration};

use thiserror::Error;

/// Custom result type for VISCA operations.
pub type Result<T, E = Error> = std::result::Result<T, E>;

/// VISCA protocol error type.
///
/// Provides comprehensive error handling for all VISCA operations.
/// The VISCA protocol has a well-defined set of error conditions
/// that map directly to camera responses and communication failures.
#[derive(thiserror::Error, Debug)]
pub enum Error {
    /// Failed to establish connection to the camera.
    #[error("Connection failed to {addr}: {source}")]
    ConnectionFailed {
        /// The address that failed to connect.
        addr: String,
        /// The underlying IO error.
        source: io::Error,
    },

    /// Connection to the camera was lost during operation.
    #[error("Connection lost: {reason}")]
    ConnectionLost {
        /// Reason for the connection loss.
        reason: String,
    },

    /// Command execution exceeded the configured timeout.
    #[error("Command timeout after {duration:?} for command: {command}")]
    CommandTimeout {
        /// Duration of the timeout.
        duration: Duration,
        /// Description of the command that timed out.
        command: String,
    },

    /// Camera is busy executing another command and cannot accept new commands.
    #[error("Camera is busy executing another command")]
    CameraBusy,

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
        expected: String,
        /// Actual bytes received.
        actual: Vec<u8>,
    },

    /// Camera explicitly rejected the command.
    #[error("Command rejected by camera: {reason}")]
    CommandRejected {
        /// Reason for rejection.
        reason: String,
    },

    /// A parameter value is outside the valid range.
    #[error("Value {value} out of range [{min}, {max}] for {parameter}")]
    OutOfRange {
        /// The invalid value provided.
        value: i32,
        /// Minimum valid value.
        min: i32,
        /// Maximum valid value.
        max: i32,
        /// Name of the parameter.
        parameter: String,
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
        feature: String,
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

    /// Failed to parse response data.
    #[error("Parse error: {0}")]
    ParseError(String),

    /// Transport layer communication error.
    #[error("Transport error: {0}")]
    TransportError(String),

    /// Invalid parameter provided to a command.
    #[error("Invalid parameter: {0}")]
    InvalidParameter(String),

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
        parameter: String,
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

    /// Operation cannot be performed in current state.
    #[error("Invalid state: {0}")]
    InvalidState(String),

    /// Command validation failed for the specified camera model.
    #[error("Command '{command}' not valid for {model:?}: {reason}")]
    ModelValidation {
        /// The camera model that failed validation.
        model: crate::constants::CameraModel,
        /// The command that failed validation.
        command: String,
        /// Reason for the validation failure.
        reason: String,
    },

    /// No response received from camera.
    #[error("No response received from camera")]
    NoResponse,

    /// Channel has been closed.
    #[error("Channel closed")]
    ChannelClosed,
}

impl Error {
    /// Create an `Error` from a VISCA error response code.
    #[must_use]
    pub const fn from_code(code: u8) -> Self {
        match code {
            0x02 => Self::SyntaxError,
            0x03 => Self::CommandBufferFull,
            0x04 => Self::CommandCanceled,
            0x05 => Self::NoSocket,
            0x41 => Self::CommandNotExecutable,
            _ => Self::Unknown(code),
        }
    }

    /// Check if this error is potentially retryable.
    #[must_use]
    pub const fn is_retryable(&self) -> bool {
        matches!(
            self,
            Self::CameraBusy
                | Self::CameraMoving { .. }
                | Self::CommandTimeout { .. }
                | Self::CommandBufferFull
                | Self::Timeout
        )
    }

    /// Get a suggested retry delay for retryable errors.
    #[must_use]
    pub const fn suggested_retry_delay(&self) -> Option<Duration> {
        match self {
            Self::CameraBusy => Some(Duration::from_millis(100)),
            Self::CameraMoving { .. } => Some(Duration::from_millis(500)),
            Self::CommandTimeout { .. } => Some(Duration::from_secs(1)),
            Self::CommandBufferFull => Some(Duration::from_millis(200)),
            Self::Timeout => Some(Duration::from_secs(2)),
            _ => None,
        }
    }
}

impl From<nom::Err<nom::error::Error<&[u8]>>> for Error {
    fn from(err: nom::Err<nom::error::Error<&[u8]>>) -> Self {
        Self::ParseError(err.to_string())
    }
}

/// Application-level error type for examples and user code.
#[doc(hidden)]
#[derive(Error, Debug)]
pub enum AppError {
    /// IO error from file or network operations.
    #[error("IO error: {0}")]
    Io(#[from] io::Error),

    /// VISCA protocol or camera communication error.
    #[error("VISCA error: {0}")]
    Visca(#[from] Error),
}

/// Error context information for debugging.
#[derive(Debug, Clone)]
pub struct ErrorContext {
    /// The operation that was being performed.
    pub operation: String,
    /// Additional context about the error.
    pub context: String,
}

impl fmt::Display for ErrorContext {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{} ({})", self.operation, self.context)
    }
}

#[cfg(test)]
#[allow(clippy::panic)]
mod tests {
    use super::*;

    #[test]
    fn test_visca_error_from_code() {
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
            Error::InvalidParameter("test".to_string()).to_string(),
            "Invalid parameter: test"
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
    fn test_app_error_from_io() {
        let io_err = io::Error::other("file not found");
        let app_err = AppError::from(io_err);
        assert!(matches!(app_err, AppError::Io(_)));
    }

    #[test]
    fn test_app_error_from_visca() {
        let visca_err = Error::SyntaxError;
        let app_err = AppError::from(visca_err);
        assert!(matches!(app_err, AppError::Visca(Error::SyntaxError)));
    }

    #[test]
    fn test_is_retryable() {
        assert!(Error::CameraBusy.is_retryable());
        assert!(Error::CameraMoving { pan: 100, tilt: 50 }.is_retryable());
        assert!(Error::CommandTimeout {
            duration: Duration::from_secs(5),
            command: "test".to_string()
        }
        .is_retryable());
        assert!(Error::CommandBufferFull.is_retryable());
        assert!(Error::Timeout.is_retryable());

        assert!(!Error::SyntaxError.is_retryable());
        assert!(!Error::CommandNotExecutable.is_retryable());
        assert!(!Error::InvalidParameter("test".to_string()).is_retryable());
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
                command: "test".to_string()
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
            Error::InvalidParameter("test".to_string()).suggested_retry_delay(),
            None
        );
    }

    #[test]
    fn test_error_classification_completeness() {
        // Ensure all retryable errors have suggested delays
        let retryable_errors = vec![
            Error::CameraBusy,
            Error::CameraMoving { pan: 0, tilt: 0 },
            Error::CommandTimeout {
                duration: Duration::from_secs(1),
                command: "test".to_string(),
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

        // Ensure non-retryable errors don't have suggested delays
        let non_retryable_errors = vec![
            Error::SyntaxError,
            Error::CommandNotExecutable,
            Error::PresetNotFound { id: 1 },
            Error::FeatureNotSupported {
                feature: "test".to_string(),
            },
            Error::InvalidParameter("test".to_string()),
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
