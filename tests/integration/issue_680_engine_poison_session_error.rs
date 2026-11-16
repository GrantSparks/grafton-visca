//! Issue #680: an engine-initiated poison must set the owner's session error,
//! so `shutdown()`/`close()` surface the true terminal cause instead of masking
//! it as a deliberate `RuntimeShutdown`.
//!
//! Owner-supplied `Close`/`Poison`/`Shutdown` inputs always set the session
//! error, but an engine-initiated terminal transition — here the strict
//! `strict_unconfirmed_poison` opt-in reacting to a receive fault while a raw
//! command is unacknowledged — reaches the owner through a `SessionEnded`
//! effect carrying the complete verdict. The owner publishes that cause
//! directly, so a `shutdown()`/`close()` on the dead
//! session returns `StreamPoisoned` (which `requires_new_session()`), the
//! `SessionChanged` diagnostic carries the real `ErrorKind`, and the documented
//! supervisor recovery loop actually rebuilds.

#![cfg(feature = "blocking")]

use grafton_visca_test_support::{fake_camera, profile_fixtures};

use std::sync::Arc;

use fake_camera::FakeCamera;
use grafton_visca::{
    blocking::Session,
    config::SessionConfig,
    error::ErrorKind,
    observability::{DiagnosticEvent, SessionStatus},
    operation::AppliedOnly,
    profiles::ProfileSpec,
    Error,
};

use profile_fixtures::NonDefaultCompileTimeProfile;

/// A raw datagram camera that fails the read pump following its first write, so
/// a raw command left awaiting its ACK meets a receive fault — the trigger for
/// the strict single-candidate poison.
fn poison_on_read_camera() -> FakeCamera {
    let mut sends = 0_usize;
    FakeCamera::new(move |_, answer| {
        sends = sends.saturating_add(1);
        if sends == 1 {
            answer.fault(Error::Io {
                source: Arc::new(std::io::Error::from(std::io::ErrorKind::ConnectionRefused)),
            });
        }
    })
}

fn strict_session() -> Session {
    let config = SessionConfig::new(
        ProfileSpec::from_compile_time::<NonDefaultCompileTimeProfile>()
            .expect("raw runtime profile"),
    )
    .with_strict_unconfirmed_poison(true);
    Session::open(poison_on_read_camera().blocking_wire(), &config).expect("owner session")
}

/// Drive the strict opt-in poison and return the session, now terminated by the
/// engine's own verdict.
fn poison_via_strict_opt_in(session: &Session) {
    let camera = session
        .camera::<NonDefaultCompileTimeProfile>(grafton_visca::types::IndividualCameraId::CAMERA_1)
        .expect("camera view");
    let error = camera
        .submit::<AppliedOnly, _>(&grafton_visca::command::zoom::Stop)
        .expect("submission")
        .applied()
        .expect_err("the strict opt-in poisons on the receive fault");
    assert!(
        matches!(error, Error::StreamPoisoned { .. }),
        "the engine's terminal cause is a stream poison, got {error:?}"
    );
}

#[test]
fn shutdown_after_engine_poison_surfaces_the_terminal_cause_not_runtime_shutdown() {
    let session = strict_session();
    poison_via_strict_opt_in(&session);

    // Before the fix this returned Ok(()) and then re-latched RuntimeShutdown
    // (requires_new_session() == false), stranding the recovery loop.
    let error = session
        .shutdown()
        .expect_err("shutdown must surface the engine's poison, not mask it");
    assert!(
        matches!(error, Error::StreamPoisoned { .. }),
        "shutdown surfaces the true terminal cause, got {error:?}"
    );
    assert!(
        error.requires_new_session(),
        "a poisoned session must still ask for a replacement after shutdown()"
    );
}

#[test]
fn close_after_engine_poison_surfaces_the_terminal_cause() {
    let session = strict_session();
    poison_via_strict_opt_in(&session);

    let error = session
        .close()
        .expect_err("close must surface the engine's poison");
    assert!(
        matches!(error, Error::StreamPoisoned { .. }),
        "close surfaces the true terminal cause, got {error:?}"
    );
    assert!(error.requires_new_session());
}

