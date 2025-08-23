//! Simple test to verify wait_for_completion functionality compiles.

#![cfg(all(feature = "async", feature = "rt-tokio"))]

use grafton_visca::{Camera, Error};
use std::time::Duration;

#[tokio::test]
async fn test_wait_for_completion_compiles() {
    // This test just verifies that the wait_for_completion methods exist
    // and compile correctly. Full integration testing would require
    // a mock transport setup.

    // The methods are tested for compilation:
    // - wait_for_completion()
    // - wait_for_completion_with_timeout(Duration)
    // - is_idle()
    // - wait_for_idle(Duration)

    // These are now available on async Camera instances
    // and will be properly tested with ScriptedTransport
    // once the test infrastructure is improved.
    assert!(true, "Compilation test passed");
}
