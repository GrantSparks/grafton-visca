// SPDX-License-Identifier: Apache-2.0
//
// Copyright (c) 2024 Grafton Machine Shed <team@grafton.ai>

//! Position-related procedural macros for the grafton-visca library

use proc_macro::TokenStream;
use quote::quote;
use syn::{parse_macro_input, ItemFn};

/// Macro for commands with position parameters that generates validation and conversion methods.
///
/// This macro provides automatic generation of:
/// - Parameter validation based on specified ranges
/// - Degrees-based variants (e.g., `set_position_degrees`)
/// - Normalized (0.0-1.0) variants (e.g., `set_position_normalized`)
///
/// # Features
/// - Validates position values against camera-specific ranges
/// - Generates convenient methods for different unit types
/// - Maintains async/sync compatibility
/// - Provide clear error messages for out-of-range values
///
/// # Example
///
/// ```rust,ignore
/// #[visca_position_command(
///     pan_range = "profile.pan_range()",
///     tilt_range = "profile.tilt_range()"
/// )]
/// fn set_pan_tilt_position(&self, pan: i16, tilt: i16) -> Result<()> {
///     let cmd = PanTiltCommand::AbsolutePosition {
///         pan: PanPosition::new(pan)?,
///         tilt: TiltPosition::new(tilt)?,
///         pan_speed: PanSpeed::new(self.default_pan_speed)?,
///         tilt_speed: TiltSpeed::new(self.default_tilt_speed)?,
///     };
///     self.send(&cmd)
/// }
/// ```
pub fn visca_position_command(attr: TokenStream, item: TokenStream) -> TokenStream {
    let input_fn = parse_macro_input!(item as ItemFn);

    // Parse attributes for range specifications
    let attr_str = attr.to_string();
    let mut range_specs = std::collections::HashMap::new();

    // Parse key=value pairs from attributes
    for pair in attr_str.split(',') {
        let pair = pair.trim();
        if let Some((key, value)) = pair.split_once('=') {
            let key = key.trim();
            let value = value.trim().trim_matches('"');
            range_specs.insert(key.to_string(), value.to_string());
        }
    }

    let vis = &input_fn.vis;
    let fn_name = &input_fn.sig.ident;
    let generics = &input_fn.sig.generics;
    let inputs = &input_fn.sig.inputs;
    let output = &input_fn.sig.output;
    let attrs = &input_fn.attrs;
    let block = &input_fn.block;

    // Generate validation code for position parameters
    let mut validations = Vec::new();

    // Analyze function parameters to find position types
    for input in inputs.iter().skip(1) {
        // Skip &self
        if let syn::FnArg::Typed(pat_type) = input {
            if let syn::Pat::Ident(pat_ident) = &*pat_type.pat {
                let param_name = &pat_ident.ident;
                let param_name_str = param_name.to_string();

                // Check if this parameter has a range specification
                if let Some(range_expr) = range_specs.get(&format!("{}_range", param_name_str)) {
                    // Parse range expression (e.g., "-170..=170")
                    if let Some((min_str, max_str)) = range_expr.split_once("..=") {
                        let min_tokens: proc_macro2::TokenStream =
                            min_str.parse().unwrap_or_else(|_| quote! { i16::MIN });
                        let max_tokens: proc_macro2::TokenStream =
                            max_str.parse().unwrap_or_else(|_| quote! { i16::MAX });

                        validations.push(quote! {
                            if !(#min_tokens..=#max_tokens).contains(&#param_name) {
                                return Err(crate::Error::OutOfRange {
                                    parameter: stringify!(#param_name).to_string(),
                                    value: #param_name as i32,
                                    min: #min_tokens as i32,
                                    max: #max_tokens as i32,
                                });
                            }
                        });
                    }
                }
            }
        }
    }

    // Generate wrapper methods for different input types
    let degrees_method = generate_degrees_variant(&input_fn, &validations);
    let normalized_method = generate_normalized_variant(&input_fn, &validations);

    // For blocking, we need to change &self to &mut self
    let blocking_inputs = inputs.iter().map(|arg| match arg {
        syn::FnArg::Receiver(receiver) => {
            if receiver.mutability.is_none() {
                syn::parse_quote! { &mut self }
            } else {
                arg.clone()
            }
        }
        other => other.clone(),
    });

    // Check if the return type indicates a command construction (returns a command type)
    // or a direct execution (returns Result)
    let is_command_construction = match output {
        syn::ReturnType::Type(_, ty) => {
            // Check if it's not a Result type
            if let syn::Type::Path(type_path) = &**ty {
                !type_path
                    .path
                    .segments
                    .iter()
                    .any(|seg| seg.ident == "Result")
            } else {
                true
            }
        }
        _ => false,
    };

    let expanded = if is_command_construction {
        // This function returns a command, so we just keep the original with validations
        quote! {
            #(#attrs)*
            #vis fn #fn_name #generics(#inputs) #output {
                #(#validations)*
                #block
            }

            #degrees_method
            #normalized_method
        }
    } else {
        // This function executes a command, so generate async/sync versions
        quote! {
            #[cfg(feature = "async")]
            #(#attrs)*
            #vis async fn #fn_name #generics(#inputs) #output {
                #(#validations)*
                #block
            }

            #[cfg(not(feature = "async"))]
            #(#attrs)*
            #vis fn #fn_name #generics(#(#blocking_inputs),*) #output {
                #(#validations)*
                #block
            }

            #degrees_method
            #normalized_method
        }
    };

    TokenStream::from(expanded)
}

