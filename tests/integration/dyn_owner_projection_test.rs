//! Owner-backed dynamic operation projection coverage.

#![cfg(all(
    any(feature = "async", feature = "blocking"),
    any(feature = "runtime-tokio", feature = "runtime-smol")
))]

use grafton_visca_test_support::{fake_camera, runtime_matrix};

use std::time::Duration;

use grafton_visca::{
    capabilities::{
        Capabilities, InquirySupport, ShutterSpeedEntry, TypedSupportSet, TypedSupportSurface,
    },
    config::SessionConfig,
    profiles::{
        CommandTimeouts, ProfileEnvelope, ProfileSpec, ProfileTiming, SonyBrc300, SonyFr7,
        TransportCompatibility,
    },
    r#async::Session,
    types::{ExposureMode, UnitInterval, ZoomDomain, ZoomPosition},
    Error,
};

use fake_camera::{frames, FakeCamera};

/// A camera that answers every inquiry with Sony BRC-300 pan and tilt data (five
/// pan and four tilt nibbles, all zero) and every command with ACK and
/// completion on socket 1, as separate reads.
fn camera() -> FakeCamera {
    FakeCamera::new(|write, answer| {
        if write.get(1) == Some(&0x09) {
            answer.reply(frames::inquiry_reply(&[0; 9]));
        } else {
            answer.reply(frames::ack(1)).reply(frames::complete(1));
        }
    })
}

fn runtime_equivalent(static_profile: &ProfileSpec) -> ProfileSpec {
    runtime_with_capabilities(static_profile, static_profile.capabilities().clone())
}

fn runtime_with_capabilities(
    static_profile: &ProfileSpec,
    capabilities: Capabilities,
) -> ProfileSpec {
    let timing: ProfileTiming = static_profile.timing();
    crate::common::profile_encodings::with_source_encodings(
        ProfileSpec::builder(capabilities)
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
                    .inquiry_spacing(timing.inquiry_spacing())
                    .command_spacing(timing.command_spacing())
                    .build()
                    .expect("valid timing"),
            )
            .maximum_command_sockets(static_profile.maximum_command_sockets())
            .supports_operation_complete(static_profile.supports_operation_complete())
            .supports_command_cancel(static_profile.supports_command_cancel())
            .position_inquiries(static_profile.position_inquiries()),
        static_profile,
    )
    .build()
    .expect("runtime profile equivalent")
}

/// A runtime profile may document the physical digital range while declining
/// to expose its typed control surface. This is the partial-profile shape the
/// dynamic API must reject for the combined normalized noun.
fn documented_digital_zoom_profile(digital_range_permission: bool) -> ProfileSpec {
    let mut capabilities =
        Capabilities::runtime_baseline("Documented Digital Zoom", 1).expect("baseline profile");
    capabilities.zoom = Some(grafton_visca::capabilities::ZoomCaps {
        optical_max: 0x4000,
        digital_max: Some(0x7000),
        speed: grafton_visca::capabilities::CapabilityRange::<u8>::new(0, 7)
            .expect("valid capability bounds"),
        direct: true,
        variable: false,
        optical_ratio: None,
    });
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
        .transports(TransportCompatibility::new().with_tcp(5678))
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
                .inquiry_spacing(Duration::ZERO)
                .command_spacing(Duration::ZERO)
                .build()
                .expect("valid timing"),
        )
        .maximum_command_sockets(grafton_visca::types::CommandSocketCount::ONE)
        .supports_operation_complete(true)
        .supports_command_cancel(false)
        .position_inquiries(None)
        .build()
        .expect("documented digital zoom profile")
}

