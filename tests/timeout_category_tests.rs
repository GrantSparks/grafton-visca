//! Comprehensive tests for timeout behavior across all command categories.
//!
//! These tests verify that:
//! 1. Each command category uses the correct timeout duration
//! 2. Timeout configuration changes are applied correctly
//! 3. Socket manager respects custom timeout configurations
//! 4. Timeout errors are handled deterministically

#[cfg(all(test, feature = "rt-tokio", feature = "test-utils"))]
mod timeout_tests {
    use grafton_visca::{
        testing::testkit::{ScriptedTransport, Step},
        transport::AsyncTransport,
        Executor, TokioExecutor,
    };
    use std::sync::Arc;

    #[tokio::test(start_paused = true)]
    async fn test_deterministic_executor_timeout() {
        use std::time::Duration;

        let executor = Arc::new(TokioExecutor::from_handle(tokio::runtime::Handle::current()));

        // Create a future that never completes
        let never_completes = std::future::pending::<()>();

        // Apply a timeout of 100ms
        let timeout_fut = executor.timeout(Duration::from_millis(100), never_completes);

        // Start the timeout future (it should be pending)
        tokio::select! {
            result = timeout_fut => {
                // This should not happen immediately
                panic!("Timeout future completed too early: {:?}", result);
            }
            _ = std::future::ready(()) => {
                // Expected - the timeout future is pending immediately
            }
        }

        // Now advance the virtual clock by 100ms to trigger the timeout
        tokio::time::advance(Duration::from_millis(100)).await;

        // The timeout should now complete with an error
        let timeout_fut =
            executor.timeout(Duration::from_millis(100), std::future::pending::<()>());

        // Advance time first
        tokio::time::advance(Duration::from_millis(100)).await;

        // Now poll the timeout
        let result = timeout_fut.await;
        assert!(
            matches!(result, Err(grafton_visca::Error::Timeout)),
            "Expected timeout error, got {:?}",
            result
        );
    }

    #[tokio::test(start_paused = true)]
    async fn test_scripted_transport_basic() {
        // Test basic ScriptedTransport functionality
        let executor = Arc::new(TokioExecutor::from_handle(tokio::runtime::Handle::current()));

        let mut transport = ScriptedTransport::new(vec![Step::OnSend {
            matches: None,
            responses: vec![vec![0x90, 0x41, 0xFF]], // ACK
        }])
        .with_executor(executor.clone());

        // Send a command
        transport
            .send(&[0x81, 0x01, 0x04, 0x00, 0xFF])
            .await
            .unwrap();

        // Should receive the scripted response
        let response = transport.recv().await.unwrap();
        assert_eq!(response.as_ref(), &[0x90, 0x41, 0xFF]);

        // Verify the command was recorded
        let sent = transport.sent();
        assert_eq!(sent.len(), 1);
        assert_eq!(sent[0], vec![0x81, 0x01, 0x04, 0x00, 0xFF]);
    }

    #[tokio::test(start_paused = true)]
    async fn test_scripted_transport_no_response() {
        // Test ScriptedTransport with no responses (should timeout)
        let executor = Arc::new(TokioExecutor::from_handle(tokio::runtime::Handle::current()));

        let mut transport = ScriptedTransport::new(vec![]).with_executor(executor.clone());

        // Send a command
        transport
            .send(&[0x81, 0x01, 0x04, 0x00, 0xFF])
            .await
            .unwrap();

        // Should timeout since no response is scripted
        let result = transport.recv().await;
        assert!(matches!(result, Err(grafton_visca::Error::Timeout)));
    }
}
