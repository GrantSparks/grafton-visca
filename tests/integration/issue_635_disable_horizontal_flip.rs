//! Issue #635: every noun surface can clear the horizontal mirror.
//!
//! `enable_horizontal_flip` shipped without its twin. These tests exercise the
//! source-backed PTZOptics mirror family; FR7 does not grant this opcode.
//! They drive
//! `disable_horizontal_flip` on all three noun surfaces against a fake camera
//! and assert the complete VISCA frame.

#![allow(clippy::expect_used, clippy::unwrap_used)]
#![cfg(any(
    feature = "blocking",
    all(feature = "async", feature = "runtime-tokio")
))]

use grafton_visca_test_support::fake_camera;

use fake_camera::{frames, FakeCamera};

/// `CAM_LR_Reverse On` — the frame `enable_horizontal_flip` writes.
const MIRROR_ON: &[u8] = &[0x81, 0x01, 0x04, 0x61, 0x02, 0xFF];

/// `CAM_LR_Reverse Off` — the frame `disable_horizontal_flip` must write.
const MIRROR_OFF: &[u8] = &[0x81, 0x01, 0x04, 0x61, 0x03, 0xFF];

/// A camera that answers every command with ACK + completion, in the framing
/// of the request.
fn acking_camera() -> FakeCamera {
    FakeCamera::visca(|_, answer| {
        answer.reply(frames::ack(1)).reply(frames::complete(1));
    })
}

#[cfg(feature = "blocking")]
mod blocking_surface {
    use grafton_visca::{
        blocking::Session,
        config::SessionConfig,
        profiles::{ProfileSpec, PtzOpticsG2},
    };

    use super::{acking_camera, FakeCamera, MIRROR_OFF, MIRROR_ON};

    fn session(profile: ProfileSpec) -> (Session, FakeCamera) {
        let camera = acking_camera();
        let session =
            Session::open(camera.blocking_wire(), &SessionConfig::new(profile)).expect("session");
        (session, camera)
    }

    #[test]
    fn blocking_noun_clears_the_horizontal_mirror() {
        let (session, fake) =
            session(ProfileSpec::from_compile_time::<PtzOpticsG2>().expect("PTZOptics profile"));
        let camera = session
            .camera::<PtzOpticsG2>(grafton_visca::types::IndividualCameraId::CAMERA_1)
            .expect("camera");

        camera.image().set_mirror(true).expect("mirror on");
        assert_eq!(fake.take_only_payload(), MIRROR_ON);

        camera.image().set_mirror(false).expect("mirror off");
        assert_eq!(fake.take_only_payload(), MIRROR_OFF);

        session.shutdown().expect("shutdown");
    }
}

#[cfg(all(feature = "async", feature = "runtime-tokio"))]
mod async_surface {
    use grafton_visca::{
        config::SessionConfig,
        profiles::{ProfileSpec, PtzOpticsG2},
        r#async::Session,
        runtime::TokioRuntime,
        transport::AddressingMode,
    };

    use super::{acking_camera, FakeCamera, MIRROR_OFF, MIRROR_ON};

    pub(super) async fn open(profile: ProfileSpec) -> (Session, FakeCamera) {
        let camera = acking_camera();
        let session = Session::open(
            camera.async_wire().with_addressing(AddressingMode::Ip),
            &SessionConfig::new(profile),
            TokioRuntime::from_current().expect("Tokio runtime"),
        )
        .await
        .expect("session");
        (session, camera)
    }

    #[tokio::test]
    async fn async_noun_clears_the_horizontal_mirror() {
        let (session, fake) =
            open(ProfileSpec::from_compile_time::<PtzOpticsG2>().expect("PTZOptics profile")).await;
        let camera = session
            .camera::<PtzOpticsG2>(grafton_visca::types::IndividualCameraId::CAMERA_1)
            .expect("camera");

        camera.image().set_mirror(true).await.expect("mirror on");
        assert_eq!(fake.take_only_payload(), MIRROR_ON);

        camera.image().set_mirror(false).await.expect("mirror off");
        assert_eq!(fake.take_only_payload(), MIRROR_OFF);

        session.shutdown().expect("shutdown");
    }
}

#[cfg(all(
    any(feature = "async", feature = "blocking"),
    feature = "runtime-tokio"
))]
mod runtime_profile_surface {
    use grafton_visca::profiles::{ProfileSpec, PtzOpticsG2};

    use super::{async_surface::open, MIRROR_OFF, MIRROR_ON};

    #[tokio::test]
    async fn dynamic_noun_clears_the_horizontal_mirror() {
        let (session, fake) =
            open(ProfileSpec::from_compile_time::<PtzOpticsG2>().expect("PTZOptics profile")).await;
        let camera = session
            .camera::<grafton_visca::profiles::AnyProfile>(
                grafton_visca::types::IndividualCameraId::CAMERA_1,
            )
            .expect("dynamic camera");
        let nouns = &camera;

        nouns.image().set_mirror(true).await.expect("mirror on");
        assert_eq!(fake.take_only_payload(), MIRROR_ON);

        nouns.image().set_mirror(false).await.expect("mirror off");
        assert_eq!(fake.take_only_payload(), MIRROR_OFF);

        session.shutdown().expect("shutdown");
    }
}
