//! Test the new wrapper API to ensure it compiles and works correctly.

use grafton_visca::{
    camera::profiles::{GenericVisca, PtzOpticsG2, SonyFR7},
    capabilities::Profile,
};

#[cfg(not(feature = "mode-async"))]
use grafton_visca::{
    transport::{BlockingTransport, HasTransportConfig},
    types::ZoomPosition,
    units::UnitInterval,
    BlockingCamera, BlockingClient, Error,
};

#[cfg(feature = "mode-async")]
use grafton_visca::{camera::AsyncCamera, transport::AsyncTransport, Executor};

#[cfg(all(feature = "mode-async", feature = "runtime-tokio"))]
use grafton_visca::{
    runtime::TransportHandle, types::ZoomPosition, units::UnitInterval, Error, TokioExecutor,
    TokioRuntime,
};

#[cfg(not(feature = "mode-async"))]
#[test]
fn test_blocking_wrapper_api() {
    fn _example<P: Profile + Default + grafton_visca::capabilities::HasDirectZoom, T>(
        camera: &BlockingClient<P, T>,
    ) -> Result<(), Error>
    where
        T: BlockingTransport + HasTransportConfig + Send + Sync + 'static,
    {
        camera.zoom_stop()?;
        camera.zoom_tele(None)?;
        camera.zoom_wide(None)?;
        camera.set_zoom(ZoomPosition::new(0x2000)?)?;
        camera.set_zoom_normalized(UnitInterval::new(0.5)?)?;
        Ok(())
    }
}

#[cfg(feature = "runtime-tokio")]
#[tokio::test]
async fn test_async_wrapper_api() {
    async fn example<
        P: Profile + Default + grafton_visca::capabilities::HasDirectZoom,
        T: AsyncTransport + Send + Sync + 'static,
        E: Executor + Send + Sync + Clone + 'static,
    >(
        camera: &AsyncCamera<P, T, E>,
    ) -> Result<(), Error> {
        camera.zoom().stop().await?;
        camera.zoom().tele().await?;
        camera.zoom().wide().await?;
        camera
            .zoom()
            .set_position(ZoomPosition::new(0x2000)?)
            .await?;
        camera
            .zoom()
            .set_normalized(UnitInterval::new(0.5)?)
            .await?;
        Ok(())
    }

    let _ = example::<PtzOpticsG2, TransportHandle<TokioRuntime>, TokioExecutor>;
}

#[test]
fn test_wrapper_creation() {
    #[cfg(feature = "mode-async")]
    fn _check_camera_type<
        P: Profile + Default,
        T: AsyncTransport + Send + Sync + 'static,
        E: Executor,
    >() {
        let _: Option<AsyncCamera<P, T, E>> = None;
    }

    #[cfg(not(feature = "mode-async"))]
    fn _check_camera_type<P: Profile + Default, T: BlockingTransport + Send + Sync + 'static>() {
        let _: Option<BlockingCamera<P, T>> = None;
    }

    #[cfg(feature = "mode-async")]
    fn _check_specific_types<T: AsyncTransport + Send + Sync + 'static, E: Executor>() {
        let _: Option<AsyncCamera<PtzOpticsG2, T, E>> = None;
        let _: Option<AsyncCamera<SonyFR7, T, E>> = None;
        let _: Option<AsyncCamera<GenericVisca, T, E>> = None;
    }

    #[cfg(not(feature = "mode-async"))]
    fn _check_specific_types<T: BlockingTransport + Send + Sync + 'static>() {
        let _: Option<BlockingCamera<PtzOpticsG2, T>> = None;
        let _: Option<BlockingCamera<SonyFR7, T>> = None;
        let _: Option<BlockingCamera<GenericVisca, T>> = None;
    }
}
