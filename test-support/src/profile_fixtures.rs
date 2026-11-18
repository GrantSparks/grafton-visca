//! Synthetic compile-time profiles and their validated runtime specifications.
//!
//! Fixtures deliberately use the same public builder as community profiles.
//! Timing and physical facts are explicit so lifecycle tests do not inherit
//! changes to a built-in camera's defaults.

use std::time::Duration;

use grafton_visca::{
    capabilities::{
        self, Capabilities, CapabilityDomain, CapabilityRange, CoordinateSystem, Domain, DomainSet,
        EndpointQuantization, ExposureCaps, FocusCaps, FocusNearEnd, ImageCaps, InquirySupport,
        MenuCaps, PanTiltCaps, PanTiltWireCodec, PowerCaps, PresetCaps, RgbGainRanges,
        RgbTuningRanges, ShutterSpeedEntry, TiltRanges, TransportSet, TypedSupportSet,
        TypedSupportSurface, WhiteBalanceCaps, ZoomCaps,
    },
    config::SessionConfig,
    profiles::{
        CommandTimeouts, CompileTimeProfile, PanTiltCoordinateConversion, ProfileEnvelope,
        ProfileId, ProfileSpec, ProfileTiming, TransportCompatibility,
    },
    types::{AffectedAxes, ExposureMode, FocusZone, WhiteBalanceMode},
};

const SYNTHETIC_TRANSPORTS: TransportCompatibility = TransportCompatibility::new()
    .with_tcp(5678)
    .with_udp(1259)
    .with_serial();

