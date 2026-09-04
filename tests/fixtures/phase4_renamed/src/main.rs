//! Downstream derive/profile smoke test with a renamed crate dependency.

use std::time::Duration;

use visca_renamed::{
    capabilities::{
        self, Exposure, Focus, HasImageProcessing, ImageProcessing, MenuCapability,
        MotionSyncMetadata, NdFilterMetadata, PanTilt, Power, Presets, ProfileMetadata,
        ProfileTypedSupport, Tally, VariableSpeedMetadata, WhiteBalance, Zoom,
    },
    command::{RawInquiryPayload, Response, ResponseParser},
    request, AffectedAxes, CameraId, CompileTimeProfile, Inquiry, PositionInquirySupport,
    ProfileSpec, Request, TransportCompatibility, ViscaEnum, ViscaInquiry, ViscaValue,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq, ViscaValue)]
#[visca_value(min = "1", max = "4")]
struct RenamedValue(u8);

mod visca_value_hygiene {
    use visca_renamed::ViscaValue;

    #[allow(dead_code)]
    mod std {}

    #[allow(dead_code)]
    struct Result<T, E>(T, E);

    #[allow(dead_code)]
    trait TryFrom<T> {}

    #[allow(dead_code)]
    trait From<T> {}

    #[allow(dead_code)]
    enum CapturedResult {
        Ok,
        Err,
    }

    #[allow(unused_imports)]
    use CapturedResult::{Err, Ok};

    #[allow(unused_macros)]
    macro_rules! format {
        ($($tokens:tt)*) => {
            compile_error!("ViscaValue expansion captured `format!`")
        };
    }

    #[allow(unused_macros)]
    macro_rules! write {
        ($($tokens:tt)*) => {
            compile_error!("ViscaValue expansion captured `write!`")
        };
    }

    #[allow(unused_macros)]
    macro_rules! stringify {
        ($($tokens:tt)*) => {
            compile_error!("ViscaValue expansion captured `stringify!`")
        };
    }

    #[derive(Debug, Clone, Copy, PartialEq, Eq, ViscaValue)]
    #[visca_value(
        valid_values = "[0x03, 0x05, 0x08]",
        display_format = "hex",
        display_prefix = "hygienic"
    )]
    struct HygienicValue(u8);

    pub(super) fn assert_hygienic_expansion() -> ::core::result::Result<(), visca_renamed::Error> {
        let value = HygienicValue::new(0x05)?;
        assert_eq!(value, HygienicValue(0x05));
        assert!(HygienicValue::new(0x04).is_err());
        assert_eq!(::std::format!("{value}"), "hygienic 0x5");
        assert_eq!(
            <HygienicValue as ::core::convert::TryFrom<u8>>::try_from(0x03)?,
            HygienicValue(0x03)
        );
        assert_eq!(
            <u8 as ::core::convert::From<HygienicValue>>::from(value),
            0x05
        );
        ::core::result::Result::Ok(())
    }
}

// Keep the base renamed-dependency leg hostile to all prelude names and macros
// emitted by `visca_range_type!`. The ts-rs derive itself intentionally uses a
// few caller-scope standard-library names, so the helper-feature leg below
// tests the real derive separately while retaining all seven hostile derives.
#[cfg(not(feature = "range-helper-derives"))]
mod visca_range_type_hygiene {
    #[allow(unused_imports)]
    use derive_shadows::{Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd};
    use visca_renamed::visca_range_type;

    #[allow(dead_code)]
    mod std {}

    #[allow(dead_code)]
    struct Result<T, E>(T, E);

    #[allow(dead_code)]
    trait TryFrom<T> {}

    #[allow(dead_code)]
    trait From<T> {}

    #[allow(dead_code)]
    struct Option<T>(T);

    #[allow(dead_code)]
    enum CapturedVariants {
        Ok,
        Err,
        Some,
        None,
    }

    #[allow(unused_imports)]
    use CapturedVariants::{Err, None, Ok, Some};

