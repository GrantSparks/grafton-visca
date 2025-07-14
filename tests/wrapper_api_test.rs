//! Test the new wrapper API to ensure it compiles and works correctly.

#[test]
fn test_blocking_wrapper_api() {
    use grafton_visca::blocking::{Camera, ZoomOps};

    // This test just verifies the API compiles correctly
    // In a real test, you'd use a mock transport

    // The blocking wrapper should expose methods without _blocking suffix
    let _example = |camera: &Camera| -> Result<(), grafton_visca::Error> {
        camera.zoom_stop()?;
        camera.zoom_in()?;
        camera.zoom_out()?;
        camera.zoom_absolute(grafton_visca::units::Normalized(0.5))?;
        Ok(())
    };
}

#[cfg(feature = "tokio")]
#[tokio::test]
async fn test_async_wrapper_api() {
    use grafton_visca::r#async::{Camera, ZoomOps};

    // This test just verifies the API compiles correctly
    // In a real test, you'd use a mock transport

    // The async wrapper should expose async methods
    #[allow(dead_code)]
    async fn example(camera: &Camera) -> Result<(), grafton_visca::Error> {
        camera.zoom_stop().await?;
        camera.zoom_in().await?;
        camera.zoom_out().await?;
        camera
            .zoom_absolute(grafton_visca::units::Normalized(0.5))
            .await?;
        Ok(())
    }
}

#[test]
fn test_wrapper_creation() {
    // This would normally use a real transport
    // Here we just test the type system

    #[allow(dead_code)]
    fn create_blocking_wrapper<
        T: grafton_visca::transport::core::BlockingTransport + Send + Sync + 'static,
    >(
        transport: T,
    ) where
        for<'a> T::SendFut<'a>: Send,
        for<'a> T::RecvFut<'a>: Send,
    {
        let camera = grafton_visca::Camera::new(transport);

        // Create blocking wrapper
        let _blocking = camera.blocking();
    }

    #[allow(dead_code)]
    fn create_async_wrapper<T: grafton_visca::transport::core::Transport + Send + Sync + 'static>(
        transport: T,
    ) where
        for<'a> T::SendFut<'a>: Send,
        for<'a> T::RecvFut<'a>: Send,
    {
        let camera = grafton_visca::Camera::new(transport);

        // Create async wrapper
        let _async = camera.r#async();
    }
}
