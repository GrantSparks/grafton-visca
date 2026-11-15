// SPDX-License-Identifier: MIT OR Apache-2.0
//
// Copyright (c) 2024 Grafton Machine Shed <team@grafton.ai>

//! ViscaInquiry derive macro implementation with parser generation support
//!
//! Attributes are parsed into [`RawAttributes`] under the crate-wide policy
//! (each key once, numeric bytes through one literal parser) and then
//! validated once into an [`InquirySpec`]. Code generation consumes only the
//! validated spec, so no generator restates a validation rule.

use proc_macro2::{Span, TokenStream};
use quote::quote;
use syn::{
    meta::ParseNestedMeta, parenthesized, parse::ParseStream, punctuated::Punctuated, DeriveInput,
    Ident, LitInt, Path, Token, Type,
};

use crate::attr::{parse_u8_literal, unknown_key, AttrSlot};

const ATTRIBUTE: &str = "visca";
const ATTRIBUTE_KEYS: &str = "one of `opcode`, `subcode`, `response`, `parser`, `parse_with`, \
                              `data_variant`, `typed_response`, `typed_field`, \
                              `typed_constructor`, or `typed_is_tuple`";

pub fn derive_visca_inquiry_impl(input: DeriveInput) -> TokenStream {
    if !matches!(input.data, syn::Data::Struct(_)) {
        return syn::Error::new_spanned(&input, "ViscaInquiry can only be derived for structs")
            .to_compile_error();
    }
    match parse_visca_attributes_from_struct(&input).and_then(|attrs| attrs.validate(&input.ident))
    {
        Ok(spec) => generate(&input.ident, &spec, &crate::crate_path::grafton_visca()),
        Err(error) => error.to_compile_error(),
    }
}

/// The `#[visca(...)]` keys as written, before cross-key validation.
#[derive(Default)]
struct RawAttributes {
    opcode: AttrSlot<u8>,
    subcode: AttrSlot<u8>,
    response: AttrSlot<Ident>,
    parser: AttrSlot<()>,
    parse_with: AttrSlot<Path>,
    data_variant: AttrSlot<Ident>,
    typed_response: AttrSlot<Type>,
    typed_field: AttrSlot<Vec<Ident>>,
    typed_constructor: AttrSlot<TypedConstructor>,
    typed_is_tuple: AttrSlot<()>,
}

/// How a typed response is built from the destructured inquiry fields.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum TypedConstructor {
    /// `Ok(field)`.
    Identity,
    /// `Type::new(field)`.
    New,
    /// `Ok(Type::new(field))`.
    OkNew,
    /// `Type::new(u8::try_from(field)?)`.
    NewU8,
    /// `Ok(Type { fields.. })`.
    OkStruct,
}

/// A validated inquiry derive.
struct InquirySpec {
    opcode: u8,
    subcode: u8,
    kind: ResponseKind,
}

enum ResponseKind {
    /// `response = Raw`: the payload is returned unparsed.
    Raw,
    /// A built-in `InquiryKind` response.
    BuiltIn(Box<BuiltInResponse>),
}

/// A canonical `InquiryKind` decoder, or an explicit custom parser, optionally
/// lifted into a typed response.
struct BuiltInResponse {
    response: Ident,
    /// The `InquiryData` variant the typed response destructures.
    variant: Ident,
    custom: Option<Path>,
    typed: Option<TypedResponse>,
}

struct TypedResponse {
    ty: Type,
    fields: Vec<Ident>,
    constructor: TypedConstructor,
    is_tuple: bool,
}

impl TypedConstructor {
    const ALL: [(&'static str, Self); 4] = [
        ("new", Self::New),
        ("ok_new", Self::OkNew),
        ("new_u8", Self::NewU8),
        ("ok_struct", Self::OkStruct),
    ];

    /// The attribute spelling, or `None` for the default conversion.
    fn name(self) -> Option<&'static str> {
        Self::ALL
            .iter()
            .find_map(|(name, constructor)| (*constructor == self).then_some(*name))
    }
}

