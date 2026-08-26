//! Issue #589: a production entry point must accept a `Priority`, and the
//! priority it accepts must actually decide dispatch order.
//!
//! Before this change every command reached the scheduler at
//! `Priority::Normal`: the camera call sites passed `None` and the blocking
//! runner hard-coded `Normal`, so `Priority::High` / `Priority::Critical` were
//! nameable but unreachable. These tests pin the wiring end to end — the
//! priority a caller sets is the priority the scheduler orders by — rather than
//! only asserting that the setter stores a value.
//!
//! Each mode runs the same A/B pair: an identical script and submission
//! sequence, differing only in the priority of the last command. The observed
//! order of bytes on the transport flips accordingly.

#[cfg(all(not(feature = "mode-async"), feature = "test-utils"))]
mod blocking_mode {
    use std::time::Duration;

    use grafton_visca::{
        camera::{profiles::PtzOpticsG2, BlockingClient, Camera},
        command::Zoom,
        mode::Blocking,
        runtime::Priority,
        testing::testkit::{helpers, ScriptedBlockingTransport, Step},
        timeout::TimeoutConfig,
        transport::RetryConfig,
        Error,
    };

    const TELE_STD: &[u8] = &[0x81, 0x01, 0x04, 0x07, 0x02, 0xFF];
    const WIDE_STD: &[u8] = &[0x81, 0x01, 0x04, 0x07, 0x03, 0xFF];
    const ZOOM_STOP: &[u8] = &[0x81, 0x01, 0x04, 0x07, 0x00, 0xFF];

    type Client = BlockingClient<PtzOpticsG2, ScriptedBlockingTransport>;

    /// A runner whose ACK timeout is short and whose retry backoff is zero, so
    /// a timed-out command is re-queued in the very next scheduler iteration.
    /// That is the one moment a blocking runner holds more than one queued
    /// command, and therefore the only place dispatch order is observable.
    fn client(steps: Vec<Step>) -> (Client, ScriptedBlockingTransport) {
        let transport = ScriptedBlockingTransport::new(steps);
        let observed = transport.clone();
        let timeout_config = TimeoutConfig::builder()
            .ack_timeout(Duration::from_millis(20))
            .build();
        let retry_config = RetryConfig {
            base_retry_delay: Duration::ZERO,
            ..RetryConfig::default()
        };
        let camera = Camera::<Blocking, PtzOpticsG2, _, ()>::new_blocking_with_config(
            transport,
            timeout_config,
            retry_config,
        )
        .expect("camera");
        (BlockingClient::from_camera(camera), observed)
    }

    /// Occupy both VISCA sockets with commands the camera never acknowledges,
    /// then run one more command at `priority` while those two are being
    /// re-queued for retry. Returns the bytes the transport observed.
    fn run_retry_contention_scenario(priority: Priority) -> Vec<Vec<u8>> {
        let (mut camera, observed) = client(vec![
            // The socket fillers are never answered: they ACK-time out and are
            // re-queued at their original (Normal) priority.
            Step::OnSend {
                matches: Some(TELE_STD.to_vec()),
                responses: vec![],
            },
            Step::OnSend {
                matches: Some(WIDE_STD.to_vec()),
                responses: vec![],
            },
            // Answers the third send only if it is the command under test.
            Step::OnSend {
                matches: Some(ZOOM_STOP.to_vec()),
                responses: vec![helpers::ack(1), helpers::complete(1)],
            },
        ]);

        assert_eq!(
            camera.command_priority(),
            Priority::Normal,
            "a freshly opened camera must submit at Normal"
        );

        // Submitted, dispatched, then detached: both sockets are now busy and
        // neither command can be answered.
        drop(camera.submit(&Zoom::TeleStd).expect("submit filler 1"));
        drop(camera.submit(&Zoom::WideStd).expect("submit filler 2"));

        camera.set_command_priority(priority);
        assert_eq!(camera.command_priority(), priority);

        // Drives the runner: the fillers ACK-time out, are promoted back onto
        // the queue, and compete with this command for the freed socket.
        let _ = camera.send_command(&Zoom::Stop);

        observed.sent()
    }

    /// Control: at the default priority the re-queued fillers keep their FIFO
    /// place, so the newest command waits behind them.
    #[test]
    fn normal_priority_waits_behind_requeued_commands() {
        let sent = run_retry_contention_scenario(Priority::Normal);

        assert_eq!(&sent[0], TELE_STD);
        assert_eq!(&sent[1], WIDE_STD);
        assert_ne!(
            &sent[2], ZOOM_STOP,
            "a Normal command must not overtake older Normal work, got {:02X?}",
            sent[2]
        );
    }

    /// Treatment: the same sequence with the handle raised to Critical sends
    /// the new command first, ahead of the older re-queued commands.
    #[test]
    fn critical_priority_preempts_requeued_commands() {
        let sent = run_retry_contention_scenario(Priority::Critical);

        assert_eq!(&sent[0], TELE_STD);
        assert_eq!(&sent[1], WIDE_STD);
        assert_eq!(
            &sent[2], ZOOM_STOP,
            "the Critical command must overtake re-queued work, got {:02X?}",
            sent[2]
        );
    }

