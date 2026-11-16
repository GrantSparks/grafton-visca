//! Downstream derive/profile smoke test with a renamed crate dependency.

use std::time::Duration;

use visca_renamed::wire::WireValue;

use visca_renamed::{
    capabilities::{self, HasImage},
    profiles::{CompileTimeProfile, ProfileSpec, TransportCompatibility},
    request::{self, Inquiry, Request},
    types::{AffectedAxes, CameraId},
    value, value_enum,
    wire::{RawInquiryPayload, Response, ResponseParser},
    ViscaInquiry,
};

#[value(min = 1, max = 4)]
struct RenamedValue(u8);

#[cfg(not(feature = "range-helper-derives"))]
mod visca_value_hygiene {
    use visca_renamed::value;

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
            compile_error!("value expansion captured `format!`")
        };
    }

    #[allow(unused_macros)]
    macro_rules! write {
        ($($tokens:tt)*) => {
            compile_error!("value expansion captured `write!`")
        };
    }

    #[allow(unused_macros)]
    macro_rules! stringify {
        ($($tokens:tt)*) => {
            compile_error!("value expansion captured `stringify!`")
        };
    }

    #[value(valid_values = [0x03, 0x05, 0x08], display = "hygienic {:#04x}")]
    struct HygienicValue(u8);

    pub(super) fn assert_hygienic_expansion() -> ::core::result::Result<(), visca_renamed::Error> {
        let value = HygienicValue::new(0x05)?;
        assert_eq!(value, HygienicValue(0x05));
        assert!(HygienicValue::new(0x04).is_err());
        assert_eq!(::std::format!("{value}"), "hygienic 0x05");
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
// emitted by `#[value]`. The ts-rs derive itself intentionally uses a
// few caller-scope standard-library names, so the helper-feature leg below
// tests the real derive separately while retaining all seven hostile derives.
#[cfg(not(feature = "range-helper-derives"))]
mod visca_range_type_hygiene {
    #[allow(unused_imports)]
    use derive_shadows::{Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd};
    use visca_renamed::value;

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
            compile_error!("value expansion captured `format!`")
        };
    }

    #[allow(unused_macros)]
    macro_rules! write {
        ($($tokens:tt)*) => {
            compile_error!("value expansion captured `write!`")
        };
    }

    #[allow(unused_macros)]
    macro_rules! stringify {
        ($($tokens:tt)*) => {
            compile_error!("value expansion captured `stringify!`")
        };
    }

    #[allow(unused_macros)]
    macro_rules! module_path {
        ($($tokens:tt)*) => {
            compile_error!("value expansion captured `module_path!`")
        };
    }

    #[value(min = 2, max = 4)]
    pub struct HygienicRange(u8);

    #[value(min = 7, max = 9)]
    pub struct SecondHygienicRange(u8);

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
            + ::core::cmp::Ord
            + ::core::hash::Hash,
    {
    }

    pub(super) fn assert_base_contract() -> ::core::result::Result<(), visca_renamed::Error> {
        assert_derive_traits::<HygienicRange>();
        assert_eq!(HygienicRange::MIN.value(), 2);
        assert_eq!(HygienicRange::MAX.value(), 4);
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
            "Parameter out of range: HygienicRange = 1 (valid range: 2..=4)"
        );
        assert_eq!(
            <HygienicRange as ::core::convert::TryFrom<u8>>::try_from(4)?,
            HygienicRange::new(4)?
        );
        assert_eq!(<u8 as ::core::convert::From<HygienicRange>>::from(value), 3);
        assert_eq!(SecondHygienicRange::new(8)?.value(), 8);
        ::core::result::Result::Ok(())
    }
}

#[cfg(feature = "range-helper-derives")]
#[allow(deprecated)]
#[allow(dead_code, non_camel_case_types)]
mod visca_range_type_helpers {
    #[allow(unused_imports)]
    use derive_shadows::{Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd};
    use visca_renamed::value;

    #[value(min = 2, max = 4)]
    /// Helper-derived range documentation.
    #[deprecated(note = "schema deprecation marker")]
    #[serde(rename = "SerdeRange")]
    #[schemars(rename = "SchemaRange")]
    #[ts(rename = "RenamedRange", export_to = "phase4-ranges/")]
    pub struct HygienicRange(u8);

    #[value(min = 8, max = 9)]
    /// Raw identifier range documentation.
    pub struct r#type(u8);

    #[value(min = 10, max = 11)]
    /// TypeScript type override documentation.
    #[ts(type = "RangeBrand")]
    pub struct TypedTsRange(u8);

    #[value(min = 12, max = 13)]
    /// Documentation superseded by explicit schema metadata.
    #[schemars(
        rename = "InlineSchemaRange",
        title = "Explicit schema title",
        description = "Explicit schema description",
        inline
    )]
    pub struct InlineMetadataRange(u8);

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
            "Parameter out of range: HygienicRange = 5 (valid range: 2..=4)"
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

#[value_enum]
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
#[visca(opcode = 0x00, response = Power, typed_response = ::visca_renamed::types::PowerStatus, typed_field = status)]
struct RenamedTypedInquiry;

