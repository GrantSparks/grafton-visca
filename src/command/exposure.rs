//! Exposure control commands for VISCA cameras.
//!
//! This module provides commands for controlling various exposure-related settings
//! including exposure mode, exposure compensation, iris, shutter, and brightness.

// Standard library imports
// (none)

// Third-party crate imports
// (none)

// Workspace / local-crate imports
use crate::macros::internal::*;

use crate::{
    command::{const_encoding::constants, encode_visca::EncodeVisca, response::ResponseType},
    error::Error,
    timeout::CommandCategory,
    types::{
        BrightnessLevel, DynamicRangeLevel, ExposureCompensationLevel, IrisLevel, ShutterSpeed,
    },
};
use grafton_visca_macros::ViscaEnum;

/// Camera exposure control modes.
#[derive(Debug, Copy, Clone, PartialEq, Eq, ViscaEnum)]
pub enum ExposureMode {
    /// Automatic exposure control - camera adjusts all exposure parameters automatically
    Auto = 0x00,
    /// Manual exposure control - user has full control over exposure parameters
    Manual = 0x03,
    /// Shutter priority mode - user controls shutter speed, camera adjusts other parameters
    Shutter = 0x0A,
    /// Iris priority mode - user controls iris/aperture, camera adjusts other parameters
    Iris = 0x0B,
    /// Brightness priority mode - user controls brightness level, camera adjusts other parameters
    Bright = 0x0D,
}

visca_param_command! {
    /// Command to set the camera's exposure mode.
    ///
    /// This command allows switching between different exposure modes such as
    /// auto, manual, shutter priority, iris priority, or brightness priority.
    pub(crate) struct ExposureCommand {
        mode: ExposureMode,
    }
    prefix = constants::exposure::MODE_PREFIX;
    param_byte = *mode as u8;
    timeout = Quick;
}

/// Exposure compensation commands.
///
/// # Example
/// ```text
/// This type is used internally by the camera methods.
/// Users should use the high-level camera API instead:
/// camera.set_exposure_compensation(true).await?;
/// ```
#[derive(Debug, Copy, Clone)]
pub enum ExposureCompensation {
    /// Enable exposure compensation
    On,
    /// Disable exposure compensation
    Off,
    /// Reset exposure compensation to 0
    Reset,
    /// Increase exposure compensation by one step
    Up,
    /// Decrease exposure compensation by one step
    Down,
    /// Set exposure compensation to a specific level (-7 to +7)
    SetLevel(ExposureCompensationLevel),
}

impl EncodeVisca for ExposureCompensation {
    type Response = ();
    const MAX_SIZE: usize = 9;

    fn encode_into(
        &self,
        camera_id: crate::camera_id::CameraId,
        buffer: &mut [u8],
    ) -> Result<usize, Error> {
        use crate::command::const_encoding::{constants, CommandBuilder};

        match self {
            Self::On | Self::Off => {
                let mut builder = CommandBuilder::<6>::new();
                builder
                    .append(constants::exposure::COMPENSATION_ON_OFF_PREFIX)
                    .push(match self {
                        Self::On => 0x02,
                        Self::Off => 0x03,
                        _ => unreachable!(),
                    })
                    .with_camera_id(camera_id)
                    .finalize();
                builder.copy_to(buffer)
            }
            Self::Reset | Self::Up | Self::Down => {
                let mut builder = CommandBuilder::<6>::new();
                builder
                    .append(constants::exposure::COMPENSATION_CONTROL_PREFIX)
                    .push(match self {
                        Self::Reset => 0x00,
                        Self::Up => 0x02,
                        Self::Down => 0x03,
                        _ => unreachable!(),
                    })
                    .with_camera_id(camera_id)
                    .finalize();
                builder.copy_to(buffer)
            }
            Self::SetLevel(level) => {
                let mut builder = CommandBuilder::<9>::new();
                builder
                    .append(constants::exposure::COMPENSATION_LEVEL_PREFIX)
                    .push(level.to_protocol_value())
                    .with_camera_id(camera_id)
                    .finalize();
                builder.copy_to(buffer)
            }
        }
    }

