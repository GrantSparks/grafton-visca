// SPDX-License-Identifier: Apache-2.0
//
// Copyright (c) 2024 Grafton Machine Shed <team@grafton.ai>

//! Procedural macros for the grafton-visca library
//!
//! This crate provides derive and attribute macros to simplify common patterns
//! in VISCA command implementations.

use proc_macro::TokenStream;
use quote::{format_ident, quote};
use syn::{parse_macro_input, DeriveInput};

mod bounded_macros;
mod command_macros;
mod inquiry_command;
mod method_macros;
mod parser_templates;
mod position_macros;
mod speed_macros;

#[proc_macro_attribute]
pub fn visca_method(attr: TokenStream, item: TokenStream) -> TokenStream {
    method_macros::visca_method(attr, item)
}

#[proc_macro_attribute]
pub fn visca_method_custom(attr: TokenStream, item: TokenStream) -> TokenStream {
    method_macros::visca_method_custom(attr, item)
}

#[proc_macro_attribute]
pub fn visca_camera_method(attr: TokenStream, item: TokenStream) -> TokenStream {
    method_macros::visca_camera_method(attr, item)
}

#[proc_macro_attribute]
pub fn visca_command(attr: TokenStream, item: TokenStream) -> TokenStream {
    command_macros::visca_command(attr, item)
}

#[proc_macro_attribute]
pub fn visca_command_variants(attr: TokenStream, item: TokenStream) -> TokenStream {
    command_macros::visca_command_variants(attr, item)
}

#[proc_macro_attribute]
pub fn visca_method_generic(attr: TokenStream, item: TokenStream) -> TokenStream {
    method_macros::visca_method_generic(attr, item)
}

#[proc_macro_attribute]
pub fn visca_inquiry(attr: TokenStream, item: TokenStream) -> TokenStream {
    method_macros::visca_inquiry(attr, item)
}

#[proc_macro_attribute]
pub fn visca_position_command(attr: TokenStream, item: TokenStream) -> TokenStream {
    position_macros::visca_position_command(attr, item)
}

