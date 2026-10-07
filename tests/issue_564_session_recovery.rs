//! Issue #564: `Error::requires_new_session()` classification and the
//! documented poison/close → rebuild recovery recipe.
//!
//! Issue #542 makes transport close, explicit shutdown, and poison distinct
//! terminal errors that all share `ErrorKind::IoClosed`. These tests pin the
//! public classification that separates them, and they exercise the recovery
//! recipe end to end: a terminal session is abandoned, a fresh session is built
//! from the same reusable `SessionConfig`, and a command is driven through it.

#![cfg(any(
    feature = "blocking",
    all(feature = "async", feature = "runtime-tokio")
))]

use std::{io, sync::Arc};

use grafton_visca_test_support::fake_camera;

#[cfg(all(feature = "async", feature = "runtime-tokio"))]
use fake_camera::AsyncWire;
#[cfg(feature = "blocking")]
use fake_camera::BlockingWire;
use fake_camera::FakeCamera;
use grafton_visca::{
    transport::{AddressingMode, TransportConfig},
    Error, ProfileSpec, SessionConfig,
};

fn profile() -> ProfileSpec {
    ProfileSpec::from_compile_time::<grafton_visca::profiles::PtzOpticsG2>().expect("profile")
}

/// A serial-addressed stream camera whose writes fail after `successful_writes`
/// accepted sends. Every accepted command is answered with an ACK and a
/// completion from the camera addressed by the request.
fn stream_camera(successful_writes: usize) -> FakeCamera {
    let mut remaining = successful_writes;
    FakeCamera::new(move |write, answer| {
        if remaining == 0 {
            answer.fail_send(Error::Io(Arc::new(io::Error::new(
                io::ErrorKind::BrokenPipe,
                "peer went away mid-frame",
            ))));
            return;
        }
        remaining -= 1;
        let target = write.first().copied().unwrap_or(0x81) & 0x0f;
        let source = 0x80 | (target.saturating_add(8) << 4);
        answer
            .reply(vec![source, 0x41, 0xff])
            .reply(vec![source, 0x51, 0xff]);
    })
}

#[cfg(feature = "blocking")]
/// A serial-addressed stream camera that reports peer closure, a zero-byte
/// read, in answer to a write.
fn closing_camera() -> FakeCamera {
    FakeCamera::new(|_, answer| {
        answer.reply(Vec::new());
    })
}

/// Serial addressing selects stream send semantics, which is the transport
/// class the spec poisons on a write failure.
fn serial_config() -> TransportConfig {
    let mut config = TransportConfig::default();
    config.addressing = AddressingMode::Serial;
    config
}

#[cfg(feature = "blocking")]
mod blocking_recovery {
    use grafton_visca::{
        blocking::Session,
        state_cache::StateEntry,
        transport::{AddressingMode, SendSemantics},
        Error, ErrorKind, StateKey,
    };

    use super::{
        closing_camera, profile, serial_config, stream_camera, BlockingWire, FakeCamera,
        SessionConfig,
    };

    /// A stream transport onto `camera`, addressed serially.
    fn stream_wire(camera: &FakeCamera) -> BlockingWire {
        camera
            .blocking_wire()
            .with_config(serial_config())
            .with_semantics(SendSemantics::Stream)
            .with_addressing(AddressingMode::Serial)
    }

    #[test]
    fn poisoned_session_requires_a_new_session_and_the_rebuilt_one_works() {
        // The reusable configuration is what recovery keeps.
        let config = SessionConfig::new(profile());

        let session = Session::open(stream_wire(&stream_camera(1)), config.clone())
            .expect("open the original session");
        let camera = session
            .camera::<grafton_visca::profiles::PtzOpticsG2>()
            .expect("camera view");
        camera
            .advanced()
            .multicast_on()
            .expect("the first command applies before the transport fails");

        let poisoned = camera
            .advanced()
            .multicast_off()
            .expect_err("a stream write failure poisons the session");
        assert!(
            matches!(poisoned, Error::StreamPoisoned { .. }),
            "expected the exact terminal session error, got {poisoned}"
        );
        assert!(
            poisoned.requires_new_session(),
            "a poisoned session must classify as needing a replacement"
        );

        // A poisoned session is terminal: every later operation keeps reporting
        // a condition that classifies the same way, and it is never revived.
        let retained = camera
            .advanced()
            .multicast_on()
            .expect_err("a poisoned session never accepts new work");
        assert!(retained.requires_new_session());
        assert!(session
            .camera::<grafton_visca::profiles::PtzOpticsG2>()
            .expect("camera view")
            .inquire(&grafton_visca::command::ZoomPositionInquiry)
            .expect_err("a poisoned session rejects inquiries too")
            .requires_new_session());

        // Recovery: drop everything bound to the dead owner and rebuild from
        // the same reusable configuration with a fresh transport.
        drop(camera);
        drop(session);

        let rebuilt = Session::open(stream_wire(&stream_camera(usize::MAX)), config)
            .expect("rebuild a fresh session");
        let rebuilt_camera = rebuilt
            .camera::<grafton_visca::profiles::PtzOpticsG2>()
            .expect("camera view on the rebuilt session");
        let cache = rebuilt_camera.state_cache();
        assert_eq!(
            cache.value(StateKey::MulticastStreaming),
            StateEntry::Unknown,
            "a fresh session starts with an unknown state cache"
        );
        rebuilt_camera
            .advanced()
            .multicast_on()
            .expect("the rebuilt session drives a command through to completion");
        match cache.value(StateKey::MulticastStreaming) {
            StateEntry::Set(value) => assert_eq!(value.as_slice(), &[1]),
            other => {
                panic!("expected the rebuilt session to record the applied state, got {other:?}")
            }
        }
        rebuilt.shutdown().expect("shutdown the rebuilt session");
    }

