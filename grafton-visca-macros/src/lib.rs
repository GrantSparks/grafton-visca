// SPDX-License-Identifier: Apache-2.0
//
// Copyright (c) 2024 Grafton Machine Shed <team@grafton.ai>

//! Procedural macros for the grafton-visca library
//!
//! This crate provides derive and attribute macros to simplify common patterns
//! in VISCA command implementations.

use proc_macro::TokenStream;
use quote::{format_ident, quote};
use syn::{parse_macro_input, DeriveInput, ItemFn, ReturnType, Type};

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
/// #[visca_method]
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
#[proc_macro_attribute]
pub fn visca_method(_attr: TokenStream, item: TokenStream) -> TokenStream {
    let input_fn = parse_macro_input!(item as ItemFn);

    let vis = &input_fn.vis;
    let sig = &input_fn.sig;
    let fn_name = &sig.ident;
    let generics = &sig.generics;
    let inputs = &sig.inputs;
    let attrs = &input_fn.attrs;
    let block = &input_fn.block;

    // For blocking, we need to change &self to &mut self
    let blocking_inputs = inputs.iter().map(|arg| {
        match arg {
            syn::FnArg::Receiver(receiver) => {
                if receiver.mutability.is_none() {
                    // Parse a new receiver with mutability
                    syn::parse_quote! { &mut self }
                } else {
                    arg.clone()
                }
            }
            other => other.clone(),
        }
    });

    let expanded = quote! {
        #[cfg(feature = "async")]
        #(#attrs)*
        #vis async fn #fn_name #generics(#inputs) -> Result<(), crate::Error> {
            let cmd = #block;
            self.send_and_wait(&cmd).await
        }

        #[cfg(not(feature = "async"))]
        #(#attrs)*
        #vis fn #fn_name #generics(#(#blocking_inputs),*) -> Result<(), crate::Error> {
            let cmd = #block;
            self.send_and_wait(&cmd)
        }
    };

    TokenStream::from(expanded)
}

/// Alternative version for methods that use the `?` operator in their implementation.
///
/// This macro wraps the body in a closure that returns Result, allowing the use of `?`.
///
/// # Example
///
/// ```rust,ignore
/// #[visca_method_custom]
/// pub fn stop(&self) -> PanTiltCommand {
///     PanTiltCommand::Move {
///         direction: PanTiltDirection::Stop,
///         pan_speed: PanSpeed::new(0)?,
///         tilt_speed: TiltSpeed::new(0)?,
///     }
/// }
/// ```
#[proc_macro_attribute]
pub fn visca_method_custom(_attr: TokenStream, item: TokenStream) -> TokenStream {
    let input_fn = parse_macro_input!(item as ItemFn);

    let vis = &input_fn.vis;
    let sig = &input_fn.sig;
    let fn_name = &sig.ident;
    let generics = &sig.generics;
    let inputs = &sig.inputs;
    let attrs = &input_fn.attrs;
    let block = &input_fn.block;

    // For blocking, we need to change &self to &mut self
    let blocking_inputs = inputs.iter().map(|arg| {
        match arg {
            syn::FnArg::Receiver(receiver) => {
                if receiver.mutability.is_none() {
                    // Parse a new receiver with mutability
                    syn::parse_quote! { &mut self }
                } else {
                    arg.clone()
                }
            }
            other => other.clone(),
        }
    });

    let expanded = quote! {
        #[cfg(feature = "async")]
        #(#attrs)*
        #vis async fn #fn_name #generics(#inputs) -> Result<(), crate::Error> {
            let cmd = (|| -> Result<_, crate::Error> {
                Ok(#block)
            })()?;
            self.send_and_wait(&cmd).await
        }

        #[cfg(not(feature = "async"))]
        #(#attrs)*
        #vis fn #fn_name #generics(#(#blocking_inputs),*) -> Result<(), crate::Error> {
            let cmd = (|| -> Result<_, crate::Error> {
                Ok(#block)
            })()?;
            self.send_and_wait(&cmd)
        }
    };

    TokenStream::from(expanded)
}

/// A macro for camera methods that automatically includes profile parameter validation.
///
/// This is the same as visca_method_custom for now but reserved for future enhancements.
///
/// # Example
///
/// ```rust,ignore
/// #[visca_camera_method]
/// pub fn set_zoom(&self, position: ZoomPosition) {
///     ZoomCommand::Position(position)
/// }
/// ```
#[proc_macro_attribute]
pub fn visca_camera_method(_attr: TokenStream, item: TokenStream) -> TokenStream {
    // For now, just delegate to visca_method_custom
    visca_method_custom(_attr, item)
}

