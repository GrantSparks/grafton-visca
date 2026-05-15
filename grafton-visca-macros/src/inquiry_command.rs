// SPDX-License-Identifier: MIT OR Apache-2.0
//
// Copyright (c) 2024 Grafton Machine Shed <team@grafton.ai>

//! ViscaInquiry derive macro implementation with parser generation support

use proc_macro2::TokenStream;
use quote::{format_ident, quote};
use syn::{parse::ParseStream, spanned::Spanned, DeriveInput, Ident, LitInt, LitStr, Path, Type};

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

            // Generate parser implementation if parser info is provided
            let parser_info = attrs.parser_info();
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

            let expanded = quote! {
                impl #crate_path::command::ViscaCommand for #struct_name {
                    type Response = #crate_path::command::InquiryData;
                    const MAX_SIZE: usize = #max_size_expr;
                    const TIMEOUT_CATEGORY: #crate_path::timeout::CommandCategory = #crate_path::timeout::CommandCategory::Quick;

                    fn write_into(&self, camera_id: #crate_path::CameraId, buffer: &mut [u8]) -> Result<usize, #crate_path::Error> {
                        #write_into_body
                    }

                    fn response_kind(&self) -> Option<#crate_path::command::InquiryKind> {
                        Some(#crate_path::command::InquiryKind::#response_kind)
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
    parser_type: Option<String>,
    field_name: Option<Ident>,
    mode_type: Option<Ident>,
    custom_fn: Option<Ident>,
    convention: Option<Ident>,
    data_variant: Option<Ident>,
    // Typed response attributes for ResponseParser impl generation:
    typed_response: Option<TypeSpec>,
    typed_field: Option<Vec<Ident>>,
    typed_constructor: Option<Ident>,
    typed_is_tuple: bool,
}

impl ViscaAttributes {
    fn parser_info(&self) -> Option<ParserInfo> {
        self.parser_type.as_ref().map(|parser_type| ParserInfo {
            parser_type: parser_type.clone(),
            field_name: self.field_name.clone(),
            mode_type: self.mode_type.clone(),
            custom_fn: self.custom_fn.clone(),
            convention: self.convention.clone(),
            data_variant: self.data_variant.clone(),
        })
    }
}

enum TypeSpec {
    LegacyString(String),
    Type(Type),
}

struct ParserInfo {
    parser_type: String,
    field_name: Option<Ident>,
    mode_type: Option<Ident>,
    custom_fn: Option<Ident>,
    convention: Option<Ident>, // OnIs02 or OnIs03 for bool_convention parser
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

                if meta.path.is_ident("opcode") || meta.path.is_ident("command") {
                    let value = meta.value()?;
                    let lit: LitInt = value.parse()?;
                    attrs.byte_value = Some(parse_u8_literal(&lit)?);
                } else if meta.path.is_ident("subcode") || meta.path.is_ident("sub_command") {
                    let value = meta.value()?;
                    let lit: LitInt = value.parse()?;
                    attrs.subcategory = Some(parse_u8_literal(&lit)?);
                } else if meta.path.is_ident("response") {
                    let value = meta.value()?;
                    attrs.response_kind = Some(parse_ident_value(value, "response")?);
                } else if meta.path.is_ident("parser") {
                    let value = meta.value()?;
                    let parser = parse_ident_value(value, "parser")?;
                    let parser_name = parser.to_string();
                    validate_parser_name(&parser_name, parser.span())?;
                    attrs.parser_type = Some(parser_name);
                } else if meta.path.is_ident("field") {
                    let value = meta.value()?;
                    attrs.field_name = Some(parse_ident_value(value, "field")?);
                } else if meta.path.is_ident("type") || meta.path.is_ident("value_type") {
                    let value = meta.value()?;
                    attrs.mode_type = Some(parse_ident_value(value, "value_type")?);
                } else if meta.path.is_ident("custom_fn") || meta.path.is_ident("parse_with") {
                    let value = meta.value()?;
                    attrs.custom_fn = Some(parse_ident_value(value, "custom_fn")?);
                } else if meta.path.is_ident("convention") {
                    let value = meta.value()?;
                    let convention = parse_ident_value(value, "convention")?;
                    validate_bool_convention(&convention)?;
                    attrs.convention = Some(convention);
                } else if meta.path.is_ident("data_variant")
                    || meta.path.is_ident("inquiry_variant")
                {
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
                } else if meta.path.is_ident("constant") || meta.path.is_ident("bytes_const") {
                    let value = meta.value()?;
                    let _ = parse_ident_value(value, "bytes_const")?;
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
    if input.peek(LitStr) {
        let lit: LitStr = input.parse()?;
        let value = lit.value();
        if value.contains("::") {
            let path: Path = syn::parse_str(&value).map_err(|_| {
                syn::Error::new(lit.span(), format!("{name} must be an identifier or path"))
            })?;
            return path_last_ident(&path, name, lit.span());
        }
        return syn::parse_str::<Ident>(&value)
            .map_err(|_| syn::Error::new(lit.span(), format!("{name} must be an identifier")));
    }

    let path: Path = input.parse()?;
    path_last_ident(&path, name, path.span())
}

fn path_last_ident(path: &Path, name: &str, span: proc_macro2::Span) -> syn::Result<Ident> {
    path.segments
        .last()
        .map(|segment| segment.ident.clone())
        .ok_or_else(|| syn::Error::new(span, format!("{name} must not be empty")))
}

fn parse_type_spec(input: ParseStream<'_>) -> syn::Result<TypeSpec> {
    if input.peek(LitStr) {
        let lit: LitStr = input.parse()?;
        Ok(TypeSpec::LegacyString(lit.value()))
    } else {
        Ok(TypeSpec::Type(input.parse()?))
    }
}

fn parse_ident_list_value(input: ParseStream<'_>) -> syn::Result<Vec<Ident>> {
    if input.peek(LitStr) {
        let lit: LitStr = input.parse()?;
        let mut fields = Vec::new();
        for field in lit.value().split_whitespace() {
            fields.push(syn::parse_str::<Ident>(field).map_err(|_| {
                syn::Error::new(lit.span(), "typed_field must contain Rust identifiers")
            })?);
        }
        if fields.is_empty() {
            return Err(syn::Error::new(
                lit.span(),
                "typed_field must name at least one field",
            ));
        }
        return Ok(fields);
    }

    let path: Path = input.parse()?;
    Ok(vec![path_last_ident(&path, "typed_field", path.span())?])
}

fn validate_parser_name(parser: &str, span: proc_macro2::Span) -> syn::Result<()> {
    match parser {
        "bool"
        | "direct_byte"
        | "byte"
        | "position"
        | "extended_nibble"
        | "nibble"
        | "flags"
        | "bit_flags"
        | "mode"
        | "mode_enum"
        | "pan_tilt"
        | "bool_convention"
        | "last_nibble"
        | "tally_status"
        | "sharpness_mode"
        | "gamma"
        | "auto_wb_sensitivity"
        | "nd_filter"
        | "picture_effect"
        | "defog_level"
        | "focus_range"
        | "custom" => Ok(()),
        _ => Err(syn::Error::new(
            span,
            format!("unknown parser strategy `{parser}`"),
        )),
    }
}

fn validate_bool_convention(convention: &Ident) -> syn::Result<()> {
    match convention.to_string().as_str() {
        "OnIs02" | "OnIs03" => Ok(()),
        other => Err(syn::Error::new(
            convention.span(),
            format!("unknown bool convention `{other}`"),
        )),
    }
}

fn validate_attrs(attrs: &ViscaAttributes, struct_name: &Ident) -> syn::Result<()> {
    let mut error = None;

    if let Some(parser) = attrs.parser_info() {
        match parser.parser_type.as_str() {
            "mode" | "mode_enum" if parser.mode_type.is_none() => push_error(
                &mut error,
                syn::Error::new_spanned(struct_name, "mode parser requires value_type"),
            ),
            "bool_convention" => {
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
            "last_nibble" if parser.field_name.is_none() => push_error(
                &mut error,
                syn::Error::new_spanned(struct_name, "last_nibble parser requires field"),
            ),
            "custom" if parser.custom_fn.is_none() => push_error(
                &mut error,
                syn::Error::new_spanned(struct_name, "custom parser requires parse_with"),
            ),
            _ => {}
        }
    }

    if attrs.typed_response.is_some() && attrs.typed_field.is_none() {
        push_error(
            &mut error,
            syn::Error::new_spanned(struct_name, "typed_response requires typed_field"),
        );
    }

    if let Some(constructor) = &attrs.typed_constructor {
        match constructor.to_string().as_str() {
            "new" | "ok_new" | "new_u8" | "ok_struct" => {}
            unknown => push_error(
                &mut error,
                syn::Error::new(
                    constructor.span(),
                    format!("unknown typed_constructor `{unknown}`"),
                ),
            ),
        }
    }

    if attrs.typed_response.is_none() && attrs.typed_constructor.is_some() {
        push_error(
            &mut error,
            syn::Error::new_spanned(struct_name, "typed_constructor requires typed_response"),
        );
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

    match parser_info.parser_type.as_str() {
        "bool" => super::parser_templates::generate_bool_parser(response_variant, crate_path),
        "direct_byte" | "byte" => {
            let field_name = format_ident!("value"); // Default field name
            super::parser_templates::generate_direct_byte_parser(
                response_variant,
                &field_name,
                crate_path,
            )
        }
        "position" => {
            super::parser_templates::generate_position_parser(response_variant, crate_path)
        }
        "extended_nibble" | "nibble" => {
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
        "flags" | "bit_flags" => {
            super::parser_templates::generate_bit_flags_parser(response_variant, crate_path)
        }
        "mode" | "mode_enum" => {
            let mode_type = parser_info
                .mode_type
                .clone()
                .expect("mode parser requires type attribute");
            super::parser_templates::generate_mode_enum_parser(
                response_variant,
                &mode_type,
                crate_path,
            )
        }
        "pan_tilt" => {
            super::parser_templates::generate_pan_tilt_parser(response_variant, crate_path)
        }
        "bool_convention" => {
            let field_name = parser_info
                .field_name
                .clone()
                .expect("bool_convention parser requires field attribute");
            let convention = parser_info
                .convention
                .clone()
                .expect("bool_convention parser requires convention attribute (OnIs02 or OnIs03)");
            super::parser_templates::generate_bool_convention_parser(
                &actual_variant,
                &field_name,
                &convention,
                crate_path,
            )
        }
        "last_nibble" => {
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
        "tally_status" => super::parser_templates::generate_tally_status_parser(crate_path),
        "sharpness_mode" => super::parser_templates::generate_sharpness_mode_parser(crate_path),
        "gamma" => super::parser_templates::generate_gamma_parser(crate_path),
        "auto_wb_sensitivity" => {
            super::parser_templates::generate_auto_wb_sensitivity_parser(crate_path)
        }
        "nd_filter" => {
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
        "picture_effect" => {
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
        "defog_level" => {
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
        "focus_range" => {
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
        "custom" => {
            let custom_fn = parser_info
                .custom_fn
                .clone()
                .expect("custom parser requires custom_fn/parse_with attribute");
            quote! {
                #custom_fn(data)
            }
        }
        _ => unreachable!("parser strategy validated during attribute parsing"),
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
fn type_spec_tokens(type_spec: &TypeSpec, crate_path: &TokenStream) -> TokenStream {
    match type_spec {
        TypeSpec::LegacyString(type_str) => parse_type_path_string(type_str, crate_path),
        TypeSpec::Type(ty) => type_tokens(ty, crate_path),
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

/// Parse the legacy string type path syntax into a `TokenStream`.
///
/// Primitive types (`bool`, `u8`, `u16`, `i8`) are emitted directly.  Relative
/// paths are prefixed with the resolved crate path for downstream derives.
fn parse_type_path_string(type_str: &str, crate_path: &TokenStream) -> TokenStream {
    match type_str {
        "bool" => quote! { bool },
        "u8" => quote! { u8 },
        "u16" => quote! { u16 },
        "i8" => quote! { i8 },
        other => {
            let parts: Vec<Ident> = other
                .split("::")
                .map(|s| format_ident!("{}", s.trim()))
                .collect();
            quote! { #crate_path::#(#parts)::* }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn downstream_inquiry_encoding_uses_stack_buffer_and_public_terminator() {
        let input: DeriveInput = syn::parse_quote! {
            #[visca(opcode = 0x47, response = "ZoomPosition")]
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
}