    #[test]
    fn peer_closure_requires_a_new_session() {
        let session = Session::open(
            stream_wire(&closing_camera()),
            SessionConfig::new(profile()),
        )
        .expect("open a session");
        let closed = session
            .camera::<grafton_visca::profiles::PtzOpticsG2>()
            .expect("camera view")
            .advanced()
            .multicast_on()
            .expect_err("a zero-byte read closes the session");
        assert!(
            matches!(closed, Error::ConnectionClosed { .. }),
            "expected the exact terminal session error, got {closed}"
        );
        assert!(closed.requires_new_session());
    }

    #[test]
    fn deliberate_shutdown_does_not_require_a_new_session() {
        let session = Session::open(
            stream_wire(&stream_camera(usize::MAX)),
            SessionConfig::new(profile()),
        )
        .expect("open a session");
        let camera = session
            .camera::<grafton_visca::profiles::PtzOpticsG2>()
            .expect("camera view");
        camera.advanced().multicast_on().expect("first command");

        session.shutdown().expect("deliberate shutdown");

        let stopped = camera
            .advanced()
            .multicast_off()
            .expect_err("a shut-down session rejects new work");
        assert!(
            matches!(stopped, Error::RuntimeShutdown),
            "expected the deliberate shutdown error, got {stopped}"
        );
        assert!(
            !stopped.requires_new_session(),
            "the application ended this session on purpose; it is not a field disconnect"
        );
        // The distinction is invisible to the error kind, which is exactly why
        // the classification is exposed as its own predicate.
        assert_eq!(stopped.kind(), ErrorKind::IoClosed);
    }
}

#[cfg(all(feature = "async", feature = "runtime-tokio"))]
mod tokio_recovery {
    use grafton_visca::{
        runtime::TokioRuntime,
        transport::{AddressingMode, SendSemantics},
        Error, Session,
    };

    use super::{profile, serial_config, stream_camera, AsyncWire, FakeCamera, SessionConfig};

    /// A stream transport onto `camera`, addressed serially.
    fn stream_wire(camera: &FakeCamera) -> AsyncWire {
        camera
            .async_wire()
            .with_config(serial_config())
            .with_semantics(SendSemantics::Stream)
            .with_addressing(AddressingMode::Serial)
    }

    #[tokio::test]
    async fn poisoned_async_session_requires_a_new_session_and_the_rebuilt_one_works() {
        let config = SessionConfig::new(profile());
        let session = Session::open(
            stream_wire(&stream_camera(1)),
            config.clone(),
            TokioRuntime::from_current().expect("tokio runtime"),
        )
        .await
        .expect("open the original session");
        let camera = session
            .camera::<grafton_visca::profiles::PtzOpticsG2>()
            .expect("camera view");
        camera
            .advanced()
            .multicast_on()
            .await
            .expect("the first command applies before the transport fails");

        let poisoned = camera
            .advanced()
            .multicast_off()
            .await
            .expect_err("a stream write failure poisons the session");
        assert!(
            matches!(poisoned, Error::StreamPoisoned { .. }),
            "expected the exact terminal session error, got {poisoned}"
        );
        assert!(poisoned.requires_new_session());
        assert!(camera
            .advanced()
            .multicast_on()
            .await
            .expect_err("a poisoned session never accepts new work")
            .requires_new_session());

        drop(camera);
        drop(session);

        let rebuilt = Session::open(
            stream_wire(&stream_camera(usize::MAX)),
            config,
            TokioRuntime::from_current().expect("tokio runtime"),
        )
        .await
        .expect("rebuild a fresh session");
        rebuilt
            .camera::<grafton_visca::profiles::PtzOpticsG2>()
            .expect("camera view on the rebuilt session")
            .advanced()
            .multicast_on()
            .await
            .expect("the rebuilt session drives a command through to completion");
        rebuilt.shutdown().expect("shutdown");
    }

    #[tokio::test]
    async fn deliberate_async_shutdown_does_not_require_a_new_session() {
        let session = Session::open(
            stream_wire(&stream_camera(usize::MAX)),
            SessionConfig::new(profile()),
            TokioRuntime::from_current().expect("tokio runtime"),
        )
        .await
        .expect("open a session");
        let camera = session
            .camera::<grafton_visca::profiles::PtzOpticsG2>()
            .expect("camera view");
        camera
            .advanced()
            .multicast_on()
            .await
            .expect("first command");

        session.shutdown().expect("deliberate shutdown");

        let stopped = camera
            .advanced()
            .multicast_off()
            .await
            .expect_err("a shut-down session rejects new work");
        assert!(
            matches!(stopped, Error::RuntimeShutdown),
            "expected the deliberate shutdown error, got {stopped}"
        );
        assert!(!stopped.requires_new_session());
    }
}
