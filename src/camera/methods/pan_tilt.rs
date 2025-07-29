//! Pan/Tilt methods for cameras using the new GAT architecture.

use crate::{
    camera::Camera,
    command::pan_tilt::{PanTilt, PanTiltDirection, PanTiltLimitCorner},
    types::{PanPosition, PanSpeed, SpeedLevel, TiltPosition, TiltSpeed},
    units::Degrees,
    Error,
};

/// Pan/Tilt operations (async).
pub trait PanTiltOps: Sized {
    /// Stop all pan/tilt movement.
    async fn pan_tilt_stop(&self) -> Result<(), Error>;

    /// Move to home position (0, 0).
    async fn pan_tilt_home(&self) -> Result<(), Error>;

    /// Move to absolute pan/tilt position in degrees.
    async fn pan_tilt_absolute(
        &self,
        pan: Degrees,
        tilt: Degrees,
        speed: SpeedLevel,
    ) -> Result<(), Error>;

    /// Move relative to current position in degrees.
    async fn pan_tilt_relative(
        &self,
        pan: Degrees,
        tilt: Degrees,
        speed: SpeedLevel,
    ) -> Result<(), Error>;

    /// Move pan/tilt in a specific direction.
    async fn pan_tilt_move(
        &self,
        direction: PanTiltDirection,
        pan_speed: PanSpeed,
        tilt_speed: TiltSpeed,
    ) -> Result<(), Error>;

    /// Reset pan/tilt to default position.
    async fn pan_tilt_reset(&self) -> Result<(), Error>;

    /// Set pan/tilt movement limit for a specific corner.
    async fn pan_tilt_limit_set(
        &self,
        corner: PanTiltLimitCorner,
        pan: PanPosition,
        tilt: TiltPosition,
    ) -> Result<(), Error>;

    /// Clear pan/tilt movement limit for a specific corner.
    async fn pan_tilt_limit_clear(&self, corner: PanTiltLimitCorner) -> Result<(), Error>;
}

/// Pan/Tilt operations (blocking).
pub trait PanTiltOpsBlocking: Sized {
    /// Stop all pan/tilt movement.
    fn pan_tilt_stop(&self) -> Result<(), Error>;

    /// Move to home position (0, 0).
    fn pan_tilt_home(&self) -> Result<(), Error>;

    /// Move to absolute pan/tilt position in degrees.
    fn pan_tilt_absolute(
        &self,
        pan: Degrees,
        tilt: Degrees,
        speed: SpeedLevel,
    ) -> Result<(), Error>;

    /// Move relative to current position in degrees.
    fn pan_tilt_relative(
        &self,
        pan: Degrees,
        tilt: Degrees,
        speed: SpeedLevel,
    ) -> Result<(), Error>;

    /// Move pan/tilt in a specific direction.
    fn pan_tilt_move(
        &self,
        direction: PanTiltDirection,
        pan_speed: PanSpeed,
        tilt_speed: TiltSpeed,
    ) -> Result<(), Error>;

    /// Reset pan/tilt to default position.
    fn pan_tilt_reset(&self) -> Result<(), Error>;

    /// Set pan/tilt movement limit for a specific corner.
    fn pan_tilt_limit_set(
        &self,
        corner: PanTiltLimitCorner,
        pan: PanPosition,
        tilt: TiltPosition,
    ) -> Result<(), Error>;

    /// Clear pan/tilt movement limit for a specific corner.
    fn pan_tilt_limit_clear(&self, corner: PanTiltLimitCorner) -> Result<(), Error>;
}

// Async implementation
impl PanTiltOps for Camera {
    async fn pan_tilt_stop(&self) -> Result<(), Error> {
        let cmd = PanTilt::Move {
            direction: PanTiltDirection::Stop,
            pan_speed: PanSpeed::ZERO,
            tilt_speed: TiltSpeed::ZERO,
        };
        self.send_command(&cmd).await?;
        Ok(())
    }

