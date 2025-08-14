//! Test that both async and blocking APIs can coexist in the same build.

use grafton_visca::{
    camera::profiles::PTZOpticsG2,
    camera::{BlockingMode, Camera},
    transport::blocking::Tcp as BlockingTcp,
};

#[cfg(feature = "rt-tokio")]
use grafton_visca::camera::AsyncMode;

#[cfg(feature = "rt-tokio")]
use grafton_visca::{transport::tokio::tcp::Tcp as AsyncTcp, TokioExecutor};

// Test that both blocking and async camera types can be defined in the same module
#[test]
fn test_both_modes_compile() {
    // Blocking camera type
    type _BlockingCamera = Camera<BlockingMode, PTZOpticsG2, BlockingTcp, ()>;

    // Async camera type (when tokio feature is enabled)
    #[cfg(feature = "rt-tokio")]
    type _AsyncCamera = Camera<AsyncMode, PTZOpticsG2, AsyncTcp, TokioExecutor>;

    // Both types should compile without conflict
    fn _accepts_blocking(_camera: &_BlockingCamera) {}

    #[cfg(feature = "rt-tokio")]
    fn _accepts_async(_camera: &_AsyncCamera) {}
}

// Test that blocking traits are available regardless of async feature
#[test]
fn test_blocking_traits_always_available() {
    use grafton_visca::{
        FocusOpsBlocking, InquiryOpsBlocking, PanTiltOpsBlocking, PowerOpsBlocking,
        PresetsOpsBlocking, ZoomOpsBlocking,
    };

    // These traits should be available even with async feature enabled
    fn _uses_blocking_traits<T>()
    where
        T: PowerOpsBlocking
            + ZoomOpsBlocking
            + FocusOpsBlocking
            + PanTiltOpsBlocking
            + PresetsOpsBlocking
            + InquiryOpsBlocking,
    {
    }
}

// Test that async traits are available when async feature is enabled
#[cfg(feature = "async")]
#[test]
fn test_async_traits_with_feature() {
    use grafton_visca::{FocusOps, InquiryOps, PanTiltOps, PowerOps, PresetsOps, ZoomOps};

    // These traits should only be available with async feature
    fn _uses_async_traits<T>()
    where
        T: PowerOps + ZoomOps + FocusOps + PanTiltOps + PresetsOps + InquiryOps,
    {
    }
}

// Test that both preludes can be used
#[test]
fn test_both_preludes_available() {
    // Blocking prelude should always be available
    use grafton_visca::prelude::blocking as blocking_prelude;

    // Use types from blocking prelude
    type _BlockingG2 = blocking_prelude::PTZOpticsG2Cam<BlockingTcp>;

    // Use types from async prelude when available
    #[cfg(all(feature = "async", feature = "rt-tokio"))]
    {
        use grafton_visca::prelude::r#async as async_prelude;
        type _AsyncG2 = async_prelude::PTZOpticsG2Cam<AsyncTcp>;
    }
}
