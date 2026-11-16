//! Issue #571: the field-debugging counters and the retry-decision event as a
//! caller of the published API sees them.
//!
//! The scenarios are scripted end to end — a camera that accepts writes and
//! never answers — and the assertions are about what the owner reported. The
//! first test pins raw ACK ambiguity as a per-request terminal that fails the
//! one command and leaves the session running (issue #671); the second
//! deliberately sets the retry budget through public
//! [`OperationalTuning`] so that "this scenario retries" is the caller's own
//! request rather than a profile constant, and the counter has something it
//! must show.

#![cfg(feature = "blocking")]
#![allow(clippy::expect_used, clippy::unwrap_used)]

use grafton_visca_test_support::fake_camera;

use std::time::Duration;

use grafton_visca::{
    blocking::Session,
    config::{OperationalTuning, SessionConfig},
    observability::{DiagnosticDeadline, DiagnosticEvent},
    operation::AppliedOnly,
    profiles::{ProfileSpec, PtzOpticsG2, SonyFr7},
    transport::AddressingMode,
    Error,
};

use fake_camera::FakeCamera;

/// A camera that takes every frame and never answers one, on an IP-addressed
/// datagram wire.
fn silent_wire() -> fake_camera::BlockingWire {
    FakeCamera::silent()
        .blocking_wire()
        .with_addressing(AddressingMode::Ip)
}

fn silent_session() -> Session {
    // The profile's own ACK deadline is used as-is: operational tuning may only
    // lengthen a profile deadline, and nothing here should depend on its value.
    let config = SessionConfig::new(
        ProfileSpec::from_compile_time::<PtzOpticsG2>().expect("built-in G2 profile"),
    );
    Session::open(silent_wire(), &config).expect("session opens over a silent camera")
}

/// A silent camera under a *caller-configured* retry budget.
///
/// The base retry count is an operational override rather than a profile
/// constant, so the scenario below is one the caller asked for retries in and
/// the counter has to show them.
fn retrying_silent_session(retry_limit: u32) -> Session {
    let config = SessionConfig::new(
        ProfileSpec::from_compile_time::<SonyFr7>().expect("built-in Sony FR7 profile"),
    )
    .with_tuning(
        OperationalTuning::new()
            .with_retry_limit(retry_limit)
            .with_initial_retry_backoff(Duration::from_millis(50))
            .with_maximum_retry_backoff(Duration::from_millis(500))
            .with_retry_budget(Duration::from_secs(30)),
    );
    Session::open(silent_wire(), &config).expect("session opens over a silent camera")
}