#[proc_macro_attribute]
pub fn visca_speed_command(attr: TokenStream, item: TokenStream) -> TokenStream {
    speed_macros::visca_speed_command(attr, item)
}
#[proc_macro_attribute]
pub fn visca_bounded_command(attr: TokenStream, item: TokenStream) -> TokenStream {
    bounded_macros::visca_bounded_command(attr, item)
}
#[proc_macro_derive(ViscaValue, attributes(visca_value))]
pub fn derive_visca_value(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);

    // Extract the struct name and inner type
    let name = &input.ident;
    let (inner_type, inner_field_index) = match &input.data {
        syn::Data::Struct(data) => match &data.fields {
            syn::Fields::Unnamed(fields) if fields.unnamed.len() == 1 => {
                let field = &fields.unnamed[0];
                (&field.ty, syn::Index::from(0))
            }
            _ => {
                return syn::Error::new_spanned(
                    &input,
                    "ViscaValue can only be derived for tuple structs with a single field",
                )
                .to_compile_error()
                .into();
            }
        },
        _ => {
            return syn::Error::new_spanned(&input, "ViscaValue can only be derived for structs")
                .to_compile_error()
                .into();
        }
    };

    // Parse attributes
    let mut min_value = None;
    let mut max_value = None;
    let mut valid_values = None;
    let mut display_format = "decimal";
    let mut display_prefix = "";

    for attr in &input.attrs {
        if attr.path().is_ident("visca_value") {
            let _ = attr.parse_nested_meta(|meta| {
                if meta.path.is_ident("min") {
                    let value: syn::LitStr = meta.value()?.parse()?;
                    min_value = Some(value.value());
                } else if meta.path.is_ident("max") {
                    let value: syn::LitStr = meta.value()?.parse()?;
                    max_value = Some(value.value());
                } else if meta.path.is_ident("valid_values") {
                    let value: syn::LitStr = meta.value()?.parse()?;
                    valid_values = Some(value.value());
                } else if meta.path.is_ident("display_format") {
                    let value: syn::LitStr = meta.value()?.parse()?;
                    display_format = match value.value().as_str() {
                        "hex" => "hex",
                        "binary" => "binary",
                        "decimal" => "decimal",
                        _ => {
                            return Err(
                                meta.error("display_format must be 'hex', 'binary', or 'decimal'")
                            )
                        }
                    };
                } else if meta.path.is_ident("display_prefix") {
                    let value: syn::LitStr = meta.value()?.parse()?;
                    display_prefix = Box::leak(value.value().into_boxed_str());
                }
                Ok(())
            });
        }
    }

    // Generate validation code
    let validation = if let Some(valid_list) = &valid_values {
        // Parse the valid values list
        let values_tokens: proc_macro2::TokenStream =
            valid_list.parse().unwrap_or_else(|_| quote! { &[] });
        quote! {
            const VALID_VALUES: &[#inner_type] = &#values_tokens;
            if !VALID_VALUES.contains(&value) {
                return Err(crate::Error::InvalidParameter(format!(
                    "{} must be one of {:?}, got {}",
                    stringify!(#name),
                    VALID_VALUES,
                    value
                )));
            }
        }
    } else if let (Some(min), Some(max)) = (&min_value, &max_value) {
        let min_tokens: proc_macro2::TokenStream = min.parse().unwrap_or_else(|_| quote! { 0 });
        let max_tokens: proc_macro2::TokenStream = max.parse().unwrap_or_else(|_| quote! { 255 });
        quote! {
            if !(#min_tokens..=#max_tokens).contains(&value) {
                return Err(crate::Error::ParameterOutOfRange {
                    parameter: stringify!(#name).to_string(),
                    value: value as i32,
                    min: #min_tokens as i32,
                    max: #max_tokens as i32,
                });
            }
        }
    } else {
        quote! {}
    };

    // Generate min/max constants if provided
    let constants = if let (Some(min), Some(max)) = (&min_value, &max_value) {
        let min_tokens: proc_macro2::TokenStream = min.parse().unwrap_or_else(|_| quote! { 0 });
        let max_tokens: proc_macro2::TokenStream = max.parse().unwrap_or_else(|_| quote! { 255 });
        quote! {
            /// Minimum value.
            pub const MIN: Self = Self(#min_tokens);

            /// Maximum value.
            pub const MAX: Self = Self(#max_tokens);
        }
    } else {
        quote! {}
    };

    // Generate display format
    let display_impl = match display_format {
        "hex" => {
            if display_prefix.is_empty() {
                quote! {
                    write!(f, "{:#02x}", self.#inner_field_index)
                }
            } else {
                quote! {
                    write!(f, "{} {:#02x}", #display_prefix, self.#inner_field_index)
                }
            }
        }
        "binary" => {
            if display_prefix.is_empty() {
                quote! {
                    write!(f, "{:#b}", self.#inner_field_index)
                }
            } else {
                quote! {
                    write!(f, "{} {:#b}", #display_prefix, self.#inner_field_index)
                }
            }
        }
        _ => {
            if display_prefix.is_empty() {
                quote! {
                    write!(f, "{}", self.#inner_field_index)
                }
            } else {
                quote! {
                    write!(f, "{} {}", #display_prefix, self.#inner_field_index)
                }
            }
        }
    };

    let expanded = quote! {
        impl #name {
            #constants

            /// Create a new value with validation.
            ///
            /// # Errors
            /// Returns an error if the value is out of range or invalid.
            pub fn new(value: #inner_type) -> Result<Self, crate::Error> {
                #validation
                Ok(Self(value))
            }

            /// Get the raw value.
            #[must_use]
            pub const fn value(self) -> #inner_type {
                self.#inner_field_index
            }
        }

        impl TryFrom<#inner_type> for #name {
            type Error = crate::Error;

            fn try_from(value: #inner_type) -> Result<Self, Self::Error> {
                Self::new(value)
            }
        }

        impl From<#name> for #inner_type {
            fn from(val: #name) -> Self {
                val.value()
            }
        }

        impl std::fmt::Display for #name {
            fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                #display_impl
            }
        }
    };

    TokenStream::from(expanded)
}

#[proc_macro_attribute]
pub fn visca_fallible_method(attr: TokenStream, item: TokenStream) -> TokenStream {
    method_macros::visca_fallible_method(attr, item)
}

/// Generate mock transport implementations for testing.
///
/// This macro generates both blocking and async mock transports that can be
/// programmed with expected command/response sequences for testing.
///
/// # Features
/// - Pre-programmed command/response sequences
/// - Error injection capabilities
/// - Command history tracking
/// - Timeout simulation
/// - Both blocking and async implementations
///
/// # Example
///
/// ```rust,ignore
/// #[visca_mock_transport]
/// struct TestTransport {
///     sequences: Vec<(Vec<u8>, Vec<u8>)>,  // (expected_command, response)
///     error_after: Option<usize>,           // Inject error after N commands
///     timeout_after: Option<usize>,         // Timeout after N commands
/// }
/// ```
///
/// Generates:
/// - `MockTransport` with blocking implementation
/// - `AsyncMockTransport` with async implementation
/// - Helper methods for test setup
#[proc_macro_attribute]
pub fn visca_mock_transport(_attr: TokenStream, item: TokenStream) -> TokenStream {
    let input = parse_macro_input!(item as DeriveInput);
    let name = &input.ident;

    let mock_name = format_ident!("Mock{}", name);
    let async_mock_name = format_ident!("AsyncMock{}", name);

    let expanded = quote! {
        #input

        /// Mock transport for testing with pre-programmed responses.
        #[derive(Debug, Clone)]
        pub struct #mock_name {
            sequences: Vec<(Vec<u8>, Vec<u8>)>,
            current_index: std::cell::RefCell<usize>,
            command_history: std::cell::RefCell<Vec<Vec<u8>>>,
            error_after: Option<usize>,
            timeout_after: Option<usize>,
            delay_ms: Option<u64>,
        }

        impl #mock_name {
            /// Create a new mock transport with the given command/response sequences.
            pub fn new(sequences: Vec<(Vec<u8>, Vec<u8>)>) -> Self {
                Self {
                    sequences,
                    current_index: std::cell::RefCell::new(0),
                    command_history: std::cell::RefCell::new(Vec::new()),
                    error_after: None,
                    timeout_after: None,
                    delay_ms: None,
                }
            }

            /// Configure to return an error after N commands.
            pub fn with_error_after(mut self, n: usize) -> Self {
                self.error_after = Some(n);
                self
            }

            /// Configure to timeout after N commands.
            pub fn with_timeout_after(mut self, n: usize) -> Self {
                self.timeout_after = Some(n);
                self
            }

            /// Configure to delay responses by the given milliseconds.
            pub fn with_delay(mut self, delay_ms: u64) -> Self {
                self.delay_ms = Some(delay_ms);
                self
            }

            /// Get the history of commands sent to this transport.
            pub fn command_history(&self) -> Vec<Vec<u8>> {
                self.command_history.borrow().clone()
            }

            /// Reset the mock transport state.
            pub fn reset(&self) {
                *self.current_index.borrow_mut() = 0;
                self.command_history.borrow_mut().clear();
            }

            /// Add a new command/response sequence.
            pub fn add_sequence(&mut self, command: Vec<u8>, response: Vec<u8>) {
                self.sequences.push((command, response));
            }
        }

        impl crate::transport::blocking::BlockingTransport for #mock_name {
            fn send_and_receive(&mut self, data: &[u8]) -> Result<Vec<u8>, crate::Error> {
                let mut index = self.current_index.borrow_mut();
                let mut history = self.command_history.borrow_mut();

                // Record the command
                history.push(data.to_vec());

                // Check for error injection
                if let Some(error_after) = self.error_after {
                    if history.len() > error_after {
                        return Err(crate::Error::Transport("Injected error".into()));
                    }
                }

                // Check for timeout injection
                if let Some(timeout_after) = self.timeout_after {
                    if history.len() > timeout_after {
                        return Err(crate::Error::Timeout);
                    }
                }

                // Simulate delay if configured
                if let Some(delay_ms) = self.delay_ms {
                    std::thread::sleep(std::time::Duration::from_millis(delay_ms));
                }

                // Find matching sequence
                if *index < self.sequences.len() {
                    let (expected_cmd, response) = &self.sequences[*index];
                    if data == expected_cmd.as_slice() {
                        *index += 1;
                        Ok(response.clone())
                    } else {
                        Err(crate::Error::Transport(format!(
                            "Unexpected command. Expected {:?}, got {:?}",
                            expected_cmd, data
                        )))
                    }
                } else {
                    Err(crate::Error::Transport("No more mock responses available".into()))
                }
            }
        }

        #[cfg(feature = "async")]
        /// Async mock transport for testing with pre-programmed responses.
        #[derive(Debug, Clone)]
        pub struct #async_mock_name {
            inner: #mock_name,
        }

        #[cfg(feature = "async")]
        impl #async_mock_name {
            /// Create a new async mock transport with the given command/response sequences.
            pub fn new(sequences: Vec<(Vec<u8>, Vec<u8>)>) -> Self {
                Self {
                    inner: #mock_name::new(sequences),
                }
            }

            /// Configure to return an error after N commands.
            pub fn with_error_after(mut self, n: usize) -> Self {
                self.inner = self.inner.with_error_after(n);
                self
            }

            /// Configure to timeout after N commands.
            pub fn with_timeout_after(mut self, n: usize) -> Self {
                self.inner = self.inner.with_timeout_after(n);
                self
            }

            /// Configure to delay responses by the given milliseconds.
            pub fn with_delay(mut self, delay_ms: u64) -> Self {
                self.inner = self.inner.with_delay(delay_ms);
                self
            }

            /// Get the history of commands sent to this transport.
            pub fn command_history(&self) -> Vec<Vec<u8>> {
                self.inner.command_history()
            }

            /// Reset the mock transport state.
            pub fn reset(&self) {
                self.inner.reset()
            }

            /// Add a new command/response sequence.
            pub fn add_sequence(&mut self, command: Vec<u8>, response: Vec<u8>) {
                self.inner.add_sequence(command, response);
            }
        }

        #[cfg(feature = "async")]
        #[async_trait::async_trait]
        impl crate::transport::AsyncTransport for #async_mock_name {
            async fn send_and_receive(&mut self, data: &[u8]) -> Result<Vec<u8>, crate::Error> {
                let mut index = self.inner.current_index.borrow_mut();
                let mut history = self.inner.command_history.borrow_mut();

                // Record the command
                history.push(data.to_vec());

                // Check for error injection
                if let Some(error_after) = self.inner.error_after {
                    if history.len() > error_after {
                        return Err(crate::Error::Transport("Injected error".into()));
                    }
                }

                // Check for timeout injection
                if let Some(timeout_after) = self.inner.timeout_after {
                    if history.len() > timeout_after {
                        return Err(crate::Error::Timeout);
                    }
                }

                // Simulate delay if configured
                if let Some(delay_ms) = self.inner.delay_ms {
                    tokio::time::sleep(std::time::Duration::from_millis(delay_ms)).await;
                }

                // Find matching sequence
                if *index < self.inner.sequences.len() {
                    let (expected_cmd, response) = &self.inner.sequences[*index];
                    if data == expected_cmd.as_slice() {
                        *index += 1;
                        Ok(response.clone())
                    } else {
                        Err(crate::Error::Transport(format!(
                            "Unexpected command. Expected {:?}, got {:?}",
                            expected_cmd, data
                        )))
                    }
                } else {
                    Err(crate::Error::Transport("No more mock responses available".into()))
                }
            }
        }
    };

    TokenStream::from(expanded)
}
/// Generate validation tests for VISCA commands.
///
/// This macro generates comprehensive test suites that validate:
/// - Command byte sequences against documentation
/// - Parameter bounds for all camera profiles
/// - Response parsing for all command types
/// - Error cases and edge conditions
///
/// # Example
///
/// ```rust,ignore
/// #[visca_test_suite]
/// mod zoom_command_tests {
///     use super::*;
///     
///     #[test_command(
///         command = "ZoomCommand::Direct(ZoomPosition::new(0x4000)?)",
///         expected_bytes = "[0x81, 0x01, 0x04, 0x47, 0x04, 0x00, 0x00, 0x00, 0xFF]",
///         profiles = ["PTZOpticsG2", "SonyEVID70"]
///     )]
///     fn test_zoom_direct() {}
///     
///     #[test_bounds(
///         parameter = "zoom_position",
///         min = "0x0000",
///         max = "0x7000",
///         profiles = ["PTZOpticsG2"]
///     )]
///     fn test_zoom_bounds() {}
/// }
/// ```
#[proc_macro_attribute]
pub fn visca_test_suite(_attr: TokenStream, item: TokenStream) -> TokenStream {
    let input = parse_macro_input!(item as syn::ItemMod);
    let mod_name = &input.ident;
    let mod_vis = &input.vis;
    let mod_attrs = &input.attrs;

    let content = match &input.content {
        Some((_, items)) => items,
        None => {
            return syn::Error::new_spanned(
                &input,
                "visca_test_suite must be applied to a module with content",
            )
            .to_compile_error()
            .into();
        }
    };

    let mut generated_tests = Vec::new();

    // Process each item in the module
    for item in content {
        if let syn::Item::Fn(func) = item {
            // Check for test attributes
            for attr in &func.attrs {
                if attr.path().is_ident("test_command") {
                    let test = generate_command_test(func, attr);
                    generated_tests.push(test);
                } else if attr.path().is_ident("test_bounds") {
                    let test = generate_bounds_test(func, attr);
                    generated_tests.push(test);
                } else if attr.path().is_ident("test_response") {
                    let test = generate_response_test(func, attr);
                    generated_tests.push(test);
                }
            }
        }
    }

    let expanded = quote! {
        #(#mod_attrs)*
        #mod_vis mod #mod_name {
            use super::*;

            #(#generated_tests)*
        }
    };

    TokenStream::from(expanded)
}

