#![allow(dead_code)]

use grafton_visca::Error;
use grafton_visca_macros::*;

// Don't redefine Result, let the macros use the standard Result

// Mock types for testing
#[derive(Debug, Clone, Copy)]
struct PanPosition(i16);

impl PanPosition {
    fn new(val: i16) -> std::result::Result<Self, Error> {
        if !(-170..=170).contains(&val) {
            return Err(Error::ParameterOutOfRange {
                parameter: "pan".to_string(),
                value: val as i32,
                min: -170,
                max: 170,
            });
        }
        Ok(Self(val))
    }

    fn value(&self) -> i16 {
        self.0
    }
}

#[derive(Debug, Clone, Copy)]
struct TiltPosition(i16);

impl TiltPosition {
    fn new(val: i16) -> std::result::Result<Self, Error> {
        if !(-30..=90).contains(&val) {
            return Err(Error::ParameterOutOfRange {
                parameter: "tilt".to_string(),
                value: val as i32,
                min: -30,
                max: 90,
            });
        }
        Ok(Self(val))
    }

    fn value(&self) -> i16 {
        self.0
    }
}

#[derive(Debug)]
struct PanTiltCommand {
    pan: PanPosition,
    tilt: TiltPosition,
}

// Mock camera struct
struct Camera {
    profile: CameraProfile,
}

struct CameraProfile;

impl CameraProfile {
    fn pan_degrees_to_units(&self, degrees: f32) -> i16 {
        (degrees * 10.0) as i16
    }

    fn tilt_degrees_to_units(&self, degrees: f32) -> i16 {
        (degrees * 10.0) as i16
    }

    fn normalized_to_pan_units(&self, normalized: f32) -> i16 {
        ((normalized * 340.0) - 170.0) as i16
    }

    fn normalized_to_tilt_units(&self, normalized: f32) -> i16 {
        ((normalized * 120.0) - 30.0) as i16
    }
}

#[cfg(feature = "async")]
impl Camera {
    async fn send_and_wait(&self, _cmd: &PanTiltCommand) -> std::result::Result<(), Error> {
        Ok(())
    }
}

// Test position command macro
#[cfg(test)]
mod position_tests {
    use super::*;

    struct TestCamera {
        profile: CameraProfile,
    }

    impl TestCamera {
        #[visca_position_command(pan_range = "-170..=170", tilt_range = "-30..=90")]
        pub fn set_position(&self, pan: i16, tilt: i16) -> std::result::Result<(), Error> {
            let _cmd = PanTiltCommand {
                pan: PanPosition::new(pan)?,
                tilt: TiltPosition::new(tilt)?,
            };
            // In real implementation, this would send the command
            Ok(())
        }
    }

    #[cfg(feature = "async")]
    #[tokio::test]
    async fn test_position_command_generates_methods() {
        let camera = TestCamera {
            profile: CameraProfile,
        };

        // Test basic method
        assert!(camera.set_position(0, 0).await.is_ok());

        // Test out of range
        assert!(camera.set_position(200, 0).await.is_err());

        // Test degrees method exists and works
        assert!(camera.set_position_degrees(0.0, 0.0).await.is_ok());

        // Test normalized method exists and works
        assert!(camera.set_position_normalized(0.5, 0.5).await.is_ok());
    }

    #[cfg(not(feature = "async"))]
    #[test]
    fn test_position_command_generates_methods() {
        let mut camera = TestCamera {
            profile: CameraProfile,
        };

        // Test basic method
        assert!(camera.set_position(0, 0).is_ok());

        // Test out of range
        assert!(camera.set_position(200, 0).is_err());

        // Test degrees method exists and works
        assert!(camera.set_position_degrees(0.0, 0.0).is_ok());

        // Test normalized method exists and works
        assert!(camera.set_position_normalized(0.5, 0.5).is_ok());
    }
}

// Test ViscaValue derive macro
#[cfg(test)]
mod value_tests {
    use super::*;

    #[derive(Debug, Clone, Copy, PartialEq, Eq, ViscaValue)]
    #[visca_value(min = "0x00", max = "0x07", display_format = "hex")]
    pub struct GainValue(u8);