    /// The per-command override reaches the scheduler the same way, and leaves
    /// the handle default alone.
    #[test]
    fn execute_with_priority_preempts_without_changing_the_handle() {
        let (camera, observed) = client(vec![
            Step::OnSend {
                matches: Some(TELE_STD.to_vec()),
                responses: vec![],
            },
            Step::OnSend {
                matches: Some(WIDE_STD.to_vec()),
                responses: vec![],
            },
            Step::OnSend {
                matches: Some(ZOOM_STOP.to_vec()),
                responses: vec![helpers::ack(1), helpers::complete(1)],
            },
        ]);

        drop(camera.submit(&Zoom::TeleStd).expect("submit filler 1"));
        drop(camera.submit(&Zoom::WideStd).expect("submit filler 2"));

        camera
            .execute_with_priority(Zoom::Stop, Priority::Critical)
            .expect("critical stop completes");

        let sent = observed.sent();
        assert_eq!(&sent[2], ZOOM_STOP);
        assert_eq!(
            camera.command_priority(),
            Priority::Normal,
            "a per-command override must not change the handle default"
        );
    }

    /// The priority belongs to the handle: another handle keeps its own value.
    #[test]
    fn priority_is_per_handle() {
        let (mut raised, _) = client(vec![]);
        let (untouched, _) = client(vec![]);

        raised.set_command_priority(Priority::Critical);

        assert_eq!(raised.command_priority(), Priority::Critical);
        assert_eq!(untouched.command_priority(), Priority::Normal);
    }

    /// Inquiries keep the runner's polling priority: raising a handle must not
    /// let status reads jump ahead of control commands.
    #[test]
    fn raising_the_handle_still_answers_inquiries() {
        use grafton_visca::command::ZoomPositionInquiry;

        let (mut camera, _observed) = client(vec![helpers::inquiry_response(
            vec![0x81, 0x09, 0x04, 0x47, 0xFF],
            1,
            vec![0x90, 0x50, 0x01, 0x02, 0x03, 0x04, 0xFF],
        )]);
        camera.set_command_priority(Priority::Critical);

        let response: Result<_, Error> = camera.send_command(&ZoomPositionInquiry);
        assert!(
            response.is_ok(),
            "inquiry should still complete: {response:?}"
        );
    }
}

#[cfg(all(feature = "mode-async", feature = "test-utils"))]
mod async_mode {
    use std::time::Duration;

    use grafton_visca::{
        camera::{profiles::GenericVisca, CameraBuilder},
        command::Zoom,
        runtime::Priority,
        testing::testkit::{helpers, DeterministicExecutor, ScriptedTransport, Step},
        Executor,
    };

    const TELE_STD: &[u8] = &[0x81, 0x01, 0x04, 0x07, 0x02, 0xFF];
    const WIDE_STD: &[u8] = &[0x81, 0x01, 0x04, 0x07, 0x03, 0xFF];
    const HOME: &[u8] = &[0x81, 0x01, 0x06, 0x04, 0xFF];
    const RESET: &[u8] = &[0x81, 0x01, 0x06, 0x05, 0xFF];
    const ZOOM_STOP: &[u8] = &[0x81, 0x01, 0x04, 0x07, 0x00, 0xFF];

    /// Submit two commands that occupy both VISCA sockets, queue two more
    /// behind them, then submit a final command at `last_priority`. Returns the
    /// bytes the transport observed, in send order.
    fn run_queue_scenario(last_priority: Priority) -> Vec<Vec<u8>> {
        use grafton_visca::command::PanTilt;

        let (executor, _clock) = DeterministicExecutor::new();

        // Only the two socket-filling commands are ACKed. Everything else stays
        // queued until the completion injected below frees a socket.
        let transport: ScriptedTransport<DeterministicExecutor> = ScriptedTransport::new(vec![
            Step::OnSend {
                matches: Some(TELE_STD.to_vec()),
                responses: vec![helpers::ack(1)],
            },
            Step::OnSend {
                matches: Some(WIDE_STD.to_vec()),
                responses: vec![helpers::ack(2)],
            },
        ])
        .with_executor(executor.clone());
        let observed = transport.clone();

        let exec = executor.clone();
        let injector = transport.clone();
        let watched = transport.clone();
        executor.block_on_bg(async move {
            let observed = watched;
            let mut camera = CameraBuilder::<DeterministicExecutor>::with_executor(exec.clone())
                .open_async::<GenericVisca, _>(transport)
                .await
                .expect("camera");

            assert_eq!(
                camera.command_priority(),
                Priority::Normal,
                "a freshly opened camera must submit at Normal"
            );

            // Response futures are kept alive for the whole scenario so no
            // submission is torn down early.
            let mut pending = Vec::new();

            // Fill both command sockets. Admission is awaited, so ordering is
            // deterministic; the sleeps keep the FIFO tie-break well defined.
            pending.push(
                camera
                    .start_command_with_id(&Zoom::TeleStd)
                    .await
                    .expect("submit socket filler 1"),
            );
            pending.push(
                camera
                    .start_command_with_id(&Zoom::WideStd)
                    .await
                    .expect("submit socket filler 2"),
            );
            wait_for_sends(&exec, &observed, 2).await;

            // Queued behind the full sockets, at the default priority.
            pending.push(
                camera
                    .start_command_with_id(&PanTilt::Home)
                    .await
                    .expect("queue normal 1"),
            );
            exec.sleep(Duration::from_millis(1)).await;
            pending.push(
                camera
                    .start_command_with_id(&PanTilt::Reset)
                    .await
                    .expect("queue normal 2"),
            );
            exec.sleep(Duration::from_millis(1)).await;

            // The command under test: submitted last, so FIFO alone would send
            // it last.
            camera.set_command_priority(last_priority);
            assert_eq!(camera.command_priority(), last_priority);
            pending.push(
                camera
                    .start_command_with_id(&Zoom::Stop)
                    .await
                    .expect("queue command under test"),
            );

            // Free exactly one socket; the scheduler now picks one queued command.
            injector.add_response(helpers::complete(1));
            wait_for_sends(&exec, &observed, 3).await;
            drop(pending);
        });

        observed.sent()
    }