    fn response_type(&self) -> Option<ResponseType> {
        None
    }

    fn timeout_kind(&self) -> CommandCategory {
        CommandCategory::Quick
    }
}

visca_builder! {
    /// Commands for controlling the camera's dynamic range.
    ///
    /// Dynamic range control adjusts the camera's ability to capture detail
    /// in both bright and dark areas of a scene simultaneously. Higher values
    /// increase the dynamic range, allowing better detail retention in scenes
    /// with high contrast.
    pub struct DynamicRange {
        /// Dynamic range level (0-8).
        level: DynamicRangeLevel,
    }
    builder<9> => |builder, level| {
        let _ = builder.append(constants::exposure::DYNAMIC_RANGE_PREFIX);
        let _ = builder.push(level.value());
    }
    timeout = Quick;
}

impl DynamicRange {
    /// Set dynamic range to a specific level (0-8).
    pub fn new(level: DynamicRangeLevel) -> Self {
        Self { level }
    }
}

/// Commands for controlling iris/aperture values.
///
/// Provides standard VISCA control operations:
/// - Reset to default value
/// - Increment/decrement by one step
/// - Set to a specific value
#[derive(Debug, Copy, Clone)]
pub enum Iris {
    /// Reset to default value.
    Reset,
    /// Increase value by one step.
    Up,
    /// Decrease value by one step.
    Down,
    /// Set iris to specific aperture value.
    SetAperture(IrisLevel),
}

// Manual implementation to add model validation
impl EncodeVisca for Iris {
    type Response = ();
    const MAX_SIZE: usize = 9;

    fn encode_into(
        &self,
        camera_id: crate::camera_id::CameraId,
        buffer: &mut [u8],
    ) -> Result<usize, Error> {
        use crate::command::const_encoding::{constants, CommandBuilder};

        match self {
            Self::Reset | Self::Up | Self::Down => {
                let mut builder = CommandBuilder::<6>::new();
                builder
                    .append(constants::exposure::IRIS_CONTROL_PREFIX)
                    .push(match self {
                        Self::Reset => 0x00,
                        Self::Up => 0x02,
                        Self::Down => 0x03,
                        _ => unreachable!(),
                    })
                    .with_camera_id(camera_id)
                    .finalize();
                builder.copy_to(buffer)
            }
            Self::SetAperture(level) => {
                let mut builder = CommandBuilder::<9>::new();
                builder
                    .append(constants::exposure::IRIS_DIRECT_PREFIX)
                    .push_nibble_pair(level.value() as u16)
                    .with_camera_id(camera_id)
                    .finalize();
                builder.copy_to(buffer)
            }
        }
    }

    fn response_type(&self) -> Option<ResponseType> {
        None
    }

    fn timeout_kind(&self) -> CommandCategory {
        CommandCategory::Quick
    }
}

/// Commands for controlling shutter speed values.
///
/// Provides standard VISCA control operations:
/// - Reset to default value
/// - Increment/decrement by one step
/// - Set to a specific value
#[derive(Debug, Copy, Clone)]
pub enum Shutter {
    /// Reset to default value.
    Reset,
    /// Increase value by one step.
    Up,
    /// Decrease value by one step.
    Down,
    /// Set shutter to specific speed value.
    SetSpeed(ShutterSpeed),
}

// Manual implementation to add model validation
impl EncodeVisca for Shutter {
    type Response = ();
    const MAX_SIZE: usize = 9;

