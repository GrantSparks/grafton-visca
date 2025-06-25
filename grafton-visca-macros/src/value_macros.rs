// SPDX-License-Identifier: Apache-2.0
//
// Copyright (c) 2024 Grafton Machine Shed <team@grafton.ai>

//! Value type macros for the grafton-visca library
//!
//! This module contains the ViscaValue derive macro and related value type helpers
//! for generating validated value types with range checking and display formatting.

use proc_macro::TokenStream;
use quote::quote;
use syn::{parse_macro_input, DeriveInput};

/// Derive macro for generating value types with validation
///
/// This macro generates:
/// - A `new()` constructor with validation
/// - `MIN` and `MAX` constants when bounds are specified
/// - `TryFrom` and `From` trait implementations
/// - `Display` implementation with configurable formatting
/// - Model-specific validation when `model_constraints` is specified
///
/// # Attributes
///
/// - `min` - Minimum allowed value
/// - `max` - Maximum allowed value
/// - `valid_values` - List of valid values (alternative to min/max)
/// - `display_format` - Display format: "hex", "binary", or "decimal" (default)
/// - `display_prefix` - Optional prefix for display output
/// - `model_constraints` - Camera models that require validation (e.g., "PTZOpticsG2")
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
/// #[visca_value(
///     valid_values = "[0x00, 0x01, 0x02, 0x03, 0x04, 0x05, 0x06, 0x07]",
///     model_constraints = "PTZOpticsG2"
/// )]
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

    // Parse attributes
    let mut min_value = None;
    let mut max_value = None;
    let mut valid_values = None;
    let mut display_format = "decimal";
    let mut display_prefix = "";
    let mut model_constraints = None;

    for attr in &input.attrs {
        if attr.path().is_ident("visca_value") {
            let _ = attr.parse_nested_meta(|meta| {
                if meta.path.is_ident("min") {
                    let value: syn::LitStr = meta.value()?.parse()?;
                    min_value = Some(value.value());
                } else if meta.path.is_ident("max") {
                    let value: syn::LitStr = meta.value()?.parse()?;
                    max_value = Some(value.value());
                } else if meta.path.is_ident("valid_values") {
                    let value: syn::LitStr = meta.value()?.parse()?;
                    valid_values = Some(value.value());
                } else if meta.path.is_ident("display_format") {
                    let value: syn::LitStr = meta.value()?.parse()?;
                    display_format = match value.value().as_str() {
                        "hex" => "hex",
                        "binary" => "binary",
                        "decimal" => "decimal",
                        _ => {
                            return Err(
                                meta.error("display_format must be 'hex', 'binary', or 'decimal'")
                            )
                        }
                    };
                } else if meta.path.is_ident("display_prefix") {
                    let value: syn::LitStr = meta.value()?.parse()?;
                    display_prefix = Box::leak(value.value().into_boxed_str());
                } else if meta.path.is_ident("model_constraints") {
                    let value: syn::LitStr = meta.value()?.parse()?;
                    model_constraints = Some(value.value());
                }
                Ok(())
            });
        }
    }

    // Generate validation code
    let validation = if let Some(valid_list) = &valid_values {
        // Parse the valid values list
        let values_tokens: proc_macro2::TokenStream =
            valid_list.parse().unwrap_or_else(|_| quote! { &[] });
        quote! {
            const VALID_VALUES: &[#inner_type] = &#values_tokens;
            if !VALID_VALUES.contains(&value) {
                return Err(crate::Error::InvalidParameter(format!(
                    "{} must be one of {:?}, got {}",
                    stringify!(#name),
                    VALID_VALUES,
                    value
                )));
            }
        }
    } else if let (Some(min), Some(max)) = (&min_value, &max_value) {
        let min_tokens: proc_macro2::TokenStream = min.parse().unwrap_or_else(|_| quote! { 0 });
        let max_tokens: proc_macro2::TokenStream = max.parse().unwrap_or_else(|_| quote! { 255 });
        quote! {
            if !(#min_tokens..=#max_tokens).contains(&value) {
                return Err(crate::Error::ParameterOutOfRange {
                    parameter: stringify!(#name).to_string(),
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
    let constants = if let (Some(min), Some(max)) = (&min_value, &max_value) {
        let min_tokens: proc_macro2::TokenStream = min.parse().unwrap_or_else(|_| quote! { 0 });
        let max_tokens: proc_macro2::TokenStream = max.parse().unwrap_or_else(|_| quote! { 255 });
        quote! {
            /// Minimum value.
            pub const MIN: Self = Self(#min_tokens);

            /// Maximum value.
            pub const MAX: Self = Self(#max_tokens);
        }
    } else if let Some(valid_list) = &valid_values {
        // When we have valid_values, compute MIN and MAX from the list
        let values_tokens: proc_macro2::TokenStream =
            valid_list.parse().unwrap_or_else(|_| quote! { &[] });
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
                    write!(f, "{:#02x}", self.#inner_field_index)
                }
            } else {
                quote! {
                    write!(f, "{} {:#02x}", #display_prefix, self.#inner_field_index)
                }
            }
        }
        "binary" => {
            if display_prefix.is_empty() {
                quote! {
                    write!(f, "{:#b}", self.#inner_field_index)
                }
            } else {
                quote! {
                    write!(f, "{} {:#b}", #display_prefix, self.#inner_field_index)
                }
            }
        }
        _ => {
            if display_prefix.is_empty() {
                quote! {
                    write!(f, "{}", self.#inner_field_index)
                }
            } else {
                quote! {
                    write!(f, "{} {}", #display_prefix, self.#inner_field_index)
                }
            }
        }
    };

    // Generate model-specific validation method if constraints are provided
    let model_validation = if let Some(constraints) = &model_constraints {
        // Parse model constraints - format: "PTZOpticsG2" or "PTZOpticsG2|PTZOpticsG3"
        let models: Vec<&str> = constraints.split('|').collect();
        let model_checks = models
            .iter()
            .map(|model| {
                let model_ident = quote::format_ident!("{}", model);
                quote! {
                    crate::constants::CameraModel::#model_ident
                }
            })
            .collect::<Vec<_>>();

        // Generate the G2_VALID_VALUES constant for backwards compatibility
        let g2_constant = if models.contains(&"PTZOpticsG2") && valid_values.is_some() {
            let values_tokens: proc_macro2::TokenStream = valid_values
                .as_ref()
                .unwrap()
                .parse()
                .unwrap_or_else(|_| quote! { &[] });
            quote! {
                /// Valid values for PTZOptics G2 cameras.
                pub const G2_VALID_VALUES: &'static [#inner_type] = &#values_tokens;
            }
        } else {
            quote! {}
        };

        quote! {
            #g2_constant

            /// Validate the value for a specific camera model.
            ///
            /// # Errors
            /// Returns an error if the value is not valid for the given camera model.
            pub fn validate_for_model(&self, model: crate::constants::CameraModel) -> Result<(), crate::Error> {
                if matches!(model, #(#model_checks)|*) {
                    // Re-run validation for this model
                    Self::new(self.value())?;
                }
                Ok(())
            }
        }
    } else {
        quote! {}
    };

    let expanded = quote! {
        impl #name {
            #constants
            #model_validation

            /// Create a new value with validation.
            ///
            /// # Errors
            /// Returns an error if the value is out of range or invalid.
            pub fn new(value: #inner_type) -> Result<Self, crate::Error> {
                #validation
                Ok(Self(value))
            }

            /// Get the raw value.
            #[must_use]
            pub const fn value(self) -> #inner_type {
                self.#inner_field_index
            }
        }

        impl TryFrom<#inner_type> for #name {
            type Error = crate::Error;

            fn try_from(value: #inner_type) -> Result<Self, Self::Error> {
                Self::new(value)
            }
        }

        impl From<#name> for #inner_type {
            fn from(val: #name) -> Self {
                val.value()
            }
        }

        impl std::fmt::Display for #name {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                #display_impl
            }
        }
    };

    TokenStream::from(expanded)
}
