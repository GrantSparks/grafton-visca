// SPDX-License-Identifier: MIT OR Apache-2.0
//
// Copyright (c) 2024 Grafton Machine Shed <team@grafton.ai>

//! Procedural macros for the grafton-visca library
//!
//! This crate provides derive macros to simplify common patterns
//! in VISCA command implementations.

#![forbid(unsafe_code)]
#![deny(missing_docs)]
#![doc(html_root_url = "https://docs.rs/grafton-visca/0.5.0")]

use proc_macro::TokenStream;

use syn::{parse_macro_input, DeriveInput};

mod forward_control;
mod inquiry_command;
mod parser_templates;
mod value_macros;
mod visca_enum;

/// Derive macro for implementing ViscaValue trait for command value types
///
/// This macro automatically generates the `ViscaValue` trait implementation
/// for types that represent VISCA command values, providing methods for
/// converting to and from byte representations.
///
/// # Example
///
/// ```rust,ignore
/// use grafton_visca_macros::ViscaValue;
///
/// #[derive(ViscaValue, Debug, Copy, Clone)]
/// #[visca_value(bytes = 2)]
/// struct ZoomPosition(u16);
/// ```
#[proc_macro_derive(ViscaValue, attributes(visca_value))]
pub fn derive_visca_value(input: TokenStream) -> TokenStream {
    value_macros::derive_visca_value(input)
}

/// Derive macro for generating InquiryCommand implementations with parser support
///
/// This macro eliminates boilerplate by automatically generating the `Command` trait
/// implementation with `to_bytes()`, `response_type()`, and `command_category()` methods,
/// as well as a `From` conversion to the `InquiryCommand` enum.
/// When parser attributes are provided, it also generates a `parse_response()` method.
///
/// # Basic Usage
///
/// ```rust,ignore
/// use grafton_visca_macros::InquiryCommand;
///
/// #[derive(InquiryCommand, Debug, Copy, Clone)]
/// #[visca(command = 0x00, response = "Power", inquiry_variant = "Power")]
/// struct PowerInquiry;
///
/// #[derive(InquiryCommand, Debug, Copy, Clone)]
/// #[visca(command = 0x47, response = "ZoomPosition", inquiry_variant = "ZoomPosition")]
/// struct ZoomPositionInquiry;
///
/// #[derive(InquiryCommand, Debug, Copy, Clone)]
/// #[visca(command = 0x12, sub_command = 0x06, response = "PanTiltPosition", inquiry_variant = "PanTiltPosition")]
/// struct PanTiltPositionInquiry;
/// ```
///
/// # With Response Parsing
///
/// Add parser attributes to automatically generate response parsing:
///
/// ```rust,ignore
/// #[derive(InquiryCommand, Debug, Copy, Clone)]
/// #[visca(command = 0x00, response = "Power", inquiry_variant = "Power", parser = "bool")]
/// struct PowerInquiry;
///
/// #[derive(InquiryCommand, Debug, Copy, Clone)]
/// #[visca(command = 0x47, response = "ZoomPosition", inquiry_variant = "ZoomPosition", parser = "position")]
/// struct ZoomPositionInquiry;
///
/// #[derive(InquiryCommand, Debug, Copy, Clone)]
/// #[visca(command = 0xA1, response = "Luminance", inquiry_variant = "Luminance", parser = "byte")]
/// struct LuminanceInquiry;
///
/// #[derive(InquiryCommand, Debug, Copy, Clone)]
/// #[visca(command = 0x44, response = "RedGain", inquiry_variant = "RedGain", parser = "offset", field = "gain", offset = 10)]
/// struct RedGainInquiry;
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
/// - The struct must have the `#[visca(...)]` attribute with required fields
/// - The `response` attribute must reference existing variants in the `ResponseType` enum
/// - The `inquiry_variant` must reference existing variants in the `InquiryCommand` enum
/// - The struct should implement `Debug`, `Copy`, and `Clone` for full compatibility
#[proc_macro_derive(InquiryCommand, attributes(visca))]
pub fn derive_inquiry_command(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    TokenStream::from(inquiry_command::derive_inquiry_command_impl(input))
}