    async fn pan_tilt_home(&self) -> Result<(), Error> {
        let cmd = PanTilt::Home;
        self.send_command(&cmd).await?;
        Ok(())
    }

    async fn pan_tilt_absolute(
        &self,
        pan: Degrees,
        tilt: Degrees,
        speed: SpeedLevel,
    ) -> Result<(), Error> {
        // Convert degrees to units using Camera's methods
        let (pan_units, tilt_units) = self.degrees_to_units(pan, tilt);

        // Validate using camera's profile
        let pan_validated = self.validate_pan(pan_units.0)?;
        let tilt_validated = self.validate_tilt(tilt_units.0)?;

        // Convert SpeedLevel to specific pan/tilt speeds
        let pan_speed_val = speed.to_pan_speed();
        let tilt_speed_val = speed.to_tilt_speed();

        // Convert logical coordinates to camera coordinates
        let (cam_pan, cam_tilt) = self.to_camera_coords(pan_validated, tilt_validated);

        // Create position objects using camera coordinates
        let pan_pos = PanPosition::new(cam_pan as i16)?;
        let tilt_pos = TiltPosition::new(cam_tilt as i16)?;
        let pan_spd = PanSpeed::new(pan_speed_val)?;
        let tilt_spd = TiltSpeed::new(tilt_speed_val)?;

        let cmd = PanTilt::AbsolutePosition {
            pan: pan_pos,
            tilt: tilt_pos,
            pan_speed: pan_spd,
            tilt_speed: tilt_spd,
        };
        self.send_command(&cmd).await?;
        Ok(())
    }

    async fn pan_tilt_relative(
        &self,
        pan: Degrees,
        tilt: Degrees,
        speed: SpeedLevel,
    ) -> Result<(), Error> {
        // Convert degrees to units using Camera's methods
        let (pan_units, tilt_units) = self.degrees_to_units(pan, tilt);

        // Convert SpeedLevel to specific pan/tilt speeds
        let pan_speed_val = speed.to_pan_speed();
        let tilt_speed_val = speed.to_tilt_speed();

        let pan_pos = PanPosition::new(pan_units.0)?;
        let tilt_pos = TiltPosition::new(tilt_units.0)?;
        let pan_spd = PanSpeed::new(pan_speed_val)?;
        let tilt_spd = TiltSpeed::new(tilt_speed_val)?;

        let cmd = PanTilt::RelativePosition {
            pan: pan_pos,
            tilt: tilt_pos,
            pan_speed: pan_spd,
            tilt_speed: tilt_spd,
        };
        self.send_command(&cmd).await?;
        Ok(())
    }

    async fn pan_tilt_move(
        &self,
        direction: PanTiltDirection,
        pan_speed: PanSpeed,
        tilt_speed: TiltSpeed,
    ) -> Result<(), Error> {
        let cmd = PanTilt::Move {
            direction,
            pan_speed,
            tilt_speed,
        };
        self.send_command(&cmd).await?;
        Ok(())
    }

    async fn pan_tilt_reset(&self) -> Result<(), Error> {
        let cmd = PanTilt::Reset;
        self.send_command(&cmd).await?;
        Ok(())
    }

    async fn pan_tilt_limit_set(
        &self,
        corner: PanTiltLimitCorner,
        pan: PanPosition,
        tilt: TiltPosition,
    ) -> Result<(), Error> {
        let cmd = PanTilt::LimitSet { corner, pan, tilt };
        self.send_command(&cmd).await?;
        Ok(())
    }

    async fn pan_tilt_limit_clear(&self, corner: PanTiltLimitCorner) -> Result<(), Error> {
        let cmd = PanTilt::LimitClear { corner };
        self.send_command(&cmd).await?;
        Ok(())
    }
}

// Blocking implementation
impl PanTiltOpsBlocking for Camera {
    fn pan_tilt_stop(&self) -> Result<(), Error> {
        let cmd = PanTilt::Move {
            direction: PanTiltDirection::Stop,
            pan_speed: PanSpeed::ZERO,
            tilt_speed: TiltSpeed::ZERO,
        };
        self.send_command_blocking(&cmd)?;
        Ok(())
    }