    fn encode_into(
        &self,
        camera_id: crate::camera_id::CameraId,
        buffer: &mut [u8],
    ) -> Result<usize, Error> {
        use crate::command::const_encoding::{constants, CommandBuilder};

        match self {
            Self::Reset | Self::Up | Self::Down => {
                let mut builder = CommandBuilder::<6>::new();
                builder
                    .append(constants::exposure::SHUTTER_CONTROL_PREFIX)
                    .push(match self {
                        Self::Reset => 0x00,
                        Self::Up => 0x02,
                        Self::Down => 0x03,
                        _ => unreachable!(),
                    })
                    .with_camera_id(camera_id)
                    .finalize();
                builder.copy_to(buffer)
            }
            Self::SetSpeed(speed) => {
                let mut builder = CommandBuilder::<9>::new();
                builder
                    .append(constants::exposure::SHUTTER_DIRECT_PREFIX)
                    .push_nibble_pair(speed.value())
                    .with_camera_id(camera_id)
                    .finalize();
                builder.copy_to(buffer)
            }
        }
    }

    fn response_type(&self) -> Option<ResponseType> {
        None
    }

    fn timeout_kind(&self) -> CommandCategory {
        CommandCategory::Quick
    }
}

/// Brightness control command.
#[derive(Debug, Clone, Copy)]
pub enum Bright {
    /// Reset brightness to default.
    Reset,
    /// Increase brightness.
    Up,
    /// Decrease brightness.
    Down,
    /// Set brightness to specific level.
    SetLevel(BrightnessLevel),
    /// Set brightness directly (Bright Direct mode).
    /// This is supported on Sony models but not on FR7.
    Direct(BrightnessLevel),
}

impl EncodeVisca for Bright {
    type Response = ();
    const MAX_SIZE: usize = 9;

    fn encode_into(
        &self,
        camera_id: crate::camera_id::CameraId,
        buffer: &mut [u8],
    ) -> Result<usize, Error> {
        use crate::command::const_encoding::{constants, CommandBuilder};

        match self {
            Self::Reset | Self::Up | Self::Down => {
                let mut builder = CommandBuilder::<6>::new();
                builder
                    .append(constants::exposure::BRIGHTNESS_CONTROL_PREFIX)
                    .push(match self {
                        Self::Reset => 0x00,
                        Self::Up => 0x02,
                        Self::Down => 0x03,
                        _ => unreachable!(),
                    })
                    .with_camera_id(camera_id)
                    .finalize();
                builder.copy_to(buffer)
            }
            Self::SetLevel(level) => {
                let mut builder = CommandBuilder::<9>::new();
                builder
                    .append(constants::exposure::BRIGHTNESS_DIRECT_PREFIX)
                    .push_nibble_pair(level.value())
                    .with_camera_id(camera_id)
                    .finalize();
                builder.copy_to(buffer)
            }
            Self::Direct(level) => {
                let mut builder = CommandBuilder::<9>::new();
                builder
                    .append(constants::exposure::BRIGHTNESS_VALUE_PREFIX)
                    .push_nibble_pair(level.value())
                    .with_camera_id(camera_id)
                    .finalize();
                builder.copy_to(buffer)
            }
        }
    }

    fn response_type(&self) -> Option<ResponseType> {
        None
    }

    fn timeout_kind(&self) -> CommandCategory {
        CommandCategory::Quick
    }
}

visca_command! {
    /// Spotlight command (Sony models).
    ///
    /// Controls the spotlight feature which enhances exposure for specific subjects.
    category = "Quick",
    enum Spotlight {
        /// Turn spotlight on
        On => {
            let cmd = crate::command::const_encoding::CommandBuilder::<6>::new()
                .append(constants::exposure::SPOTLIGHT_PREFIX)
                .append(&[0x02])
                .build();
            Ok::<Vec<u8>, Error>(cmd.to_vec())
        },
        /// Turn spotlight off
        Off => {
            let cmd = crate::command::const_encoding::CommandBuilder::<6>::new()
                .append(constants::exposure::SPOTLIGHT_PREFIX)
                .append(&[0x03])
                .build();
            Ok::<Vec<u8>, Error>(cmd.to_vec())
        },
    }
}