fn assert_inquiry_contracts() {
    fn raw<T>()
    where
        T: Request<Class = request::InquiryClass> + Inquiry<Response = RawInquiryPayload>,
    {
    }
    fn generic<T>()
    where
        T: Request<Class = request::InquiryClass> + Inquiry<Response = Response>,
    {
    }
    fn typed<T>()
    where
        T: Request<Class = request::InquiryClass>
            + Inquiry<Response = visca_renamed::types::PowerStatus>
            + ResponseParser<Response = visca_renamed::types::PowerStatus>,
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

impl CompileTimeProfile for DownstreamProfile {
    const TYPED_SUPPORT: capabilities::TypedSupportSet = capabilities::TypedSupportSet::empty();
    const ID: Option<visca_renamed::profiles::ProfileId> = None;

    fn spec() -> visca_renamed::Result<ProfileSpec> {
        use capabilities::{CapabilityRange, ImageCaps, InquirySupport};
        let base = Base::spec()?;
        let mut caps = base.capabilities().clone();
        caps.profile_id = None;
        caps.model_name = "Downstream renamed profile".into();
        caps.default_camera_id = visca_renamed::types::IndividualCameraId::new(3)?;
        caps.typed_support = Self::TYPED_SUPPORT;
        caps.inquiry_support = InquirySupport::Full;
        let pan_tilt = caps.pan_tilt.as_mut().expect("base pan/tilt");
        pan_tilt.pan_speed = CapabilityRange::<u8>::new(1, 9);
        pan_tilt.tilt_speed = CapabilityRange::<u8>::new(1, 7);
        pan_tilt.coordinates = visca_renamed::profiles::PanTiltCoordinateConversion::new(
            capabilities::CoordinateSystem::SignedCentered,
            capabilities::PanTiltWireCodec::StandardVisca,
            10.0,
            15.0,
        )?;
        let zoom = caps.zoom.as_mut().expect("base zoom");
        zoom.digital_max = None;
        zoom.direct = false;
        zoom.variable = true;
        let focus = caps.focus.as_mut().expect("base focus");
        focus.speed = CapabilityRange::<u8>::new(0, 7);
        focus.auto_focus = true;
        focus.one_push = false;
        focus.near_limit_inquiry = true;
        caps.exposure
            .as_mut()
            .expect("base exposure")
            .backlight_compensation = true;
        caps.white_balance
            .as_mut()
            .expect("base white balance")
            .one_push = false;
        // Image-domain presence is independent of a typed operation grant.
        let mut image = caps.image.take().unwrap_or(ImageCaps {
            contrast_range: None,
            sharpness_range: None,
            saturation_range: None,
            hue_range: None,
            luminance_range: None,
            gamma_range: None,
            flip: false,
            mirror: false,
            requires_settings_save_for_flip: false,
            picture_effect: false,
        });
        image.flip = true;
        image.mirror = true;
        image.hue_range = Some(CapabilityRange::<u8>::new(0, 14));
        caps.image = Some(image);
        let presets = caps.presets.as_mut().expect("base presets");
        presets.highest = 8;
        presets.recall_axes = AffectedAxes::PAN_TILT;
        presets.tour = false;
        let power = caps.power.as_mut().expect("base power");
        power.on_time = Duration::from_millis(4_250);
        power.standby = Some(Duration::from_secs(5));
        ProfileSpec::builder(caps)
            .transports(TransportCompatibility::new().with_tcp(9876))
            .envelope(visca_renamed::profiles::ProfileEnvelope::RawVisca)
            .timing(
                visca_renamed::profiles::ProfileTiming::builder()
                    .ack_timeout(Duration::from_millis(120))
                    .command_timeouts(
                        visca_renamed::profiles::CommandTimeouts::DEFAULT
                            .with_quick_timeout(Duration::from_secs(6))
                            .with_movement_timeout(Duration::from_secs(30))
                            .with_preset_timeout(Duration::from_secs(60))
                            .with_long_running_timeout(Duration::from_secs(300))
                            .with_network_timeout(Duration::from_secs(6)),
                    )
                    .inquiry_timeout(Duration::from_millis(900))
                    .cancellation_timeout(Duration::from_millis(800))
                    .ambiguity_timeout(Duration::from_millis(700))
                    .busy_timeout(Duration::ZERO)
                    .raw_inquiry_reply_skew(Duration::ZERO)
                    .inquiry_spacing(Duration::ZERO)
                    .command_spacing(Duration::ZERO)
                    .build()?,
            )
            .maximum_command_sockets(visca_renamed::types::CommandSocketCount::ONE)
            .supports_operation_complete(base.supports_operation_complete())
            .supports_command_cancel(true)
            .position_inquiries(Some(AffectedAxes::MOVEMENT))
            .build()
    }
}

// A downstream profile opts into its domain explicitly. There is no blanket
// implementation that turns runtime metadata into a static permission.
impl HasImage for DownstreamProfile {}

fn assert_image_marker<P: HasImage>() {}

fn renamed_dependency_derive_and_profile_contract() -> visca_renamed::Result<()> {
    assert_inquiry_contracts();
    assert_image_marker::<DownstreamProfile>();
    #[cfg(not(feature = "range-helper-derives"))]
    visca_value_hygiene::assert_hygienic_expansion()?;
    #[cfg(not(feature = "range-helper-derives"))]
    visca_range_type_hygiene::assert_base_contract()?;
    #[cfg(feature = "range-helper-derives")]
    visca_range_type_helpers::assert_helper_contract();

    assert_eq!(RenamedValue::new(2)?, RenamedValue(2));
    assert!(RenamedValue::new(0).is_err());
    assert_eq!(RenamedEnum::from_wire(1)?, RenamedEnum::First);
    assert!(RenamedEnum::from_wire(3).is_err());

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
    assert!(profile.capabilities().image.is_some());
    assert_eq!(
        profile
            .capabilities()
            .image
            .as_ref()
            .and_then(|image| image.hue_range),
        Some(capabilities::CapabilityRange::<u8>::new(0, 14))
    );
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