fn synthetic_spec(
    model: &str,
    typed_support: TypedSupportSet,
    ambiguity_timeout: Duration,
    command_timeouts: CommandTimeouts,
) -> grafton_visca::Result<ProfileSpec> {
    use TypedSupportSurface as S;
    let mut caps = Capabilities::runtime_baseline(model, 1)?;
    caps.typed_support = typed_support;
    caps.inquiry_support = InquirySupport::Full;
    caps.pan_tilt = Some(PanTiltCaps {
        geometry: Some(grafton_visca::capabilities::PanTiltGeometry {
            pan_range: CapabilityRange::<i32>::new(-1700, 1700)?,
            tilt_ranges: TiltRanges::Single(CapabilityRange::<i32>::new(-300, 900)?),
            coordinates: PanTiltCoordinateConversion::new(
                CoordinateSystem::SignedCentered,
                PanTiltWireCodec::StandardVisca,
                10.0,
                10.0,
            )?,
            endpoint_quantization: EndpointQuantization::Exact,
        }),
        pan_speed: CapabilityRange::<u8>::new(1, 24)?,
        tilt_speed: CapabilityRange::<u8>::new(1, 20)?,
        simultaneous: true,
        preset_recovery_time: Duration::ZERO,
        variable_speed: false,
        motion_sync_speed_range: typed_support
            .contains(S::MotionSync)
            .then_some(CapabilityRange::<u8>::new(1, 24)?),
    });
    caps.zoom = Some(ZoomCaps {
        optical_max: 0x4000,
        digital_max: Some(0x7000),
        speed: CapabilityRange::<u8>::new(0, 7)?,
        direct: true,
        variable: true,
        optical_ratio: None,
    });
    caps.focus = Some(FocusCaps {
        range: CapabilityRange::<u16>::new(0x1000, 0xf000)?,
        near_end: FocusNearEnd::Minimum,
        speed: CapabilityRange::<u8>::new(0, 7)?,
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
            .then_some(CapabilityRange::<i8>::new(-7, 7)?),
        iris_range: typed_support
            .contains(S::Iris)
            .then_some(CapabilityDomain::<u16>::new(0, 30, &[][..])?),
        gain_range: Some(CapabilityRange::<u8>::new(0, 15)?),
        shutter_speeds: vec![ShutterSpeedEntry::new(
            grafton_visca::types::ExposureTime::new(1, 60)?,
            0x01,
        )],
        brightness_range: typed_support
            .contains(S::Brightness)
            .then_some(CapabilityDomain::<u8>::new(0, 0xff, &[][..])?),
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
        color_temp_range: color_temperature.then_some(CapabilityRange::<u16>::new(2500, 8000)?),
        color_temperature_steps: if color_temperature {
            (0..=55)
                .map(|step| 2500 + 100 * step)
                .collect::<Vec<_>>()
                .into()
        } else {
            std::sync::Arc::from([])
        },
        rgb_gain: typed_support.contains(S::RgbGain).then_some(RgbGainRanges {
            red: CapabilityRange::<u8>::new(0, 0xff)?,
            blue: CapabilityRange::<u8>::new(0, 0xff)?,
        }),
        rgb_tuning: typed_support
            .contains(S::RgbTuning)
            .then_some(RgbTuningRanges {
                red: CapabilityRange::<i8>::new(-10, 10)?,
                blue: CapabilityRange::<i8>::new(-10, 10)?,
            }),
    });
    // This broad fixture exists only to instantiate generic noun contracts.
    // Unlike the old metadata-trait fixture, its granted surfaces have facts.
    if typed_support.contains(S::Contrast) {
        caps.image = Some(ImageCaps {
            contrast_range: Some(CapabilityRange::<u8>::new(0, 14)?),
            sharpness_range: Some(CapabilityRange::<u8>::new(0, 15)?),
            saturation_range: Some(CapabilityRange::<u8>::new(0, 14)?),
            hue_range: Some(CapabilityRange::<u8>::new(0, 14)?),
            luminance_range: Some(CapabilityRange::<u8>::new(0, 14)?),
            gamma_range: Some(CapabilityRange::<u8>::new(0, 4)?),
            flip: true,
            mirror: true,
            requires_settings_save_for_flip: false,
            picture_effects: vec![
                grafton_visca::types::PictureEffectMode::Off,
                grafton_visca::types::PictureEffectMode::BlackAndWhite,
            ],
        });
    }
    caps.presets = Some(PresetCaps {
        highest: 6,
        recall_axes: AffectedAxes::PAN_TILT
            .union(AffectedAxes::ZOOM)
            .union(AffectedAxes::FOCUS),
        recall_speed_range: Some(CapabilityRange::<u8>::new(1, 23)?),
        tour: false,
        thumbnail: false,
        recall_delay: Duration::ZERO,
        names: None,
    });
    caps.power = Some(PowerCaps {
        status_query: grafton_visca::capabilities::PowerStatusQuery::Standard,
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
    // These profiles settle targeted operations by polling. Scalar controls
    // need their own query only when the fixture declares that permission.
    let mut position_inquiries = AffectedAxes::MOVEMENT;
    if typed_support.contains(S::Iris) {
        position_inquiries = position_inquiries.union(AffectedAxes::IRIS);
    }
    if typed_support.contains(S::NdFilter) {
        position_inquiries = position_inquiries.union(AffectedAxes::ND_FILTER);
    }
    let mut builder = ProfileSpec::builder(caps)
        .transports(SYNTHETIC_TRANSPORTS)
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
        .maximum_command_sockets(grafton_visca::types::CommandSocketCount::TWO)
        .supports_operation_complete(false)
        .supports_command_cancel(true)
        .position_inquiries(position_inquiries);
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

macro_rules! synthetic_profile_default {
    (default, $profile:ident) => {
        impl Default for $profile {
            fn default() -> Self {
                Self
            }
        }
    };
    (no_default, $profile:ident) => {};
}

macro_rules! synthetic_command_timeouts {
    () => {
        CommandTimeouts::DEFAULT
            .with_quick_timeout(Duration::from_secs(5))
            .with_movement_timeout(Duration::from_secs(30))
            .with_preset_timeout(Duration::from_secs(60))
            .with_long_running_timeout(Duration::from_secs(300))
            .with_network_timeout(Duration::from_secs(5))
    };
    ($command_timeouts:expr) => {
        $command_timeouts
    };
}

macro_rules! synthetic_profile_impl {
    ($profile:ident, $model:literal, $typed_support:expr, $default:ident, $ambiguity_timeout:expr $(, $command_timeouts:expr)?) => {
        #[derive(Debug, Clone, Copy)]
        pub struct $profile;
        synthetic_profile_default!($default, $profile);
        impl CompileTimeProfile for $profile {
            const DOMAINS: DomainSet = {
                let mut domains = DomainSet::from_domains(&[
                    Domain::PanTilt, Domain::Zoom, Domain::Focus, Domain::Presets,
                    Domain::Power, Domain::Exposure, Domain::WhiteBalance,
                ]);
                if $typed_support.contains(TypedSupportSurface::Contrast) {
                    domains = domains.with(Domain::Image);
                }
                if $typed_support.contains(TypedSupportSurface::MenuDisplay) {
                    domains = domains.with(Domain::Menu);
                }
                domains
            };
            const TRANSPORTS: TransportSet = SYNTHETIC_TRANSPORTS.transport_set();
            const TYPED_SUPPORT: TypedSupportSet = $typed_support.union(TypedSupportSet::from_surface(TypedSupportSurface::PanTiltPositioning));
            const ID: Option<ProfileId> = None;
            fn spec() -> grafton_visca::Result<ProfileSpec> {
                synthetic_spec($model, <Self as CompileTimeProfile>::TYPED_SUPPORT, $ambiguity_timeout,
                    synthetic_command_timeouts!($($command_timeouts)?))
            }
        }
        impl capabilities::HasPower for $profile {}
        impl capabilities::HasPanTilt for $profile {}
        impl capabilities::HasPanTiltPositioning for $profile {}
        impl capabilities::HasZoom for $profile {}
        impl capabilities::HasFocus for $profile {}
        impl capabilities::HasPresets for $profile {}
        impl capabilities::HasExposure for $profile {}
        impl capabilities::HasWhiteBalance for $profile {}
    };
}

macro_rules! synthetic_profile {
    ($profile:ident, $model:literal, $typed_support:expr) => {
        synthetic_profile_impl!(
            $profile,
            $model,
            $typed_support,
            default,
            Duration::from_secs(1)
        );
    };
    ($profile:ident, $model:literal, $typed_support:expr, no_default) => {
        synthetic_profile_impl!(
            $profile,
            $model,
            $typed_support,
            no_default,
            Duration::from_secs(1)
        );
    };
}

synthetic_profile!(
    DirectZoomOnlyTypedSupport,
    "Direct Zoom Typed Support Only",
    TypedSupportSet::from_surface(TypedSupportSurface::DirectZoom)
);

// Explicit caller-owned combined zoom facts for protocol/noun tests. This is
// a custom profile (no built-in identity), not a claim about a Sony model.
synthetic_profile!(
    CombinedZoomTypedSupport,
    "Explicit Combined Zoom Fixture",
    TypedSupportSet::from_surfaces(&[
        TypedSupportSurface::DirectZoom,
        TypedSupportSurface::DigitalZoomRange,
    ])
);

impl capabilities::HasDirectZoom for CombinedZoomTypedSupport {}
impl capabilities::HasDigitalZoomRange for CombinedZoomTypedSupport {}

synthetic_profile!(
    NonDefaultCompileTimeProfile,
    "Non-Default Compile-Time Profile",
    TypedSupportSet::EMPTY,
    no_default
);

// Motion-owner tests intentionally chain successful raw position inquiries
// under exact one-second observer deadlines. Keep the production-style
// one-second pre-ACK ambiguity fact: matched inquiries no longer misuse it as
// a post-success dispatch hold (#712).
synthetic_profile_impl!(
    MotionOwnerCompileTimeProfile,
    "Motion Owner Compile-Time Profile",
    TypedSupportSet::EMPTY,
    no_default,
    Duration::from_secs(1)
);

// Socket-reuse integration tests need a real owner timeout to install an
// exact-socket quarantine without making the test wait for a production-scale
// movement deadline. The distinct fixture keeps that timing fact local to
// those tests.
synthetic_profile_impl!(
    QuarantinedSocketCompileTimeProfile,
    "Quarantined Socket Compile-Time Profile",
    TypedSupportSet::EMPTY,
    no_default,
    Duration::from_millis(250),
    CommandTimeouts::DEFAULT
        .with_quick_timeout(Duration::from_millis(100))
        .with_movement_timeout(Duration::from_millis(100))
        .with_preset_timeout(Duration::from_millis(100))
        .with_long_running_timeout(Duration::from_millis(100))
        .with_network_timeout(Duration::from_millis(100))
);

// No built-in profile declares motion sync (see the profile registry's
// `MotionSync` note), so the typed accessor is only reachable from a profile
// that opts in — which is what makes a behavioural test of the helper possible
// at all.
synthetic_profile!(
    MotionSyncTypedSupport,
    "Motion Sync Typed Support Only",
    TypedSupportSet::from_surface(TypedSupportSurface::MotionSync)
);

// The generic noun contracts in the suite bound a profile on many typed
// surfaces at once, and no built-in profile documents all of them (none
// documents motion sync at all). This fixture declares every surface those
// contracts name, so each contract is instantiated rather than only
// type-checked in the abstract.
synthetic_profile!(
    OptionalSurfacesTypedSupport,
    "Optional Surfaces Typed Support",
    TypedSupportSet::from_surfaces(&[
        TypedSupportSurface::PowerStandby,
        TypedSupportSurface::DirectShutter,
        TypedSupportSurface::FocusInfinity,
        TypedSupportSurface::PresetMemory,
        TypedSupportSurface::Shutter,
        TypedSupportSurface::Gain,
        TypedSupportSurface::GainLimit,
        TypedSupportSurface::MenuDisplay,
        TypedSupportSurface::MenuEnterReturn,
        TypedSupportSurface::MenuDisplayInquiry,
        TypedSupportSurface::AutoFocusSensitivity,
        TypedSupportSurface::AutoWhiteBalanceSensitivity,
        TypedSupportSurface::BacklightCompensation,
        TypedSupportSurface::Brightness,
        TypedSupportSurface::ColorTemperature,
        TypedSupportSurface::ColorTemperatureInquiry,
        TypedSupportSurface::Contrast,
        TypedSupportSurface::ExposureCompensation,
        TypedSupportSurface::FocusNearLimit,
        TypedSupportSurface::FocusZoneInquiry,
        TypedSupportSurface::Gamma,
        TypedSupportSurface::Hue,
        TypedSupportSurface::ImageFlip,
        TypedSupportSurface::Iris,
        TypedSupportSurface::Luminance,
        TypedSupportSurface::MotionSync,
        TypedSupportSurface::NdFilter,
        TypedSupportSurface::NoiseReduction2D,
        TypedSupportSurface::NoiseReduction2DMode,
        TypedSupportSurface::NoiseReduction3D,
        TypedSupportSurface::PictureEffect,
        TypedSupportSurface::RgbGain,
        TypedSupportSurface::RgbTuning,
        TypedSupportSurface::Saturation,
        TypedSupportSurface::Sharpness,
        TypedSupportSurface::SharpnessMode,
        TypedSupportSurface::Tally,
        TypedSupportSurface::WideDynamicRange,
    ])
);

impl capabilities::HasDirectZoom for DirectZoomOnlyTypedSupport {}

// This profile supplies the motion-sync speed range as well as its permission.
impl capabilities::HasMotionSync for MotionSyncTypedSupport {}

impl capabilities::HasAutoFocusSensitivity for OptionalSurfacesTypedSupport {}
impl capabilities::HasAutoWhiteBalanceSensitivity for OptionalSurfacesTypedSupport {}
impl capabilities::HasBacklightCompensation for OptionalSurfacesTypedSupport {}
impl capabilities::HasBrightness for OptionalSurfacesTypedSupport {}
impl capabilities::HasColorTemperature for OptionalSurfacesTypedSupport {}
impl capabilities::HasColorTemperatureInquiry for OptionalSurfacesTypedSupport {}
impl capabilities::HasContrast for OptionalSurfacesTypedSupport {}
impl capabilities::HasExposureCompensation for OptionalSurfacesTypedSupport {}
impl capabilities::HasFocusNearLimit for OptionalSurfacesTypedSupport {}
impl capabilities::HasFocusZoneInquiry for OptionalSurfacesTypedSupport {}
impl capabilities::HasGamma for OptionalSurfacesTypedSupport {}
impl capabilities::HasHue for OptionalSurfacesTypedSupport {}
impl capabilities::HasImageFlip for OptionalSurfacesTypedSupport {}
impl capabilities::HasIris for OptionalSurfacesTypedSupport {}
impl capabilities::HasLuminance for OptionalSurfacesTypedSupport {}
impl capabilities::HasMotionSync for OptionalSurfacesTypedSupport {}
impl capabilities::HasNdFilter for OptionalSurfacesTypedSupport {}
impl capabilities::HasNoiseReduction2D for OptionalSurfacesTypedSupport {}
impl capabilities::HasNoiseReduction2DMode for OptionalSurfacesTypedSupport {}
impl capabilities::HasNoiseReduction3D for OptionalSurfacesTypedSupport {}
impl capabilities::HasPictureEffect for OptionalSurfacesTypedSupport {}
impl capabilities::HasRgbGain for OptionalSurfacesTypedSupport {}
impl capabilities::HasRgbTuning for OptionalSurfacesTypedSupport {}
impl capabilities::HasSaturation for OptionalSurfacesTypedSupport {}
impl capabilities::HasSharpness for OptionalSurfacesTypedSupport {}
impl capabilities::HasSharpnessMode for OptionalSurfacesTypedSupport {}
impl capabilities::HasTally for OptionalSurfacesTypedSupport {}
impl capabilities::HasWideDynamicRange for OptionalSurfacesTypedSupport {}
impl capabilities::HasImage for OptionalSurfacesTypedSupport {}

/// A session on camera 1 with [`NonDefaultCompileTimeProfile`], the
/// two-socket raw profile.
pub fn session_config() -> SessionConfig {
    SessionConfig::new(
        ProfileSpec::from_compile_time::<NonDefaultCompileTimeProfile>()
            .expect("two-socket raw profile"),
    )
}

/// [`session_config`] with camera 2 registered on the same two-socket raw
/// profile.
pub fn two_camera_session_config() -> SessionConfig {
    session_config()
        .with_target(
            grafton_visca::types::IndividualCameraId::CAMERA_2,
            ProfileSpec::from_compile_time::<NonDefaultCompileTimeProfile>()
                .expect("two-socket raw profile"),
        )
        .expect("second target")
}

/// A session on camera 1 with the built-in Sony FR7 profile (Sony envelope).
pub fn sony_session_config() -> SessionConfig {
    SessionConfig::new(
        ProfileSpec::from_compile_time::<grafton_visca::profiles::SonyFr7>()
            .expect("Sony FR7 profile"),
    )
}

impl capabilities::HasFocusInfinity for OptionalSurfacesTypedSupport {}
impl capabilities::HasPresetMemory for OptionalSurfacesTypedSupport {}
impl capabilities::HasShutter for OptionalSurfacesTypedSupport {}
impl capabilities::HasGain for OptionalSurfacesTypedSupport {}
impl capabilities::HasGainLimit for OptionalSurfacesTypedSupport {}

impl capabilities::HasMenuDisplay for OptionalSurfacesTypedSupport {}
impl capabilities::HasMenuEnterReturn for OptionalSurfacesTypedSupport {}
impl capabilities::HasMenuDisplayInquiry for OptionalSurfacesTypedSupport {}

impl capabilities::HasDirectShutter for OptionalSurfacesTypedSupport {}

impl capabilities::HasPowerStandby for OptionalSurfacesTypedSupport {}

impl capabilities::HasMenu for OptionalSurfacesTypedSupport {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_synthetic_profile_has_a_valid_checked_specification() -> grafton_visca::Result<()> {
        for profile in [
            ProfileSpec::from_compile_time::<DirectZoomOnlyTypedSupport>()?,
            ProfileSpec::from_compile_time::<CombinedZoomTypedSupport>()?,
            ProfileSpec::from_compile_time::<NonDefaultCompileTimeProfile>()?,
            ProfileSpec::from_compile_time::<MotionOwnerCompileTimeProfile>()?,
            ProfileSpec::from_compile_time::<QuarantinedSocketCompileTimeProfile>()?,
            ProfileSpec::from_compile_time::<MotionSyncTypedSupport>()?,
            ProfileSpec::from_compile_time::<OptionalSurfacesTypedSupport>()?,
        ] {
            assert_eq!(profile.capabilities().profile_id, None);
            assert_eq!(
                profile.maximum_command_sockets(),
                grafton_visca::types::CommandSocketCount::TWO
            );
            assert!(!profile.supports_operation_complete());
            assert!(profile.supports_command_cancel());
        }
        Ok(())
    }

    #[test]
    fn synthetic_polling_queries_match_the_declared_scalar_axes() -> grafton_visca::Result<()> {
        for (support, queries) in [
            (TypedSupportSet::EMPTY, AffectedAxes::MOVEMENT),
            (
                TypedSupportSet::from_surface(TypedSupportSurface::Iris),
                AffectedAxes::MOVEMENT.union(AffectedAxes::IRIS),
            ),
            (
                TypedSupportSet::from_surface(TypedSupportSurface::NdFilter),
                AffectedAxes::MOVEMENT.union(AffectedAxes::ND_FILTER),
            ),
            (
                TypedSupportSet::from_surfaces(&[
                    TypedSupportSurface::Iris,
                    TypedSupportSurface::NdFilter,
                ]),
                AffectedAxes::ALL,
            ),
        ] {
            let profile = synthetic_spec(
                "Scalar polling fixture",
                support,
                Duration::from_secs(1),
                CommandTimeouts::DEFAULT,
            )?;
            assert_eq!(profile.capabilities().typed_support, support);
            assert_eq!(profile.position_inquiries(), Some(queries));
            assert!(!profile.supports_operation_complete());
        }
        Ok(())
    }

    #[test]
    fn consuming_target_updates_preserve_both_fixture_registrations() {
        let config = two_camera_session_config();
        assert_eq!(
            config.targets().collect::<Vec<_>>(),
            [
                grafton_visca::types::IndividualCameraId::CAMERA_1,
                grafton_visca::types::IndividualCameraId::CAMERA_2
            ]
        );
        assert_eq!(
            config
                .profile(grafton_visca::types::IndividualCameraId::CAMERA_1)
                .expect("first registration"),
            config
                .profile(grafton_visca::types::IndividualCameraId::CAMERA_2)
                .expect("second registration")
        );
        assert_eq!(config.sole_target(), None);
    }

    #[test]
    fn synthetic_shared_concepts_select_their_wire_families_explicitly() -> grafton_visca::Result<()>
    {
        let support = TypedSupportSet::from_surfaces(&[
            TypedSupportSurface::PresetRecallSpeed,
            TypedSupportSurface::OnePushWhiteBalance,
        ]);
        let profile = synthetic_spec(
            "Explicit encoding fixture",
            support,
            Duration::from_secs(1),
            CommandTimeouts::DEFAULT,
        )?;
        assert_eq!(
            profile.encodings().preset_recall_speed(),
            Some(capabilities::PresetRecallSpeedEncoding::PtzOptics)
        );
        assert_eq!(
            profile.encodings().one_push_white_balance(),
            Some(capabilities::OnePushWhiteBalanceEncoding::Standard)
        );
        Ok(())
    }
}
