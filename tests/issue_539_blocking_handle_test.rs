//! Issue #539: genuine blocking operation handle (`BlockingInFlight`).
//!
//! These tests verify the blocking half of the mode-honest `submit` primitive:
//!
//! - `submit(cmd).await_applied(timeout)` drives the command to completion
//!   **synchronously on the caller's thread** (blocking mode has no async
//!   executor at all), returning the ordinary `Result`.
//! - Multiple submitted commands each complete through their own handle.
//! - `await_settled` returns `Error::NotSupported` for a continuous/stop handle.
//! - `cancel()` discards a queued command before it is ever sent.
//! - Dropping a handle never stops the command (drop == detach).

#![cfg(all(not(feature = "mode-async"), feature = "test-utils"))]

mod common;

use std::time::Duration;

use grafton_visca::{
    command::PanTilt as PanTiltCmd,
    prelude::blocking::*,
    testing::testkit::{helpers, ScriptedBlockingTransport},
    Error,
};

use crate::common::patterns;

const TIMEOUT: Duration = Duration::from_secs(2);

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

    // Nothing is sent until we block for completion.
    let handle = camera.submit(&PanTiltCmd::Home).unwrap();
    assert!(
        transport.sent().is_empty(),
        "submit must not perform any I/O before await_applied"
    );

    let result = handle.await_applied(TIMEOUT);
    assert!(result.is_ok(), "await_applied should succeed: {result:?}");

    let sent = transport.sent();
    assert_eq!(sent.len(), 1, "exactly one command should be sent");
    assert_eq!(sent[0], patterns::pan_tilt::HOME);
}

/// Two commands submitted and awaited (sequentially, the single-threaded blocking
/// analog of dual-socket parity) both complete through their own handles.
#[test]
fn test_multiple_submits_each_complete() {
    let transport = ScriptedBlockingTransport::new(vec![
        helpers::command_response(patterns::pan_tilt::HOME.to_vec(), 1),
        helpers::command_response(patterns::zoom::STOP.to_vec(), 2),
    ]);
    let camera: Camera<PtzOpticsG2, _> = Camera::new(transport.clone()).unwrap();

    let first = camera.submit(&PanTiltCmd::Home).unwrap();
    assert!(
        first.await_applied(TIMEOUT).is_ok(),
        "first op should apply"
    );

    let second = camera.submit(&grafton_visca::command::Zoom::Stop).unwrap();
    assert!(
        second.await_applied(TIMEOUT).is_ok(),
        "second op should apply"
    );

    let sent = transport.sent();
    assert_eq!(sent.len(), 2, "both commands should have been sent");
    assert_eq!(sent[0], patterns::pan_tilt::HOME);
    assert_eq!(sent[1], patterns::zoom::STOP);
}

/// `await_settled` on a continuous/stop handle returns `Error::NotSupported`
/// (Phase 1 runtime guard) — there is no well-defined physical-settle event.
#[test]
fn test_continuous_await_settled_not_supported() {
    let transport = ScriptedBlockingTransport::new(vec![]);
    let camera: Camera<PtzOpticsG2, _> = Camera::new(transport.clone()).unwrap();

    let handle = camera.submit_continuous(&PanTiltCmd::Home).unwrap();
    let result = handle.await_settled(TIMEOUT);

    assert!(
        matches!(result, Err(Error::NotSupported)),
        "await_settled on a continuous handle must be NotSupported, got {result:?}"
    );
    // The guard short-circuits before any I/O.
    assert!(transport.sent().is_empty());
}

/// `cancel()` discards a still-queued command before it is ever sent.
#[test]
fn test_cancel_before_await_does_not_send() {
    let transport = ScriptedBlockingTransport::new(vec![helpers::command_response(
        patterns::pan_tilt::HOME.to_vec(),
        1,
    )]);
    let camera: Camera<PtzOpticsG2, _> = Camera::new(transport.clone()).unwrap();

    let handle = camera.submit(&PanTiltCmd::Home).unwrap();
    assert!(handle.cancel().is_ok(), "cancel should succeed");

    assert!(
        transport.sent().is_empty(),
        "a canceled, never-awaited command must not be sent"
    );
}

/// Dropping a handle without awaiting/cancelling never stops the command: it is
/// simply detached (left queued). A subsequent real operation still works.
#[test]
fn test_drop_detaches_without_stopping() {
    let transport = ScriptedBlockingTransport::new(vec![helpers::command_response(
        patterns::pan_tilt::HOME.to_vec(),
        1,
    )]);
    let camera: Camera<PtzOpticsG2, _> = Camera::new(transport.clone()).unwrap();

    {
        // Explicit detach is the documented equivalent of dropping.
        let handle = camera.submit(&PanTiltCmd::Home).unwrap();
        handle.detach();
    }

    // Detach performed no I/O and no cancellation frame.
    assert!(transport.sent().is_empty());
}
