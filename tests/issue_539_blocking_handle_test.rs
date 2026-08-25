//! Issue #539: genuine blocking operation handle (`BlockingInFlight`).
//!
//! These tests verify the blocking half of the mode-honest `submit` primitive:
//!
//! - `submit(cmd).await_applied(timeout)` drives the command to completion
//!   **synchronously on the caller's thread** (blocking mode has no async
//!   executor at all), returning the ordinary `Result`.
//! - Multiple submitted commands each complete through their own handle.
//! - `await_settled` returns `Error::NotSupported` for a continuous/stop handle.
//! - Profiles that support cancellation send the correct socket cancel before/after ACK.
//! - Dropping or detaching a handle never stops the already-dispatched command.

#![cfg(all(not(feature = "mode-async"), feature = "test-utils"))]

mod common;

use std::time::Duration;

use grafton_visca::{
    camera::profiles::GenericVisca,
    command::{PanTilt as PanTiltCmd, ZoomPositionInquiry},
    prelude::blocking::*,
    testing::testkit::{helpers, ScriptedBlockingTransport, Step},
    Error,
};

use crate::common::patterns;

const TIMEOUT: Duration = Duration::from_secs(2);
const CANCEL_SOCKET_1: &[u8] = &[0x81, 0x21, 0xFF];
const CANCEL_SOCKET_2: &[u8] = &[0x81, 0x22, 0xFF];
const PAN_TILT_POSITION_INQUIRY: &[u8] = &[0x81, 0x09, 0x06, 0x12, 0xFF];

fn pan_tilt_position(pan: u16, tilt: u16) -> Vec<u8> {
    let mut response = vec![0x90, 0x50];
    for value in [pan, tilt] {
        response.extend([
            ((value >> 12) & 0x0f) as u8,
            ((value >> 8) & 0x0f) as u8,
            ((value >> 4) & 0x0f) as u8,
            (value & 0x0f) as u8,
        ]);
    }
    response.push(0xff);
    response
}

fn polling_settle_steps() -> Vec<Step> {
    vec![
        helpers::command_response(patterns::pan_tilt::HOME.to_vec(), 1),
        helpers::inquiry_response(
            PAN_TILT_POSITION_INQUIRY.to_vec(),
            1,
            pan_tilt_position(0, 0),
        ),
        helpers::inquiry_response(
            PAN_TILT_POSITION_INQUIRY.to_vec(),
            1,
            pan_tilt_position(0, 0),
        ),
    ]
}

/// `submit().await_applied()` runs entirely on the caller's thread and returns
/// `Ok(())` once the camera has protocol-completed the command. No async runtime
/// is created anywhere in this test — blocking mode is structurally synchronous.
#[test]
fn test_submit_await_applied_is_synchronous() {
    let transport = ScriptedBlockingTransport::new(vec![helpers::command_response(
        patterns::pan_tilt::HOME.to_vec(),
        1,
    )]);
    let camera: Camera<PtzOpticsG2, _> = Camera::new(transport.clone()).unwrap();

    // Submission performs initial dispatch, but does not receive-pump.
    let handle = camera.submit(&PanTiltCmd::Home).unwrap();
    assert_eq!(transport.sent(), vec![patterns::pan_tilt::HOME.to_vec()]);

    let result = handle.await_applied(TIMEOUT);
    assert!(result.is_ok(), "await_applied should succeed: {result:?}");

    let sent = transport.sent();
    assert_eq!(sent.len(), 1, "exactly one command should be sent");
    assert_eq!(sent[0], patterns::pan_tilt::HOME);
}

/// Profiles with an exact operation-complete signal do not issue any fallback
/// position inquiries when awaiting physical settle.
#[test]
fn test_settled_uses_exact_completion_when_profile_supports_it() {
    let transport = ScriptedBlockingTransport::new(vec![helpers::command_response(
        patterns::pan_tilt::HOME.to_vec(),
        1,
    )]);
    let camera: Camera<PtzOpticsG2, _> = Camera::new(transport.clone()).unwrap();

    camera
        .submit(&PanTiltCmd::Home)
        .unwrap()
        .await_settled(TIMEOUT)
        .expect("operation-complete should be the settled signal");

    assert_eq!(transport.sent(), vec![patterns::pan_tilt::HOME.to_vec()]);
}

/// Profiles without operation-complete poll only the axes declared by the
/// command metadata. A pan/tilt home must not trigger zoom or focus inquiries.
#[test]
fn test_settled_polls_only_affected_axes_without_operation_complete() {
    let transport = ScriptedBlockingTransport::new(polling_settle_steps());
    let camera: Camera<GenericVisca, _> = Camera::new(transport.clone()).unwrap();

    camera
        .submit(&PanTiltCmd::Home)
        .unwrap()
        .await_settled(TIMEOUT)
        .expect("equal pan/tilt samples should be settled");

    assert_eq!(
        transport.sent(),
        vec![
            patterns::pan_tilt::HOME.to_vec(),
            PAN_TILT_POSITION_INQUIRY.to_vec(),
            PAN_TILT_POSITION_INQUIRY.to_vec(),
        ]
    );
}