/// Generate a test for command byte sequences
fn generate_command_test(func: &syn::ItemFn, attr: &syn::Attribute) -> proc_macro2::TokenStream {
    let fn_name = &func.sig.ident;
    let mut command_expr = None;
    let mut expected_bytes = None;
    let mut profiles = Vec::new();

    // Parse test attributes
    let _ = attr.parse_nested_meta(|meta| {
        if meta.path.is_ident("command") {
            let value: syn::LitStr = meta.value()?.parse()?;
            command_expr = Some(value.value());
        } else if meta.path.is_ident("expected_bytes") {
            let value: syn::LitStr = meta.value()?.parse()?;
            expected_bytes = Some(value.value());
        } else if meta.path.is_ident("profiles") {
            let value: syn::LitStr = meta.value()?.parse()?;
            // Parse the profiles array
            let profiles_str = value.value();
            profiles = profiles_str
                .trim_start_matches('[')
                .trim_end_matches(']')
                .split(',')
                .map(|s| s.trim().trim_matches('"').to_string())
                .collect();
        }
        Ok(())
    });

    let command_tokens: proc_macro2::TokenStream = command_expr
        .unwrap_or_default()
        .parse()
        .unwrap_or_else(|_| quote! { panic!("Invalid command expression") });

    let expected_tokens: proc_macro2::TokenStream = expected_bytes
        .unwrap_or_default()
        .parse()
        .unwrap_or_else(|_| quote! { vec![] });

    // Generate test for each profile
    let profile_tests: Vec<_> = profiles
        .iter()
        .map(|profile| {
            let profile_ident = format_ident!("{}", profile);
            let test_name = format_ident!("{}_{}", fn_name, profile.to_lowercase());

            quote! {
                #[test]
                fn #test_name() {
                    let cmd = #command_tokens;
                    let expected = #expected_tokens;
                    let actual = cmd.to_bytes().expect("Failed to convert command to bytes");

                    assert_eq!(
                        actual, expected,
                        "Command bytes mismatch for profile {}.\nExpected: {:?}\nActual: {:?}",
                        stringify!(#profile_ident), expected, actual
                    );
                }
            }
        })
        .collect();

    quote! {
        #(#profile_tests)*
    }
}

