//! Issue #651: every generated noun surface exposes the source-backed NR controls.
//!
//! The PTZOptics profiles used here are raw-VISCA profiles: the fake camera
//! recognises requests by their raw bytes, so an enveloped write would fail
//! the scenario rather than pass unnoticed, and each recorded write is
//! asserted to be unenveloped.

#![allow(clippy::expect_used, clippy::unwrap_used)]
#![cfg(any(
    feature = "blocking",
    all(feature = "async", feature = "runtime-tokio")
))]

use grafton_visca_test_support::fake_camera;

use fake_camera::{frames, FakeCamera};

const NR_MODE_MANUAL: &[u8] = &[0x81, 0x01, 0x04, 0x50, 0x03, 0xFF];
const NR_2D_LEVEL_3: &[u8] = &[0x81, 0x01, 0x04, 0x53, 0x03, 0xFF];
const NR_2D_OFF: &[u8] = &[0x81, 0x01, 0x04, 0x53, 0x00, 0xFF];
const NR_3D_LEVEL_8: &[u8] = &[0x81, 0x01, 0x04, 0x54, 0x08, 0xFF];
const NR_3D_OFF: &[u8] = &[0x81, 0x01, 0x04, 0x54, 0x00, 0xFF];

/// A raw-framing camera that acknowledges and completes every command. A 3D
/// noise reduction level command also sets the level its next 3D inquiry
/// reports; `inquiry_level` is the level it reports before any such command.
/// It matches requests on their raw bytes, so only an unenveloped 3D inquiry
/// is recognised.
fn noise_reduction_camera(inquiry_level: u8) -> FakeCamera {
    let mut level = inquiry_level;
    FakeCamera::new(move |bytes, answer| {
        if bytes == [0x81, 0x09, 0x04, 0x54, 0xFF] {
            answer.reply(frames::inquiry_reply(&[level]));
        } else {
            if let [0x81, 0x01, 0x04, 0x54, set_level, 0xFF] = bytes {
                level = *set_level;
            }
            answer.reply(frames::ack(1)).reply(frames::complete(1));
        }
    })
}

/// Every write so far was a raw VISCA frame, not a Sony envelope.
fn assert_raw_writes(fake: &FakeCamera) {
    for write in fake.writes() {
        assert!(
            frames::sony_split(&write).is_none(),
            "a raw profile must write unenveloped frames, got {write:02x?}"
        );
    }
}

/// Every admissible 3D noise reduction level, each read back on each of these
/// PTZOptics profiles.
///
/// Every readback is a raw inquiry that the owner releases only after the
/// profile's raw-inquiry reply-skew hold. Each round trip opens its own
/// session on its own camera, so the round trips are independent and the
/// tests run them concurrently: in sequence they would wait that hold out 27
/// times.
const NR3D_LEVELS: std::ops::RangeInclusive<u8> = 0x00..=0x08;

/// The one frame written since the last call, asserted raw.
fn one_raw_frame(fake: &FakeCamera) -> Vec<u8> {
    assert_eq!(fake.write_count(), 1, "one noun call must write one frame");
    assert_raw_writes(fake);
    fake.take_only_payload()
}

#[cfg(feature = "blocking")]
mod blocking_surface {
    use grafton_visca::{
        blocking::{Session, SessionConfig},
        capabilities::{HasImageProcessing, HasNoiseReduction3D, HasNoiseReduction3DControl},
        command::NoiseReduction2DMode,
        profile::ProfileSpec,
        profiles::{PtzOptics30X, PtzOpticsG2, PtzOpticsG3},
        types::{NoiseReduction2DLevel, NoiseReduction3DLevel},
        CompileTimeProfile, Error,
    };

    use super::{
        assert_raw_writes, noise_reduction_camera, one_raw_frame, NR3D_LEVELS, NR_2D_LEVEL_3,
        NR_2D_OFF, NR_3D_LEVEL_8, NR_3D_OFF, NR_MODE_MANUAL,
    };

