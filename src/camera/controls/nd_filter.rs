//! ND filter methods for cameras that support ND filters.
//!
//! These methods ONLY exist for cameras that implement NdFilter.

use crate::{
    command::{NdFilterMode as CommandNdFilterMode, NdFilterStep},
    Error,
};

/// ND filter operations for cameras.
///
/// This trait provides ND filter control methods that work for both blocking and async cameras.
/// The implementation differs based on the camera type - async cameras return futures,
/// while blocking cameras perform operations synchronously.
pub trait NdFilterControl {
    /// Set ND filter mode (preset or variable).
    #[cfg(feature = "async")]
    fn set_nd_filter_mode(
        &self,
        mode: CommandNdFilterMode,
    ) -> impl std::future::Future<Output = Result<(), Error>> + Send + '_;

    /// Set ND filter mode (preset or variable).
    #[cfg(not(feature = "async"))]
    fn set_nd_filter_mode(&mut self, mode: CommandNdFilterMode) -> Result<(), Error>;

    /// Set ND filter value directly (for variable mode).
    #[cfg(feature = "async")]
    fn set_nd_filter_value(
        &self,
        value: u16,
    ) -> impl std::future::Future<Output = Result<(), Error>> + Send + '_;

    /// Set ND filter value directly (for variable mode).
    #[cfg(not(feature = "async"))]
    fn set_nd_filter_value(&mut self, value: u16) -> Result<(), Error>;

    /// Set ND filter by stop value (2.0 to 7.0 stops).
    #[cfg(feature = "async")]
    fn set_nd_filter_stops(
        &self,
        stops: f32,
    ) -> impl std::future::Future<Output = Result<(), Error>> + Send + '_;

    /// Set ND filter by stop value (2.0 to 7.0 stops).
    #[cfg(not(feature = "async"))]
    fn set_nd_filter_stops(&mut self, stops: f32) -> Result<(), Error>;

    /// Step ND filter up or down.
    #[cfg(feature = "async")]
    fn step_nd_filter(
        &self,
        direction: NdFilterStep,
    ) -> impl std::future::Future<Output = Result<(), Error>> + Send + '_;

    /// Step ND filter up or down.
    #[cfg(not(feature = "async"))]
    fn step_nd_filter(&mut self, direction: NdFilterStep) -> Result<(), Error>;

    /// Enable or disable auto ND.
    #[cfg(feature = "async")]
    fn set_auto_nd(
        &self,
        enabled: bool,
    ) -> impl std::future::Future<Output = Result<(), Error>> + Send + '_;

    /// Enable or disable auto ND.
    #[cfg(not(feature = "async"))]
    fn set_auto_nd(&mut self, enabled: bool) -> Result<(), Error>;

    /// Get current ND filter setting.
    #[cfg(feature = "async")]
    fn get_nd_filter(&self) -> impl std::future::Future<Output = Result<u8, Error>> + Send + '_;

    /// Get current ND filter setting.
    #[cfg(not(feature = "async"))]
    fn get_nd_filter(&mut self) -> Result<u8, Error>;
}

// Keep the old trait names for backward compatibility during transition
/// Async ND filter control trait (deprecated, use NdFilterControl instead).
/// Blocking ND filter control trait (deprecated, use NdFilterControl instead).
// Async implementation for Camera with AsyncMode
#[cfg(feature = "async")]
impl<P, Tr, Exec> NdFilterControl for crate::camera::Camera<crate::mode::Async, P, Tr, Exec>
where
    P: crate::capabilities::Profile + crate::capabilities::nd_filter::NdFilter + Default,
    Tr: crate::transport::AsyncTransport + Send + Sync + 'static,
    Exec: crate::executor::Executor,
{
    async fn set_nd_filter_mode(&self, mode: CommandNdFilterMode) -> Result<(), Error> {
        use crate::command::nd_filter::NdFilterModeCommand;
        let cmd = NdFilterModeCommand::new(mode);
        self.send_command(&cmd).await?;
        Ok(())
    }

    async fn set_nd_filter_value(&self, value: u16) -> Result<(), Error> {
        use crate::command::nd_filter::NdFilterValue;

        let cmd = NdFilterValue::new(value).map_err(|_| Error::InvalidParameter {
            parameter: "value",
            value: value.to_string().into(),
            reason: "ND filter value out of range".into(),
        })?;
        self.send_command(&cmd).await?;
        Ok(())
    }

    async fn set_nd_filter_stops(&self, stops: f32) -> Result<(), Error> {
        use crate::command::nd_filter::NdFilterValue;

        let cmd = NdFilterValue::from_stops(stops).map_err(|_| Error::InvalidParameter {
            parameter: "stops",
            value: stops.to_string().into(),
            reason: "ND filter stops must be between 2.0 and 7.0".into(),
        })?;
        self.send_command(&cmd).await?;
        Ok(())
    }

    async fn step_nd_filter(&self, direction: NdFilterStep) -> Result<(), Error> {
        use crate::command::nd_filter::NdFilterStepCommand;

        let cmd = NdFilterStepCommand::new(direction);
        self.send_command(&cmd).await?;
        Ok(())
    }

    async fn set_auto_nd(&self, enabled: bool) -> Result<(), Error> {
        use crate::command::nd_filter::AutoNdCommand;

        let cmd = AutoNdCommand::new(enabled);
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

// Blocking implementation for BlockingCamera
#[cfg(not(feature = "async"))]
impl<P, Tr> NdFilterControl for crate::camera::Camera<crate::mode::Blocking, P, Tr, ()>
where
    P: crate::capabilities::Profile + crate::capabilities::nd_filter::NdFilter + Default,
    Tr: crate::transport::SyncTransport + Send + 'static,
{
    fn set_nd_filter_mode(&mut self, mode: CommandNdFilterMode) -> Result<(), Error> {
        use crate::command::nd_filter::NdFilterModeCommand;
        let cmd = NdFilterModeCommand::new(mode);
        self.send_command(&cmd)?;
        Ok(())
    }

    fn set_nd_filter_value(&mut self, value: u16) -> Result<(), Error> {
        use crate::command::nd_filter::NdFilterValue;

        let cmd = NdFilterValue::new(value).map_err(|_| Error::InvalidParameter {
            parameter: "value",
            value: value.to_string().into(),
            reason: "ND filter value out of range".into(),
        })?;
        self.send_command(&cmd)?;
        Ok(())
    }

    fn set_nd_filter_stops(&mut self, stops: f32) -> Result<(), Error> {
        use crate::command::nd_filter::NdFilterValue;

        let cmd = NdFilterValue::from_stops(stops).map_err(|_| Error::InvalidParameter {
            parameter: "stops",
            value: stops.to_string().into(),
            reason: "ND filter stops must be between 2.0 and 7.0".into(),
        })?;
        self.send_command(&cmd)?;
        Ok(())
    }

    fn step_nd_filter(&mut self, direction: NdFilterStep) -> Result<(), Error> {
        use crate::command::nd_filter::NdFilterStepCommand;

        let cmd = NdFilterStepCommand::new(direction);
        self.send_command(&cmd)?;
        Ok(())
    }

    fn set_auto_nd(&mut self, enabled: bool) -> Result<(), Error> {
        use crate::command::nd_filter::AutoNdCommand;

        let cmd = AutoNdCommand::new(enabled);
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
