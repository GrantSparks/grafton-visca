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
use quote::{format_ident, quote};
use syn::{
    meta::ParseNestedMeta, parenthesized, parse::ParseStream, punctuated::Punctuated,
    spanned::Spanned, DeriveInput, Ident, LitInt, Path, Token, Type,
};

use crate::attr::{parse_u8_literal, unknown_key, AttrSlot};

const ATTRIBUTE: &str = "visca";
const ATTRIBUTE_KEYS: &str = "one of `opcode`, `subcode`, `response`, `parser`, `field`, \
                              `value_type`, `parse_with`, `convention`, `data_variant`, \
                              `typed_response`, `typed_field`, `typed_constructor`, or \
                              `typed_is_tuple`";

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
    parser: AttrSlot<ParserStrategy>,
    field: AttrSlot<Ident>,
    value_type: AttrSlot<Type>,
    parse_with: AttrSlot<Path>,
    convention: AttrSlot<Path>,
    data_variant: AttrSlot<Ident>,
    typed_response: AttrSlot<Type>,
    typed_field: AttrSlot<Vec<Ident>>,
    typed_constructor: AttrSlot<TypedConstructor>,
    typed_is_tuple: AttrSlot<()>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ParserStrategy {
    Bool,
    DirectByte,
    Byte,
    Position,
    ExtendedNibble,
    Nibble,
    Flags,
    BitFlags,
    Mode,
    ModeEnum,
    PanTilt,
    BoolConvention,
    LastNibble,
    TallyStatus,
    SharpnessMode,
    Gamma,
    AutoWbSensitivity,
    NdFilter,
    PictureEffect,
    DefogLevel,
    FocusRange,
    Custom,
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
    /// `Type::new(field as u8)`.
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

/// A built-in `InquiryKind`, optionally decoded by a selector and lifted into
/// a typed response.
struct BuiltInResponse {
    response: Ident,
    /// The `InquiryData` variant a selector builds and a typed response
    /// destructures: `data_variant` with a selector, else `response`.
    variant: Ident,
    decoder: Option<PayloadDecoder>,
    typed: Option<TypedResponse>,
}

/// A parser selector with every attribute it requires.
enum PayloadDecoder {
    /// Selectors that only name the built-in table shape.
    Canonical,
    Custom(Path),
    BoolConvention {
        field: Ident,
        convention: Path,
    },
    Nibble {
        field: Ident,
    },
    Mode {
        value_type: Type,
    },
    LastNibble {
        field: Ident,
    },
    NdFilter,
    PictureEffect,
    DefogLevel,
    FocusRange,
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
            let byte = parse_u8_literal(&meta.value()?.parse::<LitInt>()?, "`opcode`")?;
            self.opcode.set(ATTRIBUTE, meta, byte)
        } else if path.is_ident("subcode") {
            let byte = parse_u8_literal(&meta.value()?.parse::<LitInt>()?, "`subcode`")?;
            self.subcode.set(ATTRIBUTE, meta, byte)
        } else if path.is_ident("response") {
            let response = parse_ident_value(meta.value()?, "response")?;
            self.response.set(ATTRIBUTE, meta, response)
        } else if path.is_ident("parser") {
            let parser = parse_parser_strategy(meta.value()?)?;
            self.parser.set(ATTRIBUTE, meta, parser)
        } else if path.is_ident("field") {
            let field = parse_ident_value(meta.value()?, "field")?;
            self.field.set(ATTRIBUTE, meta, field)
        } else if path.is_ident("value_type") {
            let value_type = meta.value()?.parse()?;
            self.value_type.set(ATTRIBUTE, meta, value_type)
        } else if path.is_ident("parse_with") {
            let parse_with = meta.value()?.parse()?;
            self.parse_with.set(ATTRIBUTE, meta, parse_with)
        } else if path.is_ident("convention") {
            let convention: Path = meta.value()?.parse()?;
            validate_bool_convention(&convention)?;
            self.convention.set(ATTRIBUTE, meta, convention)
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

    /// The one place cross-key rules are checked. Every violation found is
    /// reported together.
    fn validate(self, struct_name: &Ident) -> syn::Result<InquirySpec> {
        let Self {
            opcode,
            subcode,
            response,
            parser,
            field,
            value_type,
            parse_with,
            convention,
            data_variant,
            typed_response,
            typed_field,
            typed_constructor,
            typed_is_tuple,
        } = self;
        // A missing required key is reported on the struct; a rule between
        // keys is reported on the key that breaks it.
        let error = |message: &str| syn::Error::new_spanned(struct_name, message);
        let at = |span: Span, message: &str| syn::Error::new(span, message);

        let response = response.into_entry();
        let response_span = response.as_ref().map(|(_, span)| *span);
        let response = response
            .map(|(response, _)| response)
            .ok_or_else(|| error("visca attribute must have a response value"));
        let opcode = opcode
            .into_value()
            .ok_or_else(|| error("visca attribute must have an opcode value"));
        let parser = parser.into_entry();
        let typed_response = typed_response.into_entry();

        let raw = response.as_ref().is_ok_and(|response| response == "Raw");
        let raw_conflict = match (raw, response_span) {
            (true, Some(span)) if parser.is_some() || typed_response.is_some() => Err(at(
                span,
                "response = Raw does not support generated built-in parsers; implement \
                 ResponseParser manually",
            )),
            _ => Ok(()),
        };

        let field = field.into_value();
        let parser_span = parser.as_ref().map(|(_, span)| *span);
        let parser = parser.map(|(parser, _)| parser);
        let decoder = parser
            .zip(parser_span)
            .map(|(parser, span)| -> syn::Result<PayloadDecoder> {
                let error = |message: &str| at(span, message);
                Ok(match parser {
                    ParserStrategy::Custom => PayloadDecoder::Custom(
                        parse_with
                            .into_value()
                            .ok_or_else(|| error("custom parser requires parse_with"))?,
                    ),
                    ParserStrategy::BoolConvention => {
                        let (field, convention) = both(
                            field
                                .clone()
                                .ok_or_else(|| error("bool_convention parser requires field")),
                            convention
                                .into_value()
                                .ok_or_else(|| error("bool_convention parser requires convention")),
                        )?;
                        PayloadDecoder::BoolConvention { field, convention }
                    }
                    ParserStrategy::ExtendedNibble | ParserStrategy::Nibble => {
                        PayloadDecoder::Nibble {
                            field: field.clone().unwrap_or_else(|| format_ident!("value")),
                        }
                    }
                    ParserStrategy::Mode | ParserStrategy::ModeEnum => PayloadDecoder::Mode {
                        value_type: value_type
                            .into_value()
                            .ok_or_else(|| error("mode parser requires value_type"))?,
                    },
                    ParserStrategy::LastNibble => PayloadDecoder::LastNibble {
                        field: field
                            .clone()
                            .ok_or_else(|| error("last_nibble parser requires field"))?,
                    },
                    ParserStrategy::NdFilter => PayloadDecoder::NdFilter,
                    ParserStrategy::PictureEffect => PayloadDecoder::PictureEffect,
                    ParserStrategy::DefogLevel => PayloadDecoder::DefogLevel,
                    ParserStrategy::FocusRange => PayloadDecoder::FocusRange,
                    ParserStrategy::Bool
                    | ParserStrategy::DirectByte
                    | ParserStrategy::Byte
                    | ParserStrategy::Position
                    | ParserStrategy::Flags
                    | ParserStrategy::BitFlags
                    | ParserStrategy::PanTilt
                    | ParserStrategy::TallyStatus
                    | ParserStrategy::SharpnessMode
                    | ParserStrategy::Gamma
                    | ParserStrategy::AutoWbSensitivity => PayloadDecoder::Canonical,
                })
            })
            .transpose();

        let typed_constructor = typed_constructor.into_entry();
        let typed =
            match (typed_response, typed_field.into_entry()) {
                (Some((ty, _)), Some((fields, fields_span))) => {
                    let constructor = typed_constructor
                        .map_or(TypedConstructor::Identity, |(constructor, _)| constructor);
                    if constructor == TypedConstructor::OkStruct || fields.len() == 1 {
                    Ok(())
                } else {
                    Err(at(fields_span, &match constructor.name() {
                        Some(name) => {
                            format!("typed_constructor `{name}` requires exactly one typed_field")
                        }
                        None => "default typed response conversion requires exactly one \
                                 typed_field"
                            .to_owned(),
                    }))
                }
                .map(|()| {
                    Some(TypedResponse {
                        ty,
                        fields,
                        constructor,
                        is_tuple: typed_is_tuple.get().is_some(),
                    })
                })
                }
                (Some((_, span)), None) => Err(at(span, "typed_response requires typed_field")),
                (None, fields) => both(
                    match fields {
                        Some((_, span)) => Err(at(span, "typed_field requires typed_response")),
                        None => Ok(()),
                    },
                    match typed_constructor {
                        Some((_, span)) => {
                            Err(at(span, "typed_constructor requires typed_response"))
                        }
                        None => Ok(()),
                    },
                )
                .map(|_| None),
            };

        let ((((response, opcode), decoder), typed), ()) = both(
            both(both(both(response, opcode), decoder), typed),
            raw_conflict,
        )?;
        let subcode = subcode.into_value().unwrap_or(0x04);
        if raw {
            return Ok(InquirySpec {
                opcode,
                subcode,
                kind: ResponseKind::Raw,
            });
        }
        let variant = parser
            .and(data_variant.into_value())
            .unwrap_or_else(|| response.clone());
        Ok(InquirySpec {
            opcode,
            subcode,
            kind: ResponseKind::BuiltIn(Box::new(BuiltInResponse {
                response,
                variant,
                decoder,
                typed,
            })),
        })
    }
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

fn parse_parser_strategy(input: ParseStream<'_>) -> syn::Result<ParserStrategy> {
    let ident = parse_ident_value(input, "parser")?;
    let strategy = match ident.to_string().as_str() {
        "Bool" => ParserStrategy::Bool,
        "DirectByte" => ParserStrategy::DirectByte,
        "Byte" => ParserStrategy::Byte,
        "Position" => ParserStrategy::Position,
        "ExtendedNibble" => ParserStrategy::ExtendedNibble,
        "Nibble" => ParserStrategy::Nibble,
        "Flags" => ParserStrategy::Flags,
        "BitFlags" => ParserStrategy::BitFlags,
        "Mode" => ParserStrategy::Mode,
        "ModeEnum" => ParserStrategy::ModeEnum,
        "PanTilt" => ParserStrategy::PanTilt,
        "BoolConvention" => ParserStrategy::BoolConvention,
        "LastNibble" => ParserStrategy::LastNibble,
        "TallyStatus" => ParserStrategy::TallyStatus,
        "SharpnessMode" => ParserStrategy::SharpnessMode,
        "Gamma" => ParserStrategy::Gamma,
        "AutoWbSensitivity" => ParserStrategy::AutoWbSensitivity,
        "NdFilter" => ParserStrategy::NdFilter,
        "PictureEffect" => ParserStrategy::PictureEffect,
        "DefogLevel" => ParserStrategy::DefogLevel,
        "FocusRange" => ParserStrategy::FocusRange,
        "Custom" => ParserStrategy::Custom,
        unknown => {
            return Err(syn::Error::new(
                ident.span(),
                format!("unknown parser strategy `{unknown}`"),
            ))
        }
    };
    Ok(strategy)
}

fn validate_bool_convention(convention: &Path) -> syn::Result<()> {
    let Some(variant) = convention.segments.last() else {
        return Err(syn::Error::new(
            convention.span(),
            "convention must not be empty",
        ));
    };

    match variant.ident.to_string().as_str() {
        "OnIs02" | "OnIs03" => Ok(()),
        other => Err(syn::Error::new(
            variant.ident.span(),
            format!("unknown bool convention `{other}`"),
        )),
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

    let (response_type, route, decoder, inherent, typed_impl) = match kind {
        ResponseKind::Raw => (
            quote! { #crate_path::command::RawInquiryPayload },
            quote! { #crate_path::InquiryRoute::RAW },
            quote! {
                fn decode(
                    payload: &[u8],
                ) -> ::core::result::Result<#crate_path::command::RawInquiryPayload, #crate_path::Error> {
                    ::core::result::Result::Ok(#crate_path::command::RawInquiryPayload::from_slice(payload))
                }
                #crate_path::ResponseDecoder::from_fn(decode)
            },
            None,
            None,
        ),
        ResponseKind::BuiltIn(built_in) => {
            let BuiltInResponse {
                response,
                variant,
                decoder,
                typed,
            } = built_in.as_ref();
            let decode_body = match decoder {
                Some(decoder) => selector_decode_body(response, variant, decoder, crate_path),
                None => canonical_decode(response, crate_path),
            };
            let parse_response = decoder.is_some().then(|| {
                quote! {
                    /// Parses this inquiry's payload with the shared decoder.
                    ///
                    /// This is the exact decoder used by `Inquiry::decoder()`.
                    /// Built-in selector forms use the canonical `InquiryKind`
                    /// table; explicit selector overrides use their declared
                    /// parser on both public paths.
                    pub fn parse_response(&self, data: &[u8]) -> ::core::result::Result<#crate_path::command::InquiryData, #crate_path::Error> {
                        match Self::__grafton_visca_decode_payload(data)? {
                            #crate_path::command::Response::Inquiry(response) => {
                                ::core::result::Result::Ok(response)
                            }
                            #crate_path::command::Response::Error(error) => {
                                ::core::result::Result::Err(error)
                            }
                            _ => ::core::result::Result::Err(
                                #crate_path::Error::UnexpectedResponseType
                            ),
                        }
                    }
                }
            });
            // One payload decoder per derive: `Inquiry::decoder`, the typed
            // conversion and `parse_response` all call it.
            let inherent = quote! {
                impl #struct_name {
                    #[doc(hidden)]
                    fn __grafton_visca_decode_payload(
                        payload: &[u8],
                    ) -> ::core::result::Result<#crate_path::command::Response, #crate_path::Error> {
                        #decode_body
                    }

                    #parse_response
                }
            };
            let route = quote! {
                #crate_path::InquiryRoute::custom(
                    #crate_path::command::InquiryKind::#response as u16 + 1,
                )
            };
            match typed {
                Some(typed) => {
                    let response_type = type_tokens(&typed.ty, crate_path);
                    (
                        response_type.clone(),
                        route,
                        quote! {
                            fn decode(
                                payload: &[u8],
                            ) -> ::core::result::Result<#response_type, #crate_path::Error> {
                                <#struct_name as #crate_path::command::ResponseParser>::from_response(
                                    #struct_name::__grafton_visca_decode_payload(payload)?,
                                )
                            }
                            #crate_path::ResponseDecoder::from_fn(decode)
                        },
                        Some(inherent),
                        Some(generate_typed_impl(
                            struct_name,
                            variant,
                            typed,
                            &response_type,
                            crate_path,
                        )),
                    )
                }
                None => (
                    quote! { #crate_path::command::Response },
                    route,
                    quote! {
                        #crate_path::ResponseDecoder::from_fn(
                            #struct_name::__grafton_visca_decode_payload
                        )
                    },
                    Some(inherent),
                    None,
                ),
            }
        }
    };

    quote! {
        #inherent

        #typed_impl

        impl #struct_name {
            #[doc(hidden)]
            const __GRAFTON_VISCA_INQUIRY_BODY: [u8; 3] =
                [#crate_path::__macro_support::INQUIRY, #subcode, #opcode];
        }

        impl #crate_path::Request for #struct_name {
            type Class = #crate_path::request::Inquiry;

            const MAX_SIZE: usize =
                #crate_path::__macro_support::frame_len(&Self::__GRAFTON_VISCA_INQUIRY_BODY);
            const TIMEOUT_CLASS: #crate_path::TimeoutClass = #crate_path::TimeoutClass::Inquiry;
            const RETRY_CLASS: #crate_path::RetryClass = #crate_path::RetryClass::Inquiry;
            const CONTROL_CLASS: #crate_path::ControlClass = #crate_path::ControlClass::Normal;

            fn write_into(
                &self,
                camera_id: #crate_path::CameraId,
                buffer: &mut [u8],
            ) -> ::core::result::Result<usize, #crate_path::EncodeError> {
                #crate_path::__macro_support::write_frame(
                    camera_id,
                    &[&Self::__GRAFTON_VISCA_INQUIRY_BODY],
                    buffer,
                )
            }
        }

        impl #crate_path::Inquiry for #struct_name {
            type Response = #response_type;

            fn route(&self) -> #crate_path::InquiryRoute {
                #route
            }

            fn decoder(&self) -> #crate_path::ResponseDecoder<Self::Response> {
                #decoder
            }
        }
    }
}

