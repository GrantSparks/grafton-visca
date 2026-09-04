//! Issue #651: every generated noun surface exposes the source-backed NR controls.

#![allow(clippy::expect_used, clippy::unwrap_used)]

#[cfg(any(
    feature = "blocking",
    all(feature = "async", feature = "runtime-tokio")
))]
const NR_MODE_MANUAL: &[u8] = &[0x81, 0x01, 0x04, 0x50, 0x03, 0xFF];
#[cfg(any(
    feature = "blocking",
    all(feature = "async", feature = "runtime-tokio")
))]
const NR_2D_LEVEL_3: &[u8] = &[0x81, 0x01, 0x04, 0x53, 0x03, 0xFF];
#[cfg(any(
    feature = "blocking",
    all(feature = "async", feature = "runtime-tokio")
))]
const NR_2D_OFF: &[u8] = &[0x81, 0x01, 0x04, 0x53, 0x00, 0xFF];
#[cfg(any(
    feature = "blocking",
    all(feature = "async", feature = "runtime-tokio")
))]
const NR_3D_LEVEL_8: &[u8] = &[0x81, 0x01, 0x04, 0x54, 0x08, 0xFF];
#[cfg(any(
    feature = "blocking",
    all(feature = "async", feature = "runtime-tokio")
))]
const NR_3D_OFF: &[u8] = &[0x81, 0x01, 0x04, 0x54, 0x00, 0xFF];

#[cfg(feature = "blocking")]
mod blocking_surface {
    use std::{
        collections::VecDeque,
        sync::{Arc, Mutex},
        time::Duration,
    };

    use grafton_visca::{
        blocking::{Session, SessionConfig},
        camera::TransportKind,
        capabilities::{HasImageProcessing, HasNoiseReduction3D, HasNoiseReduction3DControl},
        command::{CommandKind, NoiseReduction2DMode},
        profile::ProfileSpec,
        profiles::{PtzOptics30X, PtzOpticsG2, PtzOpticsG3},
        transport::{BlockingTransport, HasTransportConfig, SendSemantics, TransportConfig},
        types::{NoiseReduction2DLevel, NoiseReduction3DLevel},
        CompileTimeProfile, Error,
    };

    use super::{NR_2D_LEVEL_3, NR_2D_OFF, NR_3D_LEVEL_8, NR_3D_OFF, NR_MODE_MANUAL};

    #[derive(Debug)]
    struct RecordingTransport {
        config: TransportConfig,
        responses: VecDeque<Vec<u8>>,
        writes: Arc<Mutex<Vec<Vec<u8>>>>,
        inquiry_level: u8,
    }

    impl RecordingTransport {
        fn new() -> (Self, Arc<Mutex<Vec<Vec<u8>>>>) {
            let writes = Arc::new(Mutex::new(Vec::new()));
            (
                Self {
                    config: TransportConfig::default(),
                    responses: VecDeque::new(),
                    writes: Arc::clone(&writes),
                    inquiry_level: 5,
                },
                writes,
            )
        }

        fn with_inquiry_level(inquiry_level: u8) -> Self {
            let (mut transport, _) = Self::new();
            transport.inquiry_level = inquiry_level;
            transport
        }
    }

    impl HasTransportConfig for RecordingTransport {
        fn transport_config(&self) -> &TransportConfig {
            &self.config
        }

        fn standard_transport_kind(&self) -> Option<TransportKind> {
            None
        }
    }

    impl BlockingTransport for RecordingTransport {
        fn send_with_timeout(
            &mut self,
            bytes: &[u8],
            _kind: CommandKind,
            _timeout: Duration,
        ) -> Result<(), Error> {
            self.writes
                .lock()
                .expect("writes lock")
                .push(bytes.to_vec());
            if bytes == [0x81, 0x09, 0x04, 0x54, 0xFF] {
                self.responses
                    .push_back(vec![0x90, 0x50, self.inquiry_level, 0xFF]);
            } else {
                if let [0x81, 0x01, 0x04, 0x54, level, 0xFF] = bytes {
                    self.inquiry_level = *level;
                }
                self.responses.push_back(vec![0x90, 0x41, 0xFF]);
                self.responses.push_back(vec![0x90, 0x51, 0xFF]);
            }
            Ok(())
        }

