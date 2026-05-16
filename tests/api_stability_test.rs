//! Public API contract tests for the 1.0 surface.
//!
//! These tests intentionally assert stable behavior and exported entry points.
//! Changes here should reflect an explicit public contract decision.

#[path = "common/compile_fail.rs"]
mod compile_fail;

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

        let _tcp_connect_builder = Connect::builder().tcp("127.0.0.1").with_default_port();
        let _udp_connect_builder = Connect::builder().udp("127.0.0.1:1259");

        let _camera_config =
            CameraConfig::<PtzOpticsG2>::tcp("127.0.0.1").transport_config(TransportConfig {
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
    use grafton_visca::camera::controls::{
        color::{
            ColorControl, ColorTemperatureControl, OnePushWhiteBalanceControl, RgbGainControl,
            RgbTuningControl,
        },
        exposure::{
            BacklightCompensationControl, BrightnessControl, ExposureControl, IrisControl,
            WideDynamicRangeControl,
        },
        focus::{
            AutoFocusSensitivityControl, FocusControl, FocusZoneControl, OnePushFocusControl,
            SnapFocusControl,
        },
        image_processing::{
            ContrastControl, GammaControl, HueControl, ImageFlipControl, ImageFlipModeControl,
            ImageMirrorControl, LuminanceControl, NoiseReduction2DControl, NoiseReduction3DControl,
            PictureEffectControl, SaturationControl, SharpnessControl,
        },
        inquiry::{
            AutoFocusSensitivityInquiryControl, BacklightCompensationInquiryControl,
            BrightnessInquiryControl, ColorTemperatureInquiryControl, ContrastInquiryControl,
            ExposureCompensationInquiryControl, FocusNearLimitInquiryControl,
            FocusZoneInquiryControl, GammaInquiryControl, HueInquiryControl,
            ImageFlipInquiryControl, InquiryControl, IrisInquiryControl, LuminanceInquiryControl,
            NoiseReduction2DInquiryControl, NoiseReduction3DInquiryControl,
            NoiseReductionInquiryControl, PictureEffectInquiryControl, RgbGainInquiryControl,
            RgbTuningInquiryControl, SaturationInquiryControl, SharpnessInquiryControl,
            WideDynamicRangeInquiryControl,
        },
        pan_tilt::PanTiltControl,
        power::PowerControl,
        presets::PresetsControl,
        white_balance::{
            AutoTrackingWhiteBalanceControl, AutoWhiteBalanceSensitivityControl,
            WhiteBalanceControl,
        },
        zoom::{DigitalZoomControl, DigitalZoomRangeControl, DirectZoomControl, ZoomControl},
    };

    struct CoreCameraControls<T>(PhantomData<T>);

    impl<T> CoreCameraControls<T> where
        T: PowerControl
            + ZoomControl
            + DirectZoomControl
            + DigitalZoomControl
            + DigitalZoomRangeControl
            + FocusControl
            + OnePushFocusControl
            + SnapFocusControl
            + FocusZoneControl
            + AutoFocusSensitivityControl
            + PanTiltControl
            + PresetsControl
            + InquiryControl
            + BrightnessInquiryControl
            + ContrastInquiryControl
            + SharpnessInquiryControl
            + FocusNearLimitInquiryControl
            + FocusZoneInquiryControl
            + AutoFocusSensitivityInquiryControl
            + IrisInquiryControl
            + ExposureCompensationInquiryControl
            + BacklightCompensationInquiryControl
            + WideDynamicRangeInquiryControl
            + ColorTemperatureInquiryControl
            + RgbGainInquiryControl
            + RgbTuningInquiryControl
            + SaturationInquiryControl
            + HueInquiryControl
            + LuminanceInquiryControl
            + GammaInquiryControl
            + ImageFlipInquiryControl
            + NoiseReductionInquiryControl
            + NoiseReduction2DInquiryControl
            + NoiseReduction3DInquiryControl
            + PictureEffectInquiryControl
            + ExposureControl
            + BrightnessControl
            + IrisControl
            + BacklightCompensationControl
            + WideDynamicRangeControl
            + WhiteBalanceControl
            + OnePushWhiteBalanceControl
            + AutoTrackingWhiteBalanceControl
            + AutoWhiteBalanceSensitivityControl
            + ContrastControl
            + SharpnessControl
            + ImageFlipControl
            + ImageMirrorControl
            + ImageFlipModeControl
            + SaturationControl
            + HueControl
            + LuminanceControl
            + GammaControl
            + NoiseReduction2DControl
            + NoiseReduction3DControl
            + PictureEffectControl
            + ColorControl
            + ColorTemperatureControl
            + RgbGainControl
            + RgbTuningControl
    {
    }

    let _ = CoreCameraControls::<()>(PhantomData);
}

/// Test that public value wrappers keep their validation and conversion semantics.
#[test]
fn test_public_value_type_contracts() {
    use grafton_visca::{
        types::{ColorTemp, FocusPosition, PanSpeed, TiltSpeed, ZoomPosition, ZoomSpeed},
        Error, SpeedLevel,
    };

    assert_eq!(PanSpeed::ZERO.value(), 0);
    assert_eq!(TiltSpeed::ZERO.value(), 0);
    assert_eq!(ZoomSpeed::ZERO.value(), 0);
    assert_eq!(PanSpeed::from(SpeedLevel::Medium).value(), 12);
    assert_eq!(TiltSpeed::from(SpeedLevel::Medium).value(), 10);
    assert_eq!(ZoomSpeed::from(SpeedLevel::Fastest).value(), 7);

    let pan_too_fast = PanSpeed::new(25).expect_err("pan speed 25 exceeds the VISCA range");
    assert!(matches!(
        pan_too_fast,
        Error::ParameterOutOfRange {
            parameter: "PanSpeed",
            value: 25,
            min: 0,
            max: 24,
        }
    ));

    let zoom_half = ZoomPosition::try_from(0.5_f32)
        .expect("normalized zoom 0.5 should map into the stable VISCA range");
    assert_eq!(zoom_half.value(), 0x4000);
    assert_eq!(f32::from(ZoomPosition::MIN), 0.0);
    assert_eq!(f32::from(ZoomPosition::MAX), 1.0);
    assert!(ZoomPosition::try_from(-0.1_f32).is_err());
    assert!(ZoomPosition::try_from(1.1_f32).is_err());

    let focus = FocusPosition::new(0x1234);
    assert_eq!(focus.value(), 0x1234);
    assert_eq!(u16::from(focus), 0x1234);

    let color_temp = ColorTemp::from_kelvin(5600).expect("5600K is in the public range");
    assert_eq!(color_temp.value(), 31);
    assert_eq!(color_temp.to_kelvin(), 5600);
    assert!(ColorTemp::from_kelvin(2400).is_err());
    assert!(ColorTemp::from_kelvin(8100).is_err());
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
        Capabilities, Exposure, Focus, HasNdFilter, ImageProcessing, InquirySupport, PanTilt,
        Power, Profile, ProfileMetadata, SupportsTcp, SupportsUdp, WhiteBalance, Zoom,
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
        P: Profile + HasNdFilter,
    {
    }

    // These should compile for appropriate camera profiles
    test_profile_bounds::<PtzOpticsG2>();
    test_profile_bounds::<SonyFR7>();
    test_profile_bounds::<GenericVisca>();

    test_advanced_bounds::<PtzOpticsG2>();
    test_advanced_bounds::<SonyFR7>();
    test_advanced_bounds::<GenericVisca>();

    // Only test typed ND filter support for SonyFR7.
    test_specialized_bounds::<SonyFR7>();

    assert_eq!(PtzOpticsG2::MODEL_NAME, "PtzOptics G2");
    assert_eq!(PtzOpticsG2::DEFAULT_CAMERA_ID, 1);
    assert_eq!(<PtzOpticsG2 as SupportsTcp>::DEFAULT_TCP_PORT, 5678);
    assert_eq!(<PtzOpticsG2 as SupportsUdp>::DEFAULT_UDP_PORT, 1259);
    assert_eq!(<GenericVisca as SupportsTcp>::DEFAULT_TCP_PORT, 5678);
    assert_eq!(<GenericVisca as SupportsUdp>::DEFAULT_UDP_PORT, 1259);
    assert_eq!(SonyFR7::DEFAULT_CAMERA_ID, 1);

    let g2_caps = Capabilities::from_profile::<PtzOpticsG2>();
    assert_eq!(g2_caps.model_name, "PtzOptics G2");
    assert!(g2_caps.has_basic_features());
    assert!(g2_caps.has_full_inquiry_support());
    assert_eq!(g2_caps.inquiry_support, InquirySupport::Full);
    assert!(!g2_caps.has_motion_sync);
    assert_eq!(g2_caps.max_motion_sync_speed, None);
    assert!(!g2_caps.has_nd_filter);
    assert!(g2_caps.has_iris_control);
    assert!(g2_caps.has_picture_effect);

    let fr7_caps = Capabilities::from_profile::<SonyFR7>();
    assert_eq!(fr7_caps.model_name, "Sony FR7");
    assert!(fr7_caps.has_basic_features());
    assert!(fr7_caps.has_advanced_features());
    assert!(fr7_caps.has_nd_filter);
    assert!(fr7_caps.supports_wake_on_lan);
    assert_eq!(fr7_caps.default_tcp_port, None);
    assert_eq!(fr7_caps.default_udp_port, Some(52381));
}

#[test]
#[cfg(feature = "serde")]
fn test_serde_public_contract() {
    use grafton_visca::{
        camera::{config::TransportOptions, profiles::ProfileId},
        PresetNumber, SpeedLevel,
    };

    let transport = TransportOptions::Tcp {
        address: "192.168.0.110:5678".to_string(),
    };
    let transport_json =
        serde_json::to_value(&transport).expect("TransportOptions should serialize");
    assert_eq!(
        transport_json,
        serde_json::json!({
            "type": "TCP",
            "address": "192.168.0.110:5678"
        })
    );

    assert_eq!(
        serde_json::to_string(&ProfileId::PtzOpticsG2).expect("ProfileId should serialize"),
        "\"ptz-optics-g2\""
    );
    assert_eq!(
        serde_json::to_string(&SpeedLevel::Fast).expect("SpeedLevel should serialize"),
        "\"fast\""
    );

    let preset: PresetNumber =
        serde_json::from_str("42").expect("PresetNumber should deserialize from a number");
    assert_eq!(preset.value(), 42);
}

#[test]
#[cfg(feature = "schemars")]
fn test_schemars_public_contract() {
    use grafton_visca::{
        camera::{config::TransportOptions, profiles::ProfileId},
        capabilities::Capabilities,
        PresetNumber,
    };
    use schemars::schema_for;

    let _transport_options_schema = schema_for!(TransportOptions);
    let _profile_id_schema = schema_for!(ProfileId);
    let _capabilities_schema = schema_for!(Capabilities);
    let _preset_number_schema = schema_for!(PresetNumber);
}

#[test]
#[cfg(feature = "ts-rs")]
fn test_ts_rs_public_contract() {
    use grafton_visca::{
        camera::{config::TransportOptions, profiles::ProfileId},
        PanTiltDirection, PresetNumber, SpeedLevel,
    };
    use ts_rs::{Config, TS};

    let config = Config::default();

    let transport_ts = TransportOptions::export_to_string(&config)
        .expect("TransportOptions should export a TypeScript definition");
    assert!(transport_ts.contains("type"));

    assert!(!ProfileId::export_to_string(&config)
        .expect("ProfileId should export a TypeScript definition")
        .is_empty());
    assert!(!SpeedLevel::export_to_string(&config)
        .expect("SpeedLevel should export a TypeScript definition")
        .is_empty());
    assert!(!PanTiltDirection::export_to_string(&config)
        .expect("PanTiltDirection should export a TypeScript definition")
        .is_empty());
    assert!(!PresetNumber::export_to_string(&config)
        .expect("PresetNumber should export a TypeScript definition")
        .is_empty());
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
        AddressingMode, BackoffStrategy, BufferConfig, RetryAttempt, RetryConfig,
        TcpKeepaliveConfig, TransportConfig, DEFAULT_MAX_PENDING_QUEUE_DEPTH,
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
#[cfg(feature = "dyn-api")]
fn test_dyn_api_public_contract() {
    use grafton_visca::dynapi::{
        DynCameraControl, DynFocusControl, DynMotionControl, DynPanTiltControl, DynPresetsControl,
        DynZoomControl, InFlightDyn, IntoDynCamera,
    };
    use grafton_visca::{capabilities::Capabilities, mode::BoxFuture, Error, StateCache};

    fn _assert_dyn_camera_object_safe(_: &dyn DynCameraControl) {}
    fn _assert_pan_tilt_object_safe(_: &dyn DynPanTiltControl) {}
    fn _assert_zoom_object_safe(_: &dyn DynZoomControl) {}
    fn _assert_focus_object_safe(_: &dyn DynFocusControl) {}
    fn _assert_presets_object_safe(_: &dyn DynPresetsControl) {}
    fn _assert_motion_object_safe(_: &dyn DynMotionControl) {}

    fn _assert_into_dyn_bound<T: IntoDynCamera>() {}
    fn _assert_control_accessors(camera: &dyn DynCameraControl) {
        let _: &Capabilities = camera.capabilities();
        let _: &dyn DynPanTiltControl = camera.pan_tilt();
        let _: &dyn DynZoomControl = camera.zoom();
        let _: &dyn DynFocusControl = camera.focus();
        let _: &dyn DynPresetsControl = camera.presets();
        let _: &dyn DynMotionControl = camera.motion();
        let _: &StateCache = camera.state_cache();
    }

    fn _assert_motion_waits(motion: &dyn DynMotionControl) {
        fn assert_box_future(_: BoxFuture<'_, Result<(), Error>>) {}

        assert_box_future(motion.await_idle(Duration::from_secs(1)));
        assert_box_future(motion.await_pan_tilt_idle(Duration::from_secs(1)));
        assert_box_future(motion.await_zoom_idle(Duration::from_secs(1)));
        assert_box_future(motion.await_focus_idle(Duration::from_secs(1)));
    }

    let _: PhantomData<Box<dyn DynCameraControl + Send + Sync>> = PhantomData;
    let _: PhantomData<fn() -> InFlightDyn> = PhantomData;
    let _: PhantomData<fn() -> Box<dyn DynCameraControl>> = PhantomData;
    let _: PhantomData<fn(&dyn DynCameraControl)> = PhantomData;
}

#[test]
fn test_public_compile_time_api_contracts() {
    let cases = trybuild::TestCases::new();
    cases.pass("tests/api_contract/pass/*.rs");
    #[cfg(feature = "mode-async")]
    cases.pass("tests/api_contract/pass_async/*.rs");
    #[cfg(not(feature = "mode-async"))]
    cases.pass("tests/api_contract/pass_blocking/*.rs");
    #[cfg(all(not(feature = "mode-async"), feature = "transport-serial"))]
    cases.pass("tests/api_contract/pass_serial_blocking/*.rs");
    #[cfg(feature = "transport-serial-tokio")]
    cases.pass("tests/api_contract/pass_serial_tokio/*.rs");
    #[cfg(feature = "dyn-api")]
    cases.pass("tests/api_contract/pass_dyn/*.rs");
    #[cfg(feature = "test-utils")]
    cases.pass("tests/api_contract/pass_test_utils/*.rs");
}

#[test]
fn test_internal_api_contracts_do_not_compile() {
    let mut fixture_dirs = vec!["tests/api_contract/fail"];

    if cfg!(feature = "mode-async") {
        fixture_dirs.push("tests/api_contract/fail_async_mode");
    } else {
        fixture_dirs.push("tests/api_contract/fail_blocking_mode");
    }

    if cfg!(feature = "runtime-tokio") {
        fixture_dirs.push("tests/api_contract/fail_async");
    }

    if !cfg!(feature = "test-utils") {
        fixture_dirs.push("tests/api_contract/fail_no_test_utils");
    }

    let features = compile_fail::active_grafton_visca_features();
    compile_fail::assert_compile_fail_fixtures(&fixture_dirs, &features);
}
