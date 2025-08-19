//! Test that both async and blocking APIs can coexist in the same build.

#[cfg(feature = "rt-tokio")]
use grafton_visca::{camera::AsyncMode, transport::tokio::tcp::Tcp as AsyncTcp, TokioExecutor};
use grafton_visca::{
    camera::{profiles::PtzOpticsG2, BlockingMode, Camera},
    transport::blocking::Tcp as BlockingTcp,
};

#[test]
fn test_both_modes_compile() {
    type _BlockingCamera = Camera<BlockingMode, PtzOpticsG2, BlockingTcp, ()>;

    #[cfg(feature = "rt-tokio")]
    type _AsyncCamera = Camera<AsyncMode, PtzOpticsG2, AsyncTcp, TokioExecutor>;
    fn _accepts_blocking(_camera: &_BlockingCamera) {}

    #[cfg(feature = "rt-tokio")]
    fn _accepts_async(_camera: &_AsyncCamera) {}
}

#[test]
fn test_blocking_traits_always_available() {
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
fn test_both_preludes_available() {
    use grafton_visca::prelude::blocking as blocking_prelude;

    type _BlockingG2 = blocking_prelude::PtzOpticsG2Cam<BlockingTcp>;
    #[cfg(all(feature = "async", feature = "rt-tokio"))]
    {
        use grafton_visca::prelude::r#async as async_prelude;

        type _AsyncG2 = async_prelude::PtzOpticsG2Cam<AsyncTcp>;
    }
}
