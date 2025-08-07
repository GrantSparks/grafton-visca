// SPDX-License-Identifier: Apache-2.0
//
// Copyright (c) 2024 Grafton Machine Shed <team@grafton.ai>

//! Derive macro implementation for ViscaEnum
//!
//! This module implements the ViscaEnum derive macro that automatically generates
//! `TryFrom<u8>` and `From<Enum>` for u8 implementations for enums with explicit discriminants.

use proc_macro2::TokenStream;
use quote::quote;
use syn::{
    parse::{Parse, ParseStream},
    punctuated::Punctuated,
    Attribute, Data, DeriveInput, Error, Expr, Fields, Ident, Lit, LitStr, Token, Variant,
};

/// Attributes that can be applied to the enum itself
#[derive(Default)]
struct EnumAttributes {
    /// Custom error type to use instead of crate::error::Error
    error_type: Option<syn::Path>,
    /// Whether to generate exhaustive match (default: true)
    exhaustive: Option<bool>,
}

/// Attributes that can be applied to individual variants
#[derive(Default, Clone)]
struct VariantAttributes {
    /// Custom name to use in error messages
    name: Option<String>,
    /// Skip this variant in conversion implementations
    skip: bool,
}

/// Parser for #[visca_enum(...)] attributes on the enum
impl Parse for EnumAttributes {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let mut attrs = EnumAttributes::default();

        let punctuated = Punctuated::<MetaNameValue, Token![,]>::parse_terminated(input)?;

        for meta in punctuated {
            let name_str = meta.name.to_string();
            match name_str.as_str() {
                "error_type" => {
                    if let syn::Expr::Path(expr_path) = &meta.value {
                        attrs.error_type = Some(expr_path.path.clone());
                    } else {
                        return Err(Error::new_spanned(&meta.value, "error_type must be a path"));
                    }
                }
                "exhaustive" => {
                    if let syn::Expr::Lit(expr_lit) = &meta.value {
                        if let syn::Lit::Bool(lit_bool) = &expr_lit.lit {
                            attrs.exhaustive = Some(lit_bool.value);
                        } else {
                            return Err(Error::new_spanned(
                                &meta.value,
                                "exhaustive must be a boolean",
                            ));
                        }
                    } else {
                        return Err(Error::new_spanned(
                            &meta.value,
                            "exhaustive must be a boolean literal",
                        ));
                    }
                }
                _ => {
                    return Err(Error::new_spanned(
                        meta.name,
                        format!("Unknown attribute: {name_str}"),
                    ));
                }
            }
        }

        Ok(attrs)
    }
}

/// Helper struct for parsing name = value attributes
struct MetaNameValue {
    name: Ident,
    _eq: Token![=],
    value: syn::Expr,
}

impl Parse for MetaNameValue {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        Ok(MetaNameValue {
            name: input.parse()?,
            _eq: input.parse()?,
            value: input.parse()?,
        })
    }
}

/// Implementation of the ViscaEnum derive macro
pub fn derive_visca_enum_impl(input: DeriveInput) -> TokenStream {
    match generate_visca_enum(&input) {
        Ok(tokens) => tokens,
        Err(e) => e.to_compile_error(),
    }
}

fn generate_visca_enum(input: &DeriveInput) -> Result<TokenStream, Error> {
    // Only support enums
    let enum_data = match &input.data {
        Data::Enum(data) => data,
        _ => {
            return Err(Error::new_spanned(
                input,
                "ViscaEnum can only be derived for enums",
            ))
        }
    };

    // Parse enum-level attributes
    let enum_attrs = parse_enum_attributes(&input.attrs)?;

    // Extract enum name and generics
    let enum_name = &input.ident;
    let (impl_generics, ty_generics, where_clause) = input.generics.split_for_impl();

    // Check that all variants have explicit discriminants and parse variant attributes
    let variants_with_data = extract_variants_with_attributes(enum_data)?;

    // Filter out skipped variants
    let active_variants: Vec<_> = variants_with_data
        .iter()
        .filter(|(_, _, attrs)| !attrs.skip)
        .cloned()
        .collect();

    // Check for discriminant collisions
    check_discriminant_collisions_with_attrs(&active_variants)?;

    // Generate TryFrom<u8> implementation
    let try_from_impl = generate_try_from_impl_with_attrs(
        enum_name,
        &impl_generics,
        &ty_generics,
        &where_clause,
        &active_variants,
        &enum_attrs,
    );

    // Generate From<Enum> for u8 implementation (needs ALL variants, not just active ones)
    let from_impl = generate_from_impl_with_attrs(
        enum_name,
        &impl_generics,
        &ty_generics,
        &where_clause,
        &variants_with_data, // Pass ALL variants for exhaustive matching
    );

    // Generate is_valid_discriminant function
    let is_valid_impl = generate_is_valid_discriminant(
        enum_name,
        &impl_generics,
        &ty_generics,
        &where_clause,
        &active_variants,
    );

    Ok(quote! {
        #try_from_impl
        #from_impl
        #is_valid_impl
    })
}

