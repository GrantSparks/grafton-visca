//! Typed values returned by inquiry commands.

/// Camera version information returned by the version inquiry.
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

/// Tally light state returned by the tally inquiry.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TallyStatusState {
    /// Whether the red tally light is on.
    pub red_on: bool,
    /// Whether the green tally light is on.
    pub green_on: bool,
}

/// Flip state configuration.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FlipState {
    /// Whether horizontal flip is enabled.
    pub horizontal: bool,
    /// Whether vertical flip is enabled.
    pub vertical: bool,
}
