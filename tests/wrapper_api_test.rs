//! Test the new wrapper API to ensure it compiles and works correctly.

#[test]
fn test_blocking_wrapper_api() {
    use grafton_visca::blocking::ZoomOps;
    use grafton_visca::capabilities::Profile;
    use grafton_visca::transport::Transport;
    use grafton_visca::Camera;

    // This test just verifies the API compiles correctly
    // In a real test, you'd use a mock transport

    // The blocking wrapper should expose methods without _blocking suffix
    fn _example<P: Profile, T: Transport + Send + Sync + 'static>(
        camera: &Camera<P, T>,
    ) -> Result<(), grafton_visca::Error>
    where
        T::Error: Into<grafton_visca::Error> + Send,
        for<'a> T::SendFut<'a>: Send,
        for<'a> T::RecvFut<'a>: Send,
    {
        camera.zoom_stop()?;
        camera.zoom_in()?;
        camera.zoom_out()?;
        camera.zoom_absolute(grafton_visca::units::Normalized(0.5))?;
        Ok(())
    }

    // Just test that the function compiles, don't try to reference it
    // as that would require a concrete Transport type
}

#[cfg(feature = "rt-tokio")]
#[tokio::test]
async fn test_async_wrapper_api() {
    use grafton_visca::capabilities::Profile;
    use grafton_visca::r#async::ZoomOps;
    use grafton_visca::transport::Transport;
    use grafton_visca::Camera;

    // This test just verifies the API compiles correctly
    // In a real test, you'd use a mock transport

    // The async wrapper should expose async methods
    #[allow(dead_code)]
    async fn example<P: Profile, T: Transport + Send + Sync + 'static>(
        camera: &Camera<P, T>,
    ) -> Result<(), grafton_visca::Error>
    where
        T::Error: Into<grafton_visca::Error> + Send,
        for<'a> T::SendFut<'a>: Send,
        for<'a> T::RecvFut<'a>: Send,
    {
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
    // Here we just test the type system compiles
    use grafton_visca::camera::profiles::PTZOpticsG2;
    use grafton_visca::capabilities::Profile;
    use grafton_visca::transport::Transport;
    use grafton_visca::Camera;

    // Test that generic camera creation compiles with proper constraints
    // Note: These are just type checks, not actual implementations

    // Check that Camera type exists with proper bounds
    fn _check_camera_type<P: Profile, T: Transport + Send + Sync + 'static>()
    where
        T::Error: Into<grafton_visca::Error> + Send,
        for<'a> T::SendFut<'a>: Send,
        for<'a> T::RecvFut<'a>: Send,
    {
        // This function body is never executed, we just check it compiles
        let _: Option<Camera<P, T>> = None;
    }

    // Check that specific camera types exist
    fn _check_specific_types<T: Transport + Send + Sync + 'static>()
    where
        T::Error: Into<grafton_visca::Error> + Send,
        for<'a> T::SendFut<'a>: Send,
        for<'a> T::RecvFut<'a>: Send,
    {
        let _: Option<Camera<PTZOpticsG2, T>> = None;
        let _: Option<Camera<grafton_visca::camera::profiles::SonyFR7, T>> = None;
        let _: Option<Camera<grafton_visca::camera::profiles::GenericVisca, T>> = None;
    }
}