    fn round_trip_3d<P>(level: u8) -> Result<NoiseReduction3DLevel, Error>
    where
        P: CompileTimeProfile
            + HasImageProcessing
            + HasNoiseReduction3D
            + HasNoiseReduction3DControl,
    {
        let fake = noise_reduction_camera(0);
        let session = Session::open(
            fake.blocking_wire(),
            SessionConfig::new(ProfileSpec::from_compile_time::<P>().expect("built-in profile")),
        )
        .expect("session");
        let camera = session.camera::<P>().expect("matching camera profile");
        let value = NoiseReduction3DLevel::new(level).expect("level is in the public domain");
        let result = camera
            .image()
            .set_noise_reduction_3d(value)
            .and_then(|_| camera.image().noise_reduction_3d());
        assert_raw_writes(&fake);
        session.shutdown().expect("shutdown");
        result
    }

    #[test]
    fn blocking_nouns_encode_mode_set_and_off_at_their_absolute_frames() {
        let fake = noise_reduction_camera(5);
        let session = Session::open(
            fake.blocking_wire(),
            SessionConfig::new(ProfileSpec::from_compile_time::<PtzOpticsG2>().expect("G2")),
        )
        .expect("session");
        let camera = session.camera::<PtzOpticsG2>().expect("G2 camera");

        camera
            .image()
            .set_noise_reduction_2d_mode(NoiseReduction2DMode::Manual)
            .expect("2D manual mode");
        assert_eq!(one_raw_frame(&fake), NR_MODE_MANUAL);
        camera
            .image()
            .set_noise_reduction_2d(NoiseReduction2DLevel::new(3).expect("2D level"))
            .expect("2D set");
        assert_eq!(one_raw_frame(&fake), NR_2D_LEVEL_3);
        camera.image().disable_noise_reduction_2d().expect("2D off");
        assert_eq!(one_raw_frame(&fake), NR_2D_OFF);
        camera
            .image()
            .set_noise_reduction_3d(NoiseReduction3DLevel::MAX)
            .expect("3D set");
        assert_eq!(one_raw_frame(&fake), NR_3D_LEVEL_8);
        camera.image().disable_noise_reduction_3d().expect("3D off");
        assert_eq!(one_raw_frame(&fake), NR_3D_OFF);

        session.shutdown().expect("shutdown");
    }

    #[test]
    fn blocking_session_decodes_every_admissible_nr3d_readback() {
        std::thread::scope(|scope| {
            let round_trips: Vec<_> = NR3D_LEVELS
                .map(|level| {
                    (
                        level,
                        [
                            scope.spawn(move || round_trip_3d::<PtzOpticsG2>(level)),
                            scope.spawn(move || round_trip_3d::<PtzOpticsG3>(level)),
                            scope.spawn(move || round_trip_3d::<PtzOptics30X>(level)),
                        ],
                    )
                })
                .collect();
            for (level, profiles) in round_trips {
                for round_trip in profiles {
                    let result = round_trip
                        .join()
                        .unwrap_or_else(|panic| std::panic::resume_unwind(panic));
                    assert_eq!(
                        result.expect("every admissible 3D NR setter value decodes on readback"),
                        NoiseReduction3DLevel::new(level).expect("level is in the public domain")
                    );
                }
            }
        });
    }
}

#[cfg(all(feature = "async", feature = "runtime-tokio"))]
mod async_surface {
    use grafton_visca::{
        capabilities::{HasImageProcessing, HasNoiseReduction3D, HasNoiseReduction3DControl},
        command::NoiseReduction2DMode,
        profile::ProfileSpec,
        profiles::{PtzOptics30X, PtzOpticsG2, PtzOpticsG3},
        transport::AddressingMode,
        types::{NoiseReduction2DLevel, NoiseReduction3DLevel},
        CompileTimeProfile, Error, Session, SessionConfig, TokioRuntime,
    };

    use super::{
        assert_raw_writes, noise_reduction_camera, one_raw_frame, FakeCamera, NR3D_LEVELS,
        NR_2D_LEVEL_3, NR_2D_OFF, NR_3D_LEVEL_8, NR_3D_OFF, NR_MODE_MANUAL,
    };

    pub(super) async fn open_with_profile(camera: &FakeCamera, profile: ProfileSpec) -> Session {
        Session::open(
            camera.async_wire().with_addressing(AddressingMode::Ip),
            SessionConfig::new(profile),
            TokioRuntime::from_current().expect("Tokio runtime"),
        )
        .await
        .expect("session")
    }

