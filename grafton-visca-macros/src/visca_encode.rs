use proc_macro2::TokenStream;
use quote::quote;
use syn::{Data, DataEnum, DataStruct, DeriveInput, Fields, Ident};

/// Implementation of the ViscaEncode derive macro.
///
/// This generates the EncodeVisca trait implementation for structs and enums,
/// eliminating boilerplate for VISCA command encoding.
pub fn derive_visca_encode_impl(input: DeriveInput) -> TokenStream {
    let name = input.ident;
    let (impl_generics, ty_generics, where_clause) = input.generics.split_for_impl();

    // Parse attributes
    let attrs = parse_attributes(&input.attrs);

    // Generate implementation based on data type
    let implementation = match input.data {
        Data::Struct(data_struct) => generate_struct_impl(&name, data_struct, &attrs),
        Data::Enum(data_enum) => generate_enum_impl(&name, data_enum, &attrs),
        Data::Union(_) => {
            return syn::Error::new_spanned(&name, "ViscaEncode cannot be derived for unions")
                .to_compile_error()
        }
    };

    // Use crate:: for internal usage
    quote! {
        impl #impl_generics crate::command::encode_visca::EncodeVisca for #name #ty_generics #where_clause {
            #implementation
        }
    }
}

/// Attributes that can be specified on the type
#[derive(Default)]
struct ViscaAttributes {
    response_type: Option<String>,
    max_size: Option<usize>,
    timeout_category: Option<String>,
    prefix: Option<Vec<u8>>,
}

/// Parse attributes from the type
fn parse_attributes(attrs: &[syn::Attribute]) -> ViscaAttributes {
    let mut result = ViscaAttributes::default();

    for attr in attrs {
        if !attr.path().is_ident("visca_encode") {
            continue;
        }

        // Parse the attribute arguments
        let _ = attr.parse_nested_meta(|meta| {
            if meta.path.is_ident("response") {
                let value = meta.value()?;
                let lit_str: syn::LitStr = value.parse()?;
                result.response_type = Some(lit_str.value());
            } else if meta.path.is_ident("max_size") {
                let value = meta.value()?;
                let lit_int: syn::LitInt = value.parse()?;
                result.max_size = Some(lit_int.base10_parse().unwrap_or(32));
            } else if meta.path.is_ident("timeout") {
                let value = meta.value()?;
                let lit_str: syn::LitStr = value.parse()?;
                result.timeout_category = Some(lit_str.value());
            } else if meta.path.is_ident("prefix") {
                let value = meta.value()?;
                let lit_bytes: syn::LitByteStr = value.parse()?;
                result.prefix = Some(lit_bytes.value());
            }
            Ok(())
        });
    }

    result
}

