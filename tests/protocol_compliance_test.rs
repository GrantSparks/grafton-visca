//! Protocol compliance tests for VISCA error handling.
//!
//! Every test here drives a real [`RuntimeHandle`] over a scripted transport and
//! asserts on bytes the library actually produced and on errors the library
//! actually returned. The scenarios come from the unified VISCA reference:
//!
//! * `90 60 03 FF` (Command Buffer Full) is always retryable, regardless of the
//!   command category.
//! * `90 6y 41 FF` (Command Not Executable) is retried for Movement and Preset
//!   commands, which are the categories that transiently report "still settling".
//! * The same `0x41` is terminal for Quick commands, which require the caller to
//!   change camera state first (for example, switching to manual focus).
//! * Inquiries receive a Data Reply with no ACK.
//! * The controller never has more than two commands in flight, so a well-behaved
//!   controller does not provoke the camera's "third command" buffer-full reply.

#![cfg(all(feature = "mode-async", feature = "test-utils"))]

#[cfg(feature = "runtime-tokio")]
mod tokio_protocol_compliance {
    use std::sync::Arc;
    use std::time::Duration;

    use grafton_visca::{
        camera::profiles::GenericVisca,
        command::{InquiryData, PowerInquiry, PowerOn, Response, Zoom},
        runtime::testing::RuntimeHandle,
        testing::testkit::{helpers, ScriptedTransport, Step},
        transport::{BackoffStrategy, RetryConfig},
        CameraId, Error, TokioExecutor,
    };

    /// Keep retry backoff short so a scripted retry does not add real seconds to
    /// the suite; the retry *decision* is what is under test, not the delay.
    fn fast_retry() -> RetryConfig {
        RetryConfig {
            max_retries: 3,
            base_retry_delay: Duration::from_millis(5),
            max_retry_duration: Duration::from_secs(5),
            backoff_strategy: BackoffStrategy::Constant,
        }
    }

    async fn runtime_with(
        steps: Vec<Step>,
    ) -> (
        RuntimeHandle<GenericVisca, TokioExecutor>,
        ScriptedTransport<TokioExecutor>,
    ) {
        let executor = Arc::new(TokioExecutor::from_current().expect("tokio runtime"));
        let transport: ScriptedTransport<TokioExecutor> =
            ScriptedTransport::new(steps).with_executor(executor.clone());
        let observed = transport.clone();
        let runtime = RuntimeHandle::new_with_config(transport, executor, None, fast_retry())
            .await
            .expect("runtime should start");
        (runtime, observed)
    }

    /// Per spec: buffer full means "queue it and retry when a slot frees", for
    /// every command category. `PowerOn` is a Quick command, which is *not*
    /// retryable for `0x41`, so a success here isolates the buffer-full rule.
    #[tokio::test]
    async fn buffer_full_is_retried_for_any_command_category() {
        let (runtime, transport) = runtime_with(helpers::buffer_full_then_success(1)).await;

        let result = runtime
            .send_command(&PowerOn::new(), CameraId::CAMERA_1, None)
            .await;

        assert!(
            result.is_ok(),
            "0x03 must be retried even for a Quick command, got {result:?}"
        );
        assert_eq!(
            transport.sent().len(),
            2,
            "buffer full must produce exactly one resend"
        );

        let metrics = runtime.metrics().await.expect("metrics");
        assert!(
            metrics.busy_errors >= 1,
            "the buffer-full reply should be counted as a busy error: {metrics:?}"
        );
        assert!(
            metrics.commands_retried >= 1,
            "the buffer-full reply should be counted as a retry: {metrics:?}"
        );

        runtime.shutdown().await.expect("shutdown");
    }

    /// Per spec: "the next command gets a one-time 41 FF error (camera busy);
    /// controller should catch that and retry". `Zoom::TeleStd` is a Movement
    /// command, one of the two categories where `0x41` is transient.
    #[tokio::test]
    async fn not_executable_is_retried_for_movement_commands() {
        let (runtime, transport) = runtime_with(helpers::not_executable_then_success(1)).await;

        let result = runtime
            .send_command(&Zoom::TeleStd, CameraId::CAMERA_1, None)
            .await;

        assert!(
            result.is_ok(),
            "0x41 must be retried for Movement commands, got {result:?}"
        );
        assert_eq!(
            transport.sent().len(),
            2,
            "a Movement 0x41 must produce exactly one resend"
        );

        runtime.shutdown().await.expect("shutdown");
    }

