// SPDX-License-Identifier: MIT OR Apache-2.0
//
// Copyright (c) 2024 Grafton Machine Shed <team@grafton.ai>

//! Value type macros for the grafton-visca library
//!
//! This module contains the ViscaValue derive macro and related value type helpers
//! for generating validated value types with range checking and display formatting.

use proc_macro::TokenStream;
use proc_macro2::Span;
use quote::{quote, ToTokens};
use syn::{
    meta::ParseNestedMeta, parse_macro_input, spanned::Spanned, DeriveInput, Error, Expr, Lit,
    LitStr, Result, Token, UnOp,
};

const VISCA_VALUE_ATTRIBUTE_KEYS: &str =
    "`min`, `max`, `valid_values`, `display_format`, or `display_prefix`";

struct AttributeValue<T> {
    value: T,
    key_span: Span,
}

#[derive(Default)]
struct ViscaValueAttributes {
    min: Option<AttributeValue<proc_macro2::TokenStream>>,
    max: Option<AttributeValue<proc_macro2::TokenStream>>,
    valid_values: Option<AttributeValue<String>>,
    display_format: Option<AttributeValue<String>>,
    display_prefix: Option<AttributeValue<String>>,
}

fn set_attribute<T>(
    slot: &mut Option<AttributeValue<T>>,
    value: T,
    name: &str,
    key_span: Span,
) -> Result<()> {
    if let Some(previous) = slot.as_ref() {
        let mut error = Error::new(
            key_span,
            format!("duplicate `visca_value` attribute `{name}`"),
        );
        error.combine(Error::new(
            previous.key_span,
            format!("first `{name}` specified here"),
        ));
        return Err(error);
    }

    *slot = Some(AttributeValue { value, key_span });
    Ok(())
}

