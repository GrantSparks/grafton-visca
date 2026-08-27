//! Issue #651: every noun surface can turn noise reduction off.
//!
//! `set_noise_reduction_2d` / `_3d` shipped without their twins.  The level
//! newtypes are bounded `1..=5` and `1..=8`, so the parameter cannot express
//! off, and the `0x00` wire value is produced only by `NoiseReduction2D::off()`
//! / `NoiseReduction3D::off()` — which no noun method called.  1.x paired each
//! setter with `disable_noise_reduction_2d` / `_3d`.
//!
//! These tests drive the restored disable methods on all three noun surfaces
//! against a scripted transport and assert the absolute VISCA frame, so the
//! `0x00` parameter is pinned rather than inferred.

#![allow(clippy::expect_used, clippy::unwrap_used)]

/// `CAM_NR2D` at level 3 — the frame `set_noise_reduction_2d` writes.
#[allow(dead_code)]
const NR_2D_LEVEL_3: &[u8] = &[0x81, 0x01, 0x04, 0x53, 0x03, 0xFF];

/// `CAM_NR2D Off` — the frame `disable_noise_reduction_2d` must write.
#[allow(dead_code)]
const NR_2D_OFF: &[u8] = &[0x81, 0x01, 0x04, 0x53, 0x00, 0xFF];

/// `CAM_NR3D` at level 5 — the frame `set_noise_reduction_3d` writes.
#[allow(dead_code)]
const NR_3D_LEVEL_5: &[u8] = &[0x81, 0x01, 0x04, 0x54, 0x05, 0xFF];

/// `CAM_NR3D Off` — the frame `disable_noise_reduction_3d` must write.
#[allow(dead_code)]
const NR_3D_OFF: &[u8] = &[0x81, 0x01, 0x04, 0x54, 0x00, 0xFF];

/// Splits a written frame into its optional Sony header and VISCA payload.
#[allow(dead_code)]
fn visca_payload(bytes: &[u8]) -> (Option<u32>, &[u8]) {
    let sony = bytes.len() > 8
        && bytes[0] == 0x01
        && matches!(bytes[1], 0x00 | 0x10 | 0x02 | 0x20)
        && usize::from(u16::from_be_bytes([bytes[2], bytes[3]])) == bytes.len() - 8;
    if sony {
        let sequence = u32::from_be_bytes([bytes[4], bytes[5], bytes[6], bytes[7]]);
        (Some(sequence), &bytes[8..])
    } else {
        (None, bytes)
    }
}