visca_command! {
    /// Auto Slow Shutter command.
    ///
    /// Controls the auto slow shutter feature which automatically reduces shutter speed
    /// in low light conditions to maintain proper exposure. This feature is supported
    /// on Sony cameras and FR7, but PTZOptics only supports it via HTTP API.
    category = "Quick",
    enum AutoSlowShutter {
        /// Turn auto slow shutter on
        On => {
            let cmd = crate::command::const_encoding::CommandBuilder::<6>::new()
                .append(constants::exposure::SPOT_AE_PREFIX)
                .append(&[0x02])
                .build();
            Ok::<Vec<u8>, Error>(cmd.to_vec())
        },
        /// Turn auto slow shutter off
        Off => {
            let cmd = crate::command::const_encoding::CommandBuilder::<6>::new()
                .append(constants::exposure::SPOT_AE_PREFIX)
                .append(&[0x03])
                .build();
            Ok::<Vec<u8>, Error>(cmd.to_vec())
        },
    }
}

#[cfg(test)]
#[allow(clippy::panic, clippy::unwrap_used)]
mod tests {
    use super::*;
    use crate::command::encode_visca::EncodeVisca;
    use crate::constants::CameraVariant;
    use crate::macros::test_utils::visca_test;

    // Test Auto mode
    visca_test!(
        ExposureCommand,
        test_exposure_mode_auto,
        ExposureCommand {
            mode: ExposureMode::Auto
        },
        &[0x81, 0x01, 0x04, 0x39, 0x00, 0xFF]
    );

    // Test Manual mode
    visca_test!(
        ExposureCommand,
        test_exposure_mode_manual,
        ExposureCommand {
            mode: ExposureMode::Manual
        },
        &[0x81, 0x01, 0x04, 0x39, 0x03, 0xFF]
    );

    // Test Shutter Priority mode
    visca_test!(
        ExposureCommand,
        test_exposure_mode_shutter,
        ExposureCommand {
            mode: ExposureMode::Shutter
        },
        &[0x81, 0x01, 0x04, 0x39, 0x0A, 0xFF]
    );

    // Test Iris Priority mode
    visca_test!(
        ExposureCommand,
        test_exposure_mode_iris,
        ExposureCommand {
            mode: ExposureMode::Iris
        },
        &[0x81, 0x01, 0x04, 0x39, 0x0B, 0xFF]
    );

    // Test Brightness Priority mode
    visca_test!(
        ExposureCommand,
        test_exposure_mode_bright,
        ExposureCommand {
            mode: ExposureMode::Bright
        },
        &[0x81, 0x01, 0x04, 0x39, 0x0D, 0xFF]
    );

    #[test]
    fn test_exposure_mode_try_from() {
        assert!(matches!(
            ExposureMode::try_from(0x00),
            Ok(ExposureMode::Auto)
        ));
        assert!(matches!(
            ExposureMode::try_from(0x03),
            Ok(ExposureMode::Manual)
        ));
        assert!(matches!(
            ExposureMode::try_from(0x0A),
            Ok(ExposureMode::Shutter)
        ));
        assert!(matches!(
            ExposureMode::try_from(0x0B),
            Ok(ExposureMode::Iris)
        ));
        assert!(matches!(
            ExposureMode::try_from(0x0D),
            Ok(ExposureMode::Bright)
        ));
        assert!(ExposureMode::try_from(0xFF).is_err());
    }

    #[test]
    fn test_exposure_compensation_level() {
        // Test valid values
        for value in -7..=7 {
            let level = ExposureCompensationLevel::new(value)
                .unwrap_or_else(|e| panic!("Test assertion failed: {e:?}"));
            assert_eq!(
                level.to_protocol_value(),
                u8::try_from(value + 7).unwrap_or_else(|e| panic!("Test assertion failed: {e:?}"))
            );
        }

        // Test invalid values
        assert!(ExposureCompensationLevel::new(-8).is_err());
        assert!(ExposureCompensationLevel::new(8).is_err());
    }