/// This runtime inventory intentionally documents the shared exposure-mode
/// protocol family while withholding the static/dynamic permission bit. The
/// complete profile shape proves metadata alone cannot admit the erased noun.
fn documented_exposure_mode_profile_without_typed_permission() -> ProfileSpec {
    let mut capabilities =
        Capabilities::runtime_baseline("Documented Exposure Modes", 1).expect("baseline profile");
    capabilities.exposure = Some(grafton_visca::capabilities::ExposureCaps {
        modes: vec![ExposureMode::Auto],
        backlight_compensation: false,
        wide_dynamic_range: false,
        compensation_range: None,
        iris_range: None,
        gain_range: Some(
            grafton_visca::capabilities::CapabilityRange::<u8>::new(0, 1)
                .expect("valid capability bounds"),
        ),
        shutter_speeds: vec![ShutterSpeedEntry {
            exposure: grafton_visca::types::ExposureTime::new(1, 60).expect("nonzero denominator"),
            value: 1,
        }],
        brightness_range: None,
    });
    capabilities.inquiry_support = InquirySupport::Partial;
    capabilities.typed_support = TypedSupportSet::empty();

    ProfileSpec::builder(capabilities)
        .transports(TransportCompatibility::new().with_tcp(5678))
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
                .inquiry_spacing(Duration::ZERO)
                .command_spacing(Duration::ZERO)
                .build()
                .expect("valid timing"),
        )
        .maximum_command_sockets(grafton_visca::types::CommandSocketCount::ONE)
        .supports_operation_complete(true)
        .supports_command_cancel(false)
        .position_inquiries(None)
        .build()
        .expect("documented exposure-mode profile without typed permission")
}

async fn dynamic_targeted_projection_uses_owner_settlement_wait<E>(runtime: E)
where
    E: grafton_visca::runtime::Executor,
{
    let static_profile = ProfileSpec::from_compile_time::<SonyBrc300>().expect("static profile");
    assert!(!static_profile.supports_operation_complete());
    assert_eq!(
        static_profile.position_inquiries(),
        Some(
            grafton_visca::types::AffectedAxes::PAN_TILT
                | grafton_visca::types::AffectedAxes::ZOOM
                | grafton_visca::types::AffectedAxes::FOCUS
                | grafton_visca::types::AffectedAxes::IRIS
        )
    );
    let runtime_profile = runtime_equivalent(&static_profile);
    assert_eq!(static_profile, runtime_profile);

    let fake = camera();
    let session = Session::open(
        fake.async_wire(),
        &SessionConfig::new(runtime_profile.clone()),
        runtime,
    )
    .await
    .expect("owner-backed session");
    let camera = session
        .camera::<grafton_visca::profiles::AnyProfile>(
            grafton_visca::types::IndividualCameraId::CAMERA_1,
        )
        .expect("dynamic camera");
    assert_eq!(
        camera.target(),
        grafton_visca::types::IndividualCameraId::CAMERA_1
    );
    assert_eq!(camera.profile(), &runtime_profile);
    let typed = camera
        .clone()
        .into_typed::<SonyBrc300>()
        .expect("exact dynamic-to-typed projection");
    assert_eq!(typed.target(), camera.target());

    let object = &camera;
    object
        .pan_tilt()
        .home()
        .await
        .expect("targeted admission")
        .settled()
        .await
        .expect("targeted dynamic settlement");

    let writes = fake.writes();
    assert_eq!(writes.len(), 3, "command plus two exact pan/tilt inquiries");
    assert_eq!(writes[0].get(1), Some(&0x01));
    assert!(writes[1].starts_with(&[0x81, 0x09, 0x06, 0x12]));
    assert!(writes[2].starts_with(&[0x81, 0x09, 0x06, 0x12]));

    session.shutdown().expect("session shutdown");
}

