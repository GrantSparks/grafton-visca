// grafton-visca-macros/src/dual_native_inquiry.rs
// SPDX-License-Identifier: Apache-2.0
//
// Generate *two* implementations from a single VISCA inquiry method:
//
//  * blocking version   – compiled when `async` feature is **disabled**
//  * async version      – compiled when `async` feature is **enabled**
//
// The developer writes the method once, in a blocking style:
//
//     #[dual_native_inquiry]
//     fn get_zoom_position(&mut self) -> Result<u16, Error> {
//         let cmd = InquiryCommand::ZoomPosition;
//         let bytes = cmd.to_bytes()?;
//         let response = self.send_raw(&bytes)?;
//         let parsed = Response::parse(&response)?;
//         match parsed {
//             Response::InquiryResponse(InquiryResponse::ZoomPosition{ position }) => Ok(position),
//             _ => Err(Error::UnexpectedResponseType),
//         }
//     }
//
// The macro expands to the pair of cfg-gated implementations automatically.

use proc_macro::TokenStream;
use quote::quote;
use syn::{
    fold::{self, Fold},
    parse_macro_input, FnArg, ItemFn, Receiver,
};

/// Names of methods that must be awaited in async builds.
const ASYNC_METHODS: &[&str] = &["send_raw", "send_and_wait", "send_and_receive"];

/// Fold the AST, inserting `.await` after certain method calls.
struct Awaitify;

impl Fold for Awaitify {
    fn fold_expr(&mut self, expr: syn::Expr) -> syn::Expr {
        match expr {
            syn::Expr::MethodCall(mc) => {
                let mc = fold::fold_expr_method_call(self, mc);
                // If the method is one that becomes async, attach `.await`
                if ASYNC_METHODS
                    .iter()
                    .any(|name| name == &mc.method.to_string())
                {
                    syn::Expr::Await(syn::ExprAwait {
                        attrs: Vec::new(),
                        base: Box::new(syn::Expr::MethodCall(mc)),
                        dot_token: <syn::token::Dot>::default(),
                        await_token: <syn::token::Await>::default(),
                    })
                } else {
                    syn::Expr::MethodCall(mc)
                }
            }
            other => fold::fold_expr(self, other),
        }
    }
}

pub fn dual_native_inquiry(_attr: TokenStream, item: TokenStream) -> TokenStream {
    // --- Parse the original method -----------------------------------------------------------
    let input_fn = parse_macro_input!(item as ItemFn);
    let vis = &input_fn.vis;
    let attrs = &input_fn.attrs;
    let sig = &input_fn.sig;
    let fn_ident = &sig.ident;
    let generics = &sig.generics;
    let output = &sig.output;
    let orig_inputs = &sig.inputs;
    let orig_block = &input_fn.block;

    // --- Blocking variant --------------------------------------------------------------------
    // Make sure receiver is `&mut self` (if the author wrote &self we leave it unchanged
    // because blocking builds also accept &self methods that do not mutate internal state).
    let mut blocking_inputs: syn::punctuated::Punctuated<FnArg, syn::token::Comma> =
        syn::punctuated::Punctuated::new();
    for arg in orig_inputs.iter() {
        match arg {
            FnArg::Receiver(Receiver {
                reference: Some(_),
                mutability: None,
                ..
            }) => {
                // convert `&self` to `&mut self`
                blocking_inputs.push(syn::parse_quote!( &mut self ));
            }
            _ => blocking_inputs.push(arg.clone()),
        }
    }

    let blocking_fn = quote! {
        #[cfg(not(feature = "async"))]
        #(#attrs)*
        #vis fn #fn_ident #generics(#blocking_inputs) #output {
            #orig_block
        }
    };

    // --- Async variant -----------------------------------------------------------------------
    // 1. Replace `&mut self` with `&self`
    // 2. Mark the function as `async`
    // 3. Add `.await` after calls that need it.
    let mut async_inputs: syn::punctuated::Punctuated<FnArg, syn::token::Comma> =
        syn::punctuated::Punctuated::new();
    for arg in orig_inputs.iter() {
        match arg {
            FnArg::Receiver(Receiver {
                reference: Some(_),
                mutability: Some(_),
                ..
            }) => {
                // convert `&mut self` to `&self`
                async_inputs.push(syn::parse_quote!( &self ));
            }
            _ => async_inputs.push(arg.clone()),
        }
    }

    // Transform the original block to insert `.await`
    let mut awaitifier = Awaitify;
    let async_block = awaitifier.fold_block(*orig_block.clone());

    let async_fn = quote! {
        #[cfg(feature = "async")]
        #(#attrs)*
        #vis async fn #fn_ident #generics(#async_inputs) #output {
            #async_block
        }
    };

    // --- Return combined token stream --------------------------------------------------------
    TokenStream::from(quote! {
        #blocking_fn
        #async_fn
    })
}