    // Test On command
    visca_test!(
        ExposureCompensation,
        test_exposure_compensation_on,
        ExposureCompensation::On,
        &[0x81, 0x01, 0x04, 0x3E, 0x02, 0xFF]
    );

    // Test Off command
    visca_test!(
        ExposureCompensation,
        test_exposure_compensation_off,
        ExposureCompensation::Off,
        &[0x81, 0x01, 0x04, 0x3E, 0x03, 0xFF]
    );

    // Test Reset command
    visca_test!(
        ExposureCompensation,
        test_exposure_compensation_reset,
        ExposureCompensation::Reset,
        &[0x81, 0x01, 0x04, 0x0E, 0x00, 0xFF]
    );

    // Test Up command
    visca_test!(
        ExposureCompensation,
        test_exposure_compensation_up,
        ExposureCompensation::Up,
        &[0x81, 0x01, 0x04, 0x0E, 0x02, 0xFF]
    );

    // Test Down command
    visca_test!(
        ExposureCompensation,
        test_exposure_compensation_down,
        ExposureCompensation::Down,
        &[0x81, 0x01, 0x04, 0x0E, 0x03, 0xFF]
    );

    // Test SetLevel command with value -7
    visca_test!(
        ExposureCompensation,
        test_exposure_compensation_level_minus_7,
        ExposureCompensation::SetLevel(ExposureCompensationLevel::new(-7).unwrap()),
        &[0x81, 0x01, 0x04, 0x4E, 0x00, 0x00, 0x00, 0x00, 0xFF]
    );

    // Test SetLevel command with value 0
    visca_test!(
        ExposureCompensation,
        test_exposure_compensation_level_0,
        ExposureCompensation::SetLevel(ExposureCompensationLevel::new(0).unwrap()),
        &[0x81, 0x01, 0x04, 0x4E, 0x00, 0x00, 0x00, 0x07, 0xFF]
    );

    // Test SetLevel command with value 7
    visca_test!(
        ExposureCompensation,
        test_exposure_compensation_level_plus_7,
        ExposureCompensation::SetLevel(ExposureCompensationLevel::new(7).unwrap()),
        &[0x81, 0x01, 0x04, 0x4E, 0x00, 0x00, 0x00, 0x0E, 0xFF]
    );

    #[test]
    fn test_exposure_compensation_g2_validation() {
        // Test valid G2 values
        for value in -7..=7 {
            let level = ExposureCompensationLevel::new(value)
                .unwrap_or_else(|e| panic!("Test assertion failed: {e:?}"));
            let cmd = ExposureCompensation::SetLevel(level);
            assert!(cmd.validate_for_model(CameraVariant::PTZOpticsG2).is_ok());
        }

        // Other command types should always be valid
        assert!(ExposureCompensation::On
            .validate_for_model(CameraVariant::PTZOpticsG2)
            .is_ok());
        assert!(ExposureCompensation::Off
            .validate_for_model(CameraVariant::PTZOpticsG2)
            .is_ok());
    }

    // Test dynamic range level 0
    visca_test!(
        DynamicRange,
        test_dynamic_range_level_0,
        DynamicRange::new(DynamicRangeLevel::new(0).unwrap()),
        &[0x81, 0x01, 0x04, 0x25, 0x00, 0x00, 0x00, 0x00, 0xFF]
    );

    // Test dynamic range level 4
    visca_test!(
        DynamicRange,
        test_dynamic_range_level_4,
        DynamicRange::new(DynamicRangeLevel::new(4).unwrap()),
        &[0x81, 0x01, 0x04, 0x25, 0x00, 0x00, 0x00, 0x04, 0xFF]
    );

    // Test dynamic range level 8
    visca_test!(
        DynamicRange,
        test_dynamic_range_level_8,
        DynamicRange::new(DynamicRangeLevel::new(8).unwrap()),
        &[0x81, 0x01, 0x04, 0x25, 0x00, 0x00, 0x00, 0x08, 0xFF]
    );

