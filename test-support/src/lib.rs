//! Shared helpers for grafton-visca's integration tests.
//!
//! An unpublished workspace crate that the root package takes as a
//! dev-dependency. Each integration test binary uses a different subset of
//! these helpers; as public items of a library they are not dead code in the
//! binaries that leave some of them unused.
//!
//! The crate's features mirror grafton-visca's `blocking`, `async` and
//! `test-utils` and are enabled by them, so a feature leg compiles only the helpers it can use. The
//! macros ([`facade_matrix!`], [`runtime_matrix!`],
//! [`active_grafton_visca_features!`]) expand in the test crate, where their
//! `cfg(feature = ..)` checks read grafton-visca's own features.

pub mod compile_fail;
#[cfg(any(feature = "blocking", feature = "async"))]
pub mod fake_camera;
#[cfg(feature = "blocking")]
pub mod hardware;
#[cfg(feature = "blocking")]
pub mod hardware_rest;
mod matrix;
pub mod patterns;
pub mod profile_fixtures;
pub mod retry_requests;
pub mod source_scan;
