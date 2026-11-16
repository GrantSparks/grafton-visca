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
    use grafton_visca::wire::WireValue;
    use grafton_visca::{
        blocking::Session,
        capabilities::{HasImage, HasNoiseReduction3D},
        config::SessionConfig,
        profiles::{CompileTimeProfile, ProfileSpec, PtzOptics30x, PtzOpticsG2, PtzOpticsG3},
        types::{
            NoiseReduction2DLevel, NoiseReduction2DMode, NoiseReduction3D, NoiseReduction3DLevel,
        },
        Error,
    };

    use super::{
        assert_raw_writes, noise_reduction_camera, one_raw_frame, NR3D_LEVELS, NR_2D_LEVEL_3,
        NR_2D_OFF, NR_3D_LEVEL_8, NR_3D_OFF, NR_MODE_MANUAL,
    };

    fn round_trip_3d<P>(level: u8) -> Result<NoiseReduction3D, Error>
    where
        P: CompileTimeProfile + HasImage + HasNoiseReduction3D,
    {
        let fake = noise_reduction_camera(0);
        let session = Session::open(
            fake.blocking_wire(),
            &SessionConfig::new(ProfileSpec::from_compile_time::<P>().expect("built-in profile")),
        )
        .expect("session");
        let camera = session
            .camera::<P>(grafton_visca::types::IndividualCameraId::CAMERA_1)
            .expect("matching camera profile");
        let value =
            NoiseReduction3D::from_wire(level).expect("level or Off is in the register domain");
        let set = match value {
            NoiseReduction3D::Off => camera
                .image()
                .set_noise_reduction_3d(grafton_visca::types::NoiseReduction3D::Off),
            NoiseReduction3D::Level(level) => camera
                .image()
                .set_noise_reduction_3d(grafton_visca::types::NoiseReduction3D::Level(level)),
        };
        let result = set.and_then(|_| camera.image().noise_reduction_3d());
        assert_raw_writes(&fake);
        session.shutdown().expect("shutdown");
        result
    }

    #[test]
    fn blocking_nouns_encode_mode_set_and_off_at_their_absolute_frames() {
        let fake = noise_reduction_camera(5);
        let session = Session::open(
            fake.blocking_wire(),
            &SessionConfig::new(ProfileSpec::from_compile_time::<PtzOpticsG2>().expect("G2")),
        )
        .expect("session");
        let camera = session
            .camera::<PtzOpticsG2>(grafton_visca::types::IndividualCameraId::CAMERA_1)
            .expect("G2 camera");

        camera
            .image()
            .set_noise_reduction_2d_mode(NoiseReduction2DMode::Manual)
            .expect("2D manual mode");
        assert_eq!(one_raw_frame(&fake), NR_MODE_MANUAL);
        camera
            .image()
            .set_noise_reduction_2d(grafton_visca::types::NoiseReduction2D::Level(
                NoiseReduction2DLevel::new(3).expect("2D level"),
            ))
            .expect("2D set");
        assert_eq!(one_raw_frame(&fake), NR_2D_LEVEL_3);
        camera
            .image()
            .set_noise_reduction_2d(grafton_visca::types::NoiseReduction2D::Off)
            .expect("2D off");
        assert_eq!(one_raw_frame(&fake), NR_2D_OFF);
        camera
            .image()
            .set_noise_reduction_3d(grafton_visca::types::NoiseReduction3D::Level(
                NoiseReduction3DLevel::MAX,
            ))
            .expect("3D set");
        assert_eq!(one_raw_frame(&fake), NR_3D_LEVEL_8);
        camera
            .image()
            .set_noise_reduction_3d(grafton_visca::types::NoiseReduction3D::Off)
            .expect("3D off");
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
                            scope.spawn(move || round_trip_3d::<PtzOptics30x>(level)),
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
                        NoiseReduction3D::from_wire(level)
                            .expect("level or Off is in the register domain")
                    );
                }
            }
        });
    }
}

#[cfg(all(feature = "async", feature = "runtime-tokio"))]
mod async_surface {
    use grafton_visca::wire::WireValue;
    use grafton_visca::{
        capabilities::{HasImage, HasNoiseReduction3D},
        config::SessionConfig,
        profiles::{CompileTimeProfile, ProfileSpec, PtzOptics30x, PtzOpticsG2, PtzOpticsG3},
        r#async::Session,
        runtime::TokioRuntime,
        transport::AddressingMode,
        types::{
            NoiseReduction2DLevel, NoiseReduction2DMode, NoiseReduction3D, NoiseReduction3DLevel,
        },
        Error,
    };

