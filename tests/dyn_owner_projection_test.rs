//! Owner-backed dynamic operation projection coverage.

#![cfg(all(
    feature = "dyn-api",
    any(feature = "runtime-tokio", feature = "runtime-smol")
))]

use std::{future::Future, sync::Arc};

use grafton_visca::{
    capabilities::{
        Capabilities, InquirySupport, RuntimeShutterSpeed, TypedSupportSet, TypedSupportSurface,
    },
    dynapi::{DynSessionCamera, DynSessionCameraNouns},
    profile::{PositionInquirySupport, ProfileSpec},
    profiles::{SonyBRC300, SonyFR7},
    transport::{AsyncTransport, HasTransportConfig, SendSemantics, TransportConfig},
    types::ZoomPosition,
    units::UnitInterval,
    CameraId, CommandTimeouts, Error, ExposureMode, ProfileEnvelope, ProfileTiming, Session,
    SessionConfig, TransportCompatibility, ZoomDomain,
};
use std::time::Duration;

#[derive(Debug)]
struct ScriptedTransport {
    config: TransportConfig,
    replies: flume::Receiver<Vec<u8>>,
    reply_tx: flume::Sender<Vec<u8>>,
    writes: Arc<std::sync::Mutex<Vec<Vec<u8>>>>,
}

impl HasTransportConfig for ScriptedTransport {
    fn transport_config(&self) -> &TransportConfig {
        &self.config
    }
}

impl AsyncTransport for ScriptedTransport {
    fn send(&mut self, bytes: &[u8]) -> impl Future<Output = Result<(), Error>> + Send {
        let bytes = bytes.to_vec();
        let reply_tx = self.reply_tx.clone();
        self.writes
            .lock()
            .expect("scripted transport writes lock")
            .push(bytes.clone());
        async move {
            if bytes.get(1) == Some(&0x09) {
                // Sony BRC-300 returns five pan and four tilt nibbles.
                reply_tx
                    .send_async(vec![
                        0x90, 0x50, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0xff,
                    ])
                    .await
                    .map_err(|_| Error::ConnectionClosed { reason: None })?;
            } else {
                reply_tx
                    .send_async(vec![0x90, 0x41, 0xff])
                    .await
                    .map_err(|_| Error::ConnectionClosed { reason: None })?;
                reply_tx
                    .send_async(vec![0x90, 0x51, 0xff])
                    .await
                    .map_err(|_| Error::ConnectionClosed { reason: None })?;
            }
            Ok(())
        }
    }

    #[allow(clippy::manual_async_fn)]
    fn recv_into<'a>(
        &'a mut self,
        dst: &'a mut [u8],
    ) -> impl Future<Output = Result<usize, Error>> + Send {
        async move {
            let bytes = self
                .replies
                .recv_async()
                .await
                .map_err(|_| Error::ConnectionClosed { reason: None })?;
            dst[..bytes.len()].copy_from_slice(&bytes);
            Ok(bytes.len())
        }
    }

    fn send_semantics(&self) -> SendSemantics {
        SendSemantics::Datagram
    }
}

fn transport() -> (ScriptedTransport, Arc<std::sync::Mutex<Vec<Vec<u8>>>>) {
    let (reply_tx, replies) = flume::bounded(32);
    let writes = Arc::new(std::sync::Mutex::new(Vec::new()));
    (
        ScriptedTransport {
            config: TransportConfig::default(),
            replies,
            reply_tx,
            writes: Arc::clone(&writes),
        },
        writes,
    )
}

