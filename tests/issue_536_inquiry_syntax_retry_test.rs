//! Issue #536: bounded contextual retry for transient inquiry-side `0x02`.
//!
//! Runtime-level (blocking) coverage. A scripted transport returns a `0x02`
//! syntax error for an inquiry and then a valid data reply on the resend; the
//! caller receives the successful inquiry result, exactly one resend occurs,
//! and the resend is delayed by the profile's inquiry pacing. A command-side
//! `0x02` is verified to stay terminal with no resend.

#![cfg(all(not(feature = "mode-async"), feature = "test-utils"))]

use std::time::{Duration, Instant};

use grafton_visca::{
    camera::profiles::PtzOpticsG2,
    command::{InquiryData, PanTiltPositionInquiry, Response},
    runtime::testing::BlockingRunner,
    testing::testkit::scripted_transport::{ScriptedBlockingTransport, Step},
    timeout::TimeoutConfig,
    transport::{BackoffStrategy, RetryConfig},
    CameraId,
};

/// Build a `PtzOpticsG2` runner with a short retry backoff so the profile's
/// 150ms inquiry pacing (not the backoff) dominates the observed resend timing.
fn fast_backoff_runner() -> BlockingRunner<PtzOpticsG2> {
    BlockingRunner::<PtzOpticsG2>::builder(TimeoutConfig::default())
        .retry_config(RetryConfig {
            max_retries: 3,
            base_retry_delay: Duration::from_millis(5),
            max_retry_duration: Duration::from_secs(30),
            backoff_strategy: BackoffStrategy::Constant,
        })
        .build()
}

/// Encode a pan/tilt DataReply frame: `90 50 pppp tttt FF`.
fn pan_tilt_reply(pan_u16: u16, tilt_u16: u16) -> Vec<u8> {
    let mut frame = vec![0x90, 0x50];
    for shift in [12u16, 8, 4, 0] {
        frame.push(((pan_u16 >> shift) & 0x0F) as u8);
    }
    for shift in [12u16, 8, 4, 0] {
        frame.push(((tilt_u16 >> shift) & 0x0F) as u8);
    }
    frame.push(0xFF);
    frame
}

#[test]
fn blocking_inquiry_syntax_error_retries_then_succeeds() {
    let mut runner = fast_backoff_runner();

    // First send of the inquiry gets a transient 0x02; the resend gets the data.
    let mut transport = ScriptedBlockingTransport::new(vec![
        Step::OnSend {
            matches: None,
            responses: vec![vec![0x90, 0x60, 0x02, 0xFF]], // Syntax error, no socket
        },
        Step::OnSend {
            matches: None,
            responses: vec![pan_tilt_reply(0x1234, 0x5678)],
        },
    ]);

    let started = Instant::now();
    let result = runner.send_command(&mut transport, &PanTiltPositionInquiry, CameraId::CAMERA_1);
    let elapsed = started.elapsed();

    match result {
        Ok(Response::Inquiry(InquiryData::PanTiltPosition { pan, tilt })) => {
            assert_eq!(pan, 0x1234_i16);
            assert_eq!(tilt, 0x5678_i16);
        }
        other => panic!("expected PanTiltPosition after a transient 0x02 retry, got {other:?}"),
    }

    assert_eq!(
        transport.sent().len(),
        2,
        "exactly one resend should occur after the transient 0x02"
    );
    assert!(
        elapsed >= Duration::from_millis(140),
        "resend must honor the profile inquiry pacing (elapsed = {elapsed:?})"
    );
}

#[test]
fn blocking_command_syntax_error_is_terminal_without_retry() {
    let mut runner = fast_backoff_runner();

    // A command that gets 0x02 must fail immediately with no resend.
    let mut transport = ScriptedBlockingTransport::new(vec![Step::OnSend {
        matches: None,
        responses: vec![vec![0x90, 0x60, 0x02, 0xFF]],
    }]);

    // PowerOn is a plain command (not an inquiry).
    let result = runner.send_command(
        &mut transport,
        &grafton_visca::command::PowerOn::new(),
        CameraId::CAMERA_1,
    );

    assert!(
        matches!(result, Err(grafton_visca::Error::SyntaxError)),
        "command-side 0x02 must fail terminally with SyntaxError, got {result:?}"
    );
    assert_eq!(
        transport.sent().len(),
        1,
        "command-side 0x02 must not be retried"
    );
}
