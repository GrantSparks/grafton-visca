//! Protocol auto-detection for VISCA cameras.
//!
//! This module is a compatibility facade that re-exports the protocol detection
//! functionality from `protocol::detect` to maintain backward compatibility.
//!
//! New code should use `protocol::detect` directly.

#[doc(hidden)]
#[deprecated(
    since = "0.6.0",
    note = "Use `protocol::detect` instead. This module is a compatibility facade."
)]
pub use crate::protocol::detect::{
    DetectionCandidate, DetectionResult, ProtocolDetector, TransportProtocol,
};