/// The canonical table decoder for `response`.
fn canonical_decode(response: &Ident, crate_path: &TokenStream) -> TokenStream {
    quote! {
        #crate_path::command::parse_inquiry_payload(
            payload,
            &#crate_path::command::InquiryKind::#response,
        )
    }
}

/// The payload decoder for a derive carrying a parser selector.
///
/// Shape selectors name a built-in response shape and decode with the
/// authoritative inquiry table. Transforming selectors carry their own
/// decoding here; each uses the public payload helper rather than restating a
/// wire convention.
fn selector_decode_body(
    response: &Ident,
    variant: &Ident,
    decoder: &PayloadDecoder,
    crate_path: &TokenStream,
) -> TokenStream {
    // The one-byte selectors share this guard.
    let single_byte = quote! {
        let &[byte] = payload else {
            return ::core::result::Result::Err(
                #crate_path::Error::invalid_response_length(1, payload)
            );
        };
    };

    let data = match decoder {
        PayloadDecoder::Canonical => return canonical_decode(response, crate_path),
        PayloadDecoder::Custom(parse_with) => quote! { #parse_with(payload) },
        PayloadDecoder::BoolConvention { field, convention } => {
            let convention = bool_convention_tokens(convention, crate_path);
            let parameter = field.to_string();
            quote! {
                {
                    let payload = #crate_path::command::Payload::new(payload);
                    let #field = payload.parse_bool(#parameter, #convention)?;
                    ::core::result::Result::Ok(
                        #crate_path::command::InquiryData::#variant { #field }
                    )
                }
            }
        }
        PayloadDecoder::Nibble { field } => quote! {
            {
                let payload = #crate_path::command::Payload::new(payload);
                let nibbles = <#crate_path::command::Nibbles<'_, 2> as
                    ::core::convert::TryFrom<#crate_path::command::Payload<'_>>>::try_from(payload)?;
                ::core::result::Result::Ok(
                    #crate_path::command::InquiryData::#response {
                        #field: nibbles.zero_extended_u8()?,
                    }
                )
            }
        },
        PayloadDecoder::Mode { value_type } => {
            let value_type = command_type_tokens(value_type, crate_path);
            let field = match response.to_string().as_str() {
                "FocusZone" => quote! { zone },
                "AutoFocusSensitivity" => quote! { sensitivity },
                _ => quote! { mode },
            };
            quote! {
                {
                    #single_byte
                    // The enum's own `TryFrom` error is the crate's one
                    // unknown-code error (`InvalidResponse`), so it passes
                    // through unchanged.
                    let value = <#value_type as ::core::convert::TryFrom<u8>>::try_from(byte)?;
                    ::core::result::Result::Ok(
                        #crate_path::command::InquiryData::#response { #field: value }
                    )
                }
            }
        }
        PayloadDecoder::LastNibble { field } => quote! {
            {
                let payload = #crate_path::command::Payload::new(payload);
                let nibbles = #crate_path::command::Nibbles::<4>::try_from(payload)?;
                ::core::result::Result::Ok(
                    #crate_path::command::InquiryData::#variant {
                        #field: nibbles.zero_extended_nibble()?,
                    }
                )
            }
        },
        PayloadDecoder::NdFilter => quote! {
            {
                #single_byte
                let position = #crate_path::command::NdFilterPosition::from_byte(byte);
                ::core::result::Result::Ok(
                    #crate_path::command::InquiryData::#variant { position }
                )
            }
        },
        PayloadDecoder::PictureEffect => quote! {
            {
                #single_byte
                let effect = #crate_path::command::PictureEffectMode::from_byte(byte);
                ::core::result::Result::Ok(
                    #crate_path::command::InquiryData::#variant { effect }
                )
            }
        },
        PayloadDecoder::DefogLevel => quote! {
            {
                #single_byte
                let level = #crate_path::types::DefogLevel::new(byte)?;
                ::core::result::Result::Ok(
                    #crate_path::command::InquiryData::#variant { level }
                )
            }
        },
        PayloadDecoder::FocusRange => quote! {
            {
                #single_byte
                let range = #crate_path::command::FocusRange::try_from(byte)?;
                ::core::result::Result::Ok(
                    #crate_path::command::InquiryData::#variant { range }
                )
            }
        },
    };

    quote! {
        (#data).map(#crate_path::command::Response::Inquiry)
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
        TypedConstructor::NewU8 => quote! { #response_type::new(#first as u8) },
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
                match resp {
                    #crate_path::command::Response::Inquiry(#destructure) => #construction,
                    #crate_path::command::Response::Error(e) => ::core::result::Result::Err(e),
                    _ => ::core::result::Result::Err(#crate_path::Error::UnexpectedResponseType),
                }
            }
        }
    }
}

