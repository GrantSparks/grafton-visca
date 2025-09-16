//! Test that async and blocking APIs are properly separated and don't interfere with each other.
//!
//! These features are mutually exclusive by design.

#[cfg(not(feature = "mode-async"))]
#[test]
fn test_blocking_mode_compile() {
    use grafton_visca::{camera::profiles::PtzOpticsG2, BlockingCamera};

    type _BlockingCamera =
        BlockingCamera<PtzOpticsG2, Box<dyn grafton_visca::transport::SyncTransport>>;
    fn _accepts_blocking(_camera: &_BlockingCamera) {}
}

#[cfg(feature = "rt-tokio")]
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
        use grafton_visca::BlockingCamera;
        // Use the camera-first API instead of direct transport access
        type _BlockingG2 = BlockingCamera<
            blocking_prelude::PtzOpticsG2,
            Box<dyn grafton_visca::transport::SyncTransport>,
        >;
        let _ = core::any::type_name::<_BlockingG2>();
    }

    #[cfg(all(feature = "mode-async", feature = "rt-tokio"))]
    {
        use grafton_visca::prelude::r#async as async_prelude;
        use grafton_visca::runtime_adapters::tokio::TcpTransport as AsyncTcp;
        // The async prelude doesn't export type aliases like PtzOpticsG2Cam
        // Instead, we use runtime-specific aliases or construct the type directly
        use grafton_visca::camera::builder::async_cameras::TokioCamera;
        type _AsyncG2 = TokioCamera<async_prelude::PtzOpticsG2, AsyncTcp>;
        let _ = core::any::type_name::<_AsyncG2>();
    }
}
