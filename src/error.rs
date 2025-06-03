use std::io;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum ViscaError {
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
}
