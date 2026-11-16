#![deny(unexpected_cfgs)]

// The non-ts-rs legs deliberately shadow every prelude item and macro emitted
// by the range macro. ts-rs v12 itself emits a few caller-scope standard names,
// so that leg keeps only the shadows compatible with the upstream derive; the
// renamed phase-4 fixture separately pins the real derive's exact behavior.
mod visca_range_hygiene {
    use grafton_visca::value;

    #[cfg(not(feature = "ts-rs"))]
    #[allow(dead_code)]
    mod std {}

    #[allow(dead_code)]
    struct Result<T, E>(T, E);

    #[allow(dead_code)]
    trait TryFrom<T> {}

    #[allow(dead_code)]
    trait From<T> {}

    #[cfg(not(feature = "ts-rs"))]
    #[allow(dead_code)]
    struct Option<T>(T);

    #[allow(dead_code)]
    enum CapturedVariants {
        Ok,
        Err,
        Some,
        None,
    }

    #[cfg(not(feature = "ts-rs"))]
    #[allow(unused_imports)]
    use CapturedVariants::{Err, None, Ok, Some};

    #[cfg(not(feature = "ts-rs"))]
    #[allow(unused_macros)]
    macro_rules! format {
        ($($tokens:tt)*) => {
            compile_error!("value expansion captured `format!`")
        };
    }

    #[cfg(not(feature = "ts-rs"))]
    #[allow(unused_macros)]
    macro_rules! write {
        ($($tokens:tt)*) => {
            compile_error!("value expansion captured `write!`")
        };
    }

    #[cfg(not(feature = "ts-rs"))]
    #[allow(unused_macros)]
    macro_rules! stringify {
        ($($tokens:tt)*) => {
            compile_error!("value expansion captured `stringify!`")
        };
    }

    #[cfg(not(feature = "ts-rs"))]
    #[allow(unused_macros)]
    macro_rules! module_path {
        ($($tokens:tt)*) => {
            compile_error!("value expansion captured `module_path!`")
        };
    }

    #[value(min = 2, max = 4)]
    /// Downstream-defined value with a finite valid range.
    pub struct HygienicRange(u8);

    // Availability applies to the entire generated item family. In
    // particular, the two declarations must not leave colliding helper
    // modules behind when serde/schemars/ts-rs support is enabled.
    #[value(min = 5, max = 6)]
    #[cfg(all())]
    pub struct ConditionalRange(u8);

    #[value(min = 50, max = 60)]
    #[cfg(not(all()))]
    pub struct ConditionalRange(u8);

    // Only availability predicates may be copied onto the generated module
    // and impls. The helper attributes remain attached solely to the struct.
    #[value(min = 7, max = 8)]
    #[cfg_attr(all(), cfg(all()))]
    #[cfg_attr(feature = "serde", serde(rename = "ConditionalAttrRange"))]
    #[cfg_attr(feature = "schemars", schemars(rename = "ConditionalAttrRange"))]
    #[cfg_attr(feature = "ts-rs", ts(rename = "ConditionalAttrRange"))]
    pub struct ConditionalAttrRange(u8);

    // A disabled declaration must leave no helper module, type, or impl
    // residue. These caller-owned items deliberately reuse both names.
    #[value(min = 1, max = 2)]
    #[cfg(any())]
    pub struct DisabledRange(u8);

    #[allow(non_snake_case)]
    mod __grafton_visca_value_support_DisabledRange {}

    #[allow(dead_code)]
    struct DisabledRange;

    // This second disabled declaration intentionally has no replacement type:
    // an ungated generated impl therefore fails with an unresolved-type error.
    #[value(min = 1, max = 2)]
    #[cfg(any())]
    pub struct MissingTypeRange(u8);

    #[value(min = 9, max = 10)]
    #[allow(non_camel_case_types)]
    pub struct r#match(u8);

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