fn parse_string_value(meta: &ParseNestedMeta<'_>, name: &str) -> Result<LitStr> {
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

fn invalid_bound_error(literal: &LitStr, name: &str) -> Error {
    Error::new(
        literal.span(),
        format!("`{name}` must be an unsuffixed integer literal string"),
    )
}

fn parse_bound(literal: &LitStr, name: &str) -> Result<proc_macro2::TokenStream> {
    let tokens = literal
        .value()
        .parse::<proc_macro2::TokenStream>()
        .map_err(|_| invalid_bound_error(literal, name))?;
    let expression =
        syn::parse2::<Expr>(tokens.clone()).map_err(|_| invalid_bound_error(literal, name))?;

    let integer = match &expression {
        Expr::Lit(expr) => match &expr.lit {
            Lit::Int(integer) => integer,
            _ => return Err(invalid_bound_error(literal, name)),
        },
        Expr::Unary(expr) if matches!(expr.op, UnOp::Neg(_)) => match expr.expr.as_ref() {
            Expr::Lit(expr) => match &expr.lit {
                Lit::Int(integer) => integer,
                _ => return Err(invalid_bound_error(literal, name)),
            },
            _ => return Err(invalid_bound_error(literal, name)),
        },
        _ => return Err(invalid_bound_error(literal, name)),
    };

    if !integer.suffix().is_empty() {
        return Err(invalid_bound_error(literal, name));
    }

    Ok(tokens)
}

fn unknown_attribute_error(meta: &ParseNestedMeta<'_>) -> Error {
    let name = meta.path.to_token_stream().to_string();
    Error::new_spanned(
        &meta.path,
        format!(
            "unknown `visca_value` attribute `{name}`; expected one of {VISCA_VALUE_ATTRIBUTE_KEYS}"
        ),
    )
}

fn parse_visca_value_attributes(attributes: &[syn::Attribute]) -> Result<ViscaValueAttributes> {
    let mut attrs = ViscaValueAttributes::default();

    for attr in attributes {
        if !attr.path().is_ident("visca_value") {
            continue;
        }

        attr.parse_nested_meta(|meta| {
            let key_span = meta.path.span();
            if meta.path.is_ident("min") {
                let literal = parse_string_value(&meta, "min")?;
                set_attribute(
                    &mut attrs.min,
                    parse_bound(&literal, "min")?,
                    "min",
                    key_span,
                )
            } else if meta.path.is_ident("max") {
                let literal = parse_string_value(&meta, "max")?;
                set_attribute(
                    &mut attrs.max,
                    parse_bound(&literal, "max")?,
                    "max",
                    key_span,
                )
            } else if meta.path.is_ident("valid_values") {
                let literal = parse_string_value(&meta, "valid_values")?;
                let values = syn::parse_str::<syn::ExprArray>(&literal.value()).map_err(|_| {
                    Error::new(
                        literal.span(),
                        "`valid_values` must be a non-empty array literal",
                    )
                })?;
                if values.elems.is_empty() {
                    return Err(Error::new(
                        literal.span(),
                        "`valid_values` must be a non-empty array literal",
                    ));
                }
                set_attribute(
                    &mut attrs.valid_values,
                    literal.value(),
                    "valid_values",
                    key_span,
                )
            } else if meta.path.is_ident("display_format") {
                let literal = parse_string_value(&meta, "display_format")?;
                let value = literal.value();
                match value.as_str() {
                    "hex" | "binary" | "decimal" => {}
                    _ => {
                        return Err(Error::new(
                            literal.span(),
                            "display_format must be 'hex', 'binary', or 'decimal'",
                        ))
                    }
                }
                set_attribute(&mut attrs.display_format, value, "display_format", key_span)
            } else if meta.path.is_ident("display_prefix") {
                let literal = parse_string_value(&meta, "display_prefix")?;
                set_attribute(
                    &mut attrs.display_prefix,
                    literal.value(),
                    "display_prefix",
                    key_span,
                )
            } else if meta.path.is_ident("model_constraints") {
                Err(meta
                    .error("`model_constraints` was removed; use profile capability validation"))
            } else {
                Err(unknown_attribute_error(&meta))
            }
        })?;
    }

    match (&attrs.min, &attrs.max) {
        (Some(min), None) => {
            return Err(Error::new(
                min.key_span,
                "`min` requires `max`; specify both bounds or use `valid_values`",
            ))
        }
        (None, Some(max)) => {
            return Err(Error::new(
                max.key_span,
                "`max` requires `min`; specify both bounds or use `valid_values`",
            ))
        }
        _ => {}
    }

    if let Some(valid_values) = &attrs.valid_values {
        if attrs.min.is_some() || attrs.max.is_some() {
            return Err(Error::new(
                valid_values.key_span,
                "`valid_values` cannot be combined with `min` or `max`",
            ));
        }
    }

    Ok(attrs)
}

/// Derive macro for generating value types with validation
///
/// This macro generates:
/// - A `new()` constructor with validation
/// - `MIN` and `MAX` constants when bounds are specified
/// - `TryFrom` and `From` trait implementations
/// - `Display` implementation with configurable formatting
///
/// # Attributes
///
/// - `min` and `max` - Paired minimum and maximum allowed values, written as
///   unsuffixed integer literal strings
/// - `valid_values` - List of valid values (alternative to min/max)
/// - `display_format` - Display format: "hex", "binary", or "decimal" (default)
/// - `display_prefix` - Optional prefix for display output
///
/// # Example
///
/// ```rust,ignore
/// #[derive(Debug, Clone, Copy, PartialEq, Eq, ViscaValue)]
/// #[visca_value(min = "0", max = "255")]
/// struct Brightness(u8);
///
/// #[derive(Debug, Clone, Copy, PartialEq, Eq, ViscaValue)]
/// #[visca_value(valid_values = "[0x02, 0x03]", display_format = "hex")]
/// struct PowerState(u8);
///
/// #[derive(Debug, Clone, Copy, PartialEq, Eq, ViscaValue)]
/// #[visca_value(valid_values = "[0x00, 0x01, 0x02, 0x03]")]
/// struct Gain(u8);
/// ```
pub fn derive_visca_value(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);

    // Extract the struct name and inner type
    let name = &input.ident;
    let (inner_type, inner_field_index) = match &input.data {
        syn::Data::Struct(data) => match &data.fields {
            syn::Fields::Unnamed(fields) if fields.unnamed.len() == 1 => {
                let field = &fields.unnamed[0];
                (&field.ty, syn::Index::from(0))
            }
            _ => {
                return syn::Error::new_spanned(
                    &input,
                    "ViscaValue can only be derived for tuple structs with a single field",
                )
                .to_compile_error()
                .into();
            }
        },
        _ => {
            return syn::Error::new_spanned(&input, "ViscaValue can only be derived for structs")
                .to_compile_error()
                .into();
        }
    };

    let attributes = match parse_visca_value_attributes(&input.attrs) {
        Ok(attributes) => attributes,
        Err(error) => return error.to_compile_error().into(),
    };
    let crate_path = crate::crate_path::grafton_visca();
    let display_format = attributes
        .display_format
        .as_ref()
        .map_or("decimal", |format| format.value.as_str());
    let display_prefix = attributes
        .display_prefix
        .as_ref()
        .map_or("", |prefix| prefix.value.as_str());

    // Generate validation code
    let validation = if let Some(valid_list) = &attributes.valid_values {
        // Parse the valid values list
        let values_tokens: proc_macro2::TokenStream =
            valid_list.value.parse().unwrap_or_else(|_| quote! { &[] });
        quote! {
            const VALID_VALUES: &[#inner_type] = &#values_tokens;
            if !VALID_VALUES.contains(&value) {
                return ::core::result::Result::Err(#crate_path::Error::InvalidParameter {
                    parameter: ::core::stringify!(#name),
                    value: ::std::borrow::Cow::Owned(::std::format!("{value}")),
                    reason: ::std::borrow::Cow::Owned(::std::format!("must be one of {:?}", VALID_VALUES)),
                });
            }
        }
    } else if let (Some(min), Some(max)) = (&attributes.min, &attributes.max) {
        let min_tokens = &min.value;
        let max_tokens = &max.value;
        quote! {
            if !(#min_tokens..=#max_tokens).contains(&value) {
                return ::core::result::Result::Err(#crate_path::Error::ParameterOutOfRange {
                    parameter: ::core::stringify!(#name),
                    value: value as i32,
                    min: #min_tokens as i32,
                    max: #max_tokens as i32,
                });
            }
        }
    } else {
        quote! {}
    };

    // Generate min/max constants if provided
    let constants = if let (Some(min), Some(max)) = (&attributes.min, &attributes.max) {
        let min_tokens = &min.value;
        let max_tokens = &max.value;
        quote! {
            /// Minimum value.
            pub const MIN: Self = Self(#min_tokens);

            /// Maximum value.
            pub const MAX: Self = Self(#max_tokens);
        }
    } else if let Some(valid_list) = &attributes.valid_values {
        // When we have valid_values, compute MIN and MAX from the list
        let values_tokens: proc_macro2::TokenStream =
            valid_list.value.parse().unwrap_or_else(|_| quote! { &[] });
        quote! {
            /// Minimum value (computed from valid values).
            pub const MIN: Self = {
                let arr = #values_tokens;
                let mut min = arr[0];
                let mut i = 1;
                while i < arr.len() {
                    if arr[i] < min {
                        min = arr[i];
                    }
                    i += 1;
                }
                Self(min)
            };

            /// Maximum value (computed from valid values).
            pub const MAX: Self = {
                let arr = #values_tokens;
                let mut max = arr[0];
                let mut i = 1;
                while i < arr.len() {
                    if arr[i] > max {
                        max = arr[i];
                    }
                    i += 1;
                }
                Self(max)
            };
        }
    } else {
        quote! {}
    };

    // Generate display format
    let display_impl = match display_format {
        "hex" => {
            if display_prefix.is_empty() {
                quote! {
                    ::core::write!(f, "{:#02x}", self.#inner_field_index)
                }
            } else {
                quote! {
                    ::core::write!(f, "{} {:#02x}", #display_prefix, self.#inner_field_index)
                }
            }
        }
        "binary" => {
            if display_prefix.is_empty() {
                quote! {
                    ::core::write!(f, "{:#b}", self.#inner_field_index)
                }
            } else {
                quote! {
                    ::core::write!(f, "{} {:#b}", #display_prefix, self.#inner_field_index)
                }
            }
        }
        _ => {
            if display_prefix.is_empty() {
                quote! {
                    ::core::write!(f, "{}", self.#inner_field_index)
                }
            } else {
                quote! {
                    ::core::write!(f, "{} {}", #display_prefix, self.#inner_field_index)
                }
            }
        }
    };

    let expanded = quote! {
        impl #name {
            #constants

            /// Create a new value with validation.
            ///
            /// # Errors
            /// Returns an error if the value is out of range or invalid.
            pub fn new(value: #inner_type) -> ::core::result::Result<Self, #crate_path::Error> {
                #validation
                ::core::result::Result::Ok(Self(value))
            }

            /// Get the raw value.
            #[must_use]
            pub const fn value(self) -> #inner_type {
                self.#inner_field_index
            }
        }

        impl ::core::convert::TryFrom<#inner_type> for #name {
            type Error = #crate_path::Error;

            fn try_from(value: #inner_type) -> ::core::result::Result<Self, Self::Error> {
                Self::new(value)
            }
        }

        impl ::core::convert::From<#name> for #inner_type {
            fn from(val: #name) -> Self {
                val.value()
            }
        }

        impl ::core::fmt::Display for #name {
            fn fmt(
                &self,
                f: &mut ::core::fmt::Formatter<'_>,
            ) -> ::core::fmt::Result {
                #display_impl
            }
        }
    };

    TokenStream::from(expanded)
}
