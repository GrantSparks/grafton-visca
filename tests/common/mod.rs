#![allow(missing_docs)]
//! Common test utilities for grafton-visca tests.
//!
//! This module provides shared mock implementations and utilities
//! to avoid code duplication across test files.

// Allow dead code in test utilities since not all utilities are used in every test file
#![allow(dead_code)]

// Re-export submodules
pub mod helpers;
pub mod macros;
pub mod patterns;
pub mod protocol_validator;
pub mod response_builder;
pub mod test_fixtures;

// Re-export commonly used items
#[allow(unused_imports)]
pub use protocol_validator::{ProtocolValidator, ValidationMode};