/// Derive macro for automatic enum/u8 conversions in VISCA protocol
///
/// This macro automatically generates `TryFrom<u8>` and `From<Enum> for u8`
/// implementations for enums with explicit discriminants, eliminating boilerplate
/// code for VISCA protocol value conversions.
///
/// # Requirements
///
/// - The enum must have unit variants only (no fields)
/// - All variants must have explicit discriminant values
/// - Discriminant values must be unique
/// - Discriminant values must be valid u8 values (0-255)
///
/// # Generated Implementations
///
/// The macro generates:
/// - `TryFrom<u8>` - Converts u8 values to enum variants, returning an error for invalid values
/// - `From<Enum> for u8` - Converts enum variants to their u8 discriminant values
/// - `is_valid_discriminant(u8) -> bool` - Const function to check if a value is valid
///
/// # Basic Example
///
/// ```rust,ignore
/// use grafton_visca_macros::ViscaEnum;
///
/// #[derive(Debug, Copy, Clone, PartialEq, Eq, ViscaEnum)]
/// pub enum ExposureMode {
///     Auto = 0x00,
///     Manual = 0x03,
///     Shutter = 0x0A,
///     Iris = 0x0B,
///     Bright = 0x0D,
/// }
///
/// // The macro generates:
/// // - impl TryFrom<u8> for ExposureMode { ... }
/// // - impl From<ExposureMode> for u8 { ... }
/// // - impl ExposureMode { pub const fn is_valid_discriminant(u8) -> bool { ... } }
/// ```
///
/// # Advanced Attributes
///
/// The macro supports optional attributes for customization:
///
/// ```rust,ignore
/// #[derive(Debug, Copy, Clone, PartialEq, Eq, ViscaEnum)]
/// #[visca_enum(error_type = MyError, exhaustive = false)]
/// pub enum Mode {
///     #[visca_enum(name = "Automatic Mode")]
///     Auto = 0x00,
///
///     #[visca_enum(name = "Manual Control")]
///     Manual = 0x03,
///
///     #[visca_enum(skip)]
///     _Reserved = 0xFF,  // Not included in TryFrom<u8>
/// }
/// ```
///
/// ## Enum-level attributes:
/// - `error_type` - Custom error type for TryFrom (default: `crate::error::Error`)
/// - `exhaustive` - Whether to generate exhaustive match (default: true)
///
/// ## Variant-level attributes:
/// - `name` - Custom name to use in error messages
/// - `skip` - Skip this variant in `TryFrom<u8>` (but include in `From<Enum>`)
///
/// # Error Handling
///
/// The generated `TryFrom<u8>` implementation returns an error with a descriptive
/// message listing all valid values when an invalid u8 is provided.
#[proc_macro_derive(ViscaEnum, attributes(visca_enum))]
pub fn derive_visca_enum(input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as DeriveInput);
    TokenStream::from(visca_enum::derive_visca_enum_impl(input))
}

/// Attribute macro for auto-generating CameraSession forwarding implementations
///
/// This macro eliminates boilerplate by automatically generating forwarding
/// implementations of control traits for `CameraSession`, which simply delegate
/// to the inner `Camera` instance with proper error handling.
///
/// # Usage
///
/// Apply this attribute to control trait definitions:
///
/// ```rust,ignore
/// use grafton_visca_macros::forward_control_to_session;
///
/// #[forward_control_to_session]
/// pub trait ZoomControl {
///     type Mode: Mode;
///
///     fn zoom_stop(&self) -> <Self::Mode as Mode>::Ret<'_, Result<(), Error>>;
///     fn zoom_tele_std(&self) -> <Self::Mode as Mode>::Ret<'_, Result<(), Error>>;
///     // ... more methods
/// }
/// ```
///
/// # Generated Code
///
/// The macro generates two implementations:
///
/// 1. **Async variant** (when `feature = "async"`):
///    - Forwards calls from `CameraSession<M, P, Tr, Exec>` to the inner camera
///    - Preserves the generic Mode type `M`
///
/// 2. **Blocking variant** (when `feature != "async"`):
///    - Forwards calls from `CameraSession<Blocking, P, Tr, ()>` to the inner camera
///    - Uses the concrete `Blocking` mode type
///
/// Each forwarding method:
/// - Guards access with `.as_ref().expect("Cannot access camera after session is closed")`
/// - Adds `#[inline]` for optimization
/// - Adds `#[allow(clippy::expect_used)]` to suppress lints
/// - Preserves all original method attributes and documentation
///
/// # Requirements
///
/// - The trait must have a `type Mode: Mode` associated type
/// - Methods should use `<Self::Mode as Mode>::Ret<'_, T>` for return types
/// - The trait should be implemented for `UnifiedCamera` (the actual logic)
///
/// # Benefits
///
/// - **Eliminates duplication**: No need to manually write forwarding impls
/// - **Prevents drift**: Changes to trait methods automatically propagate
/// - **Feature-gate aware**: Handles both async and blocking configurations
/// - **Zero runtime cost**: Generated code is identical to hand-written forwarding
#[proc_macro_attribute]
pub fn forward_control_to_session(_attr: TokenStream, input: TokenStream) -> TokenStream {
    let input = parse_macro_input!(input as syn::ItemTrait);
    TokenStream::from(forward_control::forward_control_to_session_impl(input))
}
