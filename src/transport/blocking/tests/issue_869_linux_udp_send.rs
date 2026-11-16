//! The real connected UDP send seam retains Linux's queued ICMP cause.

#![cfg(all(target_os = "linux", feature = "blocking"))]
#![allow(clippy::expect_used, clippy::unwrap_used)]

use std::{
    io,
    net::{SocketAddr, UdpSocket},
    sync::{Arc, Mutex},
    thread::{self, JoinHandle},
    time::Duration,
};

#[cfg(any(feature = "test-utils", feature = "runtime-tokio"))]
use crate::testing::frames;
#[cfg(not(any(feature = "test-utils", feature = "runtime-tokio")))]
use crate::wire::VISCA_TERMINATOR;
use crate::{
    error::ErrorKind,
    observability::SessionStatus,
    operation::AppliedOnly,
    transport::{BlockingTransport, HasTransportConfig, ReceiveOutcome, TransportConfig},
    Error,
};
#[cfg(not(any(feature = "test-utils", feature = "runtime-tokio")))]
#[allow(dead_code)]
#[path = "../../../testing/frames.rs"]
mod frames;

// Keep the existing synthetic profile in this lib-test crate's type universe.
// External test-support depends on the separately compiled library instance.
use crate::{
    capabilities::{
        self, Capabilities, CapabilityDomain, CapabilityRange, CoordinateSystem,
        EndpointQuantization, ExposureCaps, FocusCaps, FocusNearEnd, ImageCaps, InquirySupport,
        MenuCaps, PanTiltCaps, PanTiltWireCodec, PowerCaps, PresetCaps, RgbGainRanges,
        RgbTuningRanges, ShutterSpeedEntry, TiltRanges, TypedSupportSet, TypedSupportSurface,
        WhiteBalanceCaps, ZoomCaps,
    },
    config::SessionConfig,
    profiles::{
        CommandTimeouts, CompileTimeProfile, PanTiltCoordinateConversion, ProfileEnvelope,
        ProfileId, ProfileSpec, ProfileTiming, TransportCompatibility,
    },
    types::{AffectedAxes, ExposureMode, FocusZone, WhiteBalanceMode},
};

#[derive(Debug, Clone, Copy)]
struct NonDefaultCompileTimeProfile;
impl CompileTimeProfile for NonDefaultCompileTimeProfile {
    const TYPED_SUPPORT: TypedSupportSet = TypedSupportSet::EMPTY;
    const ID: Option<ProfileId> = None;
    fn spec() -> crate::Result<ProfileSpec> {
        synthetic_spec(
            "Non-Default Compile-Time Profile",
            Self::TYPED_SUPPORT,
            Duration::from_secs(1),
            CommandTimeouts::DEFAULT,
        )
    }
}

fn session_config() -> SessionConfig {
    SessionConfig::new(NonDefaultCompileTimeProfile::spec().expect("two-socket raw profile"))
}

struct RestartingCamera {
    endpoint: SocketAddr,
    worker: Mutex<Option<JoinHandle<()>>>,
}

impl RestartingCamera {
    fn after_send_failure(&self) {
        let socket = UdpSocket::bind(self.endpoint).expect("camera restarts on the same port");
        socket
            .set_read_timeout(Some(Duration::from_secs(3)))
            .unwrap();
        *self.worker.lock().unwrap() = Some(thread::spawn(move || {
            let mut command = [0; 128];
            let (length, peer) = socket.recv_from(&mut command).expect("later command");
            assert_eq!(&command[..length], &[0x81, 0x01, 0x04, 0x07, 0x00, 0xff]);
            for reply in [frames::ack(1), frames::complete(1)] {
                socket.send_to(&reply, peer).expect("camera response");
            }
        }));
    }
}

/// Keep the owner out of receive while Linux queues the error: the first
/// native send targets the closed port, and the second native send observes
/// its ICMP port-unreachable. No synthetic transport error enters the test.
struct PrimeSend {
    inner: crate::transport::blocking::Udp,
    first: bool,
    camera: Arc<RestartingCamera>,
}

impl HasTransportConfig for PrimeSend {
    fn identity(&self) -> crate::transport::TransportIdentity<'_> {
        self.inner.identity()
    }

    fn transport_config(&self) -> &TransportConfig {
        self.inner.transport_config()
    }
}

impl BlockingTransport for PrimeSend {
    fn send_with_timeout(&mut self, bytes: &[u8], timeout: Duration) -> Result<(), Error> {
        if !std::mem::take(&mut self.first) {
            return self.inner.send_with_timeout(bytes, timeout);
        }
        self.inner.send_with_timeout(bytes, timeout)?;
        let cause = self
            .inner
            .send_with_timeout(bytes, timeout)
            .expect_err("the second send observes loopback ICMP port-unreachable");
        assert!(
            matches!(&cause, Error::Io { source: error } if error.kind() == io::ErrorKind::ConnectionRefused)
        );
        self.camera.after_send_failure();
        Err(cause)
    }