/// Generate implementation for structs
fn generate_struct_impl(
    _name: &Ident,
    data_struct: DataStruct,
    attrs: &ViscaAttributes,
) -> TokenStream {
    let max_size = attrs.max_size.unwrap_or(32);
    let timeout_category = match &attrs.timeout_category {
        Some(cat) => {
            let cat_ident = syn::Ident::new(cat, proc_macro2::Span::call_site());
            quote! { crate::timeout::CommandCategory::#cat_ident }
        }
        None => quote! { crate::timeout::CommandCategory::Custom },
    };

    let response_type = match &attrs.response_type {
        Some(resp) => {
            let resp_ident = syn::Ident::new(resp, proc_macro2::Span::call_site());
            quote! { Some(crate::command::ViscaResponseType::#resp_ident) }
        }
        None => quote! { None },
    };

    // For structs, we need to look at the fields to determine encoding
    let encode_body = if let Some(prefix) = &attrs.prefix {
        // If a prefix is provided, use it as the base
        let prefix_bytes = prefix.iter().map(|b| quote! { #b });
        quote! {
            let mut builder = crate::command::const_encoding::ConstCommandBuilder::<#max_size>::new();
            #(builder = builder.push(#prefix_bytes);)*

            // Add any dynamic fields here
            // This is a simplified version - you'd need to handle struct fields

            let terminated = builder.with_camera_id(camera_id).terminate();
            terminated.build_into(buffer)
        }
    } else {
        // Generate encoding based on struct fields
        generate_struct_encoding(&data_struct)
    };

    quote! {
        type ViscaResponse = ();
        const MAX_SIZE: usize = #max_size;
        const TIMEOUT_CATEGORY: crate::timeout::CommandCategory = #timeout_category;

        fn encode_into(
            &self,
            camera_id: crate::camera_id::CameraId,
            buffer: &mut [u8]
        ) -> Result<usize, crate::Error> {
            #encode_body
        }

        fn response_type(&self) -> Option<crate::command::ViscaResponseType> {
            #response_type
        }
    }
}

/// Generate encoding for struct fields
fn generate_struct_encoding(data_struct: &DataStruct) -> TokenStream {
    // This is a simplified version
    // In a full implementation, you'd analyze fields and generate appropriate encoding
    match &data_struct.fields {
        Fields::Named(_fields) => {
            quote! {
                // TODO: Generate encoding based on named fields
                let builder = crate::command::const_encoding::ConstCommandBuilder::<32>::new();
                let terminated = builder.with_camera_id(camera_id).terminate();
                terminated.build_into(buffer)
            }
        }
        Fields::Unnamed(_fields) => {
            quote! {
                // TODO: Generate encoding based on unnamed fields
                let builder = crate::command::const_encoding::ConstCommandBuilder::<32>::new();
                let terminated = builder.with_camera_id(camera_id).terminate();
                terminated.build_into(buffer)
            }
        }
        Fields::Unit => {
            quote! {
                // Unit struct - just use the prefix if available
                let builder = crate::command::const_encoding::ConstCommandBuilder::<32>::new();
                let terminated = builder.with_camera_id(camera_id).terminate();
                terminated.build_into(buffer)
            }
        }
    }
}

/// Generate implementation for enums
fn generate_enum_impl(_name: &Ident, data_enum: DataEnum, attrs: &ViscaAttributes) -> TokenStream {
    let max_size = attrs.max_size.unwrap_or(32);
    let timeout_category = match &attrs.timeout_category {
        Some(cat) => {
            let cat_ident = syn::Ident::new(cat, proc_macro2::Span::call_site());
            quote! { crate::timeout::CommandCategory::#cat_ident }
        }
        None => quote! { crate::timeout::CommandCategory::Custom },
    };

    let response_type = match &attrs.response_type {
        Some(resp) => {
            let resp_ident = syn::Ident::new(resp, proc_macro2::Span::call_site());
            quote! { Some(crate::command::ViscaResponseType::#resp_ident) }
        }
        None => quote! { None },
    };

    // Generate match arms for each variant
    let match_arms = data_enum.variants.iter().map(|variant| {
        let variant_name = &variant.ident;

        // Parse variant attributes for custom bytes
        let variant_bytes = parse_variant_bytes(&variant.attrs);
        match &variant.fields {
            Fields::Unit => {
                // Unit variant - use the bytes from attributes
                if let Some(bytes) = variant_bytes {
                    let byte_literals = bytes.iter().map(|b| quote! { #b });
                    quote! {
                        Self::#variant_name => {
                            let mut builder = crate::command::const_encoding::ConstCommandBuilder::<#max_size>::new();
                            // First add the camera ID
                            builder = builder.push(camera_id.to_address_byte());
                            // Then add the command bytes
                            #(builder = builder.push(#byte_literals);)*
                            // Finally terminate
                            let terminated = builder.terminate();
                            terminated.build_into(buffer)
                        }
                    }
                } else {
                    quote! {
                        Self::#variant_name => {
                            // TODO: Generate default encoding
                            Err(crate::Error::InvalidRequest("No bytes specified for variant".into()))
                        }
                    }
                }
            }
            Fields::Named(_) | Fields::Unnamed(_) => {
                // For now, just generate a placeholder
                quote! {
                    Self::#variant_name { .. } => {
                        // TODO: Handle variant fields
                        Err(crate::Error::InvalidRequest("Complex variants not yet supported".into()))
                    }
                }
            }
        }
    });

    quote! {
        type ViscaResponse = ();
        const MAX_SIZE: usize = #max_size;
        const TIMEOUT_CATEGORY: crate::timeout::CommandCategory = #timeout_category;

        fn encode_into(
            &self,
            camera_id: crate::camera_id::CameraId,
            buffer: &mut [u8]
        ) -> Result<usize, crate::Error> {
            match self {
                #(#match_arms)*
            }
        }

        fn response_type(&self) -> Option<crate::command::ViscaResponseType> {
            #response_type
        }
    }
}

/// Parse bytes attribute from variant
fn parse_variant_bytes(attrs: &[syn::Attribute]) -> Option<Vec<u8>> {
    for attr in attrs {
        if attr.path().is_ident("visca_bytes") {
            // Parse as a list of literal integers
            let mut bytes = Vec::new();
            let result = attr.parse_args_with(|input: syn::parse::ParseStream| {
                loop {
                    if input.is_empty() {
                        break;
                    }
                    let lit: syn::LitInt = input.parse()?;
                    if let Ok(byte) = lit.base10_parse::<u8>() {
                        bytes.push(byte);
                    }
                    if !input.is_empty() {
                        let _: syn::Token![,] = input.parse()?;
                    }
                }
                Ok(())
            });

            if result.is_ok() && !bytes.is_empty() {
                return Some(bytes);
            }
        }
    }
    None
}
