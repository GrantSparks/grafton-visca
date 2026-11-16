//! Conditional value variants retain their presence in every generated method.

use grafton_visca::{value, value_enum, wire::WireValue};

#[value_enum]
enum ConditionalUnit {
    #[wire(0)]
    Always,
    #[cfg(any())]
    #[wire(0)]
    DisabledAlias,
    #[cfg(feature = "serde")]
    #[wire(1)]
    WithHelpers,
    #[cfg(not(feature = "serde"))]
    #[wire(1)]
    WithoutHelpers,
    #[cfg_attr(all(), cfg_attr(all(), cfg(any())), serde(rename = "disabled"))]
    #[wire(2)]
    NestedDisabled,
    #[cfg_attr(feature = "serde", cfg(any()), serde(rename = "disabled"))]
    #[wire(3)]
    PlainOnly,
    #[cfg_attr(feature = "serde", cfg(all()), serde(rename = "mixed_helper"))]
    #[wire(4)]
    MixedHelper,
}

#[value(min = 1, max = 3)]
struct Level(u8);

#[value_enum]
enum ConditionalPayload {
    #[wire(0)]
    Zero,
    #[cfg(feature = "serde")]
    Nonzero(Level),
}

#[value_enum]
enum SelectedPayload {
    #[cfg(feature = "serde")]
    WithHelpers(Level),
    #[cfg(not(feature = "serde"))]
    WithoutHelpers(Level),
}

#[value_enum]
enum DisabledOnly {
    #[cfg(any())]
    Gone = 99,
}

#[test]
fn unit_variants_have_the_same_presence_in_the_enum_and_wire_codec() {
    assert_eq!(
        ConditionalUnit::from_wire(0).unwrap(),
        ConditionalUnit::Always
    );
    assert_eq!(ConditionalUnit::Always.to_wire(), 0);
    assert_eq!(ConditionalUnit::Always.to_string(), "Always");
    #[cfg(feature = "serde")]
    {
        assert_eq!(
            ConditionalUnit::from_wire(1).unwrap(),
            ConditionalUnit::WithHelpers
        );
        assert_eq!(ConditionalUnit::WithHelpers.to_wire(), 1);
        assert_eq!(ConditionalUnit::WithHelpers.to_string(), "WithHelpers");
        assert!(ConditionalUnit::from_wire(3).is_err());
    }
    #[cfg(not(feature = "serde"))]
    {
        assert_eq!(
            ConditionalUnit::from_wire(1).unwrap(),
            ConditionalUnit::WithoutHelpers
        );
        assert_eq!(ConditionalUnit::WithoutHelpers.to_wire(), 1);
        assert_eq!(
            ConditionalUnit::WithoutHelpers.to_string(),
            "WithoutHelpers"
        );
        assert_eq!(
            ConditionalUnit::from_wire(3).unwrap(),
            ConditionalUnit::PlainOnly
        );
        assert_eq!(ConditionalUnit::PlainOnly.to_wire(), 3);
        assert_eq!(ConditionalUnit::PlainOnly.to_string(), "PlainOnly");
    }
    assert!(ConditionalUnit::from_wire(2).is_err());
    assert!(ConditionalUnit::from_wire(99).is_err());
    assert_eq!(
        ConditionalUnit::from_wire(4).unwrap(),
        ConditionalUnit::MixedHelper
    );
    assert_eq!(ConditionalUnit::MixedHelper.to_wire(), 4);
    assert_eq!(ConditionalUnit::MixedHelper.to_string(), "MixedHelper");
}

#[test]
fn absent_payload_restores_unknown_code_errors() {
    assert_eq!(
        ConditionalPayload::from_wire(0).unwrap(),
        ConditionalPayload::Zero
    );
    assert_eq!(ConditionalPayload::Zero.to_wire(), 0);
    assert_eq!(ConditionalPayload::Zero.to_string(), "Zero");
    #[cfg(feature = "serde")]
    {
        let value = ConditionalPayload::from_wire(2).unwrap();
        assert_eq!(value, ConditionalPayload::Nonzero(Level::new(2).unwrap()));
        assert_eq!(value.to_wire(), 2);
        assert_eq!(value.to_string(), "Nonzero 2");
    }
    #[cfg(not(feature = "serde"))]
    assert!(ConditionalPayload::from_wire(2).is_err());
    assert!(ConditionalPayload::from_wire(4).is_err());
}

#[test]
fn mutually_exclusive_payloads_select_one_fallback() {
    let level = Level::new(3).unwrap();
    let value = SelectedPayload::from_wire(3).unwrap();
    #[cfg(feature = "serde")]
    {
        assert_eq!(value, SelectedPayload::WithHelpers(level));
        assert_eq!(value.to_string(), "WithHelpers 3");
    }
    #[cfg(not(feature = "serde"))]
    {
        assert_eq!(value, SelectedPayload::WithoutHelpers(level));
        assert_eq!(value.to_string(), "WithoutHelpers 3");
    }
    assert_eq!(value.to_wire(), 3);
    assert!(SelectedPayload::from_wire(0).is_err());
    assert!(SelectedPayload::from_wire(4).is_err());
}

#[test]
fn all_disabled_variants_do_not_grant_any_wire_code() {
    assert!(DisabledOnly::from_wire(0).is_err());
    assert!(DisabledOnly::from_wire(99).is_err());
}

#[cfg(feature = "serde")]
#[test]
fn helper_attributes_stay_on_variants() {
    assert_eq!(
        serde_json::to_string(&ConditionalUnit::MixedHelper).unwrap(),
        "\"mixed_helper\""
    );
    assert_eq!(
        serde_json::from_str::<ConditionalUnit>("\"mixed_helper\"").unwrap(),
        ConditionalUnit::MixedHelper
    );
    assert!(serde_json::from_str::<ConditionalUnit>("\"disabled\"").is_err());
}