fn command_type_tokens(ty: &Type, crate_path: &TokenStream) -> TokenStream {
    if let Type::Path(type_path) = ty {
        let path = &type_path.path;
        if is_explicit_path(path) {
            quote! { #ty }
        } else if path.segments.len() == 1 {
            quote! { #crate_path::command::#path }
        } else if path
            .segments
            .first()
            .map(|segment| segment.ident == "command")
            .unwrap_or(false)
        {
            quote! { #crate_path::#path }
        } else {
            quote! { #ty }
        }
    } else {
        quote! { #ty }
    }
}

fn bool_convention_tokens(path: &Path, crate_path: &TokenStream) -> TokenStream {
    if is_explicit_path(path) {
        quote! { #path }
    } else if path.segments.len() == 1 {
        quote! { #crate_path::command::BoolConvention::#path }
    } else if path
        .segments
        .first()
        .map(|segment| segment.ident == "BoolConvention")
        .unwrap_or(false)
    {
        quote! { #crate_path::command::#path }
    } else {
        quote! { #path }
    }
}

fn type_tokens(ty: &Type, crate_path: &TokenStream) -> TokenStream {
    if let Type::Path(type_path) = ty {
        let path = &type_path.path;
        if is_primitive_path(path) || is_explicit_path(path) {
            quote! { #ty }
        } else {
            quote! { #crate_path::#path }
        }
    } else {
        quote! { #ty }
    }
}

fn is_primitive_path(path: &Path) -> bool {
    path.segments.len() == 1
        && matches!(
            path.segments[0].ident.to_string().as_str(),
            "bool" | "u8" | "u16" | "u32" | "i8" | "i16" | "i32"
        )
}

fn is_explicit_path(path: &Path) -> bool {
    path.leading_colon.is_some()
        || path
            .segments
            .first()
            .map(|segment| {
                matches!(
                    segment.ident.to_string().as_str(),
                    "crate" | "self" | "super"
                )
            })
            .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn downstream_inquiry_encoding_uses_the_shared_frame_writer() {
        let input: DeriveInput = syn::parse_quote! {
            #[visca(opcode = 0x47, response = ZoomPosition)]
            struct CustomZoomInquiry;
        };

        let tokens = derive_visca_inquiry_impl(input).to_string();

        assert!(
            !tokens.contains("vec !"),
            "generated inquiry encoder must not allocate with vec!: {tokens}"
        );
        assert!(
            !tokens.contains("to_vec"),
            "generated inquiry encoder must not allocate with to_vec(): {tokens}"
        );
        assert!(
            !tokens.contains("0xFF") && !tokens.contains("VISCA_TERMINATOR"),
            "generated downstream inquiry encoder must not write its own terminator: {tokens}"
        );
        assert!(
            tokens.contains("__macro_support :: write_frame")
                && tokens.contains("__macro_support :: INQUIRY"),
            "generated downstream inquiry encoder must use the crate's frame writer and inquiry category: {tokens}"
        );
    }

    #[test]
    fn legacy_string_attribute_syntax_is_rejected() {
        let input: DeriveInput = syn::parse_quote! {
            #[visca(opcode = 0x00, response = "Power", parser = "bool")]
            struct LegacyStringInquiry;
        };

        let tokens = derive_visca_inquiry_impl(input).to_string();

        assert!(
            tokens.contains("compile_error"),
            "legacy string syntax must fail during macro expansion: {tokens}"
        );
    }

    #[test]
    fn lowercase_parser_strategy_is_rejected() {
        let input: DeriveInput = syn::parse_quote! {
            #[visca(opcode = 0x00, response = Power, parser = bool)]
            struct LowercaseParserInquiry;
        };

        let tokens = derive_visca_inquiry_impl(input).to_string();

        assert!(
            tokens.contains("unknown parser strategy"),
            "lowercase parser aliases must not be accepted: {tokens}"
        );
    }

    #[test]
    fn legacy_attribute_key_aliases_are_rejected() {
        let inputs: [DeriveInput; 4] = [
            syn::parse_quote! {
                #[visca(command = 0x00, response = Power)]
                struct LegacyCommandKeyInquiry;
            },
            syn::parse_quote! {
                #[visca(opcode = 0x00, sub_command = 0x04, response = Power)]
                struct LegacySubCommandKeyInquiry;
            },
            syn::parse_quote! {
                #[visca(opcode = 0x00, response = Power, parser = Custom, custom_fn = crate::parse)]
                struct LegacyCustomFnKeyInquiry;
            },
            syn::parse_quote! {
                #[visca(opcode = 0x00, response = Power, inquiry_variant = Power)]
                struct LegacyInquiryVariantKeyInquiry;
            },
        ];

        for (input, key) in
            inputs
                .into_iter()
                .zip(["command", "sub_command", "custom_fn", "inquiry_variant"])
        {
            let tokens = derive_visca_inquiry_impl(input).to_string();
            assert!(
                tokens.contains(&format!(
                    "unknown `visca` attribute `{key}`; expected one of"
                )),
                "legacy key aliases must not be accepted: {tokens}"
            );
        }
    }

    fn expanded(input: DeriveInput) -> String {
        derive_visca_inquiry_impl(input).to_string()
    }

    #[test]
    fn every_integer_literal_radix_is_accepted_for_opcode_and_subcode() {
        for input in [
            syn::parse_quote! {
                #[visca(opcode = 0b0100_0111, subcode = 0o4, response = ZoomPosition)]
                struct BinaryOctalInquiry;
            },
            syn::parse_quote! {
                #[visca(opcode = 71, subcode = 4, response = ZoomPosition)]
                struct DecimalInquiry;
            },
        ] {
            let tokens = expanded(input);
            assert!(
                tokens.contains(":: INQUIRY , 4u8 , 71u8]"),
                "every radix must encode the same bytes: {tokens}"
            );
        }
    }

    #[test]
    fn repeated_keys_are_rejected_across_attributes() {
        let tokens = expanded(syn::parse_quote! {
            #[visca(opcode = 0x00, response = Power)]
            #[visca(opcode = 0x01)]
            struct RepeatedOpcodeInquiry;
        });
        assert!(
            tokens.contains("duplicate `visca` attribute `opcode`")
                && tokens.contains("first `opcode` specified here"),
            "a repeated key must not silently win: {tokens}"
        );
    }

    #[test]
    fn request_and_route_are_emitted_once_for_every_response_kind() {
        for input in [
            syn::parse_quote! {
                #[visca(opcode = 0x7E, response = Raw)]
                struct RawInquiry;
            },
            syn::parse_quote! {
                #[visca(opcode = 0x00, response = Power)]
                struct GenericInquiry;
            },
            syn::parse_quote! {
                #[visca(opcode = 0x00, response = Power, typed_response = bool, typed_field = on)]
                struct TypedInquiry;
            },
        ] {
            let tokens = expanded(input);
            assert_eq!(tokens.matches(":: Request for").count(), 1, "{tokens}");
            assert_eq!(tokens.matches(":: Inquiry for").count(), 1, "{tokens}");
            assert_eq!(tokens.matches("fn route").count(), 1, "{tokens}");
            assert!(
                tokens.matches("parse_inquiry_payload").count() <= 1,
                "the canonical decoder is written at most once: {tokens}"
            );
        }
    }

    #[test]
    fn typed_attribute_rules_report_one_wording() {
        for (input, expected) in [
            (
                syn::parse_quote! {
                    #[visca(opcode = 0x00, response = Power, typed_response = bool)]
                    struct MissingFieldInquiry;
                },
                "typed_response requires typed_field",
            ),
            (
                syn::parse_quote! {
                    #[visca(
                        opcode = 0x00,
                        response = Power,
                        typed_response = bool,
                        typed_field = on,
                        typed_constructor = build
                    )]
                    struct UnknownConstructorInquiry;
                },
                "unknown typed_constructor `build`",
            ),
        ] {
            let tokens = expanded(input);
            assert!(tokens.contains(expected), "{expected}: {tokens}");
        }
    }

    #[test]
    fn unknown_parser_strategy_is_rejected() {
        let input: DeriveInput = syn::parse_quote! {
            #[visca(opcode = 0x00, response = Power, parser = DefinitelyNotAParser)]
            struct BadParserInquiry;
        };

        let tokens = derive_visca_inquiry_impl(input).to_string();

        assert!(
            tokens.contains("unknown parser strategy"),
            "unknown parser strategy should produce a diagnostic: {tokens}"
        );
    }

    #[test]
    fn parser_attributes_enable_the_canonical_decoder_without_a_second_template() {
        let inputs: [DeriveInput; 2] = [
            syn::parse_quote! {
                #[visca(opcode = 0x7B, response = Digital, parser = Bool)]
                struct DigitalInquiry;
            },
            syn::parse_quote! {
                #[visca(opcode = 0x47, response = ZoomPosition, parser = Position)]
                struct ZoomPositionInquiry;
            },
        ];

        for input in inputs {
            let tokens = derive_visca_inquiry_impl(input).to_string();
            assert_eq!(
                tokens.matches("parse_inquiry_payload").count(),
                1,
                "the shared helper must be the only canonical parser call: {tokens}"
            );
            assert!(
                !tokens.contains("Nibbles"),
                "derive output must not duplicate nibble-width decoding: {tokens}"
            );
            assert!(
                tokens.contains("__grafton_visca_decode_payload"),
                "both public decode paths must use one generated helper: {tokens}"
            );
        }
    }

    #[test]
    fn qualified_mode_type_path_is_preserved_in_the_shared_decoder() {
        let input: DeriveInput = syn::parse_quote! {
            #[visca(
                opcode = 0x39,
                response = ExposureMode,
                parser = Mode,
                value_type = crate::support::ExposureMode
            )]
            struct ModePathInquiry;
        };

        let tokens = derive_visca_inquiry_impl(input).to_string();

        assert!(
            tokens.contains("crate :: support :: ExposureMode"),
            "the shared decoder must retain the declared mode type: {tokens}"
        );
        assert!(
            tokens.contains("__grafton_visca_decode_payload"),
            "parser derives must use one shared decoder: {tokens}"
        );
    }

    #[test]
    fn qualified_custom_parser_path_is_preserved_in_the_shared_decoder() {
        let input: DeriveInput = syn::parse_quote! {
            #[visca(
                opcode = 0x00,
                response = Power,
                parser = Custom,
                parse_with = crate::parsers::parse_power
            )]
            struct CustomParserInquiry;
        };

        let tokens = derive_visca_inquiry_impl(input).to_string();

        assert!(
            tokens.contains("crate :: parsers :: parse_power (payload)"),
            "the shared decoder must invoke the declared custom parser: {tokens}"
        );
        assert!(
            tokens.contains("__grafton_visca_decode_payload"),
            "parser derives must use one shared decoder: {tokens}"
        );
    }

    #[test]
    fn default_typed_response_conversion_rejects_multiple_fields() {
        let input: DeriveInput = syn::parse_quote! {
            #[visca(
                opcode = 0x12,
                subcode = 0x06,
                response = PanTiltPosition,
                typed_response = types::PanTiltPosition,
                typed_field = (pan, tilt)
            )]
            struct MultiFieldDefaultTypedInquiry;
        };

        let tokens = derive_visca_inquiry_impl(input).to_string();

        assert!(
            tokens.contains("default typed response conversion requires exactly one typed_field"),
            "multi-field default conversion must fail during macro expansion: {tokens}"
        );
    }

    #[test]
    fn single_argument_typed_constructors_reject_multiple_fields() {
        for (constructor, expected) in [
            (
                "new",
                "typed_constructor `new` requires exactly one typed_field",
            ),
            (
                "ok_new",
                "typed_constructor `ok_new` requires exactly one typed_field",
            ),
            (
                "new_u8",
                "typed_constructor `new_u8` requires exactly one typed_field",
            ),
        ] {
            let constructor: Ident = syn::parse_str(constructor).unwrap();
            let input: DeriveInput = syn::parse_quote! {
                #[visca(
                    opcode = 0x12,
                    subcode = 0x06,
                    response = PanTiltPosition,
                    typed_response = types::PanTiltPosition,
                    typed_field = (pan, tilt),
                    typed_constructor = #constructor
                )]
                struct MultiFieldConstructorTypedInquiry;
            };

            let tokens = derive_visca_inquiry_impl(input).to_string();

            assert!(
                tokens.contains(expected),
                "multi-field {constructor} conversion must fail during macro expansion: {tokens}"
            );
        }
    }
}
