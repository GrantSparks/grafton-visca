//! Issue #690: `set_tuning` observes the session's terminal boundary on both
//! facades.
//!
//! `set_tuning` returns the session's terminal error once the owner is gone.
//! It travels through the owner's control boundary (#780) on both facades: a
//! live session still reconfigures, and a terminated session yields its
//! retained terminal cause. Profile validation failures travel through that
//! boundary too, so an invalid proposal cannot mask an existing terminal
//! cause, and a live session rejects an invalid proposal without changing the
//! installed tuning.
//!
//! Every scenario runs on the blocking facade and on the async facade under
//! each enabled runtime.

#![cfg(any(
    feature = "blocking",
    all(
        feature = "async",
        any(feature = "runtime-tokio", feature = "runtime-smol")
    )
))]

#[path = "common/fake_camera.rs"]
mod fake_camera;
#[macro_use]
#[path = "common/matrix.rs"]
mod matrix;
#[path = "common/profile_fixtures.rs"]
mod profile_fixtures;

use std::{sync::Arc, time::Duration};

use grafton_visca::{
    completion::AppliedOnly, profile::ProfileSpec, request::builtin::ZoomStop, Error,
    OperationalTuning, SessionConfig,
};

use fake_camera::FakeCamera;
use profile_fixtures::NonDefaultCompileTimeProfile;

fn raw_config() -> SessionConfig {
    SessionConfig::new(
        ProfileSpec::from_compile_time::<NonDefaultCompileTimeProfile>()
            .expect("raw runtime profile"),
    )
}

/// A raw datagram camera that fails the read after its first write. Under the
/// strict construction policy that fault poisons the session while the
/// unsequenced command is awaiting its ACK.
fn faulting_camera() -> FakeCamera {
    let mut writes = 0usize;
    FakeCamera::new(move |_, answer| {
        writes += 1;
        if writes == 1 {
            answer.fault(Error::Io(Arc::new(std::io::Error::from(
                std::io::ErrorKind::ConnectionRefused,
            ))));
        }
    })
}

facade_matrix! {
    /// A live session still accepts a valid retune.
    fn set_tuning_on_a_live_session_still_succeeds() {
        let session = open!(FakeCamera::acking(1), raw_config()).expect("owner session");
        let tuning = OperationalTuning::new().command_spacing(Duration::from_millis(40));
        wait!(session.set_tuning(tuning)).expect("a live session accepts a valid retune");
        assert_eq!(
            session.tuning(),
            tuning,
            "the retune took effect on the live session"
        );
        session.shutdown().expect("owner shutdown");
    }

    fn set_tuning_on_a_live_session_rejects_invalid_tuning_without_changing_it() {
        let session = open!(FakeCamera::acking(1), raw_config()).expect("owner session");
        let installed = OperationalTuning::new().command_spacing(Duration::from_millis(40));
        wait!(session.set_tuning(installed)).expect("a live session accepts valid tuning");

        let error = wait!(session.set_tuning(OperationalTuning::new().ack_timeout(Duration::ZERO)))
            .expect_err("a live session still rejects a zero protocol timeout");
        assert!(matches!(error, Error::InvalidRequest(_)), "got {error:?}");
        assert_eq!(
            session.tuning(),
            installed,
            "a rejected live update leaves the installed tuning unchanged"
        );
        session.shutdown().expect("owner shutdown");
    }

    fn set_tuning_on_a_poisoned_session_returns_the_terminal_error() {
        let config = raw_config().with_strict_unconfirmed_poison(true);
        let session = open!(faulting_camera(), config).expect("owner session");
        {
            let camera = session
                .camera::<NonDefaultCompileTimeProfile>()
                .expect("camera view");
            let error = wait!(
                wait!(camera.submit::<AppliedOnly, _>(&ZoomStop))
                    .expect("submission")
                    .applied()
            )
            .expect_err("the strict opt-in poisons on the receive fault");
            assert!(matches!(error, Error::StreamPoisoned { .. }));
        }

        // Even an otherwise-valid update must report the retained terminal
        // cause once the owner is poisoned.
        let error = wait!(session.set_tuning(OperationalTuning::new()))
            .expect_err("set_tuning must not silently succeed on a poisoned session");
        assert!(
            matches!(error, Error::StreamPoisoned { .. }),
            "set_tuning surfaces the session's terminal error, got {error:?}"
        );
        assert!(error.requires_new_session());

        let error = wait!(session.set_tuning(OperationalTuning::new().ack_timeout(Duration::ZERO)))
            .expect_err("profile validation cannot mask an existing poison");
        assert!(
            matches!(error, Error::StreamPoisoned { .. }),
            "set_tuning gives the poison precedence over InvalidRequest, got {error:?}"
        );
    }

    /// `shutdown` leaves the handle usable and its terminal cause retained.
    fn set_tuning_after_shutdown_returns_the_terminal_error() {
        let session = open!(FakeCamera::acking(1), raw_config()).expect("owner session");
        session.shutdown().expect("clean shutdown");
        assert_updates_report_shutdown(
            wait!(session.set_tuning(OperationalTuning::new())),
            wait!(session.set_tuning(OperationalTuning::new().ack_timeout(Duration::ZERO))),
        );
    }

    /// A consuming `close` leaves a cloned handle with the same terminal cause.
    fn set_tuning_after_close_returns_the_terminal_error() {
        let session = open!(FakeCamera::acking(1), raw_config()).expect("owner session");
        let closed_handle = session.clone();
        wait!(session.close()).expect("clean owner shutdown");
        assert_updates_report_shutdown(
            wait!(closed_handle.set_tuning(OperationalTuning::new())),
            wait!(closed_handle.set_tuning(OperationalTuning::new().ack_timeout(Duration::ZERO))),
        );
    }
}

/// Both a valid and a profile-invalid update report the completed shutdown.
fn assert_updates_report_shutdown(valid: Result<(), Error>, invalid: Result<(), Error>) {
    let error = valid.expect_err("set_tuning must not silently succeed on a closed session");
    assert!(
        matches!(error, Error::RuntimeShutdown),
        "set_tuning surfaces the shutdown boundary, got {error:?}"
    );

    let error = invalid.expect_err("profile validation cannot mask a completed shutdown");
    assert!(
        matches!(error, Error::RuntimeShutdown),
        "set_tuning gives shutdown precedence over InvalidRequest, got {error:?}"
    );
}
