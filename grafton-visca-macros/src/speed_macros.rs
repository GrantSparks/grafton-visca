//! Speed-related macros for VISCA command generation.
//!
//! This module contains macros for generating speed-based command methods
//! with automatic validation and SpeedLevel enum conversion.

use proc_macro::TokenStream;
use quote::quote;
use syn::{parse_macro_input, ItemFn};

/// Macro for commands with speed parameters that generates validation and conversion methods.
///
/// This macro generates methods for speed-based commands with:
/// - Automatic validation of speed ranges
/// - SpeedLevel enum conversion
/// - Numeric speed variants
///
/// # Example
///
/// ```rust,ignore
/// #[visca_speed_command(
///     pan_speed_range = "0..=0x18",
///     tilt_speed_range = "0..=0x14"
/// )]
/// fn move_camera(&self, direction: PanTiltDirection, pan_speed: u8, tilt_speed: u8) -> Result<()> {
///     let cmd = PanTiltCommand::Move {
///         direction,
///         pan_speed: PanSpeed::new(pan_speed)?,
///         tilt_speed: TiltSpeed::new(tilt_speed)?,
///     };
///     self.send(&cmd)
/// }
/// ```
pub fn visca_speed_command(attr: TokenStream, item: TokenStream) -> TokenStream {
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

    // Generate validation code for speed parameters
    let mut validations = Vec::new();
    let mut speed_params = Vec::new();

    // Analyze function parameters to find speed types
    for input in inputs.iter().skip(1) {
        // Skip &self
        if let syn::FnArg::Typed(pat_type) = input {
            if let syn::Pat::Ident(pat_ident) = &*pat_type.pat {
                let param_name = &pat_ident.ident;
                let param_name_str = param_name.to_string();

                if param_name_str.contains("speed") {
                    speed_params.push(param_name.clone());

                    // Check if this parameter has a range specification
                    if let Some(range_expr) = range_specs.get(&format!("{param_name_str}_range")) {
                        // Parse range expression (e.g., "0..=7" or "0..=0x18")
                        if let Some((min_str, max_str)) = range_expr.split_once("..=") {
                            let min_tokens: proc_macro2::TokenStream =
                                min_str.parse().unwrap_or_else(|_| quote! { 0 });
                            let max_tokens: proc_macro2::TokenStream =
                                max_str.parse().unwrap_or_else(|_| quote! { 7 });

                            validations.push(quote! {
                                if #param_name > #max_tokens {
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
    }

    // Generate SpeedLevel enum variant
    let speed_level_method = generate_speed_level_variant(&input_fn, &speed_params, false);
    let speed_level_method_blocking = generate_speed_level_variant(&input_fn, &speed_params, true);

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

            #speed_level_method
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

            #speed_level_method
            #speed_level_method_blocking
        }
    };

    TokenStream::from(expanded)
}

/// Generate a variant that accepts SpeedLevel enum instead of numeric speeds
pub(crate) fn generate_speed_level_variant(
    input_fn: &ItemFn,
    speed_params: &[syn::Ident],
    is_blocking: bool,
) -> proc_macro2::TokenStream {
    if speed_params.is_empty() {
        return quote! {};
    }

    let vis = &input_fn.vis;
    let fn_name = &input_fn.sig.ident;
    let fn_name_with_level = syn::Ident::new(
        &format!("{fn_name}_with_level"),
        proc_macro2::Span::call_site(),
    );

    // Build new parameter list with SpeedLevel for speed params
    let mut new_params = Vec::new();
    let mut conversions = Vec::new();

    for input in input_fn.sig.inputs.iter() {
        match input {
            syn::FnArg::Receiver(receiver) => {
                // For blocking mode, convert &self to &mut self
                if is_blocking && receiver.mutability.is_none() {
                    new_params.push(quote! { &mut self });
                } else {
                    new_params.push(quote! { #receiver });
                }
            }
            syn::FnArg::Typed(pat_type) => {
                if let syn::Pat::Ident(pat_ident) = &*pat_type.pat {
                    let param_name = &pat_ident.ident;
                    let param_name_str = param_name.to_string();

                    if speed_params.contains(param_name) {
                        // Replace speed parameter with SpeedLevel
                        new_params.push(quote! { #param_name: SpeedLevel });

                        // Generate conversion based on parameter name
                        if param_name_str.contains("zoom") {
                            conversions.push(quote! {
                                let #param_name = #param_name.to_zoom_speed();
                            });
                        } else if param_name_str.contains("focus") {
                            conversions.push(quote! {
                                let #param_name = #param_name.to_focus_speed();
                            });
                        } else if param_name_str.contains("pan") {
                            conversions.push(quote! {
                                let #param_name = #param_name.to_pan_speed();
                            });
                        } else if param_name_str.contains("tilt") {
                            conversions.push(quote! {
                                let #param_name = #param_name.to_tilt_speed();
                            });
                        } else {
                            // Generic speed conversion
                            conversions.push(quote! {
                                let #param_name = #param_name.value();
                            });
                        }
                    } else {
                        // Keep other parameters as-is
                        new_params.push(quote! { #pat_type });
                    }
                }
            }
        }
    }

    // Extract non-speed parameters for the method call
    let call_params: Vec<_> = input_fn
        .sig
        .inputs
        .iter()
        .skip(1)
        .filter_map(|input| {
            if let syn::FnArg::Typed(pat_type) = input {
                if let syn::Pat::Ident(pat_ident) = &*pat_type.pat {
                    return Some(&pat_ident.ident);
                }
            }
            None
        })
        .collect();

    // Check if the original function returns a Result (execution) or command (construction)
    let is_execution = matches!(&input_fn.sig.output, syn::ReturnType::Type(_, ty)
        if matches!(&**ty, syn::Type::Path(type_path)
            if type_path.path.segments.iter().any(|seg| seg.ident == "Result")));

    if is_execution {
        if is_blocking {
            quote! {
                #[cfg(not(feature = "async"))]
                /// Execute command with speed level enum.
                #vis fn #fn_name_with_level(#(#new_params),*) -> Result<(), grafton_visca::Error> {
                    #(#conversions)*
                    self.#fn_name(#(#call_params),*)
                }
            }
        } else {
            quote! {
                #[cfg(feature = "async")]
                /// Execute command with speed level enum.
                #vis async fn #fn_name_with_level(#(#new_params),*) -> Result<(), grafton_visca::Error> {
                    #(#conversions)*
                    self.#fn_name(#(#call_params),*).await
                }
            }
        }
    } else {
        quote! {
            /// Execute command with speed level enum.
            #vis fn #fn_name_with_level(#(#new_params),*) -> Result<(), grafton_visca::Error> {
                #(#conversions)*
                let cmd = self.#fn_name(#(#call_params),*);
                self.send(&cmd)
            }
        }
    }
}
