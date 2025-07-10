//! Pan/Tilt methods for cameras that support movement.

use crate::camera::Camera;
use crate::capabilities::pan_tilt::PanTiltExt;
use crate::capabilities::{PanTilt, ProfileMetadata};
use crate::command::const_encoding::{
    constants::pan_tilt, encode_pan_tilt_absolute, encode_pan_tilt_relative,
};
use crate::Error;

/// Extension trait that adds pan/tilt methods to cameras.
#[allow(async_fn_in_trait)]
pub trait PanTiltMethodsExt {
    /// Stop all pan/tilt movement.
    #[cfg(not(feature = "async"))]
    fn pan_tilt_stop(&mut self) -> Result<(), Error>;

    /// Stop all pan/tilt movement.
    #[cfg(feature = "async")]
    async fn pan_tilt_stop(&self) -> Result<(), Error>;

    /// Move to home position (0, 0).
    #[cfg(not(feature = "async"))]
    fn pan_tilt_home(&mut self) -> Result<(), Error>;

    /// Move to home position (0, 0).
    #[cfg(feature = "async")]
    async fn pan_tilt_home(&self) -> Result<(), Error>;

    /// Move to absolute pan/tilt position in degrees.
    #[cfg(not(feature = "async"))]
    fn pan_tilt_absolute(
        &mut self,
        pan_degrees: f32,
        tilt_degrees: f32,
        speed: u8,
    ) -> Result<(), Error>;

    /// Move to absolute pan/tilt position in degrees.
    #[cfg(feature = "async")]
    async fn pan_tilt_absolute(
        &self,
        pan_degrees: f32,
        tilt_degrees: f32,
        speed: u8,
    ) -> Result<(), Error>;

    /// Move relative to current position in degrees.
    #[cfg(not(feature = "async"))]
    fn pan_tilt_relative(
        &mut self,
        pan_degrees: f32,
        tilt_degrees: f32,
        speed: u8,
    ) -> Result<(), Error>;

    /// Move relative to current position in degrees.
    #[cfg(feature = "async")]
    async fn pan_tilt_relative(
        &self,
        pan_degrees: f32,
        tilt_degrees: f32,
        speed: u8,
    ) -> Result<(), Error>;

    /// Move pan/tilt in a specific direction.
    #[cfg(not(feature = "async"))]
    fn pan_tilt_move(
        &mut self,
        direction: crate::command::pan_tilt::PanTiltDirection,
        pan_speed: u8,
        tilt_speed: u8,
    ) -> Result<(), Error>;

    /// Move pan/tilt in a specific direction.
    #[cfg(feature = "async")]
    async fn pan_tilt_move(
        &self,
        direction: crate::command::pan_tilt::PanTiltDirection,
        pan_speed: u8,
        tilt_speed: u8,
    ) -> Result<(), Error>;

    /// Reset pan/tilt to default position.
    #[cfg(not(feature = "async"))]
    fn pan_tilt_reset(&mut self) -> Result<(), Error>;

    /// Reset pan/tilt to default position.
    #[cfg(feature = "async")]
    async fn pan_tilt_reset(&self) -> Result<(), Error>;

    /// Set pan/tilt limit for a specific corner.
    #[cfg(not(feature = "async"))]
    fn set_pan_tilt_limit(
        &mut self,
        corner: crate::command::pan_tilt::LimitCorner,
        pan_position: crate::types::PanPosition,
        tilt_position: crate::types::TiltPosition,
    ) -> Result<(), Error>;

    /// Set pan/tilt limit for a specific corner.
    #[cfg(feature = "async")]
    async fn set_pan_tilt_limit(
        &self,
        corner: crate::command::pan_tilt::LimitCorner,
        pan_position: crate::types::PanPosition,
        tilt_position: crate::types::TiltPosition,
    ) -> Result<(), Error>;

    /// Clear pan/tilt limit for a specific corner.
    #[cfg(not(feature = "async"))]
    fn clear_pan_tilt_limit(
        &mut self,
        corner: crate::command::pan_tilt::LimitCorner,
    ) -> Result<(), Error>;

    /// Clear pan/tilt limit for a specific corner.
    #[cfg(feature = "async")]
    async fn clear_pan_tilt_limit(
        &self,
        corner: crate::command::pan_tilt::LimitCorner,
    ) -> Result<(), Error>;
}

