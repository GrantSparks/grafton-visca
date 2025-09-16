//! Testing toolkit for deterministic and scriptable tests.
//!
//! This module provides utilities for writing reliable, fast, and deterministic tests
//! without relying on wall-clock time or unpredictable timing behavior.

#![cfg(feature = "test-utils")]

#[cfg(feature = "async")]
pub mod deterministic_executor;
#[cfg(feature = "async")]
pub mod executor_selection;
pub mod scripted_transport;

#[cfg(feature = "async")]
pub use deterministic_executor::{
    DeterministicClock, DeterministicExecutor, DeterministicExecutorExt,
};
#[cfg(feature = "async")]
pub use executor_selection::{TestExecutorSelector, TestExecutorType, TestExecutors};
#[cfg(not(feature = "async"))]
pub use scripted_transport::ScriptedSyncTransport;
pub use scripted_transport::{helpers, Step};

#[cfg(feature = "async")]
pub use scripted_transport::ScriptedTransport;
