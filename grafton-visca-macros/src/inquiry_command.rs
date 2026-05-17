// SPDX-License-Identifier: MIT OR Apache-2.0
//
// Copyright (c) 2024 Grafton Machine Shed <team@grafton.ai>

//! ViscaInquiry derive macro implementation with parser generation support

use proc_macro2::TokenStream;
use quote::{format_ident, quote};
use syn::{
    parenthesized, parse::ParseStream, punctuated::Punctuated, spanned::Spanned, DeriveInput,
    Ident, LitInt, Path, Token, Type,
};

pub fn derive_visca_inquiry_impl(input: DeriveInput) -> TokenStream {
    match &input.data {
        syn::Data::Struct(_) => {
            let struct_name = &input.ident;

            // Parse visca attributes from the struct
            let attrs = match parse_visca_attributes_from_struct(&input) {
                Ok(attrs) => attrs,
                Err(error) => return error.to_compile_error(),
            };

            let mut validation_error = None;
            let response_kind = match attrs.response_kind.clone() {
                Some(response_kind) => response_kind,
                None => {
                    push_error(
                        &mut validation_error,
                        syn::Error::new_spanned(
                            struct_name,
                            "visca attribute must have a response value",
                        ),
                    );
                    format_ident!("MissingResponse")
                }
            };

            // Determine crate path once for consistency
            let is_internal_crate =
                std::env::var("CARGO_CRATE_NAME").unwrap_or_default() == "grafton_visca";
            let crate_path = if is_internal_crate {
                quote! { crate }
            } else {
                quote! { ::grafton_visca }
            };

            let byte_value = match attrs.byte_value {
                Some(byte_value) => byte_value,
                None => {
                    push_error(
                        &mut validation_error,
                        syn::Error::new_spanned(
                            struct_name,
                            "visca attribute must have an opcode value",
                        ),
                    );
                    0
                }
            };
            let subcategory = attrs.subcategory.unwrap_or(0x04);
            if let Err(error) = validate_attrs(&attrs, struct_name) {
                push_error(&mut validation_error, error);
            }
            if let Some(error) = validation_error {
                return error.to_compile_error();
            }

            let max_size_expr = quote! { 5 };
            let write_into_body = quote! {
                const LEN: usize = 5;
                if buffer.len() < LEN {
                    return Err(#crate_path::Error::BufferTooSmall {
                        required: LEN,
                        actual: buffer.len(),
                    });
                }
                buffer[0] = camera_id.to_address_byte();
                buffer[1] = 0x09;
                buffer[2] = #subcategory;
                buffer[3] = #byte_value;
                buffer[4] = #crate_path::command::VISCA_TERMINATOR;
                Ok(LEN)
            };

            let parser_info = attrs.parser_info();
            let raw_response = response_kind == "Raw";
            if raw_response && (parser_info.is_some() || attrs.typed_response.is_some()) {
                return syn::Error::new_spanned(
                    struct_name,
                    "response = Raw does not support generated built-in parsers; implement ResponseParser manually",
                )
                .to_compile_error();
            }

            // Generate parser implementation if parser info is provided
            let parse_response_impl = if let Some(parser_info) = &parser_info {
                let parser_body = generate_parser_body(&response_kind, parser_info, &crate_path);
                quote! {
                    impl #struct_name {
                        /// Parse the response data for this inquiry command
                        pub fn parse_response(&self, data: &[u8]) -> Result<#crate_path::command::InquiryData, #crate_path::Error> {
                            if data.is_empty() {
                                return Err(#crate_path::Error::invalid_response_length(1, data));
                            }
                            #parser_body
                        }
                    }
                }
            } else {
                quote! {}
            };

            // Optionally generate a typed ResponseParser impl
            let typed_impl = generate_typed_impl(
                struct_name,
                &response_kind,
                &crate_path,
                &attrs,
                &parser_info,
            );

            let behavior_expr = if raw_response {
                quote! {
                    #crate_path::command::CommandBehavior::Inquiry(
                        #crate_path::command::InquiryResponseSpec::Raw,
                    )
                }
            } else {
                quote! {
                    #crate_path::command::CommandBehavior::Inquiry(
                        #crate_path::command::InquiryResponseSpec::Builtin(
                            #crate_path::command::InquiryKind::#response_kind,
                        ),
                    )
                }
            };

            let expanded = quote! {
                impl #crate_path::command::ViscaCommand for #struct_name {
                    const MAX_SIZE: usize = #max_size_expr;
                    const TIMEOUT_CATEGORY: #crate_path::timeout::CommandCategory = #crate_path::timeout::CommandCategory::Quick;

                    fn write_into(&self, camera_id: #crate_path::CameraId, buffer: &mut [u8]) -> Result<usize, #crate_path::Error> {
                        #write_into_body
                    }

                    fn behavior(&self) -> #crate_path::command::CommandBehavior {
                        #behavior_expr
                    }
                }


                #parse_response_impl

                #typed_impl
            };

            expanded
        }
        _ => syn::Error::new_spanned(&input, "ViscaInquiry can only be derived for structs")
            .to_compile_error(),
    }
}

