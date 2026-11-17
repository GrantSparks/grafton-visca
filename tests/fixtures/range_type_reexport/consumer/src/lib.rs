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

#[value(crate = range_macro_provider::visca, valid_values = [-3, 2, 7])]
#[cfg_attr(
    feature = "range-helper-derives",
    schemars(rename = "ReexportedSparseDomain")
)]
pub struct ReexportedSparse(i8);

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
    use super::{
        r#match, ConditionalAttrRange, ConditionalRange, ReexportedRange, ReexportedSparse,
    };

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
        assert_eq!(ReexportedSparse::new(-3).unwrap().value(), -3);
        assert!(ReexportedSparse::new(3).is_err());
    }

    #[cfg(feature = "range-helper-derives")]
    #[allow(dead_code)]
    #[derive(range_macro_provider::visca::__macro_support::schemars::JsonSchema)]
    #[schemars(crate = "range_macro_provider::visca::__macro_support::schemars")]
    struct SchemaContainer {
        range: ReexportedRange,
        sparse: ReexportedSparse,
        conditional: ConditionalAttrRange,
    }

    #[cfg(feature = "range-helper-derives")]
    #[test]
    fn reexported_scalar_schemas_retain_domains_and_metadata() {
        use range_macro_provider::visca::__macro_support::schemars::schema_for;

        let range = schema_for!(ReexportedRange);
        assert_eq!(range.get("type").unwrap(), "integer");
        assert_eq!(range.get("title").unwrap(), "Reexported range");
        assert_eq!(range.get("minimum").unwrap(), 2);
        assert_eq!(range.get("maximum").unwrap(), 9);

        let sparse = schema_for!(ReexportedSparse);
        assert_eq!(sparse.get("type").unwrap(), "integer");
        assert_eq!(sparse.get("minimum").unwrap(), -3);
        assert_eq!(sparse.get("maximum").unwrap(), 7);
        let members: Vec<_> = sparse
            .get("enum")
            .unwrap()
            .as_array()
            .unwrap()
            .iter()
            .map(|value| value.as_i64().unwrap())
            .collect();
        assert_eq!(members, [-3, 2, 7]);

        let nested = schema_for!(SchemaContainer);
        let properties = nested.get("properties").unwrap();
        assert_eq!(properties["range"]["$ref"], "#/$defs/ReexportedRange");
        assert_eq!(
            properties["sparse"]["$ref"],
            "#/$defs/ReexportedSparseDomain"
        );
        assert_eq!(
            properties["conditional"]["$ref"],
            "#/$defs/ConditionalAttrRange"
        );
        let definitions = nested.get("$defs").unwrap();
        assert_eq!(definitions["ReexportedRange"]["minimum"], 2);
        assert_eq!(definitions["ReexportedRange"]["maximum"], 9);
        assert_eq!(
            definitions["ReexportedSparseDomain"]["enum"],
            sparse.as_value()["enum"]
        );
        assert_eq!(definitions["ConditionalAttrRange"]["minimum"], 5);
        assert_eq!(definitions["ConditionalAttrRange"]["maximum"], 6);
    }
}
