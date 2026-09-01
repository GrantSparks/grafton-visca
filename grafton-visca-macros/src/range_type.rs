//! Expansion adapter for the exported `visca_range_type!` macro.

use std::collections::BTreeSet;

use proc_macro2::{Span, TokenStream};
use quote::{format_ident, quote};
use syn::{
    braced, bracketed,
    parse::{Parse, ParseStream},
    parse_quote,
    punctuated::Punctuated,
    Attribute, Error, Expr, Ident, LitStr, Meta, Path, Result, Token, Type,
};

mod kw {
    syn::custom_keyword!(max);
    syn::custom_keyword!(min);
}

struct RangeTypeInput {
    crate_path: Path,
    features: BTreeSet<String>,
    attrs: Vec<Attribute>,
    name: Ident,
    inner: Type,
    min: Expr,
    max: Expr,
}

impl Parse for RangeTypeInput {
    fn parse(input: ParseStream<'_>) -> Result<Self> {
        input.parse::<Token![crate]>()?;
        input.parse::<Token![=]>()?;
        let crate_path = input.parse()?;
        input.parse::<Token![;]>()?;

        let feature_tokens;
        bracketed!(feature_tokens in input);
        let feature_idents = Punctuated::<Ident, Token![,]>::parse_terminated(&feature_tokens)?;
        let mut features = BTreeSet::new();

        for feature in feature_idents {
            let name = feature.to_string();
            if !matches!(name.as_str(), "serde" | "schemars" | "ts_rs") {
                return Err(Error::new(
                    feature.span(),
                    format!("unknown range-type helper feature `{name}`"),
                ));
            }
            if !features.insert(name.clone()) {
                return Err(Error::new(
                    feature.span(),
                    format!("duplicate range-type helper feature `{name}`"),
                ));
            }
        }

        if (features.contains("schemars") || features.contains("ts_rs"))
            && !features.contains("serde")
        {
            return Err(Error::new(
                Span::call_site(),
                "`schemars` and `ts-rs` range helpers require `serde`",
            ));
        }

        let attrs = Attribute::parse_outer(input)?;
        let name = input.parse()?;
        input.parse::<Token![:]>()?;
        let inner = input.parse()?;

        let bounds;
        braced!(bounds in input);
        bounds.parse::<kw::min>()?;
        bounds.parse::<Token![:]>()?;
        let min = bounds.parse()?;
        bounds.parse::<Token![,]>()?;
        bounds.parse::<kw::max>()?;
        bounds.parse::<Token![:]>()?;
        let max = bounds.parse()?;
        if bounds.peek(Token![,]) {
            bounds.parse::<Token![,]>()?;
        }
        if !bounds.is_empty() {
            return Err(bounds.error("unexpected tokens after range-type bounds"));
        }

        if !input.is_empty() {
            return Err(input.error("unexpected tokens after range-type declaration"));
        }

        Ok(Self {
            crate_path,
            features,
            attrs,
            name,
            inner,
            min,
            max,
        })
    }
}