fn runtime_equivalent(static_profile: &ProfileSpec) -> ProfileSpec {
    let coordinates = static_profile
        .pan_tilt_coordinates()
        .expect("Sony BRC-300 pan/tilt conversion");
    let timing: ProfileTiming = static_profile.timing();
    ProfileSpec::builder(static_profile.capabilities().clone())
        .pan_tilt_coordinates(
            coordinates.coordinate_system(),
            coordinates.pan_degrees_to_units(),
            coordinates.tilt_degrees_to_units(),
        )
        .pan_tilt_wire_codec(coordinates.wire_codec())
        .transports(static_profile.transports())
        .envelope(static_profile.envelope())
        .timing(
            ProfileTiming::builder()
                .ack_timeout(timing.ack_timeout())
                .command_timeouts(timing.command_timeouts())
                .inquiry_timeout(timing.inquiry_timeout())
                .cancellation_timeout(timing.cancellation_timeout())
                .ambiguity_timeout(timing.ambiguity_timeout())
                .busy_timeout(timing.busy_timeout())
                .raw_inquiry_reply_skew(timing.raw_inquiry_reply_skew())
                .minimum_inquiry_spacing(timing.minimum_inquiry_spacing())
                .minimum_command_spacing(timing.minimum_command_spacing())
                .build()
                .expect("valid timing"),
        )
        .maximum_command_sockets(static_profile.maximum_command_sockets())
        .supports_operation_complete(static_profile.supports_operation_complete())
        .supports_command_cancel(static_profile.supports_command_cancel())
        .preset_recall_axes(static_profile.preset_recall_axes())
        .position_inquiries(static_profile.position_inquiries())
        .build()
        .expect("runtime profile equivalent")
}

/// A runtime profile may document the physical digital range while declining
/// to expose its typed control surface. This is the partial-profile shape the
/// dynamic API must reject for the combined normalized noun.
fn documented_digital_zoom_profile(digital_range_permission: bool) -> ProfileSpec {
    let mut capabilities =
        Capabilities::runtime_baseline("Documented Digital Zoom", 1).expect("baseline profile");
    capabilities.has_zoom = true;
    capabilities.has_digital_zoom = true;
    capabilities.zoom_range_optical = 0..=0x4000;
    capabilities.zoom_range_digital = Some(0x4000..=0x7000);
    capabilities.zoom_speed = 0..=7;
    capabilities.supports_direct_zoom = true;
    capabilities.zoom_magnification_to_units = 1.0;
    capabilities.inquiry_support = InquirySupport::None;
    capabilities.typed_support = if digital_range_permission {
        TypedSupportSet::from_surfaces(&[
            TypedSupportSurface::DirectZoom,
            TypedSupportSurface::DigitalZoomRange,
        ])
    } else {
        TypedSupportSet::from_surface(TypedSupportSurface::DirectZoom)
    };

    ProfileSpec::builder(capabilities)
        .transports(TransportCompatibility::new(Some(5678), None, false))
        .envelope(ProfileEnvelope::RawVisca)
        .timing(
            ProfileTiming::builder()
                .ack_timeout(Duration::from_millis(100))
                .command_timeouts(CommandTimeouts::default())
                .inquiry_timeout(Duration::from_secs(1))
                .cancellation_timeout(Duration::from_secs(1))
                .ambiguity_timeout(Duration::from_secs(1))
                .busy_timeout(Duration::ZERO)
                .raw_inquiry_reply_skew(Duration::ZERO)
                .minimum_inquiry_spacing(Duration::ZERO)
                .minimum_command_spacing(Duration::ZERO)
                .build()
                .expect("valid timing"),
        )
        .maximum_command_sockets(1)
        .supports_operation_complete(true)
        .supports_command_cancel(false)
        .preset_recall_axes(None)
        .position_inquiries(PositionInquirySupport::new(false, false, false))
        .build()
        .expect("documented digital zoom profile")
}