/// Combines two results, keeping every error.
fn both<A, B>(a: syn::Result<A>, b: syn::Result<B>) -> syn::Result<(A, B)> {
    match (a, b) {
        (Ok(a), Ok(b)) => Ok((a, b)),
        (Err(mut first), Err(second)) => {
            first.combine(second);
            Err(first)
        }
        (Err(error), Ok(_)) | (Ok(_), Err(error)) => Err(error),
    }
}

fn parse_visca_attributes_from_struct(input: &DeriveInput) -> syn::Result<RawAttributes> {
    let mut attrs = RawAttributes::default();
    let mut saw_visca = false;

    for attr in input
        .attrs
        .iter()
        .filter(|attr| attr.path().is_ident(ATTRIBUTE))
    {
        saw_visca = true;
        attr.parse_nested_meta(|meta| attrs.parse_key(&meta))?;
    }

    if !saw_visca {
        return Err(syn::Error::new_spanned(
            &input.ident,
            "missing #[visca(...)] attribute",
        ));
    }

    Ok(attrs)
}

impl RawAttributes {
    fn parse_key(&mut self, meta: &ParseNestedMeta<'_>) -> syn::Result<()> {
        let path = &meta.path;
        if path.is_ident("typed_is_tuple") {
            self.typed_is_tuple.set(ATTRIBUTE, meta, ())
        } else if path.is_ident("opcode") {
            let byte = parse_inquiry_byte(&meta.value()?.parse::<LitInt>()?, "opcode")?;
            self.opcode.set(ATTRIBUTE, meta, byte)
        } else if path.is_ident("subcode") {
            let byte = parse_inquiry_byte(&meta.value()?.parse::<LitInt>()?, "subcode")?;
            self.subcode.set(ATTRIBUTE, meta, byte)
        } else if path.is_ident("response") {
            let response = parse_ident_value(meta.value()?, "response")?;
            self.response.set(ATTRIBUTE, meta, response)
        } else if path.is_ident("parser") {
            parse_custom_parser(meta.value()?)?;
            self.parser.set(ATTRIBUTE, meta, ())
        } else if path.is_ident("parse_with") {
            let parse_with = meta.value()?.parse()?;
            self.parse_with.set(ATTRIBUTE, meta, parse_with)
        } else if path.is_ident("data_variant") {
            let variant = parse_ident_value(meta.value()?, "data_variant")?;
            self.data_variant.set(ATTRIBUTE, meta, variant)
        } else if path.is_ident("typed_response") {
            let typed_response = meta.value()?.parse()?;
            self.typed_response.set(ATTRIBUTE, meta, typed_response)
        } else if path.is_ident("typed_field") {
            let fields = parse_ident_list_value(meta.value()?)?;
            self.typed_field.set(ATTRIBUTE, meta, fields)
        } else if path.is_ident("typed_constructor") {
            let constructor = parse_typed_constructor(meta.value()?)?;
            self.typed_constructor.set(ATTRIBUTE, meta, constructor)
        } else {
            Err(unknown_key(meta, ATTRIBUTE, ATTRIBUTE_KEYS))
        }
    }