/// Generate a test for parameter bounds
fn generate_bounds_test(func: &syn::ItemFn, attr: &syn::Attribute) -> proc_macro2::TokenStream {
    let fn_name = &func.sig.ident;
    let mut parameter = None;
    let mut min_value = None;
    let mut max_value = None;
    let mut profiles = Vec::new();

    // Parse test attributes
    let _ = attr.parse_nested_meta(|meta| {
        if meta.path.is_ident("parameter") {
            let value: syn::LitStr = meta.value()?.parse()?;
            parameter = Some(value.value());
        } else if meta.path.is_ident("min") {
            let value: syn::LitStr = meta.value()?.parse()?;
            min_value = Some(value.value());
        } else if meta.path.is_ident("max") {
            let value: syn::LitStr = meta.value()?.parse()?;
            max_value = Some(value.value());
        } else if meta.path.is_ident("profiles") {
            let value: syn::LitStr = meta.value()?.parse()?;
            let profiles_str = value.value();
            profiles = profiles_str
                .trim_start_matches('[')
                .trim_end_matches(']')
                .split(',')
                .map(|s| s.trim().trim_matches('"').to_string())
                .collect();
        }
        Ok(())
    });

    let param_name = parameter.unwrap_or_default();
    let min_tokens: proc_macro2::TokenStream = min_value
        .unwrap_or_default()
        .parse()
        .unwrap_or_else(|_| quote! { 0 });
    let max_tokens: proc_macro2::TokenStream = max_value
        .unwrap_or_default()
        .parse()
        .unwrap_or_else(|_| quote! { 0 });

    // Generate bounds tests
    let bounds_tests: Vec<_> = profiles
        .iter()
        .map(|profile| {
            let test_name = format_ident!("{}_{}_bounds", fn_name, profile.to_lowercase());

            quote! {
                #[test]
                fn #test_name() {
                    // Test minimum value
                    let min_result = #param_name::new(#min_tokens);
                    assert!(min_result.is_ok(), "Minimum value {} should be valid", #min_tokens);

                    // Test maximum value
                    let max_result = #param_name::new(#max_tokens);
                    assert!(max_result.is_ok(), "Maximum value {} should be valid", #max_tokens);

                    // Test below minimum
                    if #min_tokens > 0 {
                        let below_min = #param_name::new(#min_tokens - 1);
                        assert!(below_min.is_err(), "Value below minimum should be invalid");
                    }

                    // Test above maximum
                    let above_max = #param_name::new(#max_tokens + 1);
                    assert!(above_max.is_err(), "Value above maximum should be invalid");
                }
            }
        })
        .collect();

    quote! {
        #(#bounds_tests)*
    }
}

