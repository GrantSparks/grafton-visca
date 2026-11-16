//! Typed values returned by inquiry commands.

/// Camera version information returned by the version inquiry.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
#[cfg_attr(feature = "ts-rs", derive(ts_rs::TS))]
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
