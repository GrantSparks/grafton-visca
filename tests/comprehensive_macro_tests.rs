#![allow(dead_code)]

//! Comprehensive tests demonstrating the enhanced macro capabilities
//! These tests show:
//! 1. Async/sync generation for position, speed, and bounded commands
//! 2. Integration with camera profile system
//! 3. Error handling and validation
//! 4. Generated helper methods (degrees, normalized, percentage)

use grafton_visca::Error;
use grafton_visca_macros::*;

// Mock SpeedLevel type (normally would come from grafton_visca::types)
#[derive(Debug, Clone, Copy)]
pub enum SpeedLevel {
    Slow,
    Medium,
    Fast,
}

impl SpeedLevel {
    fn to_pan_speed(self) -> u8 {
        match self {
            SpeedLevel::Slow => 1,
            SpeedLevel::Medium => 12,
            SpeedLevel::Fast => 24,
        }
    }

    fn to_tilt_speed(self) -> u8 {
        match self {
            SpeedLevel::Slow => 1,
            SpeedLevel::Medium => 10,
            SpeedLevel::Fast => 20,
        }
    }
}

// Mock camera profile for tests
#[derive(Debug, Clone)]
struct TestProfile;

impl TestProfile {
    fn pan_degrees_to_units(&self, degrees: f32) -> i16 {
        (degrees * 100.0) as i16
    }

    fn tilt_degrees_to_units(&self, degrees: f32) -> i16 {
        (degrees * 100.0) as i16
    }

    fn normalized_to_pan_units(&self, normalized: f32) -> i16 {
        ((normalized - 0.5) * 34000.0) as i16
    }

    fn normalized_to_tilt_units(&self, normalized: f32) -> i16 {
        ((normalized - 0.5) * 12000.0) as i16
    }
}

// Mock position types
#[derive(Debug, Clone, Copy, PartialEq)]
struct PanPosition(i16);

impl PanPosition {
    fn new(val: i16) -> std::result::Result<Self, Error> {
        if !(-17000..=17000).contains(&val) {
            return Err(Error::ParameterOutOfRange {
                parameter: "pan".to_string(),
                value: val as i32,
                min: -17000,
                max: 17000,
            });
        }
        Ok(Self(val))
    }

