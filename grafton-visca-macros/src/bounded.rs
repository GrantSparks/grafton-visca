// SPDX-License-Identifier: MIT OR Apache-2.0
//
// Copyright (c) 2024 Grafton Machine Shed <team@grafton.ai>

//! The one checked-newtype generator.
//!
//! `#[derive(ViscaValue)]` and `visca_range_type!` both describe a
//! [`BoundedNewtype`] and call [`BoundedNewtype::expand`], so every generated
//! type has the same constructor, bounds, accessor and error contract:
//! `MIN`/`MAX` typed as `Self`, a checked `new`, `pub const fn value(self)`,
//! `TryFrom<inner>` and `From<Self> for inner`. A value outside a range is
//! `Error::ParameterOutOfRange`, whose bounds are `i32`, so a range's inner
//! type must implement the sealed `RangeInner` support trait (the primitives
//! that widen into `i32` without loss); a range's `new` is then a `const fn`.
//! A value outside a declared set is `Error::InvalidParameter`. What differs
//! is only the declaration: the declaration adapter also derives the standard
//! and helper traits, and the derive also emits `Display`.

use proc_macro2::{Span, TokenStream};
use quote::{quote, quote_spanned};
use syn::{
    meta::ParseNestedMeta, spanned::Spanned, Attribute, Data, DeriveInput, Error, Expr, ExprArray,
    Fields, Ident, Lit, LitStr, Result, Token, Type, UnOp,
};

use crate::attr::{unknown_key, AttrSlot};

pub(crate) const ATTRIBUTE: &str = "visca_value";
const ATTRIBUTE_KEYS: &str = "one of `min`, `max`, `valid_values`, `display_format`, or \
                              `display_prefix`";

/// The values a checked newtype accepts.
pub(crate) enum Domain {
    /// Every value in `min..=max`.
    Range { min: TokenStream, max: TokenStream },
    /// Exactly the values of a non-empty array expression.
    Set(ExprArray),
}

/// How a generated `Display` renders the raw value.
#[derive(Clone, Copy)]
enum Radix {
    Decimal,
    Hex,
    Binary,
}

/// A generated `Display` implementation.
pub(crate) struct DisplaySpec {
    radix: Radix,
    prefix: Option<String>,
}

/// Everything the generator needs to emit one checked newtype's API.
pub(crate) struct BoundedNewtype {
    pub(crate) name: Ident,
    pub(crate) inner: Type,
    pub(crate) domain: Domain,
    pub(crate) display: Option<DisplaySpec>,
}

/// The domain and display declared by `#[visca_value(...)]` attributes.
pub(crate) struct ValueAttributes {
    pub(crate) domain: Domain,
    pub(crate) display: DisplaySpec,
}