    #[test]
    fn test_dynamic_range_g2_validation() {
        // Test valid G2 values
        for value in 0..=8 {
            let level = DynamicRangeLevel::new(value)
                .unwrap_or_else(|e| panic!("Test assertion failed: {e:?}"));
            let cmd = DynamicRange::new(level);
            assert!(cmd.validate_for_model(CameraVariant::PTZOpticsG2).is_ok());
        }
    }

    // Test Reset command
    visca_test!(
        Iris,
        test_iris_reset,
        Iris::Reset,
        &[0x81, 0x01, 0x04, 0x0B, 0x00, 0xFF]
    );

    // Test Up command
    visca_test!(
        Iris,
        test_iris_up,
        Iris::Up,
        &[0x81, 0x01, 0x04, 0x0B, 0x02, 0xFF]
    );

    // Test Down command
    visca_test!(
        Iris,
        test_iris_down,
        Iris::Down,
        &[0x81, 0x01, 0x04, 0x0B, 0x03, 0xFF]
    );

    // Test SetAperture command with value 0x00
    visca_test!(
        Iris,
        test_iris_set_aperture_00,
        Iris::SetAperture(IrisLevel::new(0x00).unwrap()),
        &[0x81, 0x01, 0x04, 0x4B, 0x00, 0x00, 0x00, 0x00, 0xFF]
    );

    // Test SetAperture command with value 0x05
    visca_test!(
        Iris,
        test_iris_set_aperture_05,
        Iris::SetAperture(IrisLevel::new(0x05).unwrap()),
        &[0x81, 0x01, 0x04, 0x4B, 0x00, 0x00, 0x00, 0x05, 0xFF]
    );

    // Test SetAperture command with value 0x0A
    visca_test!(
        Iris,
        test_iris_set_aperture_0a,
        Iris::SetAperture(IrisLevel::new(0x0A).unwrap()),
        &[0x81, 0x01, 0x04, 0x4B, 0x00, 0x00, 0x00, 0x0A, 0xFF]
    );

    // Test SetAperture command with value 0x0C
    visca_test!(
        Iris,
        test_iris_set_aperture_0c,
        Iris::SetAperture(IrisLevel::new(0x0C).unwrap()),
        &[0x81, 0x01, 0x04, 0x4B, 0x00, 0x00, 0x00, 0x0C, 0xFF]
    );

    #[test]
    fn test_iris_g2_validation() {
        // Test valid G2 iris values
        for value in 0x00..=0x0C {
            let level =
                IrisLevel::new(value).unwrap_or_else(|e| panic!("Test assertion failed: {e:?}"));
            let cmd = Iris::SetAperture(level);
            assert!(cmd.validate_for_model(CameraVariant::PTZOpticsG2).is_ok());
        }

        // Test that non-G2 valid value still passes validation (G2 is more restrictive)
        // The general IrisLevel accepts values up to 0x0C, which are all valid for G2

        // Non-direct commands should always be valid
        assert!(Iris::Reset
            .validate_for_model(CameraVariant::PTZOpticsG2)
            .is_ok());
    }

    // Test Reset command
    visca_test!(
        Shutter,
        test_shutter_reset,
        Shutter::Reset,
        &[0x81, 0x01, 0x04, 0x0A, 0x00, 0xFF]
    );

    // Test Up command
    visca_test!(
        Shutter,
        test_shutter_up,
        Shutter::Up,
        &[0x81, 0x01, 0x04, 0x0A, 0x02, 0xFF]
    );

    // Test Down command
    visca_test!(
        Shutter,
        test_shutter_down,
        Shutter::Down,
        &[0x81, 0x01, 0x04, 0x0A, 0x03, 0xFF]
    );

    // Test SetSpeed command with value 0x01
    visca_test!(
        Shutter,
        test_shutter_set_speed_01,
        Shutter::SetSpeed(ShutterSpeed::new(0x01).unwrap()),
        &[0x81, 0x01, 0x04, 0x4A, 0x00, 0x00, 0x00, 0x01, 0xFF]
    );

