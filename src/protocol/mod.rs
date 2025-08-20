//! VISCA protocol encoding and decoding.
//!
//! This module provides utilities for working with the VISCA protocol,
//! including command encoding, response parsing, and transport encapsulation.

pub mod decode;
pub mod encode;

// Note: ViscaResponse from decode is internal and only used within the protocol module