    pub(super) async fn open(camera: &FakeCamera) -> Session {
        open_with_profile(
            camera,
            ProfileSpec::from_compile_time::<PtzOpticsG2>().expect("G2"),
        )
        .await
    }

    async fn round_trip_3d<P>(level: u8) -> Result<NoiseReduction3DLevel, Error>
    where
        P: CompileTimeProfile
            + HasImageProcessing
            + HasNoiseReduction3D
            + HasNoiseReduction3DControl,
    {
        let fake = noise_reduction_camera(0);
        let session = open_with_profile(
            &fake,
            ProfileSpec::from_compile_time::<P>().expect("built-in profile"),
        )
        .await;
        let camera = session.camera::<P>().expect("matching camera profile");
        let value = NoiseReduction3DLevel::new(level).expect("level is in the public domain");
        let result = match camera.image().set_noise_reduction_3d(value).await {
            Ok(_) => camera.image().noise_reduction_3d().await,
            Err(error) => Err(error),
        };
        assert_raw_writes(&fake);
        session.shutdown().expect("shutdown");
        result
    }

    #[tokio::test]
    async fn async_nouns_encode_mode_set_and_off_at_their_absolute_frames() {
        let fake = noise_reduction_camera(5);
        let session = open(&fake).await;
        let camera = session.camera::<PtzOpticsG2>().expect("G2 camera");

        camera
            .image()
            .set_noise_reduction_2d_mode(NoiseReduction2DMode::Manual)
            .await
            .expect("2D manual mode");
        assert_eq!(one_raw_frame(&fake), NR_MODE_MANUAL);
        camera
            .image()
            .set_noise_reduction_2d(NoiseReduction2DLevel::new(3).expect("2D level"))
            .await
            .expect("2D set");
        assert_eq!(one_raw_frame(&fake), NR_2D_LEVEL_3);
        camera
            .image()
            .disable_noise_reduction_2d()
            .await
            .expect("2D off");
        assert_eq!(one_raw_frame(&fake), NR_2D_OFF);
        camera
            .image()
            .set_noise_reduction_3d(NoiseReduction3DLevel::MAX)
            .await
            .expect("3D set");
        assert_eq!(one_raw_frame(&fake), NR_3D_LEVEL_8);
        camera
            .image()
            .disable_noise_reduction_3d()
            .await
            .expect("3D off");
        assert_eq!(one_raw_frame(&fake), NR_3D_OFF);

        session.shutdown().expect("shutdown");
    }

    #[tokio::test]
    async fn async_session_decodes_every_admissible_nr3d_readback() {
        let round_trips: Vec<_> = NR3D_LEVELS
            .map(|level| {
                (
                    level,
                    [
                        tokio::spawn(round_trip_3d::<PtzOpticsG2>(level)),
                        tokio::spawn(round_trip_3d::<PtzOpticsG3>(level)),
                        tokio::spawn(round_trip_3d::<PtzOptics30X>(level)),
                    ],
                )
            })
            .collect();
        for (level, profiles) in round_trips {
            for round_trip in profiles {
                let result = round_trip
                    .await
                    .unwrap_or_else(|failure| std::panic::resume_unwind(failure.into_panic()));
                assert_eq!(
                    result.expect("every admissible 3D NR setter value decodes on readback"),
                    NoiseReduction3DLevel::new(level).expect("level is in the public domain")
                );
            }
        }
    }
}

#[cfg(all(feature = "async", feature = "dyn-api", feature = "runtime-tokio"))]
mod dyn_surface {
    use grafton_visca::{
        capabilities::TypedSupportSurface,
        command::NoiseReduction2DMode,
        dynapi::DynSessionCameraNouns,
        profile::ProfileSpec,
        profiles::PtzOpticsG2,
        types::{NoiseReduction2DLevel, NoiseReduction3DLevel},
        Error,
    };

    use super::{
        assert_raw_writes,
        async_surface::{open, open_with_profile},
        noise_reduction_camera, one_raw_frame, NR_2D_LEVEL_3, NR_2D_OFF, NR_3D_LEVEL_8, NR_3D_OFF,
        NR_MODE_MANUAL,
    };