    #[allow(unused_macros)]
    macro_rules! format {
        ($($tokens:tt)*) => {
            compile_error!("visca_range_type! expansion captured `format!`")
        };
    }

    #[allow(unused_macros)]
    macro_rules! write {
        ($($tokens:tt)*) => {
            compile_error!("visca_range_type! expansion captured `write!`")
        };
    }

    #[allow(unused_macros)]
    macro_rules! stringify {
        ($($tokens:tt)*) => {
            compile_error!("visca_range_type! expansion captured `stringify!`")
        };
    }

    #[allow(unused_macros)]
    macro_rules! module_path {
        ($($tokens:tt)*) => {
            compile_error!("visca_range_type! expansion captured `module_path!`")
        };
    }

    visca_range_type! {
        HygienicRange: u8 {
            min: 2,
            max: 4
        }
    }

    visca_range_type! {
        SecondHygienicRange: u8 {
            min: 7,
            max: 9
        }
    }

    #[derive(
        ::core::fmt::Debug,
        ::core::marker::Copy,
        ::core::clone::Clone,
        ::core::cmp::PartialEq,
        ::core::cmp::Eq,
        ::core::cmp::PartialOrd,
        ::core::cmp::Ord,
    )]
    struct CoreOnlyInner(u8);

    impl ::core::fmt::Display for CoreOnlyInner {
        fn fmt(&self, formatter: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {
            ::core::write!(formatter, "{}", self.0)
        }
    }

    visca_range_type! {
        CoreOnlyRange: CoreOnlyInner {
            min: CoreOnlyInner(1),
            max: CoreOnlyInner(2)
        }
    }

    fn clone_via_trait<T: ::core::clone::Clone>(value: &T) -> T {
        value.clone()
    }

    fn copy_via_trait<T: ::core::marker::Copy>(value: &T) -> T {
        *value
    }

    fn assert_derive_traits<T>()
    where
        T: ::core::fmt::Debug
            + ::core::marker::Copy
            + ::core::clone::Clone
            + ::core::cmp::PartialEq
            + ::core::cmp::Eq
            + ::core::cmp::PartialOrd
            + ::core::cmp::Ord,
    {
    }

    pub(super) fn assert_base_contract() -> ::core::result::Result<(), visca_renamed::Error> {
        assert_derive_traits::<HygienicRange>();
        assert_eq!(HygienicRange::MIN, 2);
        assert_eq!(HygienicRange::MAX, 4);
        let value = HygienicRange::new(3)?;
        let upper = HygienicRange::new(4)?;
        let copied = copy_via_trait(&value);
        let cloned = clone_via_trait(&value);
        assert_eq!(value.value(), 3);
        assert_eq!(::std::format!("{value:?}"), "HygienicRange(3)");
        assert_eq!(copied, value);
        assert_eq!(cloned, value);
        assert_ne!(value, upper);
        assert_eq!(
            value.partial_cmp(&upper),
            ::core::option::Option::Some(::core::cmp::Ordering::Less)
        );
        assert_eq!(value.cmp(&upper), ::core::cmp::Ordering::Less);
        assert_eq!(
            ::std::format!("{}", HygienicRange::new(1).expect_err("below range")),
            "Invalid parameter 'HygienicRange': must be between 2 and 4 (value: 1)"
        );
        assert_eq!(
            <HygienicRange as ::core::convert::TryFrom<u8>>::try_from(4)?,
            HygienicRange::new(4)?
        );
        assert_eq!(<u8 as ::core::convert::From<HygienicRange>>::from(value), 3);
        assert_eq!(SecondHygienicRange::new(8)?.value(), 8);
        assert_eq!(CoreOnlyRange::new(CoreOnlyInner(2))?.value().0, 2);
        ::core::result::Result::Ok(())
    }
}

#[cfg(feature = "range-helper-derives")]
#[allow(deprecated)]
#[allow(dead_code, non_camel_case_types)]
mod visca_range_type_helpers {
    #[allow(unused_imports)]
    use derive_shadows::{Clone, Copy, Debug, Eq, Ord, PartialEq, PartialOrd};
    use visca_renamed::visca_range_type;

