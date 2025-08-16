//! VISCA protocol encoding and decoding.
//!
//! This module provides utilities for working with the VISCA protocol,
//! including command encoding, response parsing, and transport encapsulation.

pub mod decode;
pub mod encode;

pub use decode::{
    extract_inquiry_value, find_next_frame, is_complete_frame, parse_frames, parse_response,
    ViscaResponse,
};

pub use encode::{
    encode_address_set, encode_cancel, encode_frame, encode_if_clear, encode_sony_frame,
    FrameBuilder, PayloadType, SequenceGenerator, SonyHeader, VISCA_TERMINATOR,
};