/// Wraps a VISCA reply in the Sony reply envelope when one is in use.
#[allow(dead_code)]
fn envelope(sequence: Option<u32>, payload: Vec<u8>) -> Vec<u8> {
    let Some(sequence) = sequence else {
        return payload;
    };
    let mut frame = Vec::with_capacity(payload.len() + 8);
    frame.extend_from_slice(&[0x01, 0x11]);
    frame.extend_from_slice(
        &u16::try_from(payload.len())
            .expect("reply length")
            .to_be_bytes(),
    );
    frame.extend_from_slice(&sequence.to_be_bytes());
    frame.extend_from_slice(&payload);
    frame
}

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
        command::CommandKind,
        profile::ProfileSpec,
        profiles::SonyFR7,
        transport::{BlockingTransport, HasTransportConfig, SendSemantics, TransportConfig},
        types::{NoiseReduction2DLevel, NoiseReduction3DLevel},
        Error,
    };

    use super::{envelope, visca_payload, NR_2D_LEVEL_3, NR_2D_OFF, NR_3D_LEVEL_5, NR_3D_OFF};

    /// Records every frame and always answers ACK + completion.
    #[derive(Debug)]
    struct RecordingTransport {
        config: TransportConfig,
        responses: VecDeque<Vec<u8>>,
        writes: Arc<Mutex<Vec<Vec<u8>>>>,
    }

    impl RecordingTransport {
        fn new() -> (Self, Arc<Mutex<Vec<Vec<u8>>>>) {
            let writes = Arc::new(Mutex::new(Vec::new()));
            (
                Self {
                    config: TransportConfig::default(),
                    responses: VecDeque::new(),
                    writes: Arc::clone(&writes),
                },
                writes,
            )
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
        fn send_with_kind(&mut self, bytes: &[u8], _kind: CommandKind) -> Result<(), Error> {
            let (sequence, payload) = visca_payload(bytes);
            self.writes
                .lock()
                .expect("writes lock")
                .push(payload.to_vec());
            self.responses
                .push_back(envelope(sequence, vec![0x90, 0x41, 0xFF]));
            self.responses
                .push_back(envelope(sequence, vec![0x90, 0x51, 0xFF]));
            Ok(())
        }

        fn recv_into(&mut self, destination: &mut [u8]) -> Result<usize, Error> {
            self.recv_into_with_timeout(destination, Duration::from_secs(1))
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
        let mut guard = writes.lock().expect("writes lock");
        let mut frames = std::mem::take(&mut *guard);
        assert_eq!(frames.len(), 1, "expected exactly one written frame");
        frames.remove(0)
    }

    fn session(profile: ProfileSpec) -> (Session, Arc<Mutex<Vec<Vec<u8>>>>) {
        let (transport, writes) = RecordingTransport::new();
        let session = Session::open(transport, SessionConfig::new(profile)).expect("session");
        (session, writes)
    }

    #[test]
    fn blocking_noun_turns_noise_reduction_off() {
        let (session, writes) =
            session(ProfileSpec::from_compile_time::<SonyFR7>().expect("FR7 profile"));
        let camera = session.camera::<SonyFR7>().expect("camera");

        camera
            .image()
            .set_noise_reduction_2d(NoiseReduction2DLevel::new(3).expect("2D level"))
            .expect("2D on");
        assert_eq!(one_frame(&writes), NR_2D_LEVEL_3);

        camera.image().disable_noise_reduction_2d().expect("2D off");
        assert_eq!(one_frame(&writes), NR_2D_OFF);

        camera
            .image()
            .set_noise_reduction_3d(NoiseReduction3DLevel::new(5).expect("3D level"))
            .expect("3D on");
        assert_eq!(one_frame(&writes), NR_3D_LEVEL_5);

        camera.image().disable_noise_reduction_3d().expect("3D off");
        assert_eq!(one_frame(&writes), NR_3D_OFF);

        session.shutdown().expect("shutdown");
    }
}

#[cfg(all(feature = "async", feature = "runtime-tokio"))]
mod async_surface {
    use std::{
        future::Future,
        sync::{Arc, Mutex},
    };

    use grafton_visca::{
        profile::ProfileSpec,
        profiles::SonyFR7,
        transport::{
            AddressingMode, AsyncTransport, HasTransportConfig, SendSemantics, TransportConfig,
        },
        types::{NoiseReduction2DLevel, NoiseReduction3DLevel},
        Error, Session, SessionConfig, TokioRuntime,
    };

    use super::{envelope, visca_payload, NR_2D_LEVEL_3, NR_2D_OFF, NR_3D_LEVEL_5, NR_3D_OFF};

    /// Records every frame and always answers ACK + completion.
    #[derive(Debug)]
    pub(super) struct ProbeTransport {
        config: TransportConfig,
        responses: flume::Receiver<Vec<u8>>,
        response_tx: flume::Sender<Vec<u8>>,
        writes: Arc<Mutex<Vec<Vec<u8>>>>,
    }

    impl ProbeTransport {
        pub(super) fn new() -> (Self, Arc<Mutex<Vec<Vec<u8>>>>) {
            let (response_tx, responses) = flume::unbounded();
            let writes = Arc::new(Mutex::new(Vec::new()));
            (
                Self {
                    config: TransportConfig::default(),
                    responses,
                    response_tx,
                    writes: Arc::clone(&writes),
                },
                writes,
            )
        }
    }

    impl HasTransportConfig for ProbeTransport {
        fn transport_config(&self) -> &TransportConfig {
            &self.config
        }
    }

    impl AsyncTransport for ProbeTransport {
        fn send(&mut self, bytes: &[u8]) -> impl Future<Output = Result<(), Error>> + Send {
            let (sequence, payload) = visca_payload(bytes);
            self.writes
                .lock()
                .expect("writes lock")
                .push(payload.to_vec());
            let response_tx = self.response_tx.clone();
            let replies = [
                envelope(sequence, vec![0x90, 0x41, 0xFF]),
                envelope(sequence, vec![0x90, 0x51, 0xFF]),
            ];
            async move {
                for reply in replies {
                    response_tx
                        .send_async(reply)
                        .await
                        .map_err(|_| Error::ConnectionClosed { reason: None })?;
                }
                Ok(())
            }
        }

        #[allow(clippy::manual_async_fn)]
        fn recv_into<'a>(
            &'a mut self,
            destination: &'a mut [u8],
        ) -> impl Future<Output = Result<usize, Error>> + Send {
            async move {
                let response = self
                    .responses
                    .recv_async()
                    .await
                    .map_err(|_| Error::ConnectionClosed { reason: None })?;
                destination[..response.len()].copy_from_slice(&response);
                Ok(response.len())
            }
        }

        fn addressing_mode_hint(&self) -> Option<AddressingMode> {
            Some(AddressingMode::Ip)
        }

        fn send_semantics(&self) -> SendSemantics {
            SendSemantics::Datagram
        }
    }

    pub(super) fn one_frame(writes: &Arc<Mutex<Vec<Vec<u8>>>>) -> Vec<u8> {
        let mut guard = writes.lock().expect("writes lock");
        let mut frames = std::mem::take(&mut *guard);
        assert_eq!(frames.len(), 1, "expected exactly one written frame");
        frames.remove(0)
    }

    pub(super) async fn open(transport: ProbeTransport, profile: ProfileSpec) -> Session {
        Session::open(
            transport,
            SessionConfig::new(profile),
            TokioRuntime::from_current().expect("Tokio runtime"),
        )
        .await
        .expect("session")
    }

    #[tokio::test]
    async fn async_noun_turns_noise_reduction_off() {
        let (transport, writes) = ProbeTransport::new();
        let session = open(
            transport,
            ProfileSpec::from_compile_time::<SonyFR7>().expect("FR7 profile"),
        )
        .await;
        let camera = session.camera::<SonyFR7>().expect("camera");

        camera
            .image()
            .set_noise_reduction_2d(NoiseReduction2DLevel::new(3).expect("2D level"))
            .await
            .expect("2D on");
        assert_eq!(one_frame(&writes), NR_2D_LEVEL_3);

        camera
            .image()
            .disable_noise_reduction_2d()
            .await
            .expect("2D off");
        assert_eq!(one_frame(&writes), NR_2D_OFF);

        camera
            .image()
            .set_noise_reduction_3d(NoiseReduction3DLevel::new(5).expect("3D level"))
            .await
            .expect("3D on");
        assert_eq!(one_frame(&writes), NR_3D_LEVEL_5);

        camera
            .image()
            .disable_noise_reduction_3d()
            .await
            .expect("3D off");
        assert_eq!(one_frame(&writes), NR_3D_OFF);

        session.shutdown().await.expect("shutdown");
    }
}

