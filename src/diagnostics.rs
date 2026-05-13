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
