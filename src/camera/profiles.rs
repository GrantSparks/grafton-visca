//! Camera profile implementations generated from the built-in profile registry.
//!
//! Public profile types remain ordinary zero-sized Rust structs. Their metadata,
//! runtime capability constants, typed support markers, profile IDs, profile
//! groups, evidence notes, and invariant tests are emitted from
//! `profile_registry`.

pub(crate) mod profile_constants {
    use crate::{
        capabilities::ShutterSpeedEntry,
        command::{exposure_wire::ExposureMode, FocusZone},
        units::ExposureTime,
        WhiteBalanceMode,
    };

    /// A `1/denominator` second table entry. Only the `const` tables below
    /// call it, so a zero denominator is a compile error, never a runtime
    /// panic.
    const fn entry(denominator: u32, value: u8) -> ShutterSpeedEntry {
        ShutterSpeedEntry::new(ExposureTime::reciprocal(denominator), value)
    }

    /// PTZOptics G2-family shutter codes. The source register in
    /// `docs/visca_reference.md` documents only `pq = Shutter Position`, not
    /// this code-to-time table; it is retained as shipped, unsourced.
    pub const PTZ_OPTICS_G2_SHUTTER_SPEEDS: &[ShutterSpeedEntry] = &[
        entry(30, 0x01),
        entry(60, 0x02),
        entry(90, 0x03),
        entry(100, 0x04),
        entry(125, 0x05),
        entry(180, 0x06),
        entry(250, 0x07),
        entry(350, 0x08),
        entry(500, 0x09),
        entry(725, 0x0A),
        entry(1000, 0x0B),
        entry(1500, 0x0C),
        entry(2000, 0x0D),
        entry(3000, 0x0E),
        entry(4000, 0x0F),
        entry(6000, 0x10),
        entry(10000, 0x11),
    ];

    /// Sony EVI-H100 shutter codes (R8, VISCA Command Setting Values,
    /// exposure control 1/2, 60/30 mode column).
    pub const SONY_EVI_H100_SHUTTER_SPEEDS: &[ShutterSpeedEntry] = &[
        entry(1, 0x00),
        entry(2, 0x01),
        entry(4, 0x02),
        entry(8, 0x03),
        entry(15, 0x04),
        entry(30, 0x05),
        entry(60, 0x06),
        entry(90, 0x07),
        entry(100, 0x08),
        entry(125, 0x09),
        entry(180, 0x0A),
        entry(250, 0x0B),
        entry(350, 0x0C),
        entry(500, 0x0D),
        entry(725, 0x0E),
        entry(1000, 0x0F),
        entry(1500, 0x10),
        entry(2000, 0x11),
        entry(3000, 0x12),
        entry(4000, 0x13),
        entry(6000, 0x14),
        entry(10000, 0x15),
    ];

    /// Sony BRC-300 shutter codes (R12 and the Nearus text R21, BRC-300
    /// column), which are also the codes R8, R12 and R21 share and so the
    /// Generic VISCA table.
    pub const SONY_BRC300_SHUTTER_SPEEDS: &[ShutterSpeedEntry] = &[
        entry(4, 0x02),
        entry(8, 0x03),
        entry(15, 0x04),
        entry(30, 0x05),
        entry(60, 0x06),
        entry(90, 0x07),
        entry(100, 0x08),
        entry(125, 0x09),
        entry(180, 0x0A),
        entry(250, 0x0B),
        entry(350, 0x0C),
        entry(500, 0x0D),
        entry(725, 0x0E),
        entry(1000, 0x0F),
        entry(1500, 0x10),
        entry(2000, 0x11),
        entry(3000, 0x12),
        entry(4000, 0x13),
        entry(6000, 0x14),
        entry(10000, 0x15),
    ];

    /// Profiles with no registered model-specific shutter-code inventory.
    pub const NO_SHUTTER_SPEEDS: &[ShutterSpeedEntry] = &[];