// Blocking implementation for cameras with pan/tilt
#[cfg(not(feature = "async"))]
impl<P, T> PanTiltMethodsExt for Camera<P, T>
where
    P: ProfileMetadata + PanTilt + Default,
    T: crate::transport::blocking::BlockingTransport,
{
    fn pan_tilt_stop(&mut self) -> Result<(), Error> {
        self.send_const(pan_tilt::STOP)
    }

    fn pan_tilt_home(&mut self) -> Result<(), Error> {
        self.send_const(pan_tilt::HOME)
    }

    fn pan_tilt_absolute(
        &mut self,
        pan_degrees: f32,
        tilt_degrees: f32,
        speed: u8,
    ) -> Result<(), Error> {
        // Create a dummy profile instance to use extension trait methods
        let profile = P::default();

        // Use the extension trait to convert degrees to units
        let pan = profile.degrees_to_pan_units(pan_degrees);
        let tilt = profile.degrees_to_tilt_units(tilt_degrees);

        // Validate using profile constants
        let pan = profile.validate_pan(pan)?;
        let tilt = profile.validate_tilt(tilt)?;
        let pan_speed = profile.validate_pan_speed(speed);
        let tilt_speed = profile.validate_tilt_speed(speed);

        // Use const encoding
        let cmd = encode_pan_tilt_absolute(pan, tilt, pan_speed, tilt_speed);
        self.send_array(cmd)
    }

    fn pan_tilt_relative(
        &mut self,
        pan_degrees: f32,
        tilt_degrees: f32,
        speed: u8,
    ) -> Result<(), Error> {
        let profile = P::default();
        let pan = profile.degrees_to_pan_units(pan_degrees);
        let tilt = profile.degrees_to_tilt_units(tilt_degrees);
        let pan_speed = profile.validate_pan_speed(speed);
        let tilt_speed = profile.validate_tilt_speed(speed);

        let cmd = encode_pan_tilt_relative(pan, tilt, pan_speed, tilt_speed);
        self.send_array(cmd)
    }

    fn pan_tilt_move(
        &mut self,
        direction: crate::command::pan_tilt::PanTiltDirection,
        pan_speed: u8,
        tilt_speed: u8,
    ) -> Result<(), Error> {
        let profile = P::default();
        let pan_speed = profile.validate_pan_speed(pan_speed);
        let tilt_speed = profile.validate_tilt_speed(tilt_speed);

        // Use the PanTiltCommand directly instead of const encoding
        use crate::command::pan_tilt::PanTiltCommand;
        use crate::types::{PanSpeed, TiltSpeed};
        use crate::command::Command;
        
        let cmd = PanTiltCommand::Move {
            direction,
            pan_speed: PanSpeed::new(pan_speed)?,
            tilt_speed: TiltSpeed::new(tilt_speed)?,
        };
        let response_bytes = self.transport.send_blocking(&cmd.to_bytes()?)?;
        crate::command::Response::parse(&response_bytes)?.into_result()
    }

    fn pan_tilt_reset(&mut self) -> Result<(), Error> {
        self.send_const(pan_tilt::RESET)
    }

    fn set_pan_tilt_limit(
        &mut self,
        corner: crate::command::pan_tilt::LimitCorner,
        pan_position: crate::types::PanPosition,
        tilt_position: crate::types::TiltPosition,
    ) -> Result<(), Error> {
        use crate::command::pan_tilt::PanTiltLimitCommand;
        use crate::command::Command;
        
        let cmd = PanTiltLimitCommand::Set {
            corner,
            pan: pan_position,
            tilt: tilt_position,
        };
        let response_bytes = self.transport.send_blocking(&cmd.to_bytes()?)?;
        crate::command::Response::parse(&response_bytes)?.into_result()
    }

    fn clear_pan_tilt_limit(
        &mut self,
        corner: crate::command::pan_tilt::LimitCorner,
    ) -> Result<(), Error> {
        use crate::command::pan_tilt::PanTiltLimitCommand;
        use crate::command::Command;
        
        let cmd = PanTiltLimitCommand::Clear { corner };
        let response_bytes = self.transport.send_blocking(&cmd.to_bytes()?)?;
        crate::command::Response::parse(&response_bytes)?.into_result()
    }
}

