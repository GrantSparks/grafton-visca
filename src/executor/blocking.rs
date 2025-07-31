//! Blocking-only executor stubs.
//!
//! This module provides empty types when the async feature is disabled,
//! allowing the code to compile without async support.

// No async types are exposed when the async feature is disabled.
// The block_on and timeout functions in mod.rs handle all blocking execution needs.
