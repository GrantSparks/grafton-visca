// SPDX-License-Identifier: MIT OR Apache-2.0
//
// Copyright (c) 2024 Grafton Machine Shed <team@grafton.ai>

//! Derive macro implementation for ViscaEnum
//!
//! This module implements the ViscaEnum derive macro that automatically generates
//! `TryFrom<u8>` and `From<Enum>` for u8 implementations for enums with explicit discriminants.

use proc_macro2::TokenStream;
use quote::quote;
use syn::{Attribute, Data, DeriveInput, Error, Expr, Fields, Lit, LitStr, Variant};

use crate::attr::{invalid_u8_literal, parse_u8_literal, unknown_key, AttrSlot};

const ATTRIBUTE: &str = "visca_enum";

/// Attributes that can be applied to the enum itself
#[derive(Default)]
struct EnumAttributes {
    /// Custom error type to use instead of crate::error::Error
    error_type: Option<syn::Path>,
}

/// Attributes that can be applied to individual variants
#[derive(Default, Clone)]
struct VariantAttributes {
    /// Custom name to use in error messages
    name: Option<String>,
    /// Skip this variant in conversion implementations
    skip: bool,
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
    let crate_path = crate::crate_path::grafton_visca();

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

    if active_variants.is_empty() {
        return Err(Error::new_spanned(
            &input.ident,
            "ViscaEnum requires at least one non-skipped variant",
        ));
    }

    // Every variant participates in `From<Enum> for u8`, including skipped
    // variants, so uniqueness is an enum-wide invariant.
    check_discriminant_collisions_with_attrs(&variants_with_data)?;

    // Generate TryFrom<u8> implementation
    let try_from_impl = generate_try_from_impl_with_attrs(
        enum_name,
        &impl_generics,
        &ty_generics,
        &where_clause,
        &active_variants,
        &enum_attrs,
        &crate_path,
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
    let mut error_type = AttrSlot::<syn::Path>::default();

    for attr in attrs.iter().filter(|attr| attr.path().is_ident(ATTRIBUTE)) {
        attr.parse_nested_meta(|meta| {
            if meta.path.is_ident("error_type") {
                let path = meta.value()?.parse::<syn::Path>()?;
                error_type.set(ATTRIBUTE, &meta, path)
            } else {
                Err(unknown_key(&meta, ATTRIBUTE, "`error_type`"))
            }
        })?;
    }

    Ok(EnumAttributes {
        error_type: error_type.into_value(),
    })
}

/// Parse variant-level attributes
fn parse_variant_attributes(attrs: &[Attribute]) -> Result<VariantAttributes, Error> {
    let mut name = AttrSlot::<String>::default();
    let mut skip = AttrSlot::<()>::default();

    for attr in attrs.iter().filter(|attr| attr.path().is_ident(ATTRIBUTE)) {
        attr.parse_nested_meta(|meta| {
            if meta.path.is_ident("name") {
                let value = meta.value()?.parse::<LitStr>()?;
                name.set(ATTRIBUTE, &meta, value.value())
            } else if meta.path.is_ident("skip") {
                skip.set(ATTRIBUTE, &meta, ())
            } else {
                Err(unknown_key(&meta, ATTRIBUTE, "`name` or `skip`"))
            }
        })?;
    }

    Ok(VariantAttributes {
        name: name.into_value(),
        skip: skip.get().is_some(),
    })
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
        Expr::Lit(syn::ExprLit {
            lit: Lit::Int(literal),
            ..
        }) => parse_u8_literal(literal, "discriminant"),
        _ => Err(invalid_u8_literal(expr, "discriminant")),
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
                    "Discriminant value {value:#X} is already used by variant {prev_variant_ident}",
                    prev_variant_ident = prev_variant.ident
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
        format!("{first} or {second}", first = parts[0], second = parts[1])
    } else {
        let (last, rest) = parts.split_last().unwrap();
        format!("{rest_joined}, or {last}", rest_joined = rest.join(", "))
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
    crate_path: &TokenStream,
) -> TokenStream {
    // Generate match arms for conversion
    let match_arms = variants.iter().map(|(variant, value, _)| {
        let variant_name = &variant.ident;
        quote! {
            #value => ::core::result::Result::Ok(#enum_name::#variant_name)
        }
    });

    // Determine error type - use a more flexible approach
    let error_type = enum_attrs
        .error_type
        .as_ref()
        .map(|t| quote! { #t })
        .unwrap_or_else(|| {
            quote! { #crate_path::Error }
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
                ::core::result::Result::Err(#error_type::invalid_response(::std::borrow::Cow::Borrowed(#error_message), ::std::vec![value]))
            }
        } else {
            // For other error types, try to use From trait
            quote! {
                return ::core::result::Result::Err(Self::Error::from(::std::format!(
                    "Invalid value {value:#04X}, expected: {error_message}",
                    error_message = #error_message
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
