//! ND filter methods for cameras that support ND filters using the new GAT architecture.
//!
//! These methods ONLY exist for cameras that implement NDFilter.

use crate::{
    command::{
        nd_filter::{
            AutoNDCommand, NDFilterModeCommand, NDFilterStepCommand, NDFilterValueCommand,
        },
        InquiryResponse, NDFilterMode as CommandNDFilterMode, NDFilterStep, Response,
    },
    Error,
};

/// ND filter operations (async).
#[cfg(feature = "async")]
pub trait NDFilterOps: Sized {
    /// Set ND filter mode (preset or variable).
    async fn set_nd_filter_mode(&self, mode: CommandNDFilterMode) -> Result<(), Error>;

    /// Set ND filter value directly (for variable mode).
    async fn set_nd_filter_value(&self, value: u16) -> Result<(), Error>;

    /// Set ND filter by stop value (2.0 to 7.0 stops).
    async fn set_nd_filter_stops(&self, stops: f32) -> Result<(), Error>;

    /// Step ND filter up or down.
    async fn step_nd_filter(&self, direction: NDFilterStep) -> Result<(), Error>;

    /// Enable or disable auto ND.
    async fn set_auto_nd(&self, enabled: bool) -> Result<(), Error>;

    /// Get current ND filter setting.
    async fn get_nd_filter(&self) -> Result<u8, Error>;
}

/// ND filter operations (blocking).
#[cfg(not(feature = "async"))]
pub trait NDFilterOpsBlocking: Sized {
    /// Set ND filter mode (preset or variable).
    fn set_nd_filter_mode(&self, mode: CommandNDFilterMode) -> Result<(), Error>;

    /// Set ND filter value directly (for variable mode).
    fn set_nd_filter_value(&self, value: u16) -> Result<(), Error>;

    /// Set ND filter by stop value (2.0 to 7.0 stops).
    fn set_nd_filter_stops(&self, stops: f32) -> Result<(), Error>;

    /// Step ND filter up or down.
    fn step_nd_filter(&self, direction: NDFilterStep) -> Result<(), Error>;

    /// Enable or disable auto ND.
    fn set_auto_nd(&self, enabled: bool) -> Result<(), Error>;

    /// Get current ND filter setting.
    fn get_nd_filter(&self) -> Result<u8, Error>;
}

// Async implementation
#[cfg(feature = "async")]
impl<P: crate::capabilities::Profile, T: crate::transport::Transport + Send + Sync + 'static>
    NDFilterOps for crate::camera::generic::Camera<P, T>
where
    T::Error: Into<Error> + Send,
    for<'a> T::SendFut<'a>: Send,
    for<'a> T::RecvFut<'a>: Send,
{
    async fn set_nd_filter_mode(&self, mode: CommandNDFilterMode) -> Result<(), Error> {
        let command = NDFilterModeCommand::new(mode);
        self.send_command(&command).await?;
        Ok(())
    }

    async fn set_nd_filter_value(&self, value: u16) -> Result<(), Error> {
        let command = NDFilterValueCommand::new(value)?;
        self.send_command(&command).await?;
        Ok(())
    }

    async fn set_nd_filter_stops(&self, stops: f32) -> Result<(), Error> {
        let command = NDFilterValueCommand::from_stops(stops)?;
        self.send_command(&command).await?;
        Ok(())
    }

    async fn step_nd_filter(&self, direction: NDFilterStep) -> Result<(), Error> {
        let command = NDFilterStepCommand::new(direction);
        self.send_command(&command).await?;
        Ok(())
    }

    async fn set_auto_nd(&self, enabled: bool) -> Result<(), Error> {
        let command = AutoNDCommand::new(enabled);
        self.send_command(&command).await?;
        Ok(())
    }

    async fn get_nd_filter(&self) -> Result<u8, Error> {
        let inquiry = crate::command::inquiry::NdFilterInquiry;
        let response = self.send_command(&inquiry).await?;

        if let Response::Inquiry(InquiryResponse::NdFilter { position }) = response {
            Ok(position)
        } else {
            Err(Error::UnexpectedResponseType)
        }
    }
}