    /// Per spec: a Quick command such as "manual focus while in auto focus" gets
    /// `0x41` and the fix is caller-side ("switch to manual focus first"), not an
    /// automatic retry. The error must surface unchanged and the frame must not
    /// be resent.
    #[tokio::test]
    async fn not_executable_is_terminal_for_quick_commands() {
        let (runtime, transport) = runtime_with(vec![Step::OnSend {
            matches: None,
            responses: vec![helpers::not_executable(1)],
        }])
        .await;

        let result = runtime
            .send_command(&PowerOn::new(), CameraId::CAMERA_1, None)
            .await;

        assert!(
            matches!(result, Err(Error::CommandNotExecutable)),
            "0x41 must be terminal for Quick commands, got {result:?}"
        );
        assert_eq!(
            transport.sent().len(),
            1,
            "a Quick 0x41 must not be resent, sent = {:?}",
            transport.sent()
        );

        runtime.shutdown().await.expect("shutdown");
    }

    /// Per spec: inquiries are answered with a Data Reply (`90 50 ... FF`) and no
    /// ACK. The runtime must resolve the inquiry from that single frame.
    #[tokio::test]
    async fn inquiry_is_resolved_by_a_data_reply_without_ack() {
        let power_inquiry_frame = vec![0x81, 0x09, 0x04, 0x00, 0xFF];
        let (runtime, transport) = runtime_with(vec![helpers::inquiry_response(
            power_inquiry_frame.clone(),
            0,
            vec![0x90, 0x50, 0x02, 0xFF], // Data Reply: power on. No ACK precedes it.
        )])
        .await;

        let response = runtime
            .send_inquiry(&PowerInquiry, CameraId::CAMERA_1)
            .await
            .expect("inquiry should resolve from the data reply alone");

        match response {
            Response::Inquiry(InquiryData::Power { on }) => {
                assert!(on, "0x02 payload decodes to power on");
            }
            other => panic!("expected a decoded power inquiry, got {other:?}"),
        }

        assert_eq!(
            transport.sent(),
            vec![power_inquiry_frame],
            "the inquiry must be sent once, with the spec frame"
        );

        runtime.shutdown().await.expect("shutdown");
    }

    /// Per spec the camera replies `90 60 03 FF` when a third command arrives
    /// while two are in progress. The controller side of that contract is that it
    /// never sends the third frame: with two commands ACKed but not completed, a
    /// third submission stays queued until a socket frees.
    #[tokio::test]
    async fn third_command_is_withheld_while_two_are_in_flight() {
        let (runtime, transport) = runtime_with(vec![
            Step::OnSend {
                matches: None,
                responses: vec![helpers::ack(1)], // ACK only: socket 1 stays busy.
            },
            Step::OnSend {
                matches: None,
                responses: vec![helpers::ack(2)], // ACK only: socket 2 stays busy.
            },
            Step::OnSend {
                matches: None,
                responses: vec![helpers::ack(1), helpers::complete(1)],
            },
        ])
        .await;

        let first = runtime
            .send_command_with_id(&Zoom::TeleStd, CameraId::CAMERA_1, None)
            .await
            .expect("first command admitted");
        let second = runtime
            .send_command_with_id(&Zoom::WideStd, CameraId::CAMERA_1, None)
            .await
            .expect("second command admitted");

        wait_for_sent_len(&transport, 2).await;

        // Third submission: it must be admitted but must not reach the wire while
        // both sockets are occupied.
        let runtime_for_third = runtime.clone();
        let third = tokio::spawn(async move {
            runtime_for_third
                .send_command(&Zoom::Stop, CameraId::CAMERA_1, None)
                .await
        });

        tokio::time::sleep(Duration::from_millis(150)).await;
        assert_eq!(
            transport.sent().len(),
            2,
            "the controller must not send a third frame while two are in flight"
        );

        // Free a socket; the queued third command may now go out.
        transport.add_response(helpers::complete(1));
        transport.add_response(helpers::complete(2));

        let (_, first_response) = first;
        let (_, second_response) = second;
        first_response.await.expect("first command completes");
        second_response.await.expect("second command completes");

        let third_result = tokio::time::timeout(Duration::from_secs(2), third)
            .await
            .expect("third command must not hang once a socket frees")
            .expect("third command task");
        assert!(
            third_result.is_ok(),
            "third command should complete after a socket frees, got {third_result:?}"
        );
        assert_eq!(
            transport.sent().len(),
            3,
            "the third frame goes out only after a socket frees"
        );

        runtime.shutdown().await.expect("shutdown");
    }

    async fn wait_for_sent_len(transport: &ScriptedTransport<TokioExecutor>, expected: usize) {
        tokio::time::timeout(Duration::from_secs(2), async {
            while transport.sent().len() < expected {
                tokio::time::sleep(Duration::from_millis(1)).await;
            }
        })
        .await
        .unwrap_or_else(|_| panic!("expected at least {expected} sent frames"));
    }
}
