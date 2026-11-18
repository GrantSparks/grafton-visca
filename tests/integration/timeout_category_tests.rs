//! Focused tests for timeout primitives and deterministic transport behavior.
//!
//! Category-to-deadline preparation is covered by the production-owned tests
//! in `src/prepared.rs`. This integration test pins the public category
//! defaults and the scripted transport's timeout behaviour; it does not claim
//! that a raw transport smoke test exercises owner deadline selection.

#[cfg(all(test, feature = "runtime-tokio", feature = "test-utils"))]
mod timeout_tests {
    use grafton_visca::{
        profiles::CommandTimeouts,
        runtime::TokioExecutor,
        testing::testkit::{helpers, ScriptedTransport, Step},
        transport::AsyncTransport,
    };

    use std::{sync::Arc, time::Duration};

    const CUSTOM_IP_STREAM: grafton_visca::testing::testkit::ScriptedTransportIdentity =
        grafton_visca::testing::testkit::ScriptedTransportIdentity::Custom {
            semantics: grafton_visca::transport::SendSemantics::Stream,
            addressing: grafton_visca::transport::AddressingMode::Ip,
        };

    #[test]
    fn category_defaults_are_explicit_and_stable() {
        let defaults = CommandTimeouts::default();
        assert_eq!(defaults.quick_timeout(), Duration::from_secs(5));
        assert_eq!(defaults.movement_timeout(), Duration::from_secs(30));
        assert_eq!(defaults.preset_timeout(), Duration::from_secs(60));
        assert_eq!(defaults.long_running_timeout(), Duration::from_secs(300));
        assert_eq!(defaults.network_timeout(), Duration::from_secs(5));
    }

    #[tokio::test(start_paused = true)]
    async fn test_scripted_transport_basic() {
        // Test basic ScriptedTransport functionality
        let executor = Arc::new(TokioExecutor::from_handle(tokio::runtime::Handle::current()));

        let mut transport = ScriptedTransport::new(
            CUSTOM_IP_STREAM,
            vec![Step::on_send(None, vec![vec![0x90, 0x41, 0xFF]])],
        )
        .with_executor(executor.clone());

        // Send a command
        transport
            .send(&[0x81, 0x01, 0x04, 0x00, 0xFF])
            .await
            .unwrap();

        // Should receive the scripted response
        let mut buf = vec![0u8; 1024];
        let n = transport.recv_into(&mut buf).await.unwrap().copied_len();
        assert_eq!(&buf[..n], &[0x90, 0x41, 0xFF]);

        // Verify the command was recorded
        let sent = transport.sent();
        assert_eq!(sent.len(), 1);
        assert_eq!(sent[0], vec![0x81, 0x01, 0x04, 0x00, 0xFF]);
    }

    #[tokio::test(start_paused = true)]
    async fn test_scripted_transport_no_response() {
        // Test ScriptedTransport with injected timeout error to avoid stalling
        let executor = Arc::new(TokioExecutor::from_handle(tokio::runtime::Handle::current()));

        // Use injected timeout error instead of expecting recv() to timeout on its own
        let mut transport = ScriptedTransport::new(
            CUSTOM_IP_STREAM,
            vec![
                helpers::errors::transport_timeout(), // Inject timeout error for recv()
            ],
        )
        .with_executor(executor.clone());

        // Send a command
        transport
            .send(&[0x81, 0x01, 0x04, 0x00, 0xFF])
            .await
            .unwrap();

        // Should get the injected timeout error immediately (no hanging)
        let mut buf = vec![0u8; 1024];
        let result = transport.recv_into(&mut buf).await;
        assert!(matches!(result, Err(grafton_visca::Error::Timeout { .. })));
    }
}