/// Command completion and position polling consume one caller-provided
/// deadline. A stalled fallback inquiry must not start a fresh timeout budget.
#[test]
fn test_settled_deadline_bounds_a_stalled_position_inquiry() {
    let transport = ScriptedBlockingTransport::new(vec![helpers::command_response(
        patterns::pan_tilt::HOME.to_vec(),
        1,
    )]);
    let camera: Camera<GenericVisca, _> = Camera::new(transport).unwrap();
    let started = std::time::Instant::now();

    let result = camera
        .submit(&PanTiltCmd::Home)
        .unwrap()
        .await_settled(Duration::from_millis(40));

    assert!(matches!(result, Err(Error::Timeout)));
    assert!(
        started.elapsed() < Duration::from_millis(500),
        "the position inquiry must share the handle deadline"
    );
}

/// Two commands are submitted before either is awaited. The second command
/// completes first on another VISCA socket, and both exact outcomes remain
/// available through their respective handles.
#[test]
fn test_multiple_live_submits_preserve_out_of_order_completions() {
    let transport = ScriptedBlockingTransport::new(vec![
        Step::OnSend {
            matches: Some(patterns::pan_tilt::HOME.to_vec()),
            responses: vec![helpers::ack(1)],
        },
        Step::OnSend {
            matches: Some(patterns::zoom::STOP.to_vec()),
            responses: vec![helpers::ack(2), helpers::complete(2), helpers::complete(1)],
        },
    ]);
    let camera: Camera<PtzOpticsG2, _> = Camera::new(transport.clone()).unwrap();

    let first = camera.submit(&PanTiltCmd::Home).unwrap();
    let second = camera.submit(&grafton_visca::command::Zoom::Stop).unwrap();

    assert!(
        first.await_applied(TIMEOUT).is_ok(),
        "first op should apply"
    );
    assert!(
        second.await_applied(TIMEOUT).is_ok(),
        "the already-completed second op should retain its exact outcome"
    );

    let sent = transport.sent();
    assert_eq!(sent.len(), 2, "both commands should have been sent");
    assert_eq!(sent[0], patterns::pan_tilt::HOME);
    assert_eq!(sent[1], patterns::zoom::STOP);
}

/// A non-target command error is retained just like a successful completion.
/// Awaiting another handle must not discard the exact error for this handle.
#[test]
fn test_multiple_live_submits_preserve_out_of_order_error() {
    let transport = ScriptedBlockingTransport::new(vec![
        Step::OnSend {
            matches: Some(patterns::pan_tilt::HOME.to_vec()),
            responses: vec![helpers::ack(1)],
        },
        Step::OnSend {
            matches: Some(patterns::zoom::STOP.to_vec()),
            responses: vec![
                helpers::ack(2),
                vec![0x90, 0x62, 0x02, 0xFF],
                helpers::complete(1),
            ],
        },
    ]);
    let camera: Camera<PtzOpticsG2, _> = Camera::new(transport).unwrap();

    let first = camera.submit(&PanTiltCmd::Home).unwrap();
    let second = camera.submit(&grafton_visca::command::Zoom::Stop).unwrap();

    first
        .await_applied(TIMEOUT)
        .expect("first command should still complete");
    let result = second.await_applied(TIMEOUT);
    assert!(
        matches!(result, Err(Error::SyntaxError)),
        "second command should retain its exact error, got {result:?}"
    );
}

/// `await_settled` on a continuous/stop handle returns `Error::NotSupported`
/// because there is no well-defined physical-settle event.
#[test]
fn test_continuous_await_settled_not_supported() {
    let transport = ScriptedBlockingTransport::new(vec![Step::OnSend {
        matches: Some(patterns::pan_tilt::HOME.to_vec()),
        responses: vec![],
    }]);
    let camera: Camera<PtzOpticsG2, _> = Camera::new(transport.clone()).unwrap();

    let handle = camera.submit_continuous(&PanTiltCmd::Home).unwrap();
    let result = handle.await_settled(TIMEOUT);

    assert!(
        matches!(result, Err(Error::NotSupported)),
        "await_settled on a continuous handle must be NotSupported, got {result:?}"
    );
    // The guard short-circuits before receive-pumping, after initial dispatch.
    assert_eq!(transport.sent(), vec![patterns::pan_tilt::HOME.to_vec()]);
}

/// Blocking operation submission rejects inquiries before any transport I/O,
/// matching the async cancelable-command path.
#[test]
fn test_submit_rejects_inquiry_before_io() {
    let transport = ScriptedBlockingTransport::new(vec![]);
    let camera: Camera<PtzOpticsG2, _> = Camera::new(transport.clone()).unwrap();

    let result = camera.submit(&ZoomPositionInquiry);
    assert!(matches!(result, Err(Error::InquiryNotCancelable)));
    assert!(transport.sent().is_empty());
}

