//! Typed VISCA command responses.
//!
//! This module defines a `ViscaCommand` trait with an associated `Response`
//! type that maps a specific command to its parsed, strongly-typed output.
//!
//! Initial slice implements typed responses for a few common inquiries:
//! - `PowerInquiry` -> `bool`
//! - `PanTiltPositionInquiry` -> `crate::camera::PanTiltPosition`
//! - `ZoomPositionInquiry` -> `u16`

use crate::command::ViscaResponse;
use crate::error::Error;

/// Trait for commands that have an associated typed response.
pub trait ViscaCommand {
    /// Strongly-typed response for this command.
    type Response;

    /// Convert a generic `ViscaResponse` into the typed response for this command.
    fn from_response(resp: ViscaResponse) -> Result<Self::Response, Error>;
}

// Typed response structs for compound responses

/// Named struct for flip state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FlipState {
    /// Whether horizontal flip is enabled.
    pub horizontal: bool,
    /// Whether vertical flip is enabled.
    pub vertical: bool,
}

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

// Implementations are derived automatically by the `InquiryCommand` proc-macro
// for supported inquiry types.
