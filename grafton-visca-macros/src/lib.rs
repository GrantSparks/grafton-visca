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
            T: crate::transport::blocking::BlockingTransport,
        {
            let command = #block;
            self.send_and_wait(&command)
        }
    };

    TokenStream::from(expanded)
}

#[proc_macro_attribute]
pub fn visca_method_custom(_attr: TokenStream, item: TokenStream) -> TokenStream {
    // This macro is for methods that need custom command execution
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
            T: crate::transport::blocking::BlockingTransport,
        {
            let command = #block;
            self.send_and_wait(&command)
        }
    };

    TokenStream::from(expanded)
}

#[proc_macro_attribute]
pub fn visca_camera_method(attr: TokenStream, item: TokenStream) -> TokenStream {
    let input_fn = parse_macro_input!(item as ItemFn);
    // Parse attributes as a comma-separated list
    let attr_str = attr.to_string();

    let vis = &input_fn.vis;
    let fn_name = &input_fn.sig.ident;
    let attrs = &input_fn.attrs;
    let block = &input_fn.block;

    // Extract generics and parameters
    let generics = &input_fn.sig.generics;
    let (impl_generics, _ty_generics, where_clause) = generics.split_for_impl();

    // Get all parameters except self
    let params: Vec<_> = input_fn.sig.inputs.iter().skip(1).collect();

    // Check if the method returns a Result or a command directly
    let _returns_result = if let ReturnType::Type(_, ty) = &input_fn.sig.output {
        if let Type::Path(type_path) = &**ty {
            type_path
                .path
                .segments
                .first()
                .map(|seg| seg.ident == "Result")
                .unwrap_or(false)
        } else {
            false
        }
    } else {
        false
    };

    // For visca_camera_method, we always expect the body to return a Command
    // Any errors should be handled with ? within the body
    let command_execution = quote! {
        let command = { #block };
    };

    // Check for blocking_only or async_only attributes
    let mut blocking_only = false;
    let mut async_only = false;

    if attr_str.contains("blocking_only") {
        blocking_only = true;
    }
    if attr_str.contains("async_only") {
        async_only = true;
    }

    let async_impl = if !blocking_only {
        quote! {
            #[cfg(feature = "async")]
            #(#attrs)*
            #vis async fn #fn_name #impl_generics(&self #(, #params)*) -> Result<(), crate::Error>
            where
                T: crate::transport::AsyncTransport,
                #where_clause
            {
                #command_execution
                match self.send_command(&command).await? {
                    crate::Response::Completion => Ok(()),
                    crate::Response::Ack => Ok(()),
                    response => Err(crate::Error::InvalidResponse {
                        expected: "Completion".to_string(),
                        actual: format!("{:?}", response).into_bytes(),
                    }),
                }
            }
        }
    } else {
        quote! {}
    };

    let blocking_impl = if !async_only {
        quote! {
            #[cfg(not(feature = "async"))]
            #(#attrs)*
            #vis fn #fn_name #impl_generics(&mut self #(, #params)*) -> Result<(), crate::Error>
            where
                T: crate::transport::blocking::BlockingTransport,
                #where_clause
            {
                #command_execution
                self.send_and_wait(&command)
            }
        }
    } else {
        quote! {}
    };

    let expanded = quote! {
        #async_impl
        #blocking_impl
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
/// ```rust
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

    for variant in variants {
        // Parse each variant definition
        // Format: method_name(param: Type) -> "generated_name"
        if let Some((signature, generated_name)) = variant.split_once("->") {
            let signature = signature.trim();
            let generated_name = generated_name.trim().trim_matches('"');

            // Parse the method signature
            if let Some((_method_base, params)) = signature.split_once('(') {
                let params = params.trim_end_matches(')');

                // Generate a method that converts the input and calls the impl
                let method_ident = syn::Ident::new(generated_name, proc_macro2::Span::call_site());

                generated_methods.push(quote! {
                    #[visca_command]
                    pub fn #method_ident(&self, #params) -> Result<(), crate::Error> {
                        // Convert input to the expected type and call the impl
                        todo!("Implement conversion logic")
                    }
                });
            }
        }
    }

    let expanded = quote! {
        #input_fn

        #(#generated_methods)*
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
            T: crate::transport::blocking::BlockingTransport,
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
