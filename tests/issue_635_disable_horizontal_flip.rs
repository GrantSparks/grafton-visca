//! Issue #635: every noun surface can clear the horizontal mirror.
//!
//! `enable_horizontal_flip` shipped without its twin, so on a profile that has
//! `HasImageMirror` but not `HasCombinedImageFlip` — SonyFR7 here — no noun
//! method could return the mirror to off.  These tests drive
//! `disable_horizontal_flip` on all three noun surfaces against a fake camera
//! and assert the absolute VISCA frame, then pin the state-cache consequence
//! of the mirror opcode on the noun path.

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
        blocking::{Session, SessionConfig},
        command::{FlipState, ImageFlipMode},
        profile::ProfileSpec,
        profiles::{PtzOpticsG2, SonyFR7},
        state_cache::StateEntry,
        StateKey,
    };

    use super::{acking_camera, FakeCamera, MIRROR_OFF, MIRROR_ON};

    fn session(profile: ProfileSpec) -> (Session, FakeCamera) {
        let camera = acking_camera();
        let session =
            Session::open(camera.blocking_wire(), SessionConfig::new(profile)).expect("session");
        (session, camera)
    }

    #[test]
    fn blocking_noun_clears_the_horizontal_mirror() {
        let (session, fake) =
            session(ProfileSpec::from_compile_time::<SonyFR7>().expect("FR7 profile"));
        let camera = session.camera::<SonyFR7>().expect("camera");

        camera.image().enable_horizontal_flip().expect("mirror on");
        assert_eq!(fake.take_only_payload(), MIRROR_ON);

        camera
            .image()
            .disable_horizontal_flip()
            .expect("mirror off");
        assert_eq!(fake.take_only_payload(), MIRROR_OFF);

        session.shutdown().expect("shutdown");
    }

    #[test]
    fn blocking_mirror_off_invalidates_the_cached_flip_pair() {
        let (session, fake) =
            session(ProfileSpec::from_compile_time::<PtzOpticsG2>().expect("PTZOptics profile"));
        let camera = session.camera::<PtzOpticsG2>().expect("camera");

        // Only the combined opcode establishes both axes, so start from a
        // known pair.
        camera
            .image()
            .set_flip_mode(ImageFlipMode::Both)
            .expect("combined flip");
        let _ = fake.take_only_payload();
        assert_eq!(
            camera.state_cache().flip_state(),
            Some(FlipState {
                horizontal: true,
                vertical: true,
            }),
        );

        // The mirror opcode moves one axis and says nothing about the other,
        // so the pair stops being known — in both directions.
        camera
            .image()
            .disable_horizontal_flip()
            .expect("mirror off");
        assert_eq!(fake.take_only_payload(), MIRROR_OFF);
        assert_eq!(
            camera.state_cache().value(StateKey::Flip),
            StateEntry::Unknown,
        );
        assert_eq!(camera.state_cache().flip_state(), None);

        camera
            .image()
            .set_flip_mode(ImageFlipMode::Both)
            .expect("combined flip");
        let _ = fake.take_only_payload();
        camera.image().enable_horizontal_flip().expect("mirror on");
        assert_eq!(fake.take_only_payload(), MIRROR_ON);
        assert_eq!(
            camera.state_cache().value(StateKey::Flip),
            StateEntry::Unknown,
        );

        session.shutdown().expect("shutdown");
    }
}

#[cfg(all(feature = "async", feature = "runtime-tokio"))]
mod async_surface {
    use grafton_visca::{
        profile::ProfileSpec, profiles::SonyFR7, transport::AddressingMode, Session, SessionConfig,
        TokioRuntime,
    };

    use super::{acking_camera, FakeCamera, MIRROR_OFF, MIRROR_ON};

    pub(super) async fn open(profile: ProfileSpec) -> (Session, FakeCamera) {
        let camera = acking_camera();
        let session = Session::open(
            camera.async_wire().with_addressing(AddressingMode::Ip),
            SessionConfig::new(profile),
            TokioRuntime::from_current().expect("Tokio runtime"),
        )
        .await
        .expect("session");
        (session, camera)
    }

    #[tokio::test]
    async fn async_noun_clears_the_horizontal_mirror() {
        let (session, fake) =
            open(ProfileSpec::from_compile_time::<SonyFR7>().expect("FR7 profile")).await;
        let camera = session.camera::<SonyFR7>().expect("camera");

        camera
            .image()
            .enable_horizontal_flip()
            .await
            .expect("mirror on");
        assert_eq!(fake.take_only_payload(), MIRROR_ON);

        camera
            .image()
            .disable_horizontal_flip()
            .await
            .expect("mirror off");
        assert_eq!(fake.take_only_payload(), MIRROR_OFF);

        session.shutdown().expect("shutdown");
    }
}

#[cfg(all(feature = "dyn-api", feature = "runtime-tokio"))]
mod dyn_surface {
    use grafton_visca::{
        command::FlipState,
        dynapi::DynSessionCameraNouns,
        profile::ProfileSpec,
        profiles::{PtzOpticsG2, SonyFR7},
        state_cache::StateEntry,
        StateKey,
    };

    use super::{async_surface::open, MIRROR_OFF, MIRROR_ON};

    #[tokio::test]
    async fn dynamic_noun_clears_the_horizontal_mirror() {
        let (session, fake) =
            open(ProfileSpec::from_compile_time::<SonyFR7>().expect("FR7 profile")).await;
        let camera = session.camera_dyn().expect("dynamic camera");
        let nouns: &dyn DynSessionCameraNouns = &camera;

        nouns
            .image()
            .enable_horizontal_flip()
            .await
            .expect("mirror on");
        assert_eq!(fake.take_only_payload(), MIRROR_ON);

        nouns
            .image()
            .disable_horizontal_flip()
            .await
            .expect("mirror off");
        assert_eq!(fake.take_only_payload(), MIRROR_OFF);

        session.shutdown().expect("shutdown");
    }

    #[tokio::test]
    async fn dynamic_mirror_off_invalidates_the_cached_flip_pair() {
        let (session, fake) =
            open(ProfileSpec::from_compile_time::<PtzOpticsG2>().expect("PTZOptics profile")).await;
        let camera = session.camera_dyn().expect("dynamic camera");
        let nouns: &dyn DynSessionCameraNouns = &camera;

        nouns.image().set_flip_both().await.expect("combined flip");
        let _ = fake.take_only_payload();
        assert_eq!(
            camera.state_cache().flip_state(),
            Some(FlipState {
                horizontal: true,
                vertical: true,
            }),
        );

        nouns
            .image()
            .disable_horizontal_flip()
            .await
            .expect("mirror off");
        assert_eq!(fake.take_only_payload(), MIRROR_OFF);
        assert_eq!(
            camera.state_cache().value(StateKey::Flip),
            StateEntry::Unknown,
        );

        session.shutdown().expect("shutdown");
    }
}
