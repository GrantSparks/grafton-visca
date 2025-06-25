// SPDX-License-Identifier: Apache-2.0
//
// Copyright (c) 2024 Grafton Machine Shed <team@grafton.ai>

//! Test generation macros for the grafton-visca library
//!
//! This module contains macros for generating comprehensive test suites,
//! mock transports, and test helper functions.

use proc_macro::TokenStream;
use quote::{format_ident, quote};
use syn::parse_macro_input;

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
pub fn visca_mock_transport(_attr: TokenStream, item: TokenStream) -> TokenStream {
    let input = parse_macro_input!(item as syn::DeriveInput);
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