//! High-level extension trait for advanced power control operations.

#![allow(deprecated)]

// Crate imports
use crate::{
    command::InquiryCommand, error::ViscaError, transport_ext::ViscaTransportExt, ViscaResponse,
};

/// Extension trait providing advanced power control methods.
///
/// This trait extends the basic power control methods in `ViscaTransportExt`
/// with additional convenience functions.
pub trait ViscaPowerExt: ViscaTransportExt {
    /// Check if the camera is powered on.
    ///
    /// # Returns
    /// * `Ok(true)` - Camera is powered on
    /// * `Ok(false)` - Camera is powered off or in standby
    /// * `Err(_)` - Communication error
    ///
    /// # Example
    /// ```no_run
    /// # use grafton_visca::{ViscaError, ViscaTransport, ViscaPowerExt, ViscaTransportExt};
    /// # fn example(transport: &mut impl ViscaTransport) -> Result<(), ViscaError> {
    /// if transport.is_powered_on()? {
    ///     println!("Camera is powered on");
    /// } else {
    ///     println!("Camera is powered off");
    ///     transport.power_on()?;
    /// }
    /// # Ok(())
    /// # }
    /// ```
    fn is_powered_on(&mut self) -> Result<bool, ViscaError>
    where
        Self: Sized,
    {
        let response = self.send_and_wait(&InquiryCommand::Power)?;

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
    /// # Example
    /// ```no_run
    /// # use grafton_visca::{ViscaError, ViscaTransport, ViscaPowerExt};
    /// # use std::time::Duration;
    /// # fn example(transport: &mut impl ViscaTransport) -> Result<(), ViscaError> {
    /// // Power cycle with 2 second wait
    /// transport.power_cycle(Duration::from_secs(2))?;
    /// # Ok(())
    /// # }
    /// ```
    fn power_cycle(&mut self, wait_duration: std::time::Duration) -> Result<(), ViscaError>
    where
        Self: Sized,
    {
        // Power off
        self.power_off()?;

        // Wait
        std::thread::sleep(wait_duration);

        // Power on
        self.power_on()?;

        Ok(())
    }

    /// Ensure the camera is powered on.
    ///
    /// This method checks if the camera is powered on and powers it on if necessary.
    ///
    /// # Returns
    /// * `Ok(true)` - Camera was already powered on
    /// * `Ok(false)` - Camera was powered off and has been powered on
    /// * `Err(_)` - Communication error or failed to power on
    ///
    /// # Example
    /// ```no_run
    /// # use grafton_visca::{ViscaError, ViscaTransport, ViscaPowerExt};
    /// # fn example(transport: &mut impl ViscaTransport) -> Result<(), ViscaError> {
    /// // Ensure camera is ready before sending commands
    /// let was_on = transport.ensure_powered_on()?;
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
                self.power_on()?;
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
    /// # Example
    /// ```no_run
    /// # use grafton_visca::{ViscaError, ViscaTransport, ViscaPowerExt, ViscaTransportExt};
    /// # use std::time::Duration;
    /// # fn example(transport: &mut impl ViscaTransport) -> Result<(), ViscaError> {
    /// // Power on and wait up to 10 seconds for camera to be ready
    /// transport.power_on()?;
    /// transport.wait_for_power_on(
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
impl<T: ViscaTransportExt> ViscaPowerExt for T {}
