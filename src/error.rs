use std::io;
use std::time::Duration;
use thiserror::Error;

// Using traditional exhaustive enum approach for ViscaError
// This provides better ergonomics for library users who need to handle specific errors
// The VISCA protocol has a well-defined set of error conditions that are unlikely to change frequently
// Adding new variants will require a major version bump, which is acceptable for this use case
#[derive(Error, Debug)]
pub enum ViscaError {
    // Connection errors
    #[error("Connection failed to {addr}: {source}")]
    ConnectionFailed { addr: String, source: io::Error },

    #[error("Connection lost: {reason}")]
    ConnectionLost { reason: String },

    #[error("Command timeout after {duration:?} for command: {command}")]
    CommandTimeout { duration: Duration, command: String },

    // Camera state errors
    #[error("Camera is busy executing another command")]
    CameraBusy,

    #[error("Camera is still moving, position: pan={pan}, tilt={tilt}")]
    CameraMoving { pan: i16, tilt: i16 },

    #[error("Camera not initialized")]
    CameraNotReady,

    // Protocol errors
    #[error("Invalid response: expected {expected}, got {actual:?}")]
    InvalidResponse { expected: String, actual: Vec<u8> },

    #[error("Command rejected by camera: {reason}")]
    CommandRejected { reason: String },

    // Value errors
    #[error("Value {value} out of range [{min}, {max}] for {parameter}")]
    OutOfRange {
        value: i32,
        min: i32,
        max: i32,
        parameter: String,
    },

    #[error("Preset {id} not found")]
    PresetNotFound { id: u8 },

    // Feature errors
    #[error("Feature '{feature}' not supported by this camera model")]
    FeatureNotSupported { feature: String },

    // Legacy errors (keeping for backward compatibility)
    #[error("IO error: {0}")]
    Io(#[from] io::Error),

    #[error("Syntax error in VISCA command")]
    SyntaxError,

    #[error("Command buffer is full")]
    CommandBufferFull,

    #[error("Command was canceled")]
    CommandCanceled,

    #[error("No socket available")]
    NoSocket,

    #[error("Command is not executable")]
    CommandNotExecutable,

    #[error("Invalid response format")]
    InvalidResponseFormat,

    #[error("Invalid response length")]
    InvalidResponseLength,

    #[error("Unexpected response type")]
    UnexpectedResponseType,

    #[error("Unknown error code: {0:#02X}")]
    Unknown(u8),

    #[error("Parse error: {0}")]
    ParseError(String),

    #[error("Transport error: {0}")]
    TransportError(String),

    #[error("Invalid parameter: {0}")]
    InvalidParameter(String),

    #[error("Parameter out of range: {parameter} = {value} (valid range: {min}..{max})")]
    ParameterOutOfRange {
        parameter: String,
        value: i32,
        min: i32,
        max: i32,
    },

    #[error("Operation timed out")]
    Timeout,

    #[error("Invalid state: {0}")]
    InvalidState(String),
}

impl ViscaError {
    pub fn from_code(code: u8) -> Self {
        match code {
            0x02 => ViscaError::SyntaxError,
            0x03 => ViscaError::CommandBufferFull,
            0x04 => ViscaError::CommandCanceled,
            0x05 => ViscaError::NoSocket,
            0x41 => ViscaError::CommandNotExecutable,
            _ => ViscaError::Unknown(code),
        }
    }

    pub fn is_retryable(&self) -> bool {
        matches!(
            self,
            ViscaError::CameraBusy
                | ViscaError::CameraMoving { .. }
                | ViscaError::CommandTimeout { .. }
                | ViscaError::CommandBufferFull
                | ViscaError::Timeout
        )
    }

