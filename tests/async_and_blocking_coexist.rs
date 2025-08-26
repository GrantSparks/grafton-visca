//! Test that async and blocking APIs are properly separated and don't interfere with each other.
//!
//! These features are mutually exclusive by design.

#[cfg(not(feature = "async"))]
#[test]
fn test_blocking_mode_compile() {
    use grafton_visca::{
        camera::{profiles::PtzOpticsG2, BlockingMode, Camera},
        transport::blocking::Tcp as BlockingTcp,
    };

    type _BlockingCamera = Camera<BlockingMode, PtzOpticsG2, BlockingTcp, ()>;
    fn _accepts_blocking(_camera: &_BlockingCamera) {}
}

#[cfg(feature = "rt-tokio")]
#[test]
fn test_async_mode_compile() {
    use grafton_visca::{
        camera::profiles::PtzOpticsG2, camera::AsyncMode, camera::Camera,
        runtime_adapters::tokio::TcpTransport as AsyncTcp, TokioExecutor,
    };

    type _AsyncCamera = Camera<AsyncMode, PtzOpticsG2, AsyncTcp, TokioExecutor>;
    fn _accepts_async(_camera: &_AsyncCamera) {}
}

#[cfg(not(feature = "async"))]
#[test]
fn test_blocking_traits_available() {
    use grafton_visca::{
        FocusControlBlocking, InquiryControlBlocking, PanTiltControlBlocking, PowerControlBlocking,
        PresetsControlBlocking, ZoomControlBlocking,
    };

    fn _uses_blocking_traits<T>()
    where
        T: PowerControlBlocking
            + ZoomControlBlocking
            + FocusControlBlocking
            + PanTiltControlBlocking
            + PresetsControlBlocking
            + InquiryControlBlocking,
    {
    }
}

#[cfg(feature = "async")]
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
    #[cfg(not(feature = "async"))]
    {
        use grafton_visca::prelude::blocking as blocking_prelude;
        use grafton_visca::transport::blocking::Tcp as BlockingTcp;
        type _BlockingG2 = blocking_prelude::PtzOpticsG2Cam<BlockingTcp>;
        let _ = core::any::type_name::<_BlockingG2>();
    }

    #[cfg(all(feature = "async", feature = "rt-tokio"))]
    {
        use grafton_visca::prelude::r#async as async_prelude;
        use grafton_visca::runtime_adapters::tokio::TcpTransport as AsyncTcp;
        // The async prelude doesn't export type aliases like PtzOpticsG2Cam
        // Instead, we use runtime-specific aliases or construct the type directly
        use grafton_visca::TokioCamera;
        type _AsyncG2 = TokioCamera<async_prelude::PtzOpticsG2, AsyncTcp>;
        let _ = core::any::type_name::<_AsyncG2>();
    }
}
