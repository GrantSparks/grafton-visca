//! ND filter control implementation using Mode trait.

use crate::{
    camera::CommandClient,
    command::{
        inquiry_structs::NdFilterInquiry,
        nd_filter::{AutoNdCommand, NdFilterModeCommand, NdFilterStepCommand, NdFilterValue},
        NdFilterMode as CommandNdFilterMode, NdFilterStep,
    },
    mode::Mode,
    Error,
};

/// ND filter operations for cameras.
///
/// This trait provides ND filter control methods that work seamlessly for both
/// blocking and async cameras through the Mode trait system.
#[grafton_visca_macros::delegate_to_session]
pub trait NdFilterControl {
    /// The mode type for this camera (Async or Blocking).
    type Mode: Mode;

    /// Set ND filter mode (preset or variable).
    fn set_nd_filter_mode(
        &self,
        mode: CommandNdFilterMode,
    ) -> <Self::Mode as Mode>::Ret<'_, Result<(), Error>>;

    /// Set ND filter value directly (for variable mode).
    fn set_nd_filter_value(&self, value: u16) -> <Self::Mode as Mode>::Ret<'_, Result<(), Error>>;

    /// Set ND filter by stop value (2.0 to 7.0 stops).
    fn set_nd_filter_stops(&self, stops: f32) -> <Self::Mode as Mode>::Ret<'_, Result<(), Error>>;

    /// Step ND filter up or down.
    fn step_nd_filter(
        &self,
        direction: NdFilterStep,
    ) -> <Self::Mode as Mode>::Ret<'_, Result<(), Error>>;

    /// Enable or disable auto ND.
    fn set_auto_nd(&self, enabled: bool) -> <Self::Mode as Mode>::Ret<'_, Result<(), Error>>;

    /// Get current ND filter setting.
    fn get_nd_filter(&self) -> <Self::Mode as Mode>::Ret<'_, Result<u8, Error>>;
}

// Single unified implementation for all Camera types!
impl<M, P, Tr, Exec> NdFilterControl for crate::camera::Camera<M, P, Tr, Exec>
where
    M: Mode,
    P: crate::capabilities::Profile + Default + crate::capabilities::nd_filter::NdFilter,
    Self: CommandClient<M>,
    Exec: crate::executor::Executor,
{
    type Mode = M;

    fn set_nd_filter_mode(&self, mode: CommandNdFilterMode) -> M::Ret<'_, Result<(), Error>> {
        let cmd = NdFilterModeCommand::new(mode);
        self.send_and_complete(cmd)
    }

    fn set_nd_filter_value(&self, value: u16) -> M::Ret<'_, Result<(), Error>> {
        match NdFilterValue::new(value) {
            Ok(cmd) => self.send_and_complete(cmd),
            Err(_) => self.error(Error::InvalidParameter {
                parameter: "value",
                value: value.to_string().into(),
                reason: "ND filter value out of range".into(),
            }),
        }
    }

    fn set_nd_filter_stops(&self, stops: f32) -> M::Ret<'_, Result<(), Error>> {
        match NdFilterValue::from_stops(stops) {
            Ok(cmd) => self.send_and_complete(cmd),
            Err(_) => self.error(Error::InvalidParameter {
                parameter: "stops",
                value: stops.to_string().into(),
                reason: "ND filter stops must be between 2.0 and 7.0".into(),
            }),
        }
    }

    fn step_nd_filter(&self, direction: NdFilterStep) -> M::Ret<'_, Result<(), Error>> {
        let cmd = NdFilterStepCommand::new(direction);
        self.send_and_complete(cmd)
    }

    fn set_auto_nd(&self, enabled: bool) -> M::Ret<'_, Result<(), Error>> {
        let cmd = AutoNdCommand::new(enabled);
        self.send_and_complete(cmd)
    }

    fn get_nd_filter(&self) -> M::Ret<'_, Result<u8, Error>> {
        self.send_and_parse(NdFilterInquiry)
    }
}