    /// Validate cross-key rules once, before generating any decoder.
    fn validate(self, struct_name: &Ident) -> syn::Result<InquirySpec> {
        let Self {
            opcode,
            subcode,
            response,
            parser,
            parse_with,
            data_variant,
            typed_response,
            typed_field,
            typed_constructor,
            typed_is_tuple,
        } = self;
        let error = |message: &str| syn::Error::new_spanned(struct_name, message);
        let response = response.into_entry();
        let response_span = response.as_ref().map(|(_, span)| *span);
        let response = response
            .map(|(value, _)| value)
            .ok_or_else(|| error("visca attribute must have a response value"));
        let opcode = opcode
            .into_value()
            .ok_or_else(|| error("visca attribute must have an opcode value"));
        let parser = parser.into_entry();
        let custom = parse_with.into_entry();
        let data_variant = data_variant.into_entry();
        let typed_response = typed_response.into_entry();
        let typed_is_tuple = typed_is_tuple.into_entry();
        let typed_constructor = typed_constructor.into_entry();
        let mut rules = Ok(());
        let mut reject = |span: Span, message: &str| {
            rules = both(
                std::mem::replace(&mut rules, Ok(())),
                Err::<(), _>(syn::Error::new(span, message)),
            )
            .map(|_| ());
        };
        if let (None, Some(((), span))) = (&typed_response, typed_is_tuple) {
            reject(span, "typed_is_tuple requires typed_response");
        }
        match (&parser, &custom) {
            (None, Some((_, span))) => reject(*span, "parse_with requires parser = Custom"),
            (Some(((), span)), None) => reject(*span, "custom parser requires parse_with"),
            _ => {}
        }
        if let Some((_, span)) = &data_variant {
            if parser.is_none() || typed_response.is_none() {
                reject(
                    *span,
                    "data_variant requires parser = Custom and typed_response",
                );
            }
        }
        let raw = response.as_ref().is_ok_and(|value| value == "Raw");
        if raw && (parser.is_some() || typed_response.is_some()) {
            reject(response_span.unwrap_or_else(Span::call_site),
                "response = Raw does not support generated built-in parsers; implement ResponseParser manually");
        }
        let typed = match (typed_response, typed_field.into_entry()) {
            (Some((ty, _)), Some((fields, span))) => {
                let constructor =
                    typed_constructor.map_or(TypedConstructor::Identity, |(value, _)| value);
                if constructor != TypedConstructor::OkStruct && fields.len() != 1 {
                    reject(
                        span,
                        &match constructor.name() {
                            Some(name) => format!(
                                "typed_constructor `{name}` requires exactly one typed_field"
                            ),
                            None => {
                                "default typed response conversion requires exactly one typed_field"
                                    .to_owned()
                            }
                        },
                    );
                }
                Some(TypedResponse {
                    ty,
                    fields,
                    constructor,
                    is_tuple: typed_is_tuple.is_some(),
                })
            }
            (Some((_, span)), None) => {
                reject(span, "typed_response requires typed_field");
                None
            }
            (None, fields) => {
                if let Some((_, span)) = fields {
                    reject(span, "typed_field requires typed_response");
                }
                if let Some((_, span)) = typed_constructor {
                    reject(span, "typed_constructor requires typed_response");
                }
                None
            }
        };
        let ((response, opcode), ()) = both(both(response, opcode), rules)?;
        let subcode = subcode.into_value().unwrap_or(0x04);
        let kind = if raw {
            ResponseKind::Raw
        } else {
            ResponseKind::BuiltIn(Box::new(BuiltInResponse {
                variant: data_variant.map_or_else(|| response.clone(), |(value, _)| value),
                response,
                custom: custom.map(|(value, _)| value),
                typed,
            }))
        };
        Ok(InquirySpec {
            opcode,
            subcode,
            kind,
        })
    }
}

fn parse_inquiry_byte(literal: &LitInt, key: &str) -> syn::Result<u8> {
    let byte = parse_u8_literal(literal, &format!("`{key}`"))?;
    if byte == 0xFF {
        return Err(syn::Error::new_spanned(
            literal,
            format!("`{key}` must not be the VISCA terminator 0xFF"),
        ));
    }
    Ok(byte)
}

fn parse_ident_value(input: ParseStream<'_>, name: &str) -> syn::Result<Ident> {
    let ident: Ident = input.parse()?;
    if input.peek(Token![::]) {
        return Err(syn::Error::new(
            input.span(),
            format!("{name} must be a single identifier"),
        ));
    }
    Ok(ident)
}