#[cfg(all(feature = "dyn-api", feature = "runtime-tokio"))]
mod dyn_surface {
    use grafton_visca::{
        dynapi::{DynSessionCamera, DynSessionCameraNouns},
        profile::ProfileSpec,
        profiles::SonyFR7,
        types::{NoiseReduction2DLevel, NoiseReduction3DLevel},
    };

    use super::{
        async_surface::{one_frame, open, ProbeTransport},
        NR_2D_LEVEL_3, NR_2D_OFF, NR_3D_LEVEL_5, NR_3D_OFF,
    };

    #[tokio::test]
    async fn dynamic_noun_turns_noise_reduction_off() {
        let (transport, writes) = ProbeTransport::new();
        let session = open(
            transport,
            ProfileSpec::from_compile_time::<SonyFR7>().expect("FR7 profile"),
        )
        .await;
        let camera = DynSessionCamera::from_session(&session).expect("dynamic camera");
        let nouns: &dyn DynSessionCameraNouns = &camera;

        nouns
            .image()
            .set_noise_reduction_2d(NoiseReduction2DLevel::new(3).expect("2D level"))
            .await
            .expect("2D on");
        assert_eq!(one_frame(&writes), NR_2D_LEVEL_3);

        nouns
            .image()
            .disable_noise_reduction_2d()
            .await
            .expect("2D off");
        assert_eq!(one_frame(&writes), NR_2D_OFF);

        nouns
            .image()
            .set_noise_reduction_3d(NoiseReduction3DLevel::new(5).expect("3D level"))
            .await
            .expect("3D on");
        assert_eq!(one_frame(&writes), NR_3D_LEVEL_5);

        nouns
            .image()
            .disable_noise_reduction_3d()
            .await
            .expect("3D off");
        assert_eq!(one_frame(&writes), NR_3D_OFF);

        session.shutdown().await.expect("shutdown");
    }
}
