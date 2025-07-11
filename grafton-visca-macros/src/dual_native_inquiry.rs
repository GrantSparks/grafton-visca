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
use std::collections::HashSet;
use syn::{
    fold::{self, Fold},
    parse::{Parse, ParseStream},
    parse_macro_input,
    punctuated::Punctuated,
    FnArg, Ident, ItemFn, Receiver, Token,
};

/// Names of methods that must be awaited in async builds.
const ASYNC_METHODS: &[&str] = &[
    "send_raw",
    "send_and_wait",
    "send_and_receive",
    "send_const",
    "send_array",
    "send_blocking",
    "send_async",
];

/// Attribute arguments for #[dual_native_inquiry(...)]
#[derive(Default)]
struct DualNativeInquiryArgs {
    await_methods: HashSet<String>,
}

impl Parse for DualNativeInquiryArgs {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let mut args = Self::default();

        if input.is_empty() {
            return Ok(args);
        }

        // Parse "await_methods(...)"
        let ident: Ident = input.parse()?;
        if ident != "await_methods" {
            return Err(syn::Error::new(ident.span(), "expected 'await_methods'"));
        }

        let content;
        syn::parenthesized!(content in input);

        // Parse comma-separated list of method names
        let methods: Punctuated<Ident, Token![,]> =
            content.parse_terminated(Ident::parse, Token![,])?;

        for method in methods {
            args.await_methods.insert(method.to_string());
        }

        Ok(args)
    }
}

/// Fold the AST, inserting `.await` after certain method calls.
struct Awaitify {
    await_methods: HashSet<String>,
}

impl Fold for Awaitify {
    fn fold_expr(&mut self, expr: syn::Expr) -> syn::Expr {
        match expr {
            syn::Expr::MethodCall(mc) => {
                let mc = fold::fold_expr_method_call(self, mc);

                // Check if this is a method call on self
                let is_self_method = match &*mc.receiver {
                    syn::Expr::Path(path) => {
                        path.path.segments.len() == 1 && path.path.segments[0].ident == "self"
                    }
                    _ => false,
                };

                // If the method is one that becomes async, attach `.await`
                let method_name = mc.method.to_string();
                let needs_await = ASYNC_METHODS.iter().any(|name| name == &method_name)
                    || (is_self_method && self.await_methods.contains(&method_name));

                if needs_await {
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
            // Handle try expressions (e.g., method_call()?)
            syn::Expr::Try(try_expr) => {
                // First fold the inner expression
                let inner = self.fold_expr(*try_expr.expr);

                // Check if the inner expression became an await expression
                if matches!(inner, syn::Expr::Await(_)) {
                    // If so, wrap the await in a try expression
                    syn::Expr::Try(syn::ExprTry {
                        attrs: try_expr.attrs,
                        expr: Box::new(inner),
                        question_token: try_expr.question_token,
                    })
                } else {
                    // Otherwise, just update the inner expression
                    syn::Expr::Try(syn::ExprTry {
                        attrs: try_expr.attrs,
                        expr: Box::new(inner),
                        question_token: try_expr.question_token,
                    })
                }
            }
            other => fold::fold_expr(self, other),
        }
    }
}

pub fn dual_native_inquiry(attr: TokenStream, item: TokenStream) -> TokenStream {
    // --- Parse the attribute arguments -------------------------------------------------------
    let args = parse_macro_input!(attr as DualNativeInquiryArgs);

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
                blocking_inputs.push(syn::parse_quote!(&mut self));
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
                async_inputs.push(syn::parse_quote!(&self));
            }
            _ => async_inputs.push(arg.clone()),
        }
    }

    // Transform the original block to insert `.await`
    let mut awaitifier = Awaitify {
        await_methods: args.await_methods,
    };
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
