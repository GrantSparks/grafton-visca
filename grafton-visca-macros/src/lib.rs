use proc_macro::TokenStream;
use quote::quote;
use syn::{parse_macro_input, ItemFn, ReturnType, Type};

/// A method-level attribute macro for generating VISCA camera control methods.
///
/// This macro automatically generates both async and sync versions of a method
/// from a single definition. The method body should return a VISCA command.
///
/// # Example
///
/// ```rust
/// #[visca_method]
/// fn power_on(&self) -> Result<()> {
///     PowerCommand { power: Power::On }
/// }
/// ```
///
/// This generates:
/// - An async version when `feature = "async"` is enabled
/// - A sync version when `feature = "async"` is not enabled
#[proc_macro_attribute]
pub fn visca_method(_attr: TokenStream, item: TokenStream) -> TokenStream {
    let input_fn = parse_macro_input!(item as ItemFn);

    let vis = &input_fn.vis;
    let fn_name = &input_fn.sig.ident;
    let generics = &input_fn.sig.generics;
    let attrs = &input_fn.attrs;
    let block = &input_fn.block;

    // Extract parameters except self
    let params: Vec<_> = input_fn
        .sig
        .inputs
        .iter()
        .skip(1) // Skip self
        .collect();

    // Generate the actual method implementations
    let expanded = quote! {
        #[cfg(feature = "async")]
        #(#attrs)*
        #vis async fn #fn_name #generics(&self #(, #params)*) -> Result<(), crate::Error>
        where
            T: crate::transport::AsyncTransport,
        {
            let command = #block;
            match self.send_command(&command).await? {
                crate::Response::Completion => Ok(()),
                crate::Response::Ack => Ok(()),
                response => Err(crate::Error::InvalidResponse {
                    expected: "Completion".to_string(),
                    actual: format!("{:?}", response).into_bytes(),
                }),
            }
        }

        #[cfg(not(feature = "async"))]
        #(#attrs)*
        #vis fn #fn_name #generics(&mut self #(, #params)*) -> Result<(), crate::Error>
        where
            T: crate::transport::blocking::Transport,
        {
            let command = #block;
            self.send_and_wait(&command)
        }
    };

    TokenStream::from(expanded)
}

