//! Public API contracts for the owner-backed 2.0 surface.
//!
//! The detailed noun, request, lifecycle, construction, observability, and
//! dynamic inventories live in the issue-specific suites.  This file keeps
//! the feature matrix and the root exports honest without importing any of
//! the removed 1.x mode, client, control-trait, or runtime-handle vocabulary.

#[path = "common/compile_fail.rs"]
mod compile_fail;

use std::{num::NonZeroUsize, time::Duration};

#[cfg(any(feature = "async", feature = "blocking"))]
use std::marker::PhantomData;

#[test]
fn root_value_and_transport_contract() {
    use grafton_visca::{
        transport::{
            AddressingMode, BackoffStrategy, BufferConfig, RetryAttempt, RetryConfig,
            TcpKeepaliveConfig, TransportConfig, DEFAULT_MAX_PENDING_QUEUE_DEPTH,
        },
        types::{ColorTemp, FocusPosition, PanSpeed, TiltSpeed, ZoomPosition, ZoomSpeed},
        units::Raw,
        CameraId, Error, PresetNumber, SpeedLevel, ViscaSocket,
    };

    assert!(matches!(Error::from_code(0x02), Error::SyntaxError));
    assert!(matches!(Error::from_code(0x03), Error::CommandBufferFull));

    let preset = PresetNumber::new(255).expect("preset 255 is valid");
    assert_eq!(preset.value(), 255);
    assert_eq!(PresetNumber::MIN, 0);
    assert_eq!(PresetNumber::MAX, 255);

    let camera_id = CameraId::new(1).expect("camera ID 1 is valid");
    assert_eq!(camera_id.to_address_byte(), 0x81);
    assert!(CameraId::new(0).is_err());
    assert!(CameraId::new(9).is_err());
    assert!(CameraId::BROADCAST.is_broadcast());

    assert_eq!(ViscaSocket::S1.as_index(), 0);
    assert_eq!(ViscaSocket::S2.as_index(), 1);
    assert_eq!(ViscaSocket::from_protocol_byte(1), Some(ViscaSocket::S1));
    assert_eq!(ViscaSocket::from_protocol_byte(2), Some(ViscaSocket::S2));
    assert_eq!(ViscaSocket::from_protocol_byte(0), None);
    assert_eq!(SpeedLevel::Fast as u8, 3);

    assert_eq!(PanSpeed::ZERO.value(), 0);
    assert_eq!(TiltSpeed::ZERO.value(), 0);
    assert_eq!(ZoomSpeed::ZERO.value(), 0);
    assert_eq!(PanSpeed::from(SpeedLevel::Medium).value(), 12);
    assert_eq!(TiltSpeed::from(SpeedLevel::Medium).value(), 10);
    assert_eq!(ZoomSpeed::from(SpeedLevel::Fastest).value(), 7);
    assert!(PanSpeed::new(25).is_err());

    let zoom = ZoomPosition::new(0x4000).expect("raw zoom is in range");
    assert_eq!(zoom.value(), 0x4000);
    assert_eq!(ZoomPosition::MIN.value(), 0);
    assert_eq!(ZoomPosition::MAX.value(), 0x7fff);
    assert_eq!(
        ZoomPosition::try_from(Raw(0x4000_u16))
            .expect("raw zoom wrapper is checked")
            .value(),
        0x4000
    );
    assert!(ZoomPosition::try_from(Raw(0xffff_u16)).is_err());

    let focus = FocusPosition::new(0x1234);
    assert_eq!(focus.value(), 0x1234);
    assert_eq!(u16::from(focus), 0x1234);

    let color_temp = ColorTemp::from_kelvin(5600).expect("5600K is in range");
    assert_eq!(color_temp.value(), 31);
    assert_eq!(color_temp.to_kelvin(), 5600);
    assert!(ColorTemp::from_kelvin(2400).is_err());
    assert!(ColorTemp::from_kelvin(8100).is_err());

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

    let config = TransportConfig::default();
    assert_eq!(config.connect_timeout, Duration::from_secs(5));
    assert_eq!(config.read_timeout, Duration::from_secs(5));
    assert_eq!(config.write_timeout, Duration::from_secs(5));
    assert_eq!(config.addressing, AddressingMode::Ip);
    assert_eq!(config.tcp_nodelay, Some(true));
    assert_eq!(config.tcp_keepalive, Some(keepalive));
    assert_eq!(
        config.max_pending_queue_depth,
        NonZeroUsize::new(DEFAULT_MAX_PENDING_QUEUE_DEPTH).expect("nonzero queue depth")
    );
}

