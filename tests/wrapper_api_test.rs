//! Test the new wrapper API to ensure it compiles and works correctly.

#[cfg(not(feature = "async"))]
#[test]
fn test_blocking_wrapper_api() {
    use grafton_visca::{
        camera::{BlockingMode, Camera},
        capabilities::Profile,
        transport::BlockingTransport,
        ZoomControlBlocking,
    };

    fn _example<P: Profile, T>(
        camera: &mut Camera<BlockingMode, P, T, ()>,
    ) -> Result<(), grafton_visca::Error>
    where
        T: BlockingTransport + Send + Sync + 'static,
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
        camera::{AsyncMode, Camera},
        capabilities::Profile,
        transport::AsyncTransport,
        ZoomControl,
    };
    #[allow(dead_code)]
    async fn example<
        P: Profile,
        T: AsyncTransport + Send + Sync + 'static,
        E: grafton_visca::Executor,
    >(
        camera: &Camera<AsyncMode, P, T, E>,
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
    use grafton_visca::{
        camera::{profiles::PtzOpticsG2, Camera},
        capabilities::Profile,
    };

    #[cfg(feature = "async")]
    use grafton_visca::{camera::AsyncMode, transport::AsyncTransport};

    #[cfg(not(feature = "async"))]
    use grafton_visca::{camera::BlockingMode, transport::BlockingTransport};

    #[cfg(feature = "async")]
    fn _check_camera_type<P: Profile, T: AsyncTransport + Send + Sync + 'static>() {
        let _: Option<Camera<AsyncMode, P, T>> = None;
    }

    #[cfg(not(feature = "async"))]
    fn _check_camera_type<P: Profile, T: BlockingTransport + Send + Sync + 'static>() {
        let _: Option<Camera<BlockingMode, P, T>> = None;
    }

    #[cfg(feature = "async")]
    fn _check_specific_types<T: AsyncTransport + Send + Sync + 'static>() {
        use grafton_visca::camera::profiles::{GenericVisca, SonyFR7};

        let _: Option<Camera<AsyncMode, PtzOpticsG2, T>> = None;
        let _: Option<Camera<AsyncMode, SonyFR7, T>> = None;
        let _: Option<Camera<AsyncMode, GenericVisca, T>> = None;
    }

    #[cfg(not(feature = "async"))]
    fn _check_specific_types<T: BlockingTransport + Send + Sync + 'static>() {
        use grafton_visca::camera::profiles::{GenericVisca, SonyFR7};

        let _: Option<Camera<BlockingMode, PtzOpticsG2, T>> = None;
        let _: Option<Camera<BlockingMode, SonyFR7, T>> = None;
        let _: Option<Camera<BlockingMode, GenericVisca, T>> = None;
    }
}
