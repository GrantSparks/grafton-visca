//! Unit tests for blocking transport layer
//!
//! These tests verify the correct handling of VISCA frames, especially
//! edge cases like back-to-back frames in a single TCP read.

#[cfg(test)]
mod framing_tests;
