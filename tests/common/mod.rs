#![allow(missing_docs)]
//! Common test utilities for grafton-visca tests.
//!
//! This module provides shared mock implementations and utilities
//! to avoid code duplication across test files.

// Allow unsafe in tests for creating mock wakers
#![allow(unsafe_code)]

// Re-export submodules
pub mod builders;
pub mod helpers;
pub mod macros;
pub mod mock_transport_enhanced;
pub mod patterns;
pub mod protocol_validator;
pub mod response_builder;
pub mod scenario_builder;
pub mod test_fixtures;

// Re-export commonly used items
pub use mock_transport_enhanced::{MockTransport, MockTransportBuilder, MockResponse};
pub use patterns::*;
pub use protocol_validator::{ProtocolValidator, ValidationMode, ValidationError};
pub use response_builder::{ResponseBuilder, patterns as response_patterns};
pub use scenario_builder::{ScenarioBuilder, TestScenario};
pub use test_fixtures::{CommandFixtures, generators};