//! High-level extension trait for advanced power control operations.

// Crate imports
use crate::{
    command::InquiryCommand, error::ViscaError, transport_ext::ViscaTransportExt, ViscaDevice,
    ViscaResponse,
};

/// Extension trait providing advanced power control methods.
///
/// This trait extends the basic power control methods in `ViscaTransportExt`
/// with additional convenience functions.
pub trait ViscaPowerExt: ViscaDevice {
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
    /// # use grafton_visca::{ViscaError, ViscaDevice, ViscaPowerExt, ViscaTransportExt};
    /// # fn example(client: &mut impl ViscaDevice) -> Result<(), ViscaError> {
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
            ViscaResponse::InquiryResponse(crate::ViscaInquiryResponse::Power { on }) => Ok(on),
            _ => Err(ViscaError::UnexpectedResponseType),
        }
    }

    /// Power cycle the camera (turn off and back on).
    ///
    /// This method will:
    /// 1. Power off the camera
    /// 2. Wait for the specified duration
    /// 3. Power the camera back on
    ///
    /// # Arguments
    /// * `wait_duration` - How long to wait between power off and power on
    ///
    /// # Errors
    /// * `ViscaError::NetworkError` - Communication error with the camera
    /// * `ViscaError::CommandFailed` - Failed to power off or power on the camera
    ///
    /// # Example
    /// ```no_run
    /// # use grafton_visca::{ViscaError, ViscaDevice, ViscaPowerExt};
    /// # use std::time::Duration;
    /// # fn example(client: &mut impl ViscaDevice) -> Result<(), ViscaError> {
    /// // Power cycle with 2 second wait
    /// client.power_cycle(Duration::from_secs(2))?;
    /// # Ok(())
    /// # }
    /// ```
    fn power_cycle(&mut self, wait_duration: std::time::Duration) -> Result<(), ViscaError>
    where
        Self: Sized,
    {
        // Power off
        <Self as ViscaTransportExt>::power_off(self)?;

        // Wait
        std::thread::sleep(wait_duration);

        // Power on
        <Self as ViscaTransportExt>::power_on(self)?;

        Ok(())
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
    /// # use grafton_visca::{ViscaError, ViscaDevice, ViscaPowerExt};
    /// # fn example(client: &mut impl ViscaDevice) -> Result<(), ViscaError> {
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
                <Self as ViscaTransportExt>::power_on(self)?;
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
    /// # use grafton_visca::{ViscaError, ViscaDevice, ViscaPowerExt, ViscaTransportExt};
    /// # use std::time::Duration;
    /// # fn example(client: &mut impl ViscaDevice) -> Result<(), ViscaError> {
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
}

/// Implement the trait for all types that implement `ViscaTransportExt`
impl<T: ViscaDevice> ViscaPowerExt for T {}