/// This runtime inventory intentionally documents the shared exposure-mode
/// protocol family while withholding the static/dynamic permission bit. The
/// complete profile shape proves metadata alone cannot admit the erased noun.
fn documented_exposure_mode_profile_without_typed_permission() -> ProfileSpec {
    let mut capabilities =
        Capabilities::runtime_baseline("Documented Exposure Modes", 1).expect("baseline profile");
    capabilities.has_exposure = true;
    capabilities.exposure_modes = vec![ExposureMode::Auto];
    capabilities.shutter_speeds = vec![RuntimeShutterSpeed {
        label: "1/60".into(),
        value: 1,
    }];
    capabilities.gain_range = 0..=1;
    capabilities.inquiry_support = InquirySupport::Partial;
    capabilities.typed_support = TypedSupportSet::empty();

    ProfileSpec::builder(capabilities)
        .transports(TransportCompatibility::new(Some(5678), None, false))
        .envelope(ProfileEnvelope::RawVisca)
        .timing(
            ProfileTiming::builder()
                .ack_timeout(Duration::from_millis(100))
                .command_timeouts(CommandTimeouts::default())
                .inquiry_timeout(Duration::from_secs(1))
                .cancellation_timeout(Duration::from_secs(1))
                .ambiguity_timeout(Duration::from_secs(1))
                .busy_timeout(Duration::ZERO)
                .raw_inquiry_reply_skew(Duration::ZERO)
                .minimum_inquiry_spacing(Duration::ZERO)
                .minimum_command_spacing(Duration::ZERO)
                .build()
                .expect("valid timing"),
        )
        .maximum_command_sockets(1)
        .supports_operation_complete(true)
        .supports_command_cancel(false)
        .preset_recall_axes(None)
        .position_inquiries(PositionInquirySupport::new(false, false, false))
        .build()
        .expect("documented exposure-mode profile without typed permission")
}

async fn dynamic_targeted_polling<E>(runtime: E)
where
    E: grafton_visca::Executor,
{
    let static_profile = ProfileSpec::from_compile_time::<SonyBRC300>().expect("static profile");
    assert!(!static_profile.supports_operation_complete());
    assert_eq!(
        static_profile.position_inquiries(),
        PositionInquirySupport::new_with_iris_nd(true, true, true, true, false)
    );
    let runtime_profile = runtime_equivalent(&static_profile);
    assert_eq!(static_profile, runtime_profile);

    let (transport, writes) = transport();
    let session = Session::open(
        transport,
        SessionConfig::new(runtime_profile.clone()),
        runtime,
    )
    .await
    .expect("owner-backed session");
    let camera = DynSessionCamera::from_session(&session).expect("dynamic camera");
    assert_eq!(camera.target(), CameraId::CAMERA_1);
    assert_eq!(camera.profile(), &runtime_profile);
    let typed = camera
        .camera::<SonyBRC300>()
        .expect("exact dynamic-to-typed projection");
    assert_eq!(typed.target(), camera.target());

    let object: &dyn DynSessionCameraNouns = &camera;
    object
        .pan_tilt()
        .home()
        .await
        .expect("targeted admission")
        .settled()
        .await
        .expect("targeted dynamic settlement");

    {
        let writes = writes.lock().expect("writes lock");
        assert_eq!(writes.len(), 3, "command plus two exact pan/tilt inquiries");
        assert_eq!(writes[0].get(1), Some(&0x01));
        assert!(writes[1].starts_with(&[0x81, 0x09, 0x06, 0x12]));
        assert!(writes[2].starts_with(&[0x81, 0x09, 0x06, 0x12]));
    }

    session.shutdown().await.expect("session shutdown");
}