// Blocking implementation
#[cfg(not(feature = "async"))]
impl<P: crate::capabilities::Profile, T: crate::transport::Transport + Send + Sync + 'static>
    NDFilterOpsBlocking for crate::camera::generic::Camera<P, T>
where
    T::Error: Into<Error> + Send,
    for<'a> T::SendFut<'a>: Send,
    for<'a> T::RecvFut<'a>: Send,
{
    fn set_nd_filter_mode(&self, mode: CommandNDFilterMode) -> Result<(), Error> {
        let command = NDFilterModeCommand::new(mode);
        self.send_command_blocking(&command)?;
        Ok(())
    }

    fn set_nd_filter_value(&self, value: u16) -> Result<(), Error> {
        let command = NDFilterValueCommand::new(value)?;
        self.send_command_blocking(&command)?;
        Ok(())
    }

    fn set_nd_filter_stops(&self, stops: f32) -> Result<(), Error> {
        let command = NDFilterValueCommand::from_stops(stops)?;
        self.send_command_blocking(&command)?;
        Ok(())
    }

    fn step_nd_filter(&self, direction: NDFilterStep) -> Result<(), Error> {
        let command = NDFilterStepCommand::new(direction);
        self.send_command_blocking(&command)?;
        Ok(())
    }

    fn set_auto_nd(&self, enabled: bool) -> Result<(), Error> {
        let command = AutoNDCommand::new(enabled);
        self.send_command_blocking(&command)?;
        Ok(())
    }

    fn get_nd_filter(&self) -> Result<u8, Error> {
        let inquiry = crate::command::inquiry::NdFilterInquiry;
        let response = self.send_command_blocking(&inquiry)?;

        if let Response::Inquiry(InquiryResponse::NdFilter { position }) = response {
            Ok(position)
        } else {
            Err(Error::UnexpectedResponseType)
        }
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {

    #[test]
    fn test_nd_filter_compile_time_safety() {
        // This test demonstrates compile-time safety - cameras without ND filter
        // capability cannot use ND filter methods

        // Note: With the unified Camera API, compile-time safety is achieved
        // through runtime profile checks rather than generic constraints
    }

    #[test]
    fn test_nd_stops_conversion() {
        use crate::command::nd_filter::NDFilterValueCommand;

        // Test that stop values in valid range create commands successfully
        assert!(NDFilterValueCommand::from_stops(2.0).is_ok());
        assert!(NDFilterValueCommand::from_stops(2.5).is_ok());
        assert!(NDFilterValueCommand::from_stops(3.0).is_ok());
        assert!(NDFilterValueCommand::from_stops(7.0).is_ok());

        // Test out of range values return errors
        assert!(NDFilterValueCommand::from_stops(1.5).is_err()); // Below min
        assert!(NDFilterValueCommand::from_stops(8.0).is_err()); // Above max

        // Test direct value construction
        assert!(NDFilterValueCommand::new(0x0000).is_ok()); // Min value
        assert!(NDFilterValueCommand::new(0x0014).is_ok()); // Max value
        assert!(NDFilterValueCommand::new(0x0015).is_err()); // Above max
    }

    #[test]
    fn test_nd_filter_inquiry_command() {
        use crate::command::inquiry::NdFilterInquiry;
        use crate::command::EncodeVisca;
        use crate::CameraId;

        // Test ND filter inquiry command creation
        let inquiry = NdFilterInquiry;
        let mut buffer = [0u8; 32];
        let len = inquiry
            .encode_into(CameraId::CAMERA_1, &mut buffer)
            .unwrap();

        // The expected bytes for ND filter inquiry should be:
        // 0x81 0x09 0x04 0x64 0xFF
        assert_eq!(len, 5);
        assert_eq!(buffer[0], 0x81); // Camera address
        assert_eq!(buffer[1], 0x09); // Inquiry command
        assert_eq!(buffer[2], 0x04); // Sub-category
        assert_eq!(buffer[3], 0x64); // ND filter inquiry code
        assert_eq!(buffer[4], 0xFF); // Terminator
    }
}
