//! Test the new wrapper API to ensure it compiles and works correctly.

#[cfg(not(feature = "async"))]
#[test]
fn test_blocking_wrapper_api() {
    use grafton_visca::camera::{BlockingMode, Camera};
    use grafton_visca::capabilities::Profile;
    use grafton_visca::transport::BlockingTransport;
    use grafton_visca::ZoomOpsBlocking;

    // This test just verifies the API compiles correctly
    // In a real test, you'd use a mock transport

    // The blocking wrapper should expose methods without _blocking suffix
    fn _example<P: Profile, T>(
        camera: &Camera<BlockingMode, P, T, ()>,
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

    // Just test that the function compiles, don't try to reference it
    // as that would require a concrete Transport type
}

#[cfg(feature = "rt-tokio")]
#[tokio::test]
async fn test_async_wrapper_api() {
    use grafton_visca::camera::{AsyncMode, Camera};
    use grafton_visca::capabilities::Profile;
    use grafton_visca::transport::AsyncTransport;
    use grafton_visca::ZoomOps;

    // This test just verifies the API compiles correctly
    // In a real test, you'd use a mock transport

    // The async wrapper should expose async methods
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
    // This would normally use a real transport
    // Here we just test the type system compiles
    use grafton_visca::camera::profiles::PTZOpticsG2;
    #[cfg(feature = "async")]
    use grafton_visca::camera::AsyncMode;
    #[cfg(not(feature = "async"))]
    use grafton_visca::camera::BlockingMode;
    use grafton_visca::camera::Camera;
    use grafton_visca::capabilities::Profile;
    #[cfg(feature = "async")]
    use grafton_visca::transport::AsyncTransport;
    #[cfg(not(feature = "async"))]
    use grafton_visca::transport::BlockingTransport;

    // Test that generic camera creation compiles with proper constraints
    // Note: These are just type checks, not actual implementations

    // Check that Camera type exists with proper bounds
    #[cfg(feature = "async")]
    fn _check_camera_type<P: Profile, T: AsyncTransport + Send + Sync + 'static>() {
        // This function body is never executed, we just check it compiles
        let _: Option<Camera<AsyncMode, P, T>> = None;
    }

    #[cfg(not(feature = "async"))]
    fn _check_camera_type<P: Profile, T: BlockingTransport + Send + Sync + 'static>() {
        // This function body is never executed, we just check it compiles
        let _: Option<Camera<BlockingMode, P, T>> = None;
    }

    // Check that specific camera types exist
    #[cfg(feature = "async")]
    fn _check_specific_types<T: AsyncTransport + Send + Sync + 'static>() {
        let _: Option<Camera<AsyncMode, PTZOpticsG2, T>> = None;
        let _: Option<Camera<AsyncMode, grafton_visca::camera::profiles::SonyFR7, T>> = None;
        let _: Option<Camera<AsyncMode, grafton_visca::camera::profiles::GenericVisca, T>> = None;
    }

    #[cfg(not(feature = "async"))]
    fn _check_specific_types<T: BlockingTransport + Send + Sync + 'static>() {
        let _: Option<Camera<BlockingMode, PTZOpticsG2, T>> = None;
        let _: Option<Camera<BlockingMode, grafton_visca::camera::profiles::SonyFR7, T>> = None;
        let _: Option<Camera<BlockingMode, grafton_visca::camera::profiles::GenericVisca, T>> =
            None;
    }
}