    pub fn suggested_retry_delay(&self) -> Option<Duration> {
        match self {
            ViscaError::CameraBusy => Some(Duration::from_millis(100)),
            ViscaError::CameraMoving { .. } => Some(Duration::from_millis(500)),
            ViscaError::CommandTimeout { .. } => Some(Duration::from_secs(1)),
            ViscaError::CommandBufferFull => Some(Duration::from_millis(200)),
            ViscaError::Timeout => Some(Duration::from_secs(2)),
            _ => None,
        }
    }
}

impl From<nom::Err<nom::error::Error<&[u8]>>> for ViscaError {
    fn from(err: nom::Err<nom::error::Error<&[u8]>>) -> Self {
        ViscaError::ParseError(err.to_string())
    }
}

#[derive(Error, Debug)]
pub enum AppError {
    #[error("IO error: {0}")]
    Io(#[from] io::Error),

    #[error("VISCA error: {0}")]
    Visca(#[from] ViscaError),
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_visca_error_from_code() {
        assert!(matches!(
            ViscaError::from_code(0x02),
            ViscaError::SyntaxError
        ));
        assert!(matches!(
            ViscaError::from_code(0x03),
            ViscaError::CommandBufferFull
        ));
        assert!(matches!(
            ViscaError::from_code(0x04),
            ViscaError::CommandCanceled
        ));
        assert!(matches!(ViscaError::from_code(0x05), ViscaError::NoSocket));
        assert!(matches!(
            ViscaError::from_code(0x41),
            ViscaError::CommandNotExecutable
        ));
        assert!(matches!(
            ViscaError::from_code(0xFF),
            ViscaError::Unknown(0xFF)
        ));
    }

    #[test]
    fn test_visca_error_display() {
        assert_eq!(
            ViscaError::SyntaxError.to_string(),
            "Syntax error in VISCA command"
        );
        assert_eq!(
            ViscaError::CommandBufferFull.to_string(),
            "Command buffer is full"
        );
        assert_eq!(
            ViscaError::CommandCanceled.to_string(),
            "Command was canceled"
        );
        assert_eq!(ViscaError::NoSocket.to_string(), "No socket available");
        assert_eq!(
            ViscaError::CommandNotExecutable.to_string(),
            "Command is not executable"
        );
        assert_eq!(
            ViscaError::Unknown(0x99).to_string(),
            "Unknown error code: 0x99"
        );
        assert_eq!(
            ViscaError::InvalidParameter("test".to_string()).to_string(),
            "Invalid parameter: test"
        );
        assert_eq!(ViscaError::Timeout.to_string(), "Operation timed out");
    }

    #[test]
    fn test_visca_error_from_io_error() {
        let io_err = io::Error::new(io::ErrorKind::ConnectionRefused, "test error");
        let visca_err = ViscaError::from(io_err);
        assert!(matches!(visca_err, ViscaError::Io(_)));
    }

    #[test]
    fn test_visca_error_from_nom_error() {
        use nom::error::{Error as NomError, ErrorKind};
        let nom_err = nom::Err::Error(NomError::new(&b"test"[..], ErrorKind::Tag));
        let visca_err = ViscaError::from(nom_err);
        assert!(matches!(visca_err, ViscaError::ParseError(_)));
    }

    #[test]
    fn test_app_error_from_io() {
        let io_err = io::Error::new(io::ErrorKind::NotFound, "file not found");
        let app_err = AppError::from(io_err);
        assert!(matches!(app_err, AppError::Io(_)));
    }

    #[test]
    fn test_app_error_from_visca() {
        let visca_err = ViscaError::SyntaxError;
        let app_err = AppError::from(visca_err);
        assert!(matches!(app_err, AppError::Visca(ViscaError::SyntaxError)));
    }

    #[test]
    fn test_is_retryable() {
        // Retryable errors
        assert!(ViscaError::CameraBusy.is_retryable());
        assert!(ViscaError::CameraMoving { pan: 100, tilt: 50 }.is_retryable());
        assert!(ViscaError::CommandTimeout {
            duration: Duration::from_secs(5),
            command: "test".to_string()
        }
        .is_retryable());
        assert!(ViscaError::CommandBufferFull.is_retryable());
        assert!(ViscaError::Timeout.is_retryable());

        // Non-retryable errors
        assert!(!ViscaError::SyntaxError.is_retryable());
        assert!(!ViscaError::CommandNotExecutable.is_retryable());
        assert!(!ViscaError::InvalidParameter("test".to_string()).is_retryable());
        assert!(!ViscaError::PresetNotFound { id: 1 }.is_retryable());
    }

    #[test]
    fn test_suggested_retry_delay() {
        // Errors with retry delays
        assert_eq!(
            ViscaError::CameraBusy.suggested_retry_delay(),
            Some(Duration::from_millis(100))
        );
        assert_eq!(
            ViscaError::CameraMoving { pan: 100, tilt: 50 }.suggested_retry_delay(),
            Some(Duration::from_millis(500))
        );
        assert_eq!(
            ViscaError::CommandTimeout {
                duration: Duration::from_secs(5),
                command: "test".to_string()
            }
            .suggested_retry_delay(),
            Some(Duration::from_secs(1))
        );
        assert_eq!(
            ViscaError::CommandBufferFull.suggested_retry_delay(),
            Some(Duration::from_millis(200))
        );
        assert_eq!(
            ViscaError::Timeout.suggested_retry_delay(),
            Some(Duration::from_secs(2))
        );

        // Errors without retry delays
        assert_eq!(ViscaError::SyntaxError.suggested_retry_delay(), None);
        assert_eq!(
            ViscaError::InvalidParameter("test".to_string()).suggested_retry_delay(),
            None
        );
    }
}