/// Parse enum-level attributes
fn parse_enum_attributes(attrs: &[Attribute]) -> Result<EnumAttributes, Error> {
    let mut enum_attrs = EnumAttributes::default();

    for attr in attrs {
        if attr.path().is_ident("visca_enum") {
            let parsed: EnumAttributes = attr.parse_args()?;
            // Merge attributes (last one wins for each field)
            if parsed.error_type.is_some() {
                enum_attrs.error_type = parsed.error_type;
            }
            if parsed.exhaustive.is_some() {
                enum_attrs.exhaustive = parsed.exhaustive;
            }
        }
    }

    Ok(enum_attrs)
}

/// Parse variant-level attributes
fn parse_variant_attributes(attrs: &[Attribute]) -> Result<VariantAttributes, Error> {
    let mut variant_attrs = VariantAttributes::default();

    for attr in attrs {
        if attr.path().is_ident("visca_enum") {
            attr.parse_nested_meta(|meta| {
                if meta.path.is_ident("name") {
                    let value = meta.value()?;
                    let lit: LitStr = value.parse()?;
                    variant_attrs.name = Some(lit.value());
                    Ok(())
                } else if meta.path.is_ident("skip") {
                    variant_attrs.skip = true;
                    Ok(())
                } else {
                    Err(meta.error("Unknown attribute"))
                }
            })?;
        }
    }

    Ok(variant_attrs)
}

/// Extract variants with their discriminant values and attributes
fn extract_variants_with_attributes(
    enum_data: &syn::DataEnum,
) -> Result<Vec<(Variant, u8, VariantAttributes)>, Error> {
    let mut variants_with_data = Vec::new();

    for variant in &enum_data.variants {
        // Ensure variant has no fields
        match &variant.fields {
            Fields::Unit => {}
            _ => {
                return Err(Error::new_spanned(
                    variant,
                    "ViscaEnum only supports unit variants (no fields)",
                ))
            }
        }

        // Parse variant attributes
        let variant_attrs = parse_variant_attributes(&variant.attrs)?;

        // Extract discriminant value
        let discriminant = variant
            .discriminant
            .as_ref()
            .ok_or_else(|| {
                Error::new_spanned(variant, "All variants must have explicit discriminants")
            })?
            .1
            .clone();

        // Parse discriminant as u8
        let value = parse_discriminant_as_u8(&discriminant)?;
        variants_with_data.push((variant.clone(), value, variant_attrs));
    }

    Ok(variants_with_data)
}

/// Parse a discriminant expression as u8
fn parse_discriminant_as_u8(expr: &Expr) -> Result<u8, Error> {
    match expr {
        Expr::Lit(expr_lit) => match &expr_lit.lit {
            Lit::Int(lit_int) => lit_int
                .base10_parse::<u8>()
                .map_err(|_| Error::new_spanned(lit_int, "Discriminant must be a valid u8 value")),
            Lit::Byte(lit_byte) => Ok(lit_byte.value()),
            _ => Err(Error::new_spanned(
                expr,
                "Discriminant must be an integer literal",
            )),
        },
        _ => Err(Error::new_spanned(
            expr,
            "Discriminant must be a literal value",
        )),
    }
}

/// Check for discriminant collisions with attributes
fn check_discriminant_collisions_with_attrs(
    variants: &[(Variant, u8, VariantAttributes)],
) -> Result<(), Error> {
    use std::collections::HashMap;

    let mut seen = HashMap::new();

    for (variant, value, _) in variants {
        if let Some(prev_variant) = seen.insert(value, variant) {
            return Err(Error::new_spanned(
                variant,
                format!(
                    "Discriminant value {:#X} is already used by variant {}",
                    value, prev_variant.ident
                ),
            ));
        }
    }

    Ok(())
}

