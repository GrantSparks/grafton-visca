//! Power control implementation for PTZ cameras.
//!
//! This module provides power management functionality for PTZ cameras including:
//! - Power on/off operations
//! - Standby mode control
//! - Remote power management via VISCA protocol
//!
//! Power control allows you to manage the camera's power state remotely,
//! which is useful for energy management, maintenance, and system automation.
//! Note that powering off a camera will terminate the connection and may
//! require physical intervention to power back on depending on the camera model.
//!
//! The implementation uses the Mode trait to provide both blocking and async APIs
//! from a single unified codebase.

use crate::{camera::ViscaClient, mode::Mode, Error};

/// Power operations for PTZ cameras.
///
/// This trait provides power control methods that work seamlessly for both
/// blocking and async cameras through the Mode trait system.
///
/// # Important Notes
///
/// - **Power Off**: When you power off a camera, the network connection will be lost
/// - **Recovery**: Some cameras may require physical power cycling to turn back on
/// - **Standby**: Power off typically puts the camera in standby mode, not complete shutdown
/// - **Auto-start**: Some cameras support wake-on-LAN or auto-power-on features
///
/// # Examples
///
/// ## Blocking mode
/// ```ignore
/// camera.power_on()?;  // Turn camera on
/// // ... use camera ...
/// camera.power_off()?;  // Put camera in standby
/// ```
///
/// ## Async mode
/// ```ignore
/// camera.power_on().await?;  // Turn camera on
/// // ... use camera ...
/// camera.power_off().await?;  // Put camera in standby
/// ```
#[grafton_visca_macros::delegate_to_session]
pub trait PowerControl {
    /// The mode type for this camera (Async or Blocking).
    type Mode: Mode;

    /// Power on the camera.
    ///
    /// Sends a power-on command to wake the camera from standby mode.
    /// The camera will initialize and become ready for operation.
    ///
    /// # Errors
    /// Returns an error if the command fails to send or receive a response.
    /// Note that if the camera is already powered on, this may still succeed.
    fn power_on(&self) -> <Self::Mode as Mode>::Fut<'_, Result<(), Error>>;

    /// Power off the camera.
    ///
    /// Sends a power-off command to put the camera into standby mode.
    /// This will terminate the current connection and the camera will stop responding
    /// to commands until powered back on.
    ///
    /// # Important
    /// After calling this function, the camera connection will be lost and you may
    /// need to physically power cycle the camera or use wake-on-LAN to reconnect.
    ///
    /// # Errors
    /// Returns an error if the command fails to send. The response may not be
    /// received if the camera powers off immediately.
    fn power_off(&self) -> <Self::Mode as Mode>::Fut<'_, Result<(), Error>>;
}

impl<M, P, Tr, Exec> PowerControl for crate::camera::Camera<M, P, Tr, Exec>
where
    M: Mode,
    P: crate::capabilities::Profile + Default,
    Self: ViscaClient<M>,
    Exec: crate::executor::Executor,
{
    type Mode = M;

    fn power_on(&self) -> M::Fut<'_, Result<(), Error>> {
        use crate::command::power::PowerOn;
        self.execute(PowerOn::new())
    }

    fn power_off(&self) -> M::Fut<'_, Result<(), Error>> {
        use crate::command::power::PowerStandby;
        self.execute(PowerStandby::new())
    }
}