fn partition_meta(meta: &Meta) -> Result<(Option<Meta>, Option<Meta>)> {
    if meta.path().is_ident("cfg") {
        return Ok((Some(meta.clone()), None));
    }
    if !meta.path().is_ident("cfg_attr") {
        return Ok((None, Some(meta.clone())));
    }

    let Meta::List(list) = meta else {
        return Ok((None, Some(meta.clone())));
    };
    let entries = list.parse_args_with(Punctuated::<Meta, Token![,]>::parse_terminated)?;
    let mut entries = entries.into_iter();
    let Some(predicate) = entries.next() else {
        return Ok((None, Some(meta.clone())));
    };
    let mut availability = Vec::new();
    let mut remaining = Vec::new();
    for nested in entries {
        let (nested_availability, nested_remaining) = partition_meta(&nested)?;
        availability.extend(nested_availability);
        remaining.extend(nested_remaining);
    }

    if availability.is_empty() && remaining.is_empty() {
        return Ok((None, Some(meta.clone())));
    }

    let availability =
        (!availability.is_empty()).then(|| parse_quote!(cfg_attr(#predicate, #(#availability),*)));
    let remaining =
        (!remaining.is_empty()).then(|| parse_quote!(cfg_attr(#predicate, #(#remaining),*)));
    Ok((availability, remaining))
}

fn partition_attrs(attrs: &[Attribute]) -> Result<(Vec<Attribute>, Vec<Attribute>)> {
    let mut availability = Vec::new();
    let mut remaining = Vec::new();
    for attr in attrs {
        let (availability_meta, remaining_meta) = partition_meta(&attr.meta)?;
        if let Some(meta) = availability_meta {
            availability.push(parse_quote!(#[#meta]));
        }
        if let Some(meta) = remaining_meta {
            remaining.push(parse_quote!(#[#meta]));
        }
    }
    Ok((availability, remaining))
}

pub(crate) fn expand(input: TokenStream) -> Result<TokenStream> {
    let RangeTypeInput {
        crate_path,
        features,
        attrs,
        name,
        inner,
        min,
        max,
    } = syn::parse2(input)?;
    let (availability_attrs, remaining_attrs) = partition_attrs(&attrs)?;
    let helper_module = format_ident!(
        "__grafton_visca_range_type_support_{}",
        name.to_string().trim_start_matches("r#"),
        span = name.span()
    );

    let serde = features.contains("serde").then(|| {
        let helper_path = LitStr::new(&format!("{helper_module}::serde"), Span::call_site());
        let inner_type = LitStr::new(&quote!(#inner).to_string(), Span::call_site());
        quote! {
            #[derive(
                #crate_path::__macro_support::serde::Serialize,
                #crate_path::__macro_support::serde::Deserialize
            )]
            #[serde(
                crate = #helper_path,
                try_from = #inner_type,
                into = #inner_type
            )]
        }
    });
    let schemars = features.contains("schemars").then(|| {
        let helper_path = LitStr::new(&format!("{helper_module}::schemars"), Span::call_site());
        quote! {
            #[derive(#crate_path::__macro_support::schemars::JsonSchema)]
            #[schemars(crate = #helper_path, !try_from, !into)]
        }
    });
    let ts_rs = features.contains("ts_rs").then(|| {
        let helper_path = LitStr::new(&format!("{helper_module}::ts_rs"), Span::call_site());
        quote! {
            #[derive(#crate_path::__macro_support::ts_rs::TS)]
            #[ts(crate = #helper_path, export)]
        }
    });
    let helper_module = (!features.is_empty()).then(|| {
        let serde_reexport = features
            .contains("serde")
            .then(|| quote!(pub use #crate_path::__macro_support::serde;));
        let schemars_reexport = features
            .contains("schemars")
            .then(|| quote!(pub use #crate_path::__macro_support::schemars;));
        let ts_rs_reexport = features
            .contains("ts_rs")
            .then(|| quote!(pub use #crate_path::__macro_support::ts_rs;));

        quote! {
            #(#availability_attrs)*
            #[doc(hidden)]
            #[allow(non_snake_case)]
            mod #helper_module {
                #serde_reexport
                #schemars_reexport
                #ts_rs_reexport
            }
        }
    });

    Ok(quote! {
        #helper_module
        #(#availability_attrs)*
        #[derive(
            ::core::fmt::Debug,
            ::core::marker::Copy,
            ::core::clone::Clone,
            ::core::cmp::PartialEq,
            ::core::cmp::Eq,
            ::core::cmp::PartialOrd,
            ::core::cmp::Ord
        )]
        #serde
        #schemars
        #ts_rs
        #(#remaining_attrs)*
        pub struct #name(#inner);

        #(#availability_attrs)*
        impl #name {
            /// Minimum allowed value
            pub const MIN: #inner = #min;
            /// Maximum allowed value
            pub const MAX: #inner = #max;

            /// Create a new instance with validation
            pub fn new(
                value: #inner,
            ) -> ::core::result::Result<Self, #crate_path::Error> {
                if !(Self::MIN..=Self::MAX).contains(&value) {
                    return ::core::result::Result::Err(#crate_path::Error::InvalidParameter {
                        parameter: ::core::stringify!(#name),
                        value: ::std::borrow::Cow::Owned(::std::format!("{value}")),
                        reason: {
                            let min = Self::MIN;
                            let max = Self::MAX;
                            ::std::borrow::Cow::Owned(::std::format!(
                                "must be between {min} and {max}"
                            ))
                        },
                    });
                }
                ::core::result::Result::Ok(Self(value))
            }

            /// Get the inner value
            #[must_use]
            pub fn value(&self) -> #inner {
                self.0
            }
        }

        #(#availability_attrs)*
        impl ::core::convert::TryFrom<#inner> for #name {
            type Error = #crate_path::Error;

            fn try_from(
                value: #inner,
            ) -> ::core::result::Result<Self, Self::Error> {
                Self::new(value)
            }
        }

        #(#availability_attrs)*
        impl ::core::convert::From<#name> for #inner {
            fn from(val: #name) -> Self {
                val.0
            }
        }
    })
}
