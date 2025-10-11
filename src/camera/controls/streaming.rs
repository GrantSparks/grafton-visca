//! Streaming control implementation for PTZ cameras.
//!
//! This module provides network streaming control functionality including:
//! - Multicast streaming enable/disable for network distribution
//! - NDI (Network Device Interface) quality control for professional workflows
//! - Streaming protocol configuration for broadcast applications
//!
//! Streaming controls are essential for cameras used in live production
//! environments where video needs to be distributed over IP networks.
//! These features are typically found on professional PTZ cameras that
//! support network-based video distribution protocols.
//!
//! The implementation uses the Mode trait to provide both blocking and async APIs
//! from a single unified codebase.

use crate::{camera::ViscaClient, mode::Mode, types::NdiQuality, Error};

/// Streaming operations for PTZ cameras.
///
/// This trait provides network streaming control methods that work seamlessly for both
/// blocking and async cameras through the Mode trait system.
///
/// # Streaming Protocols
///
/// - **Multicast**: Efficiently distributes video to multiple recipients on a network
/// - **NDI**: NewTek's Network Device Interface for professional video workflows
///
/// # Quality Considerations
///
/// Different streaming qualities balance bandwidth usage with video fidelity:
/// - Higher quality = better image, more bandwidth
/// - Lower quality = reduced image quality, less bandwidth
/// - Quality settings affect both local processing and network load
///
/// # Examples
///
/// ## Blocking mode
/// ```ignore
/// camera.enable_multicast()?;  // Enable multicast streaming
/// camera.set_ndi_quality(NdiQuality::High)?;  // Set high quality NDI
/// camera.disable_multicast()?;  // Disable when done
/// ```
///
/// ## Async mode
/// ```ignore
/// camera.enable_multicast().await?;  // Enable multicast streaming
/// camera.set_ndi_quality(NdiQuality::High).await?;  // Set high quality NDI
/// camera.disable_multicast().await?;  // Disable when done
/// ```
#[grafton_visca_macros::delegate_to_session]
pub trait StreamingControl {
    /// The mode type for this camera (Async or Blocking).
    type Mode: Mode;

    /// Enable multicast streaming for NDI cameras.
    ///
    /// Enables multicast network streaming, allowing the camera's video output
    /// to be distributed to multiple recipients simultaneously over IP networks.
    /// This is more efficient than unicast when serving multiple clients.
    ///
    /// # Errors
    /// Returns an error if the command fails or multicast streaming is not supported.
    fn enable_multicast(&self) -> <Self::Mode as Mode>::Fut<'_, Result<(), Error>>;

    /// Disable multicast streaming for NDI cameras.
    ///
    /// Disables multicast network streaming. The camera may continue to support
    /// unicast streaming or other video output methods.
    ///
    /// # Errors
    /// Returns an error if the command fails or multicast streaming is not supported.
    fn disable_multicast(&self) -> <Self::Mode as Mode>::Fut<'_, Result<(), Error>>;

    /// Set the NDI streaming quality.
    ///
    /// Configures the quality level for NDI (Network Device Interface) streaming.
    /// Higher quality settings provide better image fidelity but require more
    /// network bandwidth and processing power.
    ///
    /// # Parameters
    /// - `quality`: The NDI quality level to set
    ///
    /// # Errors
    /// Returns an error if the command fails or NDI streaming is not supported.
    fn set_ndi_quality(
        &self,
        quality: NdiQuality,
    ) -> <Self::Mode as Mode>::Fut<'_, Result<(), Error>>;
}

// Single unified implementation for all Camera types!
impl<M, P, Tr, Exec> StreamingControl for crate::camera::Camera<M, P, Tr, Exec>
where
    M: Mode,
    P: crate::capabilities::Profile + Default,
    Self: ViscaClient<M>,
    Exec: crate::executor::Executor,
{
    type Mode = M;

    fn enable_multicast(&self) -> M::Fut<'_, Result<(), Error>> {
        use crate::command::streaming::MulticastStreaming;
        self.execute(MulticastStreaming::On)
    }

    fn disable_multicast(&self) -> M::Fut<'_, Result<(), Error>> {
        use crate::command::streaming::MulticastStreaming;
        self.execute(MulticastStreaming::Off)
    }

    fn set_ndi_quality(&self, quality: NdiQuality) -> M::Fut<'_, Result<(), Error>> {
        use crate::command::streaming::SetNdiQuality;
        self.execute(SetNdiQuality::new(quality))
    }
}