impl BoundedNewtype {
    /// Emits the inherent API, the conversions and the optional `Display`.
    ///
    /// `availability` carries the declaration's `cfg` attributes onto every
    /// generated impl.
    pub(crate) fn expand(
        &self,
        crate_path: &TokenStream,
        availability: &[Attribute],
    ) -> TokenStream {
        let Self {
            name,
            inner,
            domain,
            display,
        } = self;

        let i32_type = quote! { ::core::primitive::i32 };
        let (constants, constructor, guard) = match domain {
            Domain::Range { min, max } => {
                let message = format!(
                    "`{}` declares `min` greater than `max`",
                    name.to_string().trim_start_matches("r#")
                );
                // Spanned at the inner type so an inner type that cannot be
                // widened into `i32` is reported where it was declared.
                let require_inner = quote_spanned! {declared_span(inner)=>
                    __grafton_visca_range_inner::<#inner>();
                };
                (
                    quote! {
                        /// The smallest accepted value.
                        pub const MIN: Self = Self(#min);
                        /// The largest accepted value.
                        pub const MAX: Self = Self(#max);
                    },
                    // `RangeInner` admits only primitives that widen into `i32`
                    // without loss, so the `as` casts are exact and `new` can
                    // be a `const fn`.
                    quote! {
                        /// Creates a value after checking it against `MIN..=MAX`.
                        ///
                        /// # Errors
                        /// Returns `Error::ParameterOutOfRange` when `value` lies
                        /// outside `MIN..=MAX`.
                        pub const fn new(
                            value: #inner,
                        ) -> ::core::result::Result<Self, #crate_path::Error> {
                            if value < Self::MIN.0 || value > Self::MAX.0 {
                                return ::core::result::Result::Err(
                                    #crate_path::Error::parameter_out_of_range(
                                        ::core::stringify!(#name),
                                        value as #i32_type,
                                        Self::MIN.0 as #i32_type,
                                        Self::MAX.0 as #i32_type,
                                    ),
                                );
                            }
                            ::core::result::Result::Ok(Self(value))
                        }
                    },
                    Some(quote! {
                        #(#availability)*
                        const _: () = {
                            const fn __grafton_visca_range_inner<
                                T: #crate_path::__macro_support::RangeInner,
                            >() {
                            }
                            #require_inner
                            ::core::assert!(#name::MIN.0 <= #name::MAX.0, #message);
                        };
                    }),
                )
            }
            Domain::Set(values) => {
                // The declared values stay local to each item, so nothing is
                // added to the caller's type beyond the documented API.
                let local_values = quote! {
                    const VALUES: &[#inner] = &#values;
                };
                let extreme = |keep: TokenStream| {
                    quote! {
                        {
                            #local_values
                            let mut extreme = VALUES[0];
                            let mut index = 1;
                            while index < VALUES.len() {
                                if #keep {
                                    extreme = VALUES[index];
                                }
                                index += 1;
                            }
                            Self(extreme)
                        }
                    }
                };
                let min = extreme(quote! { VALUES[index] < extreme });
                let max = extreme(quote! { VALUES[index] > extreme });
                (
                    quote! {
                        /// The smallest accepted value.
                        pub const MIN: Self = #min;
                        /// The largest accepted value.
                        pub const MAX: Self = #max;
                    },
                    quote! {
                        /// Creates a value after checking it against the declared
                        /// values.
                        ///
                        /// # Errors
                        /// Returns `Error::InvalidParameter` when `value` is not one
                        /// of the declared values.
                        pub fn new(
                            value: #inner,
                        ) -> ::core::result::Result<Self, #crate_path::Error> {
                            #local_values
                            if !VALUES.contains(&value) {
                                return ::core::result::Result::Err(
                                    #crate_path::Error::invalid_parameter(
                                        ::core::stringify!(#name),
                                        ::std::borrow::Cow::Owned(::std::format!("{value}")),
                                        ::std::borrow::Cow::Owned(::std::format!(
                                            "must be one of {:?}",
                                            VALUES
                                        )),
                                    ),
                                );
                            }
                            ::core::result::Result::Ok(Self(value))
                        }
                    },
                    None,
                )
            }
        };

        let display = display.as_ref().map(|display| {
            let body = display.body();
            quote! {
                #(#availability)*
                impl ::core::fmt::Display for #name {
                    fn fmt(&self, f: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {
                        #body
                    }
                }
            }
        });

        quote! {
            #(#availability)*
            impl #name {
                #constants

                #constructor

                /// Returns the raw value.
                #[must_use]
                pub const fn value(self) -> #inner {
                    self.0
                }
            }

            #guard

            #(#availability)*
            impl ::core::convert::TryFrom<#inner> for #name {
                type Error = #crate_path::Error;

                fn try_from(value: #inner) -> ::core::result::Result<Self, Self::Error> {
                    Self::new(value)
                }
            }

            #(#availability)*
            impl ::core::convert::From<#name> for #inner {
                fn from(value: #name) -> Self {
                    value.0
                }
            }

            #display
        }
    }
}

