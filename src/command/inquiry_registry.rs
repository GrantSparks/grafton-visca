//! Generated inquiry registry.
//!
//! The built-in inquiry table lives in [`super::inquiry_structs`] and generates
//! response routing together with query command metadata.  This module preserves
//! the internal registry path used by response decoding code.

pub(crate) use super::inquiry_structs::dispatch;
pub use super::inquiry_structs::{InquiryData, InquiryKind};