    use super::{
        assert_raw_writes, noise_reduction_camera, one_raw_frame, FakeCamera, NR3D_LEVELS,
        NR_2D_LEVEL_3, NR_2D_OFF, NR_3D_LEVEL_8, NR_3D_OFF, NR_MODE_MANUAL,
    };

    pub(super) async fn open_with_profile(camera: &FakeCamera, profile: ProfileSpec) -> Session {
        Session::open(
            camera.async_wire().with_addressing(AddressingMode::Ip),
            &SessionConfig::new(profile),
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

    async fn round_trip_3d<P>(level: u8) -> Result<NoiseReduction3D, Error>
    where
        P: CompileTimeProfile + HasImage + HasNoiseReduction3D,
    {
        let fake = noise_reduction_camera(0);
        let session = open_with_profile(
            &fake,
            ProfileSpec::from_compile_time::<P>().expect("built-in profile"),
        )
        .await;
        let camera = session
            .camera::<P>(grafton_visca::types::IndividualCameraId::CAMERA_1)
            .expect("matching camera profile");
        let value =
            NoiseReduction3D::from_wire(level).expect("level or Off is in the register domain");
        let set = match value {
            NoiseReduction3D::Off => {
                camera
                    .image()
                    .set_noise_reduction_3d(grafton_visca::types::NoiseReduction3D::Off)
                    .await
            }
            NoiseReduction3D::Level(level) => {
                camera
                    .image()
                    .set_noise_reduction_3d(grafton_visca::types::NoiseReduction3D::Level(level))
                    .await
            }
        };
        let result = match set {
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
        let camera = session
            .camera::<PtzOpticsG2>(grafton_visca::types::IndividualCameraId::CAMERA_1)
            .expect("G2 camera");

        camera
            .image()
            .set_noise_reduction_2d_mode(NoiseReduction2DMode::Manual)
            .await
            .expect("2D manual mode");
        assert_eq!(one_raw_frame(&fake), NR_MODE_MANUAL);
        camera
            .image()
            .set_noise_reduction_2d(grafton_visca::types::NoiseReduction2D::Level(
                NoiseReduction2DLevel::new(3).expect("2D level"),
            ))
            .await
            .expect("2D set");
        assert_eq!(one_raw_frame(&fake), NR_2D_LEVEL_3);
        camera
            .image()
            .set_noise_reduction_2d(grafton_visca::types::NoiseReduction2D::Off)
            .await
            .expect("2D off");
        assert_eq!(one_raw_frame(&fake), NR_2D_OFF);
        camera
            .image()
            .set_noise_reduction_3d(grafton_visca::types::NoiseReduction3D::Level(
                NoiseReduction3DLevel::MAX,
            ))
            .await
            .expect("3D set");
        assert_eq!(one_raw_frame(&fake), NR_3D_LEVEL_8);
        camera
            .image()
            .set_noise_reduction_3d(grafton_visca::types::NoiseReduction3D::Off)
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
                        tokio::spawn(round_trip_3d::<PtzOptics30x>(level)),
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
                    NoiseReduction3D::from_wire(level)
                        .expect("level or Off is in the register domain")
                );
            }
        }
    }
}

#[cfg(all(
    feature = "async",
    any(feature = "async", feature = "blocking"),
    feature = "runtime-tokio"
))]
mod runtime_profile_surface {
    use grafton_visca::{
        capabilities::TypedSupportSurface,
        profiles::{ProfileSpec, PtzOpticsG2},
        types::{
            NoiseReduction2DLevel, NoiseReduction2DMode, NoiseReduction3D, NoiseReduction3DLevel,
        },
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
        let mut capabilities = source.capabilities().clone();
        capabilities.profile_id = None;
        capabilities.model_name = "NR runtime profile".into();
        capabilities.typed_support = transform_typed_support(capabilities.typed_support);

        crate::common::profile_encodings::with_source_encodings(
            ProfileSpec::builder(capabilities)
                .transports(source.transports())
                .envelope(source.envelope())
                .timing(source.timing())
                .maximum_command_sockets(source.maximum_command_sockets())
                .supports_operation_complete(source.supports_operation_complete())
                .supports_command_cancel(source.supports_command_cancel())
                .position_inquiries(source.position_inquiries()),
            &source,
        )
        .build()
    }

    fn paired_noise_reduction_runtime_profile() -> ProfileSpec {
        runtime_noise_reduction_profile(|typed_support| typed_support)
            .expect("paired NR runtime profile")
    }

    fn runtime_profile_without_noise_reduction_grants() -> grafton_visca::Result<ProfileSpec> {
        runtime_noise_reduction_profile(|typed_support| {
            typed_support
                .without(TypedSupportSurface::NoiseReduction2D)
                .without(TypedSupportSurface::NoiseReduction2DMode)
                .without(TypedSupportSurface::NoiseReduction3D)
        })
    }

