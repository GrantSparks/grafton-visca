//! Typed VISCA command responses.
//!
//! This module defines typed response parsing for inquiry commands.
//!
//! Built-in inquiry implementations are generated from the internal inquiry
//! table; downstream inquiry commands can implement this trait directly or use
//! the `ViscaInquiry` derive with typed response attributes.

use crate::{command::Response, error::Error};

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

// Built-in inquiry implementations are generated from the internal inquiry
// table. The `ViscaInquiry` proc macro remains available for downstream
// extension commands.
