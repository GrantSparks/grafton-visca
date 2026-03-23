//! Tests for CameraBuilder runtime support

#[cfg(all(feature = "mode-async", feature = "runtime-tokio"))]
#[test]
fn test_tokio_builder() {
    use grafton_visca::camera::CameraBuilder;
    use grafton_visca::runtime::TokioRuntime;
    // This should compile and work within a tokio runtime
    let rt = tokio::runtime::Runtime::new().unwrap();
    rt.block_on(async {
        let runtime = TokioRuntime::from_current().unwrap();
        let builder = CameraBuilder::with_executor(runtime);
        // Verify we get the right type
        let _: CameraBuilder<TokioRuntime> = builder;
    });
}

#[cfg(all(feature = "mode-async", feature = "runtime-smol"))]
#[test]
fn test_smol_builder() {
    use grafton_visca::camera::CameraBuilder;
    use grafton_visca::runtime::SmolRuntime;
    // This should compile - smol doesn't require runtime setup
    let runtime = SmolRuntime::new();
    let builder = CameraBuilder::with_executor(runtime);
    // Verify we get the right type
    let _: CameraBuilder<SmolRuntime> = builder;
}
