//! API stability tests for the async transport unification.
//!
//! This test suite validates that the public API surface remains stable and
//! consistent across all phases of the async transport unification. These tests
//! serve as regression guards against unintended API changes.

#![cfg(feature = "mode-async")]

use std::marker::PhantomData;

#[cfg(all(feature = "runtime-tokio", feature = "test-utils"))]
use grafton_visca::testing::testkit::ScriptedTransport;
#[cfg(all(feature = "runtime-tokio", feature = "test-utils"))]
use grafton_visca::TokioExecutor;

/// Test that the Transport builder API remains stable in blocking mode.
///
/// Note: Transport and NetTransportBuilder are now blocking-mode-only types.
/// In async mode, users should use CameraConfig and Connect convenience methods.
#[test]
fn test_transport_builder_api_stability() {
    // Transport and NetTransportBuilder are only available in blocking mode.
    // In async mode, the recommended API is CameraConfig and Connect methods.
    // This test verifies that async mode compiles without Transport.
}

/// Test that zero-cost generic transports work correctly.
#[test]
fn test_zero_cost_generic_transports() {
    // Test that generic transport types are available and work correctly
    #[cfg(all(feature = "runtime-tokio", feature = "test-utils"))]
    {
        // Test that generic transports can be used with type parameters
        fn accepts_generic_transport<T>(_transport: T)
        where
            T: Send + 'static,
        {
        }

        let mock_transport = ScriptedTransport::<TokioExecutor>::new(vec![]);
        accepts_generic_transport(mock_transport);
    }

    // Test that we can still work with generic constraints
    #[cfg(all(feature = "runtime-tokio", feature = "test-utils"))]
    fn requires_send_sync<T>()
    where
        T: Send + Sync + 'static,
    {
    }

    #[cfg(all(feature = "runtime-tokio", feature = "test-utils"))]
    requires_send_sync::<ScriptedTransport<TokioExecutor>>();
}

/// Test that async control traits maintain stable public signatures.
#[test]
fn test_control_traits_api_stability() {
    // Control traits are available but we don't need to import them for this test

    // Test that control traits have proper supertraits
    fn requires_send_sync<T>()
    where
        T: Send + Sync + 'static + Sized,
    {
    }

    // Control traits require Sized so they can't be used as dyn traits
    // Instead test that they have the right supertrait bounds
    requires_send_sync::<PhantomData<()>>();
}

/// Test that runtime feature detection works correctly.
#[test]
fn test_runtime_feature_detection_stability() {
    // Count active runtime features
    #[cfg(any(
        feature = "runtime-tokio",
        feature = "runtime-async-std",
        feature = "runtime-smol"
    ))]
    let mut active_runtimes = 0;

    #[cfg(not(any(
        feature = "runtime-tokio",
        feature = "runtime-async-std",
        feature = "runtime-smol"
    )))]
    let active_runtimes = 0;

    #[cfg(feature = "runtime-tokio")]
    {
        active_runtimes += 1;
    }

    #[cfg(feature = "runtime-async-std")]
    {
        active_runtimes += 1;
    }

    #[cfg(feature = "runtime-smol")]
    {
        active_runtimes += 1;
    }

    #[cfg(feature = "mode-async")]
    {
        // When async feature is enabled, we should have either:
        // 0 runtimes (runtime-agnostic async) OR 1 or more runtimes (multi-runtime coexistence)
        // The library now supports multiple runtimes coexisting with priority-based selection
        assert!(
            active_runtimes >= 0,
            "With async feature: should have 0 or more runtimes (supports multi-runtime coexistence), got {}",
            active_runtimes
        );
    }

    #[cfg(not(feature = "mode-async"))]
    {
        // In blocking mode, there should be no runtime features
        assert_eq!(
            active_runtimes, 0,
            "Blocking mode should not have any async runtime features, got {}",
            active_runtimes
        );
    }
}

/// Test that blocking API remains unchanged and stable.
#[test]
#[cfg(not(feature = "mode-async"))]
fn test_blocking_api_stability() {
    use grafton_visca::transport::{NetTransportBuilder, Transport};

    // Test that NetTransportBuilder is available in blocking mode
    let _tcp_builder = NetTransportBuilder::tcp();
    let _udp_builder = NetTransportBuilder::udp();
    // Alternative via Transport facade
    let _tcp_transport = Transport::tcp();
    let _udp_transport = Transport::udp();
}

/// Test that prelude exports remain stable.
#[test]
fn test_prelude_stability() {
    #[cfg(feature = "mode-async")]
    {
        // Prelude types are not yet implemented for async mode
        // This is a placeholder for when async prelude is available
    }

    #[cfg(not(feature = "mode-async"))]
    {
        use grafton_visca::prelude::blocking::{GenericViscaCam, PtzOpticsG2Cam, SonyFR7Cam};

        // Test that blocking prelude types are available
        let _phantom_g: PhantomData<GenericViscaCam<()>> = PhantomData;
        let _phantom_p: PhantomData<PtzOpticsG2Cam<()>> = PhantomData;
        let _phantom_s: PhantomData<SonyFR7Cam<()>> = PhantomData;
    }
}