/// A macro for defining VISCA command methods with custom logic.
///
/// This variant allows for more complex method implementations that may
/// involve multiple steps or custom processing.
///
/// # Example
///
/// ```rust
/// #[visca_method_custom]
/// fn set_position(&self, pan: impl Into<PanPosition>, tilt: impl Into<TiltPosition>) -> Result<()> {
///     let pan_pos = pan.into();
///     let tilt_pos = tilt.into();
///     PanTiltCommand::AbsolutePosition {
///         pan: pan_pos,
///         tilt: tilt_pos,
///         speed: P::Speed::default(),
///     }
/// }
/// ```
#[proc_macro_attribute]
pub fn visca_method_custom(_attr: TokenStream, item: TokenStream) -> TokenStream {
    let input_fn = parse_macro_input!(item as ItemFn);

    let vis = &input_fn.vis;
    let fn_name = &input_fn.sig.ident;
    let attrs = &input_fn.attrs;
    let block = &input_fn.block;

    // Extract generics from the function signature
    let generics = &input_fn.sig.generics;
    let (impl_generics, _ty_generics, where_clause) = generics.split_for_impl();

    // Build the parameter list for the generated functions
    let params = input_fn.sig.inputs.iter().skip(1).collect::<Vec<_>>();

    // For custom methods, we expect the block to contain all the logic
    // except for the final send_and_wait call. The last expression should
    // be the command to send.

    // Check if there's already a where clause
    let transport_bounds = if where_clause.is_some() {
        // If there's a where clause, we need to merge with it
        quote! {
            T: crate::transport::AsyncTransport,
        }
    } else {
        // If no where clause, create one
        quote! {
            where T: crate::transport::AsyncTransport,
        }
    };

    let blocking_transport_bounds = if where_clause.is_some() {
        quote! {
            T: crate::transport::blocking::Transport,
        }
    } else {
        quote! {
            where T: crate::transport::blocking::Transport,
        }
    };

    let expanded = quote! {
        #[cfg(feature = "async")]
        #(#attrs)*
        #vis async fn #fn_name #impl_generics(&self #(, #params)*) -> Result<(), crate::Error>
        #where_clause
            #transport_bounds
        {
            let command = { #block };
            match self.send_command(&command).await? {
                crate::Response::Completion => Ok(()),
                crate::Response::Ack => Ok(()),
                response => Err(crate::Error::InvalidResponse {
                    expected: "Completion".to_string(),
                    actual: format!("{:?}", response).into_bytes(),
                }),
            }
        }

        #[cfg(not(feature = "async"))]
        #(#attrs)*
        #vis fn #fn_name #impl_generics(&mut self #(, #params)*) -> Result<(), crate::Error>
        #where_clause
            #blocking_transport_bounds
        {
            let command = { #block };
            self.send_and_wait(&command)
        }
    };

    TokenStream::from(expanded)
}

/// A macro for defining VISCA command methods with generic parameters.
///
/// This variant is specifically for methods with generic type parameters
/// and complex trait bounds.
///
/// # Example
///
/// ```rust
/// #[visca_method_generic]
/// fn set_zoom<Z>(&self, position: Z) -> Result<(), crate::Error>
/// where
///     Z: TryInto<ZoomPosition>,
///     Z::Error: Into<crate::Error>,
/// {
///     let position = position.try_into().map_err(Into::into)?;
///     ZoomCommand::Direct(position)
/// }
/// ```
#[proc_macro_attribute]
pub fn visca_method_generic(_attr: TokenStream, item: TokenStream) -> TokenStream {
    let input_fn = parse_macro_input!(item as ItemFn);

    let vis = &input_fn.vis;
    let fn_name = &input_fn.sig.ident;
    let attrs = &input_fn.attrs;
    let block = &input_fn.block;

    // Extract all the signature components
    let generics = &input_fn.sig.generics;
    let params = &input_fn.sig.inputs;
    let output = &input_fn.sig.output;

    // For async version, just copy everything as-is since it already has the right structure
    // For blocking version, we need to change &self to &mut self
    let mut blocking_params = params.clone();
    for (i, param) in blocking_params.iter_mut().enumerate() {
        if i == 0 {
            if let syn::FnArg::Receiver(receiver) = param {
                let mut new_receiver = receiver.clone();
                new_receiver.mutability = Some(syn::token::Mut::default());
                *param = syn::FnArg::Receiver(new_receiver);
            }
        }
    }

    let expanded = quote! {
        #[cfg(feature = "async")]
        #(#attrs)*
        #vis async fn #fn_name #generics(#params) #output
        where
            T: crate::transport::AsyncTransport,
        {
            #block
        }

        #[cfg(not(feature = "async"))]
        #(#attrs)*
        #vis fn #fn_name #generics(#blocking_params) #output
        where
            T: crate::transport::blocking::Transport,
        {
            // For blocking version, we need to transform async calls
            // This is a simplified version - in practice you might need more sophisticated transformation
            let async_block = quote! { #block };
            let block_str = async_block.to_string();

            // Replace .await? with ?
            let transformed = block_str.replace(".await?", "?");
            // Replace .await with empty
            let transformed = transformed.replace(".await", "");
            // Replace send_command with send_and_wait for the final command
            let transformed = transformed.replace("self.send_command", "self.send_and_wait");

            // This is a hack - ideally we'd parse and transform the AST
            // For now, we'll include the original block and let the developer handle it
            #block
        }
    };

    TokenStream::from(expanded)
}

/// A macro for defining inquiry methods that return values from the camera.
///
/// # Example
///
/// ```rust
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
    let _return_type = match output {
        ReturnType::Type(_, ty) => {
            if let Type::Path(type_path) = &**ty {
                if let Some(segment) = type_path.path.segments.last() {
                    if segment.ident == "Result" {
                        // Extract T from Result<T>
                        quote! { #ty }
                    } else {
                        quote! { Result<Response> }
                    }
                } else {
                    quote! { Result<Response> }
                }
            } else {
                quote! { Result<Response> }
            }
        }
        _ => quote! { Result<Response> },
    };

    let expanded = quote! {
        #[cfg(feature = "async")]
        #(#attrs)*
        #vis async fn #fn_name #generics(#inputs) #output {
            let command = #block;
            let response = self.send_and_wait_async(&command).await?;
            // Parse the response based on the expected return type
            // This would need to be customized based on the actual response parsing logic
            todo!("Response parsing for inquiry commands")
        }

        #[cfg(not(feature = "async"))]
        #(#attrs)*
        #vis fn #fn_name #generics(#inputs) #output {
            let command = #block;
            let response = self.send_and_wait(&command)?;
            // Parse the response based on the expected return type
            // This would need to be customized based on the actual response parsing logic
            todo!("Response parsing for inquiry commands")
        }
    };

    TokenStream::from(expanded)
}
