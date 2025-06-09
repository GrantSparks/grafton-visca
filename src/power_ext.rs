//! High-level extension trait for advanced power control operations.

// Crate imports
use crate::{
    command::{InquiryCommand, PowerCommand, power::Power},
    error::Error as ViscaError, Transport, Response,
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
    /// # use grafton_visca::{ViscaError, Transport, ViscaPowerExt};
    /// # fn example(client: &mut impl Transport) -> Result<(), ViscaError> {
    /// if client.is_powered_on()? {
    ///     println!("Camera is powered on");
    /// } else {
    ///     println!("Camera is powered off");
    ///     // Use PowerCommand directly
    ///     client.execute_command(&PowerCommand { power: Power::On })?;
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
    /// # use grafton_visca::{ViscaError, Transport, ViscaPowerExt};
    /// # fn example(client: &mut impl Transport) -> Result<(), ViscaError> {
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
    /// # use grafton_visca::{ViscaError, Transport, ViscaPowerExt};
    /// # use std::time::Duration;
    /// # fn example(client: &mut impl Transport) -> Result<(), ViscaError> {
    /// // Power on and wait up to 10 seconds for camera to be ready
    /// // Power on using PowerCommand
    /// client.execute_command(&PowerCommand { power: Power::On })?;
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
}

/// Implement the trait for all types that implement `ViscaTransportExt`
impl<T: Transport> ViscaPowerExt for T {}
