//! Resolves the main crate under its downstream dependency name.

use proc_macro2::{Ident, Span, TokenStream};
use proc_macro_crate::{crate_name, FoundCrate};
use quote::quote;

pub(crate) fn grafton_visca() -> TokenStream {
    match crate_name("grafton-visca") {
        // Rustdoc compiles examples as an external crate while retaining the
        // package manifest. A canonical absolute path works in both contexts;
        // the defining library supplies a private `extern crate self` alias.
        Ok(FoundCrate::Itself) => quote!(::grafton_visca),
        Ok(FoundCrate::Name(name)) => {
            let ident = Ident::new(&name, Span::call_site());
            quote!(::#ident)
        }
        Err(error) => {
            let message = format!(
                "could not resolve the `grafton-visca` dependency for derive output: {error}"
            );
            quote!(compile_error!(#message))
        }
    }
}
