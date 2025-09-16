//! Test the new wrapper API to ensure it compiles and works correctly.

#[cfg(not(feature = "mode-async"))]
#[test]
fn test_blocking_wrapper_api() {
    // External crates
    use grafton_visca::{
        capabilities::Profile,
        transport::{HasTransportConfig, SyncTransport},
        BlockingClient,
    };

    fn _example<P: Profile + Default, T>(
        camera: &BlockingClient<P, T>,
    ) -> Result<(), grafton_visca::Error>
    where
        T: SyncTransport + HasTransportConfig + Send + Sync + 'static,
    {
        camera.zoom_stop()?;
        camera.zoom_tele_std()?;
        camera.zoom_wide_std()?;
        camera.zoom_absolute(grafton_visca::units::Normalized(0.5))?;
        Ok(())
    }
}

#[cfg(feature = "runtime-tokio")]
#[tokio::test]
async fn test_async_wrapper_api() {
    // External crates
    use grafton_visca::{
        camera::AsyncCamera, capabilities::Profile, transport::AsyncTransport, ZoomControl,
    };
    #[allow(dead_code)]
    async fn example<
        P: Profile + Default,
        T: AsyncTransport + Send + Sync + 'static,
        E: grafton_visca::Executor,
    >(
        camera: &AsyncCamera<P, T, E>,
    ) -> Result<(), grafton_visca::Error> {
        camera.zoom().stop().await?;
        camera.zoom().tele().await?;
        camera.zoom().wide().await?;
        camera
            .zoom()
            .absolute(grafton_visca::units::Normalized(0.5))
            .await?;
        Ok(())
    }
}

#[test]
fn test_wrapper_creation() {
    // External crates
    use grafton_visca::{camera::profiles::PtzOpticsG2, capabilities::Profile};

    #[cfg(feature = "mode-async")]
    use grafton_visca::{camera::AsyncCamera, transport::AsyncTransport};

    #[cfg(not(feature = "mode-async"))]
    use grafton_visca::{transport::SyncTransport, BlockingCamera};

    #[cfg(feature = "mode-async")]
    fn _check_camera_type<
        P: Profile + Default,
        T: AsyncTransport + Send + Sync + 'static,
        E: grafton_visca::Executor,
    >() {
        let _: Option<AsyncCamera<P, T, E>> = None;
    }

    #[cfg(not(feature = "mode-async"))]
    fn _check_camera_type<P: Profile + Default, T: SyncTransport + Send + Sync + 'static>() {
        let _: Option<BlockingCamera<P, T>> = None;
    }

    #[cfg(feature = "mode-async")]
    fn _check_specific_types<
        T: AsyncTransport + Send + Sync + 'static,
        E: grafton_visca::Executor,
    >() {
        use grafton_visca::camera::profiles::{GenericVisca, SonyFR7};

        let _: Option<AsyncCamera<PtzOpticsG2, T, E>> = None;
        let _: Option<AsyncCamera<SonyFR7, T, E>> = None;
        let _: Option<AsyncCamera<GenericVisca, T, E>> = None;
    }

    #[cfg(not(feature = "mode-async"))]
    fn _check_specific_types<T: SyncTransport + Send + Sync + 'static>() {
        use grafton_visca::camera::profiles::{GenericVisca, SonyFR7};

        let _: Option<BlockingCamera<PtzOpticsG2, T>> = None;
        let _: Option<BlockingCamera<SonyFR7, T>> = None;
        let _: Option<BlockingCamera<GenericVisca, T>> = None;
    }
}
