//! Issue #597: the async `Camera` is documented as `Clone`, so it must be
//! `Clone` — and cloning must mean what the docs say it means.
//!
//! The type-level docs, the connection-pooling example in the `runtime` module,
//! and the per-handle caveats added by #584 (timeout view) and #589 (command
//! priority) all describe one contract:
//!
//! * the connection is **shared** — both handles drive one runtime, one
//!   transport, one command queue;
//! * teardown happens on the **last** handle, so dropping a clone leaves the
//!   original working;
//! * the per-handle view (timeout config, command priority) is **copied at
//!   clone time and then diverges**.
//!
//! These tests pin all three, plus the emergency-stop pattern the priority docs
//! recommend for a camera shared behind an `Arc`.

#![cfg(all(feature = "mode-async", feature = "test-utils"))]

use std::{sync::Arc, time::Duration};

use grafton_visca::{
    camera::{profiles::GenericVisca, CameraBuilder},
    command::{PanTilt, Zoom},
    runtime::Priority,
    testing::testkit::{helpers, DeterministicExecutor, ScriptedTransport, Step},
    timeout::TimeoutConfig,
    Executor,
};

const TELE_STD: &[u8] = &[0x81, 0x01, 0x04, 0x07, 0x02, 0xFF];
const WIDE_STD: &[u8] = &[0x81, 0x01, 0x04, 0x07, 0x03, 0xFF];
const HOME: &[u8] = &[0x81, 0x01, 0x06, 0x04, 0xFF];
const ZOOM_STOP: &[u8] = &[0x81, 0x01, 0x04, 0x07, 0x00, 0xFF];

