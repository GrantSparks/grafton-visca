//! Public API contract tests for the 1.0 surface.
//!
//! These tests intentionally assert stable behavior and exported entry points.
//! Changes here should reflect an explicit public contract decision.

use std::{marker::PhantomData, num::NonZeroUsize, time::Duration};

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
    #[cfg(not(feature = "mode-async"))]
    {
        use grafton_visca::{
            transport::{NetTransportBuilder, Transport},
            Error,
        };

        let tcp_missing_address = NetTransportBuilder::tcp()
            .build_blocking()
            .expect_err("missing address should be rejected without I/O");
        assert!(matches!(
            tcp_missing_address,
            Error::InvalidParameter {
                parameter: "address",
                ..
            }
        ));

        let udp_missing_address = Transport::udp()
            .build_blocking()
            .expect_err("missing address should be rejected without I/O");
        assert!(matches!(
            udp_missing_address,
            Error::InvalidParameter {
                parameter: "address",
                ..
            }
        ));

        let _tcp_builder = Transport::tcp()
            .address("127.0.0.1:5678")
            .connect_timeout(Duration::from_millis(250))
            .read_timeout(Duration::from_millis(500))
            .write_timeout(Duration::from_millis(500));
        let _udp_builder = NetTransportBuilder::udp()
            .address("127.0.0.1:1259")
            .max_retries(5)
            .retry_delay(Duration::from_millis(25));
    }

    #[cfg(feature = "mode-async")]
    {
        use grafton_visca::{
            camera::{CameraConfig, Connect},
            profiles::PtzOpticsG2,
            transport::{TcpKeepaliveConfig, TransportConfig},
        };

        let _connect_builder = Connect::builder()
            .tcp("127.0.0.1")
            .with_default_port()
            .udp("127.0.0.1:1259");

        let _camera_config = CameraConfig::<PtzOpticsG2>::new()
            .tcp()
            .address("127.0.0.1")
            .transport_config(TransportConfig {
                tcp_keepalive: Some(TcpKeepaliveConfig::for_visca_long_lived_tcp()),
                ..TransportConfig::default()
            });
    }
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
    use grafton_visca::{
        ExposureControl, FocusControl, ImageProcessingControl, InquiryControl, PanTiltControl,
        PowerControl, PresetsControl, WhiteBalanceControl, ZoomControl,
    };

    #[allow(dead_code)]
    fn requires_core_camera_controls<T>()
    where
        T: PowerControl
            + ZoomControl
            + FocusControl
            + PanTiltControl
            + PresetsControl
            + InquiryControl
            + ExposureControl
            + WhiteBalanceControl
            + ImageProcessingControl,
    {
    }
}

