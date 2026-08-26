//! Priority scheduling and busy-retry cascade behaviour of the async runtime.
//!
//! These tests drive a real [`RuntimeHandle`] over a scripted transport. VISCA
//! allows at most two commands in flight, so a submission that arrives while both
//! sockets are occupied has to wait in the scheduler's pending queue. The queue is
//! ordered by [`Priority`] (highest first, FIFO within a priority), and a `0x03`
//! (Command Buffer Full) reply is retried under the configured [`RetryConfig`]
//! until the retry budget is spent.

#![cfg(all(feature = "mode-async", feature = "test-utils"))]

#[cfg(feature = "runtime-tokio")]
mod tokio_priority_cascade {
    use std::sync::Arc;
    use std::time::Duration;

    use grafton_visca::{
        camera::profiles::GenericVisca,
        command::{PowerOn, PowerStandby, Zoom},
        runtime::testing::{Priority, RuntimeHandle},
        testing::testkit::{helpers, ScriptedTransport, Step},
        timeout::TimeoutConfig,
        transport::{BackoffStrategy, RetryConfig},
        CameraId, Error, TokioExecutor,
    };

    const ZOOM_STOP: [u8; 6] = [0x81, 0x01, 0x04, 0x07, 0x00, 0xFF];
    const ZOOM_TELE_STD: [u8; 6] = [0x81, 0x01, 0x04, 0x07, 0x02, 0xFF];
    const ZOOM_WIDE_STD: [u8; 6] = [0x81, 0x01, 0x04, 0x07, 0x03, 0xFF];

    fn fast_retry(max_retries: u32) -> RetryConfig {
        RetryConfig {
            max_retries,
            base_retry_delay: Duration::from_millis(5),
            max_retry_duration: Duration::from_secs(5),
            backoff_strategy: BackoffStrategy::Constant,
        }
    }

    async fn runtime_with(
        steps: Vec<Step>,
        retry: RetryConfig,
    ) -> (
        RuntimeHandle<GenericVisca, TokioExecutor>,
        ScriptedTransport<TokioExecutor>,
    ) {
        let executor = Arc::new(TokioExecutor::from_current().expect("tokio runtime"));
        let transport: ScriptedTransport<TokioExecutor> =
            ScriptedTransport::new(steps).with_executor(executor.clone());
        let observed = transport.clone();
        // Generous per-category deadlines: these tests deliberately hold commands
        // ACKed-but-incomplete, and a deadline firing mid-test would free a socket
        // and make the ordering assertions flaky on a loaded runner.
        let timeouts = TimeoutConfig::uniform(Duration::from_secs(120));
        let runtime = RuntimeHandle::new_with_config(transport, executor, Some(timeouts), retry)
            .await
            .expect("runtime should start");
        (runtime, observed)
    }

    async fn wait_for_sent_len(transport: &ScriptedTransport<TokioExecutor>, expected: usize) {
        tokio::time::timeout(Duration::from_secs(2), async {
            while transport.sent().len() < expected {
                tokio::time::sleep(Duration::from_millis(1)).await;
            }
        })
        .await
        .unwrap_or_else(|_| {
            panic!(
                "expected at least {expected} sent frames, saw {:?}",
                transport.sent()
            )
        });
    }

