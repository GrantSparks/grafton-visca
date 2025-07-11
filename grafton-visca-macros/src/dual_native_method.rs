// grafton-visca-macros/src/dual_native_method.rs
// SPDX-License-Identifier: Apache-2.0
//
// Generate *two* implementations from a single VISCA method:
//
//  * blocking version   – compiled when `async` feature is **disabled**
//  * async version      – compiled when `async` feature is **enabled**
//
// The developer writes the method once, in a blocking style:
//
//     #[dual_native_method]
//     fn pan_tilt_stop(&mut self) -> Result<(), Error> {
//         self.send_const(pan_tilt::STOP)
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
    "send_command",
];

/// Attribute arguments for #[dual_native_method(...)]
#[derive(Default)]
struct DualNativeMethodArgs {
    await_methods: HashSet<String>,
}

impl Parse for DualNativeMethodArgs {
    fn parse(input: ParseStream) -> syn::Result<Self> {
        let mut args = Self::default();

        // Parse await_methods(method1, method2, ...)
        if input.peek(Ident) {
            let ident: Ident = input.parse()?;
            if ident == "await_methods" {
                let content;
                let _paren = syn::parenthesized!(content in input);
                let methods: Punctuated<Ident, Token![,]> =
                    content.parse_terminated(Ident::parse, Token![,])?;

                for method in methods {
                    args.await_methods.insert(method.to_string());
                }
            } else {
                return Err(syn::Error::new(ident.span(), "expected `await_methods`"));
            }
        }

        Ok(args)
    }
}

/// Fold the AST, inserting `.await` after certain method calls.
struct Awaitify {
    /// Methods to await (from attribute args)
    custom_methods: HashSet<String>,
}

impl Fold for Awaitify {
    fn fold_expr(&mut self, expr: syn::Expr) -> syn::Expr {
        match expr {
            // Handle try expressions that contain method calls
            syn::Expr::Try(try_expr) => {
                let inner = self.fold_expr(*try_expr.expr);

                // Check if the inner expression is a method call that needs awaiting
                if let syn::Expr::MethodCall(ref mc) = inner {
                    let method_name = mc.method.to_string();
                    let needs_await = ASYNC_METHODS.iter().any(|name| name == &method_name)
                        || self.custom_methods.contains(&method_name);

                    // Check if receiver is self
                    let is_self_call = matches!(
                        &*mc.receiver,
                        syn::Expr::Path(path) if path.path.segments.len() == 1
                            && path.path.segments[0].ident == "self"
                    );

                    if needs_await || (is_self_call && self.custom_methods.contains(&method_name)) {
                        // Create await expression
                        let await_expr = syn::Expr::Await(syn::ExprAwait {
                            attrs: Vec::new(),
                            base: Box::new(inner),
                            dot_token: Default::default(),
                            await_token: Default::default(),
                        });

                        // Re-wrap with try
                        return syn::Expr::Try(syn::ExprTry {
                            attrs: try_expr.attrs,
                            expr: Box::new(await_expr),
                            question_token: try_expr.question_token,
                        });
                    }
                }

                // Re-wrap with try if no await needed
                syn::Expr::Try(syn::ExprTry {
                    attrs: try_expr.attrs,
                    expr: Box::new(inner),
                    question_token: try_expr.question_token,
                })
            }

            // Handle regular method calls
            syn::Expr::MethodCall(mc) => {
                let mc = fold::fold_expr_method_call(self, mc);
                let method_name = mc.method.to_string();

                // Check if this is a method that needs awaiting
                let needs_await = ASYNC_METHODS.iter().any(|name| name == &method_name)
                    || self.custom_methods.contains(&method_name);

                // Check if receiver is self
                let is_self_call = matches!(
                    &*mc.receiver,
                    syn::Expr::Path(path) if path.path.segments.len() == 1
                        && path.path.segments[0].ident == "self"
                );

                if needs_await || (is_self_call && self.custom_methods.contains(&method_name)) {
                    syn::Expr::Await(syn::ExprAwait {
                        attrs: Vec::new(),
                        base: Box::new(syn::Expr::MethodCall(mc)),
                        dot_token: Default::default(),
                        await_token: Default::default(),
                    })
                } else {
                    syn::Expr::MethodCall(mc)
                }
            }
            other => fold::fold_expr(self, other),
        }
    }
}

pub fn dual_native_method(attr: TokenStream, item: TokenStream) -> TokenStream {
    // Parse the attribute arguments
    let args = parse_macro_input!(attr as DualNativeMethodArgs);

    // Parse the original method
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
    // Keep the original receiver type (usually &mut self for command methods)
    let blocking_fn = quote! {
        #[cfg(not(feature = "async"))]
        #(#attrs)*
        #vis fn #fn_ident #generics(#orig_inputs) #output {
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
        custom_methods: args.await_methods,
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