    fn runtime_noise_reduction_profile(
        transform_typed_support: impl FnOnce(
            grafton_visca::capabilities::TypedSupportSet,
        ) -> grafton_visca::capabilities::TypedSupportSet,
    ) -> grafton_visca::Result<ProfileSpec> {
        let source = ProfileSpec::from_compile_time::<PtzOpticsG2>().expect("G2 profile");
        let coordinates = source
            .pan_tilt_coordinates()
            .expect("G2 pan/tilt conversion");
        let mut capabilities = source.capabilities().clone();
        capabilities.profile_id = None;
        capabilities.model_name = "NR runtime profile".into();
        capabilities.typed_support = transform_typed_support(capabilities.typed_support);

        ProfileSpec::builder(capabilities)
            .pan_tilt_coordinates(
                coordinates.coordinate_system(),
                coordinates.pan_degrees_to_units(),
                coordinates.tilt_degrees_to_units(),
            )
            .pan_tilt_wire_codec(coordinates.wire_codec())
            .transports(source.transports())
            .envelope(source.envelope())
            .timing(source.timing())
            .maximum_command_sockets(source.maximum_command_sockets())
            .supports_operation_complete(source.supports_operation_complete())
            .supports_command_cancel(source.supports_command_cancel())
            .preset_recall_axes(source.preset_recall_axes())
            .position_inquiries(source.position_inquiries())
            .build()
    }

    fn paired_noise_reduction_runtime_profile() -> ProfileSpec {
        runtime_noise_reduction_profile(|typed_support| typed_support)
            .expect("paired NR runtime profile")
    }

    fn inquiry_only_noise_reduction_profile() -> grafton_visca::Result<ProfileSpec> {
        runtime_noise_reduction_profile(|typed_support| {
            typed_support
                .without(TypedSupportSurface::NoiseReduction2DControl)
                .without(TypedSupportSurface::NoiseReduction3DControl)
        })
    }

    #[tokio::test]
    async fn dynamic_nouns_encode_mode_set_and_off_at_their_absolute_frames() {
        let fake = noise_reduction_camera(5);
        let session = open(&fake).await;
        let camera = session.camera_dyn().expect("dynamic camera");
        let nouns: &dyn DynSessionCameraNouns = &camera;

        nouns
            .image()
            .set_noise_reduction_2d_mode(NoiseReduction2DMode::Manual)
            .await
            .expect("2D manual mode");
        assert_eq!(one_raw_frame(&fake), NR_MODE_MANUAL);
        nouns
            .image()
            .set_noise_reduction_2d(NoiseReduction2DLevel::new(3).expect("2D level"))
            .await
            .expect("2D set");
        assert_eq!(one_raw_frame(&fake), NR_2D_LEVEL_3);
        nouns
            .image()
            .disable_noise_reduction_2d()
            .await
            .expect("2D off");
        assert_eq!(one_raw_frame(&fake), NR_2D_OFF);
        nouns
            .image()
            .set_noise_reduction_3d(NoiseReduction3DLevel::MAX)
            .await
            .expect("3D set");
        assert_eq!(one_raw_frame(&fake), NR_3D_LEVEL_8);
        nouns
            .image()
            .disable_noise_reduction_3d()
            .await
            .expect("3D off");
        assert_eq!(one_raw_frame(&fake), NR_3D_OFF);

        session.shutdown().expect("shutdown");
    }

    #[test]
    fn inquiry_only_runtime_profile_is_rejected_before_transport_construction() {
        let error = inquiry_only_noise_reduction_profile()
            .expect_err("inquiry-only NR profile violates the paired-surface invariant");
        assert!(matches!(error, Error::InvalidRequest(message)
            if message.contains("noise-reduction metadata and paired inquiry/control typed support must agree")));
    }

    #[tokio::test]
    async fn dynamic_runtime_profile_uses_the_same_full_nr3d_reply_domain() {
        let fake = noise_reduction_camera(0x08);
        let session = open_with_profile(&fake, paired_noise_reduction_runtime_profile()).await;
        let camera = session.camera_dyn().expect("dynamic camera");
        let nouns: &dyn DynSessionCameraNouns = &camera;

        let level = nouns
            .image()
            .noise_reduction_3d()
            .await
            .expect("runtime profiles decode the full public 3D NR domain");
        assert_eq!(level, NoiseReduction3DLevel::MAX);
        assert_raw_writes(&fake);

        session.shutdown().expect("shutdown");
    }
}