/// A procedural macro for defining VISCA command methods that need better ergonomics.
///
/// This macro is specifically for methods that need custom implementation patterns
/// while maintaining consistency with the library's async/sync dual support.
///
/// # Features
/// - Generates properly formatted async/sync versions
/// - Maintains consistent error handling
/// - Preserves method documentation
///
/// # Example
///
/// ```rust,ignore
/// #[visca_command]
/// pub fn zoom_to_position(&self, position: u16) -> Result<()> {
///     let zoom_pos = ZoomPosition::new(position)?;
///     self.send(&ZoomCommand::Position(zoom_pos))
/// }
/// ```
#[proc_macro_attribute]
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
#[proc_macro_attribute]
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
                                format!("Failed to parse type '{}': {}", param_type_str, e),
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
        if to_str.contains("GainValue") {
            return quote! { crate::types::GainValue::new(#param_name)? };
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

    if from_str.contains("Radians") {
        if to_str.contains("ZoomPosition") {
            return quote! { self.profile.radians_to_zoom_position(#param_name)? };
        } else if to_str.contains("FocusPosition") {
            return quote! { self.profile.radians_to_focus_position(#param_name)? };
        } else if to_str.contains("ViscaUnits") {
            return quote! { self.profile.radians_to_units(#param_name) };
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
        } else if to_str.contains("Radians") {
            return quote! { crate::units::Radians(#param_name) };
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

/// A macro for defining VISCA command methods with generic parameters.
///
/// This variant is specifically for methods with generic type parameters
/// and complex trait bounds.
///
/// # Example
///
/// ```rust,ignore
/// #[visca_method_generic]
/// pub fn set_normalized_position<T>(&self, value: T) -> Result<()>
/// where
///     T: Into<Normalized>,
/// {
///     let normalized = value.into();
///     let position = self.profile.normalized_to_position(normalized);
///     self.send(&PositionCommand(position))
/// }
/// ```
#[proc_macro_attribute]
pub fn visca_method_generic(_attr: TokenStream, item: TokenStream) -> TokenStream {
    let input_fn = parse_macro_input!(item as ItemFn);

    let vis = &input_fn.vis;
    let sig = &input_fn.sig;
    let fn_name = &sig.ident;
    let generics = &sig.generics;
    let where_clause = &generics.where_clause;
    let inputs = &sig.inputs;
    let output = &sig.output;
    let attrs = &input_fn.attrs;
    let block = &input_fn.block;

    let expanded = quote! {
        #[cfg(feature = "async")]
        #(#attrs)*
        #vis async fn #fn_name #generics(#inputs) #output #where_clause {
            #block
        }

        #[cfg(not(feature = "async"))]
        #(#attrs)*
        #vis fn #fn_name #generics(#inputs) #output #where_clause {
            #block
        }
    };

    TokenStream::from(expanded)
}

/// A macro for defining inquiry methods that return values from the camera.
///
/// # Example
///
/// ```rust,ignore
/// #[visca_inquiry]
/// fn get_zoom_position(&self) -> Result<ZoomPosition> {
///     InquiryCommand::ZoomPosition
/// }
/// ```
#[proc_macro_attribute]
pub fn visca_inquiry(_attr: TokenStream, item: TokenStream) -> TokenStream {
    let input_fn = parse_macro_input!(item as ItemFn);

    let vis = &input_fn.vis;
    let fn_name = &input_fn.sig.ident;
    let generics = &input_fn.sig.generics;
    let inputs = &input_fn.sig.inputs;
    let output = &input_fn.sig.output;
    let attrs = &input_fn.attrs;
    let block = &input_fn.block;

    // Extract the return type from Result<T>
    let return_type = match output {
        ReturnType::Type(_, ty) => {
            if let Type::Path(type_path) = &**ty {
                if let Some(segment) = type_path.path.segments.last() {
                    if segment.ident == "Result" {
                        // Extract T from Result<T>
                        if let syn::PathArguments::AngleBracketed(args) = &segment.arguments {
                            if let Some(syn::GenericArgument::Type(inner_type)) = args.args.first()
                            {
                                inner_type
                            } else {
                                return syn::Error::new_spanned(
                                    output,
                                    "Expected Result<T> return type",
                                )
                                .to_compile_error()
                                .into();
                            }
                        } else {
                            return syn::Error::new_spanned(
                                output,
                                "Expected Result<T> with type parameter",
                            )
                            .to_compile_error()
                            .into();
                        }
                    } else {
                        return syn::Error::new_spanned(
                            output,
                            "visca_inquiry methods must return Result<T>",
                        )
                        .to_compile_error()
                        .into();
                    }
                } else {
                    return syn::Error::new_spanned(output, "Invalid return type")
                        .to_compile_error()
                        .into();
                }
            } else {
                return syn::Error::new_spanned(output, "Expected Result<T> return type")
                    .to_compile_error()
                    .into();
            }
        }
        _ => {
            return syn::Error::new_spanned(
                &input_fn.sig,
                "visca_inquiry methods must have a return type",
            )
            .to_compile_error()
            .into();
        }
    };

    // Parse the command from the block
    let command_expr = if let Some(stmt) = block.stmts.first() {
        if let syn::Stmt::Expr(expr, _) = stmt {
            expr
        } else {
            return syn::Error::new_spanned(
                block,
                "Expected InquiryCommand expression in function body",
            )
            .to_compile_error()
            .into();
        }
    } else {
        return syn::Error::new_spanned(block, "Function body must contain an InquiryCommand")
            .to_compile_error()
            .into();
    };

    // Generate the response parsing logic based on the command
    let response_parsing = generate_inquiry_response_parsing(command_expr, return_type);

    let expanded = quote! {
        #[cfg(feature = "async")]
        #(#attrs)*
        #vis async fn #fn_name #generics(#inputs) #output {
            let command = #block;
            let response = self.send_and_receive(&command).await?;
            #response_parsing
        }

        #[cfg(not(feature = "async"))]
        #(#attrs)*
        #vis fn #fn_name #generics(#inputs) #output {
            let command = #block;
            let response = self.send_and_receive(&command)?;
            #response_parsing
        }
    };

    TokenStream::from(expanded)
}

/// Generate response parsing code based on the inquiry command and expected return type
fn generate_inquiry_response_parsing(
    command_expr: &syn::Expr,
    return_type: &syn::Type,
) -> proc_macro2::TokenStream {
    // Extract the InquiryCommand variant from the expression
    let command_variant = if let syn::Expr::Path(expr_path) = command_expr {
        if let Some(segment) = expr_path.path.segments.last() {
            segment.ident.to_string()
        } else {
            return quote! {
                return Err(crate::Error::UnexpectedResponseType);
            };
        }
    } else {
        return quote! {
            return Err(crate::Error::UnexpectedResponseType);
        };
    };

    // Map command variant to response variant and extraction
    let response_match = match command_variant.as_str() {
        "Power" => quote! {
            match response {
                crate::Response::InquiryResponse(crate::command::InquiryResponse::Power { on }) => Ok(on),
                crate::Response::Error(e) => Err(e),
                _ => Err(crate::Error::UnexpectedResponseType),
            }
        },
        "ZoomPosition" => quote! {
            match response {
                crate::Response::InquiryResponse(crate::command::InquiryResponse::ZoomPosition { position }) => Ok(position),
                crate::Response::Error(e) => Err(e),
                _ => Err(crate::Error::UnexpectedResponseType),
            }
        },
        "PanTiltPosition" => {
            // Check if return type is a tuple
            if let syn::Type::Tuple(_) = return_type {
                quote! {
                    match response {
                        crate::Response::InquiryResponse(crate::command::InquiryResponse::PanTiltPosition { pan, tilt }) => Ok((pan, tilt)),
                        crate::Response::Error(e) => Err(e),
                        _ => Err(crate::Error::UnexpectedResponseType),
                    }
                }
            } else {
                quote! {
                    match response {
                        crate::Response::InquiryResponse(crate::command::InquiryResponse::PanTiltPosition { pan, tilt }) => Ok(PanTiltPosition { pan, tilt }),
                        crate::Response::Error(e) => Err(e),
                        _ => Err(crate::Error::UnexpectedResponseType),
                    }
                }
            }
        }
        "FocusPosition" => quote! {
            match response {
                crate::Response::InquiryResponse(crate::command::InquiryResponse::FocusPosition { position }) => Ok(position),
                crate::Response::Error(e) => Err(e),
                _ => Err(crate::Error::UnexpectedResponseType),
            }
        },
        "ExposureMode" => quote! {
            match response {
                crate::Response::InquiryResponse(crate::command::InquiryResponse::ExposureMode { mode }) => Ok(mode),
                crate::Response::Error(e) => Err(e),
                _ => Err(crate::Error::UnexpectedResponseType),
            }
        },
        "WhiteBalanceMode" => quote! {
            match response {
                crate::Response::InquiryResponse(crate::command::InquiryResponse::WhiteBalance { mode }) => Ok(mode),
                crate::Response::Error(e) => Err(e),
                _ => Err(crate::Error::UnexpectedResponseType),
            }
        },
        "Luminance" => quote! {
            match response {
                crate::Response::InquiryResponse(crate::command::InquiryResponse::Luminance(level)) => Ok(level),
                crate::Response::Error(e) => Err(e),
                _ => Err(crate::Error::UnexpectedResponseType),
            }
        },
        "Contrast" => quote! {
            match response {
                crate::Response::InquiryResponse(crate::command::InquiryResponse::Contrast(level)) => Ok(level),
                crate::Response::Error(e) => Err(e),
                _ => Err(crate::Error::UnexpectedResponseType),
            }
        },
        "Sharpness" => quote! {
            match response {
                crate::Response::InquiryResponse(crate::command::InquiryResponse::Sharpness { value }) => Ok(value),
                crate::Response::Error(e) => Err(e),
                _ => Err(crate::Error::UnexpectedResponseType),
            }
        },
        "Saturation" => quote! {
            match response {
                crate::Response::InquiryResponse(crate::command::InquiryResponse::Saturation { level }) => Ok(level),
                crate::Response::Error(e) => Err(e),
                _ => Err(crate::Error::UnexpectedResponseType),
            }
        },
        "Hue" => quote! {
            match response {
                crate::Response::InquiryResponse(crate::command::InquiryResponse::Hue { hue }) => Ok(hue),
                crate::Response::Error(e) => Err(e),
                _ => Err(crate::Error::UnexpectedResponseType),
            }
        },
        "Gain" => quote! {
            match response {
                crate::Response::InquiryResponse(crate::command::InquiryResponse::Gain { gain }) => Ok(gain),
                crate::Response::Error(e) => Err(e),
                _ => Err(crate::Error::UnexpectedResponseType),
            }
        },
        "GainLimit" => quote! {
            match response {
                crate::Response::InquiryResponse(crate::command::InquiryResponse::GainLimit { limit }) => Ok(limit),
                crate::Response::Error(e) => Err(e),
                _ => Err(crate::Error::UnexpectedResponseType),
            }
        },
        "Iris" => quote! {
            match response {
                crate::Response::InquiryResponse(crate::command::InquiryResponse::Iris { position }) => Ok(position),
                crate::Response::Error(e) => Err(e),
                _ => Err(crate::Error::UnexpectedResponseType),
            }
        },
        "Shutter" => quote! {
            match response {
                crate::Response::InquiryResponse(crate::command::InquiryResponse::Shutter { position }) => Ok(position),
                crate::Response::Error(e) => Err(e),
                _ => Err(crate::Error::UnexpectedResponseType),
            }
        },
        "Bright" => quote! {
            match response {
                crate::Response::InquiryResponse(crate::command::InquiryResponse::Bright { position }) => Ok(position),
                crate::Response::Error(e) => Err(e),
                _ => Err(crate::Error::UnexpectedResponseType),
            }
        },
        "ExposureCompensation" => quote! {
            match response {
                crate::Response::InquiryResponse(crate::command::InquiryResponse::ExposureCompensation { value }) => Ok(value),
                crate::Response::Error(e) => Err(e),
                _ => Err(crate::Error::UnexpectedResponseType),
            }
        },
        "ExposureCompensationMode" => quote! {
            match response {
                crate::Response::InquiryResponse(crate::command::InquiryResponse::ExposureCompensationMode { on }) => Ok(on),
                crate::Response::Error(e) => Err(e),
                _ => Err(crate::Error::UnexpectedResponseType),
            }
        },
        "Backlight" => quote! {
            match response {
                crate::Response::InquiryResponse(crate::command::InquiryResponse::Backlight { status }) => Ok(status),
                crate::Response::Error(e) => Err(e),
                _ => Err(crate::Error::UnexpectedResponseType),
            }
        },
        "AntiFlicker" => quote! {
            match response {
                crate::Response::InquiryResponse(crate::command::InquiryResponse::AntiFlicker { mode }) => Ok(mode),
                crate::Response::Error(e) => Err(e),
                _ => Err(crate::Error::UnexpectedResponseType),
            }
        },
        "RedGain" => quote! {
            match response {
                crate::Response::InquiryResponse(crate::command::InquiryResponse::RedGain { gain }) => Ok(gain),
                crate::Response::Error(e) => Err(e),
                _ => Err(crate::Error::UnexpectedResponseType),
            }
        },
        "BlueGain" => quote! {
            match response {
                crate::Response::InquiryResponse(crate::command::InquiryResponse::BlueGain { gain }) => Ok(gain),
                crate::Response::Error(e) => Err(e),
                _ => Err(crate::Error::UnexpectedResponseType),
            }
        },
        "ImageFlip" => {
            // Check if return type is a tuple
            if let syn::Type::Tuple(_) = return_type {
                quote! {
                    match response {
                        crate::Response::InquiryResponse(crate::command::InquiryResponse::ImageFlip { vertical, horizontal }) => Ok((vertical, horizontal)),
                        crate::Response::Error(e) => Err(e),
                        _ => Err(crate::Error::UnexpectedResponseType),
                    }
                }
            } else {
                quote! {
                    match response {
                        crate::Response::InquiryResponse(crate::command::InquiryResponse::ImageFlip { vertical, horizontal }) => Ok(ImageFlipState { vertical, horizontal }),
                        crate::Response::Error(e) => Err(e),
                        _ => Err(crate::Error::UnexpectedResponseType),
                    }
                }
            }
        }
        "SharpnessMode" => quote! {
            match response {
                crate::Response::InquiryResponse(crate::command::InquiryResponse::SharpnessMode { mode }) => Ok(mode),
                crate::Response::Error(e) => Err(e),
                _ => Err(crate::Error::UnexpectedResponseType),
            }
        },
        "ColorTemperature" => quote! {
            match response {
                crate::Response::InquiryResponse(crate::command::InquiryResponse::ColorTemperature { temperature }) => Ok(temperature),
                crate::Response::Error(e) => Err(e),
                _ => Err(crate::Error::UnexpectedResponseType),
            }
        },
        "NoiseReduction2D" => quote! {
            match response {
                crate::Response::InquiryResponse(crate::command::InquiryResponse::NoiseReduction2D { level }) => Ok(level),
                crate::Response::Error(e) => Err(e),
                _ => Err(crate::Error::UnexpectedResponseType),
            }
        },
        "NoiseReduction3D" => quote! {
            match response {
                crate::Response::InquiryResponse(crate::command::InquiryResponse::NoiseReduction3D { level }) => Ok(level),
                crate::Response::Error(e) => Err(e),
                _ => Err(crate::Error::UnexpectedResponseType),
            }
        },
        "BlackWhite" => quote! {
            match response {
                crate::Response::InquiryResponse(crate::command::InquiryResponse::BlackWhite { on }) => Ok(on),
                crate::Response::Error(e) => Err(e),
                _ => Err(crate::Error::UnexpectedResponseType),
            }
        },
        "FocusZone" => quote! {
            match response {
                crate::Response::InquiryResponse(crate::command::InquiryResponse::FocusZone { zone }) => Ok(zone),
                crate::Response::Error(e) => Err(e),
                _ => Err(crate::Error::UnexpectedResponseType),
            }
        },
        "AutoFocusSensitivity" => quote! {
            match response {
                crate::Response::InquiryResponse(crate::command::InquiryResponse::AutoFocusSensitivity { sensitivity }) => Ok(sensitivity),
                crate::Response::Error(e) => Err(e),
                _ => Err(crate::Error::UnexpectedResponseType),
            }
        },
        "FocusNearLimit" => quote! {
            match response {
                crate::Response::InquiryResponse(crate::command::InquiryResponse::FocusNearLimit { position }) => Ok(position),
                crate::Response::Error(e) => Err(e),
                _ => Err(crate::Error::UnexpectedResponseType),
            }
        },
        "DynamicRange" => quote! {
            match response {
                crate::Response::InquiryResponse(crate::command::InquiryResponse::DynamicRange { level }) => Ok(level),
                crate::Response::Error(e) => Err(e),
                _ => Err(crate::Error::UnexpectedResponseType),
            }
        },
        _ => quote! {
            // Default case for unknown commands
            return Err(crate::Error::UnexpectedResponseType);
        },
    };

    response_match
}

/// Macro for commands that take position parameters with automatic validation.
///
/// This macro generates methods that:
/// - Validate position ranges based on camera profile limits
/// - Handle unit conversions (degrees, normalized, raw units)
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
#[proc_macro_attribute]
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
#[proc_macro_attribute]
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
                    if let Some(range_expr) = range_specs.get(&format!("{}_range", param_name_str))
                    {
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
fn generate_speed_level_variant(
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
        &format!("{}_with_level", fn_name),
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
                self.#fn_name(#(#call_params),*)
            }
        }
    }
}

/// Macro for commands that have bounded parameters (e.g., levels 0-7, percentages 0-100).
///
/// This macro generates methods with automatic validation for bounded values,
/// including percentage-based variants and named level enums.
///
/// # Example
///
/// ```rust,ignore
/// #[visca_bounded_command(
///     luminance_level = "0..=7",
///     saturation_level = "0..=0x0E"
/// )]
/// fn set_image_adjustments(&self, luminance_level: u8, saturation_level: u8) -> Result<()> {
///     let cmd = ImageCommand::AdjustLevels {
///         luminance: ImageLevel::new(luminance_level)?,
///         saturation: SaturationLevel::new(saturation_level)?,
///     };
///     self.send(&cmd)
/// }
/// ```
#[proc_macro_attribute]
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
fn generate_percentage_variant(
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
            /// Set values using percentages (0.0 to 100.0).
            #vis fn #fn_name_percentage(&self, #(#params),*) -> Result<(), crate::Error> {
                #(#param_conversions)*
                self.#fn_name(#(#param_names),*)
            }
        }
    }
}

/// Generate a variant that uses named level enums
fn generate_level_variant(
    input_fn: &ItemFn,
    bounded_params: &[(syn::Ident, String)],
) -> proc_macro2::TokenStream {
    // Only generate level variant for parameters with specific patterns
    let level_params: Vec<_> = bounded_params
        .iter()
        .filter(|(name, _)| {
            let name_str = name.to_string();
            name_str.contains("level") || name_str.contains("gain") || name_str.contains("limit")
        })
        .collect();

    if level_params.is_empty() {
        return quote! {};
    }

    let vis = &input_fn.vis;
    let fn_name = &input_fn.sig.ident;
    let fn_name_with_levels = syn::Ident::new(
        &format!("{}_with_levels", fn_name),
        proc_macro2::Span::call_site(),
    );

    // Build new parameter list with level enums
    let mut new_params = Vec::new();
    let mut conversions = Vec::new();

    for input in input_fn.sig.inputs.iter() {
        match input {
            syn::FnArg::Receiver(_) => new_params.push(quote! { &self }),
            syn::FnArg::Typed(pat_type) => {
                if let syn::Pat::Ident(pat_ident) = &*pat_type.pat {
                    let param_name = &pat_ident.ident;
                    let param_name_str = param_name.to_string();

                    if level_params.iter().any(|(name, _)| name == param_name) {
                        // Determine the appropriate level type
                        let level_type = if param_name_str.contains("luminance") {
                            quote! { crate::types::LuminanceLevel }
                        } else if param_name_str.contains("contrast") {
                            quote! { crate::types::ContrastLevel }
                        } else if param_name_str.contains("saturation") {
                            quote! { crate::types::SaturationLevel }
                        } else if param_name_str.contains("sharpness") {
                            quote! { crate::types::SharpnessLevel }
                        } else if param_name_str.contains("gain") {
                            quote! { crate::types::GainLevel }
                        } else {
                            quote! { crate::types::Level }
                        };

                        new_params.push(quote! { #param_name: #level_type });
                        conversions.push(quote! {
                            let #param_name = #param_name.value();
                        });
                    } else {
                        // Keep other parameters as-is
                        new_params.push(quote! { #pat_type });
                    }
                }
            }
        }
    }

    // Extract all parameters for the method call
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
        quote! {
            #[cfg(feature = "async")]
            /// Execute command with named level enums.
            #vis async fn #fn_name_with_levels(#(#new_params),*) -> Result<(), crate::Error> {
                #(#conversions)*
                self.#fn_name(#(#call_params),*).await
            }

            #[cfg(not(feature = "async"))]
            /// Execute command with named level enums.
            #vis fn #fn_name_with_levels(#(#new_params),*) -> Result<(), crate::Error> {
                #(#conversions)*
                self.#fn_name(#(#call_params),*)
            }
        }
    } else {
        quote! {
            /// Execute command with named level enums.
            #vis fn #fn_name_with_levels(#(#new_params),*) -> Result<(), crate::Error> {
                #(#conversions)*
                self.#fn_name(#(#call_params),*)
            }
        }
    }
}

/// Derive macro for generating common implementations for VISCA value types.
///
/// This macro generates bounded value types with validation and automatic
/// trait implementations for VISCA protocol value types.
///
/// # Features
/// - Automatic `new()` constructor with validation
/// - `value()` getter method
/// - `TryFrom` implementations for the underlying type
/// - `From` implementation for converting back to the underlying type
/// - `Display` implementation
/// - Optional min/max bounds validation
/// - Optional valid values list
///
/// # Attributes
///
/// - `#[visca_value(min = "0x00", max = "0xFF")]` - Set min/max bounds
/// - `#[visca_value(valid_values = "[0x00, 0x01, 0x02]")]` - Set valid values list
/// - `#[visca_value(display_format = "hex")]` - Use hex display format
/// - `#[visca_value(display_format = "decimal")]` - Use decimal display format
/// - `#[visca_value(display_prefix = "Level")]` - Add prefix to display
///
/// # Example
///
/// ```rust,ignore
/// #[derive(Debug, Clone, Copy, PartialEq, Eq, ViscaValue)]
/// #[visca_value(min = "0x00", max = "0x07", display_format = "hex")]
/// pub struct GainValue(u8);
///
/// #[derive(Debug, Clone, Copy, PartialEq, Eq, ViscaValue)]
/// #[visca_value(valid_values = "[0x00, 0x01, 0x02, 0x03]", display_prefix = "Mode")]
/// pub struct ExposureMode(u8);
/// ```
#[proc_macro_derive(ViscaValue, attributes(visca_value))]
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

    let expanded = quote! {
        impl #name {
            #constants

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

/// A macro for methods that can fail during command construction.
///
/// This macro provides better error propagation than the current `?` in macro blocks
/// by automatically adding error context and supporting both async and sync variants.
///
/// # Features
/// - Automatic error context addition with command name and parameters
/// - Support for methods that return `Result<(), Error>` during construction
/// - Handles both async and sync implementations
/// - Better error messages with parameter information
///
/// # Example
///
/// ```rust,ignore
/// #[visca_fallible_method]
/// pub fn set_pan_tilt_absolute(
///     &self,
///     pan: i16,
///     tilt: i16,
///     pan_speed: u8,
///     tilt_speed: u8,
/// ) -> Result<PanTiltCommand, Error> {
///     // Validate parameters
///     if pan < -170 || pan > 170 {
///         return Err(Error::ParameterOutOfRange {
///             parameter: "pan".to_string(),
///             value: pan as i32,
///             min: -170,
///             max: 170,
///         });
///     }
///     
///     Ok(PanTiltCommand::AbsolutePosition {
///         pan: PanPosition::new(pan)?,
///         tilt: TiltPosition::new(tilt)?,
///         pan_speed: PanSpeed::new(pan_speed)?,
///         tilt_speed: TiltSpeed::new(tilt_speed)?,
///     })
/// }
/// ```
#[proc_macro_attribute]
pub fn visca_fallible_method(_attr: TokenStream, item: TokenStream) -> TokenStream {
    let input_fn = parse_macro_input!(item as ItemFn);

    let vis = &input_fn.vis;
    let sig = &input_fn.sig;
    let fn_name = &sig.ident;
    let generics = &sig.generics;
    let inputs = &sig.inputs;
    let attrs = &input_fn.attrs;
    let block = &input_fn.block;

    // Extract parameter names for error context
    let param_names: Vec<_> = inputs
        .iter()
        .skip(1) // Skip &self
        .filter_map(|arg| {
            if let syn::FnArg::Typed(pat_type) = arg {
                if let syn::Pat::Ident(pat_ident) = &*pat_type.pat {
                    return Some(&pat_ident.ident);
                }
            }
            None
        })
        .collect();

    // Build parameter context string
    let context_params = if param_names.is_empty() {
        quote! { String::new() }
    } else {
        let param_formats: Vec<_> = param_names
            .iter()
            .map(|name| {
                let name_str = name.to_string();
                quote! { format!("{}: {:?}", #name_str, #name) }
            })
            .collect();
        quote! {
            vec![#(#param_formats),*].join(", ")
        }
    };

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

    let expanded = quote! {
        #[cfg(feature = "async")]
        #(#attrs)*
        #vis async fn #fn_name #generics(#inputs) -> Result<(), grafton_visca::Error> {
            let result: Result<_, grafton_visca::Error> = (|| {
                #block
            })();

            match result {
                Ok(cmd) => self.send_and_wait(&cmd).await,
                Err(e) => {
                    let context = format!(
                        "Failed to construct {} command with parameters: {}",
                        stringify!(#fn_name),
                        #context_params
                    );
                    log::error!("{}: {:?}", context, e);
                    Err(e)
                }
            }
        }

        #[cfg(not(feature = "async"))]
        #(#attrs)*
        #vis fn #fn_name #generics(#(#blocking_inputs),*) -> Result<(), grafton_visca::Error> {
            let result: Result<_, grafton_visca::Error> = (|| {
                #block
            })();

            match result {
                Ok(cmd) => self.send_and_wait(&cmd),
                Err(e) => {
                    let context = format!(
                        "Failed to construct {} command with parameters: {}",
                        stringify!(#fn_name),
                        #context_params
                    );
                    log::error!("{}: {:?}", context, e);
                    Err(e)
                }
            }
        }
    };

    TokenStream::from(expanded)
}

/// Generate mock transport implementations for testing.
///
/// This macro generates both blocking and async mock transports that can be
/// programmed with expected command/response sequences for testing.
///
/// # Features
/// - Pre-programmed command/response sequences
/// - Error injection capabilities
/// - Command history tracking
/// - Timeout simulation
/// - Both blocking and async implementations
///
/// # Example
///
/// ```rust,ignore
/// #[visca_mock_transport]
/// struct TestTransport {
///     sequences: Vec<(Vec<u8>, Vec<u8>)>,  // (expected_command, response)
///     error_after: Option<usize>,           // Inject error after N commands
///     timeout_after: Option<usize>,         // Timeout after N commands
/// }
/// ```
///
/// Generates:
/// - `MockTransport` with blocking implementation
/// - `AsyncMockTransport` with async implementation
/// - Helper methods for test setup
#[proc_macro_attribute]
pub fn visca_mock_transport(_attr: TokenStream, item: TokenStream) -> TokenStream {
    let input = parse_macro_input!(item as DeriveInput);
    let name = &input.ident;

    let mock_name = format_ident!("Mock{}", name);
    let async_mock_name = format_ident!("AsyncMock{}", name);

    let expanded = quote! {
        #input

        /// Mock transport for testing with pre-programmed responses.
        #[derive(Debug, Clone)]
        pub struct #mock_name {
            sequences: Vec<(Vec<u8>, Vec<u8>)>,
            current_index: std::cell::RefCell<usize>,
            command_history: std::cell::RefCell<Vec<Vec<u8>>>,
            error_after: Option<usize>,
            timeout_after: Option<usize>,
            delay_ms: Option<u64>,
        }

        impl #mock_name {
            /// Create a new mock transport with the given command/response sequences.
            pub fn new(sequences: Vec<(Vec<u8>, Vec<u8>)>) -> Self {
                Self {
                    sequences,
                    current_index: std::cell::RefCell::new(0),
                    command_history: std::cell::RefCell::new(Vec::new()),
                    error_after: None,
                    timeout_after: None,
                    delay_ms: None,
                }
            }

            /// Configure to return an error after N commands.
            pub fn with_error_after(mut self, n: usize) -> Self {
                self.error_after = Some(n);
                self
            }

            /// Configure to timeout after N commands.
            pub fn with_timeout_after(mut self, n: usize) -> Self {
                self.timeout_after = Some(n);
                self
            }

            /// Configure to delay responses by the given milliseconds.
            pub fn with_delay(mut self, delay_ms: u64) -> Self {
                self.delay_ms = Some(delay_ms);
                self
            }

            /// Get the history of commands sent to this transport.
            pub fn command_history(&self) -> Vec<Vec<u8>> {
                self.command_history.borrow().clone()
            }

            /// Reset the mock transport state.
            pub fn reset(&self) {
                *self.current_index.borrow_mut() = 0;
                self.command_history.borrow_mut().clear();
            }

            /// Add a new command/response sequence.
            pub fn add_sequence(&mut self, command: Vec<u8>, response: Vec<u8>) {
                self.sequences.push((command, response));
            }
        }

        impl crate::transport::blocking::BlockingTransport for #mock_name {
            fn send_and_receive(&mut self, data: &[u8]) -> Result<Vec<u8>, crate::Error> {
                let mut index = self.current_index.borrow_mut();
                let mut history = self.command_history.borrow_mut();

                // Record the command
                history.push(data.to_vec());

                // Check for error injection
                if let Some(error_after) = self.error_after {
                    if history.len() > error_after {
                        return Err(crate::Error::Transport("Injected error".into()));
                    }
                }

                // Check for timeout injection
                if let Some(timeout_after) = self.timeout_after {
                    if history.len() > timeout_after {
                        return Err(crate::Error::Timeout);
                    }
                }

                // Simulate delay if configured
                if let Some(delay_ms) = self.delay_ms {
                    std::thread::sleep(std::time::Duration::from_millis(delay_ms));
                }

                // Find matching sequence
                if *index < self.sequences.len() {
                    let (expected_cmd, response) = &self.sequences[*index];
                    if data == expected_cmd.as_slice() {
                        *index += 1;
                        Ok(response.clone())
                    } else {
                        Err(crate::Error::Transport(format!(
                            "Unexpected command. Expected {:?}, got {:?}",
                            expected_cmd, data
                        )))
                    }
                } else {
                    Err(crate::Error::Transport("No more mock responses available".into()))
                }
            }
        }

        #[cfg(feature = "async")]
        /// Async mock transport for testing with pre-programmed responses.
        #[derive(Debug, Clone)]
        pub struct #async_mock_name {
            inner: #mock_name,
        }

        #[cfg(feature = "async")]
        impl #async_mock_name {
            /// Create a new async mock transport with the given command/response sequences.
            pub fn new(sequences: Vec<(Vec<u8>, Vec<u8>)>) -> Self {
                Self {
                    inner: #mock_name::new(sequences),
                }
            }

            /// Configure to return an error after N commands.
            pub fn with_error_after(mut self, n: usize) -> Self {
                self.inner = self.inner.with_error_after(n);
                self
            }

            /// Configure to timeout after N commands.
            pub fn with_timeout_after(mut self, n: usize) -> Self {
                self.inner = self.inner.with_timeout_after(n);
                self
            }

            /// Configure to delay responses by the given milliseconds.
            pub fn with_delay(mut self, delay_ms: u64) -> Self {
                self.inner = self.inner.with_delay(delay_ms);
                self
            }

            /// Get the history of commands sent to this transport.
            pub fn command_history(&self) -> Vec<Vec<u8>> {
                self.inner.command_history()
            }

            /// Reset the mock transport state.
            pub fn reset(&self) {
                self.inner.reset()
            }

            /// Add a new command/response sequence.
            pub fn add_sequence(&mut self, command: Vec<u8>, response: Vec<u8>) {
                self.inner.add_sequence(command, response);
            }
        }

        #[cfg(feature = "async")]
        #[async_trait::async_trait]
        impl crate::transport::AsyncTransport for #async_mock_name {
            async fn send_and_receive(&mut self, data: &[u8]) -> Result<Vec<u8>, crate::Error> {
                let mut index = self.inner.current_index.borrow_mut();
                let mut history = self.inner.command_history.borrow_mut();

                // Record the command
                history.push(data.to_vec());

                // Check for error injection
                if let Some(error_after) = self.inner.error_after {
                    if history.len() > error_after {
                        return Err(crate::Error::Transport("Injected error".into()));
                    }
                }

                // Check for timeout injection
                if let Some(timeout_after) = self.inner.timeout_after {
                    if history.len() > timeout_after {
                        return Err(crate::Error::Timeout);
                    }
                }

                // Simulate delay if configured
                if let Some(delay_ms) = self.inner.delay_ms {
                    tokio::time::sleep(std::time::Duration::from_millis(delay_ms)).await;
                }

                // Find matching sequence
                if *index < self.inner.sequences.len() {
                    let (expected_cmd, response) = &self.inner.sequences[*index];
                    if data == expected_cmd.as_slice() {
                        *index += 1;
                        Ok(response.clone())
                    } else {
                        Err(crate::Error::Transport(format!(
                            "Unexpected command. Expected {:?}, got {:?}",
                            expected_cmd, data
                        )))
                    }
                } else {
                    Err(crate::Error::Transport("No more mock responses available".into()))
                }
            }
        }
    };

    TokenStream::from(expanded)
}
/// Generate validation tests for VISCA commands.
///
/// This macro generates comprehensive test suites that validate:
/// - Command byte sequences against documentation
/// - Parameter bounds for all camera profiles
/// - Response parsing for all command types
/// - Error cases and edge conditions
///
/// # Example
///
/// ```rust,ignore
/// #[visca_test_suite]
/// mod zoom_command_tests {
///     use super::*;
///     
///     #[test_command(
///         command = "ZoomCommand::Direct(ZoomPosition::new(0x4000)?)",
///         expected_bytes = "[0x81, 0x01, 0x04, 0x47, 0x04, 0x00, 0x00, 0x00, 0xFF]",
///         profiles = ["PTZOpticsG2", "SonyEVID70"]
///     )]
///     fn test_zoom_direct() {}
///     
///     #[test_bounds(
///         parameter = "zoom_position",
///         min = "0x0000",
///         max = "0x7000",
///         profiles = ["PTZOpticsG2"]
///     )]
///     fn test_zoom_bounds() {}
/// }
/// ```
#[proc_macro_attribute]
pub fn visca_test_suite(_attr: TokenStream, item: TokenStream) -> TokenStream {
    let input = parse_macro_input!(item as syn::ItemMod);
    let mod_name = &input.ident;
    let mod_vis = &input.vis;
    let mod_attrs = &input.attrs;

    let content = match &input.content {
        Some((_, items)) => items,
        None => {
            return syn::Error::new_spanned(
                &input,
                "visca_test_suite must be applied to a module with content",
            )
            .to_compile_error()
            .into();
        }
    };

    let mut generated_tests = Vec::new();

    // Process each item in the module
    for item in content {
        if let syn::Item::Fn(func) = item {
            // Check for test attributes
            for attr in &func.attrs {
                if attr.path().is_ident("test_command") {
                    let test = generate_command_test(func, attr);
                    generated_tests.push(test);
                } else if attr.path().is_ident("test_bounds") {
                    let test = generate_bounds_test(func, attr);
                    generated_tests.push(test);
                } else if attr.path().is_ident("test_response") {
                    let test = generate_response_test(func, attr);
                    generated_tests.push(test);
                }
            }
        }
    }

    let expanded = quote! {
        #(#mod_attrs)*
        #mod_vis mod #mod_name {
            use super::*;

            #(#generated_tests)*
        }
    };

    TokenStream::from(expanded)
}

/// Generate a test for command byte sequences
fn generate_command_test(func: &syn::ItemFn, attr: &syn::Attribute) -> proc_macro2::TokenStream {
    let fn_name = &func.sig.ident;
    let mut command_expr = None;
    let mut expected_bytes = None;
    let mut profiles = Vec::new();

    // Parse test attributes
    let _ = attr.parse_nested_meta(|meta| {
        if meta.path.is_ident("command") {
            let value: syn::LitStr = meta.value()?.parse()?;
            command_expr = Some(value.value());
        } else if meta.path.is_ident("expected_bytes") {
            let value: syn::LitStr = meta.value()?.parse()?;
            expected_bytes = Some(value.value());
        } else if meta.path.is_ident("profiles") {
            let value: syn::LitStr = meta.value()?.parse()?;
            // Parse the profiles array
            let profiles_str = value.value();
            profiles = profiles_str
                .trim_start_matches('[')
                .trim_end_matches(']')
                .split(',')
                .map(|s| s.trim().trim_matches('"').to_string())
                .collect();
        }
        Ok(())
    });

    let command_tokens: proc_macro2::TokenStream = command_expr
        .unwrap_or_default()
        .parse()
        .unwrap_or_else(|_| quote! { panic!("Invalid command expression") });

    let expected_tokens: proc_macro2::TokenStream = expected_bytes
        .unwrap_or_default()
        .parse()
        .unwrap_or_else(|_| quote! { vec![] });

    // Generate test for each profile
    let profile_tests: Vec<_> = profiles
        .iter()
        .map(|profile| {
            let profile_ident = format_ident!("{}", profile);
            let test_name = format_ident!("{}_{}", fn_name, profile.to_lowercase());

            quote! {
                #[test]
                fn #test_name() {
                    let cmd = #command_tokens;
                    let expected = #expected_tokens;
                    let actual = cmd.to_bytes().expect("Failed to convert command to bytes");

                    assert_eq!(
                        actual, expected,
                        "Command bytes mismatch for profile {}.\nExpected: {:?}\nActual: {:?}",
                        stringify!(#profile_ident), expected, actual
                    );
                }
            }
        })
        .collect();

    quote! {
        #(#profile_tests)*
    }
}

/// Generate a test for parameter bounds
fn generate_bounds_test(func: &syn::ItemFn, attr: &syn::Attribute) -> proc_macro2::TokenStream {
    let fn_name = &func.sig.ident;
    let mut parameter = None;
    let mut min_value = None;
    let mut max_value = None;
    let mut profiles = Vec::new();

    // Parse test attributes
    let _ = attr.parse_nested_meta(|meta| {
        if meta.path.is_ident("parameter") {
            let value: syn::LitStr = meta.value()?.parse()?;
            parameter = Some(value.value());
        } else if meta.path.is_ident("min") {
            let value: syn::LitStr = meta.value()?.parse()?;
            min_value = Some(value.value());
        } else if meta.path.is_ident("max") {
            let value: syn::LitStr = meta.value()?.parse()?;
            max_value = Some(value.value());
        } else if meta.path.is_ident("profiles") {
            let value: syn::LitStr = meta.value()?.parse()?;
            let profiles_str = value.value();
            profiles = profiles_str
                .trim_start_matches('[')
                .trim_end_matches(']')
                .split(',')
                .map(|s| s.trim().trim_matches('"').to_string())
                .collect();
        }
        Ok(())
    });

    let param_name = parameter.unwrap_or_default();
    let min_tokens: proc_macro2::TokenStream = min_value
        .unwrap_or_default()
        .parse()
        .unwrap_or_else(|_| quote! { 0 });
    let max_tokens: proc_macro2::TokenStream = max_value
        .unwrap_or_default()
        .parse()
        .unwrap_or_else(|_| quote! { 0 });

    // Generate bounds tests
    let bounds_tests: Vec<_> = profiles
        .iter()
        .map(|profile| {
            let test_name = format_ident!("{}_{}_bounds", fn_name, profile.to_lowercase());

            quote! {
                #[test]
                fn #test_name() {
                    // Test minimum value
                    let min_result = #param_name::new(#min_tokens);
                    assert!(min_result.is_ok(), "Minimum value {} should be valid", #min_tokens);

                    // Test maximum value
                    let max_result = #param_name::new(#max_tokens);
                    assert!(max_result.is_ok(), "Maximum value {} should be valid", #max_tokens);

                    // Test below minimum
                    if #min_tokens > 0 {
                        let below_min = #param_name::new(#min_tokens - 1);
                        assert!(below_min.is_err(), "Value below minimum should be invalid");
                    }

                    // Test above maximum
                    let above_max = #param_name::new(#max_tokens + 1);
                    assert!(above_max.is_err(), "Value above maximum should be invalid");
                }
            }
        })
        .collect();

    quote! {
        #(#bounds_tests)*
    }
}

/// Generate a test for response parsing
fn generate_response_test(func: &syn::ItemFn, attr: &syn::Attribute) -> proc_macro2::TokenStream {
    let fn_name = &func.sig.ident;
    let mut response_bytes = None;
    let mut expected_value = None;
    let mut response_type = None;

    // Parse test attributes
    let _ = attr.parse_nested_meta(|meta| {
        if meta.path.is_ident("response_bytes") {
            let value: syn::LitStr = meta.value()?.parse()?;
            response_bytes = Some(value.value());
        } else if meta.path.is_ident("expected") {
            let value: syn::LitStr = meta.value()?.parse()?;
            expected_value = Some(value.value());
        } else if meta.path.is_ident("response_type") {
            let value: syn::LitStr = meta.value()?.parse()?;
            response_type = Some(value.value());
        }
        Ok(())
    });

    let bytes_tokens: proc_macro2::TokenStream = response_bytes
        .unwrap_or_default()
        .parse()
        .unwrap_or_else(|_| quote! { vec![] });

    let expected_tokens: proc_macro2::TokenStream = expected_value
        .unwrap_or_default()
        .parse()
        .unwrap_or_else(|_| quote! { () });

    let type_tokens: proc_macro2::TokenStream = response_type
        .unwrap_or_default()
        .parse()
        .unwrap_or_else(|_| quote! { () });

    quote! {
        #[test]
        fn #fn_name() {
            let response_bytes = #bytes_tokens;
            let response = Response::parse(&response_bytes, Some(#type_tokens))
                .expect("Failed to parse response");

            match response {
                Response::InquiryResponse(inquiry) => {
                    let actual = inquiry.into_value();
                    assert_eq!(actual, #expected_tokens, "Response value mismatch");
                }
                _ => panic!("Expected inquiry response, got {:?}", response),
            }
        }
    }
}
