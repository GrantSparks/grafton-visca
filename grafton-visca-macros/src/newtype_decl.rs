// SPDX-License-Identifier: MIT OR Apache-2.0
//
// Copyright (c) 2024 Grafton Machine Shed <team@grafton.ai>

//! Declaration adapter behind `grafton_visca::visca_range_type!` and the
//! main crate's own checked value types.
//!
//! A derive cannot attach derives to the struct it is applied to, and it
//! cannot see the main crate's features. This adapter declares the struct
//! itself, after a declarative macro defined in the main crate has selected
//! the enabled helper features, so the serde, schemars and ts-rs derives are
//! written here once instead of on every type. The checked API comes from the
//! same [`BoundedNewtype`] expansion that `#[derive(ViscaValue)]` uses.
//!
//! Two declaration forms are accepted after the `crate = ...; [features]`
//! prefix:
//!
//! - `#[attrs] Name: Inner { min: expr, max: expr }`, the public
//!   `visca_range_type!` grammar, which also derives the standard traits;
//! - `#[attrs] vis struct Name(Inner);` carrying a `#[visca_value(...)]`
//!   attribute in the derive's grammar.

use std::collections::BTreeSet;

use proc_macro2::{Span, TokenStream};
use quote::{format_ident, quote, ToTokens};
use syn::{
    braced, bracketed, parenthesized,
    parse::{Parse, ParseStream},
    parse_quote,
    punctuated::Punctuated,
    Attribute, Error, Expr, Field, Ident, LitStr, Meta, Path, Result, Token, Visibility,
};

use crate::bounded::{self, parse_value_attributes, BoundedNewtype, Domain};

mod kw {
    syn::custom_keyword!(max);
    syn::custom_keyword!(min);
}

/// The struct declaration the adapter emits.
struct Declaration {
    attrs: Vec<Attribute>,
    vis: Visibility,
    field: Field,
    /// Standard derives the public range grammar promises.
    standard_derives: Option<TokenStream>,
}

struct NewtypeInput {
    crate_path: Path,
    features: BTreeSet<String>,
    declaration: Declaration,
    newtype: BoundedNewtype,
}

impl Parse for NewtypeInput {
    fn parse(input: ParseStream<'_>) -> Result<Self> {
        input.parse::<Token![crate]>()?;
        input.parse::<Token![=]>()?;
        let crate_path = input.parse()?;
        input.parse::<Token![;]>()?;
        let features = parse_features(input)?;

        let attrs = Attribute::parse_outer(input)?;
        let vis: Visibility = input.parse()?;
        let (declaration, newtype) = if input.peek(Token![struct]) {
            parse_item_form(input, attrs, vis)?
        } else if matches!(vis, Visibility::Inherited) {
            parse_range_form(input, attrs)?
        } else {
            return Err(input.error("expected `struct` after the visibility"));
        };

        if !input.is_empty() {
            return Err(input.error("unexpected tokens after the newtype declaration"));
        }

        Ok(Self {
            crate_path,
            features,
            declaration,
            newtype,
        })
    }
}

fn parse_features(input: ParseStream<'_>) -> Result<BTreeSet<String>> {
    let feature_tokens;
    bracketed!(feature_tokens in input);
    let feature_idents = Punctuated::<Ident, Token![,]>::parse_terminated(&feature_tokens)?;
    let mut features = BTreeSet::new();

    for feature in feature_idents {
        let name = feature.to_string();
        if !matches!(name.as_str(), "serde" | "schemars" | "ts_rs") {
            return Err(Error::new(
                feature.span(),
                format!("unknown newtype helper feature `{name}`"),
            ));
        }
        if !features.insert(name.clone()) {
            return Err(Error::new(
                feature.span(),
                format!("duplicate newtype helper feature `{name}`"),
            ));
        }
    }

    if (features.contains("schemars") || features.contains("ts_rs")) && !features.contains("serde")
    {
        return Err(Error::new(
            Span::call_site(),
            "`schemars` and `ts-rs` newtype helpers require `serde`",
        ));
    }
    Ok(features)
}