async fn dynamic_combined_normalized_zoom_requires_typed_digital_range<E>(runtime: E)
where
    E: grafton_visca::runtime::Executor,
{
    let midpoint = UnitInterval::new(0.5).expect("unit interval midpoint");
    let partial_profile = documented_digital_zoom_profile(false);
    let partial_fake = camera();
    let partial_session = Session::open(
        partial_fake.async_wire(),
        &SessionConfig::new(partial_profile),
        runtime.clone(),
    )
    .await
    .expect("partial-profile session");
    let partial_camera = partial_session
        .camera::<grafton_visca::profiles::AnyProfile>(
            grafton_visca::types::IndividualCameraId::CAMERA_1,
        )
        .expect("partial dynamic camera");

    // Both values map within the documented numeric range; the midpoint also
    // falls inside the optical range. Neither may infer permission from that
    // overlap when the noun selected the combined domain.
    for position in [midpoint, UnitInterval::MAX] {
        let error = partial_camera
            .zoom()
            .set_normalized(position, ZoomDomain::OpticalPlusDigital)
            .await
            .expect_err("combined normalized zoom requires typed digital-range permission");
        assert!(matches!(
            error,
            Error::FeatureNotSupported {
                feature: "optical-plus-digital zoom positioning",
                ..
            }
        ));
    }
    assert!(
        partial_fake.writes().is_empty(),
        "rejected dynamic nouns must not reach the transport"
    );

    // Explicit raw positions and optical-domain normalization remain direct
    // zoom controls, so the partial profile still admits both.
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
        .set_normalized(midpoint, ZoomDomain::Optical)
        .await
        .expect("explicit optical-domain normalization")
        .applied()
        .await
        .expect("explicit optical-domain target applied");
    assert_eq!(
        partial_fake.write_count(),
        2,
        "only the two direct/optical positive controls reach the transport"
    );
    partial_session
        .shutdown()
        .expect("partial session shutdown");

    // A profile that grants the same typed digital-range permission admits
    // both the overlap midpoint and the combined endpoint through the exact
    // same erased noun path.
    let supported_profile = documented_digital_zoom_profile(true);
    let supported_fake = camera();
    let supported_session = Session::open(
        supported_fake.async_wire(),
        &SessionConfig::new(supported_profile),
        runtime,
    )
    .await
    .expect("supported-profile session");
    let supported_camera = supported_session
        .camera::<grafton_visca::profiles::AnyProfile>(
            grafton_visca::types::IndividualCameraId::CAMERA_1,
        )
        .expect("supported dynamic camera");
    for position in [midpoint, UnitInterval::MAX] {
        supported_camera
            .zoom()
            .set_normalized(position, ZoomDomain::OpticalPlusDigital)
            .await
            .expect("combined normalized zoom with typed permission")
            .applied()
            .await
            .expect("combined normalized target applied");
    }
    assert_eq!(
        supported_fake.write_count(),
        2,
        "both supported combined-domain controls reach the transport"
    );
    supported_session
        .shutdown()
        .expect("supported session shutdown");
}

async fn dynamic_shared_exposure_modes_require_typed_permission<E>(runtime: E)
where
    E: grafton_visca::runtime::Executor,
{
    // Sony FR7 retains exposure controls, but does not document the shared
    // `04 39` mode family. The erased noun remains callable and rejects both
    // projections before the owner reaches its transport.
    let fr7_profile = ProfileSpec::from_compile_time::<SonyFr7>().expect("Sony FR7 profile");
    assert!(fr7_profile.capabilities().exposure.is_some());
    assert!(fr7_profile
        .capabilities()
        .exposure
        .as_ref()
        .expect("exposure domain")
        .modes
        .is_empty());
    assert!(!fr7_profile
        .capabilities()
        .supports(TypedSupportSurface::ExposureMode));

    let fr7_fake = camera();
    let fr7_session = Session::open(
        fr7_fake.async_wire(),
        &SessionConfig::new(fr7_profile),
        runtime.clone(),
    )
    .await
    .expect("Sony FR7 session");
    let fr7_camera = fr7_session
        .camera::<grafton_visca::profiles::AnyProfile>(
            grafton_visca::types::IndividualCameraId::CAMERA_1,
        )
        .expect("Sony FR7 camera");
    let fr7_nouns = &fr7_camera;

    let error = fr7_nouns
        .exposure()
        .set_mode(ExposureMode::Auto)
        .await
        .expect_err("FR7 shared exposure-mode command must be rejected");
    assert!(matches!(
        error,
        Error::FeatureNotSupported {
            feature: "shared exposure-mode family",
            ..
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
            feature: "shared exposure-mode family",
            ..
        }
    ));
    assert!(
        fr7_fake.writes().is_empty(),
        "both rejected Sony FR7 exposure-mode noun calls must stay preflight"
    );
    fr7_session.shutdown().expect("Sony FR7 session shutdown");

    // This is mutation-sensitive: the runtime profile has every discovery
    // fact that makes the shared mode family physically plausible, but omits
    // only its typed permission bit.
    let partial_profile = documented_exposure_mode_profile_without_typed_permission();
    assert!(partial_profile.capabilities().exposure.is_some());
    assert!(!partial_profile
        .capabilities()
        .exposure
        .as_ref()
        .expect("exposure domain")
        .modes
        .is_empty());
    assert!(!partial_profile
        .capabilities()
        .supports(TypedSupportSurface::ExposureMode));

    let partial_fake = camera();
    let partial_session = Session::open(
        partial_fake.async_wire(),
        &SessionConfig::new(partial_profile),
        runtime,
    )
    .await
    .expect("partial exposure-mode session");
    let partial_camera = partial_session
        .camera::<grafton_visca::profiles::AnyProfile>(
            grafton_visca::types::IndividualCameraId::CAMERA_1,
        )
        .expect("partial exposure-mode camera");
    let partial_nouns = &partial_camera;

    let error = partial_nouns
        .exposure()
        .set_mode(ExposureMode::Auto)
        .await
        .expect_err("metadata alone must not admit shared exposure-mode command");
    assert!(matches!(
        error,
        Error::FeatureNotSupported {
            feature: "shared exposure-mode family",
            ..
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
            feature: "shared exposure-mode family",
            ..
        }
    ));
    assert!(
        partial_fake.writes().is_empty(),
        "both rejected partial-profile exposure-mode noun calls must stay preflight"
    );
    partial_session
        .shutdown()
        .expect("partial exposure-mode session shutdown");
}

