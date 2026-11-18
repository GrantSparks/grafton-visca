//! Cancellation write time and response time use separate budgets on both facades.

#![cfg(all(feature = "test-utils", any(feature = "blocking", feature = "async")))]

use std::{io, time::Duration};

use grafton_visca::{
    config::{OperationalTuning, SessionConfig},
    error::{Certainty, FailureContext, FailureStage},
    operation::CancellationOutcome,
    profiles::GenericVisca,
    testing::testkit::ManualClock,
    transport::SendSemantics,
    types::IndividualCameraId,
    Error,
};
use grafton_visca_test_support::fake_camera::{frames, FakeCamera, ZOOM_STOP};

const CANCEL_SOCKET_ONE: &[u8] = &[0x81, 0x21, 0xff];
const RESPONSE_WINDOW: Duration = Duration::from_secs(1);
const SLOW_WRITE: Duration = Duration::from_millis(1_001);
const CONFIRM_DELAY: Duration = Duration::from_millis(5);

#[derive(Clone, Copy, Debug)]
enum CameraResponse {
    Confirmed,
    Silent,
    BrokenPipe,
}

fn camera(clock: &ManualClock, write_delay: Duration, response: CameraResponse) -> FakeCamera {
    let clock = clock.clone();
    FakeCamera::new(move |write, answer| {
        if write == CANCEL_SOCKET_ONE {
            // The fake transport records the cancel and returns only after
            // this elapsed write time. The default I/O write budget is 5 s.
            clock.advance(write_delay);
            if matches!(response, CameraResponse::BrokenPipe) {
                answer.fail_send(io::Error::from(io::ErrorKind::BrokenPipe).into());
            }
        }
    })
}

fn config() -> SessionConfig {
    SessionConfig::from_compile_time::<GenericVisca>()
        .expect("generic profile")
        .with_tuning(OperationalTuning::new().with_ack_timeout(Duration::from_secs(2)))
}

macro_rules! check_response_window {
    (await: [$($await:tt)*], $clock:ident, $fake:ident, $session:ident,
        $ack_first:ident, $response:ident, $wait_for_command:expr, $wait_for_ack:expr,
        $send_confirmation:expr, $write_delay:ident) => {{
        let camera = $session
            .camera::<GenericVisca>(IndividualCameraId::CAMERA_1)
            .expect("camera view");
        let mut operation = camera.zoom().stop() $($await)* .expect("original command admitted");
        $wait_for_command;
        if !$ack_first {
            assert!(matches!(
                operation.cancel_with_timeout(Duration::from_millis(50)) $($await)*,
                Err(Error::ObservationTimeout { .. })
            ));
            // Verify the owner's intent is installed before permitting the ACK.
            assert_eq!($session.metrics() $($await)* .expect("metrics").cancellations, 1);
            assert_eq!($fake.writes(), vec![ZOOM_STOP.to_vec()]);
        }

        if matches!($response, CameraResponse::Confirmed) {
            ($send_confirmation)($fake.clone());
        }
        let before_ack = $clock.now();
        $fake.push(frames::ack(1));
        if $ack_first {
            $wait_for_ack;
        }

        let cancellation = operation.cancel() $($await)*;
        match $response {
            CameraResponse::Confirmed => {
                assert_eq!(
                    cancellation.expect("confirmation within the post-write window"),
                    CancellationOutcome::Cancelled
                );
                assert!(matches!(operation.applied() $($await)*, Err(Error::CommandCancelled)));
            }
            CameraResponse::Silent => {
                let error = cancellation.expect_err("camera did not acknowledge the cancellation");
                assert!(matches!(error, Error::Timeout { .. }), "{error:?}");
                assert_eq!(error.failure_context(), Some(FailureContext::new(
                    FailureStage::CancellationAttempt, Certainty::StillLive,
                )));
                assert!($clock.now() >= before_ack + $write_delay + RESPONSE_WINDOW);
                assert_eq!($session.metrics() $($await)* .expect("metrics").active, 1);
                // The expired cancellation observation leaves the original
                // socket correlation and independent terminal result live.
                $fake.push(frames::canceled(1));
                assert!(matches!(operation.applied() $($await)*, Err(Error::CommandCancelled)));
            }
            CameraResponse::BrokenPipe => {
                assert!(matches!(cancellation, Err(Error::StreamPoisoned { .. })));
                assert!(matches!(operation.applied() $($await)*, Err(Error::StreamPoisoned { .. })));
            }
        }
        assert_eq!($fake.writes(), vec![ZOOM_STOP.to_vec(), CANCEL_SOCKET_ONE.to_vec()]);
        if matches!($response, CameraResponse::BrokenPipe) {
            assert!(matches!($session.close() $($await)*, Err(Error::StreamPoisoned { .. })));
        } else {
            $session.close() $($await)* .expect("session remains healthy");
        }
    }};
}

fn cases() -> impl Iterator<Item = (bool, Duration, CameraResponse)> {
    [true, false].into_iter().flat_map(|ack_first| {
        [Duration::ZERO, SLOW_WRITE]
            .into_iter()
            .flat_map(move |write_delay| {
                [
                    CameraResponse::Confirmed,
                    CameraResponse::Silent,
                    CameraResponse::BrokenPipe,
                ]
                .into_iter()
                .map(move |response| (ack_first, write_delay, response))
            })
    })
}

#[cfg(feature = "blocking")]
#[test]
fn blocking() {
    for (ack_first, write_delay, response) in cases() {
        let clock = ManualClock::new();
        let participant = clock.enter();
        let fake = camera(&clock, write_delay, response);
        let session = clock
            .open_blocking_session(
                &participant,
                fake.blocking_wire()
                    .with_semantics(SendSemantics::Stream)
                    .on_clock(&clock),
                config(),
            )
            .expect("raw stream session");
        check_response_window!(await: [], clock, fake, session, ack_first, response,
            fake.wait_for_writes_on(&clock, 1), fake.wait_for_reads_on(&clock, 1),
            |peer: FakeCamera| {
                let timer = clock.clone();
                clock.spawn(move || {
                    peer.wait_for_writes_on(&timer, 2);
                    timer.sleep(CONFIRM_DELAY);
                    peer.push(frames::canceled(1));
                });
            }, write_delay);
    }
}

#[cfg(feature = "async")]
#[test]
fn async_manual_clock() {
    use grafton_visca::{
        r#async::Session, runtime::Executor, testing::testkit::ManualClockExecutor,
    };

    for (ack_first, write_delay, response) in cases() {
        let clock = ManualClock::new();
        let executor = ManualClockExecutor::local(&clock);
        executor.run_until(async {
            let fake = camera(&clock, write_delay, response);
            let session = Session::open(
                fake.async_wire().with_semantics(SendSemantics::Stream),
                &config(),
                executor.clone(),
            )
            .await
            .expect("raw stream session");
            check_response_window!(await: [.await], clock, fake, session, ack_first, response,
                fake.wait_for_writes_async(&executor, 1).await,
                fake.wait_for_reads_async(&executor, 1).await,
                |peer: FakeCamera| {
                    let timer = executor.clone();
                    executor.spawn_detached(async move {
                        peer.wait_for_writes_async(&timer, 2).await;
                        timer.sleep(CONFIRM_DELAY).await;
                        peer.push(frames::canceled(1));
                    });
                }, write_delay);
        });
    }
}
