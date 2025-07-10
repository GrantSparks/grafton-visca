// SPDX-License-Identifier: Apache-2.0
//
// Copyright (c) 2024 Grafton Machine Shed <team@grafton.ai>

//! Command-related procedural macros for the grafton-visca library

use proc_macro::TokenStream;
use quote::quote;
use syn::{parse_macro_input, ItemFn, ReturnType};

/// A procedural macro for defining VISCA command methods with automatic
/// async/sync generation.
///
/// This macro generates both async and sync versions of a method that sends
/// a VISCA command, handling the conditional compilation and error propagation.
///
/// # Features
/// - Automatically generates async version when `async` feature is enabled
/// - Automatically generates sync version when `async` feature is not enabled
/// - Handles both `Result<()>` and `Result<Response>` return types
/// - Forwards all method attributes and visibility
///
/// # Example
///
/// ```rust,ignore
/// #[visca_command]
/// pub fn power_on(&self) -> PowerCommand {
///     PowerCommand { power: Power::On }
/// }
/// ```
///
/// Expands to:
///
/// ```rust,ignore
/// #[cfg(feature = "async")]
/// pub async fn power_on(&self) -> Result<(), Error> {
///     let cmd = PowerCommand { power: Power::On };
///     self.send_and_wait(&cmd).await
/// }
///
/// #[cfg(not(feature = "async"))]
/// pub fn power_on(&mut self) -> Result<(), Error> {
///     let cmd = PowerCommand { power: Power::On };
///     self.send_and_wait(&cmd)
/// }
/// ```
pub fn visca_command(_attr: TokenStream, item: TokenStream) -> TokenStream {
    let input = parse_macro_input!(item as ItemFn);

    let vis = &input.vis;
    let sig = &input.sig;
    let fn_name = &sig.ident;
    let inputs = &sig.inputs;
    let output = &sig.output;
    let attrs = &input.attrs;
    let block = &input.block;

    // Check if this returns a Response or just Result<()>
    let returns_response = match output {
        ReturnType::Type(_, ty) => {
            let type_str = quote!(#ty).to_string();
            type_str.contains("Response")
        }
        _ => false,
    };

    let method = if returns_response {
        quote! { send_and_receive }
    } else {
        quote! { send }
    };

    let expanded = quote! {
        #[cfg(feature = "async")]
        #(#attrs)*
        #vis async fn #fn_name(#inputs) #output {
            let result = (|| #block)();
            match result {
                Ok(cmd) => self.#method(&cmd).await,
                Err(e) => Err(e),
            }
        }

        #[cfg(not(feature = "async"))]
        #(#attrs)*
        #vis fn #fn_name(#inputs) #output {
            let result = (|| #block)();
            match result {
                Ok(cmd) => self.#method(&cmd),
                Err(e) => Err(e),
            }
        }
    };

    TokenStream::from(expanded)
}

/// Macro for generating multiple method variants for different input types.
///
/// This allows creating ergonomic APIs that accept multiple input types without
/// the complexity of generic trait bounds.
///
/// # Example
///
/// ```rust,ignore
/// #[visca_command_variants(
///     set_zoom(position: u16) -> "set_zoom_raw",
///     set_zoom(position: ZoomPosition) -> "set_zoom",
///     set_zoom(percentage: Percentage<f32>) -> "set_zoom_percentage",
///     set_zoom(magnification: Magnification<f32>) -> "set_zoom_magnification"
/// )]
/// fn set_zoom_impl(position: ZoomPosition) -> ZoomCommand {
///     ZoomCommand::Direct(position)
/// }
/// ```
pub fn visca_command_variants(attr: TokenStream, item: TokenStream) -> TokenStream {
    let input_fn = parse_macro_input!(item as ItemFn);

    // Parse the attribute to extract variant definitions
    let attr_str = attr.to_string();
    let variants: Vec<_> = attr_str
        .split(',')
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
        .collect();

    let mut generated_methods = Vec::new();

    // Extract the implementation function's expected parameter type
    let impl_fn_name = &input_fn.sig.ident;
    let impl_param_type = if let Some(syn::FnArg::Typed(pat_type)) = input_fn.sig.inputs.first() {
        &pat_type.ty
    } else {
        return syn::Error::new_spanned(
            &input_fn.sig,
            "Implementation function must have at least one parameter",
        )
        .to_compile_error()
        .into();
    };

    for variant in variants {
        // Parse each variant definition
        // Format: method_name(param: Type) -> "generated_name"
        if let Some((signature, generated_name)) = variant.split_once("->") {
            let signature = signature.trim();
            let generated_name = generated_name.trim().trim_matches('"');

            // Parse the method signature
            if let Some((_method_base, params_str)) = signature.split_once('(') {
                let params_str = params_str.trim_end_matches(')');

                // Parse parameter name and type
                if let Some((param_name, param_type)) = params_str.split_once(':') {
                    let param_name = param_name.trim();
                    let param_type_str = param_type.trim();

                    // Parse the parameter type
                    let param_type: syn::Type = match syn::parse_str(param_type_str) {
                        Ok(ty) => ty,
                        Err(e) => {
                            return syn::Error::new(
                                proc_macro2::Span::call_site(),
                                format!("Failed to parse type '{param_type_str}': {e}"),
                            )
                            .to_compile_error()
                            .into();
                        }
                    };

                    let param_ident = syn::Ident::new(param_name, proc_macro2::Span::call_site());
                    let method_ident =
                        syn::Ident::new(generated_name, proc_macro2::Span::call_site());

                    // Generate conversion code based on the types
                    let conversion =
                        generate_conversion(&param_ident, &param_type, impl_param_type);

                    generated_methods.push(quote! {
                        #[visca_command]
                        pub fn #method_ident(&self, #param_ident: #param_type) -> Result<(), crate::Error> {
                            let converted = #conversion;
                            let command = #impl_fn_name(converted);
                            self.send(&command)
                        }
                    });
                }
            }
        }
    }

    let expanded = quote! {
        #input_fn

        #(#generated_methods)*
    };

    TokenStream::from(expanded)
}

