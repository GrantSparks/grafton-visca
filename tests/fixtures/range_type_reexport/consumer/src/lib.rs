//! Re-export-only downstream expansion contract.
//!
//! This crate intentionally has no direct dependency on `grafton-visca`,
//! `serde`, `schemars`, or `ts-rs`.

use range_macro_provider::value;

#[value(crate = range_macro_provider::visca, min = 2, max = 9)]
/// A range declared through a one-hop macro re-export.
#[cfg_attr(feature = "range-helper-derives", serde(rename = "ReexportedRange"))]
#[cfg_attr(feature = "range-helper-derives", schemars(title = "Reexported range"))]
#[cfg_attr(feature = "range-helper-derives", ts(rename = "ReexportedRange"))]
pub struct ReexportedRange(u8);

#[value(crate = range_macro_provider::visca, min = 3, max = 4)]
#[cfg(all())]
pub struct ConditionalRange(u8);

#[value(crate = range_macro_provider::visca, min = 30, max = 40)]
#[cfg(not(all()))]
pub struct ConditionalRange(u8);

#[value(crate = range_macro_provider::visca, min = 5, max = 6)]
#[cfg_attr(all(), cfg(all()))]
#[cfg_attr(
    feature = "range-helper-derives",
    serde(rename = "ConditionalAttrRange")
)]
#[cfg_attr(
    feature = "range-helper-derives",
    schemars(rename = "ConditionalAttrRange")
)]
#[cfg_attr(feature = "range-helper-derives", ts(rename = "ConditionalAttrRange"))]
pub struct ConditionalAttrRange(u8);

#[value(crate = range_macro_provider::visca, min = 7, max = 8)]
#[cfg(any())]
pub struct DisabledRange(u8);

#[allow(non_snake_case)]
mod __grafton_visca_value_support_DisabledRange {}

#[allow(dead_code)]
struct DisabledRange;

#[value(crate = range_macro_provider::visca, min = 7, max = 8)]
#[cfg(any())]
pub struct MissingTypeRange(u8);

#[value(crate = range_macro_provider::visca, min = 9, max = 10)]
#[allow(non_camel_case_types)]
pub struct r#match(u8);

#[cfg(test)]
mod tests {
    use super::{r#match, ConditionalAttrRange, ConditionalRange, ReexportedRange};

    #[test]
    fn reexported_range_keeps_its_validation_contract() {
        assert_eq!(ReexportedRange::new(2).unwrap().value(), 2);
        assert_eq!(ReexportedRange::new(9).unwrap().value(), 9);
        assert!(ReexportedRange::new(1).is_err());
        assert!(ReexportedRange::new(10).is_err());
        assert_eq!(ConditionalRange::new(4).unwrap().value(), 4);
        assert!(ConditionalRange::new(30).is_err());
        assert_eq!(ConditionalAttrRange::new(6).unwrap().value(), 6);
        assert_eq!(r#match::new(9).unwrap().value(), 9);
    }
}
