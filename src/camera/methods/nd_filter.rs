//! ND filter methods for cameras that support ND filters using the new GAT architecture.
//!
//! These methods ONLY exist for cameras that implement NDFilter.

use crate::{
    command::{NDFilterMode as CommandNDFilterMode, NDFilterStep},
    Error,
};

/// ND filter operations (async).
#[cfg(feature = "async")]
pub trait NdFilterControl: Sized {
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
pub trait NdFilterControlBlocking: Sized {
    /// Set ND filter mode (preset or variable).
    fn set_nd_filter_mode(&mut self, mode: CommandNDFilterMode) -> Result<(), Error>;

    /// Set ND filter value directly (for variable mode).
    fn set_nd_filter_value(&mut self, value: u16) -> Result<(), Error>;

    /// Set ND filter by stop value (2.0 to 7.0 stops).
    fn set_nd_filter_stops(&mut self, stops: f32) -> Result<(), Error>;

    /// Step ND filter up or down.
    fn step_nd_filter(&mut self, direction: NDFilterStep) -> Result<(), Error>;

    /// Enable or disable auto ND.
    fn set_auto_nd(&mut self, enabled: bool) -> Result<(), Error>;

    /// Get current ND filter setting.
    fn get_nd_filter(&mut self) -> Result<u8, Error>;
}

// Async implementation for Camera with AsyncMode
#[cfg(feature = "async")]
impl<P, T, E> NdFilterControl for crate::camera::Camera<crate::camera::AsyncMode, P, T, E>
where
    P: crate::capabilities::Profile + crate::capabilities::nd_filter::NDFilter,
    T: crate::transport::AsyncTransport + Send + Sync + 'static,
    E: crate::executor::Executor,
{
    async fn set_nd_filter_mode(&self, mode: CommandNDFilterMode) -> Result<(), Error> {
        use crate::command::nd_filter::NdFilterModeCmd;

        let cmd = NdFilterModeCmd::new(mode);
        self.send_command(&cmd).await?;
        Ok(())
    }

    async fn set_nd_filter_value(&self, value: u16) -> Result<(), Error> {
        use crate::command::nd_filter::NDFilterValue;

        let cmd = NDFilterValue::new(value).map_err(|_| Error::InvalidParameter {
            parameter: "value",
            value: value.to_string().into(),
            reason: "ND filter value out of range".into(),
        })?;
        self.send_command(&cmd).await?;
        Ok(())
    }

    async fn set_nd_filter_stops(&self, stops: f32) -> Result<(), Error> {
        use crate::command::nd_filter::NDFilterValue;

        let cmd = NDFilterValue::from_stops(stops).map_err(|_| Error::InvalidParameter {
            parameter: "stops",
            value: stops.to_string().into(),
            reason: "ND filter stops must be between 2.0 and 7.0".into(),
        })?;
        self.send_command(&cmd).await?;
        Ok(())
    }

    async fn step_nd_filter(&self, direction: NDFilterStep) -> Result<(), Error> {
        use crate::command::nd_filter::NdFilterStepCmd;

        let cmd = NdFilterStepCmd::new(direction);
        self.send_command(&cmd).await?;
        Ok(())
    }

    async fn set_auto_nd(&self, enabled: bool) -> Result<(), Error> {
        use crate::command::nd_filter::AutoNDCommand;

        let cmd = AutoNDCommand::new(enabled);
        self.send_command(&cmd).await?;
        Ok(())
    }

    async fn get_nd_filter(&self) -> Result<u8, Error> {
        use crate::command::{inquiry::NdFilterInquiry, response::ViscaResponse, InquiryResponse};

        let inquiry = NdFilterInquiry;
        let response = self.send_command(&inquiry).await?;
        match response {
            ViscaResponse::Inquiry(InquiryResponse::NdFilter { position }) => Ok(position),
            _ => Err(Error::UnexpectedResponseType),
        }
    }
}

// Blocking implementation for Camera with BlockingMode
impl<P, T> NdFilterControlBlocking for crate::camera::Camera<crate::camera::BlockingMode, P, T, ()>
where
    P: crate::capabilities::Profile + crate::capabilities::nd_filter::NDFilter,
    T: crate::transport::BlockingTransport + Send + 'static,
{
    fn set_nd_filter_mode(&mut self, mode: CommandNDFilterMode) -> Result<(), Error> {
        use crate::command::nd_filter::NdFilterModeCmd;

        let cmd = NdFilterModeCmd::new(mode);
        self.send_command(&cmd)?;
        Ok(())
    }

    fn set_nd_filter_value(&mut self, value: u16) -> Result<(), Error> {
        use crate::command::nd_filter::NDFilterValue;

        let cmd = NDFilterValue::new(value).map_err(|_| Error::InvalidParameter {
            parameter: "value",
            value: value.to_string().into(),
            reason: "ND filter value out of range".into(),
        })?;
        self.send_command(&cmd)?;
        Ok(())
    }

    fn set_nd_filter_stops(&mut self, stops: f32) -> Result<(), Error> {
        use crate::command::nd_filter::NDFilterValue;

        let cmd = NDFilterValue::from_stops(stops).map_err(|_| Error::InvalidParameter {
            parameter: "stops",
            value: stops.to_string().into(),
            reason: "ND filter stops must be between 2.0 and 7.0".into(),
        })?;
        self.send_command(&cmd)?;
        Ok(())
    }

    fn step_nd_filter(&mut self, direction: NDFilterStep) -> Result<(), Error> {
        use crate::command::nd_filter::NdFilterStepCmd;

        let cmd = NdFilterStepCmd::new(direction);
        self.send_command(&cmd)?;
        Ok(())
    }

    fn set_auto_nd(&mut self, enabled: bool) -> Result<(), Error> {
        use crate::command::nd_filter::AutoNDCommand;

        let cmd = AutoNDCommand::new(enabled);
        self.send_command(&cmd)?;
        Ok(())
    }

    fn get_nd_filter(&mut self) -> Result<u8, Error> {
        use crate::command::{inquiry::NdFilterInquiry, response::ViscaResponse, InquiryResponse};

        let inquiry = NdFilterInquiry;
        let response = self.send_command(&inquiry)?;
        match response {
            ViscaResponse::Inquiry(InquiryResponse::NdFilter { position }) => Ok(position),
            _ => Err(Error::UnexpectedResponseType),
        }
    }
}