impl DisplaySpec {
    fn body(&self) -> TokenStream {
        let spec = match self.radix {
            Radix::Decimal => "{}",
            Radix::Hex => "{:#04x}",
            Radix::Binary => "{:#b}",
        };
        match &self.prefix {
            Some(prefix) => {
                let format = format!("{{}} {spec}");
                quote! { ::core::write!(f, #format, #prefix, self.0) }
            }
            None => quote! { ::core::write!(f, #spec, self.0) },
        }
    }
}

/// The span of the type as written, looking through the invisible groups a
/// declarative macro wraps around a captured `ty` fragment.
fn declared_span(ty: &Type) -> Span {
    match ty {
        Type::Group(group) => declared_span(&group.elem),
        other => other.span(),
    }
}

/// Requires a non-generic tuple struct with exactly one field, returning that
/// field's type.
fn single_field_type<'a>(
    macro_name: &str,
    span_source: &dyn quote::ToTokens,
    fields: &'a Fields,
) -> Result<&'a Type> {
    match fields {
        Fields::Unnamed(fields) if fields.unnamed.len() == 1 => Ok(&fields.unnamed[0].ty),
        _ => Err(Error::new_spanned(
            span_source,
            format!("{macro_name} requires a tuple struct with exactly one field"),
        )),
    }
}

/// `#[derive(ViscaValue)]`.
pub(crate) fn derive_visca_value(input: &DeriveInput) -> Result<TokenStream> {
    let Data::Struct(data) = &input.data else {
        return Err(Error::new_spanned(
            input,
            "ViscaValue can only be derived for structs",
        ));
    };
    if !input.generics.params.is_empty() {
        return Err(Error::new_spanned(
            &input.generics,
            "ViscaValue does not support generic structs",
        ));
    }
    let inner = single_field_type("ViscaValue", input, &data.fields)?.clone();
    let ValueAttributes { domain, display } =
        parse_value_attributes(&input.attrs, input.ident.span())?;

    Ok(BoundedNewtype {
        name: input.ident.clone(),
        inner,
        domain,
        display: Some(display),
    }
    .expand(&crate::crate_path::grafton_visca(), &[]))
}

/// Parses every `#[visca_value(...)]` attribute among `attrs`.
///
/// `item_span` locates the diagnostic when no domain is declared.
pub(crate) fn parse_value_attributes(
    attrs: &[Attribute],
    item_span: Span,
) -> Result<ValueAttributes> {
    let mut min = AttrSlot::<TokenStream>::default();
    let mut max = AttrSlot::<TokenStream>::default();
    let mut valid_values = AttrSlot::<ExprArray>::default();
    let mut radix = AttrSlot::<Radix>::default();
    let mut prefix = AttrSlot::<String>::default();

    for attr in attrs.iter().filter(|attr| attr.path().is_ident(ATTRIBUTE)) {
        attr.parse_nested_meta(|meta| {
            if meta.path.is_ident("min") {
                let bound = parse_bound(&string_value(&meta, "min")?, "min")?;
                min.set(ATTRIBUTE, &meta, bound)
            } else if meta.path.is_ident("max") {
                let bound = parse_bound(&string_value(&meta, "max")?, "max")?;
                max.set(ATTRIBUTE, &meta, bound)
            } else if meta.path.is_ident("valid_values") {
                let literal = string_value(&meta, "valid_values")?;
                let values = literal
                    .parse::<ExprArray>()
                    .ok()
                    .filter(|values| !values.elems.is_empty())
                    .ok_or_else(|| {
                        Error::new(
                            literal.span(),
                            "`valid_values` must be a non-empty array literal",
                        )
                    })?;
                valid_values.set(ATTRIBUTE, &meta, values)
            } else if meta.path.is_ident("display_format") {
                let literal = string_value(&meta, "display_format")?;
                let value = match literal.value().as_str() {
                    "decimal" => Radix::Decimal,
                    "hex" => Radix::Hex,
                    "binary" => Radix::Binary,
                    _ => {
                        return Err(Error::new(
                            literal.span(),
                            "`display_format` must be \"hex\", \"binary\", or \"decimal\"",
                        ))
                    }
                };
                radix.set(ATTRIBUTE, &meta, value)
            } else if meta.path.is_ident("display_prefix") {
                let literal = string_value(&meta, "display_prefix")?;
                prefix.set(ATTRIBUTE, &meta, literal.value())
            } else {
                Err(unknown_key(&meta, ATTRIBUTE, ATTRIBUTE_KEYS))
            }
        })?;
    }

    let domain = match (
        min.into_entry(),
        max.into_entry(),
        valid_values.into_entry(),
    ) {
        (Some((min, _)), Some((max, _)), None) => Domain::Range { min, max },
        (None, None, Some((values, _))) => Domain::Set(values),
        (Some(_), _, Some((_, span))) | (_, Some(_), Some((_, span))) => {
            return Err(Error::new(
                span,
                "`valid_values` cannot be combined with `min` or `max`",
            ))
        }
        (Some((_, span)), None, None) => {
            return Err(Error::new(
                span,
                "`min` requires `max`; specify both bounds or use `valid_values`",
            ))
        }
        (None, Some((_, span)), None) => {
            return Err(Error::new(
                span,
                "`max` requires `min`; specify both bounds or use `valid_values`",
            ))
        }
        (None, None, None) => {
            return Err(Error::new(
                item_span,
                "`visca_value` requires `min` and `max`, or `valid_values`",
            ))
        }
    };

    Ok(ValueAttributes {
        domain,
        display: DisplaySpec {
            radix: radix.get().copied().unwrap_or(Radix::Decimal),
            prefix: prefix.into_value(),
        },
    })
}

fn string_value(meta: &ParseNestedMeta<'_>, name: &str) -> Result<LitStr> {
    if !meta.input.peek(Token![=]) {
        return Err(meta.error(format!(
            "`{name}` requires a string literal value, for example `{name} = \"...\"`"
        )));
    }
    let value = meta.value()?;
    let value_span = value.span();
    value.parse::<LitStr>().map_err(|_| {
        Error::new(
            value_span,
            format!("`{name}` must be provided as a string literal"),
        )
    })
}

/// Parses a bound written as an unsuffixed, optionally negated, integer
/// literal inside a string.
fn parse_bound(literal: &LitStr, name: &str) -> Result<TokenStream> {
    let invalid = || {
        Error::new(
            literal.span(),
            format!("`{name}` must be an unsuffixed integer literal string"),
        )
    };
    let expression = literal.parse::<Expr>().map_err(|_| invalid())?;
    let integer = match &expression {
        Expr::Unary(unary) if matches!(unary.op, UnOp::Neg(_)) => unary.expr.as_ref(),
        other => other,
    };
    match integer {
        Expr::Lit(expr) => match &expr.lit {
            Lit::Int(integer) if integer.suffix().is_empty() => Ok(quote! { #expression }),
            _ => Err(invalid()),
        },
        _ => Err(invalid()),
    }
}