/// Generate a degrees-based variant of the position method
fn generate_degrees_variant(
    input_fn: &ItemFn,
    validations: &[proc_macro2::TokenStream],
) -> proc_macro2::TokenStream {
    let vis = &input_fn.vis;
    let fn_name = &input_fn.sig.ident;
    let fn_name_degrees = syn::Ident::new(
        &format!("{}_degrees", fn_name),
        proc_macro2::Span::call_site(),
    );

    // Extract position parameters and convert them
    let mut param_conversions = Vec::new();
    let mut param_names = Vec::new();
    let mut param_types = Vec::new();

    for input in input_fn.sig.inputs.iter().skip(1) {
        // Skip &self
        if let syn::FnArg::Typed(pat_type) = input {
            if let syn::Pat::Ident(pat_ident) = &*pat_type.pat {
                let param_name = &pat_ident.ident;
                let param_name_str = param_name.to_string();

                if param_name_str.contains("pan") || param_name_str.contains("tilt") {
                    param_names.push(param_name.clone());
                    param_types.push(quote! { f32 });

                    if param_name_str.contains("pan") {
                        param_conversions.push(quote! {
                            let #param_name = self.profile.pan_degrees_to_units(#param_name);
                        });
                    } else if param_name_str.contains("tilt") {
                        param_conversions.push(quote! {
                            let #param_name = self.profile.tilt_degrees_to_units(#param_name);
                        });
                    }
                }
            }
        }
    }

    if param_names.is_empty() {
        return quote! {};
    }

    // Build parameter list
    let params: Vec<_> = param_names
        .iter()
        .zip(param_types.iter())
        .map(|(name, ty)| {
            quote! { #name: #ty }
        })
        .collect();

    // Check if the original function returns a Result (execution) or command (construction)
    let is_execution = matches!(&input_fn.sig.output, syn::ReturnType::Type(_, ty) 
        if matches!(&**ty, syn::Type::Path(type_path) 
            if type_path.path.segments.iter().any(|seg| seg.ident == "Result")));

    if is_execution {
        quote! {
            #[cfg(feature = "async")]
            /// Set position using degrees.
            #vis async fn #fn_name_degrees(&self, #(#params),*) -> Result<(), crate::Error> {
                #(#param_conversions)*
                #(#validations)*
                self.#fn_name(#(#param_names),*).await
            }

            #[cfg(not(feature = "async"))]
            /// Set position using degrees.
            #vis fn #fn_name_degrees(&mut self, #(#params),*) -> Result<(), crate::Error> {
                #(#param_conversions)*
                #(#validations)*
                self.#fn_name(#(#param_names),*)
            }
        }
    } else {
        quote! {
            /// Set position using degrees.
            #vis fn #fn_name_degrees(&self, #(#params),*) -> Result<(), crate::Error> {
                #(#param_conversions)*
                #(#validations)*
                self.#fn_name(#(#param_names),*)
            }
        }
    }
}

