//! Diagnostics and health check utilities for PTZ cameras.
//!
//! This module provides tools to check camera connectivity, measure latency,
//! and perform health checks on VISCA-compatible cameras.

use std::time::Duration;

use crate::{error::Error, mode::Mode};

/// Result of a camera probe operation.
///
/// Contains information about the camera's responsiveness and connection health.
#[derive(Debug, Clone)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
pub struct ProbeReport {
    /// Round-trip time for the probe command, if measurable.
    pub rtt: Option<Duration>,
    /// Whether the transport layer is functioning correctly.
    pub transport_ok: bool,
    /// Optional firmware version if retrieved during probe.
    pub firmware_version: Option<String>,
}

impl ProbeReport {
    /// Create a new probe report.
    pub fn new(rtt: Option<Duration>, transport_ok: bool) -> Self {
        Self {
            rtt,
            transport_ok,
            firmware_version: None,
        }
    }

    /// Check if the probe was successful.
    pub fn is_healthy(&self) -> bool {
        self.transport_ok
    }
}

/// Extensions for cameras to add diagnostic methods.
///
/// This trait provides probe/ping functionality for health checks and latency measurement.
///
/// # Example
/// ```ignore
/// use grafton_visca::diagnostics::Diagnostics;
///
/// // Probe the camera
/// let report = camera.probe().await?;
/// if report.is_healthy() {
///     println!("Camera is responsive, RTT: {:?}", report.rtt);
/// }
///
/// // Simple ping check
/// if camera.ping().await? {
///     println!("Camera is online");
/// }
///
/// // Measure average latency
/// let latency = camera.measure_latency(5).await?;
/// println!("Average RTT: {:?}", latency);
/// ```
pub trait Diagnostics {
    /// The mode type for this camera.
    type Mode: Mode;

    /// Probe the camera for connectivity and health.
    ///
    /// Performs a lightweight version inquiry to check if the camera is responsive
    /// and measures the round-trip time.
    ///
    /// # Errors
    /// Returns an error if the camera is not reachable or the probe fails.
    fn probe(&self) -> <Self::Mode as Mode>::Fut<'_, Result<ProbeReport, Error>>;

    /// Ping the camera for a simple connectivity check.
    ///
    /// Returns `true` if the camera responds, `false` otherwise.
    ///
    /// # Errors
    /// Returns an error only for transport-level failures.
    fn ping(&self) -> <Self::Mode as Mode>::Fut<'_, Result<bool, Error>>;

    /// Measure round-trip latency to the camera.
    ///
    /// Performs multiple probe operations and returns the average RTT.
    ///
    /// # Arguments
    /// * `samples` - Number of measurements to average (minimum 1)
    ///
    /// # Errors
    /// Returns an error if no successful measurements could be made.
    fn measure_latency(
        &self,
        samples: usize,
    ) -> <Self::Mode as Mode>::Fut<'_, Result<Duration, Error>>;
}

// Macro to implement diagnostics for camera types
#[doc(hidden)]
#[macro_export]
macro_rules! impl_diagnostics {
    ($camera:ty) => {
        impl $crate::diagnostics::Diagnostics for $camera {
            type Mode = <$camera as $crate::camera::controls::inquiry::InquiryControl>::Mode;

            fn probe(
                &self,
            ) -> <Self::Mode as $crate::mode::Mode>::Fut<
                '_,
                Result<$crate::diagnostics::ProbeReport, $crate::error::Error>,
            > {
                use std::time::Instant;
                use $crate::camera::controls::inquiry::InquiryControl;
                use $crate::mode::Mode;

                // Perform a real inquiry and measure RTT
                Self::Mode::from_future(async move {
                    let start = Instant::now();

                    // Use power_state inquiry as it's lightweight and universally supported
                    match InquiryControl::power_state(self).await {
                        Ok(_) => {
                            let rtt = start.elapsed();
                            Ok($crate::diagnostics::ProbeReport::new(Some(rtt), true))
                        }
                        Err(e) => {
                            // Transport failed - return unsuccessful probe
                            let rtt = start.elapsed();
                            Ok($crate::diagnostics::ProbeReport::new(Some(rtt), false))
                        }
                    }
                })
            }

            fn ping(
                &self,
            ) -> <Self::Mode as $crate::mode::Mode>::Fut<'_, Result<bool, $crate::error::Error>>
            {
                use $crate::camera::controls::inquiry::InquiryControl;
                use $crate::mode::Mode;

                // Ping is just a simple check - return true if inquiry succeeds
                Self::Mode::from_future(async move {
                    match InquiryControl::power_state(self).await {
                        Ok(_) => Ok(true),
                        Err(_) => Ok(false),
                    }
                })
            }

            fn measure_latency(
                &self,
                samples: usize,
            ) -> <Self::Mode as $crate::mode::Mode>::Fut<
                '_,
                Result<std::time::Duration, $crate::error::Error>,
            > {
                use std::time::{Duration, Instant};
                use $crate::camera::controls::inquiry::InquiryControl;
                use $crate::mode::Mode;

                let samples = if samples == 0 { 1 } else { samples };

                // Perform multiple measurements and average them
                Self::Mode::from_future(async move {
                    let mut total_duration = Duration::ZERO;
                    let mut successful_samples = 0;

                    for _ in 0..samples {
                        let start = Instant::now();
                        if InquiryControl::power_state(self).await.is_ok() {
                            total_duration += start.elapsed();
                            successful_samples += 1;
                        }
                    }

                    if successful_samples == 0 {
                        Err($crate::error::Error::Io(std::io::Error::new(
                            std::io::ErrorKind::Other,
                            "No successful latency measurements",
                        )))
                    } else {
                        Ok(total_duration / successful_samples as u32)
                    }
                })
            }
        }
    };
}