    visca_range_type! {
        /// Helper-derived range documentation.
        #[deprecated(note = "schema deprecation marker")]
        #[serde(rename = "SerdeRange")]
        #[schemars(rename = "SchemaRange")]
        #[ts(rename = "RenamedRange", export_to = "phase4-ranges/")]
        HygienicRange: u8 {
            min: 2,
            max: 4
        }
    }

    visca_range_type! {
        /// Raw identifier range documentation.
        r#type: u8 {
            min: 8,
            max: 9
        }
    }

    visca_range_type! {
        /// TypeScript type override documentation.
        #[ts(type = "RangeBrand")]
        TypedTsRange: u8 {
            min: 10,
            max: 11
        }
    }

    visca_range_type! {
        /// Documentation superseded by explicit schema metadata.
        #[schemars(
            rename = "InlineSchemaRange",
            title = "Explicit schema title",
            description = "Explicit schema description",
            inline
        )]
        InlineMetadataRange: u8 {
            min: 12,
            max: 13
        }
    }

    /// Distinct non-inline inner schema documentation.
    #[derive(
        ::core::fmt::Debug,
        ::core::marker::Copy,
        ::core::clone::Clone,
        ::core::cmp::PartialEq,
        ::core::cmp::Eq,
        ::core::cmp::PartialOrd,
        ::core::cmp::Ord,
        visca_renamed::__macro_support::serde::Serialize,
        visca_renamed::__macro_support::serde::Deserialize,
        visca_renamed::__macro_support::schemars::JsonSchema,
        visca_renamed::__macro_support::ts_rs::TS,
    )]
    #[serde(crate = "visca_renamed::__macro_support::serde")]
    #[schemars(crate = "visca_renamed::__macro_support::schemars")]
    #[ts(crate = "visca_renamed::__macro_support::ts_rs")]
    struct NonInlineInner(u8);

    impl ::core::fmt::Display for NonInlineInner {
        fn fmt(&self, formatter: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {
            ::core::write!(formatter, "{}", self.0)
        }
    }

    visca_range_type! {
        /// Wrapper metadata around a non-inline inner schema.
        #[schemars(rename = "NonInlineRangeSchema")]
        NonInlineRange: NonInlineInner {
            min: NonInlineInner(1),
            max: NonInlineInner(3)
        }
    }

    #[derive(visca_renamed::__macro_support::schemars::JsonSchema)]
    #[schemars(crate = "visca_renamed::__macro_support::schemars")]
    struct RangeContainer {
        value: HygienicRange,
        inline_value: InlineMetadataRange,
    }

    fn assert_serde<T>()
    where
        T: visca_renamed::__macro_support::serde::Serialize
            + for<'de> visca_renamed::__macro_support::serde::Deserialize<'de>,
    {
    }

    fn assert_schema_contract() {
        use visca_renamed::__macro_support::schemars::JsonSchema;

        assert!(!<HygienicRange as JsonSchema>::inline_schema());
        assert_eq!(<HygienicRange as JsonSchema>::schema_name(), "SchemaRange");
        assert_eq!(
            <HygienicRange as JsonSchema>::schema_id(),
            "phase4_renamed_dependency::visca_range_type_helpers::SchemaRange"
        );

        let root = visca_renamed::__macro_support::schemars::schema_for!(HygienicRange);
        let root = ::serde_json::to_value(root).expect("serialize root schema");
        assert_eq!(
            root.pointer("/title"),
            Some(&::serde_json::json!("SchemaRange"))
        );
        assert_eq!(
            root.pointer("/description"),
            Some(&::serde_json::json!("Helper-derived range documentation."))
        );
        assert_eq!(
            root.pointer("/deprecated"),
            Some(&::serde_json::json!(true))
        );
        assert_eq!(root.pointer("/type"), Some(&::serde_json::json!("integer")));
        assert_eq!(root.pointer("/minimum"), Some(&::serde_json::json!(0)));
        assert_eq!(root.pointer("/maximum"), Some(&::serde_json::json!(255)));
        assert_ne!(root.pointer("/minimum"), Some(&::serde_json::json!(2)));
        assert_ne!(root.pointer("/maximum"), Some(&::serde_json::json!(4)));

        let nested = visca_renamed::__macro_support::schemars::schema_for!(RangeContainer);
        let nested = ::serde_json::to_value(nested).expect("serialize nested schema");
        assert_eq!(
            nested.pointer("/properties/value/$ref"),
            Some(&::serde_json::json!("#/$defs/SchemaRange")),
            "nested wrapper was not referenced: {nested}"
        );
        assert_eq!(
            nested.pointer("/$defs/SchemaRange/description"),
            Some(&::serde_json::json!("Helper-derived range documentation."))
        );
        assert_eq!(
            nested.pointer("/$defs/SchemaRange/deprecated"),
            Some(&::serde_json::json!(true))
        );
        assert_eq!(
            nested.pointer("/$defs/SchemaRange/type"),
            Some(&::serde_json::json!("integer"))
        );
        assert!(<InlineMetadataRange as JsonSchema>::inline_schema());
        assert_eq!(
            <InlineMetadataRange as JsonSchema>::schema_name(),
            "InlineSchemaRange"
        );
        assert!(
            nested.pointer("/properties/inline_value/$ref").is_none(),
            "inline schema unexpectedly used a reference: {nested}"
        );
        assert_eq!(
            nested.pointer("/properties/inline_value/title"),
            Some(&::serde_json::json!("Explicit schema title"))
        );
        assert_eq!(
            nested.pointer("/properties/inline_value/description"),
            Some(&::serde_json::json!("Explicit schema description"))
        );
        assert!(nested.pointer("/$defs/InlineSchemaRange").is_none());

        let non_inline = visca_renamed::__macro_support::schemars::schema_for!(NonInlineRange);
        let non_inline = ::serde_json::to_value(non_inline).expect("serialize non-inline schema");
        assert_eq!(
            non_inline.pointer("/description"),
            Some(&::serde_json::json!(
                "Wrapper metadata around a non-inline inner schema."
            ))
        );
        assert_eq!(
            non_inline.pointer("/$ref"),
            Some(&::serde_json::json!("#/$defs/NonInlineInner")),
            "non-inline inner schema was not referenced: {non_inline}"
        );
        assert!(non_inline.pointer("/$defs/NonInlineInner").is_some());
        assert_eq!(
            non_inline.pointer("/$defs/NonInlineInner/description"),
            Some(&::serde_json::json!(
                "Distinct non-inline inner schema documentation."
            ))
        );
    }

    fn assert_ts_contract() {
        use visca_renamed::__macro_support::ts_rs::TS;

        let config = visca_renamed::__macro_support::ts_rs::Config::default();
        assert_eq!(
            <HygienicRange as TS>::docs().as_deref(),
            Some("/**\n * Helper-derived range documentation.\n */\n")
        );
        assert_eq!(<HygienicRange as TS>::ident(&config), "RenamedRange");
        assert_eq!(<HygienicRange as TS>::name(&config), "RenamedRange");
        assert_eq!(
            <HygienicRange as TS>::decl(&config),
            "type RenamedRange = number;"
        );
        assert_eq!(
            <HygienicRange as TS>::output_path(),
            ::core::option::Option::Some(::std::path::PathBuf::from(
                "phase4-ranges/RenamedRange.ts"
            ))
        );
        assert_eq!(<r#type as TS>::ident(&config), "type");
        assert_eq!(<r#type as TS>::name(&config), "type");
        assert_eq!(<r#type as TS>::decl(&config), "type type = number;");
        assert_eq!(<TypedTsRange as TS>::ident(&config), "TypedTsRange");
        assert_eq!(<TypedTsRange as TS>::name(&config), "TypedTsRange");
        assert_eq!(<TypedTsRange as TS>::inline(&config), "RangeBrand");
        assert_eq!(
            <TypedTsRange as TS>::decl(&config),
            "type TypedTsRange = RangeBrand;"
        );
        assert_eq!(
            <TypedTsRange as TS>::output_path(),
            ::core::option::Option::Some(::std::path::PathBuf::from("TypedTsRange.ts"))
        );
        assert!(::std::panic::catch_unwind(|| {
            <HygienicRange as TS>::inline_flattened(&config)
        })
        .is_err());
    }

    pub(super) fn assert_helper_contract() {
        assert_serde::<HygienicRange>();
        assert_serde::<r#type>();

        let valid = HygienicRange::new(3).expect("valid helper value");
        assert_eq!(
            ::serde_json::to_string(&valid).expect("serialize range scalar"),
            "3"
        );
        assert_eq!(
            ::serde_json::from_str::<HygienicRange>("4").expect("deserialize upper bound"),
            HygienicRange::new(4).expect("construct upper bound")
        );
        let invalid = ::serde_json::from_str::<HygienicRange>("5")
            .expect_err("out-of-range scalar must be rejected");
        assert_eq!(invalid.classify(), ::serde_json::error::Category::Data);
        assert_eq!(
            invalid.to_string(),
            "Invalid parameter 'HygienicRange': must be between 2 and 4 (value: 5)"
        );

        assert_eq!(r#type::new(8).expect("raw range").value(), 8);
        assert_schema_contract();
        assert_ts_contract();
    }

    #[cfg(test)]
    #[test]
    fn generated_auto_export_function_writes_the_configured_binding() {
        use visca_renamed::__macro_support::ts_rs::TS;

        let config = visca_renamed::__macro_support::ts_rs::Config::from_env();
        export_bindings_hygienicrange();
        let relative = HygienicRange::output_path().expect("derived output path");
        let path = config.out_dir().join(relative);
        let binding = ::std::fs::read_to_string(&path)
            .unwrap_or_else(|error| panic!("failed to read {}: {error}", path.display()));
        assert!(binding.contains("Helper-derived range documentation."));
        assert!(binding.contains("export type RenamedRange = number;"));
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, ViscaEnum)]
enum RenamedEnum {
    First = 1,
    Second = 2,
}

#[derive(Debug, Clone, Copy, ViscaInquiry)]
#[visca(opcode = 0x7e, response = Raw)]
struct RenamedRawInquiry;

#[derive(Debug, Clone, Copy, ViscaInquiry)]
#[visca(opcode = 0x00, response = Power)]
struct RenamedResponseInquiry;

#[derive(Debug, Clone, Copy, ViscaInquiry)]
#[visca(opcode = 0x00, response = Power, typed_response = bool, typed_field = on)]
struct RenamedTypedInquiry;

fn assert_inquiry_contracts() {
    fn raw<T>()
    where
        T: Request<Class = request::Inquiry> + Inquiry<Response = RawInquiryPayload>,
    {
    }
    fn generic<T>()
    where
        T: Request<Class = request::Inquiry> + Inquiry<Response = Response>,
    {
    }
    fn typed<T>()
    where
        T: Request<Class = request::Inquiry>
            + Inquiry<Response = bool>
            + ResponseParser<Response = bool>,
    {
    }

    raw::<RenamedRawInquiry>();
    generic::<RenamedResponseInquiry>();
    typed::<RenamedTypedInquiry>();
}

/// A downstream static profile exercises the public profile contract without
/// importing any implementation-only camera/runtime types.
#[derive(Debug, Clone, Copy)]
struct DownstreamProfile;

type Base = visca_renamed::profiles::GenericVisca;

impl ProfileMetadata for DownstreamProfile {
    const MODEL_NAME: &'static str = "Downstream renamed profile";
    const DEFAULT_CAMERA_ID: u8 = 3;
    type Envelope = visca_renamed::transport::RawVisca;
    const ACK_TIMEOUT: Duration = Duration::from_millis(120);
    const COMMAND_TIMEOUTS: visca_renamed::CommandTimeouts = visca_renamed::CommandTimeouts::new(
        Duration::from_secs(6),
        Duration::from_secs(30),
        Duration::from_secs(60),
        Duration::from_secs(300),
        Duration::from_secs(6),
    );
}

impl PanTilt for DownstreamProfile {
    const PAN_RANGE: capabilities::CapabilityRange<i32> = <Base as PanTilt>::PAN_RANGE;
    const TILT_RANGE: capabilities::CapabilityRange<i32> = <Base as PanTilt>::TILT_RANGE;
    const MAX_PAN_SPEED: u8 = 9;
    const MAX_TILT_SPEED: u8 = 7;
    const PAN_DEGREES_TO_UNITS: f32 = 10.0;
    const TILT_DEGREES_TO_UNITS: f32 = 15.0;
}

impl Zoom for DownstreamProfile {
    const OPTICAL_ZOOM_MAX: u16 = <Base as Zoom>::OPTICAL_ZOOM_MAX;
    const DIGITAL_ZOOM_MAX: Option<u16> = None;
    const ZOOM_SPEED_RANGE: capabilities::CapabilityRange<u8> = <Base as Zoom>::ZOOM_SPEED_RANGE;
    const SUPPORTS_DIRECT_ZOOM: bool = false;
    const ZOOM_MAGNIFICATION_TO_UNITS: f32 = <Base as Zoom>::ZOOM_MAGNIFICATION_TO_UNITS;
}

impl Focus for DownstreamProfile {
    const FOCUS_NEAR_LIMIT: u16 = <Base as Focus>::FOCUS_NEAR_LIMIT;
    const FOCUS_FAR_LIMIT: u16 = <Base as Focus>::FOCUS_FAR_LIMIT;
    const SUPPORTS_AUTO_FOCUS: bool = true;
    const SUPPORTS_ONE_PUSH_FOCUS: bool = false;
}

impl Exposure for DownstreamProfile {
    const EXPOSURE_MODES: &'static [visca_renamed::ExposureMode] =
        <Base as Exposure>::EXPOSURE_MODES;
    const IRIS_RANGE: Option<capabilities::CapabilityRange<u16>> = <Base as Exposure>::IRIS_RANGE;
    const SHUTTER_SPEEDS: &'static [capabilities::ShutterSpeed] =
        <Base as Exposure>::SHUTTER_SPEEDS;
    const GAIN_RANGE: capabilities::CapabilityRange<u8> = <Base as Exposure>::GAIN_RANGE;
    const BRIGHTNESS_RANGE: Option<capabilities::CapabilityRange<u16>> =
        <Base as Exposure>::BRIGHTNESS_RANGE;
    const SUPPORTS_BACKLIGHT_COMP: bool = true;
}

impl WhiteBalance for DownstreamProfile {
    const WB_MODES: &'static [visca_renamed::WhiteBalanceMode] = <Base as WhiteBalance>::WB_MODES;
    const SUPPORTS_ONE_PUSH_WB: bool = false;
    const RG_TUNING_RANGE: Option<capabilities::CapabilityRange<i8>> =
        <Base as WhiteBalance>::RG_TUNING_RANGE;
    const BG_TUNING_RANGE: Option<capabilities::CapabilityRange<i8>> =
        <Base as WhiteBalance>::BG_TUNING_RANGE;
}

impl ImageProcessing for DownstreamProfile {
    const CONTRAST_RANGE: Option<capabilities::CapabilityRange<u8>> =
        <Base as ImageProcessing>::CONTRAST_RANGE;
    const SHARPNESS_RANGE: Option<capabilities::CapabilityRange<u8>> =
        <Base as ImageProcessing>::SHARPNESS_RANGE;
    const SATURATION_RANGE: Option<capabilities::CapabilityRange<u8>> =
        <Base as ImageProcessing>::SATURATION_RANGE;
    const SUPPORTS_FLIP: bool = true;
    const SUPPORTS_MIRROR: bool = true;
    const SUPPORTS_HUE: bool = true;
    const HUE_RANGE: Option<capabilities::CapabilityRange<u8>> =
        Some(capabilities::CapabilityRange::<u8>::new(0, 14));
    const SUPPORTS_IMAGE_PROCESSING: bool = true;
}

// The downstream profile documents a real image surface above, so it opts in
// to the matching static noun marker and runtime base permission explicitly.
// Metadata alone is not typed permission: Generic VISCA intentionally has the
// same metadata trait but no `HasImageProcessing` implementation.
impl HasImageProcessing for DownstreamProfile {}

fn assert_image_marker<P: HasImageProcessing>() {}

impl Presets for DownstreamProfile {
    const MAX_PRESETS: u8 = 8;
    const PRESET_SPEED_RANGE: capabilities::CapabilityRange<u8> =
        <Base as Presets>::PRESET_SPEED_RANGE;
    const SUPPORTS_PRESET_TOUR: bool = false;
}

impl Power for DownstreamProfile {
    const POWER_ON_TIME: Duration = Duration::from_millis(4_250);
    const SUPPORTS_STANDBY: bool = true;
}

impl MenuCapability for DownstreamProfile {}
impl Tally for DownstreamProfile {}
impl MotionSyncMetadata for DownstreamProfile {}
impl NdFilterMetadata for DownstreamProfile {}
impl VariableSpeedMetadata for DownstreamProfile {}
impl ProfileTypedSupport for DownstreamProfile {
    const TYPED_SUPPORT: capabilities::TypedSupportSet = capabilities::TypedSupportSet::empty();
}

impl CompileTimeProfile for DownstreamProfile {
    const TRANSPORTS: TransportCompatibility = TransportCompatibility::new(Some(9876), None, false);
    const INQUIRY_TIMEOUT: Duration = Duration::from_millis(900);
    const CANCELLATION_TIMEOUT: Duration = Duration::from_millis(800);
    const AMBIGUITY_TIMEOUT: Duration = Duration::from_millis(700);
    const MAXIMUM_COMMAND_SOCKETS: u8 = 1;
    const PRESET_RECALL_AXES: Option<AffectedAxes> = Some(AffectedAxes::PAN_TILT);
    const POSITION_INQUIRIES: PositionInquirySupport =
        PositionInquirySupport::new(true, true, true);
}

fn renamed_dependency_derive_and_profile_contract() -> visca_renamed::Result<()> {
    assert_inquiry_contracts();
    assert_image_marker::<DownstreamProfile>();
    visca_value_hygiene::assert_hygienic_expansion()?;
    #[cfg(not(feature = "range-helper-derives"))]
    visca_range_type_hygiene::assert_base_contract()?;
    #[cfg(feature = "range-helper-derives")]
    visca_range_type_helpers::assert_helper_contract();

    assert_eq!(RenamedValue::new(2)?, RenamedValue(2));
    assert!(RenamedValue::new(0).is_err());
    assert_eq!(RenamedEnum::try_from(1)?, RenamedEnum::First);
    assert!(RenamedEnum::try_from(3).is_err());

    let inquiry = RenamedRawInquiry;
    let mut buffer = [0; <RenamedRawInquiry as Request>::MAX_SIZE];
    assert_eq!(inquiry.write_into(CameraId::CAMERA_1, &mut buffer)?, 5);
    assert_eq!(
        inquiry.decoder().decode(&[0x12, 0x34])?.as_slice(),
        &[0x12, 0x34]
    );

    let profile = ProfileSpec::from_compile_time::<DownstreamProfile>()?;
    assert_eq!(
        profile.capabilities().model_name,
        "Downstream renamed profile"
    );
    assert!(profile.capabilities().has_image_processing);
    assert_eq!(profile.capabilities().hue_range, Some(0..=14));
    assert_eq!(profile.transports().tcp_port(), Some(9876));
    Ok(())
}

#[test]
fn renamed_dependency_derive_and_profile_contract_runs() -> visca_renamed::Result<()> {
    renamed_dependency_derive_and_profile_contract()
}

fn main() -> visca_renamed::Result<()> {
    renamed_dependency_derive_and_profile_contract()
}
