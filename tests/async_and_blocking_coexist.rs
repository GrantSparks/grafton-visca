//! Test that async and blocking APIs are properly separated and don't interfere with each other.
//!
//! These features are mutually exclusive by design.

#[cfg(not(feature = "mode-async"))]
#[test]
fn test_blocking_mode_compile() {
    use grafton_visca::{
        camera::profiles::PtzOpticsG2, transport::BlockingTransportHandle, BlockingClient,
    };

    type _BlockingCamera = BlockingClient<PtzOpticsG2, BlockingTransportHandle>;
    fn _accepts_blocking(_camera: &_BlockingCamera) {}
}

#[cfg(feature = "runtime-tokio")]
#[test]
fn test_async_mode_compile() {
    use grafton_visca::{
        camera::profiles::PtzOpticsG2, camera::AsyncCamera,
        runtime_adapters::tokio::TcpTransport as AsyncTcp, TokioExecutor,
    };

    type _AsyncCamera = AsyncCamera<PtzOpticsG2, AsyncTcp, TokioExecutor>;
    fn _accepts_async(_camera: &_AsyncCamera) {}
}

#[cfg(not(feature = "mode-async"))]
#[test]
fn test_blocking_traits_available() {
    use grafton_visca::{
        FocusControl, InquiryControl, PanTiltControl, PowerControl, PresetsControl, ZoomControl,
    };

    fn _uses_blocking_traits<T>()
    where
        T: PowerControl
            + ZoomControl
            + FocusControl
            + PanTiltControl
            + PresetsControl
            + InquiryControl,
    {
    }
}

#[cfg(feature = "mode-async")]
#[test]
fn test_async_traits_with_feature() {
    use grafton_visca::{
        FocusControl, InquiryControl, PanTiltControl, PowerControl, PresetsControl, ZoomControl,
    };

    fn _uses_async_traits<T>()
    where
        T: PowerControl
            + ZoomControl
            + FocusControl
            + PanTiltControl
            + PresetsControl
            + InquiryControl,
    {
    }
}

#[test]
fn test_preludes_per_mode() {
    #[cfg(not(feature = "mode-async"))]
    {
        use grafton_visca::prelude::blocking as blocking_prelude;
        use grafton_visca::{transport::BlockingTransportHandle, BlockingClient};
        // Use the camera-first API instead of direct transport access
        type _BlockingG2 = BlockingClient<blocking_prelude::PtzOpticsG2, BlockingTransportHandle>;
        let _ = core::any::type_name::<_BlockingG2>();
    }

    #[cfg(all(feature = "mode-async", feature = "runtime-tokio"))]
    {
        use grafton_visca::prelude::r#async as async_prelude;
        use grafton_visca::runtime_adapters::tokio::TcpTransport as AsyncTcp;
        // The async prelude doesn't export type aliases like PtzOpticsG2Cam
        // Instead, we use runtime-specific aliases or construct the type directly
        use grafton_visca::{AsyncCamera, TokioExecutor};
        type _AsyncG2 = AsyncCamera<async_prelude::PtzOpticsG2, AsyncTcp, TokioExecutor>;
        let _ = core::any::type_name::<_AsyncG2>();
    }
}
