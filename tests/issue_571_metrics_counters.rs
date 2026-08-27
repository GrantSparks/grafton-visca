//! Issue #571: the field-debugging counters and the retry-decision event as a
//! caller of the published API sees them.
//!
//! The scenario is scripted end to end — a camera that accepts writes and never
//! answers — and the assertions are about what the owner reported, never about
//! which retry policy the profile happens to carry. A retry policy change must
//! not be able to break this file.

#![cfg(feature = "blocking")]
#![allow(clippy::expect_used, clippy::unwrap_used)]

use std::time::Duration;

use grafton_visca::{
    blocking::Session,
    command::{CommandKind, ImageFreeze},
    profile::ProfileSpec,
    profiles::PtzOpticsG2,
    transport::{
        AddressingMode, BlockingTransport, HasTransportConfig, SendSemantics, TransportConfig,
    },
    DiagnosticDeadline, DiagnosticEvent, Error, SessionConfig,
};

/// A camera that takes every frame and never answers one.
#[derive(Debug, Default)]
struct SilentCamera {
    config: TransportConfig,
}

impl HasTransportConfig for SilentCamera {
    fn transport_config(&self) -> &TransportConfig {
        &self.config
    }
}

impl BlockingTransport for SilentCamera {
    fn send_with_kind(&mut self, _bytes: &[u8], _kind: CommandKind) -> Result<(), Error> {
        Ok(())
    }

    fn recv_into(&mut self, dst: &mut [u8]) -> Result<usize, Error> {
        self.recv_into_with_timeout(dst, Duration::from_millis(1))
    }

    fn recv_into_with_timeout(
        &mut self,
        _dst: &mut [u8],
        _timeout: Duration,
    ) -> Result<usize, Error> {
        // A read that found nothing, not a transport fault.
        Err(Error::Timeout)
    }

    fn addressing_mode_hint(&self) -> Option<AddressingMode> {
        Some(AddressingMode::Ip)
    }

    fn send_semantics(&self) -> SendSemantics {
        SendSemantics::Datagram
    }
}

fn silent_session() -> Session {
    // The profile's own ACK deadline is used as-is: operational tuning may only
    // lengthen a profile deadline, and nothing here should depend on its value.
    let config = SessionConfig::new(
        ProfileSpec::from_compile_time::<PtzOpticsG2>().expect("built-in G2 profile"),
    );
    Session::open(SilentCamera::default(), config).expect("session opens over a silent camera")
}

/// The counter exists, and every expiry carries the retry decision that the
/// event stream then confirms — which is the whole point: a subscriber no
/// longer has to infer the decision from a phase change and a missing retry.
#[test]
fn an_unanswered_command_counts_ack_timeouts_and_reports_each_retry_decision() {
    let session = silent_session();
    let camera = session.camera::<PtzOpticsG2>().expect("camera facade");
    let error = camera
        .execute(&ImageFreeze::on())
        .expect_err("a silent camera cannot acknowledge");
    assert!(
        matches!(error, Error::Timeout | Error::CommandTimeout { .. }),
        "expected a timeout, got {error:?}"
    );

    let metrics = session.metrics().expect("metrics");
    let events = session.drain_diagnostics().expect("diagnostics");
    assert_eq!(
        metrics.dropped_diagnostics, 0,
        "the scenario must fit the diagnostic ring for the pairing below to hold"
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
    assert!(
        !expiries.is_empty(),
        "no ACK deadline expiry was reported: {events:?}"
    );
    let retries = events
        .iter()
        .filter(|event| matches!(event, DiagnosticEvent::RetryScheduled { .. }))
        .count();
    assert_eq!(
        expiries.iter().filter(|will_retry| **will_retry).count(),
        retries,
        "each expiry claiming a retry must be matched by exactly one scheduled retry"
    );
    assert_eq!(
        expiries.last(),
        Some(&false),
        "the expiry that gave up must say so"
    );

    assert_eq!(metrics.ack_timeouts, expiries.len() as u64);
    assert_eq!(metrics.retries_scheduled, retries as u64);
    assert_eq!(metrics.completion_timeouts, 0);
    assert_eq!(metrics.inquiry_timeouts, 0);
    assert_eq!(metrics.busy_errors, 0);
    assert_eq!(metrics.protocol_errors, 0);
    assert_eq!(metrics.ignored_unmatched_sequenced_replies, 0);
    session.shutdown().expect("shutdown");
}