    /// Documented PTZ color-temperature values, indexed by wire step.
    pub const PTZ_COLOR_TEMPERATURE_STEPS: &[u16] = &[
        2500, 2600, 2700, 2800, 2900, 3000, 3100, 3200, 3300, 3400, 3500, 3600, 3700, 3800, 3900,
        4000, 4100, 4200, 4300, 4400, 4500, 4600, 4700, 4800, 4900, 5000, 5100, 5200, 5300, 5400,
        5500, 5600, 5700, 5800, 5900, 6000, 6100, 6200, 6300, 6400, 6500, 6600, 6700, 6800, 6900,
        7000, 7100, 7200, 7300, 7400, 7500, 7600, 7700, 7800, 7900, 8000,
    ];

    /// The five shared, source-backed AE modes.
    pub const STANDARD_EXPOSURE_MODES: &[ExposureMode] = &[
        ExposureMode::Auto,
        ExposureMode::Manual,
        ExposureMode::Shutter,
        ExposureMode::Iris,
        ExposureMode::Bright,
    ];

    /// Shared modes listed by the BRC-H900 command and inquiry tables (R11).
    pub const BRC_H900_EXPOSURE_MODES: &[ExposureMode] = &[
        ExposureMode::Auto,
        ExposureMode::Manual,
        ExposureMode::Shutter,
        ExposureMode::Iris,
    ];

    /// Profiles with no source-backed support for the shared `04 39` AE-mode
    /// command and inquiry family.
    pub const NO_SHARED_EXPOSURE_MODES: &[ExposureMode] = &[];

    /// Documented `CAM_AFZone` values plus `Zone03`, which the PTZOptics G2
    /// bench accepts and reads back (2026-10-04, #795).
    pub const PTZ_OPTICS_G2_FOCUS_ZONES: &[FocusZone] = &[
        FocusZone::Top,
        FocusZone::Center,
        FocusZone::Bottom,
        FocusZone::Zone03,
    ];

    /// The documented `CAM_AFZone` values only (R14/R20).
    pub const DOCUMENTED_FOCUS_ZONES: &[FocusZone] =
        &[FocusZone::Top, FocusZone::Center, FocusZone::Bottom];

    /// Profiles without focus-zone selection.
    pub const NO_FOCUS_ZONES: &[FocusZone] = &[];

    pub const STANDARD_WB_MODES: &[WhiteBalanceMode] = &[
        WhiteBalanceMode::Auto,
        WhiteBalanceMode::Indoor,
        WhiteBalanceMode::Outdoor,
        WhiteBalanceMode::OnePush,
        WhiteBalanceMode::Manual,
    ];

    /// The standard modes plus direct color-temperature white balance.
    pub const COLOR_TEMP_WB_MODES: &[WhiteBalanceMode] = &[
        WhiteBalanceMode::Auto,
        WhiteBalanceMode::Indoor,
        WhiteBalanceMode::Outdoor,
        WhiteBalanceMode::OnePush,
        WhiteBalanceMode::Manual,
        WhiteBalanceMode::ColorTemperature,
    ];
}

#[macro_use]
#[path = "profile_registry.rs"]
pub(crate) mod profile_registry;

builtin_profile_registry!(__define_builtin_profiles);

