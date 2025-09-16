//! Tests for CameraBuilder runtime support

#[cfg(all(feature = "mode-async", feature = "runtime-tokio"))]
#[test]
fn test_tokio_builder() {
    use grafton_visca::camera::CameraBuilder;
    use grafton_visca::runtime_trait::TokioRuntime;
    // This should compile and work within a tokio runtime
    let rt = tokio::runtime::Runtime::new().unwrap();
    rt.block_on(async {
        let runtime = TokioRuntime::from_current().unwrap();
        let builder = CameraBuilder::with_executor(runtime);
        // Verify we get the right type
        let _: CameraBuilder<TokioRuntime> = builder;
    });
}

#[cfg(all(feature = "mode-async", feature = "runtime-async-std"))]
#[test]
fn test_async_std_builder() {
    use grafton_visca::camera::CameraBuilder;
    use grafton_visca::runtime_trait::AsyncStdRuntime;
    // This should compile - async-std doesn't require runtime setup
    let runtime = AsyncStdRuntime::new();
    let builder = CameraBuilder::with_executor(runtime);
    // Verify we get the right type
    let _: CameraBuilder<AsyncStdRuntime> = builder;
}

#[cfg(all(feature = "mode-async", feature = "runtime-smol"))]
#[test]
fn test_smol_builder() {
    use grafton_visca::camera::CameraBuilder;
    use grafton_visca::runtime_trait::SmolRuntime;
    // This should compile - smol doesn't require runtime setup
    let runtime = SmolRuntime::new();
    let builder = CameraBuilder::with_executor(runtime);
    // Verify we get the right type
    let _: CameraBuilder<SmolRuntime> = builder;
}

#[cfg(all(
    feature = "mode-async",
    feature = "runtime-tokio",
    feature = "runtime-async-std",
    feature = "runtime-smol"
))]
#[test]
fn test_all_builders_compile() {
    use grafton_visca::camera::CameraBuilder;

    // Test that all builder methods exist and compile
    use grafton_visca::runtime_trait::{AsyncStdRuntime, SmolRuntime, TokioRuntime};

    let rt = tokio::runtime::Runtime::new().unwrap();
    rt.block_on(async {
        let runtime = TokioRuntime::from_current().unwrap();
        let _ = CameraBuilder::with_executor(runtime);
    });

    let async_std_runtime = AsyncStdRuntime::new();
    let _ = CameraBuilder::with_executor(async_std_runtime);

    let smol_runtime = SmolRuntime::new();
    let _ = CameraBuilder::with_executor(smol_runtime);
}