async fn dynamic_combined_normalized_zoom_respects_typed_permission<E>(runtime: E)
where
    E: grafton_visca::Executor,
{
    let midpoint = UnitInterval::new(0.5).expect("unit interval midpoint");
    let partial_profile = documented_digital_zoom_profile(false);
    let (partial_transport, partial_writes) = transport();
    let partial_session = Session::open(
        partial_transport,
        SessionConfig::new(partial_profile),
        runtime.clone(),
    )
    .await
    .expect("partial-profile session");
    let partial_camera =
        DynSessionCamera::from_session(&partial_session).expect("partial dynamic camera");

    // Both values map within the documented numeric range; the midpoint also
    // falls inside the optical range. Neither may infer permission from that
    // overlap when the noun selected the combined domain.
    for position in [midpoint, UnitInterval::ONE] {
        let error = partial_camera
            .zoom()
            .set_normalized_in_domain(position, ZoomDomain::OpticalPlusDigital)
            .await
            .expect_err("combined normalized zoom requires typed digital-range permission");
        assert!(matches!(
            error,
            Error::FeatureNotSupported {
                feature: "optical-plus-digital zoom positioning"
            }
        ));
    }
    assert!(
        partial_writes
            .lock()
            .expect("partial writes lock")
            .is_empty(),
        "rejected dynamic nouns must not reach the transport"
    );

    // Explicit raw positions and both optical-normalization entry points remain
    // direct zoom controls, so the partial profile still admits all of them.
    partial_camera
        .zoom()
        .set_position(ZoomPosition::new(0x3800).expect("optical raw target"))
        .await
        .expect("explicit optical target")
        .applied()
        .await
        .expect("explicit optical target applied");
    partial_camera
        .zoom()
        .set_normalized(midpoint)
        .await
        .expect("ordinary optical normalization")
        .applied()
        .await
        .expect("ordinary optical target applied");
    partial_camera
        .zoom()
        .set_normalized_in_domain(midpoint, ZoomDomain::Optical)
        .await
        .expect("explicit optical-domain normalization")
        .applied()
        .await
        .expect("explicit optical-domain target applied");
    assert_eq!(
        partial_writes.lock().expect("partial writes lock").len(),
        3,
        "only the three direct/optical positive controls reach the transport"
    );
    partial_session
        .shutdown()
        .await
        .expect("partial session shutdown");

    // A profile that grants the same typed digital-range permission admits
    // both the overlap midpoint and the combined endpoint through the exact
    // same erased noun path.
    let supported_profile = documented_digital_zoom_profile(true);
    let (supported_transport, supported_writes) = transport();
    let supported_session = Session::open(
        supported_transport,
        SessionConfig::new(supported_profile),
        runtime,
    )
    .await
    .expect("supported-profile session");
    let supported_camera =
        DynSessionCamera::from_session(&supported_session).expect("supported dynamic camera");
    for position in [midpoint, UnitInterval::ONE] {
        supported_camera
            .zoom()
            .set_normalized_in_domain(position, ZoomDomain::OpticalPlusDigital)
            .await
            .expect("combined normalized zoom with typed permission")
            .applied()
            .await
            .expect("combined normalized target applied");
    }
    assert_eq!(
        supported_writes
            .lock()
            .expect("supported writes lock")
            .len(),
        2,
        "both supported combined-domain controls reach the transport"
    );
    supported_session
        .shutdown()
        .await
        .expect("supported session shutdown");
}