fn parse_ident_list_value(input: ParseStream<'_>) -> syn::Result<Vec<Ident>> {
    if input.peek(syn::token::Paren) {
        let content;
        parenthesized!(content in input);
        let fields = Punctuated::<Ident, Token![,]>::parse_terminated(&content)?;
        if fields.is_empty() {
            return Err(syn::Error::new(
                content.span(),
                "typed_field must name at least one field",
            ));
        }
        return Ok(fields.into_iter().collect());
    }

    Ok(vec![parse_ident_value(input, "typed_field")?])
}

fn parse_typed_constructor(input: ParseStream<'_>) -> syn::Result<TypedConstructor> {
    let ident = parse_ident_value(input, "typed_constructor")?;
    let name = ident.to_string();
    TypedConstructor::ALL
        .iter()
        .find_map(|(candidate, constructor)| (*candidate == name).then_some(*constructor))
        .ok_or_else(|| syn::Error::new(ident.span(), format!("unknown typed_constructor `{name}`")))
}

fn parse_custom_parser(input: ParseStream<'_>) -> syn::Result<()> {
    let ident = parse_ident_value(input, "parser")?;
    if ident == "Custom" {
        Ok(())
    } else {
        Err(syn::Error::new(ident.span(), format!(
            "parser = {ident} is unsupported; remove parser to use the canonical inquiry decoder, or use parser = Custom with Payload::parse_bool or Nibbles::zero_extended_*"
        )))
    }
}