#[cfg(test)]
#[allow(clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn test_ptzoptics_g2_capabilities() {
        let spec = ProfileId::PtzOpticsG2.spec().expect("G2 profile");
        let pan = spec.capabilities().pan_tilt.as_ref().expect("pan/tilt");
        assert!(pan.pan_range.contains(0));
        assert!(pan.pan_range.contains(2448));
        assert!(!pan.pan_range.contains(2449));
        assert_eq!(pan.coordinates.pan_units(170.0), Some(2448));
        assert_eq!(pan.coordinates.pan_degrees(2448), 170.0);
    }

    /// PTZOptics G2 bench facts remain source-backed runtime records.
    #[test]
    fn ptzoptics_g2_geometry_matches_bench_facts() {
        let spec = ProfileId::PtzOpticsG2.spec().expect("G2 profile");
        let caps = spec.capabilities();
        let conversion = caps.pan_tilt.as_ref().expect("pan/tilt").coordinates;
        let zoom = caps.zoom.as_ref().expect("zoom");
        assert_eq!(conversion.pan_degrees_to_units(), 14.4);
        assert_eq!(conversion.tilt_degrees_to_units(), 14.4);
        assert_eq!(zoom.optical_max, 0x4000);
        assert_eq!(zoom.digital_max, None);
    }

    /// The shared conversion retains half-away-from-zero rounding (#808).
    #[test]
    fn pan_tilt_helpers_round_half_away_from_zero_like_the_request_path() {
        let spec = ProfileId::PtzOpticsG2.spec().expect("G2 profile");
        let conversion = spec.pan_tilt_coordinates().expect("pan/tilt");
        assert_eq!(conversion.pan_units(10.05), Some(145));
        assert_eq!(conversion.pan_units(0.05), Some(1));
        assert_eq!(conversion.pan_units(-0.05), Some(-1));
        assert_eq!(conversion.tilt_units(-10.05), Some(-145));
    }

    /// #808: raw positions convert with each camera's own scale. The removed
    /// profile-less helper applied the G2 scale to every camera, which made
    /// GenericVisca `2880` NaN.
    #[test]
    fn raw_positions_convert_with_the_profiles_own_scale() -> crate::Result<()> {
        fn position<P: crate::CompileTimeProfile>(
            pan: i32,
            tilt: i32,
        ) -> crate::Result<crate::types::PanTiltPosition> {
            crate::ProfileSpec::from_compile_time::<P>()?
                .pan_tilt_coordinates()
                .ok_or(crate::Error::FeatureNotSupported {
                    feature: "pan/tilt coordinate conversion",
                })?
                .to_degrees(crate::camera::PanTiltUnits::new(pan, tilt))
        }

        // R8's position table: pan `1E1B` is +170°, tilt `FC75` -20° and
        // `0FF0` +90°.
        let evi = position::<SonyEviH100>(0x1E1B, -0x038B)?;
        assert!((evi.pan.value() - 170.0).abs() < 0.01, "{}", evi.pan);
        assert!((evi.tilt.value() + 20.0).abs() < 0.01, "{}", evi.tilt);
        let evi_up = position::<SonyEviH100>(0, 0x0FF0)?;
        assert!((evi_up.tilt.value() - 90.0).abs() < 0.01, "{}", evi_up.tilt);
        let generic = position::<GenericVisca>(2880, -1440)?;
        assert_eq!((generic.pan.value(), generic.tilt.value()), (180.0, -90.0));
        let g2 = position::<PtzOpticsG2>(2448, 1296)?;
        assert_eq!((g2.pan.value(), g2.tilt.value()), (170.0, 90.0));
        Ok(())
    }

    /// #807: a shutter fraction maps through the profile's own table. The
    /// removed profile-independent conversion encoded 1/60 as `0x07` for
    /// every camera, and the Sony profiles shared a table that sent `05`
    /// (1/30 s on every Sony source) for 1/1000 s. Each code below is the
    /// model source's own: R8 for EVI-H100, R12/R21 for BRC-300 and Nearus,
    /// and their shared codes for Generic VISCA.
    #[test]
    fn shutter_fractions_resolve_through_each_profile_table() {
        use crate::{capabilities::Capabilities, units::ExposureTime};

        let code = |caps: &Capabilities, n, d| {
            ExposureTime::new(n, d)
                .ok()
                .and_then(|exposure| caps.shutter_speed_for(exposure).ok())
                .map(|s| s.value())
        };
        let g2 = ProfileId::PtzOpticsG2
            .spec()
            .expect("profile")
            .capabilities()
            .clone();
        assert_eq!(code(&g2, 1, 60), Some(0x02));
        assert_eq!(code(&g2, 1, 1000), Some(0x0B));
        assert_eq!(code(&g2, 2, 120), Some(0x02));
        assert_eq!(code(&g2, 1, 0), None);

        let evi = ProfileId::SonyEviH100
            .spec()
            .expect("profile")
            .capabilities()
            .clone();
        for (denominator, expected) in [
            (1, 0x00),
            (2, 0x01),
            (30, 0x05),
            (60, 0x06),
            (1000, 0x0F),
            (10000, 0x15),
        ] {
            assert_eq!(
                code(&evi, 1, denominator),
                Some(expected),
                "EVI-H100 1/{denominator}"
            );
        }
        for caps in [
            ProfileId::SonyBrc300
                .spec()
                .expect("profile")
                .capabilities()
                .clone(),
            ProfileId::NearusBrc300
                .spec()
                .expect("profile")
                .capabilities()
                .clone(),
            ProfileId::GenericVisca
                .spec()
                .expect("profile")
                .capabilities()
                .clone(),
        ] {
            assert_eq!(code(&caps, 1, 4), Some(0x02), "{}", caps.model_name);
            assert_eq!(code(&caps, 1, 30), Some(0x05), "{}", caps.model_name);
            assert_eq!(code(&caps, 1, 90), Some(0x07), "{}", caps.model_name);
            assert_eq!(code(&caps, 1, 1000), Some(0x0F), "{}", caps.model_name);
            assert_eq!(code(&caps, 1, 10000), Some(0x15), "{}", caps.model_name);
            assert_eq!(code(&caps, 1, 1), None, "{}", caps.model_name);
        }
        // No sourced table: no typed shutter code at all.
        for caps in [
            ProfileId::SonyFr7
                .spec()
                .expect("profile")
                .capabilities()
                .clone(),
            ProfileId::SonyBrcH900
                .spec()
                .expect("profile")
                .capabilities()
                .clone(),
        ] {
            assert!(
                caps.exposure
                    .as_ref()
                    .is_some_and(|domain| domain.shutter_speeds.is_empty()),
                "{}",
                caps.model_name
            );
            assert_eq!(code(&caps, 1, 60), None, "{}", caps.model_name);
        }
    }

    #[test]
    fn test_nd_filter_capability() {
        let spec = ProfileId::SonyFr7.spec().expect("FR7 profile");
        assert!(spec
            .capabilities()
            .supports(crate::capabilities::TypedSupportSurface::NdFilter));
        assert!(crate::types::NdFilterDensity::new(0x14).is_ok());
        assert!(crate::types::NdFilterDensity::new(0x15).is_err());
        assert!(crate::types::NdFilterDensity::new(128).is_err());
    }

    #[test]
    fn test_profile_metadata() {
        assert_eq!(ProfileId::PtzOpticsG2.display_name(), "PtzOptics G2");
        assert_eq!(ProfileId::SonyFr7.display_name(), "Sony FR7");
    }

    /// Every profile belongs to exactly the group that lists it. The literal
    /// membership is pinned once, in the closed inventory of
    /// `tests/integration/issue_548_supported_surface_inventory.rs`.
    #[test]
    fn every_profile_is_listed_by_exactly_its_group() {
        for profile in ProfileId::all() {
            let group = profile.profile_group();
            assert!(
                group.profiles().contains(profile),
                "{profile:?} not in {group:?}"
            );
        }
        let listed: usize = [
            ProfileGroup::GenericVisca,
            ProfileGroup::PtzOptics,
            ProfileGroup::SonyProfessional,
        ]
        .iter()
        .map(|group| group.profiles().len())
        .sum();
        assert_eq!(listed, ProfileId::all().len());
    }

    #[test]
    fn test_profile_group_display() {
        assert_eq!(ProfileGroup::GenericVisca.display_name(), "Generic VISCA");
        assert_eq!(ProfileGroup::PtzOptics.display_name(), "PtzOptics Series");
        assert_eq!(
            ProfileGroup::SonyProfessional.display_name(),
            "Sony Professional"
        );

        assert_eq!(format!("{}", ProfileGroup::GenericVisca), "Generic VISCA");
        assert_eq!(format!("{}", ProfileGroup::PtzOptics), "PtzOptics Series");
    }

    /// A group reports a transport or envelope fact only when every member
    /// has it, and its weakest member's inquiry support.
    #[test]
    fn group_facts_hold_for_every_member() {
        for group in [
            ProfileGroup::GenericVisca,
            ProfileGroup::PtzOptics,
            ProfileGroup::SonyProfessional,
        ] {
            let members = group.profiles();
            assert_eq!(
                group.supports_tcp(),
                members.iter().all(|id| id
                    .transport_compatibility()
                    .supports(crate::transport::StandardTransport::Tcp))
            );
            assert_eq!(
                group.supports_udp(),
                members.iter().all(|id| id
                    .transport_compatibility()
                    .supports(crate::transport::StandardTransport::Udp))
            );
            assert_eq!(
                group.supports_serial(),
                members
                    .iter()
                    .all(|id| id.transport_compatibility().supports_serial())
            );
            assert_eq!(
                group.uses_sony_encapsulation(),
                members.iter().all(ProfileId::uses_sony_encapsulation)
            );
            for member in members {
                assert!(
                    !matches!(
                        (group.inquiry_support(), member.inquiry_support()),
                        (
                            crate::capabilities::InquirySupport::Full,
                            crate::capabilities::InquirySupport::Partial
                        ) | (
                            crate::capabilities::InquirySupport::Full
                                | crate::capabilities::InquirySupport::Partial,
                            crate::capabilities::InquirySupport::None
                        )
                    ),
                    "{group:?} reports more inquiry support than {member:?}"
                );
            }
        }
    }

    #[test]
    fn test_profile_transport_support() {
        for profile in ProfileId::all() {
            let spec = profile.spec().expect("built-in profile");
            let transports = spec.transports();
            assert!(
                transports.supports(crate::transport::StandardTransport::Tcp)
                    || transports.supports(crate::transport::StandardTransport::Udp)
                    || transports.supports_serial(),
                "{profile:?} has no transport"
            );
            if profile.uses_sony_encapsulation() {
                assert!(transports.tcp_port().is_none() && !transports.supports_serial());
            }
            assert_eq!(profile.default_tcp_port(), transports.tcp_port());
            assert_eq!(profile.default_udp_port(), transports.udp_port());
        }
    }

    #[cfg(feature = "serde")]
    #[test]
    fn persisted_profile_identifiers_are_explicit_and_pinned() {
        for (id, expected) in [
            (ProfileId::PtzOpticsG2, "ptzoptics_g2"),
            (ProfileId::PtzOpticsG3, "ptzoptics_g3"),
            (ProfileId::PtzOptics30x, "ptzoptics_30x"),
            (ProfileId::SonyFr7, "sony_fr7"),
            (ProfileId::SonyBrcH900, "sony_brc_h900"),
            (ProfileId::SonyEviH100, "sony_evi_h100"),
            (ProfileId::SonyBrc300, "sony_brc_300"),
            (ProfileId::NearusBrc300, "nearus_brc_300"),
            (ProfileId::GenericVisca, "generic_visca"),
        ] {
            assert_eq!(serde_json::to_value(id).expect("profile id"), expected);
            assert_eq!(
                serde_json::from_value::<ProfileId>(serde_json::json!(expected))
                    .expect("profile id"),
                id
            );
        }
        assert_eq!(
            serde_json::to_value(ProfileGroup::PtzOptics).expect("group"),
            "ptzoptics"
        );
    }

    #[test]
    fn test_profile_description() {
        let desc = ProfileId::SonyFr7.description();
        assert!(desc.contains("Professional"));
        assert!(desc.contains("ND filter"));

        let desc = ProfileId::PtzOpticsG2.description();
        assert!(desc.contains("20x"));
        assert!(desc.contains("128 presets"));
        assert!(!desc.contains("digital zoom"));

        let desc = ProfileId::GenericVisca.description();
        assert!(desc.contains("Conservative"));
    }

    #[test]
    fn test_profile_encapsulation_consistency() {
        for profile in ProfileId::all() {
            let group = profile.profile_group();
            assert_eq!(
                profile.uses_sony_encapsulation(),
                group.uses_sony_encapsulation(),
                "Profile {:?} and group {:?} encapsulation mismatch",
                profile,
                group
            );
        }
    }
}
