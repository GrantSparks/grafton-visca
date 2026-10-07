//! Issues #721/#750: a raw camera's named socket reuse supersedes stale local
//! ownership and inert exact-socket quarantine holds.
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
#![allow(clippy::expect_used)]

use grafton_visca_test_support::{facade_matrix, fake_camera, profile_fixtures};

use std::time::Duration;

use grafton_visca::{
    completion::AppliedOnly, profile::ProfileSpec, request::builtin::ZoomDrive,
    CancellationOutcome, Error, SessionConfig,
};

#[cfg(all(
    feature = "async",
    any(feature = "runtime-tokio", feature = "runtime-smol")
))]
use fake_camera::AsyncWire;
#[cfg(feature = "blocking")]
use fake_camera::BlockingWire;
use fake_camera::{frames, FakeCamera};
#[cfg(all(
    feature = "async",
    any(feature = "runtime-tokio", feature = "runtime-smol")
))]
use grafton_visca::transport::AddressingMode;
use profile_fixtures::{NonDefaultCompileTimeProfile, QuarantinedSocketCompileTimeProfile};

const CANCEL_SOCKET_ONE: &[u8] = &[0x81, 0x21, 0xff];

#[derive(Debug, Clone, Copy)]
enum ReuseScenario {
    LiveOwner,
    QuarantinedSocket,
}

/// A raw datagram camera that names socket one for both commands.
fn reused_socket_camera(scenario: ReuseScenario) -> FakeCamera {
    let mut write_number = 0usize;
    FakeCamera::new(move |bytes, answer| {
        write_number += 1;
        match (scenario, write_number) {
            // A earns S1, but its completion is lost. The camera then
            // releases and reuses S1 for B.
            (ReuseScenario::LiveOwner, 1 | 2) => {
                answer.reply(frames::ack(1));
            }
            // The cancellation wire itself is asserted by the test. A normal B
            // completion then proves S1 resolves to B, not displaced A.
            (ReuseScenario::LiveOwner, _) if bytes == CANCEL_SOCKET_ONE => {
                answer.reply(frames::complete(1));
            }
            (ReuseScenario::LiveOwner, _) => {}
            // A timed out after owning S1, so the engine retained an
            // exact S1 hold. The camera nevertheless names S1 for B and
            // completes B normally.
            (ReuseScenario::QuarantinedSocket, 1) => {
                answer.reply(frames::ack(1));
            }
            (ReuseScenario::QuarantinedSocket, _) if bytes == CANCEL_SOCKET_ONE => {
                answer.reply(frames::complete(1));
            }
            (ReuseScenario::QuarantinedSocket, 2) => {
                answer.reply(frames::ack(1)).reply(frames::complete(1));
            }
            (ReuseScenario::QuarantinedSocket, _) => {}
        }
    })
}

/// The camera plus the transport facts each facade reports: the blocking wire
/// reports no addressing hint, the async wire reports IP addressing.
struct Reuse {
    camera: FakeCamera,
}

impl Reuse {
    fn new(scenario: ReuseScenario) -> Self {
        Self {
            camera: reused_socket_camera(scenario),
        }
    }

    /// The blocking wire, in the shape `open!` asks its camera for.
    #[cfg(feature = "blocking")]
    fn blocking_wire(&self) -> BlockingWire {
        self.camera.blocking_wire()
    }

    /// The async wire, in the shape `open!` asks its camera for.
    #[cfg(all(
        feature = "async",
        any(feature = "runtime-tokio", feature = "runtime-smol")
    ))]
    fn async_wire(&self) -> AsyncWire {
        self.camera.async_wire().with_addressing(AddressingMode::Ip)
    }
}

facade_matrix! {
    fn raw_socket_reuse_drives_cancel_and_completion_on_the_named_socket() {
        let fake = Reuse::new(ReuseScenario::LiveOwner);
        let session = open!(
            fake,
            SessionConfig::new(
                ProfileSpec::from_compile_time::<NonDefaultCompileTimeProfile>()
                    .expect("two-socket raw profile"),
            )
        )
        .expect("session");
        let camera = session
            .camera::<NonDefaultCompileTimeProfile>()
            .expect("camera");

        let mut displaced = wait!(camera.submit::<AppliedOnly, _>(&ZoomDrive::Tele))
            .expect("first command writes");
        let mut successor = wait!(camera.submit::<AppliedOnly, _>(&ZoomDrive::Wide))
            .expect("second command writes after A's ACK");

        assert!(matches!(
            wait!(displaced.applied()),
            Err(Error::UnsequencedCommandUnconfirmed)
        ));
        assert_eq!(
            fake.camera.write_count(),
            2,
            "the displaced wait must not emit another command"
        );
        assert_eq!(
            wait!(successor.cancel_with_timeout(Duration::from_secs(1)))
                .expect("S1 completion settles the successor"),
            CancellationOutcome::Completed
        );

        let writes = fake.camera.writes();
        assert_eq!(writes.len(), 3, "two commands and one socket cancellation");
        assert_eq!(writes[2], CANCEL_SOCKET_ONE);
        session.shutdown().expect("shutdown");
    }

    fn raw_ack_reuses_a_quarantined_socket_without_stranding_the_successor() {
        let fake = Reuse::new(ReuseScenario::QuarantinedSocket);
        let session = open!(
            fake,
            SessionConfig::new(
                ProfileSpec::from_compile_time::<QuarantinedSocketCompileTimeProfile>()
                    .expect("two-socket raw profile with short test deadlines"),
            )
        )
        .expect("session");
        let camera = session
            .camera::<QuarantinedSocketCompileTimeProfile>()
            .expect("camera");

        let mut displaced = wait!(camera.submit::<AppliedOnly, _>(&ZoomDrive::Tele))
            .expect("first command writes");
        let displaced_result = wait!(displaced.applied());
        assert!(
            matches!(displaced_result, Err(Error::UnsequencedCommandUnconfirmed)),
            "the protocol terminal installs the raw S1 quarantine before B is admitted: {displaced_result:?}"
        );

        let mut successor = wait!(camera.submit::<AppliedOnly, _>(&ZoomDrive::Wide))
            .expect("successor writes while the stale S1 hold remains");
        wait!(successor.applied())
            .expect("camera-named S1 ACK and completion settle the successor normally");

        assert_eq!(
            fake.camera.write_count(),
            2,
            "two commands and B's normal S1 completion"
        );
        session.shutdown().expect("shutdown");
    }
}
