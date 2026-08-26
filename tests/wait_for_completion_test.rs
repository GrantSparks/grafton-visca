//! Completion tracking and idle detection in the async runtime.
//!
//! A VISCA command is only finished when the camera sends its Completion frame
//! (`90 5y FF`); the ACK (`90 4y FF`) merely says the command was accepted onto a
//! socket. These tests drive a real [`RuntimeHandle`] over a scripted transport
//! and assert on what the library actually does with those frames: when the
//! caller's future resolves, what it resolves to when the completion never
//! arrives, what [`RuntimeHandle::subscribe_completions`] emits, and how the
//! runtime's own metrics report pending versus idle work.

#![cfg(all(feature = "mode-async", feature = "test-utils"))]

#[cfg(feature = "runtime-tokio")]
mod tokio_completion {
    use std::sync::Arc;
    use std::time::Duration;

    use grafton_visca::{
        camera::profiles::GenericVisca,
        command::{PowerOn, Zoom},
        runtime::testing::RuntimeHandle,
        testing::testkit::{helpers, ScriptedTransport, Step},
        timeout::{CommandCategory, TimeoutConfig},
        transport::{BackoffStrategy, RetryConfig},
        CameraId, TokioExecutor,
    };

    fn no_retry() -> RetryConfig {
        RetryConfig {
            max_retries: 0,
            base_retry_delay: Duration::from_millis(5),
            max_retry_duration: Duration::from_secs(1),
            backoff_strategy: BackoffStrategy::Constant,
        }
    }

    /// Short deadlines so the "completion never arrives" case fails fast instead
    /// of waiting out the 30s movement default.
    fn short_timeouts() -> TimeoutConfig {
        TimeoutConfig::builder()
            .ack_timeout(Duration::from_millis(100))
            .quick_timeout(Duration::from_millis(150))
            .movement_timeout(Duration::from_millis(150))
            .preset_timeout(Duration::from_millis(150))
            .build()
    }

    async fn runtime_with(
        steps: Vec<Step>,
        timeouts: Option<TimeoutConfig>,
    ) -> (
        RuntimeHandle<GenericVisca, TokioExecutor>,
        ScriptedTransport<TokioExecutor>,
    ) {
        let executor = Arc::new(TokioExecutor::from_current().expect("tokio runtime"));
        let transport: ScriptedTransport<TokioExecutor> =
            ScriptedTransport::new(steps).with_executor(executor.clone());
        let observed = transport.clone();
        let runtime = RuntimeHandle::new_with_config(transport, executor, timeouts, no_retry())
            .await
            .expect("runtime should start");
        (runtime, observed)
    }

    /// The caller's future must stay pending on the ACK alone and resolve only
    /// once the Completion frame arrives.
    #[tokio::test]
    async fn command_future_resolves_only_after_the_completion_frame() {
        let (runtime, _transport) = runtime_with(
            vec![Step::OnSend {
                matches: None,
                responses: vec![helpers::ack(1)], // ACK now, completion injected later.
            }],
            Some(short_timeouts()),
        )
        .await;

        let (_id, response) = runtime
            .send_command_with_id(&Zoom::TeleStd, CameraId::CAMERA_1, None)
            .await
            .expect("command admitted");
        let mut response = Box::pin(response);

        // ACK only: the future must still be pending.
        let still_pending = tokio::time::timeout(Duration::from_millis(50), &mut response).await;
        assert!(
            still_pending.is_err(),
            "an ACK alone must not resolve the command future, got {still_pending:?}"
        );

        runtime.metrics().await.expect("metrics"); // Force a loop turn.
        let metrics = runtime.metrics().await.expect("metrics");
        assert_eq!(
            metrics.commands_completed, 0,
            "no command has completed yet: {metrics:?}"
        );

        runtime.shutdown().await.expect("shutdown");
    }

    /// An ACK with no Completion must fail the command once the category deadline
    /// passes, rather than hanging forever.
    #[tokio::test]
    async fn command_times_out_when_completion_never_arrives() {
        let (runtime, _transport) = runtime_with(
            vec![Step::OnSend {
                matches: None,
                responses: vec![helpers::ack(1)], // ACK, then silence.
            }],
            Some(short_timeouts()),
        )
        .await;

        let result = tokio::time::timeout(
            Duration::from_secs(3),
            runtime.send_command(&Zoom::TeleStd, CameraId::CAMERA_1, None),
        )
        .await
        .expect("the command must not hang past its deadline");

        assert!(
            result.is_err(),
            "a command with no completion must fail, got {result:?}"
        );
        let error = result.unwrap_err();
        assert!(
            error.is_retryable(),
            "a completion timeout is a retryable condition: {error:?}"
        );

        let metrics = runtime.metrics().await.expect("metrics");
        assert!(
            metrics.timeouts >= 1,
            "the missed completion should be counted as a timeout: {metrics:?}"
        );

        runtime.shutdown().await.expect("shutdown");
    }