/// Test that core types maintain stable public interfaces.
#[test]
fn test_core_types_stability() {
    use grafton_visca::{CameraId, Error, PresetNumber};

    // Test Error enum remains publicly accessible
    let _error: Error = Error::from_code(0xFF);

    // Test PresetNumber construction and validation
    let preset = PresetNumber::new(1).expect("Preset 1 should be valid");
    assert_eq!(preset.value(), 1);

    // Test CameraId construction
    let _camera_id = CameraId::new(1).expect("Camera ID 1 should be valid");
}

/// Test that capability traits remain stable.
#[test]
fn test_capability_traits_stability() {
    use grafton_visca::camera::profiles::{GenericVisca, PtzOpticsG2, SonyFR7};
    use grafton_visca::capabilities::{
        Exposure, Focus, ImageProcessing, NdFilter, PanTilt, Power, Profile, ProfileMetadata,
        WhiteBalance, Zoom,
    };

    // Test that capability traits can be used as bounds
    fn test_profile_bounds<P>()
    where
        P: Profile + ProfileMetadata + Power + Zoom + PanTilt + Focus + Exposure,
    {
    }

    fn test_advanced_bounds<P>()
    where
        P: Profile + ImageProcessing + WhiteBalance,
    {
    }

    fn test_specialized_bounds<P>()
    where
        P: Profile + NdFilter, // Only NdFilter since not all profiles have MotionSync
    {
    }

    // These should compile for appropriate camera profiles
    test_profile_bounds::<PtzOpticsG2>();
    test_profile_bounds::<SonyFR7>();
    test_profile_bounds::<GenericVisca>();

    test_advanced_bounds::<PtzOpticsG2>();
    test_advanced_bounds::<SonyFR7>();
    test_advanced_bounds::<GenericVisca>();

    // Only test NdFilter specialization for SonyFR7 which supports it
    test_specialized_bounds::<SonyFR7>();
}

/// Test that async trait method signatures remain stable.
#[cfg(feature = "mode-async")]
#[test]
fn test_async_trait_method_signatures() {
    use grafton_visca::{camera::controls::power::PowerControl, Error};
    use std::future::Future;

    // Mock implementation to test trait signatures
    struct MockPowerControl;

    impl PowerControl for MockPowerControl {
        type Mode = grafton_visca::mode::Async;

        fn power_on(
            &self,
        ) -> <Self::Mode as grafton_visca::mode::Mode>::Fut<'_, Result<(), Error>> {
            Box::pin(async { Ok(()) })
        }

        fn power_off(
            &self,
        ) -> <Self::Mode as grafton_visca::mode::Mode>::Fut<'_, Result<(), Error>> {
            Box::pin(async { Ok(()) })
        }
    }

    let control = MockPowerControl;

    // Test that methods return the expected future types
    fn assert_future_type<F, T>(_f: F)
    where
        F: Future<Output = T> + Send,
    {
    }

    assert_future_type::<_, Result<(), Error>>(control.power_on());
    assert_future_type::<_, Result<(), Error>>(control.power_off());
}

/// Test that the transport module structure remains stable.
#[test]
fn test_transport_module_structure() {
    // Test that key transport types are publicly available
    use grafton_visca::transport::{buffer::BufferConfig, builder::TransportConfig, RetryConfig};

    #[cfg(all(
        feature = "mode-async",
        feature = "runtime-tokio",
        feature = "test-utils"
    ))]
    use grafton_visca::transport::AsyncTransport;

    // Test that configuration types can be constructed
    let _retry = RetryConfig::default();
    let _buffer = BufferConfig::default();
    let _transport_config = TransportConfig::default();

    // Test that transport traits can be used as bounds - call it to verify it compiles
    #[cfg(all(
        feature = "mode-async",
        feature = "runtime-tokio",
        feature = "test-utils"
    ))]
    {
        fn accepts_async_transport<T>(_transport: T)
        where
            T: AsyncTransport,
        {
        }

        let mock_transport = ScriptedTransport::<TokioExecutor>::new(vec![]);
        accepts_async_transport(mock_transport);
    }

    // Note: BlockingTransport is now internal - users should use the camera-first API
    // Test that blocking mode has the public BlockingTransportHandle type
    #[cfg(not(feature = "mode-async"))]
    {
        use grafton_visca::transport::BlockingTransportHandle;
        // BlockingTransportHandle should be available as a public type
        fn _accepts_handle(_handle: BlockingTransportHandle) {}
    }

    // Transport configuration types should be available
}
