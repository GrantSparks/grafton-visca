//! Camera profile implementations generated from the built-in profile registry.
//!
//! Public profile types remain ordinary zero-sized Rust structs. Their metadata,
//! runtime capability constants, typed support markers, profile IDs, profile
//! groups, evidence notes, and invariant tests are emitted from
//! `profile_registry`.

use std::fmt;

use crate::error::Error;

mod profile_constants {
    use crate::{capabilities::ShutterSpeed, command::exposure::ExposureMode, WhiteBalanceMode};

    pub const PTZ_OPTICS_G2_SHUTTER_SPEEDS: &[ShutterSpeed] = &[
        ShutterSpeed::new("1/30", 0x01),
        ShutterSpeed::new("1/60", 0x02),
        ShutterSpeed::new("1/90", 0x03),
        ShutterSpeed::new("1/100", 0x04),
        ShutterSpeed::new("1/125", 0x05),
        ShutterSpeed::new("1/180", 0x06),
        ShutterSpeed::new("1/250", 0x07),
        ShutterSpeed::new("1/350", 0x08),
        ShutterSpeed::new("1/500", 0x09),
        ShutterSpeed::new("1/725", 0x0A),
        ShutterSpeed::new("1/1000", 0x0B),
        ShutterSpeed::new("1/1500", 0x0C),
        ShutterSpeed::new("1/2000", 0x0D),
        ShutterSpeed::new("1/3000", 0x0E),
        ShutterSpeed::new("1/4000", 0x0F),
        ShutterSpeed::new("1/6000", 0x10),
        ShutterSpeed::new("1/10000", 0x11),
    ];

    pub const GENERIC_VISCA_SHUTTER_SPEEDS: &[ShutterSpeed] = &[
        ShutterSpeed::new("1/30", 0x00),
        ShutterSpeed::new("1/60", 0x01),
        ShutterSpeed::new("1/100", 0x02),
        ShutterSpeed::new("1/250", 0x03),
        ShutterSpeed::new("1/500", 0x04),
        ShutterSpeed::new("1/1000", 0x05),
        ShutterSpeed::new("1/2000", 0x06),
        ShutterSpeed::new("1/4000", 0x07),
        ShutterSpeed::new("1/10000", 0x08),
    ];

    /// Standard exposure modes supported by most VISCA cameras.
    pub const STANDARD_EXPOSURE_MODES: &[ExposureMode] = &[
        ExposureMode::Auto,
        ExposureMode::Manual,
        ExposureMode::Shutter,
        ExposureMode::Iris,
        ExposureMode::Bright,
    ];

    pub const PTZ_OPTICS_EXPOSURE_MODES: &[ExposureMode] = &[
        ExposureMode::Auto,
        ExposureMode::Manual,
        ExposureMode::Shutter,
        ExposureMode::Iris,
        ExposureMode::Bright,
    ];

    /// Profiles whose exposure controls use a model-specific command family
    /// must not advertise the shared `04 39` VISCA modes.
    pub const NO_SHARED_EXPOSURE_MODES: &[ExposureMode] = &[];

    pub const STANDARD_WB_MODES: &[WhiteBalanceMode] = &[
        WhiteBalanceMode::Auto,
        WhiteBalanceMode::Indoor,
        WhiteBalanceMode::Outdoor,
        WhiteBalanceMode::OnePush,
        WhiteBalanceMode::Manual,
    ];

    pub const SONY_COLOR_TEMP_WB_MODES: &[WhiteBalanceMode] = &[
        WhiteBalanceMode::Auto,
        WhiteBalanceMode::Indoor,
        WhiteBalanceMode::Outdoor,
        WhiteBalanceMode::OnePush,
        WhiteBalanceMode::Manual,
        WhiteBalanceMode::ColorTemperature,
    ];

    pub const PTZ_OPTICS_WB_MODES: &[WhiteBalanceMode] = &[
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

define_builtin_profiles!();

/// Preset ID for PtzOptics G2 cameras (0-127).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct G2PresetId(u8);

impl G2PresetId {
    /// Create a new preset ID with validation.
    pub fn new(id: u8) -> Result<Self, Error> {
        if id <= 127 {
            Ok(Self(id))
        } else {
            Err(Error::InvalidPreset {
                preset: id,
                max: 127,
            })
        }
    }

    /// Home preset (preset 0).
    pub const HOME: Self = Self(0);

    /// First user preset.
    pub const PRESET1: Self = Self(1);
}

impl fmt::Display for G2PresetId {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.0 == 0 {
            write!(f, "Home")
        } else {
            write!(f, "Preset {}", self.0)
        }
    }
}

impl From<G2PresetId> for u8 {
    fn from(preset: G2PresetId) -> Self {
        preset.0
    }
}

impl TryFrom<u8> for G2PresetId {
    type Error = Error;

