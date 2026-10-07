//! Camera profile implementations generated from the built-in profile registry.
//!
//! Public profile types remain ordinary zero-sized Rust structs. Their metadata,
//! runtime capability constants, typed support markers, profile IDs, profile
//! groups, evidence notes, and invariant tests are emitted from
//! `profile_registry`.

mod profile_constants {
    use crate::{
        capabilities::ShutterSpeedEntry,
        command::{exposure::ExposureMode, FocusZone},
        units::Fraction,
        WhiteBalanceMode,
    };

    /// A `1/denominator` second table entry. Only the `const` tables below
    /// call it, so a zero denominator is a compile error, never a runtime
    /// panic.
    #[allow(clippy::panic)]
    const fn entry(denominator: u32, value: u8) -> ShutterSpeedEntry {
        match Fraction::new(1, denominator) {
            Some(exposure) => ShutterSpeedEntry::new(exposure, value),
            None => panic!("shutter denominators are nonzero"),
        }
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

    /// Profiles whose shutter table could not be read from their source.
    pub const NO_SHUTTER_SPEEDS: &[ShutterSpeedEntry] = &[];

    /// The five shared AE modes; also the `Exposure::EXPOSURE_MODES` default.
    pub(crate) use crate::capabilities::exposure::STANDARD_EXPOSURE_MODES;

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
        crate::capabilities::focus::DOCUMENTED_FOCUS_ZONES;

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

    pub const SONY_FR7_WB_MODES: &[WhiteBalanceMode] = &[
        WhiteBalanceMode::Auto,
        WhiteBalanceMode::Indoor,
        WhiteBalanceMode::Outdoor,
        WhiteBalanceMode::ATW,
        WhiteBalanceMode::OnePush,
        WhiteBalanceMode::Manual,
    ];
}

#[macro_use]
#[path = "profile_registry.rs"]
pub(crate) mod profile_registry;

builtin_profile_registry!(__define_builtin_profiles);

#[cfg(test)]
mod tests {
    use super::*;
    use crate::capabilities::{
        nd_filter::NdFilterMetadataExt, pan_tilt::PanTiltExt, ProfileMetadata,
    };

    #[test]
    fn test_ptzoptics_g2_capabilities() {
        let camera = PtzOpticsG2;

        assert!(camera.validate_pan(0).is_ok());
        assert!(camera.validate_pan(2448).is_ok());
        assert!(camera.validate_pan(2449).is_err());

        assert_eq!(camera.degrees_to_pan_units(170.0), Some(2448));
        assert_eq!(camera.pan_units_to_degrees(2448), 170.0);
    }

    /// PTZOptics G2 bench facts: 14.4 pan/tilt units per degree and an
    /// absolute optical zoom range of `0..=0x4000`.
    #[test]
    fn ptzoptics_g2_geometry_matches_bench_facts() {
        use crate::capabilities::{PanTilt, Zoom};

        assert_eq!(<PtzOpticsG2 as PanTilt>::PAN_DEGREES_TO_UNITS, 14.4);
        assert_eq!(<PtzOpticsG2 as PanTilt>::TILT_DEGREES_TO_UNITS, 14.4);
        assert_eq!(<PtzOpticsG2 as Zoom>::OPTICAL_ZOOM_MAX, 0x4000);
        assert_eq!(<PtzOpticsG2 as Zoom>::DIGITAL_ZOOM_MAX, None);
    }

    /// #808: the public helper rounds like the request path (144.72 → 145;
    /// it previously truncated to 144), and ±0.05° is ±1 unit, not 0.
    #[test]
    fn pan_tilt_helpers_round_half_away_from_zero_like_the_request_path() {
        let camera = PtzOpticsG2;
        assert_eq!(camera.degrees_to_pan_units(10.05), Some(145));
        assert_eq!(camera.degrees_to_pan_units(0.05), Some(1));
        assert_eq!(camera.degrees_to_pan_units(-0.05), Some(-1));
        assert_eq!(camera.degrees_to_tilt_units(-10.05), Some(-145));
    }

    /// #808: raw positions convert with each camera's own scale. The removed
    /// profile-less helper applied the G2 scale to every camera, which made
    /// GenericVisca `2880` NaN.
    #[test]
    fn raw_positions_convert_with_the_profiles_own_scale() {
        // R8's position table: pan `1E1B` is +170°, tilt `FC75` -20° and
        // `0FF0` +90°.
        let evi = crate::camera::PanTiltPosition::new(0x1E1B, -0x038B)
            .as_degrees_with_profile(&SonyEVIH100);
        assert!((evi.pan.0 - 170.0).abs() < 0.01, "{}", evi.pan.0);
        assert!((evi.tilt.0 + 20.0).abs() < 0.01, "{}", evi.tilt.0);
        let evi_up =
            crate::camera::PanTiltPosition::new(0, 0x0FF0).as_degrees_with_profile(&SonyEVIH100);
        assert!((evi_up.tilt.0 - 90.0).abs() < 0.01, "{}", evi_up.tilt.0);
        let generic =
            crate::camera::PanTiltPosition::new(2880, -1440).as_degrees_with_profile(&GenericVisca);
        assert_eq!((generic.pan.0, generic.tilt.0), (180.0, -90.0));
        let g2 =
            crate::camera::PanTiltPosition::new(2448, 1296).as_degrees_with_profile(&PtzOpticsG2);
        assert_eq!((g2.pan.0, g2.tilt.0), (170.0, 90.0));
    }