    async fn wait_for_sends(
        executor: &std::sync::Arc<DeterministicExecutor>,
        transport: &ScriptedTransport<DeterministicExecutor>,
        expected: usize,
    ) {
        for _ in 0..200 {
            if transport.sent().len() >= expected {
                return;
            }
            executor.sleep(Duration::from_millis(1)).await;
        }
        panic!(
            "transport only observed {} sends, expected {expected}",
            transport.sent().len()
        );
    }

    /// Control: with every command at the default priority, the queue is FIFO,
    /// so the first command queued is the one dispatched when a socket frees.
    #[test]
    fn normal_priority_keeps_fifo_order() {
        let sent = run_queue_scenario(Priority::Normal);

        assert_eq!(&sent[0], TELE_STD);
        assert_eq!(&sent[1], WIDE_STD);
        assert_eq!(
            &sent[2], HOME,
            "at equal priority the oldest queued command must win, got {:02X?}",
            sent[2]
        );
    }

    /// Treatment: the same sequence, with only the last command raised to
    /// Critical, dispatches that command ahead of the older queued work.
    #[test]
    fn critical_priority_preempts_queued_normal_commands() {
        let sent = run_queue_scenario(Priority::Critical);

        assert_eq!(&sent[0], TELE_STD);
        assert_eq!(&sent[1], WIDE_STD);
        assert_eq!(
            &sent[2], ZOOM_STOP,
            "the Critical command must overtake the queued Normal ones, got {:02X?}",
            sent[2]
        );
        assert_ne!(&sent[2], HOME);
        assert_ne!(&sent[2], RESET);
    }

    /// The per-command override reaches the runtime and leaves the handle
    /// default alone, which is what makes it usable on an `Arc`-shared camera.
    #[test]
    fn execute_with_priority_leaves_the_handle_default_alone() {
        let (executor, _clock) = DeterministicExecutor::new();
        let transport: ScriptedTransport<DeterministicExecutor> =
            ScriptedTransport::new(vec![Step::OnSend {
                matches: Some(ZOOM_STOP.to_vec()),
                responses: vec![helpers::ack(1), helpers::complete(1)],
            }])
            .with_executor(executor.clone());

        let exec = executor.clone();
        executor.block_on_bg(async move {
            let camera = CameraBuilder::<DeterministicExecutor>::with_executor(exec)
                .open_async::<GenericVisca, _>(transport)
                .await
                .expect("camera");

            camera
                .execute_with_priority(Zoom::Stop, Priority::Critical)
                .await
                .expect("critical stop completes");

            assert_eq!(
                camera.command_priority(),
                Priority::Normal,
                "a per-command override must not change the handle default"
            );
        });
    }

    /// The priority belongs to the handle: another handle keeps its own value.
    #[test]
    fn priority_is_per_handle() {
        let (executor, _clock) = DeterministicExecutor::new();
        let raised_transport: ScriptedTransport<DeterministicExecutor> =
            ScriptedTransport::new(vec![]).with_executor(executor.clone());
        let untouched_transport: ScriptedTransport<DeterministicExecutor> =
            ScriptedTransport::new(vec![]).with_executor(executor.clone());

        let exec = executor.clone();
        executor.block_on_bg(async move {
            let mut raised = CameraBuilder::<DeterministicExecutor>::with_executor(exec.clone())
                .open_async::<GenericVisca, _>(raised_transport)
                .await
                .expect("camera");
            let untouched = CameraBuilder::<DeterministicExecutor>::with_executor(exec)
                .open_async::<GenericVisca, _>(untouched_transport)
                .await
                .expect("camera");

            raised.set_command_priority(Priority::Critical);

            assert_eq!(raised.command_priority(), Priority::Critical);
            assert_eq!(untouched.command_priority(), Priority::Normal);
        });
    }
}