/// `Name: Inner { min: expr, max: expr }`.
fn parse_range_form(
    input: ParseStream<'_>,
    attrs: Vec<Attribute>,
) -> Result<(Declaration, BoundedNewtype)> {
    let name: Ident = input.parse()?;
    input.parse::<Token![:]>()?;
    let inner = input.parse()?;

    let bounds;
    braced!(bounds in input);
    bounds.parse::<kw::min>()?;
    bounds.parse::<Token![:]>()?;
    let min: Expr = bounds.parse()?;
    bounds.parse::<Token![,]>()?;
    bounds.parse::<kw::max>()?;
    bounds.parse::<Token![:]>()?;
    let max: Expr = bounds.parse()?;
    if bounds.peek(Token![,]) {
        bounds.parse::<Token![,]>()?;
    }
    if !bounds.is_empty() {
        return Err(bounds.error("unexpected tokens after range-type bounds"));
    }

    let field = Field {
        attrs: Vec::new(),
        vis: Visibility::Inherited,
        mutability: syn::FieldMutability::None,
        ident: None,
        colon_token: None,
        ty: inner,
    };
    Ok((
        Declaration {
            attrs,
            vis: parse_quote!(pub),
            standard_derives: Some(quote! {
                #[derive(
                    ::core::fmt::Debug,
                    ::core::marker::Copy,
                    ::core::clone::Clone,
                    ::core::cmp::PartialEq,
                    ::core::cmp::Eq,
                    ::core::cmp::PartialOrd,
                    ::core::cmp::Ord
                )]
            }),
            field: field.clone(),
        },
        BoundedNewtype {
            name,
            inner: field.ty,
            domain: Domain::Range {
                min: min.into_token_stream(),
                max: max.into_token_stream(),
            },
            display: None,
        },
    ))
}

/// `vis struct Name(Inner);` with a `#[visca_value(...)]` attribute.
fn parse_item_form(
    input: ParseStream<'_>,
    attrs: Vec<Attribute>,
    vis: Visibility,
) -> Result<(Declaration, BoundedNewtype)> {
    input.parse::<Token![struct]>()?;
    let name: Ident = input.parse()?;
    let content;
    parenthesized!(content in input);
    let field = Field::parse_unnamed(&content)?;
    if !content.is_empty() {
        return Err(content.error("a checked newtype has exactly one field"));
    }
    input.parse::<Token![;]>()?;

    let values = parse_value_attributes(&attrs, name.span())?;
    let attrs = attrs
        .into_iter()
        .filter(|attr| !attr.path().is_ident(bounded::ATTRIBUTE))
        .collect();
    Ok((
        Declaration {
            attrs,
            vis,
            standard_derives: None,
            field: field.clone(),
        },
        BoundedNewtype {
            name,
            inner: field.ty,
            domain: values.domain,
            display: Some(values.display),
        },
    ))
}

/// Splits a `cfg`, or the `cfg` parts of a `cfg_attr`, from the rest of an
/// attribute: the former must also gate every generated impl.
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
    let NewtypeInput {
        crate_path,
        features,
        declaration,
        newtype,
    } = syn::parse2(input)?;
    let Declaration {
        attrs,
        vis,
        field,
        standard_derives,
    } = declaration;
    let (availability_attrs, remaining_attrs) = partition_attrs(&attrs)?;
    let name = &newtype.name;
    let inner = &newtype.inner;
    let helper_module = format_ident!(
        "__grafton_visca_newtype_support_{}",
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
        let reexports = features.iter().map(|feature| {
            let feature = Ident::new(feature, Span::call_site());
            quote!(pub use #crate_path::__macro_support::#feature;)
        });
        quote! {
            #(#availability_attrs)*
            #[doc(hidden)]
            #[allow(non_snake_case)]
            mod #helper_module {
                #(#reexports)*
            }
        }
    });

    let api = newtype.expand(&crate_path.to_token_stream(), &availability_attrs);

    Ok(quote! {
        #helper_module
        #(#availability_attrs)*
        #standard_derives
        #serde
        #schemars
        #ts_rs
        #(#remaining_attrs)*
        #vis struct #name(#field);

        #api
    })
}