    fn pan_tilt_home(&self) -> Result<(), Error> {
        let cmd = PanTilt::Home;
        self.send_command_blocking(&cmd)?;
        Ok(())
    }

    fn pan_tilt_absolute(
        &self,
        pan: Degrees,
        tilt: Degrees,
        speed: SpeedLevel,
    ) -> Result<(), Error> {
        // Convert degrees to units using Camera's methods
        let (pan_units, tilt_units) = self.degrees_to_units(pan, tilt);

        // Validate using camera's profile
        let pan_validated = self.validate_pan(pan_units.0)?;
        let tilt_validated = self.validate_tilt(tilt_units.0)?;

        // Convert SpeedLevel to specific pan/tilt speeds
        let pan_speed_val = speed.to_pan_speed();
        let tilt_speed_val = speed.to_tilt_speed();

        // Convert logical coordinates to camera coordinates
        let (cam_pan, cam_tilt) = self.to_camera_coords(pan_validated, tilt_validated);

        // Create position objects using camera coordinates
        let pan_pos = PanPosition::new(cam_pan as i16)?;
        let tilt_pos = TiltPosition::new(cam_tilt as i16)?;
        let pan_spd = PanSpeed::new(pan_speed_val)?;
        let tilt_spd = TiltSpeed::new(tilt_speed_val)?;

        let cmd = PanTilt::AbsolutePosition {
            pan: pan_pos,
            tilt: tilt_pos,
            pan_speed: pan_spd,
            tilt_speed: tilt_spd,
        };
        self.send_command_blocking(&cmd)?;
        Ok(())
    }

    fn pan_tilt_relative(
        &self,
        pan: Degrees,
        tilt: Degrees,
        speed: SpeedLevel,
    ) -> Result<(), Error> {
        // Convert degrees to units using Camera's methods
        let (pan_units, tilt_units) = self.degrees_to_units(pan, tilt);

        // Convert SpeedLevel to specific pan/tilt speeds
        let pan_speed_val = speed.to_pan_speed();
        let tilt_speed_val = speed.to_tilt_speed();

        let pan_pos = PanPosition::new(pan_units.0)?;
        let tilt_pos = TiltPosition::new(tilt_units.0)?;
        let pan_spd = PanSpeed::new(pan_speed_val)?;
        let tilt_spd = TiltSpeed::new(tilt_speed_val)?;

        let cmd = PanTilt::RelativePosition {
            pan: pan_pos,
            tilt: tilt_pos,
            pan_speed: pan_spd,
            tilt_speed: tilt_spd,
        };
        self.send_command_blocking(&cmd)?;
        Ok(())
    }

    fn pan_tilt_move(
        &self,
        direction: PanTiltDirection,
        pan_speed: PanSpeed,
        tilt_speed: TiltSpeed,
    ) -> Result<(), Error> {
        let cmd = PanTilt::Move {
            direction,
            pan_speed,
            tilt_speed,
        };
        self.send_command_blocking(&cmd)?;
        Ok(())
    }

    fn pan_tilt_reset(&self) -> Result<(), Error> {
        let cmd = PanTilt::Reset;
        self.send_command_blocking(&cmd)?;
        Ok(())
    }

    fn pan_tilt_limit_set(
        &self,
        corner: PanTiltLimitCorner,
        pan: PanPosition,
        tilt: TiltPosition,
    ) -> Result<(), Error> {
        let cmd = PanTilt::LimitSet { corner, pan, tilt };
        self.send_command_blocking(&cmd)?;
        Ok(())
    }

    fn pan_tilt_limit_clear(&self, corner: PanTiltLimitCorner) -> Result<(), Error> {
        let cmd = PanTilt::LimitClear { corner };
        self.send_command_blocking(&cmd)?;
        Ok(())
    }
}