#[test]
fn engine_poison_reports_poisoned_status_and_names_the_diagnostic_reason() {
    let session = strict_session();
    let diagnostics = session.subscribe_diagnostics(128).expect("diagnostics");
    poison_via_strict_opt_in(&session);

    let metrics = session.metrics().expect("terminal metrics survive poison");
    assert_eq!(metrics.session, SessionStatus::Poisoned);
    assert_eq!(metrics.active, 0);
    assert_eq!(metrics.pending, 0);
    // The SessionChanged diagnostic reason is the real ErrorKind (IoClosed for a
    // stream poison), not the Other it collapsed to while session_error stayed
    // unset.
    let diagnostics: Vec<_> = std::iter::from_fn(|| diagnostics.try_recv()).collect();
    let reason = diagnostics
        .iter()
        .find_map(|event| match event {
            DiagnosticEvent::SessionChanged { to, reason, .. }
                if *to == SessionStatus::Poisoned =>
            {
                Some(*reason)
            }
            _ => None,
        })
        .expect("a SessionChanged diagnostic into Poisoned was recorded");
    assert_eq!(
        reason,
        ErrorKind::IoClosed,
        "the poison's diagnostic reason must be IoClosed, not Other"
    );
}

/// The documented recovery loop: a caller that keys on `requires_new_session()`
/// must be able to detect the dead session and rebuild it. Before the fix the
/// `shutdown()` in a cleanup path returned `Ok(())` and the follow-up verdict
/// was `RuntimeShutdown`, so this loop would never rebuild.
#[test]
fn engine_poison_drives_a_supervisor_rebuild() {
    let mut session = strict_session();
    poison_via_strict_opt_in(&session);

    let mut rebuilt = false;
    for _ in 0..2 {
        match session.shutdown() {
            Ok(()) => break,
            Err(error) if error.requires_new_session() => {
                // A supervisor would open a fresh transport here; a healthy
                // replacement session shuts down cleanly.
                session = healthy_session();
                rebuilt = true;
            }
            Err(other) => panic!("unexpected terminal error: {other:?}"),
        }
    }
    assert!(rebuilt, "the poisoned session drove exactly one rebuild");
    session
        .shutdown()
        .expect("the rebuilt session shuts down cleanly");
}

fn healthy_session() -> Session {
    let config = SessionConfig::new(
        ProfileSpec::from_compile_time::<NonDefaultCompileTimeProfile>()
            .expect("raw runtime profile"),
    );
    // A camera that answers every command, for the rebuilt half of the
    // recovery loop.
    Session::open(FakeCamera::acking(1).blocking_wire(), &config).expect("healthy session")
}

/// #951: socket reuse is evidence of a lost completion, so the strict policy
/// ends both operations and the owner with the same recoverable session cause.
#[test]
fn socket_reuse_in_a_strict_session_surfaces_the_poison_to_every_observer() {
    use grafton_visca::{config::OperationalTuning, observability::DiagnosticPhase};
    use std::time::Duration;

    let camera = FakeCamera::new(|_, answer| {
        answer.reply(fake_camera::frames::ack(1));
    });
    let config = SessionConfig::new(
        ProfileSpec::from_compile_time::<NonDefaultCompileTimeProfile>().expect("raw profile"),
    )
    .with_strict_unconfirmed_poison(true)
    .with_tuning(
        OperationalTuning::new()
            .with_ack_timeout(Duration::from_secs(5))
            .with_quick_timeout(Duration::from_secs(5)),
    );
    let session = Session::open(camera.blocking_wire(), &config).expect("strict session");
    let diagnostics = session.subscribe_diagnostics(128).expect("diagnostics");
    let view = session
        .camera::<NonDefaultCompileTimeProfile>(grafton_visca::types::IndividualCameraId::CAMERA_1)
        .expect("view");
    let mut first = view
        .submit::<AppliedOnly, _>(&grafton_visca::command::zoom::Stop)
        .expect("first command");
    // Synchronize on the engine transition, so the second command's ACK
    // displaces an executing owner rather than competing with an unpaid ACK.
    loop {
        let event = diagnostics
            .recv_with_timeout(Duration::from_secs(1))
            .expect("owner diagnostic")
            .expect("first command acknowledged promptly");
        if matches!(
            event,
            DiagnosticEvent::Transition {
                to: DiagnosticPhase::Executing,
                ..
            }
        ) {
            break;
        }
    }
    let mut second = view
        .submit::<AppliedOnly, _>(&grafton_visca::command::zoom::Stop)
        .expect("second command");
    for error in [
        first
            .applied()
            .expect_err("stale completion poisons first operation"),
        second
            .applied()
            .expect_err("session poison reaches successor"),
        session
            .clone()
            .close()
            .expect_err("session retains its poison"),
    ] {
        assert!(
            matches!(error, Error::StreamPoisoned { ref source, .. }
            if matches!(source.as_ref(), Error::UnsequencedCommandUnconfirmed)),
            "{error:?}"
        );
        assert!(error.requires_new_session());
    }
    let metrics = session.metrics().expect("terminal metrics");
    assert_eq!(metrics.session, SessionStatus::Poisoned);
    assert_eq!((metrics.active, metrics.pending), (0, 0));
}