async fn evidence_refinements_predict_every_withheld_builtin_call<E>(runtime: E)
where
    E: grafton_visca::runtime::Executor,
{
    use grafton_visca::{
        profiles::*,
        types::{GainLevel, GainLimit, PresetNumber, ShutterSpeed},
    };
    let profiles = [
        ProfileSpec::from_compile_time::<PtzOpticsG2>().unwrap(),
        ProfileSpec::from_compile_time::<PtzOpticsG3>().unwrap(),
        ProfileSpec::from_compile_time::<PtzOptics30x>().unwrap(),
        ProfileSpec::from_compile_time::<SonyFr7>().unwrap(),
        ProfileSpec::from_compile_time::<SonyBrcH900>().unwrap(),
        ProfileSpec::from_compile_time::<SonyEviH100>().unwrap(),
        ProfileSpec::from_compile_time::<SonyBrc300>().unwrap(),
        ProfileSpec::from_compile_time::<NearusBrc300>().unwrap(),
        ProfileSpec::from_compile_time::<GenericVisca>().unwrap(),
    ];
    let mut refusals = 0;
    for profile in profiles {
        let fake = camera();
        let session = Session::open(
            fake.async_wire(),
            &SessionConfig::new(profile.clone()),
            runtime.clone(),
        )
        .await
        .unwrap();
        let camera = session
            .camera::<grafton_visca::profiles::AnyProfile>(
                grafton_visca::types::IndividualCameraId::CAMERA_1,
            )
            .unwrap();
        macro_rules! refused {
            ($request:expr) => {{
                assert!(
                    matches!($request.await, Err(Error::FeatureNotSupported { .. })),
                    "{}: {}",
                    profile.name(),
                    stringify!($request)
                );
                refusals += 1;
            }};
        }
        if !profile
            .capabilities()
            .supports(TypedSupportSurface::PowerStandby)
        {
            refused!(camera.power().standby());
        }
        if !profile
            .capabilities()
            .supports(TypedSupportSurface::FocusInfinity)
        {
            refused!(camera.focus().infinity());
        }
        if !profile
            .capabilities()
            .supports(TypedSupportSurface::PresetMemory)
        {
            refused!(camera.presets().store(PresetNumber::MIN));
            refused!(camera.presets().clear(PresetNumber::MIN));
        }
        if !profile
            .capabilities()
            .supports(TypedSupportSurface::Shutter)
        {
            refused!(camera.exposure().shutter_reset());
            refused!(camera.exposure().shutter_up());
            refused!(camera.exposure().shutter_down());
            refused!(camera.exposure().shutter());
        }
        if !profile
            .capabilities()
            .supports(TypedSupportSurface::DirectShutter)
        {
            refused!(camera
                .exposure()
                .set_shutter(ShutterSpeed::new(0).expect("valid value")));
        }
        if !profile.capabilities().supports(TypedSupportSurface::Gain) {
            refused!(camera.exposure().gain_reset());
            refused!(camera.exposure().gain_up());
            refused!(camera.exposure().gain_down());
            refused!(camera.exposure().set_gain(GainLevel::MIN));
            refused!(camera.exposure().gain());
        }
        if !profile
            .capabilities()
            .supports(TypedSupportSurface::GainLimit)
        {
            refused!(camera.exposure().set_gain_limit(GainLimit::MIN));
            refused!(camera.exposure().gain_limit());
        }
        for (mode, operation) in [
            (grafton_visca::types::WhiteBalanceMode::Auto, 0),
            (grafton_visca::types::WhiteBalanceMode::Indoor, 1),
            (grafton_visca::types::WhiteBalanceMode::Outdoor, 2),
        ] {
            if !profile
                .capabilities()
                .white_balance
                .as_ref()
                .is_some_and(|facts| facts.modes.contains(&mode))
            {
                match operation {
                    0 => refused!(camera
                        .white_balance()
                        .set_mode(grafton_visca::types::WhiteBalanceMode::Auto)),
                    1 => refused!(camera
                        .white_balance()
                        .set_mode(grafton_visca::types::WhiteBalanceMode::Indoor)),
                    _ => refused!(camera
                        .white_balance()
                        .set_mode(grafton_visca::types::WhiteBalanceMode::Outdoor)),
                }
            }
        }
        if !profile
            .capabilities()
            .supports(TypedSupportSurface::MenuDisplay)
        {
            refused!(camera.menu().set_visible(true));
        }
        if !profile
            .capabilities()
            .supports(TypedSupportSurface::MenuEnterReturn)
        {
            refused!(camera.menu().enter());
            refused!(camera.menu().back());
        }
        if !profile
            .capabilities()
            .supports(TypedSupportSurface::MenuDisplayInquiry)
        {
            refused!(camera.menu().visible());
        }
        assert!(
            fake.writes().is_empty(),
            "{} refused before transport I/O",
            profile.name()
        );
        session.shutdown().unwrap();
    }
    assert_eq!(refusals, 59);
}