    // Test SetSpeed command with value 0x05
    visca_test!(
        Shutter,
        test_shutter_set_speed_05,
        Shutter::SetSpeed(ShutterSpeed::new(0x05).unwrap()),
        &[0x81, 0x01, 0x04, 0x4A, 0x00, 0x00, 0x00, 0x05, 0xFF]
    );

    // Test SetSpeed command with value 0x10
    visca_test!(
        Shutter,
        test_shutter_set_speed_10,
        Shutter::SetSpeed(ShutterSpeed::new(0x10).unwrap()),
        &[0x81, 0x01, 0x04, 0x4A, 0x00, 0x00, 0x01, 0x00, 0xFF]
    );

    // Test SetSpeed command with value 0x11
    visca_test!(
        Shutter,
        test_shutter_set_speed_11,
        Shutter::SetSpeed(ShutterSpeed::new(0x11).unwrap()),
        &[0x81, 0x01, 0x04, 0x4A, 0x00, 0x00, 0x01, 0x01, 0xFF]
    );

    #[test]
    fn test_shutter_g2_validation() {
        // Test valid G2 shutter values
        for value in 0x01..=0x11 {
            let speed =
                ShutterSpeed::new(value).unwrap_or_else(|e| panic!("Test assertion failed: {e:?}"));
            let cmd = Shutter::SetSpeed(speed);
            assert!(cmd.validate_for_model(CameraVariant::PTZOpticsG2).is_ok());
        }

        // Test that non-G2 valid value still passes validation (G2 is more restrictive)
        // The general ShutterSpeed accepts values up to 0x11, which are all valid for G2

        // Non-direct commands should always be valid
        assert!(Shutter::Reset
            .validate_for_model(CameraVariant::PTZOpticsG2)
            .is_ok());
    }

    // Test Reset command
    visca_test!(
        Bright,
        test_bright_reset,
        Bright::Reset,
        &[0x81, 0x01, 0x04, 0x0D, 0x00, 0xFF]
    );

    // Test Up command
    visca_test!(
        Bright,
        test_bright_up,
        Bright::Up,
        &[0x81, 0x01, 0x04, 0x0D, 0x02, 0xFF]
    );

    // Test Down command
    visca_test!(
        Bright,
        test_bright_down,
        Bright::Down,
        &[0x81, 0x01, 0x04, 0x0D, 0x03, 0xFF]
    );

    // Test SetLevel command with value 0x00
    visca_test!(
        Bright,
        test_bright_set_level_00,
        Bright::SetLevel(BrightnessLevel::new(0x00).unwrap()),
        &[0x81, 0x01, 0x04, 0x4D, 0x00, 0x00, 0x00, 0x00, 0xFF]
    );

    // Test SetLevel command with value 0x08
    visca_test!(
        Bright,
        test_bright_set_level_08,
        Bright::SetLevel(BrightnessLevel::new(0x08).unwrap()),
        &[0x81, 0x01, 0x04, 0x4D, 0x00, 0x00, 0x00, 0x08, 0xFF]
    );

    // Test SetLevel command with value 0x10
    visca_test!(
        Bright,
        test_bright_set_level_10,
        Bright::SetLevel(BrightnessLevel::new(0x10).unwrap()),
        &[0x81, 0x01, 0x04, 0x4D, 0x00, 0x00, 0x01, 0x00, 0xFF]
    );

    // Test SetLevel command with value 0x11
    visca_test!(
        Bright,
        test_bright_set_level_11,
        Bright::SetLevel(BrightnessLevel::new(0x11).unwrap()),
        &[0x81, 0x01, 0x04, 0x4D, 0x00, 0x00, 0x01, 0x01, 0xFF]
    );