/// Emits the derive's output. `impl Request` and `impl Inquiry` are each
/// written once; the response kind only selects their varying parts.
fn generate(struct_name: &Ident, spec: &InquirySpec, crate_path: &TokenStream) -> TokenStream {
    let InquirySpec {
        opcode,
        subcode,
        kind,
    } = spec;
    let (response_type, decoder, inherent, typed_impl, profiled) = match kind {
        ResponseKind::Raw => (
            quote! { #crate_path::command::RawInquiryPayload },
            quote! {
                fn decode(payload: &[u8]) -> ::core::result::Result<#crate_path::command::RawInquiryPayload, #crate_path::Error> {
                    ::core::result::Result::Ok(#crate_path::command::RawInquiryPayload::from_slice(payload))
                }
                #crate_path::ResponseDecoder::from_fn(decode)
            },
            None,
            None,
            None,
        ),
        ResponseKind::BuiltIn(built_in) => {
            let BuiltInResponse {
                response,
                variant,
                custom,
                typed,
            } = built_in.as_ref();
            let body = if let Some(parse) = custom {
                quote! { #parse(payload).map(#crate_path::command::Response::Inquiry) }
            } else {
                quote! { #crate_path::command::parse_inquiry_payload(payload, &#crate_path::command::InquiryKind::#response) }
            };
            let inherent = quote! {
                impl #struct_name {
                    #[doc(hidden)]
                    fn __grafton_visca_decode_payload(payload: &[u8]) -> ::core::result::Result<#crate_path::command::Response, #crate_path::Error> {
                        (|| -> ::core::result::Result<#crate_path::command::Response, #crate_path::Error> { #body })()
                            .map_err(|error| #crate_path::__macro_support::inquiry_reply_error(error, payload))
                    }

                    /// Parses this inquiry's payload with the canonical table or explicit custom parser.
                    pub fn parse_response(&self, payload: &[u8]) -> ::core::result::Result<#crate_path::command::InquiryData, #crate_path::Error> {
                        match Self::__grafton_visca_decode_payload(payload)? {
                            #crate_path::command::Response::Inquiry(data) => ::core::result::Result::Ok(data),
                            #crate_path::command::Response::Error(error) => ::core::result::Result::Err(error),
                            _ => ::core::result::Result::Err(#crate_path::Error::UnexpectedResponseType),
                        }
                    }
                }
            };
            let response_type = typed.as_ref().map_or_else(
                || quote! { #crate_path::command::Response },
                |typed| {
                    let ty = &typed.ty;
                    quote! { #ty }
                },
            );
            let typed_impl = typed.as_ref().map(|typed| {
                generate_typed_impl(struct_name, variant, typed, &response_type, crate_path)
            });
            let lift = if typed.is_some() {
                quote! { <#struct_name as #crate_path::command::ResponseParser>::from_response(response) }
            } else {
                quote! { ::core::result::Result::Ok(response) }
            };
            let decoder = quote! {
                fn decode(payload: &[u8]) -> ::core::result::Result<#response_type, #crate_path::Error> {
                    let response = #struct_name::__grafton_visca_decode_payload(payload)?;
                    (|| -> ::core::result::Result<#response_type, #crate_path::Error> { #lift })()
                        .map_err(|error| #crate_path::__macro_support::inquiry_reply_error(error, payload))
                }
                #crate_path::ResponseDecoder::from_fn(decode)
            };
            let profiled = custom.is_none().then(|| quote! {
                fn decoder_for_profile(&self, profile: &#crate_path::ProfileSpec) -> #crate_path::ResponseDecoder<Self::Response> {
                    fn lift(response: #crate_path::command::Response) -> ::core::result::Result<#response_type, #crate_path::Error> { #lift }
                    #crate_path::__macro_support::inquiry_decoder(#crate_path::command::InquiryKind::#response, profile, lift)
                }
            });
            (response_type, decoder, Some(inherent), typed_impl, profiled)
        }
    };
    quote! {
        #inherent
        #typed_impl
        impl #struct_name {
            #[doc(hidden)]
            const __GRAFTON_VISCA_INQUIRY_BODY: [u8; 3] = [#crate_path::__macro_support::INQUIRY, #subcode, #opcode];
        }
        impl #crate_path::Request for #struct_name {
            type Class = #crate_path::request::Inquiry;
            const MAX_SIZE: usize = #crate_path::__macro_support::frame_len(&Self::__GRAFTON_VISCA_INQUIRY_BODY);
            const TIMEOUT_CLASS: #crate_path::TimeoutClass = #crate_path::TimeoutClass::Inquiry;
            const RETRY_CLASS: #crate_path::RetryClass = #crate_path::RetryClass::Inquiry;
            const CONTROL_CLASS: #crate_path::ControlClass = #crate_path::ControlClass::Normal;
            fn write_into(&self, camera_id: #crate_path::CameraId, buffer: &mut [u8]) -> ::core::result::Result<usize, #crate_path::Error> {
                #crate_path::__macro_support::write_frame(camera_id, &[&Self::__GRAFTON_VISCA_INQUIRY_BODY], buffer)
            }
        }
        impl #crate_path::Inquiry for #struct_name {
            type Response = #response_type;
            fn decoder(&self) -> #crate_path::ResponseDecoder<Self::Response> { #decoder }
            #profiled
        }
    }
}

/// Generates the `command::ResponseParser` impl for a typed response.
fn generate_typed_impl(
    struct_name: &Ident,
    variant: &Ident,
    typed: &TypedResponse,
    response_type: &TokenStream,
    crate_path: &TokenStream,
) -> TokenStream {
    let TypedResponse {
        fields,
        constructor,
        is_tuple,
        ..
    } = typed;

    let destructure = if *is_tuple {
        quote! { #crate_path::command::InquiryData::#variant(#(#fields),*) }
    } else {
        quote! { #crate_path::command::InquiryData::#variant { #(#fields),* } }
    };

    let first = &fields[0];
    let construction = match constructor {
        TypedConstructor::Identity => quote! { ::core::result::Result::Ok(#first) },
        TypedConstructor::New => quote! { #response_type::new(#first) },
        TypedConstructor::OkNew => {
            quote! { ::core::result::Result::Ok(#response_type::new(#first)) }
        }
        TypedConstructor::NewU8 => quote! {
            {
                let value = <::core::primitive::u8 as ::core::convert::TryFrom<_>>::try_from(#first)
                    .map_err(|_| #crate_path::Error::invalid_response(
                        "typed response value fitting u8", ::std::vec::Vec::new(),
                    ))?;
                #response_type::new(value)
            }
        },
        TypedConstructor::OkStruct => {
            quote! { ::core::result::Result::Ok(#response_type { #(#fields),* }) }
        }
    };

    quote! {
        impl #crate_path::command::ResponseParser for #struct_name {
            type Response = #response_type;

            fn from_response(
                resp: #crate_path::command::Response,
            ) -> ::core::result::Result<Self::Response, #crate_path::Error> {
                (match resp {
                    #crate_path::command::Response::Inquiry(#destructure) => #construction,
                    #crate_path::command::Response::Error(e) => ::core::result::Result::Err(e),
                    _ => ::core::result::Result::Err(#crate_path::Error::UnexpectedResponseType),
                }).map_err(|error| {
                    #crate_path::__macro_support::inquiry_reply_error(error, &[])
                })
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn expanded(input: DeriveInput) -> String {
        match parse_visca_attributes_from_struct(&input)
            .and_then(|attrs| attrs.validate(&input.ident))
        {
            Ok(spec) => generate(&input.ident, &spec, &quote! { ::grafton_visca }).to_string(),
            Err(error) => error.to_compile_error().to_string(),
        }
    }

    #[test]
    fn raw_canonical_and_typed_requests_emit_one_request_and_decoder() {
        for input in [
            syn::parse_quote! { #[visca(opcode = 0x7e, response = Raw)] struct Raw; },
            syn::parse_quote! { #[visca(opcode = 0, response = Power)] struct Canonical; },
            syn::parse_quote! { #[visca(opcode = 0, response = Power, typed_response = LocalStatus, typed_field = status)] struct Typed; },
        ] {
            let tokens = expanded(input);
            assert_eq!(tokens.matches(":: Request for").count(), 1, "{tokens}");
            assert_eq!(tokens.matches(":: Inquiry for").count(), 1, "{tokens}");
            assert!(
                tokens.contains("__macro_support :: write_frame"),
                "{tokens}"
            );
            assert!(!tokens.contains("compile_error"), "{tokens}");
        }
    }

    #[test]
    fn canonical_decoders_emit_profile_support_and_preserve_local_type_paths() {
        let tokens = expanded(syn::parse_quote! {
            #[visca(opcode = 0x12, response = PanTiltPosition, typed_response = LocalPosition, typed_field = pan)]
            struct Probe;
        });
        assert!(tokens.contains("fn decoder_for_profile"), "{tokens}");
        assert!(
            tokens.contains("__macro_support :: inquiry_decoder"),
            "{tokens}"
        );
        assert!(tokens.contains("type Response = LocalPosition"), "{tokens}");
        assert!(
            !tokens.contains("grafton_visca :: LocalPosition"),
            "{tokens}"
        );
    }

    #[test]
    fn every_removed_selector_names_the_canonical_and_custom_replacements() {
        for parser in [
            "Bool",
            "DirectByte",
            "Byte",
            "Position",
            "ExtendedNibble",
            "Nibble",
            "Flags",
            "BitFlags",
            "Mode",
            "ModeEnum",
            "PanTilt",
            "BoolConvention",
            "LastNibble",
            "TallyStatus",
            "SharpnessMode",
            "Gamma",
            "AutoWbSensitivity",
            "NdFilter",
            "PictureEffect",
            "DefogLevel",
            "FocusRange",
        ] {
            let input = syn::parse_str(&format!(
                "#[visca(opcode = 0, response = Power, parser = {parser})] struct Probe;"
            ))
            .unwrap();
            let tokens = expanded(input);
            assert!(
                tokens.contains("remove parser to use the canonical inquiry decoder"),
                "{parser}: {tokens}"
            );
            assert!(tokens.contains("parser = Custom"), "{parser}: {tokens}");
        }
    }

    #[test]
    fn custom_parser_paths_are_preserved_without_profile_overrides() {
        let tokens = expanded(syn::parse_quote! {
            #[visca(opcode = 0, response = Power, parser = Custom, parse_with = crate::parsers::power)] struct Probe;
        });
        assert!(
            tokens.contains("crate :: parsers :: power (payload)"),
            "{tokens}"
        );
        assert!(!tokens.contains("fn decoder_for_profile"), "{tokens}");
    }

    #[test]
    fn narrowing_is_checked_and_uses_absolute_primitive_and_trait_paths() {
        let tokens = expanded(syn::parse_quote! {
            #[visca(opcode = 0, response = Iris, typed_response = LocalLevel, typed_field = position, typed_constructor = new_u8)] struct Probe;
        });
        assert!(
            tokens.contains(":: core :: primitive :: u8 as :: core :: convert :: TryFrom"),
            "{tokens}"
        );
        assert!(!tokens.contains("as u8"), "{tokens}");
    }

    #[test]
    fn invalid_key_combinations_are_reported_at_the_attributes() {
        for (input, expected) in [
            (
                syn::parse_quote! { #[visca(opcode = 0, response = Power, parser = Custom)] struct Probe; },
                "custom parser requires parse_with",
            ),
            (
                syn::parse_quote! { #[visca(opcode = 0, response = Power, parse_with = custom)] struct Probe; },
                "parse_with requires parser = Custom",
            ),
            (
                syn::parse_quote! { #[visca(opcode = 0, response = Power, typed_is_tuple)] struct Probe; },
                "typed_is_tuple requires typed_response",
            ),
            (
                syn::parse_quote! { #[visca(opcode = 0, response = Power, typed_response = u8)] struct Probe; },
                "typed_response requires typed_field",
            ),
            (
                syn::parse_quote! { #[visca(opcode = 0, response = Power, typed_field = status)] struct Probe; },
                "typed_field requires typed_response",
            ),
            (
                syn::parse_quote! { #[visca(opcode = 0, response = Power, typed_constructor = new)] struct Probe; },
                "typed_constructor requires typed_response",
            ),
            (
                syn::parse_quote! { #[visca(opcode = 0, response = Power, parser = Custom, parse_with = custom, data_variant = Digital)] struct Probe; },
                "data_variant requires parser = Custom and typed_response",
            ),
            (
                syn::parse_quote! { #[visca(opcode = 0, response = Raw, parser = Custom, parse_with = custom)] struct Probe; },
                "response = Raw does not support",
            ),
        ] {
            let tokens = expanded(input);
            assert!(tokens.contains(expected), "{expected}: {tokens}");
        }
    }

    #[test]
    fn attribute_bytes_reject_terminators_suffixes_and_out_of_range_literals() {
        for options in [
            "opcode = 0xff",
            "opcode = 256",
            "opcode = 0u8",
            "opcode = 0, subcode = 0xff",
        ] {
            let tokens = expanded(
                syn::parse_str(&format!(
                    "#[visca({options}, response = Power)] struct Probe;"
                ))
                .unwrap(),
            );
            assert!(tokens.contains("compile_error"), "{tokens}");
        }
        for byte in ["0x47", "0o107", "0b0100_0111", "71"] {
            let tokens = expanded(
                syn::parse_str(&format!(
                    "#[visca(opcode = {byte}, response = Power)] struct Probe;"
                ))
                .unwrap(),
            );
            assert!(!tokens.contains("compile_error"), "{tokens}");
        }
    }

    #[test]
    fn repeated_keys_and_missing_required_attributes_are_rejected() {
        for input in [
            syn::parse_quote! { struct Probe; },
            syn::parse_quote! { #[visca(opcode = 0)] struct Probe; },
            syn::parse_quote! { #[visca(response = Power)] struct Probe; },
            syn::parse_quote! { #[visca(opcode = 0, response = Power)] #[visca(opcode = 1)] struct Probe; },
        ] {
            assert!(expanded(input).contains("compile_error"));
        }
    }
}