#[derive(Default)]
struct ViscaAttributes {
    byte_value: Option<u8>,
    subcategory: Option<u8>,
    response_kind: Option<Ident>,
    parser_type: Option<ParserStrategy>,
    field_name: Option<Ident>,
    mode_type: Option<Type>,
    parse_with: Option<Path>,
    convention: Option<Path>,
    data_variant: Option<Ident>,
    // Typed response attributes for ResponseParser impl generation:
    typed_response: Option<Type>,
    typed_field: Option<Vec<Ident>>,
    typed_constructor: Option<Ident>,
    typed_is_tuple: bool,
}

impl ViscaAttributes {
    fn parser_info(&self) -> Option<ParserInfo> {
        self.parser_type.as_ref().map(|parser_type| ParserInfo {
            parser_type: *parser_type,
            field_name: self.field_name.clone(),
            mode_type: self.mode_type.clone(),
            parse_with: self.parse_with.clone(),
            convention: self.convention.clone(),
            data_variant: self.data_variant.clone(),
        })
    }
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

#[derive(Clone)]
struct ParserInfo {
    parser_type: ParserStrategy,
    field_name: Option<Ident>,
    mode_type: Option<Type>,
    parse_with: Option<Path>,
    convention: Option<Path>, // OnIs02 or OnIs03 for bool_convention parser
    data_variant: Option<Ident>, // InquiryData variant if different from response (InquiryKind)
}

fn parse_visca_attributes_from_struct(input: &DeriveInput) -> syn::Result<ViscaAttributes> {
    let mut attrs = ViscaAttributes::default();
    let mut saw_visca = false;

    for attr in &input.attrs {
        if attr.path().is_ident("visca") {
            saw_visca = true;
            attr.parse_nested_meta(|meta| {
                if meta.path.is_ident("typed_is_tuple") {
                    attrs.typed_is_tuple = true;
                    return Ok(());
                }

                if meta.path.is_ident("opcode") {
                    let value = meta.value()?;
                    let lit: LitInt = value.parse()?;
                    attrs.byte_value = Some(parse_u8_literal(&lit)?);
                } else if meta.path.is_ident("subcode") {
                    let value = meta.value()?;
                    let lit: LitInt = value.parse()?;
                    attrs.subcategory = Some(parse_u8_literal(&lit)?);
                } else if meta.path.is_ident("response") {
                    let value = meta.value()?;
                    attrs.response_kind = Some(parse_ident_value(value, "response")?);
                } else if meta.path.is_ident("parser") {
                    let value = meta.value()?;
                    attrs.parser_type = Some(parse_parser_strategy(value)?);
                } else if meta.path.is_ident("field") {
                    let value = meta.value()?;
                    attrs.field_name = Some(parse_ident_value(value, "field")?);
                } else if meta.path.is_ident("value_type") {
                    let value = meta.value()?;
                    attrs.mode_type = Some(parse_type_spec(value)?);
                } else if meta.path.is_ident("parse_with") {
                    let value = meta.value()?;
                    attrs.parse_with = Some(parse_path_value(value, "parse_with")?);
                } else if meta.path.is_ident("convention") {
                    let value = meta.value()?;
                    let convention = parse_path_value(value, "convention")?;
                    validate_bool_convention(&convention)?;
                    attrs.convention = Some(convention);
                } else if meta.path.is_ident("data_variant") {
                    let value = meta.value()?;
                    attrs.data_variant = Some(parse_ident_value(value, "data_variant")?);
                } else if meta.path.is_ident("typed_response") {
                    let value = meta.value()?;
                    attrs.typed_response = Some(parse_type_spec(value)?);
                } else if meta.path.is_ident("typed_field") {
                    let value = meta.value()?;
                    attrs.typed_field = Some(parse_ident_list_value(value)?);
                } else if meta.path.is_ident("typed_constructor") {
                    let value = meta.value()?;
                    attrs.typed_constructor = Some(parse_ident_value(value, "typed_constructor")?);
                } else {
                    return Err(meta.error("unknown visca attribute key"));
                }
                Ok(())
            })?;
        }
    }

    if !saw_visca {
        return Err(syn::Error::new_spanned(
            &input.ident,
            "missing #[visca(...)] attribute",
        ));
    }

    Ok(attrs)
}

fn push_error(target: &mut Option<syn::Error>, error: syn::Error) {
    if let Some(existing) = target {
        existing.combine(error);
    } else {
        *target = Some(error);
    }
}

fn parse_u8_literal(lit: &LitInt) -> syn::Result<u8> {
    let suffix = lit.suffix();
    if !suffix.is_empty() {
        return Err(syn::Error::new(
            lit.span(),
            "numeric visca fields must be unsuffixed u8 literals",
        ));
    }

    let raw = lit.to_string().replace('_', "");
    let parsed = if let Some(hex) = raw.strip_prefix("0x").or_else(|| raw.strip_prefix("0X")) {
        u8::from_str_radix(hex, 16)
    } else {
        raw.parse::<u8>()
    };
    parsed.map_err(|_| syn::Error::new(lit.span(), "value must fit in u8"))
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

fn parse_path_value(input: ParseStream<'_>, _name: &str) -> syn::Result<Path> {
    let path: Path = input.parse()?;
    Ok(path)
}

fn parse_type_spec(input: ParseStream<'_>) -> syn::Result<Type> {
    input.parse()
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

fn validate_attrs(attrs: &ViscaAttributes, struct_name: &Ident) -> syn::Result<()> {
    let mut error = None;

    if let Some(parser) = attrs.parser_info() {
        match parser.parser_type {
            ParserStrategy::Mode | ParserStrategy::ModeEnum if parser.mode_type.is_none() => {
                push_error(
                    &mut error,
                    syn::Error::new_spanned(struct_name, "mode parser requires value_type"),
                )
            }
            ParserStrategy::BoolConvention => {
                if parser.field_name.is_none() {
                    push_error(
                        &mut error,
                        syn::Error::new_spanned(
                            struct_name,
                            "bool_convention parser requires field",
                        ),
                    );
                }
                if parser.convention.is_none() {
                    push_error(
                        &mut error,
                        syn::Error::new_spanned(
                            struct_name,
                            "bool_convention parser requires convention",
                        ),
                    );
                }
            }
            ParserStrategy::LastNibble if parser.field_name.is_none() => push_error(
                &mut error,
                syn::Error::new_spanned(struct_name, "last_nibble parser requires field"),
            ),
            ParserStrategy::Custom if parser.parse_with.is_none() => push_error(
                &mut error,
                syn::Error::new_spanned(struct_name, "custom parser requires parse_with"),
            ),
            _ => {}
        }
    }

    match (&attrs.typed_response, &attrs.typed_field) {
        (Some(_), None) => push_error(
            &mut error,
            syn::Error::new_spanned(struct_name, "typed_response requires typed_field"),
        ),
        (None, Some(_)) => push_error(
            &mut error,
            syn::Error::new_spanned(struct_name, "typed_field requires typed_response"),
        ),
        _ => {}
    }

    let constructor_name = attrs
        .typed_constructor
        .as_ref()
        .map(std::string::ToString::to_string);
    if let Some(constructor) = &attrs.typed_constructor {
        match constructor_name.as_deref() {
            Some("new" | "ok_new" | "new_u8" | "ok_struct") => {}
            Some(unknown) => push_error(
                &mut error,
                syn::Error::new(
                    constructor.span(),
                    format!("unknown typed_constructor `{unknown}`"),
                ),
            ),
            None => {}
        }
    }

    if attrs.typed_response.is_none() && attrs.typed_constructor.is_some() {
        push_error(
            &mut error,
            syn::Error::new_spanned(struct_name, "typed_constructor requires typed_response"),
        );
    }

    if let Some(field_names) = &attrs.typed_field {
        match constructor_name.as_deref() {
            None | Some("new" | "ok_new" | "new_u8") => {
                if attrs.typed_response.is_some() && field_names.len() != 1 {
                    let message = match constructor_name.as_deref() {
                        Some(name) => {
                            format!("typed_constructor `{name}` requires exactly one typed_field")
                        }
                        None => {
                            "default typed response conversion requires exactly one typed_field"
                                .to_owned()
                        }
                    };
                    push_error(&mut error, syn::Error::new_spanned(struct_name, message));
                }
            }
            Some("ok_struct") | Some(_) => {}
        }
    }

    if let Some(error) = error {
        Err(error)
    } else {
        Ok(())
    }
}

fn generate_parser_body(
    response_variant: &Ident,
    parser_info: &ParserInfo,
    crate_path: &TokenStream,
) -> TokenStream {
    // Use data_variant if specified, otherwise use response_variant
    let actual_variant = parser_info
        .data_variant
        .clone()
        .unwrap_or_else(|| response_variant.clone());

    match parser_info.parser_type {
        ParserStrategy::Bool => {
            super::parser_templates::generate_bool_parser(response_variant, crate_path)
        }
        ParserStrategy::DirectByte | ParserStrategy::Byte => {
            let field_name = format_ident!("value"); // Default field name
            super::parser_templates::generate_direct_byte_parser(
                response_variant,
                &field_name,
                crate_path,
            )
        }
        ParserStrategy::Position => {
            super::parser_templates::generate_position_parser(response_variant, crate_path)
        }
        ParserStrategy::ExtendedNibble | ParserStrategy::Nibble => {
            let field_name = parser_info
                .field_name
                .clone()
                .unwrap_or_else(|| format_ident!("value"));
            super::parser_templates::generate_extended_nibble_parser(
                response_variant,
                &field_name,
                crate_path,
            )
        }
        ParserStrategy::Flags | ParserStrategy::BitFlags => {
            super::parser_templates::generate_bit_flags_parser(response_variant, crate_path)
        }
        ParserStrategy::Mode | ParserStrategy::ModeEnum => {
            let mode_type = parser_info
                .mode_type
                .clone()
                .expect("mode parser requires type attribute");
            let mode_type = command_type_tokens(&mode_type, crate_path);
            super::parser_templates::generate_mode_enum_parser(
                response_variant,
                &mode_type,
                crate_path,
            )
        }
        ParserStrategy::PanTilt => {
            super::parser_templates::generate_pan_tilt_parser(response_variant, crate_path)
        }
        ParserStrategy::BoolConvention => {
            let field_name = parser_info
                .field_name
                .clone()
                .expect("bool_convention parser requires field attribute");
            let convention = parser_info
                .convention
                .clone()
                .expect("bool_convention parser requires convention attribute (OnIs02 or OnIs03)");
            let convention = bool_convention_tokens(&convention, crate_path);
            super::parser_templates::generate_bool_convention_parser(
                &actual_variant,
                &field_name,
                &convention,
                crate_path,
            )
        }
        ParserStrategy::LastNibble => {
            let field_name = parser_info
                .field_name
                .clone()
                .expect("last_nibble parser requires field attribute");
            super::parser_templates::generate_last_nibble_parser(
                &actual_variant,
                &field_name,
                crate_path,
            )
        }
        ParserStrategy::TallyStatus => {
            super::parser_templates::generate_tally_status_parser(crate_path)
        }
        ParserStrategy::SharpnessMode => {
            super::parser_templates::generate_sharpness_mode_parser(crate_path)
        }
        ParserStrategy::Gamma => super::parser_templates::generate_gamma_parser(crate_path),
        ParserStrategy::AutoWbSensitivity => {
            super::parser_templates::generate_auto_wb_sensitivity_parser(crate_path)
        }
        ParserStrategy::NdFilter => {
            let field_name = format_ident!("position");
            let converter_type = quote! { #crate_path::command::NdFilterPosition };
            super::parser_templates::generate_byte_converter_parser(
                &actual_variant,
                &field_name,
                &converter_type,
                super::parser_templates::ConverterMethod::FromByte,
                crate_path,
            )
        }
        ParserStrategy::PictureEffect => {
            let field_name = format_ident!("effect");
            let converter_type = quote! { #crate_path::command::PictureEffectMode };
            super::parser_templates::generate_byte_converter_parser(
                &actual_variant,
                &field_name,
                &converter_type,
                super::parser_templates::ConverterMethod::FromByte,
                crate_path,
            )
        }
        ParserStrategy::DefogLevel => {
            let field_name = format_ident!("level");
            let converter_type = quote! { #crate_path::types::DefogLevel };
            super::parser_templates::generate_byte_converter_parser(
                &actual_variant,
                &field_name,
                &converter_type,
                super::parser_templates::ConverterMethod::New,
                crate_path,
            )
        }
        ParserStrategy::FocusRange => {
            let field_name = format_ident!("range");
            let converter_type = quote! { #crate_path::command::FocusRange };
            super::parser_templates::generate_byte_converter_parser(
                &actual_variant,
                &field_name,
                &converter_type,
                super::parser_templates::ConverterMethod::TryFrom,
                crate_path,
            )
        }
        ParserStrategy::Custom => {
            let parse_with = parser_info
                .parse_with
                .clone()
                .expect("custom parser requires parse_with attribute");
            quote! {
                #parse_with(data)
            }
        }
    }
}

/// Generate an impl of `command::ResponseParser` for the struct based on
/// `typed_*` attributes. Returns empty tokens if no `typed_response` attribute is set.
fn generate_typed_impl(
    struct_name: &Ident,
    response_variant: &Ident,
    crate_path: &TokenStream,
    attrs: &ViscaAttributes,
    parser_info: &Option<ParserInfo>,
) -> TokenStream {
    let Some(typed_response) = &attrs.typed_response else {
        return quote! {};
    };

    let response_type = type_spec_tokens(typed_response, crate_path);

    // Use data_variant from parser info if present, otherwise use response_variant
    let data_variant = parser_info
        .as_ref()
        .and_then(|p| p.data_variant.clone())
        .unwrap_or_else(|| response_variant.clone());

    let Some(field_names) = &attrs.typed_field else {
        return syn::Error::new_spanned(
            struct_name,
            "typed_response requires typed_field attribute",
        )
        .to_compile_error();
    };

    // Generate the destructure pattern
    let destructure = if attrs.typed_is_tuple {
        quote! { #crate_path::command::InquiryData::#data_variant(#(#field_names),*) }
    } else {
        quote! { #crate_path::command::InquiryData::#data_variant { #(#field_names),* } }
    };

    // Generate the construction expression
    let constructor = attrs
        .typed_constructor
        .as_ref()
        .map(std::string::ToString::to_string);
    let construction = match constructor.as_deref() {
        None => {
            let f = &field_names[0];
            quote! { Ok(#f) }
        }
        Some("new") => {
            let f = &field_names[0];
            quote! { #response_type::new(#f) }
        }
        Some("ok_new") => {
            let f = &field_names[0];
            quote! { Ok(#response_type::new(#f)) }
        }
        Some("new_u8") => {
            let f = &field_names[0];
            quote! { #response_type::new(#f as u8) }
        }
        Some("ok_struct") => {
            quote! { Ok(#response_type { #(#field_names),* }) }
        }
        Some(unknown) => {
            let msg = format!("unknown typed_constructor: {unknown}");
            return syn::Error::new_spanned(struct_name, msg).to_compile_error();
        }
    };

    quote! {
        impl #crate_path::command::ResponseParser for #struct_name {
            type Response = #response_type;

            fn from_response(
                resp: #crate_path::command::Response,
            ) -> Result<Self::Response, #crate_path::Error> {
                match resp {
                    #crate_path::command::Response::Inquiry(#destructure) => #construction,
                    #crate_path::command::Response::Error(e) => Err(e),
                    _ => Err(#crate_path::Error::UnexpectedResponseType),
                }
            }
        }
    }
}

/// Convert a typed response specification into a generated type path.
fn type_spec_tokens(ty: &Type, crate_path: &TokenStream) -> TokenStream {
    type_tokens(ty, crate_path)
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
    fn downstream_inquiry_encoding_uses_stack_buffer_and_public_terminator() {
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
            !tokens.contains("0xFF"),
            "generated downstream inquiry encoder must use VISCA_TERMINATOR: {tokens}"
        );
        assert!(
            tokens.contains("command :: VISCA_TERMINATOR"),
            "generated downstream inquiry encoder must use the public terminator path: {tokens}"
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

        for input in inputs {
            let tokens = derive_visca_inquiry_impl(input).to_string();
            assert!(
                tokens.contains("unknown visca attribute key"),
                "legacy key aliases must not be accepted: {tokens}"
            );
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
    fn qualified_mode_type_path_is_preserved() {
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
            "qualified value_type path should be preserved: {tokens}"
        );
        assert!(
            !tokens.contains("command :: ExposureMode as TryFrom"),
            "qualified value_type path must not be reduced to its last segment: {tokens}"
        );
    }

    #[test]
    fn qualified_custom_parser_path_is_preserved() {
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
            tokens.contains("crate :: parsers :: parse_power (data)"),
            "qualified custom parser path should be preserved: {tokens}"
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
