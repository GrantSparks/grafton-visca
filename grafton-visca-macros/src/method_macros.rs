//! Method-related macros for VISCA command generation.
//!
//! This module contains macros that generate async/sync method implementations
//! for VISCA camera control operations.

use proc_macro::TokenStream;
use quote::quote;
use syn::{parse_macro_input, ItemFn, ReturnType, Type};

/// A macro that generates async and sync versions of VISCA command methods.
///
/// This macro takes a method that returns a command struct and automatically generates:
/// - An async version (when `async` feature is enabled) that sends the command
/// - A sync version (when `async` feature is disabled) that sends the command
///
/// # Features
/// - Automatically adds `await` for async versions
/// - Changes `&self` to `&mut self` for blocking versions
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
pub fn visca_camera_method(_attr: TokenStream, item: TokenStream) -> TokenStream {
    // For now, just delegate to visca_method_custom
    visca_method_custom(_attr, item)
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
                "visca_inquiry method body must be a single expression",
            )
            .to_compile_error()
            .into();
        }
    } else {
        return syn::Error::new_spanned(block, "visca_inquiry method body cannot be empty")
            .to_compile_error()
            .into();
    };

    // Generate the response parsing logic based on the command
    let response_parsing = generate_inquiry_response_parsing(command_expr, return_type);

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
        #vis async fn #fn_name #generics(#inputs) #output {
            let command = #block;
            let response = self.send_and_receive(&command).await?;
            #response_parsing
        }

        #[cfg(not(feature = "async"))]
        #(#attrs)*
        #vis fn #fn_name #generics(#(#blocking_inputs),*) #output {
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