/// Generate conversion code from input type to expected type
fn generate_conversion(
    param_name: &syn::Ident,
    from_type: &syn::Type,
    to_type: &syn::Type,
) -> proc_macro2::TokenStream {
    // Check if types are the same
    if quote!(#from_type).to_string() == quote!(#to_type).to_string() {
        return quote! { #param_name };
    }

    // Handle common conversions
    let from_str = quote!(#from_type).to_string();
    let to_str = quote!(#to_type).to_string();

    // Direct numeric to position type conversions
    if from_str == "u16" {
        if to_str.contains("ZoomPosition") {
            return quote! { crate::types::ZoomPosition::new(#param_name)? };
        } else if to_str.contains("FocusPosition") {
            return quote! { crate::types::FocusPosition::new(#param_name)? };
        } else if to_str.contains("ColorTemperature") {
            return quote! { crate::types::ColorTemperature::new(#param_name)? };
        }
    }

    if from_str == "u8" {
        if to_str.contains("Gain") && !to_str.contains("GainLimit") {
            return quote! { crate::types::Gain::new(#param_name)? };
        } else if to_str.contains("RedGain") {
            return quote! { crate::types::RedGain::new(#param_name)? };
        } else if to_str.contains("BlueGain") {
            return quote! { crate::types::BlueGain::new(#param_name)? };
        } else if to_str.contains("SaturationLevel") {
            return quote! { crate::types::SaturationLevel::new(#param_name)? };
        } else if to_str.contains("LuminanceLevel") {
            return quote! { crate::types::LuminanceLevel::new(#param_name)? };
        } else if to_str.contains("ContrastLevel") {
            return quote! { crate::types::ContrastLevel::new(#param_name)? };
        } else if to_str.contains("SharpnessLevel") {
            return quote! { crate::types::SharpnessLevel::new(#param_name)? };
        } else if to_str.contains("PanSpeed") {
            return quote! { crate::types::PanSpeed::new(#param_name)? };
        } else if to_str.contains("TiltSpeed") {
            return quote! { crate::types::TiltSpeed::new(#param_name)? };
        }
    }

    // Unit type conversions
    if from_str.contains("Degrees") {
        if to_str.contains("ZoomPosition") {
            return quote! { self.profile.degrees_to_zoom_position(#param_name)? };
        } else if to_str.contains("FocusPosition") {
            return quote! { self.profile.degrees_to_focus_position(#param_name)? };
        } else if to_str.contains("ViscaUnits") {
            return quote! { self.profile.degrees_to_units(#param_name) };
        }
    }

    if from_str.contains("Normalized") {
        if to_str.contains("ZoomPosition") {
            return quote! { crate::types::ZoomPosition::try_from(#param_name.0)? };
        } else if to_str.contains("FocusPosition") {
            return quote! { crate::types::FocusPosition::try_from(#param_name.0)? };
        } else if to_str.contains("ViscaUnits") {
            return quote! { self.profile.normalized_to_units(#param_name) };
        } else if to_str.contains("Position") {
            return quote! { self.profile.normalized_to_position(#param_name)? };
        }
    }

    if from_str.contains("Percentage") {
        if to_str.contains("ZoomPosition") {
            return quote! { crate::types::ZoomPosition::try_from(#param_name.0 / 100.0)? };
        } else if to_str.contains("FocusPosition") {
            return quote! { crate::types::FocusPosition::try_from(#param_name.0 / 100.0)? };
        } else if to_str.contains("Level") {
            return quote! { self.profile.percentage_to_level(#param_name)? };
        } else if to_str.contains("Position") {
            return quote! { self.profile.percentage_to_position(#param_name)? };
        }
    }

    if from_str.contains("Magnification") && to_str.contains("ZoomPosition") {
        return quote! { self.profile.magnification_to_zoom_position(#param_name)? };
    }

    if from_str.contains("Kelvin") && to_str.contains("ColorTemperature") {
        return quote! { crate::types::ColorTemperature::from_kelvin(#param_name.0)? };
    }

    // f32/f64 to normalized types
    if from_str == "f32" || from_str == "f64" {
        if to_str.contains("Normalized") {
            return quote! { crate::units::Normalized(#param_name) };
        } else if to_str.contains("Percentage") {
            return quote! { crate::units::Percentage(#param_name) };
        } else if to_str.contains("Degrees") {
            return quote! { crate::units::Degrees(#param_name) };
        } else if to_str.contains("Magnification") {
            return quote! { crate::units::Magnification(#param_name) };
        }
    }

    // SpeedLevel conversions
    if from_str.contains("SpeedLevel") {
        if to_str.contains("PanSpeed") {
            return quote! { #param_name.to_pan_speed() };
        } else if to_str.contains("TiltSpeed") {
            return quote! { #param_name.to_tilt_speed() };
        } else if to_str.contains("ZoomSpeed") {
            return quote! { #param_name.to_zoom_speed() };
        } else if to_str.contains("FocusSpeed") {
            return quote! { #param_name.to_focus_speed() };
        }
    }

    // Default fallback - try From/Into trait
    quote! { #param_name.try_into()? }
}