    fn try_from(value: u8) -> Result<Self, Self::Error> {
        Self::new(value)
    }
}

/// Gain values for PtzOptics G2 cameras.
#[derive(Debug, Clone, Copy, PartialEq, Eq, crate::ViscaEnum)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
#[cfg_attr(feature = "ts-rs", derive(ts_rs::TS), ts(export))]
#[cfg_attr(feature = "serde", serde(rename_all = "snake_case"))]
pub enum G2Gain {
    /// 0dB gain
    Gain0dB = 0,
    /// 3dB gain
    Gain3dB = 1,
    /// 6dB gain
    Gain6dB = 2,
    /// 9dB gain
    Gain9dB = 3,
    /// 12dB gain
    Gain12dB = 4,
    /// 15dB gain
    Gain15dB = 5,
    /// 18dB gain
    Gain18dB = 6,
    /// 21dB gain
    Gain21dB = 7,
    /// 24dB gain
    Gain24dB = 8,
}

impl fmt::Display for G2Gain {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let db = match self {
            G2Gain::Gain0dB => "0dB",
            G2Gain::Gain3dB => "3dB",
            G2Gain::Gain6dB => "6dB",
            G2Gain::Gain9dB => "9dB",
            G2Gain::Gain12dB => "12dB",
            G2Gain::Gain15dB => "15dB",
            G2Gain::Gain18dB => "18dB",
            G2Gain::Gain21dB => "21dB",
            G2Gain::Gain24dB => "24dB",
        };
        write!(f, "{db}")
    }
}

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

        assert_eq!(camera.degrees_to_pan_units(170.0), 2448);
        assert_eq!(camera.pan_units_to_degrees(2448), 170.0);
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

    #[test]
    fn test_profile_groups() {
        assert_eq!(
            ProfileId::PtzOpticsG2.profile_group(),
            ProfileGroup::PtzOpticsG2
        );
        assert_eq!(
            ProfileId::PtzOpticsG3.profile_group(),
            ProfileGroup::PtzOpticsG2
        );
        assert_eq!(
            ProfileId::PtzOptics30X.profile_group(),
            ProfileGroup::PtzOpticsG2
        );
        assert_eq!(
            ProfileId::SonyFr7.profile_group(),
            ProfileGroup::SonyProfessional
        );
        assert_eq!(
            ProfileId::SonyBrcH900.profile_group(),
            ProfileGroup::SonyProfessional
        );
        assert_eq!(
            ProfileId::GenericVisca.profile_group(),
            ProfileGroup::GenericVisca
        );
        assert_eq!(
            ProfileId::SonyBrc300.profile_group(),
            ProfileGroup::GenericVisca
        );
        assert_eq!(
            ProfileId::SonyEviH100.profile_group(),
            ProfileGroup::GenericVisca
        );
        assert_eq!(
            ProfileId::NearusBrc300.profile_group(),
            ProfileGroup::GenericVisca
        );
    }

    #[test]
    fn test_profile_group_profiles() {
        let generic_profiles = ProfileGroup::GenericVisca.profiles();
        assert_eq!(generic_profiles.len(), 4);
        assert!(generic_profiles.contains(&ProfileId::GenericVisca));
        assert!(generic_profiles.contains(&ProfileId::SonyBrc300));
        assert!(generic_profiles.contains(&ProfileId::SonyEviH100));
        assert!(generic_profiles.contains(&ProfileId::NearusBrc300));

        let ptzoptics_profiles = ProfileGroup::PtzOpticsG2.profiles();
        assert_eq!(ptzoptics_profiles.len(), 3);
        assert!(ptzoptics_profiles.contains(&ProfileId::PtzOpticsG2));
        assert!(ptzoptics_profiles.contains(&ProfileId::PtzOpticsG3));
        assert!(ptzoptics_profiles.contains(&ProfileId::PtzOptics30X));

        let sony_pro_profiles = ProfileGroup::SonyProfessional.profiles();
        assert_eq!(sony_pro_profiles.len(), 2);
        assert!(sony_pro_profiles.contains(&ProfileId::SonyFr7));
        assert!(sony_pro_profiles.contains(&ProfileId::SonyBrcH900));

        for profile in ProfileId::all() {
            let group = profile.profile_group();
            assert!(
                group.profiles().contains(profile),
                "Profile {:?} not found in its group {:?}",
                profile,
                group
            );
        }
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

    #[test]
    fn test_profile_group_transport_support() {
        for group in &[ProfileGroup::GenericVisca, ProfileGroup::PtzOpticsG2] {
            assert!(group.supports_tcp());
            assert!(group.supports_udp());
            assert!(group.supports_serial());
        }

        assert!(!ProfileGroup::SonyProfessional.supports_tcp());
        assert!(ProfileGroup::SonyProfessional.supports_udp());
        assert!(!ProfileGroup::SonyProfessional.supports_serial());
    }

    #[test]
    fn test_profile_group_encapsulation() {
        assert!(!ProfileGroup::GenericVisca.uses_sony_encapsulation());
        assert!(!ProfileGroup::PtzOpticsG2.uses_sony_encapsulation());
        assert!(ProfileGroup::SonyProfessional.uses_sony_encapsulation());
    }

    #[test]
    fn test_profile_transport_support() {
        for profile in ProfileId::all() {
            let sony_encapsulated = profile.uses_sony_encapsulation();
            assert_eq!(profile.supports_tcp(), !sony_encapsulated);
            assert!(profile.supports_udp());
            assert_eq!(profile.supports_serial(), !sony_encapsulated);
            assert_eq!(profile.default_tcp_port().is_some(), profile.supports_tcp());
            assert_eq!(profile.default_udp_port().is_some(), profile.supports_udp());
        }
    }

    #[test]
    fn test_profile_vendor() {
        assert_eq!(ProfileId::PtzOpticsG2.vendor(), "PtzOptics");
        assert_eq!(ProfileId::PtzOpticsG3.vendor(), "PtzOptics");
        assert_eq!(ProfileId::PtzOptics30X.vendor(), "PtzOptics");

        assert_eq!(ProfileId::SonyFr7.vendor(), "Sony");
        assert_eq!(ProfileId::SonyBrcH900.vendor(), "Sony");
        assert_eq!(ProfileId::SonyEviH100.vendor(), "Sony");
        assert_eq!(ProfileId::SonyBrc300.vendor(), "Sony");

        assert_eq!(ProfileId::NearusBrc300.vendor(), "Nearus");
        assert_eq!(ProfileId::GenericVisca.vendor(), "Generic");
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
