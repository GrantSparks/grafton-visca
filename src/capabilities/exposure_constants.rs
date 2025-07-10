//! Exposure constants for specific camera models.

use crate::capabilities::exposure::ShutterSpeed;

/// Shutter speeds for PTZOptics G2 cameras.
pub const PTZOPTICS_G2_SHUTTER_SPEEDS: &[ShutterSpeed] = &[
    ShutterSpeed::new("1/30", 0x01),
    ShutterSpeed::new("1/60", 0x02),
    ShutterSpeed::new("1/90", 0x03),
    ShutterSpeed::new("1/100", 0x04),
    ShutterSpeed::new("1/125", 0x05),
    ShutterSpeed::new("1/180", 0x06),
    ShutterSpeed::new("1/250", 0x07),
    ShutterSpeed::new("1/350", 0x08),
    ShutterSpeed::new("1/500", 0x09),
    ShutterSpeed::new("1/725", 0x0A),
    ShutterSpeed::new("1/1000", 0x0B),
    ShutterSpeed::new("1/1500", 0x0C),
    ShutterSpeed::new("1/2000", 0x0D),
    ShutterSpeed::new("1/3000", 0x0E),
    ShutterSpeed::new("1/4000", 0x0F),
    ShutterSpeed::new("1/6000", 0x10),
    ShutterSpeed::new("1/10000", 0x11),
];

/// Shutter speeds for generic VISCA cameras (subset of common speeds).
pub const GENERIC_VISCA_SHUTTER_SPEEDS: &[ShutterSpeed] = &[
    ShutterSpeed::new("1/30", 0x00),
    ShutterSpeed::new("1/60", 0x01),
    ShutterSpeed::new("1/100", 0x02),
    ShutterSpeed::new("1/250", 0x03),
    ShutterSpeed::new("1/500", 0x04),
    ShutterSpeed::new("1/1000", 0x05),
    ShutterSpeed::new("1/2000", 0x06),
    ShutterSpeed::new("1/4000", 0x07),
    ShutterSpeed::new("1/10000", 0x08),
];