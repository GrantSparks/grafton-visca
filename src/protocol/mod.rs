//! VISCA protocol encoding and decoding.
//!
//! This module provides utilities for working with the VISCA protocol,
//! including command encoding, response parsing, and transport encapsulation.

#[cfg(any(feature = "blocking", feature = "async"))]
pub mod framer;
pub mod response;
pub mod sony;
