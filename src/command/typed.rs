//! Typed VISCA command responses.
//!
//! This module defines typed response parsing for inquiry commands.
//!
//! Built-in inquiry implementations are generated from the internal inquiry
//! table; downstream inquiry commands can implement this trait directly for
//! raw custom payloads or use the `ViscaInquiry` derive with built-in typed
//! response attributes.

use crate::{command::Response, error::Error};

/// Trait for commands that can parse responses into strongly-typed values.
pub trait ResponseParser {
    /// Strongly-typed response for this command.
    type Response;

    /// Convert a generic `Response` into the typed response for this command.
    ///
    /// This direct conversion has no camera-profile context. Built-in inquiries
    /// whose documented reply domain varies by profile perform that narrower
    /// validation in the session decoder selected during request preparation;
    /// direct conversion validates only the profile-neutral public value type.
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
// extension commands, while raw custom inquiry payloads use manual parsers.