    fn recv_into_with_timeout(
        &mut self,
        bytes: &mut [u8],
        timeout: Duration,
    ) -> Result<ReceiveOutcome, Error> {
        self.inner.recv_into_with_timeout(bytes, timeout)
    }
}

#[test]
fn a_real_second_udp_send_preserves_econnrefused_and_later_work_applies() {
    let reservation = UdpSocket::bind("127.0.0.1:0").unwrap();
    let endpoint = reservation.local_addr().unwrap();
    drop(reservation);
    let camera = Arc::new(RestartingCamera {
        endpoint,
        worker: Mutex::new(None),
    });
    let inner = crate::transport::blocking::Udp::connect_with_config(
        &endpoint.to_string(),
        TransportConfig::for_udp(),
    )
    .expect("native connected UDP transport");
    let session = crate::blocking::Session::open(
        PrimeSend {
            inner,
            first: true,
            camera: Arc::clone(&camera),
        },
        &session_config(),
    )
    .unwrap();
    let target = session
        .camera::<NonDefaultCompileTimeProfile>(crate::types::CameraId::CAMERA_1)
        .unwrap();
    let mut first = target
        .submit::<AppliedOnly, _>(&crate::command::zoom::Stop)
        .unwrap();
    let failure = first.applied().expect_err("the actual send failed");
    assert_eq!(failure.kind(), ErrorKind::Transport);
    assert!(!failure.requires_new_session());
    let mut source: Option<&(dyn std::error::Error + 'static)> = Some(&failure);
    let mut os_kind = None;
    while let Some(error) = source {
        if let Some(error) = error.downcast_ref::<io::Error>() {
            os_kind = Some(error.kind());
            break;
        }
        if let Some(error) = error.downcast_ref::<Arc<io::Error>>() {
            os_kind = Some(error.kind());
            break;
        }
        source = error.source();
    }
    assert_eq!(os_kind, Some(io::ErrorKind::ConnectionRefused));
    let mut next = target
        .submit::<AppliedOnly, _>(&crate::command::zoom::Stop)
        .unwrap();
    next.applied().expect("the same UDP session remains usable");
    let metrics = session.metrics().unwrap();
    assert_eq!(metrics.session, SessionStatus::Running);
    assert_eq!(metrics.writes, 2);
    assert_eq!(metrics.write_failures, 1);
    session.close().unwrap();
    camera
        .worker
        .lock()
        .unwrap()
        .take()
        .unwrap()
        .join()
        .unwrap();
}

fn synthetic_spec(
    model: &str,
    typed_support: TypedSupportSet,
    ambiguity_timeout: Duration,
    command_timeouts: CommandTimeouts,
) -> crate::Result<ProfileSpec> {
    use TypedSupportSurface as S;
    let mut caps = Capabilities::runtime_baseline(model, 1)?;
    caps.typed_support = typed_support;
    caps.inquiry_support = InquirySupport::Full;
    caps.pan_tilt = Some(PanTiltCaps {
        pan_range: CapabilityRange::<i32>::new(-1700, 1700),
        tilt_ranges: TiltRanges::Single(CapabilityRange::<i32>::new(-300, 900)),
        pan_speed: CapabilityRange::<u8>::new(1, 24),
        tilt_speed: CapabilityRange::<u8>::new(1, 20),
        coordinates: PanTiltCoordinateConversion::new(
            CoordinateSystem::SignedCentered,
            PanTiltWireCodec::StandardVisca,
            10.0,
            10.0,
        )?,
        endpoint_quantization: EndpointQuantization::Exact,
        simultaneous: true,
        preset_recovery_time: Duration::ZERO,
        variable_speed: false,
        motion_sync_speed_range: typed_support
            .contains(S::MotionSync)
            .then(|| CapabilityRange::<u8>::new(1, 24)),
    });
    caps.zoom = Some(ZoomCaps {
        optical_max: 0x4000,
        digital_max: Some(0x7000),
        speed: CapabilityRange::<u8>::new(0, 7),
        direct: true,
        variable: true,
        optical_ratio: None,
    });
    caps.focus = Some(FocusCaps {
        range: CapabilityRange::<u16>::new(0x1000, 0xf000),
        near_end: FocusNearEnd::Minimum,
        speed: CapabilityRange::<u8>::new(0, 7),
        auto_focus: true,
        one_push: true,
        zone_inquiry: typed_support.contains(S::FocusZoneInquiry),
        zones: vec![FocusZone::Top, FocusZone::Center, FocusZone::Bottom],
        af_sensitivity: true,
        near_limit_inquiry: true,
    });
    caps.exposure = Some(ExposureCaps {
        modes: vec![
            ExposureMode::Auto,
            ExposureMode::Manual,
            ExposureMode::Shutter,
            ExposureMode::Iris,
            ExposureMode::Bright,
        ],
        backlight_compensation: typed_support.contains(S::BacklightCompensation),
        wide_dynamic_range: typed_support.contains(S::WideDynamicRange),
        compensation_range: typed_support
            .contains(S::ExposureCompensation)
            .then(|| CapabilityRange::<i8>::new(-7, 7)),
        iris_range: typed_support
            .contains(S::Iris)
            .then(|| CapabilityDomain::<u16>::new(0, 30)),
        gain_range: Some(CapabilityRange::<u8>::new(0, 15)),
        shutter_speeds: vec![ShutterSpeedEntry::new(
            crate::types::ExposureTime::new(1, 60)?,
            0x01,
        )],
        brightness_range: typed_support
            .contains(S::Brightness)
            .then(|| CapabilityDomain::<u8>::new(0, 0xff)),
    });
    let color_temperature = typed_support.contains(S::ColorTemperature)
        || typed_support.contains(S::ColorTemperatureInquiry);
    let mut white_modes = vec![WhiteBalanceMode::Auto, WhiteBalanceMode::Manual];
    if color_temperature {
        white_modes.push(WhiteBalanceMode::ColorTemperature);
    }
    caps.white_balance = Some(WhiteBalanceCaps {
        modes: white_modes,
        one_push: typed_support.contains(S::OnePushWhiteBalance),
        color_temp_range: color_temperature.then(|| CapabilityRange::<u16>::new(2500, 8000)),
        color_temperature_steps: if color_temperature {
            (0..=55)
                .map(|step| 2500 + 100 * step)
                .collect::<Vec<_>>()
                .into()
        } else {
            Arc::from([])
        },
        rgb_gain: typed_support.contains(S::RgbGain).then(|| RgbGainRanges {
            red: CapabilityRange::<u8>::new(0, 0xff),
            blue: CapabilityRange::<u8>::new(0, 0xff),
        }),
        rgb_tuning: typed_support
            .contains(S::RgbTuning)
            .then(|| RgbTuningRanges {
                red: CapabilityRange::<i8>::new(-10, 10),
                blue: CapabilityRange::<i8>::new(-10, 10),
            }),
    });
    // This broad fixture exists only to instantiate generic noun contracts.
    // Unlike the old metadata-trait fixture, its granted surfaces have facts.
    if typed_support.contains(S::Contrast) {
        caps.image = Some(ImageCaps {
            contrast_range: Some(CapabilityRange::<u8>::new(0, 14)),
            sharpness_range: Some(CapabilityRange::<u8>::new(0, 15)),
            saturation_range: Some(CapabilityRange::<u8>::new(0, 14)),
            hue_range: Some(CapabilityRange::<u8>::new(0, 14)),
            luminance_range: Some(CapabilityRange::<u8>::new(0, 14)),
            gamma_range: Some(CapabilityRange::<u8>::new(0, 4)),
            flip: true,
            mirror: true,
            requires_settings_save_for_flip: false,
            picture_effect: true,
        });
    }
    caps.presets = Some(PresetCaps {
        highest: 6,
        recall_axes: AffectedAxes::PAN_TILT
            .union(AffectedAxes::ZOOM)
            .union(AffectedAxes::FOCUS),
        recall_speed_range: Some(CapabilityRange::<u8>::new(1, 23)),
        tour: false,
        thumbnail: false,
        recall_delay: Duration::ZERO,
        names: None,
    });
    caps.power = Some(PowerCaps {
        on_time: Duration::from_secs(5),
        standby: typed_support
            .contains(S::PowerStandby)
            .then_some(Duration::from_secs(5)),
        wake_on_lan: false,
        retains_settings: true,
        home_on_up: false,
    });
    if typed_support.contains(S::MenuDisplay) {
        caps.menu = Some(MenuCaps {});
    }
    let mut builder = ProfileSpec::builder(caps)
        .transports(
            TransportCompatibility::new()
                .with_tcp(5678)
                .with_udp(1259)
                .with_serial(),
        )
        .envelope(ProfileEnvelope::RawVisca)
        .timing(
            ProfileTiming::builder()
                .ack_timeout(Duration::from_millis(100))
                .command_timeouts(command_timeouts)
                .inquiry_timeout(Duration::from_secs(1))
                .cancellation_timeout(Duration::from_secs(1))
                .ambiguity_timeout(ambiguity_timeout)
                .busy_timeout(Duration::ZERO)
                .raw_inquiry_reply_skew(Duration::ZERO)
                .inquiry_spacing(Duration::ZERO)
                .command_spacing(Duration::ZERO)
                .build()?,
        )
        .maximum_command_sockets(2)
        .supports_operation_complete(false)
        .supports_command_cancel(true)
        .position_inquiries(AffectedAxes::MOVEMENT);
    // These caller-owned fixtures choose concrete encoding families explicitly.
    // Grants describe permission and do not infer profile wire encodings.
    if typed_support.contains(S::PresetRecallSpeed) {
        builder = builder.encoding(capabilities::PresetRecallSpeedEncoding::PtzOptics);
    }
    if typed_support.contains(S::OnePushWhiteBalance) {
        builder = builder.encoding(capabilities::OnePushWhiteBalanceEncoding::Standard);
    }
    builder.build()
}
