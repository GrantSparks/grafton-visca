//! Exposure-table values retained directly in profile capability records.

use crate::units::ExposureTime;

/// One entry of a profile's shutter-speed table: an exposure time and the
/// profile-specific wire code that selects it.
///
/// Registry rows and runtime [`ExposureCaps::shutter_speeds`](crate::capabilities::ExposureCaps::shutter_speeds)
/// hold the same entries. The wire code itself is the
/// [`crate::types::ShutterSpeed`] value.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
#[cfg_attr(feature = "ts-rs", derive(ts_rs::TS), ts(export))]
#[cfg_attr(feature = "serde", serde(deny_unknown_fields))]
pub struct ShutterSpeedEntry {
    /// Exposure time selected by this code, such as `1/60`; serialized as
    /// `"1/60"`.
    pub exposure: ExposureTime,
    /// VISCA shutter code.
    pub value: u8,
}

impl ShutterSpeedEntry {
    /// Creates a table entry.
    #[must_use]
    pub const fn new(exposure: ExposureTime, value: u8) -> Self {
        Self { exposure, value }
    }
}