/// Generate a normalized (0.0-1.0) variant of the position method
fn generate_normalized_variant(
    input_fn: &ItemFn,
    validations: &[proc_macro2::TokenStream],
) -> proc_macro2::TokenStream {
    let vis = &input_fn.vis;
    let fn_name = &input_fn.sig.ident;
    let fn_name_normalized = syn::Ident::new(
        &format!("{}_normalized", fn_name),
        proc_macro2::Span::call_site(),
    );

    // Extract position parameters and convert them
    let mut param_conversions = Vec::new();
    let mut param_names = Vec::new();
    let mut param_types = Vec::new();

    for input in input_fn.sig.inputs.iter().skip(1) {
        // Skip &self
        if let syn::FnArg::Typed(pat_type) = input {
            if let syn::Pat::Ident(pat_ident) = &*pat_type.pat {
                let param_name = &pat_ident.ident;
                let param_name_str = param_name.to_string();

                if param_name_str.contains("pan")
                    || param_name_str.contains("tilt")
                    || param_name_str.contains("zoom")
                    || param_name_str.contains("focus")
                {
                    param_names.push(param_name.clone());
                    param_types.push(quote! { f32 });

                    if param_name_str.contains("pan") {
                        param_conversions.push(quote! {
                            let #param_name = self.profile.normalized_to_pan_units(#param_name);
                        });
                    } else if param_name_str.contains("tilt") {
                        param_conversions.push(quote! {
                            let #param_name = self.profile.normalized_to_tilt_units(#param_name);
                        });
                    } else if param_name_str.contains("zoom") {
                        param_conversions.push(quote! {
                            let #param_name = self.profile.normalized_to_zoom_units(#param_name);
                        });
                    } else if param_name_str.contains("focus") {
                        param_conversions.push(quote! {
                            let #param_name = self.profile.normalized_to_focus_units(#param_name);
                        });
                    }
                }
            }
        }
    }

    if param_names.is_empty() {
        return quote! {};
    }

    // Build parameter list
    let params: Vec<_> = param_names
        .iter()
        .zip(param_types.iter())
        .map(|(name, ty)| {
            quote! { #name: #ty }
        })
        .collect();

    // Check if the original function returns a Result (execution) or command (construction)
    let is_execution = matches!(&input_fn.sig.output, syn::ReturnType::Type(_, ty) 
        if matches!(&**ty, syn::Type::Path(type_path) 
            if type_path.path.segments.iter().any(|seg| seg.ident == "Result")));

    if is_execution {
        quote! {
            #[cfg(feature = "async")]
            /// Set position using normalized values (0.0 to 1.0).
            #vis async fn #fn_name_normalized(&self, #(#params),*) -> Result<(), crate::Error> {
                #(#param_conversions)*
                #(#validations)*
                self.#fn_name(#(#param_names),*).await
            }

            #[cfg(not(feature = "async"))]
            /// Set position using normalized values (0.0 to 1.0).
            #vis fn #fn_name_normalized(&mut self, #(#params),*) -> Result<(), crate::Error> {
                #(#param_conversions)*
                #(#validations)*
                self.#fn_name(#(#param_names),*)
            }
        }
    } else {
        quote! {
            /// Set position using normalized values (0.0 to 1.0).
            #vis fn #fn_name_normalized(&self, #(#params),*) -> Result<(), crate::Error> {
                #(#param_conversions)*
                #(#validations)*
                self.#fn_name(#(#param_names),*)
            }
        }
    }
}