    #[tokio::test]
    async fn dynamic_nouns_encode_mode_set_and_off_at_their_absolute_frames() {
        let fake = noise_reduction_camera(5);
        let session = open(&fake).await;
        let camera = session
            .camera::<grafton_visca::profiles::AnyProfile>(
                grafton_visca::types::IndividualCameraId::CAMERA_1,
            )
            .expect("dynamic camera");
        let nouns = &camera;

        nouns
            .image()
            .set_noise_reduction_2d_mode(NoiseReduction2DMode::Manual)
            .await
            .expect("2D manual mode");
        assert_eq!(one_raw_frame(&fake), NR_MODE_MANUAL);
        nouns
            .image()
            .set_noise_reduction_2d(grafton_visca::types::NoiseReduction2D::Level(
                NoiseReduction2DLevel::new(3).expect("2D level"),
            ))
            .await
            .expect("2D set");
        assert_eq!(one_raw_frame(&fake), NR_2D_LEVEL_3);
        nouns
            .image()
            .set_noise_reduction_2d(grafton_visca::types::NoiseReduction2D::Off)
            .await
            .expect("2D off");
        assert_eq!(one_raw_frame(&fake), NR_2D_OFF);
        nouns
            .image()
            .set_noise_reduction_3d(grafton_visca::types::NoiseReduction3D::Level(
                NoiseReduction3DLevel::MAX,
            ))
            .await
            .expect("3D set");
        assert_eq!(one_raw_frame(&fake), NR_3D_LEVEL_8);
        nouns
            .image()
            .set_noise_reduction_3d(grafton_visca::types::NoiseReduction3D::Off)
            .await
            .expect("3D off");
        assert_eq!(one_raw_frame(&fake), NR_3D_OFF);

        session.shutdown().expect("shutdown");
    }

    #[tokio::test]
    async fn unified_noise_reduction_grants_gate_controls_and_inquiries_before_transport_io() {
        let profile = runtime_profile_without_noise_reduction_grants()
            .expect("physical NR facts remain separate from the unified family grant");
        let fake = noise_reduction_camera(5);
        let session = open_with_profile(&fake, profile).await;
        let camera = session
            .camera::<grafton_visca::profiles::AnyProfile>(
                grafton_visca::types::IndividualCameraId::CAMERA_1,
            )
            .expect("dynamic camera");
        let image = camera.image();
        for result in [
            image
                .set_noise_reduction_2d_mode(NoiseReduction2DMode::Manual)
                .await,
            image
                .set_noise_reduction_2d(grafton_visca::types::NoiseReduction2D::Level(
                    NoiseReduction2DLevel::MAX,
                ))
                .await,
            image
                .set_noise_reduction_2d(grafton_visca::types::NoiseReduction2D::Off)
                .await,
            image
                .set_noise_reduction_3d(grafton_visca::types::NoiseReduction3D::Level(
                    NoiseReduction3DLevel::MAX,
                ))
                .await,
            image
                .set_noise_reduction_3d(grafton_visca::types::NoiseReduction3D::Off)
                .await,
        ] {
            assert!(matches!(result, Err(Error::FeatureNotSupported { .. })));
        }
        assert_eq!(
            fake.write_count(),
            0,
            "controls reject before transport I/O"
        );
        let inquiry_error = image
            .noise_reduction_3d()
            .await
            .expect_err("the same NR family grant gates inquiry and control");
        assert!(matches!(
            inquiry_error,
            Error::FeatureNotSupported {
                feature: "inquiry NoiseReduction3dInquiry",
                ..
            }
        ));
        assert_eq!(
            fake.write_count(),
            0,
            "inquiries reject before transport I/O"
        );
        session.shutdown().expect("shutdown");
    }

    #[tokio::test]
    async fn dynamic_runtime_profile_uses_the_same_full_nr3d_reply_domain() {
        let fake = noise_reduction_camera(0x08);
        let session = open_with_profile(&fake, paired_noise_reduction_runtime_profile()).await;
        let camera = session
            .camera::<grafton_visca::profiles::AnyProfile>(
                grafton_visca::types::IndividualCameraId::CAMERA_1,
            )
            .expect("dynamic camera");
        let nouns = &camera;

        let level = nouns
            .image()
            .noise_reduction_3d()
            .await
            .expect("runtime profiles decode the full public 3D NR domain");
        assert_eq!(level, NoiseReduction3D::Level(NoiseReduction3DLevel::MAX));
        assert_raw_writes(&fake);

        session.shutdown().expect("shutdown");
    }
}
