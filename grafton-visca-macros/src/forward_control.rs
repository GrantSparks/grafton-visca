// SPDX-License-Identifier: MIT OR Apache-2.0
//
// Copyright (c) 2024 Grafton Machine Shed <team@grafton.ai>

//! Procedural macro for auto-generating CameraSession forwarding implementations

use proc_macro2::TokenStream;
use quote::quote;
use syn::{FnArg, Pat, TraitItem};

/// Parse the trait and generate both async and blocking forwarding implementations
pub fn delegate_to_session_impl(input: syn::ItemTrait) -> TokenStream {
    let trait_name = &input.ident;

    // Parse trait methods
    let methods: Vec<_> = input
        .items
        .iter()
        .filter_map(|item| match item {
            TraitItem::Fn(method) => Some(method),
            _ => None,
        })
        .collect();

    // Generate forwarding methods for both async and blocking
    let async_methods = generate_forwarding_methods(&methods);
    let blocking_methods = generate_forwarding_methods(&methods);

    // Build the full implementation - now with Open state constraint
    quote! {
        #input

        #[cfg(feature = "mode-async")]
        impl<M, P, Tr, Exec> #trait_name for crate::camera::CameraSession<M, P, Tr, Exec, crate::camera::session::Open>
        where
            M: crate::mode::Mode,
            P: crate::capabilities::Profile,
            Exec: crate::executor::Executor,
            crate::camera::Camera<M, P, Tr, Exec>: #trait_name<Mode = M>,
        {
            type Mode = M;

            #async_methods
        }

        #[cfg(not(feature = "mode-async"))]
        impl<P, Tr> #trait_name for crate::camera::CameraSession<crate::mode::Blocking, P, Tr, (), crate::camera::session::Open>
        where
            P: crate::capabilities::Profile,
            Tr: crate::transport::BlockingTransport
                + crate::transport::HasTransportConfig
                + Send
                + 'static,
            crate::camera::Camera<crate::mode::Blocking, P, Tr, ()>: #trait_name<Mode = crate::mode::Blocking>,
        {
            type Mode = crate::mode::Blocking;

            #blocking_methods
        }
    }
}

/// Generate forwarding method implementations
fn generate_forwarding_methods(methods: &[&syn::TraitItemFn]) -> TokenStream {
    let mut generated_methods = Vec::new();

    for method in methods {
        let sig = &method.sig;
        let method_name = &sig.ident;
        let method_attrs = &method.attrs;

        // Skip associated type definitions (type Mode = ...)
        if method_name == "Mode" {
            continue;
        }

        // Extract parameter names for forwarding
        let param_names: Vec<_> = sig
            .inputs
            .iter()
            .filter_map(|arg| match arg {
                FnArg::Receiver(_) => None,
                FnArg::Typed(pat_type) => {
                    if let Pat::Ident(ident) = &*pat_type.pat {
                        Some(&ident.ident)
                    } else {
                        None
                    }
                }
            })
            .collect();

        // Determine the receiver type and generate appropriate forwarding
        // Now that we use typestate pattern, camera() and camera_mut() return direct references
        let forwarding_call = match sig.inputs.first() {
            Some(FnArg::Receiver(receiver)) => {
                if receiver.mutability.is_some() {
                    // &mut self
                    quote! {
                        self.camera_mut().#method_name(#(#param_names),*)
                    }
                } else {
                    // &self
                    quote! {
                        self.camera().#method_name(#(#param_names),*)
                    }
                }
            }
            _ => {
                // No receiver (associated function) - shouldn't happen in control traits
                continue;
            }
        };

        // Generate the full method with attributes (no more expect_used needed)
        generated_methods.push(quote! {
            #(#method_attrs)*
            #[inline]
            #sig {
                #forwarding_call
            }
        });
    }

    quote! {
        #(#generated_methods)*
    }
}
