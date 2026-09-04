//! No-op derives used to prove exported declarative macros do not resolve
//! built-in derives from the downstream invocation site.

use proc_macro::TokenStream;

#[proc_macro_derive(Debug)]
pub fn debug(_input: TokenStream) -> TokenStream {
    TokenStream::new()
}

#[proc_macro_derive(Copy)]
pub fn copy(_input: TokenStream) -> TokenStream {
    TokenStream::new()
}

#[proc_macro_derive(Clone)]
pub fn clone(_input: TokenStream) -> TokenStream {
    TokenStream::new()
}

#[proc_macro_derive(PartialEq)]
pub fn partial_eq(_input: TokenStream) -> TokenStream {
    TokenStream::new()
}

#[proc_macro_derive(Eq)]
pub fn eq(_input: TokenStream) -> TokenStream {
    TokenStream::new()
}

#[proc_macro_derive(PartialOrd)]
pub fn partial_ord(_input: TokenStream) -> TokenStream {
    TokenStream::new()
}

#[proc_macro_derive(Ord)]
pub fn ord(_input: TokenStream) -> TokenStream {
    TokenStream::new()
}