    pub(super) fn assert_derive_contract() {
        assert_derive_traits::<HygienicRange>();
        let lower = HygienicRange::new(2).expect("lower bound");
        let upper = HygienicRange::new(4).expect("upper bound");
        assert_eq!(::std::format!("{lower:?}"), "HygienicRange(2)");
        assert_eq!(lower.cmp(&upper), ::core::cmp::Ordering::Less);
        assert_eq!(
            lower.partial_cmp(&upper),
            ::core::option::Option::Some(::core::cmp::Ordering::Less)
        );
        assert_eq!(
            ConditionalRange::new(6)
                .expect("selected cfg range")
                .value(),
            6
        );
        assert_eq!(
            ConditionalAttrRange::new(8)
                .expect("cfg_attr-selected range")
                .value(),
            8
        );
        assert_eq!(r#match::new(9).expect("raw identifier range").value(), 9);
    }
}

fn main() {
    use visca_range_hygiene::HygienicRange;

    visca_range_hygiene::assert_derive_contract();
    assert_eq!(HygienicRange::MIN.value(), 2);
    assert_eq!(HygienicRange::MAX.value(), 4);

    let value = HygienicRange::new(3).expect("in-range value");
    assert_eq!(value.value(), 3);
    assert_eq!(
        ::std::format!("{}", HygienicRange::new(1).expect_err("below range")),
        "Parameter out of range: HygienicRange = 1 (valid range: 2..=4)"
    );
    assert_eq!(
        <HygienicRange as ::core::convert::TryFrom<u8>>::try_from(4)
            .expect("upper bound conversion"),
        HygienicRange::new(4).expect("upper bound construction")
    );
    assert_eq!(<u8 as ::core::convert::From<HygienicRange>>::from(value), 3);

    #[cfg(feature = "serde")]
    assert_serde_derives::<HygienicRange>();
    #[cfg(feature = "schemars")]
    assert_schemars_derives();
    #[cfg(feature = "ts-rs")]
    assert_ts_rs_derives();
}

#[cfg(feature = "serde")]
fn assert_serde_derives<T>()
where
    T: grafton_visca::__macro_support::serde::Serialize
        + for<'de> grafton_visca::__macro_support::serde::Deserialize<'de>,
{
}

#[cfg(feature = "schemars")]
fn assert_schemars_derives() {
    use grafton_visca::__macro_support::schemars::JsonSchema;

    assert!(!<visca_range_hygiene::HygienicRange as JsonSchema>::inline_schema());
    assert_eq!(
        <visca_range_hygiene::HygienicRange as JsonSchema>::schema_name(),
        "HygienicRange"
    );
    assert!(
        <visca_range_hygiene::HygienicRange as JsonSchema>::schema_id()
            .ends_with("::visca_range_hygiene::HygienicRange")
    );
}

#[cfg(feature = "ts-rs")]
fn assert_ts_rs_derives() {
    use grafton_visca::__macro_support::ts_rs::TS;

    let config = grafton_visca::__macro_support::ts_rs::Config::default();
    assert_eq!(
        <visca_range_hygiene::HygienicRange as TS>::docs().as_deref(),
        Some("/**\n * Downstream-defined value with a finite valid range.\n */\n")
    );
    assert_eq!(
        <visca_range_hygiene::HygienicRange as TS>::ident(&config),
        "HygienicRange"
    );
    assert_eq!(
        <visca_range_hygiene::HygienicRange as TS>::name(&config),
        "HygienicRange"
    );
    assert_eq!(
        <visca_range_hygiene::HygienicRange as TS>::decl(&config),
        "type HygienicRange = number;"
    );
    assert_eq!(
        <visca_range_hygiene::HygienicRange as TS>::output_path(),
        ::core::option::Option::Some(::std::path::PathBuf::from("HygienicRange.ts"))
    );
    assert!(::std::panic::catch_unwind(|| {
        <visca_range_hygiene::HygienicRange as TS>::inline_flattened(&config)
    })
    .is_err());
}