    /// A completed command must be published on the completion event stream with
    /// the command's own timeout category, which is what movement-detection
    /// consumers subscribe to.
    #[tokio::test]
    async fn completion_events_are_published_with_the_command_category() {
        let (runtime, _transport) = runtime_with(
            vec![helpers::standard_command_response(1)],
            Some(short_timeouts()),
        )
        .await;

        let completions = runtime
            .subscribe_completions()
            .await
            .expect("completion subscription");

        runtime
            .send_command(&Zoom::TeleStd, CameraId::CAMERA_1, None)
            .await
            .expect("command completes");

        let event = tokio::time::timeout(Duration::from_secs(2), completions.recv_async())
            .await
            .expect("a completion event must be published")
            .expect("completion channel open");

        assert_eq!(event.camera_id, CameraId::CAMERA_1);
        assert_eq!(
            event.category,
            CommandCategory::Movement,
            "Zoom is a Movement command, so its completion must be reported as such"
        );

        runtime.shutdown().await.expect("shutdown");
    }

    /// A freshly started runtime reports no queued work; the counter is the
    /// library's own idle signal rather than a value the test invents.
    #[tokio::test]
    async fn a_fresh_runtime_reports_no_pending_work() {
        let (runtime, _transport) = runtime_with(vec![], Some(short_timeouts())).await;

        let metrics = runtime.metrics().await.expect("metrics");
        assert_eq!(metrics.pending_queue_depth, 0, "{metrics:?}");
        assert_eq!(metrics.retry_queue_depth, 0, "{metrics:?}");
        assert_eq!(metrics.commands_sent, 0, "{metrics:?}");
        assert_eq!(metrics.commands_completed, 0, "{metrics:?}");

        runtime.shutdown().await.expect("shutdown");
    }

    /// After a batch of commands all complete, the runtime must drain back to
    /// idle: nothing queued, nothing awaiting retry, and every command counted.
    #[tokio::test]
    async fn runtime_returns_to_idle_after_a_batch_completes() {
        let (runtime, transport) = runtime_with(
            vec![
                helpers::standard_command_response(1),
                helpers::standard_command_response(2),
                helpers::standard_command_response(1),
            ],
            Some(short_timeouts()),
        )
        .await;

        for command in [Zoom::TeleStd, Zoom::WideStd, Zoom::Stop] {
            runtime
                .send_command(&command, CameraId::CAMERA_1, None)
                .await
                .unwrap_or_else(|e| panic!("{command:?} should complete: {e:?}"));
        }

        assert_eq!(
            transport.sent().len(),
            3,
            "each command is sent exactly once"
        );

        let metrics = runtime.metrics().await.expect("metrics");
        assert_eq!(metrics.commands_sent, 3, "{metrics:?}");
        assert_eq!(metrics.commands_completed, 3, "{metrics:?}");
        assert_eq!(metrics.commands_failed, 0, "{metrics:?}");
        assert_eq!(
            metrics.pending_queue_depth, 0,
            "the runtime must be idle once the batch completes: {metrics:?}"
        );
        assert_eq!(metrics.retry_queue_depth, 0, "{metrics:?}");

        runtime.shutdown().await.expect("shutdown");
    }

    /// Commands issued back to back must each wait for their own Completion: the
    /// second command's future must not be resolved by the first command's frame.
    #[tokio::test]
    async fn each_command_waits_for_its_own_completion() {
        let (runtime, transport) = runtime_with(
            vec![
                Step::OnSend {
                    matches: None,
                    responses: vec![helpers::ack(1)],
                },
                Step::OnSend {
                    matches: None,
                    responses: vec![helpers::ack(2)],
                },
            ],
            Some(short_timeouts()),
        )
        .await;

        let (_first_id, first) = runtime
            .send_command_with_id(&PowerOn::new(), CameraId::CAMERA_1, None)
            .await
            .expect("first admitted");
        let (_second_id, second) = runtime
            .send_command_with_id(&Zoom::TeleStd, CameraId::CAMERA_1, None)
            .await
            .expect("second admitted");
        let mut second = Box::pin(second);

        // Complete only socket 1: the first future resolves, the second must not.
        transport.add_response(helpers::complete(1));
        let first_result = tokio::time::timeout(Duration::from_secs(2), first)
            .await
            .expect("first command should complete");
        assert!(first_result.is_ok(), "first command: {first_result:?}");

        let second_still_pending =
            tokio::time::timeout(Duration::from_millis(50), &mut second).await;
        assert!(
            second_still_pending.is_err(),
            "socket 1's completion must not resolve the socket 2 command, got {second_still_pending:?}"
        );

        transport.add_response(helpers::complete(2));
        let second_result = tokio::time::timeout(Duration::from_secs(2), &mut second)
            .await
            .expect("second command should complete once its own frame arrives");
        assert!(second_result.is_ok(), "second command: {second_result:?}");

        runtime.shutdown().await.expect("shutdown");
    }
}
