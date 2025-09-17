//! Typed VISCA command responses.
//!
//! This module defines a `ViscaCommand` trait with an associated `Response`
//! type that maps a specific command to its parsed, strongly-typed output.
//!
//! Initial slice implements typed responses for a few common inquiries:
//! - `PowerInquiry` -> `bool`
//! - `PanTiltPositionInquiry` -> `crate::camera::PanTiltPosition`
//! - `ZoomPositionInquiry` -> `u16`

use crate::command::Response;
use crate::error::Error;

// Re-export FlipState from inquiry_types since the derive macro expects it here
pub use crate::command::inquiry_types::FlipState;

/// Trait for commands that can parse responses into strongly-typed values.
pub trait ResponseParser {
    /// Strongly-typed response for this command.
    type Response;

    /// Convert a generic `Response` into the typed response for this command.
    fn from_response(resp: Response) -> Result<Self::Response, Error>;
}

// Typed response structs for compound responses

/// Named struct for tally light state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TallyStatusState {
    /// Whether the red tally light is on.
    pub red_on: bool,
    /// Whether the green tally light is on.
    pub green_on: bool,
}

/// Named struct for version information.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct VersionInfo {
    /// Vendor identifier.
    pub vendor: u16,
    /// Model identifier.
    pub model: u16,
    /// ROM version as a packed integer.
    pub rom_version: u32,
    /// Maximum supported socket index.
    pub max_socket: u8,
}

// Implementations are derived automatically by the `ViscaInquiry` proc-macro
// for supported inquiry types.
