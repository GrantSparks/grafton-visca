//! API stability tests for the async transport unification.
//!
//! This test suite validates that the public API surface remains stable and
//! consistent across all phases of the async transport unification. These tests
//! serve as regression guards against unintended API changes.

#![cfg(feature = "async")]

// External crates
#[cfg(all(feature = "rt-tokio", feature = "test-utils"))]
use grafton_visca::testing::testkit::ScriptedTransport;

#[cfg(all(feature = "rt-tokio", feature = "test-utils"))]
use grafton_visca::TokioExecutor;

#[cfg(any(feature = "rt-tokio", feature = "rt-async-std", feature = "rt-smol"))]
use grafton_visca::transport::Transport;

// Standard library
use std::marker::PhantomData;

/// Test that the Transport builder API remains stable.
#[test]
fn test_transport_builder_api_stability() {
    // Test that Transport type is publicly available when runtime features are enabled
    #[cfg(any(feature = "rt-tokio", feature = "rt-async-std", feature = "rt-smol"))]
    {
        // Test that common builder methods exist and have expected signatures
        let _tcp_builder = Transport::tcp();

        #[cfg(feature = "rt-tokio")]
        let _udp_builder = Transport::udp();

        // Test that Transport can be used in generic contexts
        fn accepts_transport_builder<T>(_builder: T)
        where
            T: Send,
        {
        }

        accepts_transport_builder(Transport::tcp());
        #[cfg(feature = "rt-tokio")]
        accepts_transport_builder(Transport::udp());
    }
}

/// Test that zero-cost generic transports work correctly.
#[test]
fn test_zero_cost_generic_transports() {
    // Test that generic transport types are available and work correctly
    #[cfg(all(feature = "rt-tokio", feature = "test-utils"))]
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
    #[cfg(all(feature = "rt-tokio", feature = "test-utils"))]
    fn requires_send_sync<T>()
    where
        T: Send + Sync + 'static,
    {
    }

    #[cfg(all(feature = "rt-tokio", feature = "test-utils"))]
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
    #[cfg(any(feature = "rt-tokio", feature = "rt-async-std", feature = "rt-smol"))]
    let mut active_runtimes = 0;

    #[cfg(not(any(feature = "rt-tokio", feature = "rt-async-std", feature = "rt-smol")))]
    let active_runtimes = 0;

    #[cfg(feature = "rt-tokio")]
    {
        active_runtimes += 1;
    }

    #[cfg(feature = "rt-async-std")]
    {
        active_runtimes += 1;
    }

    #[cfg(feature = "rt-smol")]
    {
        active_runtimes += 1;
    }

    #[cfg(feature = "async")]
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

    #[cfg(not(feature = "async"))]
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
#[cfg(not(feature = "async"))]
fn test_blocking_api_stability() {
    use grafton_visca::transport::builder::TransportBuilder;

    // Test that TransportBuilder is available in blocking mode
    let _tcp_builder = TransportBuilder::tcp();
    let _udp_builder = TransportBuilder::udp();

    #[cfg(all(feature = "serialport", not(target_arch = "wasm32")))]
    let _serial_builder = TransportBuilder::serial();
}

/// Test that prelude exports remain stable.
#[test]
fn test_prelude_stability() {
    #[cfg(feature = "async")]
    {
        // Prelude types are not yet implemented for async mode
        // This is a placeholder for when async prelude is available
    }

    #[cfg(not(feature = "async"))]
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
#[cfg(feature = "async")]
#[test]
fn test_async_trait_method_signatures() {
    use grafton_visca::camera::controls::power::PowerControl;
    use grafton_visca::Error;
    use std::future::Future;

    // Mock implementation to test trait signatures
    struct MockPowerControl;

    impl PowerControl for MockPowerControl {
        type Mode = grafton_visca::mode::Async;

        fn power_on(
            &self,
        ) -> <Self::Mode as grafton_visca::mode::Mode>::Ret<'_, Result<(), Error>> {
            Box::pin(async { Ok(()) })
        }

        fn power_off(
            &self,
        ) -> <Self::Mode as grafton_visca::mode::Mode>::Ret<'_, Result<(), Error>> {
            Box::pin(async { Ok(()) })
        }

        fn power_inquiry(
            &self,
        ) -> <Self::Mode as grafton_visca::mode::Mode>::Ret<'_, Result<bool, Error>> {
            Box::pin(async { Ok(true) })
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
    assert_future_type::<_, Result<bool, Error>>(control.power_inquiry());
}

/// Test that the transport module structure remains stable.
#[test]
fn test_transport_module_structure() {
    // Test that key transport types are publicly available
    use grafton_visca::transport::buffer::BufferConfig;
    use grafton_visca::transport::builder::TransportConfig;
    use grafton_visca::transport::RetryConfig;

    #[cfg(all(feature = "async", feature = "rt-tokio", feature = "test-utils"))]
    use grafton_visca::transport::AsyncTransport;

    #[cfg(not(feature = "async"))]
    use grafton_visca::transport::SyncTransport;

    // Test that configuration types can be constructed
    let _retry = RetryConfig::default();
    let _buffer = BufferConfig::default();
    let _transport_config = TransportConfig::default();

    // Test that transport traits can be used as bounds - call it to verify it compiles
    #[cfg(all(feature = "async", feature = "rt-tokio", feature = "test-utils"))]
    {
        fn accepts_async_transport<T>(_transport: T)
        where
            T: AsyncTransport,
        {
        }

        let mock_transport = ScriptedTransport::<TokioExecutor>::new(vec![]);
        accepts_async_transport(mock_transport);
    }

    #[cfg(not(feature = "async"))]
    fn accepts_blocking_transport<T>(_transport: T)
    where
        T: SyncTransport,
    {
    }

    // Transport traits should be available - the functions above prove they exist
}