        fn recv_into_with_timeout(
            &mut self,
            destination: &mut [u8],
            _timeout: Duration,
        ) -> Result<usize, Error> {
            let response = self.responses.pop_front().ok_or(Error::Timeout)?;
            destination[..response.len()].copy_from_slice(&response);
            Ok(response.len())
        }

        fn send_semantics(&self) -> SendSemantics {
            SendSemantics::Datagram
        }
    }

    fn one_frame(writes: &Arc<Mutex<Vec<Vec<u8>>>>) -> Vec<u8> {
        let mut writes = writes.lock().expect("writes lock");
        assert_eq!(writes.len(), 1, "one noun call must write one frame");
        writes.remove(0)
    }

    fn round_trip_3d<P>(level: u8) -> Result<NoiseReduction3DLevel, Error>
    where
        P: CompileTimeProfile
            + HasImageProcessing
            + HasNoiseReduction3D
            + HasNoiseReduction3DControl,
    {
        let session = Session::open(
            RecordingTransport::with_inquiry_level(0),
            SessionConfig::new(ProfileSpec::from_compile_time::<P>().expect("built-in profile")),
        )
        .expect("session");
        let camera = session.camera::<P>().expect("matching camera profile");
        let value = NoiseReduction3DLevel::new(level).expect("level is in the public domain");
        let result = camera
            .image()
            .set_noise_reduction_3d(value)
            .and_then(|_| camera.image().noise_reduction_3d());
        session.shutdown().expect("shutdown");
        result
    }

    #[test]
    fn blocking_nouns_encode_mode_set_and_off_at_their_absolute_frames() {
        let (transport, writes) = RecordingTransport::new();
        let session = Session::open(
            transport,
            SessionConfig::new(ProfileSpec::from_compile_time::<PtzOpticsG2>().expect("G2")),
        )
        .expect("session");
        let camera = session.camera::<PtzOpticsG2>().expect("G2 camera");

        camera
            .image()
            .set_noise_reduction_2d_mode(NoiseReduction2DMode::Manual)
            .expect("2D manual mode");
        assert_eq!(one_frame(&writes), NR_MODE_MANUAL);
        camera
            .image()
            .set_noise_reduction_2d(NoiseReduction2DLevel::new(3).expect("2D level"))
            .expect("2D set");
        assert_eq!(one_frame(&writes), NR_2D_LEVEL_3);
        camera.image().disable_noise_reduction_2d().expect("2D off");
        assert_eq!(one_frame(&writes), NR_2D_OFF);
        camera
            .image()
            .set_noise_reduction_3d(NoiseReduction3DLevel::MAX)
            .expect("3D set");
        assert_eq!(one_frame(&writes), NR_3D_LEVEL_8);
        camera.image().disable_noise_reduction_3d().expect("3D off");
        assert_eq!(one_frame(&writes), NR_3D_OFF);

        session.shutdown().expect("shutdown");
    }

    #[test]
    fn blocking_session_decodes_every_admissible_nr3d_readback() {
        for level in 0x00..=0x08 {
            for result in [
                round_trip_3d::<PtzOpticsG2>(level),
                round_trip_3d::<PtzOpticsG3>(level),
                round_trip_3d::<PtzOptics30X>(level),
            ] {
                assert_eq!(
                    result.expect("every admissible 3D NR setter value decodes on readback"),
                    NoiseReduction3DLevel::new(level).expect("level is in the public domain")
                );
            }
        }
    }
}

#[cfg(all(feature = "async", feature = "runtime-tokio"))]
mod async_surface {
    use std::{
        future::Future,
        sync::{Arc, Mutex},
    };

    use grafton_visca::{
        capabilities::{HasImageProcessing, HasNoiseReduction3D, HasNoiseReduction3DControl},
        command::NoiseReduction2DMode,
        profile::ProfileSpec,
        profiles::{PtzOptics30X, PtzOpticsG2, PtzOpticsG3},
        transport::{
            AddressingMode, AsyncTransport, HasTransportConfig, SendSemantics, TransportConfig,
        },
        types::{NoiseReduction2DLevel, NoiseReduction3DLevel},
        CompileTimeProfile, Error, Session, SessionConfig, TokioRuntime,
    };

    use super::{NR_2D_LEVEL_3, NR_2D_OFF, NR_3D_LEVEL_8, NR_3D_OFF, NR_MODE_MANUAL};