/// The counter exists, and every expiry carries the retry decision that the
/// event stream then confirms — which is the whole point: a subscriber no
/// longer has to infer the decision from a phase change and a missing retry.
#[test]
fn an_unanswered_command_counts_ack_timeouts_and_reports_each_retry_decision() {
    let session = silent_session();
    let diagnostics = session.subscribe_diagnostics(128).expect("diagnostics");
    let camera = session
        .camera::<PtzOpticsG2>(grafton_visca::types::IndividualCameraId::CAMERA_1)
        .expect("camera facade");
    let error = camera
        .execute(&grafton_visca::command::power::On)
        .expect_err("a silent camera cannot acknowledge");
    assert!(
        matches!(error, Error::UnsequencedCommandUnconfirmed),
        "a sent raw command whose ACK is lost fails on its own, got {error:?}"
    );
    assert!(
        !error.requires_new_session(),
        "issue #671: the unconfirmed command fails per request; the session survives"
    );

    let metrics = session.metrics().expect("metrics");
    let events: Vec<_> = std::iter::from_fn(|| diagnostics.try_recv()).collect();
    assert_eq!(
        metrics.dropped_diagnostic_events, 0,
        "the scenario must fit the diagnostic subscription for the pairing below to hold"
    );

    let expiries: Vec<bool> = events
        .iter()
        .filter_map(|event| match event {
            DiagnosticEvent::DeadlineExpired {
                deadline: DiagnosticDeadline::Ack,
                will_retry,
                ..
            } => Some(*will_retry),
            _ => None,
        })
        .collect();
    assert_eq!(
        expiries,
        [false],
        "the raw ACK ambiguity must not be retried: {events:?}"
    );
    let retries = events
        .iter()
        .filter(|event| matches!(event, DiagnosticEvent::RetryScheduled { .. }))
        .count();
    assert_eq!(
        retries, 0,
        "raw ACK ambiguity must not schedule a replay: {events:?}"
    );

    assert_eq!(metrics.ack_timeouts, expiries.len() as u64);
    assert_eq!(metrics.ack_timeouts, 1);
    assert_eq!(metrics.retries_scheduled, 0);
    assert_eq!(metrics.completion_timeouts, 0);
    assert_eq!(metrics.inquiry_timeouts, 0);
    assert_eq!(metrics.busy_errors, 0);
    assert_eq!(metrics.protocol_errors, 0);
    assert_eq!(metrics.ignored_unmatched_sequenced_replies, 0);
    assert_eq!(metrics.writes, 1);
    // Issue #671: the raw ACK ambiguity fails only this command; the session is
    // still running afterwards rather than poisoned.
    assert_eq!(
        metrics.session,
        grafton_visca::observability::SessionStatus::Running
    );
    // A concluded owner retains its counters as a stable read-only snapshot.
    session.clone().close().expect("the live session closes");
    let terminal = session.metrics().expect("terminal metrics");
    assert_eq!(
        terminal.session,
        grafton_visca::observability::SessionStatus::Shutdown
    );
    assert_eq!(terminal.active, 0);
    assert_eq!(terminal.pending, 0);
    assert_eq!(terminal.writes, metrics.writes);
    assert_eq!(terminal.ack_timeouts, metrics.ack_timeouts);
    for _ in 0..8 {
        assert_eq!(session.metrics().expect("retained metrics"), terminal);
    }
}

/// The pairing above holds even with retries removed engine-wide — every count
/// on both sides would simply be zero — so this is the leg that pins the
/// counter to a caller-configured scenario that safely retries. A movement
/// request uses the base retry count directly; quick commands intentionally
/// add their documented two extra retries.
///
/// The retry budget here is the caller's own [`OperationalTuning`] override
/// rather than a profile constant, so "this scenario retries" is a property of
/// the request as submitted and not of whichever policy the shipped profile
/// happens to carry. A silent camera exhausts that budget, so the counter must
/// report exactly one scheduled retry per attempt beyond the first, and the
/// last expiry is the one that gave up.
#[test]
fn a_caller_configured_retry_budget_is_visible_in_the_scheduled_retry_counter() {
    const RETRY_LIMIT: u32 = 2;

    let session = retrying_silent_session(RETRY_LIMIT);
    let diagnostics = session.subscribe_diagnostics(128).expect("diagnostics");
    let camera = session
        .camera::<SonyFr7>(grafton_visca::types::IndividualCameraId::CAMERA_1)
        .expect("camera facade");
    let error = camera
        .submit::<AppliedOnly, _>(&grafton_visca::command::zoom::Tele)
        .expect("submission")
        .applied()
        .expect_err("a silent camera cannot acknowledge");
    assert!(
        matches!(error, Error::Timeout { .. }),
        "the sequence-correlated retry budget must end in Timeout, got {error:?}"
    );

    let metrics = session.metrics().expect("metrics");
    let events: Vec<_> = std::iter::from_fn(|| diagnostics.try_recv()).collect();
    assert_eq!(
        metrics.dropped_diagnostic_events, 0,
        "the scenario must fit the diagnostic subscription for the pairing below to hold"
    );

    assert_eq!(metrics.retries_scheduled, RETRY_LIMIT as u64);
    assert_eq!(
        metrics.ack_timeouts,
        metrics.retries_scheduled + 1,
        "every expiry but the last one schedules a retry: {events:?}"
    );

    let retries = events
        .iter()
        .filter(|event| matches!(event, DiagnosticEvent::RetryScheduled { .. }))
        .count();
    assert_eq!(retries, RETRY_LIMIT as usize);
    assert_eq!(metrics.retries_scheduled, retries as u64);
    assert_eq!(metrics.terminal, 1);
    assert_eq!(metrics.completion_timeouts, 0);
    assert_eq!(metrics.inquiry_timeouts, 0);
    session.shutdown().expect("shutdown");
}