// Async implementation for cameras with pan/tilt
#[cfg(feature = "async")]
impl<P, T> PanTiltMethodsExt for Camera<P, T>
where
    P: ProfileMetadata + PanTilt + Default,
    T: crate::transport::AsyncTransport,
{
    async fn pan_tilt_stop(&self) -> Result<(), Error> {
        self.send_const(pan_tilt::STOP).await
    }

    async fn pan_tilt_home(&self) -> Result<(), Error> {
        self.send_const(pan_tilt::HOME).await
    }

    async fn pan_tilt_absolute(
        &self,
        pan_degrees: f32,
        tilt_degrees: f32,
        speed: u8,
    ) -> Result<(), Error> {
        // Create a dummy instance for validation
        struct Validator<P>(std::marker::PhantomData<P>);
        impl<P: PanTilt> Validator<P> {
            fn validate(
                &self,
                pan_degrees: f32,
                tilt_degrees: f32,
                speed: u8,
            ) -> Result<(i16, i16, u8, u8), Error> {
                let dummy = DummyCamera::<P>::default();
                let pan = dummy.degrees_to_pan_units(pan_degrees);
                let tilt = dummy.degrees_to_tilt_units(tilt_degrees);
                let pan = dummy.validate_pan(pan)?;
                let tilt = dummy.validate_tilt(tilt)?;
                let pan_speed = dummy.validate_pan_speed(speed);
                let tilt_speed = dummy.validate_tilt_speed(speed);
                Ok((pan, tilt, pan_speed, tilt_speed))
            }
        }

        struct DummyCamera<P>(std::marker::PhantomData<P>);

        impl<P> Default for DummyCamera<P> {
            fn default() -> Self {
                Self(std::marker::PhantomData)
            }
        }
        impl<P: PanTilt> PanTilt for DummyCamera<P> {
            const PAN_RANGE: std::ops::Range<i16> = P::PAN_RANGE;
            const TILT_RANGE: std::ops::Range<i16> = P::TILT_RANGE;
            const MAX_PAN_SPEED: u8 = P::MAX_PAN_SPEED;
            const MAX_TILT_SPEED: u8 = P::MAX_TILT_SPEED;
            const PAN_DEGREES_TO_UNITS: f32 = P::PAN_DEGREES_TO_UNITS;
            const TILT_DEGREES_TO_UNITS: f32 = P::TILT_DEGREES_TO_UNITS;
        }

        let validator = Validator::<P>(std::marker::PhantomData);
        let (pan, tilt, pan_speed, tilt_speed) =
            validator.validate(pan_degrees, tilt_degrees, speed)?;

        let cmd = encode_pan_tilt_absolute(pan, tilt, pan_speed, tilt_speed);
        self.send_array(cmd).await
    }

    async fn pan_tilt_relative(
        &self,
        pan_degrees: f32,
        tilt_degrees: f32,
        speed: u8,
    ) -> Result<(), Error> {
        // Similar validation approach
        struct Validator<P>(std::marker::PhantomData<P>);
        impl<P: PanTilt> Validator<P> {
            fn validate(
                &self,
                pan_degrees: f32,
                tilt_degrees: f32,
                speed: u8,
            ) -> Result<(i16, i16, u8, u8), Error> {
                let dummy = DummyCamera::<P>::default();
                let pan = dummy.degrees_to_pan_units(pan_degrees);
                let tilt = dummy.degrees_to_tilt_units(tilt_degrees);
                let pan_speed = dummy.validate_pan_speed(speed);
                let tilt_speed = dummy.validate_tilt_speed(speed);
                Ok((pan, tilt, pan_speed, tilt_speed))
            }
        }

        struct DummyCamera<P>(std::marker::PhantomData<P>);

        impl<P> Default for DummyCamera<P> {
            fn default() -> Self {
                Self(std::marker::PhantomData)
            }
        }
        impl<P: PanTilt> PanTilt for DummyCamera<P> {
            const PAN_RANGE: std::ops::Range<i16> = P::PAN_RANGE;
            const TILT_RANGE: std::ops::Range<i16> = P::TILT_RANGE;
            const MAX_PAN_SPEED: u8 = P::MAX_PAN_SPEED;
            const MAX_TILT_SPEED: u8 = P::MAX_TILT_SPEED;
            const PAN_DEGREES_TO_UNITS: f32 = P::PAN_DEGREES_TO_UNITS;
            const TILT_DEGREES_TO_UNITS: f32 = P::TILT_DEGREES_TO_UNITS;
        }

        let validator = Validator::<P>(std::marker::PhantomData);
        let (pan, tilt, pan_speed, tilt_speed) =
            validator.validate(pan_degrees, tilt_degrees, speed)?;

        let cmd = encode_pan_tilt_relative(pan, tilt, pan_speed, tilt_speed);
        self.send_array(cmd).await
    }

    async fn pan_tilt_move(
        &self,
        direction: crate::command::pan_tilt::PanTiltDirection,
        pan_speed: u8,
        tilt_speed: u8,
    ) -> Result<(), Error> {
        struct Validator<P>(std::marker::PhantomData<P>);
        impl<P: PanTilt> Validator<P> {
            fn validate(&self, pan_speed: u8, tilt_speed: u8) -> (u8, u8) {
                let dummy = DummyCamera::<P>::default();
                let pan_speed = dummy.validate_pan_speed(pan_speed);
                let tilt_speed = dummy.validate_tilt_speed(tilt_speed);
                (pan_speed, tilt_speed)
            }
        }

        struct DummyCamera<P>(std::marker::PhantomData<P>);
        impl<P> Default for DummyCamera<P> {
            fn default() -> Self {
                Self(std::marker::PhantomData)
            }
        }
        impl<P: PanTilt> PanTilt for DummyCamera<P> {
            const PAN_RANGE: std::ops::Range<i16> = P::PAN_RANGE;
            const TILT_RANGE: std::ops::Range<i16> = P::TILT_RANGE;
            const MAX_PAN_SPEED: u8 = P::MAX_PAN_SPEED;
            const MAX_TILT_SPEED: u8 = P::MAX_TILT_SPEED;
            const PAN_DEGREES_TO_UNITS: f32 = P::PAN_DEGREES_TO_UNITS;
            const TILT_DEGREES_TO_UNITS: f32 = P::TILT_DEGREES_TO_UNITS;
        }

        let validator = Validator::<P>(std::marker::PhantomData);
        let (pan_speed, tilt_speed) = validator.validate(pan_speed, tilt_speed);

        // Use the PanTiltCommand directly instead of const encoding
        use crate::command::pan_tilt::PanTiltCommand;
        use crate::types::{PanSpeed, TiltSpeed};
        use crate::command::Command;
        
        let cmd = PanTiltCommand::Move {
            direction,
            pan_speed: PanSpeed::new(pan_speed)?,
            tilt_speed: TiltSpeed::new(tilt_speed)?,
        };
        let response_bytes = self.transport.send_async(&cmd.to_bytes()?).await?;
        crate::command::Response::parse(&response_bytes)?.into_result()
    }

    async fn pan_tilt_reset(&self) -> Result<(), Error> {
        self.send_const(pan_tilt::RESET).await
    }

    async fn set_pan_tilt_limit(
        &self,
        corner: crate::command::pan_tilt::LimitCorner,
        pan_position: crate::types::PanPosition,
        tilt_position: crate::types::TiltPosition,
    ) -> Result<(), Error> {
        use crate::command::pan_tilt::PanTiltLimitCommand;
        use crate::command::Command;
        
        let cmd = PanTiltLimitCommand::Set {
            corner,
            pan: pan_position,
            tilt: tilt_position,
        };
        let response_bytes = self.transport.send_async(&cmd.to_bytes()?).await?;
        crate::command::Response::parse(&response_bytes)?.into_result()
    }

    async fn clear_pan_tilt_limit(
        &self,
        corner: crate::command::pan_tilt::LimitCorner,
    ) -> Result<(), Error> {
        use crate::command::pan_tilt::PanTiltLimitCommand;
        use crate::command::Command;
        
        let cmd = PanTiltLimitCommand::Clear { corner };
        let response_bytes = self.transport.send_async(&cmd.to_bytes()?).await?;
        crate::command::Response::parse(&response_bytes)?.into_result()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::profiles::PTZOpticsG2;

    #[test]
    fn test_pan_tilt_methods_exist() {
        #[derive(Debug)]
        struct MockTransport;

        #[cfg(not(feature = "async"))]
        impl crate::transport::blocking::BlockingTransport for MockTransport {
            fn send(&mut self, _data: &[u8]) -> Result<(), Error> {
                Ok(())
            }
            fn receive(&mut self, _timeout: std::time::Duration) -> Result<Vec<u8>, Error> {
                Ok(vec![0x90, 0x50, 0xFF])
            }
            fn is_connected(&self) -> bool {
                true
            }
            fn description(&self) -> &str {
                "MockTransport"
            }
        }

        let mut _camera: Camera<PTZOpticsG2, MockTransport> = Camera::new(MockTransport);

        // These methods exist because PTZOpticsG2 implements PanTilt
        #[cfg(not(feature = "async"))]
        {
            let _ = _camera.pan_tilt_stop();
            let _ = _camera.pan_tilt_home();
            let _ = _camera.pan_tilt_absolute(45.0, 30.0, 10);
        }
    }
}