    /// Both VISCA sockets are held by ACKed-but-incomplete commands, then three
    /// commands are submitted in ascending priority order. As sockets free up the
    /// scheduler must drain them in *descending* priority order, not submission
    /// order.
    #[tokio::test]
    async fn queued_commands_drain_in_priority_order_not_submission_order() {
        // Every scripted send is ACK-only, so a socket stays busy until the test
        // explicitly injects a completion.
        let ack_only = |socket: u8| Step::OnSend {
            matches: None,
            responses: vec![helpers::ack(socket)],
        };
        let (runtime, transport) = runtime_with(
            vec![
                ack_only(1),
                ack_only(2),
                ack_only(1),
                ack_only(2),
                ack_only(1),
            ],
            fast_retry(3),
        )
        .await;

        // Occupy both sockets.
        let _hold_a = runtime
            .send_command_with_id(&PowerOn::new(), CameraId::CAMERA_1, None)
            .await
            .expect("first hold admitted");
        let _hold_b = runtime
            .send_command_with_id(&PowerStandby::new(), CameraId::CAMERA_1, None)
            .await
            .expect("second hold admitted");
        wait_for_sent_len(&transport, 2).await;

        // Submit lowest priority first so submission order and priority order
        // disagree; only the priority order can produce the asserted sequence.
        let _normal = runtime
            .send_command_with_id(&Zoom::TeleStd, CameraId::CAMERA_1, Some(Priority::Normal))
            .await
            .expect("normal admitted");
        let _high = runtime
            .send_command_with_id(&Zoom::WideStd, CameraId::CAMERA_1, Some(Priority::High))
            .await
            .expect("high admitted");
        let _critical = runtime
            .send_command_with_id(&Zoom::Stop, CameraId::CAMERA_1, Some(Priority::Critical))
            .await
            .expect("critical admitted");

        // Nothing may go out while both sockets are busy.
        tokio::time::sleep(Duration::from_millis(100)).await;
        assert_eq!(
            transport.sent().len(),
            2,
            "queued commands must wait for a free socket, sent = {:?}",
            transport.sent()
        );

        // Free socket 1: the Critical command must take it.
        transport.add_response(helpers::complete(1));
        wait_for_sent_len(&transport, 3).await;
        assert_eq!(
            transport.sent()[2],
            ZOOM_STOP.to_vec(),
            "Critical must be dequeued before High and Normal"
        );

        // Free socket 2: the High command is next.
        transport.add_response(helpers::complete(2));
        wait_for_sent_len(&transport, 4).await;
        assert_eq!(
            transport.sent()[3],
            ZOOM_WIDE_STD.to_vec(),
            "High must be dequeued before Normal"
        );

        // Completing the Critical command frees socket 1 again for Normal.
        transport.add_response(helpers::complete(1));
        wait_for_sent_len(&transport, 5).await;
        assert_eq!(
            transport.sent()[4],
            ZOOM_TELE_STD.to_vec(),
            "Normal is dequeued last"
        );

        runtime.shutdown().await.expect("shutdown");
    }

    /// A camera that keeps replying `90 60 03 FF` must exhaust the retry budget
    /// and surface a terminal error rather than retrying forever.
    #[tokio::test]
    async fn persistent_busy_exhausts_the_retry_budget() {
        let busy = || Step::OnSend {
            matches: None,
            responses: vec![helpers::buffer_full(0)],
        };
        // max_retries = 2 means the original send plus two resends, then failure.
        let (runtime, transport) =
            runtime_with(vec![busy(), busy(), busy(), busy()], fast_retry(2)).await;

        let result = runtime
            .send_command(&Zoom::TeleStd, CameraId::CAMERA_1, None)
            .await;

        assert!(
            result.is_err(),
            "a camera that is permanently busy must fail the command, got {result:?}"
        );
        let error = result.unwrap_err();
        // The runtime surfaces the camera's own last error rather than a synthetic
        // one, so the caller can see why the budget was spent.
        assert!(
            matches!(error, Error::CommandBufferFull),
            "expected the camera's 0x03 to surface once the budget is spent, got {error:?}"
        );

        let sent = transport.sent().len();
        assert_eq!(
            sent, 3,
            "one original send plus two retries should reach the wire, saw {sent}"
        );

        let metrics = runtime.metrics().await.expect("metrics");
        assert!(
            metrics.busy_errors >= 3,
            "every 0x03 reply should be counted: {metrics:?}"
        );
        assert_eq!(
            metrics.commands_failed, 1,
            "the exhausted command should be counted as failed: {metrics:?}"
        );

        runtime.shutdown().await.expect("shutdown");
    }

    /// Priority must not change *whether* a busy command is retried: a Low
    /// priority command that hits `0x03` still gets its resend and still
    /// succeeds.
    #[tokio::test]
    async fn busy_retry_applies_to_low_priority_commands() {
        let (runtime, transport) =
            runtime_with(helpers::buffer_full_then_success(1), fast_retry(3)).await;

        let result = runtime
            .send_command(&Zoom::TeleStd, CameraId::CAMERA_1, Some(Priority::Low))
            .await;

        assert!(
            result.is_ok(),
            "a Low priority command must still be retried after 0x03, got {result:?}"
        );
        assert_eq!(
            transport.sent().len(),
            2,
            "0x03 must produce exactly one resend regardless of priority"
        );

        runtime.shutdown().await.expect("shutdown");
    }
}