    #[derive(Debug)]
    pub(super) struct RecordingTransport {
        config: TransportConfig,
        replies: flume::Receiver<Vec<u8>>,
        reply_tx: flume::Sender<Vec<u8>>,
        writes: Arc<Mutex<Vec<Vec<u8>>>>,
        inquiry_level: u8,
    }

    impl RecordingTransport {
        pub(super) fn new() -> (Self, Arc<Mutex<Vec<Vec<u8>>>>) {
            let (reply_tx, replies) = flume::unbounded();
            let writes = Arc::new(Mutex::new(Vec::new()));
            (
                Self {
                    config: TransportConfig::default(),
                    replies,
                    reply_tx,
                    writes: Arc::clone(&writes),
                    inquiry_level: 5,
                },
                writes,
            )
        }

        pub(super) fn with_inquiry_level(inquiry_level: u8) -> Self {
            let (mut transport, _) = Self::new();
            transport.inquiry_level = inquiry_level;
            transport
        }
    }

    impl HasTransportConfig for RecordingTransport {
        fn transport_config(&self) -> &TransportConfig {
            &self.config
        }
    }

    impl AsyncTransport for RecordingTransport {
        fn send(&mut self, bytes: &[u8]) -> impl Future<Output = Result<(), Error>> + Send {
            self.writes
                .lock()
                .expect("writes lock")
                .push(bytes.to_vec());
            if let [0x81, 0x01, 0x04, 0x54, level, 0xFF] = bytes {
                self.inquiry_level = *level;
            }
            let reply_tx = self.reply_tx.clone();
            let inquiry_level = self.inquiry_level;
            let is_nr3d_inquiry = bytes == [0x81, 0x09, 0x04, 0x54, 0xFF];
            async move {
                if is_nr3d_inquiry {
                    return reply_tx
                        .send_async(vec![0x90, 0x50, inquiry_level, 0xFF])
                        .await
                        .map_err(|_| Error::ConnectionClosed { reason: None });
                }
                reply_tx
                    .send_async(vec![0x90, 0x41, 0xFF])
                    .await
                    .map_err(|_| Error::ConnectionClosed { reason: None })?;
                reply_tx
                    .send_async(vec![0x90, 0x51, 0xFF])
                    .await
                    .map_err(|_| Error::ConnectionClosed { reason: None })
            }
        }

        async fn recv_into(&mut self, destination: &mut [u8]) -> Result<usize, Error> {
            let reply = self
                .replies
                .recv_async()
                .await
                .map_err(|_| Error::ConnectionClosed { reason: None })?;
            destination[..reply.len()].copy_from_slice(&reply);
            Ok(reply.len())
        }

        fn addressing_mode_hint(&self) -> Option<AddressingMode> {
            Some(AddressingMode::Ip)
        }

        fn send_semantics(&self) -> SendSemantics {
            SendSemantics::Datagram
        }
    }

    pub(super) async fn open_with_profile(
        transport: RecordingTransport,
        profile: ProfileSpec,
    ) -> Session {
        Session::open(
            transport,
            SessionConfig::new(profile),
            TokioRuntime::from_current().expect("Tokio runtime"),
        )
        .await
        .expect("session")
    }

    pub(super) async fn open(transport: RecordingTransport) -> Session {
        open_with_profile(
            transport,
            ProfileSpec::from_compile_time::<PtzOpticsG2>().expect("G2"),
        )
        .await
    }

    pub(super) fn one_frame(writes: &Arc<Mutex<Vec<Vec<u8>>>>) -> Vec<u8> {
        let mut writes = writes.lock().expect("writes lock");
        assert_eq!(writes.len(), 1, "one noun call must write one frame");
        writes.remove(0)
    }

    async fn round_trip_3d<P>(level: u8) -> Result<NoiseReduction3DLevel, Error>
    where
        P: CompileTimeProfile
            + HasImageProcessing
            + HasNoiseReduction3D
            + HasNoiseReduction3DControl,
    {
        let session = open_with_profile(
            RecordingTransport::with_inquiry_level(0),
            ProfileSpec::from_compile_time::<P>().expect("built-in profile"),
        )
        .await;
        let camera = session.camera::<P>().expect("matching camera profile");
        let value = NoiseReduction3DLevel::new(level).expect("level is in the public domain");
        let result = match camera.image().set_noise_reduction_3d(value).await {
            Ok(_) => camera.image().noise_reduction_3d().await,
            Err(error) => Err(error),
        };
        session.shutdown().await.expect("shutdown");
        result
    }

