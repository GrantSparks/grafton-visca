//! Owner-backed runtime-profile projections for the canonical session APIs.
//!
//! These projections erase the compile-time profile marker at the public
//! boundary. They do not create a second owner, transport lifecycle, timeout
//! model, or state cache. With `blocking`, `BlockingDynSessionCamera` provides
//! a native synchronous runtime-profile view over typed request values. With
//! `async`, `DynSessionCamera` and its object-safe noun/custom-operation traits
//! additionally erase request types where object safety requires it.
//!
//! ## Dynamic noun surface
//!
//! The async projection exposes all 14 domain nouns as object-safe traits.
//! Their methods are counterparts of the static accessor surface and return
//! boxed futures containing either an inquiry result, `()`, or an erased
//! operation handle. Applied-only operations intentionally have no
//! protocol-settlement wait. Targeted operations retain their
//! profile-selected wait through the dynamic handle, and custom typed
//! operations use the same owner admission path.

#![cfg(feature = "dyn-api")]

#[cfg(feature = "async")]
use std::{future::Future, pin::Pin};

/// A dynamically dispatched, sendable future returned by object-safe dynamic
/// API methods.
#[cfg(feature = "async")]
pub type DynFuture<'a, T> = Pin<Box<dyn Future<Output = T> + Send + 'a>>;

#[cfg(feature = "blocking")]
mod blocking_projection;
#[cfg(feature = "async")]
mod custom;
#[cfg(feature = "async")]
mod nouns;
#[cfg(feature = "async")]
mod owner_projection;

#[cfg(feature = "blocking")]
pub use blocking_projection::BlockingDynSessionCamera;
#[cfg(feature = "async")]
pub use custom::{
    submit_applied, submit_targeted, DynAppliedRequest, DynCustomOperations, DynTargetedRequest,
};
#[cfg(feature = "async")]
pub use nouns::{
    DynAdvanced, DynExposure, DynFocus, DynImage, DynMenu, DynMotion, DynMotionSync, DynNdFilter,
    DynPanTilt, DynPower, DynPresets, DynSessionCameraNouns, DynSystem, DynTally, DynWhiteBalance,
    DynZoom, DYN_NOUN_CONVENIENCE_METHODS, DYN_NOUN_CONVENIENCE_METHOD_COUNT, DYN_NOUN_COUNT,
    DYN_NOUN_INQUIRY_METHOD_COUNT, DYN_NOUN_TARGET_METHOD_COUNT,
};
#[cfg(feature = "async")]
pub use owner_projection::{
    DynAppliedOperation, DynCancellation, DynSessionCamera, DynSessionCameraControl,
    DynTargetedOperation,
};