/// Generate error message with attribute support
fn generate_error_message_with_attrs(variants: &[(Variant, u8, VariantAttributes)]) -> String {
    let parts: Vec<String> = variants
        .iter()
        .map(|(variant, value, attrs)| {
            let name = attrs
                .name
                .as_ref()
                .cloned()
                .unwrap_or_else(|| variant.ident.to_string());
            format!("{value:#04X} ({name})")
        })
        .collect();

    if parts.len() == 1 {
        parts[0].clone()
    } else if parts.len() == 2 {
        format!("{} or {}", parts[0], parts[1])
    } else {
        let (last, rest) = parts.split_last().unwrap();
        format!("{}, or {}", rest.join(", "), last)
    }
}

/// Generate TryFrom<u8> implementation with attributes
fn generate_try_from_impl_with_attrs(
    enum_name: &syn::Ident,
    impl_generics: &syn::ImplGenerics,
    ty_generics: &syn::TypeGenerics,
    where_clause: &Option<&syn::WhereClause>,
    variants: &[(Variant, u8, VariantAttributes)],
    enum_attrs: &EnumAttributes,
) -> TokenStream {
    // Generate match arms for conversion
    let match_arms = variants.iter().map(|(variant, value, _)| {
        let variant_name = &variant.ident;
        quote! {
            #value => Ok(#enum_name::#variant_name)
        }
    });

    // Determine error type - use a more flexible approach
    let error_type = enum_attrs
        .error_type
        .as_ref()
        .map(|t| quote! { #t })
        .unwrap_or_else(|| {
            // Try to use crate::error::Error, but allow for external usage
            quote! { crate::error::Error }
        });

    // Generate error message with custom names
    let error_message = generate_error_message_with_attrs(variants);

    // Generate error constructor
    let error_constructor = {
        // Check if the error type looks like it ends with "Error"
        let is_grafton_error = enum_attrs
            .error_type
            .as_ref()
            .map(|p| {
                p.segments
                    .last()
                    .map(|s| s.ident == "Error")
                    .unwrap_or(false)
            })
            .unwrap_or(true); // Default case also uses Error

        if is_grafton_error {
            // Use InvalidResponse constructor for grafton_visca::Error
            quote! {
                Err(#error_type::InvalidResponse {
                    expected: ::std::borrow::Cow::Borrowed(#error_message),
                    actual: vec![value],
                })
            }
        } else {
            // For other error types, try to use From trait
            quote! {
                return Err(Self::Error::from(format!(
                    "Invalid value {:#04X}, expected: {}",
                    value,
                    #error_message
                )))
            }
        }
    };

    quote! {
        impl #impl_generics ::std::convert::TryFrom<u8> for #enum_name #ty_generics #where_clause {
            type Error = #error_type;

            fn try_from(value: u8) -> ::std::result::Result<Self, Self::Error> {
                match value {
                    #(#match_arms,)*
                    _ => #error_constructor,
                }
            }
        }
    }
}

/// Generate From<Enum> for u8 implementation with attributes
fn generate_from_impl_with_attrs(
    enum_name: &syn::Ident,
    impl_generics: &syn::ImplGenerics,
    ty_generics: &syn::TypeGenerics,
    where_clause: &Option<&syn::WhereClause>,
    variants: &[(Variant, u8, VariantAttributes)],
) -> TokenStream {
    // Generate match arms for all variants (including skipped ones)
    // This ensures exhaustive matching in the From implementation
    let match_arms = variants.iter().map(|(variant, value, _)| {
        let variant_name = &variant.ident;
        quote! {
            #enum_name::#variant_name => #value
        }
    });

    quote! {
        impl #impl_generics ::std::convert::From<#enum_name #ty_generics> for u8 #where_clause {
            fn from(value: #enum_name #ty_generics) -> Self {
                match value {
                    #(#match_arms,)*
                }
            }
        }
    }
}

/// Generate const fn is_valid_discriminant
fn generate_is_valid_discriminant(
    enum_name: &syn::Ident,
    impl_generics: &syn::ImplGenerics,
    ty_generics: &syn::TypeGenerics,
    where_clause: &Option<&syn::WhereClause>,
    variants: &[(Variant, u8, VariantAttributes)],
) -> TokenStream {
    // Generate match arms for validation
    let match_arms = variants.iter().map(|(_, value, _)| {
        quote! {
            #value => true
        }
    });

    quote! {
        impl #impl_generics #enum_name #ty_generics #where_clause {
            /// Check if a u8 value is a valid discriminant for this enum.
            pub const fn is_valid_discriminant(value: u8) -> bool {
                match value {
                    #(#match_arms,)*
                    _ => false,
                }
            }
        }
    }
}