async fn typed_views_refuse_other_builtins_and_unidentified_custom_profiles<E>(runtime: E)
where
    E: grafton_visca::runtime::Executor,
{
    use grafton_visca::profiles::{AnyProfile, ProfileId, PtzOpticsG2};

    let profile = ProfileId::PtzOpticsG2.spec().expect("G2 profile");
    let mut capabilities = profile.capabilities().clone();
    capabilities.profile_id = None;
    let custom = runtime_with_capabilities(&profile, capabilities);
    for registered in [profile, custom] {
        let session = Session::open(
            camera().async_wire(),
            &SessionConfig::new(registered),
            runtime.clone(),
        )
        .await
        .expect("session");
        assert!(session
            .camera::<SonyFr7>(grafton_visca::types::IndividualCameraId::CAMERA_1)
            .is_err());
        let camera = session
            .camera::<AnyProfile>(grafton_visca::types::IndividualCameraId::CAMERA_1)
            .expect("view");
        assert_eq!(
            camera.clone().into_typed::<PtzOpticsG2>().is_ok(),
            camera.profile().capabilities().profile_id == Some(ProfileId::PtzOpticsG2),
        );
        session.close().await.expect("session closes");
    }
}

runtime_matrix!(
    typed_views_refuse_other_builtins_and_unidentified_custom_profiles,
    dynamic_targeted_projection_uses_owner_settlement_wait,
    dynamic_combined_normalized_zoom_requires_typed_digital_range,
    dynamic_shared_exposure_modes_require_typed_permission,
    evidence_refinements_predict_every_withheld_builtin_call,
);
