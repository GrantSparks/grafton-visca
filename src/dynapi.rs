//! Owner-backed dynamic projections for the canonical session API.
//!
//! The dynamic API erases only profile/request types at the public boundary.
//! It does not create a second owner, transport lifecycle, timeout model, or
//! state cache. Built-in noun methods, motion observation, and custom
//! operation submission all delegate to the same owner-backed async session.
//!
//! ## Dynamic noun surface
//!
//! [`DynSessionCameraNouns`] exposes the 14 domain nouns [`DynPower`],
//! [`DynZoom`], [`DynSystem`], [`DynPanTilt`], [`DynFocus`], [`DynExposure`],
//! [`DynWhiteBalance`], [`DynImage`], [`DynPresets`], [`DynTally`],
//! [`DynNdFilter`], [`DynMotionSync`], [`DynMenu`], and [`DynAdvanced`].
//! [`DynMotion`] is the separate motion-safety and observation view. Their
//! methods are the object-safe counterparts of the static accessor surface
//! and return [`DynFuture`] values containing either an inquiry result, `()`,
//! [`DynAppliedOperation`], or [`DynTargetedOperation`]. Applied-only
//! operations intentionally have no physical-settlement wait; targeted
//! operations retain that wait through their dynamic handle. Custom typed
//! operations use [`DynCustomOperations`] and the same owner admission path.

#![cfg(feature = "dyn-api")]

use std::{future::Future, pin::Pin};

/// A dynamically dispatched, sendable future returned by object-safe dynamic
/// API methods.
pub type DynFuture<'a, T> = Pin<Box<dyn Future<Output = T> + Send + 'a>>;

mod custom;
mod nouns;
mod owner_projection;

pub use custom::{
    submit_applied, submit_targeted, DynAppliedRequest, DynCustomOperations, DynTargetedRequest,
};
pub use nouns::{
    DynAdvanced, DynExposure, DynFocus, DynImage, DynMenu, DynMotion, DynMotionSync, DynNdFilter,
    DynPanTilt, DynPower, DynPresets, DynSessionCameraNouns, DynSystem, DynTally, DynWhiteBalance,
    DynZoom, DYN_NOUN_COUNT, DYN_NOUN_INQUIRY_METHOD_COUNT, DYN_NOUN_TARGET_METHOD_COUNT,
};
pub use owner_projection::{
    DynAppliedOperation, DynCancellation, DynSessionCamera, DynSessionCameraControl,
    DynTargetedOperation,
};