/// Generate a test for response parsing
fn generate_response_test(func: &syn::ItemFn, attr: &syn::Attribute) -> proc_macro2::TokenStream {
    let fn_name = &func.sig.ident;
    let mut response_bytes = None;
    let mut expected_value = None;
    let mut response_type = None;

    // Parse test attributes
    let _ = attr.parse_nested_meta(|meta| {
        if meta.path.is_ident("response_bytes") {
            let value: syn::LitStr = meta.value()?.parse()?;
            response_bytes = Some(value.value());
        } else if meta.path.is_ident("expected") {
            let value: syn::LitStr = meta.value()?.parse()?;
            expected_value = Some(value.value());
        } else if meta.path.is_ident("response_type") {
            let value: syn::LitStr = meta.value()?.parse()?;
            response_type = Some(value.value());
        }
        Ok(())
    });

    let bytes_tokens: proc_macro2::TokenStream = response_bytes
        .unwrap_or_default()
        .parse()
        .unwrap_or_else(|_| quote! { vec![] });

    let expected_tokens: proc_macro2::TokenStream = expected_value
        .unwrap_or_default()
        .parse()
        .unwrap_or_else(|_| quote! { () });

    let type_tokens: proc_macro2::TokenStream = response_type
        .unwrap_or_default()
        .parse()
        .unwrap_or_else(|_| quote! { () });

    quote! {
        #[test]
        fn #fn_name() {
            let response_bytes = #bytes_tokens;
            let response = Response::parse(&response_bytes, Some(#type_tokens))
                .expect("Failed to parse response");

            match response {
                Response::InquiryResponse(inquiry) => {
                    let actual = inquiry.into_value();
                    assert_eq!(actual, #expected_tokens, "Response value mismatch");
                }
                _ => panic!("Expected inquiry response, got {:?}", response),
            }
        }
    }
}

/// Derive macro for generating InquiryCommand implementations with parser support
///
/// This macro eliminates boilerplate by automatically generating the `Command` trait
/// implementation with `to_bytes()`, `response_type()`, and `command_category()` methods.
/// When parser attributes are provided, it also generates a `parse_response()` method.
///
/// # Basic Usage
///
/// ```rust,ignore
/// #[derive(Debug, InquiryCommand, PartialEq)]
/// enum MyInquiry {
///     #[visca(0x00, response = Power)]
///     Power,
///     
///     #[visca(0x47, response = ZoomPosition)]
///     ZoomPos,
///     
///     #[visca(0x12, subcategory = 0x06, response = PanTiltPosition)]
///     PanTiltPos,
/// }
/// ```
///
/// # With Response Parsing
///
/// Add parser attributes to automatically generate response parsing:
///
/// ```rust,ignore
/// #[derive(Debug, InquiryCommand, PartialEq)]
/// enum MyInquiry {
///     #[visca(0x00, response = Power, parser = "bool")]
///     Power,
///
///     #[visca(0x47, response = ZoomPosition, parser = "position")]
///     ZoomPos,
///
///     #[visca(0xA1, response = Luminance, parser = "byte")]
///     Luminance,
///
///     #[visca(0x44, response = RedGain, parser = "offset", field = "gain", offset = 10)]
///     RedGain,
/// }
/// ```
///
/// # Supported Parser Types
///
/// - `"bool"` - Boolean values (0x02 = true, 0x03 = false)
/// - `"byte"` - Direct byte value
/// - `"position"` - 4-nibble position value (converts to u16)
/// - `"nibble"` - Extended nibble encoding
/// - `"offset"` - Byte value with offset subtraction
/// - `"flags"` - Bit flags (for image flip)
/// - `"mode"` - Enum value parsing
/// - `"pan_tilt"` - Special parser for pan/tilt positions
///
/// # Requirements
///
/// The `response` attribute must reference existing variants in the `ResponseType`
/// and `InquiryResponse` enums.
#[proc_macro_derive(InquiryCommand, attributes(visca))]
pub fn derive_inquiry_command(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    TokenStream::from(inquiry_command::derive_inquiry_command_impl(input))
}