async fn dynamic_shared_exposure_modes_require_typed_permission<E>(runtime: E)
where
    E: grafton_visca::Executor,
{
    // Sony FR7 retains exposure controls, but does not document the shared
    // `04 39` mode family. The erased noun remains callable and rejects both
    // projections before the owner reaches its transport.
    let fr7_profile = ProfileSpec::from_compile_time::<SonyFR7>().expect("Sony FR7 profile");
    assert!(fr7_profile.capabilities().has_exposure);
    assert!(fr7_profile.capabilities().exposure_modes.is_empty());
    assert!(!fr7_profile
        .capabilities()
        .supports_typed(TypedSupportSurface::ExposureMode));

    let (fr7_transport, fr7_writes) = transport();
    let fr7_session = Session::open(
        fr7_transport,
        SessionConfig::new(fr7_profile),
        runtime.clone(),
    )
    .await
    .expect("Sony FR7 session");
    let fr7_camera = DynSessionCamera::from_session(&fr7_session).expect("Sony FR7 camera");
    let fr7_nouns: &dyn DynSessionCameraNouns = &fr7_camera;

    let error = fr7_nouns
        .exposure()
        .set_mode(ExposureMode::Auto)
        .await
        .expect_err("FR7 shared exposure-mode command must be rejected");
    assert!(matches!(
        error,
        Error::FeatureNotSupported {
            feature: "shared exposure-mode control"
        }
    ));
    let error = fr7_nouns
        .exposure()
        .mode()
        .await
        .expect_err("FR7 shared exposure-mode inquiry must be rejected");
    assert!(matches!(
        error,
        Error::FeatureNotSupported {
            feature: "inquiry ExposureModeInquiry"
        }
    ));
    assert!(
        fr7_writes.lock().expect("Sony FR7 writes lock").is_empty(),
        "both rejected Sony FR7 exposure-mode noun calls must stay preflight"
    );
    fr7_session
        .shutdown()
        .await
        .expect("Sony FR7 session shutdown");

    // This is mutation-sensitive: the runtime profile has every discovery
    // fact that makes the shared mode family physically plausible, but omits
    // only its typed permission bit.
    let partial_profile = documented_exposure_mode_profile_without_typed_permission();
    assert!(partial_profile.capabilities().has_exposure);
    assert!(!partial_profile.capabilities().exposure_modes.is_empty());
    assert!(!partial_profile
        .capabilities()
        .supports_typed(TypedSupportSurface::ExposureMode));

    let (partial_transport, partial_writes) = transport();
    let partial_session = Session::open(
        partial_transport,
        SessionConfig::new(partial_profile),
        runtime,
    )
    .await
    .expect("partial exposure-mode session");
    let partial_camera =
        DynSessionCamera::from_session(&partial_session).expect("partial exposure-mode camera");
    let partial_nouns: &dyn DynSessionCameraNouns = &partial_camera;

    let error = partial_nouns
        .exposure()
        .set_mode(ExposureMode::Auto)
        .await
        .expect_err("metadata alone must not admit shared exposure-mode command");
    assert!(matches!(
        error,
        Error::FeatureNotSupported {
            feature: "shared exposure-mode control"
        }
    ));
    let error = partial_nouns
        .exposure()
        .mode()
        .await
        .expect_err("metadata alone must not admit shared exposure-mode inquiry");
    assert!(matches!(
        error,
        Error::FeatureNotSupported {
            feature: "typed inquiry ExposureModeInquiry"
        }
    ));
    assert!(
        partial_writes
            .lock()
            .expect("partial exposure-mode writes lock")
            .is_empty(),
        "both rejected partial-profile exposure-mode noun calls must stay preflight"
    );
    partial_session
        .shutdown()
        .await
        .expect("partial exposure-mode session shutdown");
}

#[cfg(feature = "runtime-tokio")]
#[tokio::test]
async fn tokio_dynamic_targeted_projection_uses_owner_settlement_wait() {
    dynamic_targeted_polling(grafton_visca::TokioRuntime::from_current().expect("runtime")).await;
}

#[cfg(feature = "runtime-tokio")]
#[tokio::test]
async fn tokio_dynamic_combined_normalized_zoom_requires_typed_digital_range() {
    dynamic_combined_normalized_zoom_respects_typed_permission(
        grafton_visca::TokioRuntime::from_current().expect("runtime"),
    )
    .await;
}

#[cfg(feature = "runtime-tokio")]
#[tokio::test]
async fn tokio_dynamic_shared_exposure_modes_require_typed_permission() {
    dynamic_shared_exposure_modes_require_typed_permission(
        grafton_visca::TokioRuntime::from_current().expect("runtime"),
    )
    .await;
}

#[cfg(feature = "runtime-smol")]
#[test]
fn smol_dynamic_targeted_projection_uses_owner_settlement_wait() {
    smol::block_on(dynamic_targeted_polling(grafton_visca::SmolRuntime::new()));
}

#[cfg(feature = "runtime-smol")]
#[test]
fn smol_dynamic_combined_normalized_zoom_requires_typed_digital_range() {
    smol::block_on(dynamic_combined_normalized_zoom_respects_typed_permission(
        grafton_visca::SmolRuntime::new(),
    ));
}

#[cfg(feature = "runtime-smol")]
#[test]
fn smol_dynamic_shared_exposure_modes_require_typed_permission() {
    smol::block_on(dynamic_shared_exposure_modes_require_typed_permission(
        grafton_visca::SmolRuntime::new(),
    ));
}
