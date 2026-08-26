//! Resolves the main crate under its downstream dependency name.

use proc_macro2::{Ident, Span, TokenStream};
use proc_macro_crate::{crate_name, FoundCrate};
use quote::quote;

pub(crate) fn grafton_visca() -> TokenStream {
    match crate_name("grafton-visca") {
        Ok(FoundCrate::Itself) => quote!(crate),
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