    #[tokio::test]
    async fn async_nouns_encode_mode_set_and_off_at_their_absolute_frames() {
        let (transport, writes) = RecordingTransport::new();
        let session = open(transport).await;
        let camera = session.camera::<PtzOpticsG2>().expect("G2 camera");

        camera
            .image()
            .set_noise_reduction_2d_mode(NoiseReduction2DMode::Manual)
            .await
            .expect("2D manual mode");
        assert_eq!(one_frame(&writes), NR_MODE_MANUAL);
        camera
            .image()
            .set_noise_reduction_2d(NoiseReduction2DLevel::new(3).expect("2D level"))
            .await
            .expect("2D set");
        assert_eq!(one_frame(&writes), NR_2D_LEVEL_3);
        camera
            .image()
            .disable_noise_reduction_2d()
            .await
            .expect("2D off");
        assert_eq!(one_frame(&writes), NR_2D_OFF);
        camera
            .image()
            .set_noise_reduction_3d(NoiseReduction3DLevel::MAX)
            .await
            .expect("3D set");
        assert_eq!(one_frame(&writes), NR_3D_LEVEL_8);
        camera
            .image()
            .disable_noise_reduction_3d()
            .await
            .expect("3D off");
        assert_eq!(one_frame(&writes), NR_3D_OFF);

        session.shutdown().await.expect("shutdown");
    }

    #[tokio::test]
    async fn async_session_decodes_every_admissible_nr3d_readback() {
        for level in 0x00..=0x08 {
            for result in [
                round_trip_3d::<PtzOpticsG2>(level).await,
                round_trip_3d::<PtzOpticsG3>(level).await,
                round_trip_3d::<PtzOptics30X>(level).await,
            ] {
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
        dynapi::{DynSessionCamera, DynSessionCameraNouns},
        profile::ProfileSpec,
        profiles::PtzOpticsG2,
        types::{NoiseReduction2DLevel, NoiseReduction3DLevel},
        Error,
    };

    use super::{
        async_surface::{one_frame, open, open_with_profile, RecordingTransport},
        NR_2D_LEVEL_3, NR_2D_OFF, NR_3D_LEVEL_8, NR_3D_OFF, NR_MODE_MANUAL,
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
        let (transport, writes) = RecordingTransport::new();
        let session = open(transport).await;
        let camera = DynSessionCamera::from_session(&session).expect("dynamic camera");
        let nouns: &dyn DynSessionCameraNouns = &camera;

        nouns
            .image()
            .set_noise_reduction_2d_mode(NoiseReduction2DMode::Manual)
            .await
            .expect("2D manual mode");
        assert_eq!(one_frame(&writes), NR_MODE_MANUAL);
        nouns
            .image()
            .set_noise_reduction_2d(NoiseReduction2DLevel::new(3).expect("2D level"))
            .await
            .expect("2D set");
        assert_eq!(one_frame(&writes), NR_2D_LEVEL_3);
        nouns
            .image()
            .disable_noise_reduction_2d()
            .await
            .expect("2D off");
        assert_eq!(one_frame(&writes), NR_2D_OFF);
        nouns
            .image()
            .set_noise_reduction_3d(NoiseReduction3DLevel::MAX)
            .await
            .expect("3D set");
        assert_eq!(one_frame(&writes), NR_3D_LEVEL_8);
        nouns
            .image()
            .disable_noise_reduction_3d()
            .await
            .expect("3D off");
        assert_eq!(one_frame(&writes), NR_3D_OFF);

        session.shutdown().await.expect("shutdown");
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
        let session = open_with_profile(
            RecordingTransport::with_inquiry_level(0x08),
            paired_noise_reduction_runtime_profile(),
        )
        .await;
        let camera = DynSessionCamera::from_session(&session).expect("dynamic camera");
        let nouns: &dyn DynSessionCameraNouns = &camera;

        let level = nouns
            .image()
            .noise_reduction_3d()
            .await
            .expect("runtime profiles decode the full public 3D NR domain");
        assert_eq!(level, NoiseReduction3DLevel::MAX);

        session.shutdown().await.expect("shutdown");
    }
}