async fn wait_for_sends(
    executor: &Arc<DeterministicExecutor>,
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

/// Both handles talk to the same camera: a command from the original and a
/// command from the clone arrive, in order, on the one scripted transport.
#[test]
fn a_clone_submits_over_the_same_session() {
    let (executor, _clock) = DeterministicExecutor::new();
    let transport: ScriptedTransport<DeterministicExecutor> = ScriptedTransport::new(vec![
        helpers::command_response(TELE_STD.to_vec(), 1),
        helpers::command_response(WIDE_STD.to_vec(), 1),
    ])
    .with_executor(executor.clone());
    let observed = transport.clone();

    let exec = executor.clone();
    executor.block_on_bg(async move {
        let camera = CameraBuilder::<DeterministicExecutor>::with_executor(exec)
            .open_async::<GenericVisca, _>(transport)
            .await
            .expect("camera");

        // One connection was opened; this is a second handle onto it.
        let clone = camera.clone();

        camera
            .execute(Zoom::TeleStd)
            .await
            .expect("the original submits");
        clone
            .execute(Zoom::WideStd)
            .await
            .expect("the clone submits over the same session");
    });

    let sent = observed.sent();
    assert_eq!(
        sent.len(),
        2,
        "both handles must use the one transport, got {sent:02X?}"
    );
    assert_eq!(&sent[0], TELE_STD);
    assert_eq!(&sent[1], WIDE_STD);
}

/// Teardown is refcounted: dropping a clone must not shut the runtime down
/// under the original.
#[test]
fn dropping_a_clone_leaves_the_original_session_alive() {
    let (executor, _clock) = DeterministicExecutor::new();
    let transport: ScriptedTransport<DeterministicExecutor> = ScriptedTransport::new(vec![
        helpers::command_response(TELE_STD.to_vec(), 1),
        helpers::command_response(WIDE_STD.to_vec(), 1),
    ])
    .with_executor(executor.clone());
    let observed = transport.clone();

    let exec = executor.clone();
    executor.block_on_bg(async move {
        let camera = CameraBuilder::<DeterministicExecutor>::with_executor(exec.clone())
            .open_async::<GenericVisca, _>(transport)
            .await
            .expect("camera");

        let clone = camera.clone();
        clone
            .execute(Zoom::TeleStd)
            .await
            .expect("the clone submits");
        drop(clone);

        // Give a stray shutdown, if one were sent, time to reach the runtime.
        exec.sleep(Duration::from_millis(50)).await;

        camera
            .execute(Zoom::WideStd)
            .await
            .expect("the original must still submit after its clone is dropped");
    });

    let sent = observed.sent();
    assert_eq!(&sent[0], TELE_STD);
    assert_eq!(
        sent.get(1).map(Vec::as_slice),
        Some(WIDE_STD),
        "the session must survive the clone's drop, got {sent:02X?}"
    );
}

/// The write-only state cache is shared, not forked: a cache write made through
/// one handle is visible through the other, because both describe the state of
/// the one physical camera.
#[test]
fn the_state_cache_is_shared_between_handles() {
    use grafton_visca::ExposureControl;

    const AUTO_SLOW_SHUTTER_ON: &[u8] = &[0x81, 0x01, 0x04, 0x5A, 0x02, 0xFF];

    let (executor, _clock) = DeterministicExecutor::new();
    let transport: ScriptedTransport<DeterministicExecutor> =
        ScriptedTransport::new(vec![helpers::command_response(
            AUTO_SLOW_SHUTTER_ON.to_vec(),
            1,
        )])
        .with_executor(executor.clone());

    let exec = executor.clone();
    executor.block_on_bg(async move {
        let camera = CameraBuilder::<DeterministicExecutor>::with_executor(exec)
            .open_async::<GenericVisca, _>(transport)
            .await
            .expect("camera");
        let clone = camera.clone();

        assert_eq!(
            clone.state_cache().auto_slow_shutter(),
            None,
            "nothing has been written yet"
        );

        camera
            .enable_auto_slow_shutter()
            .await
            .expect("the original writes through to the cache");

        assert_eq!(
            clone.state_cache().auto_slow_shutter(),
            Some(true),
            "a clone must observe cache writes made through the original"
        );
    });
}

/// Per-handle state is copied at clone time and independent afterwards: raising
/// the clone's priority or re-pointing its timeout view leaves the original's
/// getters alone.
#[test]
fn per_handle_state_is_copied_then_diverges() {
    let (executor, _clock) = DeterministicExecutor::new();
    let transport: ScriptedTransport<DeterministicExecutor> =
        ScriptedTransport::new(vec![]).with_executor(executor.clone());

    let exec = executor.clone();
    executor.block_on_bg(async move {
        let mut camera = CameraBuilder::<DeterministicExecutor>::with_executor(exec)
            .open_async::<GenericVisca, _>(transport)
            .await
            .expect("camera");
        camera.set_command_priority(Priority::High);
        let original_ack = camera.timeout_config().ack_timeout;

        // Copied at clone time.
        let mut clone = camera.clone();
        assert_eq!(
            clone.command_priority(),
            Priority::High,
            "a clone starts from the original's priority"
        );
        assert_eq!(
            clone.timeout_config().ack_timeout,
            original_ack,
            "a clone starts from the original's timeout view"
        );

        // Independent afterwards.
        let raised_ack = original_ack + Duration::from_millis(137);
        clone.set_command_priority(Priority::Critical);
        clone
            .set_timeout_config(TimeoutConfig::builder().ack_timeout(raised_ack).build())
            .await
            .expect("the clone updates the shared runtime and its own view");

        assert_eq!(
            clone.command_priority(),
            Priority::Critical,
            "the clone keeps what it was set to"
        );
        assert_eq!(clone.timeout_config().ack_timeout, raised_ack);

        assert_eq!(
            camera.command_priority(),
            Priority::High,
            "the original's priority must not follow the clone's"
        );
        assert_eq!(
            camera.timeout_config().ack_timeout,
            original_ack,
            "the original's timeout view must not follow the clone's"
        );
    });
}

/// The emergency-stop pattern the #589 priority docs recommend for a camera
/// shared behind an `Arc`: clone off a private handle, raise it to `Critical`,
/// and submit. Its command overtakes work the original already queued, and the
/// original handle's own priority is untouched.
#[test]
fn a_raised_clone_preempts_work_queued_by_the_original() {
    let (executor, _clock) = DeterministicExecutor::new();

    // Only the two socket-filling commands are ACKed; everything else stays
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
    let injector = transport.clone();

    let exec = executor.clone();
    let watched = transport.clone();
    executor.block_on_bg(async move {
        let observed = watched;
        let camera = CameraBuilder::<DeterministicExecutor>::with_executor(exec.clone())
            .open_async::<GenericVisca, _>(transport)
            .await
            .expect("camera");

        // Response futures are held for the whole scenario so no submission is
        // torn down early.
        let mut pending = Vec::new();

        // Fill both VISCA sockets.
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

        // Queued behind the full sockets at the default priority.
        pending.push(
            camera
                .start_command_with_id(&PanTilt::Home)
                .await
                .expect("queue normal work"),
        );
        exec.sleep(Duration::from_millis(1)).await;

        // The emergency handle: same connection, its own priority.
        let mut emergency = camera.clone();
        emergency.set_command_priority(Priority::Critical);
        assert_eq!(
            camera.command_priority(),
            Priority::Normal,
            "raising the clone must not raise the shared handle"
        );

        pending.push(
            emergency
                .start_command_with_id(&Zoom::Stop)
                .await
                .expect("queue the emergency stop"),
        );

        // Free exactly one socket; the scheduler now picks one queued command.
        injector.add_response(helpers::complete(1));
        wait_for_sends(&exec, &observed, 3).await;
        drop(pending);
    });

    let sent = observed.sent();
    assert_eq!(&sent[0], TELE_STD);
    assert_eq!(&sent[1], WIDE_STD);
    assert_eq!(
        &sent[2], ZOOM_STOP,
        "the raised clone's command must overtake the original's queued work, got {:02X?}",
        sent[2]
    );
    assert_ne!(&sent[2], HOME);
}