#[test]
fn profile_marker_and_capability_contract() {
    use grafton_visca::{
        capabilities::{
            Capabilities, Exposure, Focus, HasNdFilter, ImageProcessing, InquirySupport, PanTilt,
            Power, Profile, ProfileMetadata, SupportsTcp, SupportsUdp, WhiteBalance, Zoom,
        },
        profiles::{GenericVisca, PtzOpticsG2, SonyFR7},
        CompileTimeProfile,
    };

    fn profile<P>()
    where
        P: CompileTimeProfile
            + Profile
            + ProfileMetadata
            + Power
            + Zoom
            + PanTilt
            + Focus
            + Exposure,
    {
    }

    fn advanced<P>()
    where
        P: Profile + ImageProcessing + WhiteBalance,
    {
    }

    fn nd_filter<P: HasNdFilter>() {}

    profile::<PtzOpticsG2>();
    profile::<SonyFR7>();
    profile::<GenericVisca>();
    advanced::<PtzOpticsG2>();
    advanced::<SonyFR7>();
    advanced::<GenericVisca>();
    nd_filter::<SonyFR7>();

    assert_eq!(PtzOpticsG2::MODEL_NAME, "PtzOptics G2");
    assert_eq!(PtzOpticsG2::DEFAULT_CAMERA_ID, 1);
    assert_eq!(<PtzOpticsG2 as SupportsTcp>::DEFAULT_TCP_PORT, 5678);
    assert_eq!(<PtzOpticsG2 as SupportsUdp>::DEFAULT_UDP_PORT, 1259);
    assert_eq!(<GenericVisca as SupportsTcp>::DEFAULT_TCP_PORT, 5678);
    assert_eq!(<GenericVisca as SupportsUdp>::DEFAULT_UDP_PORT, 1259);
    assert_eq!(SonyFR7::DEFAULT_CAMERA_ID, 1);

    let g2 = Capabilities::from_profile::<PtzOpticsG2>();
    assert!(g2.has_basic_features());
    assert!(g2.has_full_inquiry_support());
    assert_eq!(g2.inquiry_support, InquirySupport::Full);
    assert!(!g2.has_motion_sync);
    assert!(!g2.has_nd_filter);
    assert!(g2.has_iris_control);
    assert!(g2.has_picture_effect);

    let fr7 = Capabilities::from_profile::<SonyFR7>();
    assert!(fr7.has_basic_features());
    assert!(fr7.has_advanced_features());
    assert!(fr7.has_nd_filter);
    assert!(fr7.supports_wake_on_lan);
    assert_eq!(fr7.default_tcp_port, None);
    assert_eq!(fr7.default_udp_port, Some(52381));
}

#[cfg(feature = "async")]
#[test]
fn async_root_exports_are_profile_and_handle_safe() {
    use grafton_visca::{
        completion::{AppliedOnly, Targeted},
        profiles::PtzOpticsG2,
        Camera, Cancellation, CompileTimeProfile, Operation, Session, SessionConfig,
    };

    fn assert_profile<P: CompileTimeProfile>() {}
    fn assert_send_sync<T: Send + Sync>() {}

    assert_profile::<PtzOpticsG2>();
    let _: PhantomData<Camera<PtzOpticsG2>> = PhantomData;
    let _: PhantomData<Session> = PhantomData;
    let _: PhantomData<SessionConfig> = PhantomData;
    let _: PhantomData<Operation<AppliedOnly>> = PhantomData;
    let _: PhantomData<Operation<Targeted>> = PhantomData;
    let _: PhantomData<Cancellation> = PhantomData;
    assert_send_sync::<Operation<AppliedOnly>>();
    assert_send_sync::<Operation<Targeted>>();
    assert_send_sync::<Cancellation>();
}

