//! High-level extension trait for advanced power control operations.

// Crate imports
use crate::{
    command::{power::Power, InquiryCommand, PowerCommand},
    error::Error as ViscaError,
    Response, Transport,
};

/// Extension trait providing power control methods.
///
/// This trait provides all power-related control methods for VISCA devices.
pub trait ViscaPowerExt: Transport {
    /// Check if the camera is powered on.
    ///
    /// # Returns
    /// * `Ok(true)` - Camera is powered on
    /// * `Ok(false)` - Camera is powered off or in standby
    ///
    /// # Errors
    /// * `ViscaError::NetworkError` - Communication error with the camera
    /// * `ViscaError::UnexpectedResponseType` - Camera returned unexpected response format
    ///
    /// # Example
    /// ```no_run
    /// # use grafton_visca::{Error, Transport, ViscaPowerExt};
    /// # fn example(client: &mut impl Transport) -> Result<(), Error> {
    /// if client.is_powered_on()? {
    ///     println!("Camera is powered on");
    /// } else {
    ///     println!("Camera is powered off");
    ///     client.power_on()?;
    /// }
    /// # Ok(())
    /// # }
    /// ```
    fn is_powered_on(&mut self) -> Result<bool, ViscaError>
    where
        Self: Sized,
    {
        let response = self.execute_command(&InquiryCommand::Power)?;

        match response {
            Response::InquiryResponse(crate::ViscaInquiryResponse::Power { on }) => Ok(on),
            _ => Err(ViscaError::UnexpectedResponseType),
        }
    }

    /// Ensure the camera is powered on.
    ///
    /// This method checks if the camera is powered on and powers it on if necessary.
    ///
    /// # Returns
    /// * `Ok(true)` - Camera was already powered on
    /// * `Ok(false)` - Camera was powered off and has been powered on
    ///
    /// # Errors
    /// * `ViscaError::NetworkError` - Communication error with the camera
    /// * `ViscaError::CommandFailed` - Failed to power on the camera
    /// * `ViscaError::UnexpectedResponseType` - Camera returned unexpected response format
    ///
    /// # Example
    /// ```no_run
    /// # use grafton_visca::{Error, Transport, ViscaPowerExt};
    /// # fn example(client: &mut impl Transport) -> Result<(), Error> {
    /// // Ensure camera is ready before sending commands
    /// let was_on = client.ensure_powered_on()?;
    /// if !was_on {
    ///     println!("Camera was powered off, now powered on");
    ///     // May want to wait for camera to fully initialize
    ///     std::thread::sleep(std::time::Duration::from_secs(2));
    /// }
    /// # Ok(())
    /// # }
    /// ```
    fn ensure_powered_on(&mut self) -> Result<bool, ViscaError>
    where
        Self: Sized,
    {
        match self.is_powered_on() {
            Ok(true) => Ok(true),
            Ok(false) => {
                self.execute_command(&PowerCommand { power: Power::On })?;
                Ok(false)
            }
            Err(e) => Err(e),
        }
    }

    /// Wait for the camera to be ready after power on.
    ///
    /// This method repeatedly checks the power status until the camera responds
    /// as powered on, or until the timeout is reached.
    ///
    /// # Arguments
    /// * `timeout` - Maximum time to wait for camera to be ready
    /// * `check_interval` - How often to check the power status
    ///
    /// # Errors
    /// * `ViscaError::Timeout` - Camera did not power on within the specified timeout
    /// * `ViscaError::NetworkError` - Communication error while checking power status
    ///
    /// # Example
    /// ```no_run
    /// # use grafton_visca::{Error, Transport, ViscaPowerExt};
    /// # use std::time::Duration;
    /// # fn example(client: &mut impl Transport) -> Result<(), Error> {
    /// // Power on and wait up to 10 seconds for camera to be ready
    /// client.power_on()?;
    /// client.wait_for_power_on(
    ///     Duration::from_secs(10),
    ///     Duration::from_millis(500)
    /// )?;
    /// println!("Camera is ready!");
    /// # Ok(())
    /// # }
    /// ```
    fn wait_for_power_on(
        &mut self,
        timeout: std::time::Duration,
        check_interval: std::time::Duration,
    ) -> Result<(), ViscaError>
    where
        Self: Sized,
    {
        let start = std::time::Instant::now();

        loop {
            match self.is_powered_on() {
                Ok(true) => return Ok(()),
                Ok(false) => {
                    if start.elapsed() > timeout {
                        return Err(ViscaError::Timeout);
                    }
                    std::thread::sleep(check_interval);
                }
                Err(_) => {
                    // Camera might still be initializing
                    if start.elapsed() > timeout {
                        return Err(ViscaError::Timeout);
                    }
                    std::thread::sleep(check_interval);
                }
            }
        }
    }

    /// Power on the camera.
    ///
    /// This is a convenience method that sends the power on command.
    ///
    /// # Errors
    /// * `ViscaError::NetworkError` - Communication error with the camera
    /// * `ViscaError::CommandFailed` - Camera rejected the power on command
    ///
    /// # Example
    /// ```no_run
    /// # use grafton_visca::{Error, Transport, ViscaPowerExt};
    /// # fn example(client: &mut impl Transport) -> Result<(), Error> {
    /// // Power on the camera
    /// client.power_on()?;
    /// 
    /// // Wait for it to be ready
    /// client.wait_for_power_on(
    ///     std::time::Duration::from_secs(5),
    ///     std::time::Duration::from_millis(500)
    /// )?;
    /// # Ok(())
    /// # }
    /// ```
    fn power_on(&mut self) -> Result<(), ViscaError> {
        match self.execute_command(&PowerCommand { power: Power::On })? {
            Response::Completion => Ok(()),
            Response::Error(e) => Err(e),
            _ => Err(ViscaError::UnexpectedResponseType),
        }
    }

    /// Power off the camera (standby mode).
    ///
    /// This is a convenience method that sends the power off (standby) command.
    ///
    /// # Errors
    /// * `ViscaError::NetworkError` - Communication error with the camera
    /// * `ViscaError::CommandFailed` - Camera rejected the power off command
    ///
    /// # Example
    /// ```no_run
    /// # use grafton_visca::{Error, Transport, ViscaPowerExt};
    /// # fn example(client: &mut impl Transport) -> Result<(), Error> {
    /// // Power off the camera
    /// client.power_off()?;
    /// # Ok(())
    /// # }
    /// ```
    fn power_off(&mut self) -> Result<(), ViscaError> {
        match self.execute_command(&PowerCommand { power: Power::Standby })? {
            Response::Completion => Ok(()),
            Response::Error(e) => Err(e),
            _ => Err(ViscaError::UnexpectedResponseType),
        }
    }
}

/// Implement the trait for all types that implement `Transport`
impl<T: Transport> ViscaPowerExt for T {}