    fn value(&self) -> i16 {
        self.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
struct TiltPosition(i16);

impl TiltPosition {
    fn new(val: i16) -> std::result::Result<Self, Error> {
        if !(-3000..=9000).contains(&val) {
            return Err(Error::ParameterOutOfRange {
                parameter: "tilt".to_string(),
                value: val as i32,
                min: -3000,
                max: 9000,
            });
        }
        Ok(Self(val))
    }

    fn value(&self) -> i16 {
        self.0
    }
}

// Mock command types
#[derive(Debug, PartialEq)]
struct AbsolutePositionCommand {
    pan: PanPosition,
    tilt: TiltPosition,
}

#[derive(Debug, PartialEq)]
struct MoveWithSpeedCommand {
    pan_speed: u8,
    tilt_speed: u8,
}

#[derive(Debug, PartialEq)]
struct BrightnessCommand {
    level: u8,
}

// Mock transport and camera
#[cfg(feature = "async")]
#[derive(Debug)]
struct AsyncTransport;

#[cfg(not(feature = "async"))]
#[derive(Debug)]
struct BlockingTransport;

// Test camera implementation with all macro types
struct TestCamera<T> {
    transport: T,
    profile: TestProfile,
}

impl<T> TestCamera<T> {
    fn new(transport: T) -> Self {
        Self {
            transport,
            profile: TestProfile,
        }
    }
}

#[cfg(feature = "async")]
impl TestCamera<AsyncTransport> {
    async fn send_and_wait<C>(&self, _cmd: &C) -> std::result::Result<(), Error> {
        Ok(())
    }
}

#[cfg(not(feature = "async"))]
impl TestCamera<BlockingTransport> {
    fn send_and_wait<C>(&mut self, _cmd: &C) -> std::result::Result<(), Error> {
        Ok(())
    }
}

#[cfg(feature = "async")]
impl TestCamera<AsyncTransport> {
    // Position command with validation and helper methods
    #[visca_position_command(pan_range = "-17000..=17000", tilt_range = "-3000..=9000")]
    pub fn set_absolute_position(&self, pan: i16, tilt: i16) -> std::result::Result<(), Error> {
        let _cmd = AbsolutePositionCommand {
            pan: PanPosition::new(pan)?,
            tilt: TiltPosition::new(tilt)?,
        };

        // In real implementation, this would send the command
        Ok(())
    }

    // Speed command with validation
    #[visca_speed_command(pan_speed_range = "0x01..=0x18", tilt_speed_range = "0x01..=0x14")]
    pub fn move_with_speed(&self, pan_speed: u8, tilt_speed: u8) -> std::result::Result<(), Error> {
        let _cmd = MoveWithSpeedCommand {
            pan_speed,
            tilt_speed,
        };

        // In real implementation, this would send the command
        Ok(())
    }

    // Bounded command with percentage helper
    #[visca_bounded_command(brightness = "0..=10")]
    pub fn set_brightness(&self, brightness: u8) -> std::result::Result<(), Error> {
        let _cmd = BrightnessCommand { level: brightness };

        // In real implementation, this would send the command
        Ok(())
    }

    // Fallible method that works in both modes
    #[visca_fallible_method]
    pub fn execute_complex_command(
        &self,
        pan: i16,
        tilt: i16,
    ) -> std::result::Result<AbsolutePositionCommand, Error> {
        if pan == 0 && tilt == 0 {
            return Err(Error::InvalidParameter(
                "Cannot create command for (0,0)".to_string(),
            ));
        }

        Ok(AbsolutePositionCommand {
            pan: PanPosition::new(pan)?,
            tilt: TiltPosition::new(tilt)?,
        })
    }
}

#[cfg(not(feature = "async"))]
impl TestCamera<BlockingTransport> {
    // Position command with validation and helper methods
    #[visca_position_command(pan_range = "-17000..=17000", tilt_range = "-3000..=9000")]
    pub fn set_absolute_position(&self, pan: i16, tilt: i16) -> std::result::Result<(), Error> {
        let _cmd = AbsolutePositionCommand {
            pan: PanPosition::new(pan)?,
            tilt: TiltPosition::new(tilt)?,
        };

        // In real implementation, this would send the command
        Ok(())
    }

    // Speed command with validation
    #[visca_speed_command(pan_speed_range = "0x01..=0x18", tilt_speed_range = "0x01..=0x14")]
    pub fn move_with_speed(
        &self,
        pan_speed: u8,
        tilt_speed: u8,
    ) -> std::result::Result<(), Error> {
        let _cmd = MoveWithSpeedCommand {
            pan_speed,
            tilt_speed,
        };

        // In real implementation, this would send the command
        Ok(())
    }

    // Bounded command with percentage helper
    #[visca_bounded_command(brightness = "0..=10")]
    pub fn set_brightness(&self, brightness: u8) -> std::result::Result<(), Error> {
        let _cmd = BrightnessCommand { level: brightness };

        // In real implementation, this would send the command
        Ok(())
    }

    // Fallible method that works in both modes
    #[visca_fallible_method]
    pub fn execute_complex_command(
        &self,
        pan: i16,
        tilt: i16,
    ) -> std::result::Result<AbsolutePositionCommand, Error> {
        if pan == 0 && tilt == 0 {
            return Err(Error::InvalidParameter(
                "Cannot create command for (0,0)".to_string(),
            ));
        }

        Ok(AbsolutePositionCommand {
            pan: PanPosition::new(pan)?,
            tilt: TiltPosition::new(tilt)?,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(feature = "async")]
    #[tokio::test]
    async fn test_position_command_async() {
        let transport = AsyncTransport;
        let camera = TestCamera::new(transport);

        // Test basic position setting
        assert!(camera.set_absolute_position(0, 0).await.is_ok());
        assert!(camera.set_absolute_position(5000, 3000).await.is_ok());

        // Test range validation
        assert!(camera.set_absolute_position(20000, 0).await.is_err());
        assert!(camera.set_absolute_position(0, 10000).await.is_err());

        // Test degrees helper method
        assert!(camera
            .set_absolute_position_degrees(45.0, 30.0)
            .await
            .is_ok());
        assert!(camera
            .set_absolute_position_degrees(-170.0, -30.0)
            .await
            .is_ok());

        // Test normalized helper method
        assert!(camera
            .set_absolute_position_normalized(0.5, 0.5)
            .await
            .is_ok());
        assert!(camera
            .set_absolute_position_normalized(0.0, 1.0)
            .await
            .is_ok());
    }

    #[cfg(not(feature = "async"))]
    #[test]
    fn test_position_command_blocking() {
        let transport = BlockingTransport;
        let mut camera = TestCamera::new(transport);

        // Test basic position setting
        assert!(camera.set_absolute_position(0, 0).is_ok());
        assert!(camera.set_absolute_position(5000, 3000).is_ok());

        // Test range validation
        assert!(camera.set_absolute_position(20000, 0).is_err());
        assert!(camera.set_absolute_position(0, 10000).is_err());

        // Test degrees helper method
        assert!(camera.set_absolute_position_degrees(45.0, 30.0).is_ok());
        assert!(camera.set_absolute_position_degrees(-170.0, -30.0).is_ok());

        // Test normalized helper method
        assert!(camera.set_absolute_position_normalized(0.5, 0.5).is_ok());
        assert!(camera.set_absolute_position_normalized(0.0, 1.0).is_ok());
    }

    #[cfg(feature = "async")]
    #[tokio::test]
    async fn test_speed_command_async() {
        let transport = AsyncTransport;
        let camera = TestCamera::new(transport);

        // Test valid speeds
        assert!(camera.move_with_speed(0x10, 0x10).await.is_ok());
        assert!(camera.move_with_speed(0x01, 0x01).await.is_ok());
        assert!(camera.move_with_speed(0x18, 0x14).await.is_ok());

        // Test that speed level variant was generated
        assert!(camera
            .move_with_speed_with_level(SpeedLevel::Slow, SpeedLevel::Slow)
            .await
            .is_ok());
        assert!(camera
            .move_with_speed_with_level(SpeedLevel::Fast, SpeedLevel::Fast)
            .await
            .is_ok());
    }

    #[cfg(not(feature = "async"))]
    #[test]
    fn test_speed_command_blocking() {
        let transport = BlockingTransport;
        let mut camera = TestCamera::new(transport);

        // Test valid speeds
        assert!(camera.move_with_speed(0x10, 0x10).is_ok());
        assert!(camera.move_with_speed(0x01, 0x01).is_ok());
        assert!(camera.move_with_speed(0x18, 0x14).is_ok());

        // Test that speed level variant was generated
        assert!(camera
            .move_with_speed_with_level(SpeedLevel::Slow, SpeedLevel::Slow)
            .is_ok());
        assert!(camera
            .move_with_speed_with_level(SpeedLevel::Fast, SpeedLevel::Fast)
            .is_ok());
    }

    #[cfg(feature = "async")]
    #[tokio::test]
    async fn test_bounded_command_async() {
        let transport = AsyncTransport;
        let camera = TestCamera::new(transport);

        // Test valid brightness levels
        assert!(camera.set_brightness(0).await.is_ok());
        assert!(camera.set_brightness(5).await.is_ok());
        assert!(camera.set_brightness(10).await.is_ok());

        // Test brightness validation
        assert!(camera.set_brightness(11).await.is_err());

        // Test percentage helper method
        assert!(camera.set_brightness_percentage(0.0).await.is_ok());
        assert!(camera.set_brightness_percentage(50.0).await.is_ok());
        assert!(camera.set_brightness_percentage(100.0).await.is_ok());
        assert!(camera.set_brightness_percentage(101.0).await.is_err());
        assert!(camera.set_brightness_percentage(-1.0).await.is_err());
    }

    #[cfg(not(feature = "async"))]
    #[test]
    fn test_bounded_command_blocking() {
        let transport = BlockingTransport;
        let mut camera = TestCamera::new(transport);

        // Test valid brightness levels
        assert!(camera.set_brightness(0).is_ok());
        assert!(camera.set_brightness(5).is_ok());
        assert!(camera.set_brightness(10).is_ok());

        // Test brightness validation
        assert!(camera.set_brightness(11).is_err());

        // Test percentage helper method
        assert!(camera.set_brightness_percentage(0.0).is_ok());
        assert!(camera.set_brightness_percentage(50.0).is_ok());
        assert!(camera.set_brightness_percentage(100.0).is_ok());
        assert!(camera.set_brightness_percentage(101.0).is_err());
        assert!(camera.set_brightness_percentage(-1.0).is_err());
    }

    #[cfg(feature = "async")]
    #[tokio::test]
    async fn test_fallible_method_async() {
        let transport = AsyncTransport;
        let camera = TestCamera::new(transport);

        // Test successful command execution
        assert!(camera.execute_complex_command(1000, 2000).await.is_ok());

        // Test custom validation error
        assert!(camera.execute_complex_command(0, 0).await.is_err());

        // Test parameter validation error
        assert!(camera.execute_complex_command(20000, 0).await.is_err());
    }

    #[cfg(not(feature = "async"))]
    #[test]
    fn test_fallible_method_blocking() {
        let transport = BlockingTransport;
        let mut camera = TestCamera::new(transport);

        // Test successful command execution
        assert!(camera.execute_complex_command(1000, 2000).is_ok());

        // Test custom validation error
        assert!(camera.execute_complex_command(0, 0).is_err());

        // Test parameter validation error
        assert!(camera.execute_complex_command(20000, 0).is_err());
    }
}

// Test ViscaValue derive macro with different display formats
#[cfg(test)]
mod visca_value_tests {
    use super::*;

    #[derive(Debug, Clone, Copy, PartialEq, Eq, ViscaValue)]
    #[visca_value(min = "0x00", max = "0xFF", display_format = "hex")]
    pub struct HexValue(u8);

    #[derive(Debug, Clone, Copy, PartialEq, Eq, ViscaValue)]
    #[visca_value(min = "0", max = "100", display_format = "decimal")]
    pub struct DecimalValue(u8);

    #[derive(Debug, Clone, Copy, PartialEq, Eq, ViscaValue)]
    #[visca_value(min = "0", max = "7", display_format = "binary")]
    pub struct BinaryValue(u8);

    #[test]
    fn test_visca_value_display_formats() {
        // Test hex format
        let hex = HexValue::new(255).unwrap();
        assert_eq!(format!("{}", hex), "0xff");
        assert_eq!(hex.value(), 255);

        // Test decimal format
        let dec = DecimalValue::new(42).unwrap();
        assert_eq!(format!("{}", dec), "42");
        assert_eq!(dec.value(), 42);

        // Test binary format
        let bin = BinaryValue::new(5).unwrap();
        assert_eq!(format!("{}", bin), "0b101");
        assert_eq!(bin.value(), 5);

        // Test validation
        assert!(DecimalValue::new(101).is_err());
        assert!(BinaryValue::new(8).is_err());

        // Test conversions
        let val: u8 = hex.into();
        assert_eq!(val, 255);

        let hex2: HexValue = 128u8.try_into().unwrap();
        assert_eq!(hex2.value(), 128);
    }
}
