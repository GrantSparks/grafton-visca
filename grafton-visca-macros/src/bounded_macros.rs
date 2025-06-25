//! Bounded parameter macros for VISCA command generation.
//!
//! This module contains macros for generating bounded command methods
//! with automatic validation and percentage/level conversion variants.

use proc_macro::TokenStream;
use quote::quote;
use syn::{parse_macro_input, ItemFn};

/// Macro for commands with bounded parameters that generates validation and conversion methods.
///
/// This macro generates methods for bounded commands with:
/// - Automatic validation of parameter ranges
/// - Percentage-based variants (0-100%)
/// - Named level variants (Low, Medium, High)
///
/// # Example
///
/// ```rust,ignore
/// #[visca_bounded_command(
///     luminance = "0..=0x0E",
///     contrast = "0..=0x0E"
/// )]
/// fn set_luminance_contrast(&self, luminance: u8, contrast: u8) -> Result<()> {
///     let cmd = LuminanceContrastCommand {
///         luminance: LuminanceLevel::new(luminance)?,
///         contrast: ContrastLevel::new(contrast)?,
///     };
///     self.send(&cmd)
/// }
/// ```
pub fn visca_bounded_command(attr: TokenStream, item: TokenStream) -> TokenStream {
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

    // Generate validation code for bounded parameters
    let mut validations = Vec::new();
    let mut bounded_params = Vec::new();

    // Analyze function parameters
    for input in inputs.iter().skip(1) {
        // Skip &self
        if let syn::FnArg::Typed(pat_type) = input {
            if let syn::Pat::Ident(pat_ident) = &*pat_type.pat {
                let param_name = &pat_ident.ident;
                let param_name_str = param_name.to_string();

                // Check if this parameter has a range specification
                if let Some(range_expr) = range_specs.get(&param_name_str) {
                    bounded_params.push((param_name.clone(), range_expr.clone()));

                    // Parse range expression (e.g., "0..=7" or "0..=0x0E")
                    if let Some((min_str, max_str)) = range_expr.split_once("..=") {
                        let min_tokens: proc_macro2::TokenStream =
                            min_str.parse().unwrap_or_else(|_| quote! { 0 });
                        let max_tokens: proc_macro2::TokenStream =
                            max_str.parse().unwrap_or_else(|_| quote! { 100 });

                        validations.push(quote! {
                            if !(#min_tokens..=#max_tokens).contains(&#param_name) {
                                return Err(crate::Error::ParameterOutOfRange {
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

    // Generate percentage-based variant if applicable
    let percentage_method = generate_percentage_variant(&input_fn, &bounded_params);

    // Generate named level variant if applicable
    let level_method = generate_level_variant(&input_fn, &bounded_params);

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

            #percentage_method
            #level_method
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

            #percentage_method
            #level_method
        }
    };

    TokenStream::from(expanded)
}

/// Generate a percentage-based variant of the bounded method
pub(crate) fn generate_percentage_variant(
    input_fn: &ItemFn,
    bounded_params: &[(syn::Ident, String)],
) -> proc_macro2::TokenStream {
    if bounded_params.is_empty() {
        return quote! {};
    }

    let vis = &input_fn.vis;
    let fn_name = &input_fn.sig.ident;
    let fn_name_percentage = syn::Ident::new(
        &format!("{}_percentage", fn_name),
        proc_macro2::Span::call_site(),
    );

    // Extract bounded parameters and convert them
    let mut param_conversions = Vec::new();
    let mut param_names = Vec::new();
    let mut param_types = Vec::new();

    for (param_name, range_expr) in bounded_params {
        param_names.push(param_name.clone());
        param_types.push(quote! { f32 });

        // Parse the max value from range expression
        if let Some((_, max_str)) = range_expr.split_once("..=") {
            let max_tokens: proc_macro2::TokenStream =
                max_str.parse().unwrap_or_else(|_| quote! { 100 });

            param_conversions.push(quote! {
                if !(0.0..=100.0).contains(&#param_name) {
                    return Err(crate::Error::ParameterOutOfRange {
                        parameter: format!("{}_percentage", stringify!(#param_name)),
                        value: #param_name as i32,
                        min: 0,
                        max: 100,
                    });
                }
                let #param_name = ((#param_name / 100.0) * (#max_tokens as f32)) as u8;
            });
        }
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
            /// Set values using percentages (0.0 to 100.0).
            #vis async fn #fn_name_percentage(&self, #(#params),*) -> Result<(), crate::Error> {
                #(#param_conversions)*
                self.#fn_name(#(#param_names),*).await
            }

            #[cfg(not(feature = "async"))]
            /// Set values using percentages (0.0 to 100.0).
            #vis fn #fn_name_percentage(&mut self, #(#params),*) -> Result<(), crate::Error> {
                #(#param_conversions)*
                self.#fn_name(#(#param_names),*)
            }
        }
    } else {
        quote! {
            /// Create command using percentages (0.0 to 100.0).
            #vis fn #fn_name_percentage(&self, #(#params),*) -> Result<(), crate::Error> {
                #(#param_conversions)*
                let cmd = self.#fn_name(#(#param_names),*);
                self.send(&cmd)
            }
        }
    }
}

/// Generate a named level variant of the bounded method
pub(crate) fn generate_level_variant(
    input_fn: &ItemFn,
    bounded_params: &[(syn::Ident, String)],
) -> proc_macro2::TokenStream {
    // Only generate level variant for single-parameter methods with specific names
    if bounded_params.len() != 1 {
        return quote! {};
    }

    let (param_name, range_expr) = &bounded_params[0];
    let param_name_str = param_name.to_string();

    // Only generate for parameters that make sense with levels
    if !["luminance", "contrast", "sharpness", "saturation", "hue"]
        .contains(&param_name_str.as_str())
    {
        return quote! {};
    }

    let vis = &input_fn.vis;
    let fn_name = &input_fn.sig.ident;
    let fn_name_level = syn::Ident::new(
        &format!("{}_level", fn_name),
        proc_macro2::Span::call_site(),
    );

    // Parse the max value from range expression
    let max_val = if let Some((_, max_str)) = range_expr.split_once("..=") {
        max_str
            .parse::<u8>()
            .or_else(|_| {
                // Try parsing as hex (0x0E -> 14)
                if max_str.starts_with("0x") || max_str.starts_with("0X") {
                    u8::from_str_radix(&max_str[2..], 16)
                } else {
                    Err("Invalid number format".parse::<u8>().unwrap_err())
                }
            })
            .unwrap_or(10)
    } else {
        10
    };

    // Calculate level thresholds
    let low_val = max_val / 3;
    let medium_val = (max_val * 2) / 3;

    // Capitalize the parameter name for the enum type
    let level_enum = syn::Ident::new(
        &format!("{}Level", capitalize_first(&param_name_str)),
        proc_macro2::Span::call_site(),
    );

    // Check if the original function returns a Result (execution) or command (construction)
    let is_execution = matches!(&input_fn.sig.output, syn::ReturnType::Type(_, ty) 
        if matches!(&**ty, syn::Type::Path(type_path) 
            if type_path.path.segments.iter().any(|seg| seg.ident == "Result")));

    if is_execution {
        quote! {
            #[cfg(feature = "async")]
            /// Set value using named levels (Low, Medium, High).
            #vis async fn #fn_name_level(&self, level: #level_enum) -> Result<(), crate::Error> {
                let value = match level {
                    #level_enum::Low => #low_val,
                    #level_enum::Medium => #medium_val,
                    #level_enum::High => #max_val,
                };
                self.#fn_name(value).await
            }

            #[cfg(not(feature = "async"))]
            /// Set value using named levels (Low, Medium, High).
            #vis fn #fn_name_level(&mut self, level: #level_enum) -> Result<(), crate::Error> {
                let value = match level {
                    #level_enum::Low => #low_val,
                    #level_enum::Medium => #medium_val,
                    #level_enum::High => #max_val,
                };
                self.#fn_name(value)
            }
        }
    } else {
        quote! {
            /// Create command using named levels (Low, Medium, High).
            #vis fn #fn_name_level(&self, level: #level_enum) -> Result<(), crate::Error> {
                let value = match level {
                    #level_enum::Low => #low_val,
                    #level_enum::Medium => #medium_val,
                    #level_enum::High => #max_val,
                };
                let cmd = self.#fn_name(value);
                self.send(&cmd)
            }
        }
    }
}

fn capitalize_first(s: &str) -> String {
    let mut chars = s.chars();
    match chars.next() {
        None => String::new(),
        Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
    }
}
