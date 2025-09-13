//! Runtime-neutral driver for VISCA send/recv pipeline.
//!
//! This module provides unified abstractions for the send/recv pipeline that
//! work across both async and blocking modes, eliminating code duplication
//! while maintaining zero-cost abstractions through monomorphization.

pub mod scheduler;
pub mod send;

pub use scheduler::SchedulerLike;
pub(crate) use send::send_one;
pub use send::SendGuard;
