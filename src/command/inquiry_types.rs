//! Types used in inquiry responses.

/// Camera version information.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Version {
    /// Vendor ID.
    pub vendor: u16,
    /// Model ID.
    pub model: u16,
    /// ROM version.
    pub rom_version: u32,
    /// Maximum socket number.
    pub max_socket: u8,
}

/// Tally light status.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TallyStatus {
    /// Whether the red tally light is on.
    pub red_on: bool,
    /// Whether the green tally light is on.
    pub green_on: bool,
}

/// Image flip status.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ImageFlipStatus {
    /// Whether vertical flip is enabled.
    pub vertical: bool,
    /// Whether horizontal flip is enabled.
    pub horizontal: bool,
}

/// Flip mode configuration.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FlipMode {
    /// Whether horizontal flip is enabled.
    pub horizontal: bool,
    /// Whether vertical flip is enabled.
    pub vertical: bool,
}

/// Night/Day mode setting.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NightDayMode {
    /// Day mode.
    Day,
    /// Night mode.
    Night,
}

/// Iris control mode.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum IrisControl {
    /// Auto iris control.
    Auto,
    /// Manual iris control.
    Manual,
}
