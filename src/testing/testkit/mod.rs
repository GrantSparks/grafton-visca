//! Testing toolkit for deterministic and scriptable tests.
//!
//! This module provides utilities for writing reliable, fast, and deterministic tests
//! without relying on wall-clock time or unpredictable timing behavior.

#![cfg(feature = "test-utils")]

#[cfg(feature = "mode-async")]
pub mod deterministic_executor;
#[cfg(feature = "mode-async")]
pub mod executor_selection;
pub mod scripted_transport;

#[cfg(feature = "mode-async")]
pub use deterministic_executor::{
    DeterministicClock, DeterministicExecutor, DeterministicExecutorExt,
};
#[cfg(feature = "mode-async")]
pub use executor_selection::{TestExecutorSelector, TestExecutorType, TestExecutors};
#[cfg(feature = "mode-async")]
pub use scripted_transport::ScriptedTransport;

#[cfg(not(feature = "mode-async"))]
pub use scripted_transport::ScriptedBlockingTransport;

pub use scripted_transport::{helpers, Step};