/// Test that runtime feature detection works correctly.
#[test]
fn test_runtime_feature_detection_stability() {
    // Count active runtime features
    #[cfg(any(feature = "runtime-tokio", feature = "runtime-smol"))]
    let mut active_runtimes = 0;

    #[cfg(not(any(feature = "runtime-tokio", feature = "runtime-smol")))]
    let active_runtimes = 0;

    #[cfg(feature = "runtime-tokio")]
    {
        active_runtimes += 1;
    }

    #[cfg(feature = "runtime-smol")]
    {
        active_runtimes += 1;
    }

    #[cfg(feature = "mode-async")]
    {
        assert!(
            active_runtimes <= 2,
            "The 1.0 async contract currently exposes at most Tokio and smol runtimes, got {}",
            active_runtimes
        );

        #[cfg(feature = "runtime-tokio")]
        {
            let _: PhantomData<grafton_visca::runtime::TokioRuntime> = PhantomData;
        }

        #[cfg(feature = "runtime-smol")]
        {
            let _: PhantomData<grafton_visca::runtime::SmolRuntime> = PhantomData;
        }
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
        use grafton_visca::prelude::r#async::{AwaitConfig, GenericVisca, PtzOpticsG2, SonyFR7};

        let _await_config: PhantomData<AwaitConfig> = PhantomData;
        let _phantom_g: PhantomData<GenericVisca> = PhantomData;
        let _phantom_p: PhantomData<PtzOpticsG2> = PhantomData;
        let _phantom_s: PhantomData<SonyFR7> = PhantomData;

        #[cfg(feature = "runtime-tokio")]
        {
            let _runtime: PhantomData<grafton_visca::prelude::r#async::TokioRuntime> = PhantomData;
        }

        #[cfg(feature = "runtime-smol")]
        {
            let _runtime: PhantomData<grafton_visca::prelude::r#async::SmolRuntime> = PhantomData;
        }
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
    use grafton_visca::{CameraId, Error, PresetNumber, ViscaSocket};

    // Test Error enum remains publicly accessible
    assert!(matches!(Error::from_code(0x02), Error::SyntaxError));
    assert!(matches!(Error::from_code(0x03), Error::CommandBufferFull));

    // Test PresetNumber construction and validation
    let preset = PresetNumber::new(255).expect("Preset 255 should be valid");
    assert_eq!(preset.value(), 255);
    assert_eq!(PresetNumber::MIN, 0);
    assert_eq!(PresetNumber::MAX, 255);

    // Test CameraId construction
    let camera_id = CameraId::new(1).expect("Camera ID 1 should be valid");
    assert_eq!(camera_id.id(), 1);
    assert_eq!(camera_id.to_address_byte(), 0x81);
    assert!(CameraId::new(0).is_err());
    assert!(CameraId::new(9).is_err());
    assert_eq!(CameraId::BROADCAST.to_address_byte(), 0x88);
    assert!(CameraId::BROADCAST.is_broadcast());

    assert_eq!(ViscaSocket::S1.as_index(), 0);
    assert_eq!(ViscaSocket::S2.as_index(), 1);
    assert_eq!(ViscaSocket::from_protocol_byte(1), Some(ViscaSocket::S1));
    assert_eq!(ViscaSocket::from_protocol_byte(2), Some(ViscaSocket::S2));
    assert_eq!(ViscaSocket::from_protocol_byte(0), None);
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

    assert_eq!(PtzOpticsG2::MODEL_NAME, "PtzOptics G2");
    assert_eq!(PtzOpticsG2::DEFAULT_CAMERA_ID, 1);
    assert_eq!(PtzOpticsG2::DEFAULT_TCP_PORT, 5678);
    assert_eq!(PtzOpticsG2::DEFAULT_UDP_PORT, 1259);
    assert_eq!(GenericVisca::DEFAULT_TCP_PORT, 5678);
    assert_eq!(GenericVisca::DEFAULT_UDP_PORT, 1259);
    assert_eq!(SonyFR7::DEFAULT_CAMERA_ID, 1);
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
    use grafton_visca::transport::{
        buffer::BufferConfig,
        builder::{TransportConfig, DEFAULT_MAX_PENDING_QUEUE_DEPTH},
        AddressingMode, BackoffStrategy, RetryAttempt, RetryConfig, TcpKeepaliveConfig,
    };

    #[cfg(all(
        feature = "mode-async",
        feature = "runtime-tokio",
        feature = "test-utils"
    ))]
    use grafton_visca::transport::AsyncTransport;

    // Test that configuration types have stable defaults.
    let retry = RetryConfig::default();
    assert_eq!(retry.max_retries, 3);
    assert_eq!(retry.base_retry_delay, Duration::from_millis(100));
    assert_eq!(retry.max_retry_duration, Duration::from_secs(10));
    assert_eq!(retry.backoff_strategy, BackoffStrategy::Exponential);
    assert_eq!(
        retry.calculate_delay(RetryAttempt::FIRST, None),
        Duration::from_millis(100)
    );
    assert_eq!(
        retry.calculate_delay(
            RetryAttempt::new(3).expect("retry attempt 3 is valid"),
            None
        ),
        Duration::from_millis(400)
    );
    assert_eq!(
        RetryAttempt::from_retries_done(0),
        Some(RetryAttempt::FIRST)
    );
    assert_eq!(
        RetryAttempt::from_retries_done(2)
            .expect("third attempt should be representable")
            .get(),
        3
    );
    assert_eq!(RetryAttempt::new(0), None);

    let buffer = BufferConfig::default();
    assert_eq!(buffer.recv_buffer_size, 128);
    assert_eq!(buffer.send_buffer_size, 128);
    assert_eq!(buffer.max_buffer_size, 8192);
    assert_eq!(BufferConfig::for_udp().recv_buffer_size, 1024);
    assert_eq!(BufferConfig::for_raw_ip().send_buffer_size, 256);

    let keepalive = TcpKeepaliveConfig::for_visca_long_lived_tcp();
    assert_eq!(keepalive.idle, Duration::from_secs(10));
    assert_eq!(keepalive.interval, Some(Duration::from_secs(10)));

    let transport_config = TransportConfig::default();
    assert_eq!(transport_config.connect_timeout, Duration::from_secs(5));
    assert_eq!(transport_config.read_timeout, Duration::from_secs(5));
    assert_eq!(transport_config.write_timeout, Duration::from_secs(5));
    assert_eq!(transport_config.addressing, AddressingMode::Ip);
    assert_eq!(transport_config.tcp_nodelay, Some(true));
    assert_eq!(transport_config.tcp_keepalive, Some(keepalive));
    assert_eq!(
        transport_config.max_pending_queue_depth,
        NonZeroUsize::new(DEFAULT_MAX_PENDING_QUEUE_DEPTH)
            .expect("default queue depth is non-zero")
    );

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

    // Transport configuration types should be available.
}

#[test]
fn test_raw_command_extension_contract() {
    use grafton_visca::{command::ViscaCommand, timeout::CommandCategory, CameraId, Error};

    struct CustomCommand;

    impl ViscaCommand for CustomCommand {
        type Response = ();

        const MAX_SIZE: usize = 6;
        const TIMEOUT_CATEGORY: CommandCategory = CommandCategory::Quick;

        fn write_into(&self, camera_id: CameraId, buffer: &mut [u8]) -> Result<usize, Error> {
            if buffer.len() < Self::MAX_SIZE {
                return Err(Error::BufferTooSmall {
                    required: Self::MAX_SIZE,
                    actual: buffer.len(),
                });
            }

            buffer[..Self::MAX_SIZE].copy_from_slice(&[
                camera_id.to_address_byte(),
                0x01,
                0x04,
                0x00,
                0x02,
                0xFF,
            ]);
            Ok(Self::MAX_SIZE)
        }

        fn response_kind(&self) -> Option<grafton_visca::command::InquiryKind> {
            None
        }
    }

    let command = CustomCommand;
    let encoded = command
        .to_fixed_bytes::<6>(CameraId::CAMERA_1)
        .expect("custom command should encode");
    assert_eq!(encoded.as_slice(), &[0x81, 0x01, 0x04, 0x00, 0x02, 0xFF]);
    assert_eq!(
        command.command_kind(),
        grafton_visca::command::CommandKind::Command
    );
}

#[test]
fn test_compile_time_api_contracts() {
    let cases = trybuild::TestCases::new();
    cases.pass("tests/api_contract/pass/*.rs");
    cases.compile_fail("tests/api_contract/fail/*.rs");
}
