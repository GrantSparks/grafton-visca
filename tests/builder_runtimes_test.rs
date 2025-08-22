//! Tests for CameraBuilder runtime support

#[cfg(all(feature = "async", feature = "rt-tokio"))]
#[test]
fn test_tokio_builder() {
    use grafton_visca::camera::CameraBuilder;
    use grafton_visca::TokioExecutor;

    // This should compile and work within a tokio runtime
    let rt = tokio::runtime::Runtime::new().unwrap();
    rt.block_on(async {
        let builder = CameraBuilder::tokio().unwrap();
        // Verify we get the right type
        let _: CameraBuilder<TokioExecutor> = builder;
    });
}

#[cfg(all(feature = "async", feature = "rt-async-std"))]
#[test]
fn test_async_std_builder() {
    use grafton_visca::camera::CameraBuilder;
    use grafton_visca::AsyncStdExecutor;

    // This should compile - async-std doesn't require runtime setup
    let builder = CameraBuilder::async_std();
    // Verify we get the right type
    let _: CameraBuilder<AsyncStdExecutor> = builder;
}

#[cfg(all(feature = "async", feature = "rt-smol"))]
#[test]
fn test_smol_builder() {
    use grafton_visca::camera::CameraBuilder;
    use grafton_visca::SmolExecutor;

    // This should compile - smol doesn't require runtime setup
    let builder = CameraBuilder::smol();
    // Verify we get the right type
    let _: CameraBuilder<SmolExecutor> = builder;
}

#[cfg(all(
    feature = "async",
    feature = "rt-tokio",
    feature = "rt-async-std",
    feature = "rt-smol"
))]
#[test]
fn test_all_builders_compile() {
    use grafton_visca::camera::CameraBuilder;

    // Test that all builder methods exist and compile
    let rt = tokio::runtime::Runtime::new().unwrap();
    rt.block_on(async {
        let _ = CameraBuilder::tokio().unwrap();
    });

    let _ = CameraBuilder::async_std();
    let _ = CameraBuilder::smol();
}