    // Test Direct command with value 0x00
    visca_test!(
        Bright,
        test_bright_direct_00,
        Bright::Direct(BrightnessLevel::new(0x00).unwrap()),
        &[0x81, 0x01, 0x04, 0x0D, 0x00, 0x00, 0x00, 0x00, 0xFF]
    );

    // Test Direct command with value 0x08
    visca_test!(
        Bright,
        test_bright_direct_08,
        Bright::Direct(BrightnessLevel::new(0x08).unwrap()),
        &[0x81, 0x01, 0x04, 0x0D, 0x00, 0x00, 0x00, 0x08, 0xFF]
    );

    // Test Direct command with value 0x10
    visca_test!(
        Bright,
        test_bright_direct_10,
        Bright::Direct(BrightnessLevel::new(0x10).unwrap()),
        &[0x81, 0x01, 0x04, 0x0D, 0x00, 0x00, 0x01, 0x00, 0xFF]
    );

    // Test Direct command with value 0x11
    visca_test!(
        Bright,
        test_bright_direct_11,
        Bright::Direct(BrightnessLevel::new(0x11).unwrap()),
        &[0x81, 0x01, 0x04, 0x0D, 0x00, 0x00, 0x01, 0x01, 0xFF]
    );

    #[test]
    fn test_bright_g2_validation() {
        // Test valid G2 brightness values
        for value in 0x00..=0x11 {
            let level = BrightnessLevel::new(value)
                .unwrap_or_else(|e| panic!("Test assertion failed: {e:?}"));
            let cmd = Bright::SetLevel(level);
            assert!(cmd.validate_for_model(CameraVariant::PTZOpticsG2).is_ok());
        }

        // Test that non-G2 valid value still passes validation (G2 is more restrictive)
        // The general BrightnessLevel accepts values up to 0x11, which are all valid for G2

        // Non-direct commands should always be valid
        assert!(Bright::Reset
            .validate_for_model(CameraVariant::PTZOpticsG2)
            .is_ok());
    }

    #[test]
    fn test_command_categories() {
        // All exposure commands should be Quick category
        assert_eq!(
            ExposureCommand {
                mode: ExposureMode::Auto
            }
            .timeout_kind(),
            CommandCategory::Quick
        );
        assert_eq!(
            ExposureCompensation::On.timeout_kind(),
            CommandCategory::Quick
        );
        assert_eq!(
            DynamicRange::new(
                DynamicRangeLevel::new(5)
                    .unwrap_or_else(|e| panic!("Test assertion failed: {e:?}"))
            )
            .timeout_kind(),
            CommandCategory::Quick
        );
        assert_eq!(Iris::Reset.timeout_kind(), CommandCategory::Quick);
        assert_eq!(Shutter::Reset.timeout_kind(), CommandCategory::Quick);
        assert_eq!(Bright::Reset.timeout_kind(), CommandCategory::Quick);
    }

    #[test]
    fn test_response_types() {
        // All exposure commands should return None for response_type
        assert!(ExposureCommand {
            mode: ExposureMode::Auto
        }
        .response_type()
        .is_none());
        assert!(ExposureCompensation::On.response_type().is_none());
        assert!(DynamicRange::new(
            DynamicRangeLevel::new(5).unwrap_or_else(|e| panic!("Test assertion failed: {e:?}"))
        )
        .response_type()
        .is_none());
        assert!(Iris::Reset.response_type().is_none());
        assert!(Shutter::Reset.response_type().is_none());
        assert!(Bright::Reset.response_type().is_none());
    }

    // Test On command
    visca_test!(
        AutoSlowShutter,
        test_auto_slow_shutter_on,
        AutoSlowShutter::On,
        &[0x81, 0x01, 0x04, 0x5A, 0x02, 0xFF]
    );

    // Test Off command
    visca_test!(
        AutoSlowShutter,
        test_auto_slow_shutter_off,
        AutoSlowShutter::Off,
        &[0x81, 0x01, 0x04, 0x5A, 0x03, 0xFF]
    );
}