#[cfg(feature = "blocking")]
#[test]
fn blocking_root_exports_are_profile_and_handle_safe() {
    use grafton_visca::{
        blocking::{Camera, Operation, Session},
        completion::{AppliedOnly, Targeted},
        profiles::PtzOpticsG2,
        CompileTimeProfile, SessionConfig,
    };

    fn assert_profile<P: CompileTimeProfile>() {}

    assert_profile::<PtzOpticsG2>();
    let _: PhantomData<Camera<'static, PtzOpticsG2>> = PhantomData;
    let _: PhantomData<Session> = PhantomData;
    let _: PhantomData<SessionConfig> = PhantomData;
    let _: PhantomData<Operation<'static, AppliedOnly>> = PhantomData;
    let _: PhantomData<Operation<'static, Targeted>> = PhantomData;
}

#[cfg(all(feature = "blocking", feature = "async"))]
#[test]
fn blocking_and_async_facades_coexist() {
    use grafton_visca::completion::{AppliedOnly, Targeted};
    use grafton_visca::{blocking, profiles::PtzOpticsG2, Camera, Operation, Session};

    let _: PhantomData<blocking::Camera<'static, PtzOpticsG2>> = PhantomData;
    let _: PhantomData<blocking::Session> = PhantomData;
    let _: PhantomData<Camera<PtzOpticsG2>> = PhantomData;
    let _: PhantomData<Session> = PhantomData;
    let _: PhantomData<Operation<AppliedOnly>> = PhantomData;
    let _: PhantomData<Operation<Targeted>> = PhantomData;
}

#[cfg(feature = "dyn-api")]
#[test]
fn dyn_root_is_object_safe() {
    use grafton_visca::dynapi::{
        DynCustomOperations, DynMotion, DynSessionCamera, DynSessionCameraControl,
        DynSessionCameraNouns,
    };

    fn control(_: &dyn DynSessionCameraControl) {}
    fn nouns(_: &dyn DynSessionCameraNouns) {}
    fn custom(_: &dyn DynCustomOperations) {}
    fn motion(_: &dyn DynMotion) {}
    fn owner(camera: &DynSessionCamera) {
        control(camera);
        nouns(camera);
        custom(camera);
        motion(camera.motion());
    }

    let _ = owner;
}

#[test]
fn canonical_compile_contracts() {
    // The base-only feature leg has no conditional directory to append.
    #[allow(unused_mut)]
    let mut pass_dirs = vec!["tests/api_contract/pass"];
    #[cfg(feature = "async")]
    pass_dirs.push("tests/api_contract/pass_async");
    #[cfg(feature = "blocking")]
    pass_dirs.push("tests/api_contract/pass_blocking");
    #[cfg(all(feature = "blocking", feature = "async"))]
    pass_dirs.push("tests/api_contract/pass_coexistence");
    #[cfg(feature = "dyn-api")]
    pass_dirs.push("tests/api_contract/pass_dyn");
    #[cfg(feature = "test-utils")]
    pass_dirs.push("tests/api_contract/pass_test_utils");
    #[cfg(all(feature = "blocking", feature = "transport-serial"))]
    pass_dirs.push("tests/api_contract/pass_serial_blocking");
    #[cfg(feature = "transport-serial-tokio")]
    pass_dirs.push("tests/api_contract/pass_serial_tokio");

    let mut fail_dirs = vec!["tests/api_contract/fail"];
    #[cfg(feature = "async")]
    fail_dirs.push("tests/api_contract/fail_async_canonical");
    #[cfg(all(feature = "blocking", not(feature = "async")))]
    {
        fail_dirs.push("tests/api_contract/fail_blocking_mode");
        fail_dirs.push("tests/api_contract/fail_blocking_only");
    }
    #[cfg(all(feature = "async", not(feature = "blocking")))]
    fail_dirs.push("tests/api_contract/fail_async_only");
    #[cfg(all(feature = "blocking", feature = "async"))]
    fail_dirs.push("tests/api_contract/fail_blocking_mode");
    #[cfg(feature = "dyn-api")]
    {
        fail_dirs.push("tests/api_contract/fail_dyn_async");
        fail_dirs.push("tests/api_contract/fail_must_use_dyn");
    }
    #[cfg(not(any(feature = "blocking", feature = "async")))]
    fail_dirs.push("tests/api_contract/fail_no_canonical");
    #[cfg(not(feature = "test-utils"))]
    fail_dirs.push("tests/api_contract/fail_no_test_utils");

    let active_features = compile_fail::active_grafton_visca_features();
    compile_fail::assert_compile_pass_fixtures(&pass_dirs, &active_features);
    compile_fail::assert_compile_fail_fixtures(&fail_dirs, &active_features);
}