    /// #807: a shutter fraction maps through the profile's own table. The
    /// removed profile-independent conversion encoded 1/60 as `0x07` for
    /// every camera, and the Sony profiles shared a table that sent `05`
    /// (1/30 s on every Sony source) for 1/1000 s. Each code below is the
    /// model source's own: R8 for EVI-H100, R12/R21 for BRC-300 and Nearus,
    /// and their shared codes for Generic VISCA.
    #[test]
    fn shutter_fractions_resolve_through_each_profile_table() {
        use crate::{capabilities::Capabilities, units::Fraction};

        let code = |caps: &Capabilities, n, d| {
            Fraction::new(n, d)
                .and_then(|exposure| caps.shutter_speed_for(exposure).ok())
                .map(|s| s.value())
        };
        let g2 = Capabilities::from_profile::<PtzOpticsG2>();
        assert_eq!(code(&g2, 1, 60), Some(0x02));
        assert_eq!(code(&g2, 1, 1000), Some(0x0B));
        assert_eq!(code(&g2, 2, 120), Some(0x02));
        assert_eq!(code(&g2, 1, 0), None);

        let evi = Capabilities::from_profile::<SonyEVIH100>();
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
            Capabilities::from_profile::<SonyBRC300>(),
            Capabilities::from_profile::<NearusBRC300>(),
            Capabilities::from_profile::<GenericVisca>(),
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
            Capabilities::from_profile::<SonyFR7>(),
            Capabilities::from_profile::<SonyBRCH900>(),
        ] {
            assert!(caps.shutter_speeds.is_empty(), "{}", caps.model_name);
            assert_eq!(code(&caps, 1, 60), None, "{}", caps.model_name);
        }
    }

    #[test]
    fn test_nd_filter_capability() {
        let fr7 = SonyFR7;

        assert!(fr7.has_nd_filter());
        assert_eq!(fr7.nd_filter_description(), "Variable ND filter");
        assert!(fr7.validate_nd_filter(128).is_ok());
    }

    #[test]
    fn test_profile_metadata() {
        assert_eq!(PtzOpticsG2::MODEL_NAME, "PtzOptics G2");
        assert_eq!(SonyFR7::MODEL_NAME, "Sony FR7");
    }

    /// Every profile belongs to exactly the group that lists it. The literal
    /// membership is pinned once, in the closed inventory of
    /// `tests/issue_548_supported_surface_inventory.rs`.
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
            ProfileGroup::PtzOpticsG2,
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
        assert_eq!(ProfileGroup::PtzOpticsG2.display_name(), "PtzOptics Series");
        assert_eq!(
            ProfileGroup::SonyProfessional.display_name(),
            "Sony Professional"
        );

        assert_eq!(format!("{}", ProfileGroup::GenericVisca), "Generic VISCA");
        assert_eq!(format!("{}", ProfileGroup::PtzOpticsG2), "PtzOptics Series");
    }

    /// A group reports a transport or envelope fact only when every member
    /// has it, and its weakest member's inquiry support.
    #[test]
    fn group_facts_hold_for_every_member() {
        for group in [
            ProfileGroup::GenericVisca,
            ProfileGroup::PtzOpticsG2,
            ProfileGroup::SonyProfessional,
        ] {
            let members = group.profiles();
            assert_eq!(
                group.supports_tcp(),
                members.iter().all(ProfileId::supports_tcp)
            );
            assert_eq!(
                group.supports_udp(),
                members.iter().all(ProfileId::supports_udp)
            );
            assert_eq!(
                group.supports_serial(),
                members.iter().all(ProfileId::supports_serial)
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
            assert!(
                profile.supports_tcp() || profile.supports_udp() || profile.supports_serial(),
                "{profile:?} has no transport"
            );
            if profile.uses_sony_encapsulation() {
                assert!(!profile.supports_tcp() && !profile.supports_serial());
            }
            assert_eq!(profile.default_tcp_port().is_some(), profile.supports_tcp());
            assert_eq!(profile.default_udp_port().is_some(), profile.supports_udp());
        }
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
