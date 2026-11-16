//! The single expansion pipeline for scalar and enum value attributes.

use std::collections::{BTreeMap, BTreeSet};

use proc_macro2::{Span, TokenStream};
use quote::{format_ident, quote};
use syn::{
    bracketed,
    fold::Fold,
    parenthesized,
    parse::{Parse, ParseStream, Parser},
    parse_quote,
    punctuated::Punctuated,
    spanned::Spanned,
    Attribute, Data, DeriveInput, Error, Expr, ExprArray, Fields, Ident, Lit, LitStr, Meta, Path,
    Result, Token, Type,
};

use crate::attr::{unknown_key, AttrSlot};

#[derive(Clone, Copy)]
pub(crate) enum Kind {
    Scalar,
    Enum,
}

enum Display {
    Format(LitStr),
    Function(Path),
    None,
}

#[derive(Default)]
struct ScalarArgs {
    crate_path: Option<Path>,
    min: Option<Expr>,
    max: Option<Expr>,
    values: Option<ExprArray>,
    display: Option<Display>,
    bias: Option<(Expr, Type)>,
    error: Option<Expr>,
}

impl Parse for ScalarArgs {
    fn parse(input: ParseStream<'_>) -> Result<Self> {
        let mut crate_path = AttrSlot::default();
        let mut min = AttrSlot::default();
        let mut max = AttrSlot::default();
        let mut values = AttrSlot::default();
        let mut display = AttrSlot::default();
        let mut bias = AttrSlot::default();
        let mut error = AttrSlot::default();
        let parser = syn::meta::parser(|meta| {
            if meta.path.is_ident("crate") {
                crate_path.set("value", &meta, meta.value()?.parse()?)
            } else if meta.path.is_ident("min") {
                min.set("value", &meta, meta.value()?.parse()?)
            } else if meta.path.is_ident("max") {
                max.set("value", &meta, meta.value()?.parse()?)
            } else if meta.path.is_ident("valid_values") {
                let array: ExprArray = meta.value()?.parse()?;
                if array.elems.is_empty() {
                    return Err(meta.error("`valid_values` must be a non-empty array literal"));
                }
                values.set("value", &meta, array)
            } else if meta.path.is_ident("display") {
                display.set("value", &meta, parse_display(meta.value()?)?)
            } else if meta.path.is_ident("wire") {
                let value = meta.value()?;
                let function: Ident = value.parse()?;
                if function != "biased" && function != "centered" {
                    return Err(Error::new(
                        function.span(),
                        "`wire` expects `biased(offset, type)` or `centered(offset, type)`",
                    ));
                }
                let arguments;
                parenthesized!(arguments in value);
                let offset = integer_expression(arguments.parse()?)?;
                let wire = if arguments.peek(Token![,]) {
                    arguments.parse::<Token![,]>()?;
                    arguments.parse::<Type>()?
                } else {
                    parse_quote!(u8)
                };
                if !arguments.is_empty() {
                    return Err(arguments.error("unexpected tokens in wire representation"));
                }
                require_integer(&wire, false)?;
                bias.set("value", &meta, (offset, wire))
            } else if meta.path.is_ident("error") {
                error.set("value", &meta, meta.value()?.parse()?)
            } else {
                Err(unknown_key(
                    &meta,
                    "value",
                    "`min`, `max`, `valid_values`, `display`, `wire`, `error`, or `crate`",
                ))
            }
        });
        parser.parse2(input.parse()?)?;
        let result = Self {
            crate_path: crate_path.into_value(),
            min: min.into_value(),
            max: max.into_value(),
            values: values.into_value(),
            display: display.into_value(),
            bias: bias.into_value(),
            error: error.into_value(),
        };
        match (&result.min, &result.max, &result.values) {
            (Some(_), Some(_), None) | (None, None, Some(_)) => Ok(result),
            (_, _, Some(_)) => {
                Err(input.error("`valid_values` cannot be combined with `min` or `max`"))
            }
            (Some(_), None, None) => {
                Err(input.error("`min` requires `max`; specify both bounds or use `valid_values`"))
            }
            (None, Some(_), None) => {
                Err(input.error("`max` requires `min`; specify both bounds or use `valid_values`"))
            }
            (None, None, None) => {
                Err(input.error("`value` requires `min` and `max`, or `valid_values`"))
            }
        }
    }
}

#[derive(Default)]
struct EnumArgs {
    crate_path: Option<Path>,
    wire: Option<Type>,
}

impl Parse for EnumArgs {
    fn parse(input: ParseStream<'_>) -> Result<Self> {
        let mut crate_path = AttrSlot::default();
        let mut wire = AttrSlot::default();
        let parser = syn::meta::parser(|meta| {
            if meta.path.is_ident("crate") {
                crate_path.set("value_enum", &meta, meta.value()?.parse()?)
            } else if meta.path.is_ident("wire") {
                wire.set("value_enum", &meta, meta.value()?.parse()?)
            } else {
                Err(unknown_key(&meta, "value_enum", "`wire` or `crate`"))
            }
        });
        parser.parse2(input.parse()?)?;
        Ok(Self {
            crate_path: crate_path.into_value(),
            wire: wire.into_value(),
        })
    }
}

fn parse_display(input: ParseStream<'_>) -> Result<Display> {
    if input.peek(LitStr) {
        return Ok(Display::Format(input.parse()?));
    }
    let path: Path = input.parse()?;
    if path.is_ident("none") {
        Ok(Display::None)
    } else {
        Ok(Display::Function(path))
    }
}

