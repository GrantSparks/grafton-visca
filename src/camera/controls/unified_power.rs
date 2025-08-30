//! Mode-parametrized power control trait using the Mode trait system.

use crate::{mode::Mode, Error};

/// Unified power control trait that works with both blocking and async modes.
///
/// This trait uses the Mode trait system to provide a single API surface
/// that works correctly in both blocking and async contexts. The return
/// types adapt automatically based on the Mode parameter.
///
/// # Examples
///
/// ```rust,ignore
/// use grafton_visca::{Camera, mode::{Async, Blocking}};
/// use grafton_visca::camera::controls::unified_power::UnifiedPowerControl;
///
/// // Async usage
/// let async_camera: Camera<Async, Profile, Transport, Executor> = ...;
/// async_camera.power_on().await?; // Returns a future
///
/// // Blocking usage  
/// let blocking_camera: Camera<Blocking, Profile, Transport, ()> = ...;
/// blocking_camera.power_on().await?; // Returns immediately via Ready<T>
/// ```
pub trait UnifiedPowerControl<M>
where
    M: Mode,
{
    /// Power on the camera.
    ///
    /// Returns `M::Ret<Result<(), Error>>` which adapts to the mode:
    /// - Async mode: Returns a boxed future
    /// - Blocking mode: Returns Ready<Result<(), Error>> (immediate)
    fn power_on(&self) -> M::Ret<Result<(), Error>>;

    /// Power off the camera.
    ///
    /// Returns `M::Ret<Result<(), Error>>` which adapts to the mode:
    /// - Async mode: Returns a boxed future  
    /// - Blocking mode: Returns Ready<Result<(), Error>> (immediate)
    fn power_off(&self) -> M::Ret<Result<(), Error>>;

    /// Query the current power status.
    ///
    /// Returns `M::Ret<Result<bool, Error>>` which adapts to the mode:
    /// - Async mode: Returns a boxed future
    /// - Blocking mode: Returns Ready<Result<bool, Error>> (immediate)
    fn power_inquiry(&self) -> M::Ret<Result<bool, Error>>;
}

// Implementation for the unified camera type
impl<M, P, Tr, Exec> UnifiedPowerControl<M> for crate::camera::unified::Camera<M, P, Tr, Exec>
where
    M: Mode + 'static,
    P: crate::capabilities::Profile + Default,
    Tr: Send + Sync,
{
    fn power_on(&self) -> M::Ret<Result<(), Error>> {
        use crate::command::power::Power;
        let cmd = Power::On;
        self.send_command(&cmd)
    }

    fn power_off(&self) -> M::Ret<Result<(), Error>> {
        use crate::command::power::Power;
        let cmd = Power::Standby;
        self.send_command(&cmd)
    }

    fn power_inquiry(&self) -> M::Ret<Result<bool, Error>> {
        use crate::command::inquiry_structs::PowerInquiry;
        let cmd = PowerInquiry;

        // Use send_command_typed to get the typed response
        // This now connects to actual transport operations
        self.send_command_typed(&cmd)
    }
}

#[cfg(test)]
mod tests {

    #[tokio::test]
    async fn test_unified_power_control_concept() {
        // This test demonstrates the concept - actual implementation would need
        // real camera instances

        // The key insight is that both async and blocking modes can be awaited:
        // - Async returns actual futures
        // - Blocking returns Ready<T> which immediately resolves

        // This provides a truly unified API surface!
    }
}
