//! Test the new wrapper API to ensure it compiles and works correctly.

#[cfg(not(feature = "async"))]
#[test]
fn test_blocking_wrapper_api() {
    use grafton_visca::{
        camera::BlockingCamera, capabilities::Profile, transport::SyncTransport, ZoomControl,
    };

    fn _example<P: Profile + Default, T>(
        camera: &mut BlockingCamera<P, T>,
    ) -> Result<(), grafton_visca::Error>
    where
        T: SyncTransport + Send + Sync + 'static,
    {
        camera.zoom_stop()?;
        camera.zoom_tele_std()?;
        camera.zoom_wide_std()?;
        camera.zoom_absolute(grafton_visca::units::Normalized(0.5))?;
        Ok(())
    }
}

#[cfg(feature = "rt-tokio")]
#[tokio::test]
async fn test_async_wrapper_api() {
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
        camera.zoom_stop().await?;
        camera.zoom_tele_std().await?;
        camera.zoom_wide_std().await?;
        camera
            .zoom_absolute(grafton_visca::units::Normalized(0.5))
            .await?;
        Ok(())
    }
}

#[test]
fn test_wrapper_creation() {
    use grafton_visca::{camera::profiles::PtzOpticsG2, capabilities::Profile};

    #[cfg(feature = "async")]
    use grafton_visca::{camera::AsyncCamera, transport::AsyncTransport};

    #[cfg(not(feature = "async"))]
    use grafton_visca::camera::BlockingCamera;

    #[cfg(not(feature = "async"))]
    use grafton_visca::transport::SyncTransport;

    #[cfg(feature = "async")]
    fn _check_camera_type<
        P: Profile + Default,
        T: AsyncTransport + Send + Sync + 'static,
        E: grafton_visca::Executor,
    >() {
        let _: Option<AsyncCamera<P, T, E>> = None;
    }

    #[cfg(not(feature = "async"))]
    fn _check_camera_type<P: Profile + Default, T: SyncTransport + Send + Sync + 'static>() {
        let _: Option<BlockingCamera<P, T>> = None;
    }

    #[cfg(feature = "async")]
    fn _check_specific_types<
        T: AsyncTransport + Send + Sync + 'static,
        E: grafton_visca::Executor,
    >() {
        use grafton_visca::camera::profiles::{GenericVisca, SonyFR7};

        let _: Option<AsyncCamera<PtzOpticsG2, T, E>> = None;
        let _: Option<AsyncCamera<SonyFR7, T, E>> = None;
        let _: Option<AsyncCamera<GenericVisca, T, E>> = None;
    }

    #[cfg(not(feature = "async"))]
    fn _check_specific_types<T: SyncTransport + Send + Sync + 'static>() {
        use grafton_visca::camera::profiles::{GenericVisca, SonyFR7};

        let _: Option<BlockingCamera<PtzOpticsG2, T>> = None;
        let _: Option<BlockingCamera<SonyFR7, T>> = None;
        let _: Option<BlockingCamera<GenericVisca, T>> = None;
    }
}