fn integer_expression(expression: Expr) -> Result<Expr> {
    let value = match &expression {
        Expr::Unary(unary) if matches!(unary.op, syn::UnOp::Neg(_)) => &*unary.expr,
        other => other,
    };
    if matches!(value, Expr::Lit(literal) if matches!(&literal.lit, Lit::Int(integer) if integer.suffix().is_empty()))
    {
        Ok(expression)
    } else {
        Err(Error::new_spanned(
            expression,
            "value bounds and wire codes must be unsuffixed integer literals",
        ))
    }
}

fn require_integer(inner: &Type, bounded: bool) -> Result<()> {
    let Type::Path(path) = inner else {
        return Err(Error::new_spanned(
            inner,
            "value requires a primitive integer inner type",
        ));
    };
    let allowed = if bounded {
        &["u8", "u16", "i8", "i16", "i32"][..]
    } else {
        &["u8", "u16", "u32", "i8", "i16", "i32"][..]
    };
    if path.qself.is_none()
        && path.path.segments.len() == 1
        && allowed.iter().any(|name| path.path.is_ident(name))
    {
        Ok(())
    } else {
        Err(Error::new_spanned(
            inner,
            "range bounds are reported as `i32`; use `u8`, `u16`, `i8`, `i16` or `i32`",
        ))
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum ScalarKind {
    Integer,
    Float,
}

fn scalar_kind(inner: &Type) -> Result<ScalarKind> {
    if matches!(inner, Type::Path(path) if path.qself.is_none() && (path.path.is_ident("f32") || path.path.is_ident("f64")))
    {
        Ok(ScalarKind::Float)
    } else {
        require_integer(inner, true)?;
        Ok(ScalarKind::Integer)
    }
}

fn validate_scalar_arguments(args: &ScalarArgs, inner: &Type) -> Result<ScalarKind> {
    let kind = scalar_kind(inner)?;
    let values = args
        .min
        .iter()
        .chain(&args.max)
        .chain(args.values.iter().flat_map(|values| values.elems.iter()));
    for value in values {
        if kind == ScalarKind::Integer {
            integer_expression(value.clone())?;
        } else {
            fn float_expression(value: &Expr) -> Result<()> {
                match value {
                    Expr::Path(_) => Ok(()),
                    Expr::Lit(literal) if matches!(&literal.lit, Lit::Float(_) | Lit::Int(_)) => {
                        Ok(())
                    }
                    Expr::Unary(unary) if matches!(unary.op, syn::UnOp::Neg(_)) => {
                        float_expression(&unary.expr)
                    }
                    _ => Err(Error::new_spanned(
                        value,
                        "float bounds must be numeric literals or constants",
                    )),
                }
            }
            float_expression(value)?;
        }
    }
    if kind == ScalarKind::Float && args.bias.is_some() {
        return Err(Error::new_spanned(
            inner,
            "floating-point values use their identity wire representation",
        ));
    }
    Ok(kind)
}

pub(crate) fn attribute(kind: Kind, args: TokenStream, item: TokenStream) -> Result<TokenStream> {
    let explicit = match kind {
        Kind::Scalar => syn::parse2::<ScalarArgs>(args.clone())?.crate_path,
        Kind::Enum => syn::parse2::<EnumArgs>(args.clone())?.crate_path,
    };
    let crate_path = explicit.map_or_else(crate::crate_path::grafton_visca, |path| quote!(#path));
    let input: DeriveInput = syn::parse2(item)?;
    validate_shape(kind, &input)?;
    if let (Kind::Scalar, Data::Struct(data)) = (kind, &input.data) {
        let Fields::Unnamed(fields) = &data.fields else {
            unreachable!()
        };
        validate_scalar_arguments(&syn::parse2(args.clone())?, &fields.unnamed[0].ty)?;
    }
    let kind = match kind {
        Kind::Scalar => quote!(scalar),
        Kind::Enum => quote!(enum),
    };
    Ok(quote! {
        #crate_path::__macro_support::value_helpers! {
            #kind [#args] #input
        }
    })
}

fn validate_shape(kind: Kind, input: &DeriveInput) -> Result<()> {
    if !input.generics.params.is_empty() {
        return Err(Error::new_spanned(
            &input.generics,
            "value attributes do not support generic declarations",
        ));
    }
    match (kind, &input.data) {
        (Kind::Scalar, Data::Struct(data)) => match &data.fields {
            Fields::Unnamed(fields) if fields.unnamed.len() == 1 => {
                let field = &fields.unnamed[0];
                if !matches!(field.vis, syn::Visibility::Inherited) {
                    return Err(Error::new_spanned(
                        &field.vis,
                        "a checked value's inner field must be private",
                    ));
                }
                scalar_kind(&field.ty).map(|_| ())
            }
            _ => Err(Error::new_spanned(
                input,
                "value requires a tuple struct with exactly one field",
            )),
        },
        (Kind::Enum, Data::Enum(_)) => Ok(()),
        (Kind::Scalar, _) => Err(Error::new_spanned(
            input,
            "value requires a tuple struct with exactly one field",
        )),
        (Kind::Enum, _) => Err(Error::new_spanned(input, "value_enum requires an enum")),
    }
}

struct Expansion {
    crate_path: Path,
    features: BTreeSet<String>,
    kind: Kind,
    args: TokenStream,
    input: DeriveInput,
}

impl Parse for Expansion {
    fn parse(input: ParseStream<'_>) -> Result<Self> {
        input.parse::<Token![crate]>()?;
        input.parse::<Token![=]>()?;
        let crate_path = input.parse()?;
        input.parse::<Token![;]>()?;
        let feature_tokens;
        bracketed!(feature_tokens in input);
        let feature_names = Punctuated::<Ident, Token![,]>::parse_terminated(&feature_tokens)?;
        let mut features = BTreeSet::new();
        for feature in feature_names {
            let name = feature.to_string();
            if !matches!(name.as_str(), "serde" | "schemars" | "ts_rs") {
                return Err(Error::new_spanned(feature, "unknown value helper feature"));
            }
            if !features.insert(name) {
                return Err(Error::new_spanned(
                    feature,
                    "duplicate value helper feature",
                ));
            }
        }
        if (features.contains("schemars") || features.contains("ts_rs"))
            && !features.contains("serde")
        {
            return Err(input.error("schemars and ts-rs value helpers require serde"));
        }
        input.parse::<Token![;]>()?;
        let kind = if input.peek(Token![enum]) {
            input.parse::<Token![enum]>()?;
            Kind::Enum
        } else {
            let word: Ident = input.parse()?;
            if word != "scalar" {
                return Err(Error::new_spanned(word, "expected scalar or enum"));
            }
            Kind::Scalar
        };
        let arguments;
        bracketed!(arguments in input);
        Ok(Self {
            crate_path,
            features,
            kind,
            args: arguments.parse()?,
            input: input.parse()?,
        })
    }
}

pub(crate) fn expand(tokens: TokenStream) -> Result<TokenStream> {
    let expansion: Expansion = syn::parse2(tokens)?;
    validate_shape(expansion.kind, &expansion.input)?;
    match expansion.kind {
        Kind::Scalar => expand_scalar(expansion),
        Kind::Enum => expand_enum(expansion),
    }
}

fn standard_derives() -> TokenStream {
    quote! { #[derive(
        ::core::fmt::Debug,
        ::core::clone::Clone,
        ::core::marker::Copy,
        ::core::cmp::PartialEq,
        ::core::cmp::Eq,
        ::core::cmp::PartialOrd,
        ::core::cmp::Ord,
        ::core::hash::Hash
    )] }
}

/// Preserve arbitrary user attributes, but do not duplicate generated derives.
fn strip_generated_derives(attribute: &Attribute) -> Result<Option<Attribute>> {
    fn clean(meta: &Meta) -> Result<Option<Meta>> {
        if meta.path().is_ident("cfg_attr") {
            let Meta::List(list) = meta else {
                return Ok(Some(meta.clone()));
            };
            let entries = list.parse_args_with(Punctuated::<Meta, Token![,]>::parse_terminated)?;
            let mut entries = entries.into_iter();
            let Some(predicate) = entries.next() else {
                return Ok(Some(meta.clone()));
            };
            let retained = entries
                .filter_map(|entry| clean(&entry).transpose())
                .collect::<Result<Vec<_>>>()?;
            return Ok(
                (!retained.is_empty()).then(|| parse_quote!(cfg_attr(#predicate, #(#retained),*)))
            );
        }
        if meta.path().is_ident("serde") {
            let Meta::List(list) = meta else {
                return Ok(Some(meta.clone()));
            };
            let options = list.parse_args_with(Punctuated::<Meta, Token![,]>::parse_terminated)?;
            let remaining: Vec<_> = options
                .into_iter()
                .filter(|entry| !entry.path().is_ident("rename_all"))
                .collect();
            return Ok((!remaining.is_empty()).then(|| parse_quote!(serde(#(#remaining),*))));
        }
        if !meta.path().is_ident("derive") {
            return Ok(Some(meta.clone()));
        }
        let Meta::List(list) = meta else {
            return Ok(Some(meta.clone()));
        };
        let derives = list.parse_args_with(Punctuated::<Path, Token![,]>::parse_terminated)?;
        let remaining: Vec<_> = derives
            .into_iter()
            .filter(|path| {
                path.segments.last().is_none_or(|segment| {
                    !matches!(
                        segment.ident.to_string().as_str(),
                        "Debug"
                            | "Clone"
                            | "Copy"
                            | "PartialEq"
                            | "Eq"
                            | "PartialOrd"
                            | "Ord"
                            | "Hash"
                            | "Serialize"
                            | "Deserialize"
                            | "JsonSchema"
                            | "TS"
                    )
                })
            })
            .collect();
        Ok((!remaining.is_empty()).then(|| parse_quote!(derive(#(#remaining),*))))
    }
    clean(&attribute.meta).map(|meta| meta.map(|meta| parse_quote!(#[#meta])))
}

fn availability(attributes: &[Attribute]) -> Result<Vec<Attribute>> {
    fn cfg_part(meta: &Meta) -> Result<Option<Meta>> {
        if meta.path().is_ident("cfg") {
            return Ok(Some(meta.clone()));
        }
        if !meta.path().is_ident("cfg_attr") {
            return Ok(None);
        }
        let Meta::List(list) = meta else {
            return Ok(None);
        };
        let entries = list.parse_args_with(Punctuated::<Meta, Token![,]>::parse_terminated)?;
        let mut entries = entries.into_iter();
        let Some(predicate) = entries.next() else {
            return Ok(None);
        };
        let retained = entries
            .filter_map(|entry| cfg_part(&entry).transpose())
            .collect::<Result<Vec<_>>>()?;
        Ok((!retained.is_empty()).then(|| parse_quote!(cfg_attr(#predicate, #(#retained),*))))
    }
    attributes
        .iter()
        .filter_map(|attribute| cfg_part(&attribute.meta).transpose())
        .map(|meta| meta.map(|meta| parse_quote!(#[#meta])))
        .collect()
}

fn helpers(expansion: &Expansion, inner: Option<&Type>) -> (TokenStream, TokenStream) {
    let path = &expansion.crate_path;
    let name = &expansion.input.ident;
    let module = format_ident!(
        "__grafton_visca_value_support_{}",
        name.to_string().trim_start_matches("r#")
    );
    let features = &expansion.features;
    let reexports = features.iter().map(|feature| {
        let feature = Ident::new(feature, Span::call_site());
        quote!(pub use #path::__macro_support::#feature;)
    });
    let support = (!features.is_empty()).then(|| {
        quote! {
            #[doc(hidden)]
            #[allow(non_snake_case)]
            mod #module { #(#reexports)* }
        }
    });
    let serde_path = LitStr::new(&format!("{module}::serde"), Span::call_site());
    let serde = features.contains("serde").then(|| {
        let conversion = inner.map(|inner| {
            let inner = LitStr::new(&quote!(#inner).to_string(), Span::call_site());
            quote!(#[serde(try_from = #inner, into = #inner)])
        });
        let naming = inner.is_none().then(|| quote!(#[serde(rename_all = "snake_case")]));
        quote! {
            #[derive(#path::__macro_support::serde::Serialize, #path::__macro_support::serde::Deserialize)]
            #[serde(crate = #serde_path)]
            #conversion
            #naming
        }
    });
    let schemars = features.contains("schemars").then(|| {
        let helper = LitStr::new(&format!("{module}::schemars"), Span::call_site());
        let transparent = inner.map(|_| quote!(#[schemars(!try_from, !into)]));
        quote! {
            #[derive(#path::__macro_support::schemars::JsonSchema)]
            #[schemars(crate = #helper)]
            #transparent
        }
    });
    let typescript = features.contains("ts_rs").then(|| {
        let helper = LitStr::new(&format!("{module}::ts_rs"), Span::call_site());
        quote! {
            #[derive(#path::__macro_support::ts_rs::TS)]
            #[ts(crate = #helper, export)]
        }
    });
    (quote!(#support), quote!(#serde #schemars #typescript))
}

fn expand_scalar(mut expansion: Expansion) -> Result<TokenStream> {
    let args: ScalarArgs = syn::parse2(expansion.args.clone())?;
    let Data::Struct(data) = &expansion.input.data else {
        unreachable!()
    };
    let Fields::Unnamed(fields) = &data.fields else {
        unreachable!()
    };
    let inner = fields.unnamed[0].ty.clone();
    let kind = validate_scalar_arguments(&args, &inner)?;
    let name = expansion.input.ident.clone();
    let path = &expansion.crate_path;
    let attrs = availability(&expansion.input.attrs)?;
    let (support, helper_derives) = helpers(&expansion, Some(&inner));
    let normalized = |value: TokenStream| {
        if kind == ScalarKind::Float {
            quote!({ let value: #inner = #value; if value == 0.0 { 0.0 } else { value } })
        } else {
            value
        }
    };
    let bounds = match (&args.min, &args.max, &args.values) {
        (Some(min), Some(max), None) => {
            let min = normalized(quote!(#min));
            let max = normalized(quote!(#max));
            quote! {
                /// The smallest accepted value.
                pub const MIN: Self = Self(#min);
                /// The largest accepted value.
                pub const MAX: Self = Self(#max);
            }
        }
        (_, _, Some(values)) => {
            let extreme = |comparison: TokenStream| {
                let selected = normalized(quote!(extreme));
                quote!({
                    const VALUES: &[#inner] = &#values;
                    let mut extreme = VALUES[0];
                    let mut index = 1;
                    while index < VALUES.len() {
                        if #comparison { extreme = VALUES[index]; }
                        index += 1;
                    }
                    Self(#selected)
                })
            };
            let min = extreme(quote!(VALUES[index] < extreme));
            let max = extreme(quote!(VALUES[index] > extreme));
            quote! {
                /// The smallest accepted value.
                pub const MIN: Self = #min;
                /// The largest accepted value.
                pub const MAX: Self = #max;

            }
        }
        _ => unreachable!(),
    };
    let default_error = if kind == ScalarKind::Float {
        quote!(#path::__macro_support::invalid_value(::core::stringify!(#name), "value must be finite and within the declared bounds"))
    } else if let Some(values) = &args.values {
        let values_text = values
            .elems
            .iter()
            .map(code_key)
            .collect::<Result<Vec<_>>>()?
            .join(", ");
        let domain = LitStr::new(&format!("must be one of [{values_text}]"), values.span());
        quote!(#path::__macro_support::invalid_value(::core::stringify!(#name), #domain))
    } else {
        quote!(#path::Error::parameter_out_of_range(::core::stringify!(#name), value as i32, Self::MIN.0 as i32, Self::MAX.0 as i32))
    };
    struct BindValue;
    impl Fold for BindValue {
        fn fold_expr_path(&mut self, mut expression: syn::ExprPath) -> syn::ExprPath {
            if expression.qself.is_none() && expression.path.is_ident("value") {
                expression.path = parse_quote!(value);
                expression
            } else {
                syn::fold::fold_expr_path(self, expression)
            }
        }
    }
    let error = args.error.as_ref().map_or(default_error, |error| {
        let error = BindValue.fold_expr(error.clone());
        quote!(#error)
    });
    let checked = normalized(quote!(value));
    let checked = quote!(Self(#checked));
    let finite_guard = (kind == ScalarKind::Float).then(|| {
        quote! {
            if !value.is_finite() { return ::core::result::Result::Err(#error); }
        }
    });
    let validate = if let Some(values) = &args.values {
        quote! {
            #finite_guard
            const VALUES: &[#inner] = &#values;
            let mut index = 0;
            while index < VALUES.len() {
                if value == VALUES[index] { return ::core::result::Result::Ok(#checked); }
                index += 1;
            }
            ::core::result::Result::Err(#error)
        }
    } else {
        quote! {
            #finite_guard
            if value < Self::MIN.0 || value > Self::MAX.0 { return ::core::result::Result::Err(#error); }
            ::core::result::Result::Ok(#checked)
        }
    };
    let display = match args
        .display
        .unwrap_or_else(|| Display::Format(parse_quote!("{}")))
    {
        Display::Format(format) => quote! {
            #(#attrs)*
            impl ::core::fmt::Display for #name {
                fn fmt(&self, formatter: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result {
                    ::core::write!(formatter, #format, self.0)
                }
            }
        },
        Display::Function(format) => quote! {
            #(#attrs)*
            impl ::core::fmt::Display for #name {
                fn fmt(&self, formatter: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result { #format(self, formatter) }
            }
        },
        Display::None => TokenStream::new(),
    };
    let wire = if let Some((bias, wire)) = args.bias {
        quote! {
            #(#attrs)*
            const _: () = {
                ::core::assert!((#name::MIN.0 as i64) + (#bias as i64) >= (<#wire>::MIN as i64), "value wire representation underflows");
                ::core::assert!((#name::MAX.0 as i64) + (#bias as i64) <= (<#wire>::MAX as i64), "value wire representation overflows");
            };
            #(#attrs)*
            impl #path::wire::WireValue for #name {
                type Wire = #wire;
                #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
                fn to_wire(self) -> Self::Wire { (<::core::primitive::i64 as ::core::convert::From<#inner>>::from(self.0) + (#bias as ::core::primitive::i64)) as #wire }
                fn from_wire(value: Self::Wire) -> #path::Result<Self> {
                    let raw = <::core::primitive::i64 as ::core::convert::From<#wire>>::from(value) - (#bias as ::core::primitive::i64);
                    let checked = <#inner as ::core::convert::TryFrom<::core::primitive::i64>>::try_from(raw).map_err(|_| #path::Error::invalid_parameter(::core::stringify!(#name), ::std::format!("{value:?}"), "wire value is outside the value domain"))?;
                    Self::new(checked)
                }
            }
        }
    } else {
        quote! {
            #(#attrs)*
            impl #path::wire::WireValue for #name {
                type Wire = #inner;
                fn to_wire(self) -> Self::Wire { self.0 }
                fn from_wire(value: Self::Wire) -> #path::Result<Self> { Self::new(value) }
            }
        }
    };
    expansion.input.attrs = expansion
        .input
        .attrs
        .iter()
        .filter_map(|attr| strip_generated_derives(attr).transpose())
        .collect::<Result<_>>()?;
    if kind == ScalarKind::Float && expansion.features.contains("schemars") {
        if let (Some(min), Some(max)) = (&args.min, &args.max) {
            let Data::Struct(data) = &mut expansion.input.data else {
                unreachable!()
            };
            let Fields::Unnamed(fields) = &mut data.fields else {
                unreachable!()
            };
            fields.unnamed[0]
                .attrs
                .push(parse_quote!(#[schemars(range(min = #min, max = #max))]));
        }
    }
    let input = &expansion.input;
    let derives = if kind == ScalarKind::Float {
        quote!(#[derive(::core::fmt::Debug, ::core::clone::Clone, ::core::marker::Copy, ::core::cmp::PartialEq)])
    } else {
        standard_derives()
    };
    let float_contract = (kind == ScalarKind::Float).then(|| quote! {
        #(#attrs)*
        impl ::core::cmp::Eq for #name {}
        #(#attrs)*
        impl ::core::cmp::PartialOrd for #name {
            fn partial_cmp(&self, other: &Self) -> ::core::option::Option<::core::cmp::Ordering> {
                ::core::option::Option::Some(::core::cmp::Ord::cmp(self, other))
            }
        }
        #(#attrs)*
        impl ::core::cmp::Ord for #name {
            fn cmp(&self, other: &Self) -> ::core::cmp::Ordering { self.0.total_cmp(&other.0) }
        }
        #(#attrs)*
        impl ::core::hash::Hash for #name {
            fn hash<H: ::core::hash::Hasher>(&self, state: &mut H) { ::core::hash::Hash::hash(&self.0.to_bits(), state); }
        }
    });
    let finite_bounds = (kind == ScalarKind::Float).then(|| {
        let finite_values = args.values.as_ref().map(|values| quote! {
            const VALUES: &[#inner] = &#values;
            let mut index = 0;
            while index < VALUES.len() {
                ::core::assert!(VALUES[index].is_finite(), "value declares a non-finite domain value");
                index += 1;
            }
        });
        quote! {
            ::core::assert!(#name::MIN.0.is_finite() && #name::MAX.0.is_finite(), "value declares a non-finite bound");
            #finite_values
        }
    });
    Ok(quote! {
        #(#attrs)* #support
        #derives #helper_derives #input
        #(#attrs)*
        const _: () = {
            #finite_bounds
            ::core::assert!(#name::MIN.0 <= #name::MAX.0, "value declares min greater than max");
        };
        #(#attrs)*
        impl #name {
            #bounds
            /// Creates a value after checking its declared domain.
            ///
            /// # Errors
            /// Returns an error when the value is outside the declared domain.
            #[allow(clippy::cast_lossless, trivial_numeric_casts)]
            pub const fn new(value: #inner) -> #path::Result<Self> { #validate }
            /// Returns the checked scalar value.
            #[must_use]
            pub const fn value(self) -> #inner { self.0 }
        }
        #(#attrs)*
        impl ::core::convert::TryFrom<#inner> for #name {
            type Error = #path::Error;
            fn try_from(value: #inner) -> #path::Result<Self> { Self::new(value) }
        }
        #(#attrs)*
        impl ::core::convert::From<#name> for #inner {
            fn from(value: #name) -> Self { value.0 }
        }
        #display
        #wire
        #float_contract
    })
}

fn code_key(expression: &Expr) -> Result<String> {
    match expression {
        Expr::Tuple(tuple) => tuple
            .elems
            .iter()
            .map(code_key)
            .collect::<Result<Vec<_>>>()
            .map(|parts| format!("({})", parts.join(","))),
        Expr::Lit(literal) if matches!(literal.lit, Lit::Int(_)) => {
            let Lit::Int(integer) = &literal.lit else {
                unreachable!()
            };
            if !integer.suffix().is_empty() {
                return Err(Error::new_spanned(
                    integer,
                    "wire codes must be unsuffixed integer literals",
                ));
            }
            integer.base10_parse::<u64>().map(|value| value.to_string())
        }
        Expr::Unary(unary) if matches!(unary.op, syn::UnOp::Neg(_)) => {
            Ok(format!("-{}", code_key(&unary.expr)?))
        }
        _ => Err(Error::new_spanned(
            expression,
            "wire codes must be integer literals or literal tuples",
        )),
    }
}

fn validate_code(expression: &Expr, wire: &Type) -> Result<()> {
    if let Type::Tuple(types) = wire {
        let Expr::Tuple(values) = expression else {
            return Err(Error::new_spanned(
                expression,
                "tuple wire types require a literal tuple code",
            ));
        };
        if values.elems.len() != types.elems.len() {
            return Err(Error::new_spanned(
                expression,
                "wire code tuple has the wrong number of elements",
            ));
        }
        for (value, ty) in values.elems.iter().zip(&types.elems) {
            validate_code(value, ty)?;
        }
        return Ok(());
    }
    require_integer(wire, false)?;
    let Type::Path(path) = wire else {
        unreachable!()
    };
    let name = path.path.segments[0].ident.to_string();
    let (min, max) = match name.as_str() {
        "u8" => (0, i128::from(u8::MAX)),
        "u16" => (0, i128::from(u16::MAX)),
        "u32" => (0, i128::from(u32::MAX)),
        "i8" => (i128::from(i8::MIN), i128::from(i8::MAX)),
        "i16" => (i128::from(i16::MIN), i128::from(i16::MAX)),
        "i32" => (i128::from(i32::MIN), i128::from(i32::MAX)),
        _ => unreachable!(),
    };
    let key = code_key(expression)?;
    if key
        .parse::<i128>()
        .is_ok_and(|value| (min..=max).contains(&value))
    {
        Ok(())
    } else {
        Err(Error::new_spanned(
            expression,
            format!("wire code must be an unsuffixed integer literal in {min}..={max}"),
        ))
    }
}

/// Translate conditional presence into one predicate for the missing-payload arm.
/// Non-presence attributes stay on the declaration and never reach match arms.
fn presence_predicate(attributes: &[Attribute]) -> Result<TokenStream> {
    fn condition(meta: &Meta) -> Result<Option<TokenStream>> {
        let Meta::List(list) = meta else {
            return Ok(None);
        };
        if meta.path().is_ident("cfg") {
            let predicate: TokenStream = list.parse_args()?;
            return Ok(Some(predicate));
        }
        if !meta.path().is_ident("cfg_attr") {
            return Ok(None);
        }
        let entries = list.parse_args_with(Punctuated::<Meta, Token![,]>::parse_terminated)?;
        let mut entries = entries.into_iter();
        let Some(predicate) = entries.next() else {
            return Ok(None);
        };
        let nested = entries
            .filter_map(|entry| condition(&entry).transpose())
            .collect::<Result<Vec<_>>>()?;
        Ok((!nested.is_empty()).then(|| quote!(any(not(#predicate), all(#(#nested),*)))))
    }

    let conditions = attributes
        .iter()
        .filter_map(|attribute| condition(&attribute.meta).transpose())
        .collect::<Result<Vec<_>>>()?;
    Ok(quote!(all(#(#conditions),*)))
}

fn expand_enum(mut expansion: Expansion) -> Result<TokenStream> {
    let options: EnumArgs = syn::parse2(expansion.args.clone())?;
    let wire = options.wire.unwrap_or_else(|| parse_quote!(u8));
    let path = &expansion.crate_path;
    let name = &expansion.input.ident;
    let attrs = availability(&expansion.input.attrs)?;
    let (support, helper_derives) = helpers(&expansion, None);
    let Data::Enum(data) = &mut expansion.input.data else {
        unreachable!()
    };
    let mut known = Vec::new();
    let mut fallback = Vec::new();
    let mut payload_presence: Vec<Vec<Attribute>> = Vec::new();
    let mut to_wire = Vec::new();
    let mut display_arms = Vec::new();
    let mut keys: BTreeMap<String, Vec<Vec<Attribute>>> = BTreeMap::new();
    let mut conditional_conflicts = Vec::new();
    let has_payload = data
        .variants
        .iter()
        .any(|variant| !matches!(variant.fields, Fields::Unit));
    for variant in &mut data.variants {
        let id = &variant.ident;
        let mut code: Option<Expr> = None;
        let mut display = None;
        let mut remaining = Vec::new();
        for attribute in &variant.attrs {
            if attribute.path().is_ident("wire") {
                if code.is_some() {
                    return Err(Error::new_spanned(attribute, "duplicate variant wire code"));
                }
                code = Some(attribute.parse_args()?);
            } else if attribute.path().is_ident("value") {
                let mut slot = AttrSlot::default();
                attribute.parse_nested_meta(|meta| {
                    if meta.path.is_ident("display") {
                        slot.set("value", &meta, meta.value()?.parse::<LitStr>()?)
                    } else {
                        Err(unknown_key(&meta, "value", "`display`"))
                    }
                })?;
                if display.is_some() {
                    return Err(Error::new_spanned(attribute, "duplicate variant display"));
                }
                display = slot.into_value();
            } else {
                remaining.push(attribute.clone());
            }
        }
        variant.attrs = remaining;
        let presence = availability(&variant.attrs)?;
        match &variant.fields {
            Fields::Unit => {
                if let Some((_, value)) = &variant.discriminant {
                    if code.is_some() {
                        return Err(Error::new_spanned(
                            variant,
                            "a variant cannot specify both a discriminant and a wire attribute",
                        ));
                    }
                    code = Some(value.clone());
                }
                let code = code.ok_or_else(|| {
                    Error::new_spanned(
                        &variant.ident,
                        "each unit variant needs an explicit discriminant or #[wire(code)]",
                    )
                })?;
                validate_code(&code, &wire)?;
                let same_code = keys.entry(code_key(&code)?).or_default();
                for previous in same_code.iter() {
                    if previous.is_empty() && presence.is_empty() {
                        return Err(Error::new_spanned(&code, "duplicate value_enum wire code"));
                    }
                    conditional_conflicts.push(quote! {
                        #(#attrs)*
                        #(#previous)*
                        #(#presence)*
                        const _: () = {
                            ::core::assert!(false, "duplicate value_enum wire code");
                        };
                    });
                }
                same_code.push(presence.clone());
                known.push(quote!(#(#presence)* #code => ::core::result::Result::Ok(Self::#id),));
                to_wire.push(quote!(#(#presence)* Self::#id => #code,));
                let label = display.unwrap_or_else(|| LitStr::new(&id.to_string(), id.span()));
                display_arms.push(quote!(#(#presence)* Self::#id => formatter.write_str(#label),));
                // Tuple payload enums cannot carry implicit repr-dependent
                // discriminants; WireValue owns their explicit code mapping.
                if has_payload {
                    variant.discriminant = None;
                }
            }
            Fields::Unnamed(fields) if fields.unnamed.len() == 1 => {
                if code.is_some() || variant.discriminant.is_some() {
                    return Err(Error::new_spanned(
                        variant,
                        "a payload variant delegates its wire code to its value",
                    ));
                }
                for previous in &payload_presence {
                    if previous.is_empty() && presence.is_empty() {
                        return Err(Error::new_spanned(
                            variant,
                            "value_enum supports at most one checked payload variant",
                        ));
                    }
                    conditional_conflicts.push(quote! {
                        #(#attrs)*
                        #(#previous)*
                        #(#presence)*
                        const _: () = {
                            ::core::assert!(false, "value_enum supports at most one checked payload variant");
                        };
                    });
                }
                payload_presence.push(presence.clone());
                let inner = &fields.unnamed[0].ty;
                fallback.push(
                    quote!(#(#presence)* other => <#inner as #path::wire::WireValue>::from_wire(other).map(Self::#id),),
                );
                to_wire.push(
                    quote!(#(#presence)* Self::#id(value) => <#inner as #path::wire::WireValue>::to_wire(value),),
                );
                let label =
                    display.unwrap_or_else(|| LitStr::new(&format!("{id} {{}}"), id.span()));
                display_arms.push(
                    quote!(#(#presence)* Self::#id(value) => ::core::write!(formatter, #label, value),),
                );
            }
            _ => {
                return Err(Error::new_spanned(
                    variant,
                    "value_enum variants must be unit variants or one checked tuple value",
                ))
            }
        }
    }
    if data.variants.is_empty() {
        return Err(Error::new_spanned(
            data.enum_token,
            "value_enum requires at least one variant",
        ));
    }
    let payload_predicates = payload_presence
        .iter()
        .map(|presence| presence_predicate(presence))
        .collect::<Result<Vec<_>>>()?;
    let no_payload = quote!(#[cfg(not(any(#(#payload_predicates),*)))]);
    let fallback = quote! {
        #(#fallback)*
        #no_payload
        other => ::core::result::Result::Err(#path::Error::invalid_parameter(::core::stringify!(#name), ::std::format!("{other:?}"), "unknown wire value")),
    };
    expansion.input.attrs = expansion
        .input
        .attrs
        .iter()
        .filter_map(|attr| strip_generated_derives(attr).transpose())
        .collect::<Result<_>>()?;
    let input = &expansion.input;
    let derives = standard_derives();
    Ok(quote! {
        #(#attrs)* #support
        #derives #helper_derives #input
        #(#conditional_conflicts)*
        #(#attrs)*
        impl #path::wire::WireValue for #name {
            type Wire = #wire;
            fn to_wire(self) -> Self::Wire { match self { #(#to_wire)* } }
            fn from_wire(value: Self::Wire) -> #path::Result<Self> { match value { #(#known)* #fallback } }
        }
        #(#attrs)*
        impl ::core::fmt::Display for #name {
            fn fmt(&self, formatter: &mut ::core::fmt::Formatter<'_>) -> ::core::fmt::Result { match *self { #(#display_arms)* } }
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scalar(arguments: TokenStream, input: TokenStream) -> Result<TokenStream> {
        expand(quote!(crate = ::visca; []; scalar [#arguments] #input))
    }

    fn value_enum(arguments: TokenStream, input: TokenStream) -> Result<TokenStream> {
        expand(quote!(crate = ::visca; []; enum [#arguments] #input))
    }

    #[test]
    fn scalar_expansion_has_one_complete_value_contract() {
        let result = scalar(
            quote!(min = -7, max = 7, display = "Level {}", wire = centered(7)),
            quote!(
                pub struct Level(i8);
            ),
        )
        .unwrap()
        .to_string();
        for required in [
            "pub const fn new",
            "pub const fn value",
            "const MIN",
            "const MAX",
            "Hash",
            "WireValue",
            "TryFrom",
            "Display",
        ] {
            assert!(result.contains(required), "missing {required}: {result}");
        }
    }

    #[test]
    fn options_and_shapes_are_rejected_before_codegen() {
        for arguments in [
            quote!(min = 0),
            quote!(max = 7),
            quote!(min = 0, max = 7, min = 1),
            quote!(min = 0, max = 7, unknown = true),
            quote!(valid_values = []),
        ] {
            assert!(scalar(
                arguments,
                quote!(
                    struct Level(u8);
                )
            )
            .is_err());
        }
        assert!(scalar(
            quote!(min = 0, max = 7),
            quote!(
                struct Level(pub u8);
            )
        )
        .is_err());
        assert!(scalar(
            quote!(min = 0, max = 7),
            quote!(
                struct Level<T>(T);
            )
        )
        .is_err());
        assert!(scalar(
            quote!(min = 0, max = 7),
            quote!(
                struct Level(bool);
            )
        )
        .is_err());
    }

    #[test]
    fn enum_codes_are_compared_by_value_across_radices() {
        assert!(value_enum(
            quote!(),
            quote!(
                enum Mode {
                    A = 10,
                    B = 0x0a,
                }
            )
        )
        .is_err());
        assert!(value_enum(
            quote!(wire = (u8, u8)),
            quote!(
                enum Direction {
                    #[wire((3,1))]
                    Up,
                    #[wire((0x3,0b1))]
                    Alias,
                }
            )
        )
        .is_err());
        assert!(value_enum(
            quote!(),
            quote!(
                enum Wide {
                    TooWide = 256,
                }
            )
        )
        .is_err());
        assert!(value_enum(
            quote!(),
            quote!(
                enum Negative {
                    Negative = -1,
                }
            )
        )
        .is_err());
        assert!(value_enum(
            quote!(),
            quote!(
                enum Suffixed {
                    Suffixed = 1u8,
                }
            )
        )
        .is_err());
    }

    #[test]
    fn payloads_forward_through_their_checked_wire_value() {
        let result = value_enum(
            quote!(),
            quote!(
                enum Noise {
                    Off = 0,
                    Level(CheckedLevel),
                }
            ),
        )
        .unwrap()
        .to_string();
        assert!(result.contains("CheckedLevel as :: visca :: wire :: WireValue"));
        assert!(!result.contains("Off = 0"));
        assert!(value_enum(
            quote!(),
            quote!(
                enum Invalid {
                    A(u8),
                    B(u8),
                }
            )
        )
        .is_err());
    }

    #[test]
    fn conditional_availability_gates_all_generated_items() {
        let result = scalar(
            quote!(min = 0, max = 7),
            quote!(
                #[cfg_attr(any(), cfg(any()))]
                struct Level(u8);
            ),
        )
        .unwrap()
        .to_string();
        assert!(result.matches("cfg_attr").count() >= 5);
    }

    #[test]
    fn finite_float_expansion_uses_the_same_value_pipeline() {
        for (bounds, item) in [
            (
                quote!(min = f32::MIN, max = f32::MAX),
                quote!(
                    struct Degrees(f32);
                ),
            ),
            (
                quote!(min = -1.0, max = 1.0),
                quote!(
                    struct Unit(f64);
                ),
            ),
        ] {
            let result = scalar(bounds, item).unwrap().to_string();
            for required in [
                "is_finite",
                "total_cmp",
                "to_bits",
                "const MIN",
                "const MAX",
                "const fn new",
                "WireValue",
            ] {
                assert!(result.contains(required), "missing {required}: {result}");
            }
        }
        assert!(scalar(
            quote!(min = 0.0, max = 1.0, wire = centered(7)),
            quote!(
                struct Invalid(f32);
            )
        )
        .is_err());
        assert!(scalar(
            quote!(min = "bad", max = 1.0),
            quote!(
                struct Invalid(f32);
            )
        )
        .is_err());
        assert!(scalar(
            quote!(min = "bad", max = 7),
            quote!(
                struct Invalid(u8);
            )
        )
        .is_err());
    }
}
