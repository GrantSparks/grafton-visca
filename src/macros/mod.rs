//! Consolidated module for all VISCA macros.
//!
//! This module provides a centralized location for all macros used in the grafton-visca crate,
//! with clear separation between public API macros and internal implementation details.
//!
//! ## Module Organization
//!
//! - `public` - Macros that form part of the stable public API
//! - `internal` - Internal implementation macros not intended for external use
//! - `test_utils` - Test-specific utilities
//!
//! ## Public API Macros
//!
//! These macros are re-exported at the crate root and are considered stable:
//!
//! - [`visca_bounded_param!`] - Create validated newtype wrappers for numeric parameters
//! - [`forward_facade!`] - Forward trait methods through wrapper types
//!
//! ## Internal Macros
//!
//! These macros are used internally for implementing VISCA commands and are not part
//! of the public API. They may change without notice:
//!
//! - Command generators: `visca_command!`, `visca_bool_command!`, `visca_builder!`,
//!   `visca_param_command!`, `visca_const_command!`
//! - Const utilities: `visca_bytes!`, `visca_prefix!`
//! - Test utilities: `visca_test!`

// Public API macros - these are exported at crate root
pub mod public;

// Internal implementation macros - not exported
pub(crate) mod internal;

// Test utilities
#[cfg(test)]
pub(crate) mod test_utils;

// Public macros are available at crate root via #[macro_export]
// No re-export needed here
