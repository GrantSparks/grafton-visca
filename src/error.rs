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
