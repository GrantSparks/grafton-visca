//! A deliberately thin macro provider.
//!
//! The consumer depends only on this crate, so a successful expansion cannot
//! rely on `grafton-visca` or any helper crate being in the consumer's extern
//! prelude.

pub use grafton_visca::visca_range_type;
