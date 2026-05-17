#![allow(missing_docs)]
#![allow(dead_code)]
//! Common test utilities for grafton-visca tests.
//!
//! This module provides shared mock implementations and utilities
//! to avoid code duplication across test files.

pub mod compile_fail;
pub mod helpers;
pub mod macros;
pub mod patterns;
pub mod profile_fixtures;
pub mod protocol_validator;
pub mod response_builder;
pub mod test_fixtures;

#[allow(unused_imports)]
pub use protocol_validator::{ProtocolValidator, ValidationMode};