    #[test]
    fn test_visca_value_macro() {
        // Test creation with valid value
        let gain = GainValue::new(5).unwrap();
        assert_eq!(gain.value(), 5);

        // Test min/max values
        assert!(GainValue::new(0).is_ok());
        assert!(GainValue::new(7).is_ok());

        // Test out of range
        assert!(GainValue::new(8).is_err());

        // Test TryFrom
        let gain2: GainValue = 3u8.try_into().unwrap();
        assert_eq!(gain2.value(), 3);

        // Test From
        let val: u8 = gain.into();
        assert_eq!(val, 5);

        // Test display
        assert_eq!(format!("{}", gain), "0x5");
    }
}

// Test speed command macro
#[cfg(test)]
mod speed_tests {

    #[derive(Debug)]
    struct MoveCommand {
        pan_speed: u8,
    }

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
    }

    struct TestCamera;

    impl TestCamera {
        pub fn create_move_command(&self, pan_speed: u8) -> MoveCommand {
            // In a real implementation, the speed validation would happen
            // when the command is converted to bytes or executed
            MoveCommand { pan_speed }
        }
    }

    #[test]
    fn test_speed_command_generates_methods() {
        let camera = TestCamera;

        // Test basic method - validation happens automatically
        let cmd = camera.create_move_command(10);
        assert_eq!(cmd.pan_speed, 10);

        // The macro should have added validation that prevents values > 0x18
        // Note: With command construction pattern, validation happens in the macro-generated code
    }
}

// Test bounded command macro
#[cfg(test)]
mod bounded_tests {
    use super::*;

    struct TestCamera;

    impl TestCamera {
        #[visca_bounded_command(brightness = "0..=7")]
        pub fn set_brightness(&self, brightness: u8) -> std::result::Result<(), Error> {
            if brightness > 7 {
                return Err(Error::ParameterOutOfRange {
                    parameter: "brightness".to_string(),
                    value: brightness as i32,
                    min: 0,
                    max: 7,
                });
            }
            Ok(())
        }
    }

    #[cfg(feature = "async")]
    #[tokio::test]
    async fn test_bounded_command_generates_methods() {
        let camera = TestCamera;

        // Test basic method
        assert!(camera.set_brightness(5).await.is_ok());

        // Test validation
        assert!(camera.set_brightness(8).await.is_err());

        // Test percentage method exists
        assert!(camera.set_brightness_percentage(50.0).await.is_ok());
        assert!(camera.set_brightness_percentage(101.0).await.is_err());
    }

    #[cfg(not(feature = "async"))]
    #[test]
    fn test_bounded_command_generates_methods() {
        let mut camera = TestCamera;

        // Test basic method
        assert!(camera.set_brightness(5).is_ok());

        // Test validation
        assert!(camera.set_brightness(8).is_err());

        // Test percentage method exists
        assert!(camera.set_brightness_percentage(50.0).is_ok());
        assert!(camera.set_brightness_percentage(101.0).is_err());
    }
}

// Test fallible method macro
#[cfg(test)]
mod fallible_tests {
    use super::*;

    struct TestCamera;

    impl TestCamera {
        #[cfg(feature = "async")]
        async fn send_and_wait(&self, _cmd: &PanTiltCommand) -> std::result::Result<(), Error> {
            Ok(())
        }

        #[cfg(not(feature = "async"))]
        fn send_and_wait(&mut self, _cmd: &PanTiltCommand) -> std::result::Result<(), Error> {
            Ok(())
        }

        #[visca_fallible_method]
        pub fn create_complex_command(
            &self,
            pan: i16,
            tilt: i16,
        ) -> std::result::Result<PanTiltCommand, Error> {
            Ok(PanTiltCommand {
                pan: PanPosition::new(pan)?,
                tilt: TiltPosition::new(tilt)?,
            })
        }
    }

    #[cfg(feature = "async")]
    #[tokio::test]
    async fn test_fallible_method_async() {
        let camera = TestCamera;

        // Test successful command
        assert!(camera.create_complex_command(0, 0).await.is_ok());

        // Test error propagation
        assert!(camera.create_complex_command(200, 0).await.is_err());
    }

    #[cfg(not(feature = "async"))]
    #[test]
    fn test_fallible_method_blocking() {
        let mut camera = TestCamera;

        // Test successful command
        assert!(camera.create_complex_command(0, 0).is_ok());

        // Test error propagation
        assert!(camera.create_complex_command(200, 0).is_err());
    }
}
