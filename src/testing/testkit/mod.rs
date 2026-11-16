//! Testing toolkit for deterministic and scriptable tests.
//!
//! This module provides utilities for writing reliable, fast, and deterministic tests
//! without relying on wall-clock time or unpredictable timing behavior.

#![cfg(feature = "test-utils")]

mod scripted_transport;

testkit_utility_exports!(exports);