/// Cancel before ACK is recorded by command ID, then emitted for the socket as
/// soon as a later receive pump processes that ACK.
#[test]
fn test_cancel_before_ack_is_sent_on_ack() {
    let transport = ScriptedBlockingTransport::new(vec![
        Step::OnSend {
            matches: Some(patterns::pan_tilt::HOME.to_vec()),
            responses: vec![],
        },
        Step::OnSend {
            matches: Some(patterns::zoom::STOP.to_vec()),
            responses: vec![helpers::ack(2), helpers::complete(2)],
        },
        Step::OnSend {
            matches: Some(CANCEL_SOCKET_1.to_vec()),
            responses: vec![],
        },
    ]);
    let camera: Camera<GenericVisca, _> = Camera::new(transport.clone()).unwrap();

    let first = camera.submit(&PanTiltCmd::Home).unwrap();
    first.cancel().expect("pre-ACK cancel request");
    assert_eq!(transport.sent(), vec![patterns::pan_tilt::HOME.to_vec()]);

    // The ACK arrives after cancel was requested. Awaiting another live handle
    // drives the receive loop and emits the deferred socket cancel.
    transport.add_response(helpers::ack(1));
    let second = camera.submit(&grafton_visca::command::Zoom::Stop).unwrap();
    second.await_applied(TIMEOUT).expect("second command");

    assert_eq!(
        transport.sent(),
        vec![
            patterns::pan_tilt::HOME.to_vec(),
            patterns::zoom::STOP.to_vec(),
            CANCEL_SOCKET_1.to_vec(),
        ]
    );
}

/// Once an ACK has assigned a socket, cancel sends that socket's VISCA cancel
/// immediately without requiring another receive pump.
#[test]
fn test_cancel_after_ack_sends_immediately() {
    let transport = ScriptedBlockingTransport::new(vec![
        Step::OnSend {
            matches: Some(patterns::pan_tilt::HOME.to_vec()),
            responses: vec![helpers::ack(1)],
        },
        Step::OnSend {
            matches: Some(patterns::zoom::STOP.to_vec()),
            responses: vec![helpers::ack(2), helpers::complete(1)],
        },
        Step::OnSend {
            matches: Some(CANCEL_SOCKET_2.to_vec()),
            responses: vec![],
        },
    ]);
    let camera: Camera<GenericVisca, _> = Camera::new(transport.clone()).unwrap();

    let first = camera.submit(&PanTiltCmd::Home).unwrap();
    let second = camera.submit(&grafton_visca::command::Zoom::Stop).unwrap();
    first.await_applied(TIMEOUT).expect("first command");

    second.cancel().expect("post-ACK cancel");
    assert_eq!(transport.sent().last().unwrap(), CANCEL_SOCKET_2);
}

/// G2 cameras reject the VISCA socket-cancel command, so cancelling an
/// already-sent operation reports that limitation without emitting the frame.
#[test]
fn test_ptzoptics_g2_sent_cancel_is_not_supported() {
    let transport = ScriptedBlockingTransport::new(vec![Step::OnSend {
        matches: Some(patterns::pan_tilt::HOME.to_vec()),
        responses: vec![],
    }]);
    let camera: Camera<PtzOpticsG2, _> = Camera::new(transport.clone()).unwrap();

    let result = camera.submit(&PanTiltCmd::Home).unwrap().cancel();

    assert!(matches!(result, Err(Error::NotSupported)));
    assert_eq!(transport.sent(), vec![patterns::pan_tilt::HOME.to_vec()]);
}

/// Dropping a handle without awaiting/cancelling never stops the command: it is
/// detached after initial dispatch, and no cancellation frame is emitted.
#[test]
fn test_detach_does_not_stop_dispatched_command() {
    let transport = ScriptedBlockingTransport::new(vec![Step::OnSend {
        matches: Some(patterns::pan_tilt::HOME.to_vec()),
        responses: vec![],
    }]);
    let camera: Camera<PtzOpticsG2, _> = Camera::new(transport.clone()).unwrap();

    camera.submit(&PanTiltCmd::Home).unwrap().detach();

    assert_eq!(transport.sent(), vec![patterns::pan_tilt::HOME.to_vec()]);
}

#[test]
fn test_drop_does_not_stop_dispatched_command() {
    let transport = ScriptedBlockingTransport::new(vec![Step::OnSend {
        matches: Some(patterns::pan_tilt::HOME.to_vec()),
        responses: vec![],
    }]);
    let camera: Camera<PtzOpticsG2, _> = Camera::new(transport.clone()).unwrap();

    let handle = camera.submit(&PanTiltCmd::Home).unwrap();
    drop(handle);

    assert_eq!(transport.sent(), vec![patterns::pan_tilt::HOME.to_vec()]);
}
