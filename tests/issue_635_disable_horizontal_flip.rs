//! Issue #635: every noun surface can clear the horizontal mirror.
//!
//! `enable_horizontal_flip` shipped without its twin, so on a profile that has
//! `HasImageMirror` but not `HasCombinedImageFlip` — SonyFR7 here — no noun
//! method could return the mirror to off.  These tests drive the restored
//! `disable_horizontal_flip` on all three noun surfaces against a scripted
//! transport and assert the absolute VISCA frame, then pin the state-cache
//! consequence of the mirror opcode on the noun path.

#![allow(clippy::expect_used, clippy::unwrap_used)]

/// `CAM_LR_Reverse On` — the frame `enable_horizontal_flip` writes.
#[cfg(any(
    feature = "blocking",
    all(feature = "async", feature = "runtime-tokio")
))]
const MIRROR_ON: &[u8] = &[0x81, 0x01, 0x04, 0x61, 0x02, 0xFF];

/// `CAM_LR_Reverse Off` — the frame `disable_horizontal_flip` must write.
#[cfg(any(
    feature = "blocking",
    all(feature = "async", feature = "runtime-tokio")
))]
const MIRROR_OFF: &[u8] = &[0x81, 0x01, 0x04, 0x61, 0x03, 0xFF];

/// Splits a written frame into its optional Sony header and VISCA payload.
#[cfg(any(
    feature = "blocking",
    all(feature = "async", feature = "runtime-tokio")
))]
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
#[cfg(any(
    feature = "blocking",
    all(feature = "async", feature = "runtime-tokio")
))]
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
        command::{CommandKind, FlipState, ImageFlipMode},
        profile::ProfileSpec,
        profiles::{PtzOpticsG2, SonyFR7},
        state_cache::StateEntry,
        transport::{BlockingTransport, HasTransportConfig, SendSemantics, TransportConfig},
        Error, StateKey,
    };

    use super::{envelope, visca_payload, MIRROR_OFF, MIRROR_ON};

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
        fn send_with_timeout(
            &mut self,
            bytes: &[u8],
            _kind: CommandKind,
            _timeout: Duration,
        ) -> Result<(), Error> {
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
    fn blocking_noun_clears_the_horizontal_mirror() {
        let (session, writes) =
            session(ProfileSpec::from_compile_time::<SonyFR7>().expect("FR7 profile"));
        let camera = session.camera::<SonyFR7>().expect("camera");

        camera.image().enable_horizontal_flip().expect("mirror on");
        assert_eq!(one_frame(&writes), MIRROR_ON);

        camera
            .image()
            .disable_horizontal_flip()
            .expect("mirror off");
        assert_eq!(one_frame(&writes), MIRROR_OFF);

        session.shutdown().expect("shutdown");
    }

    #[test]
    fn blocking_mirror_off_invalidates_the_cached_flip_pair() {
        let (session, writes) =
            session(ProfileSpec::from_compile_time::<PtzOpticsG2>().expect("PTZOptics profile"));
        let camera = session.camera::<PtzOpticsG2>().expect("camera");

        // Only the combined opcode establishes both axes, so start from a
        // known pair.
        camera
            .image()
            .set_flip_mode(ImageFlipMode::Both)
            .expect("combined flip");
        let _ = one_frame(&writes);
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
        assert_eq!(one_frame(&writes), MIRROR_OFF);
        assert_eq!(
            camera.state_cache().value(StateKey::Flip),
            StateEntry::Unknown,
        );
        assert_eq!(camera.state_cache().flip_state(), None);

        camera
            .image()
            .set_flip_mode(ImageFlipMode::Both)
            .expect("combined flip");
        let _ = one_frame(&writes);
        camera.image().enable_horizontal_flip().expect("mirror on");
        assert_eq!(one_frame(&writes), MIRROR_ON);
        assert_eq!(
            camera.state_cache().value(StateKey::Flip),
            StateEntry::Unknown,
        );

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
        Error, Session, SessionConfig, TokioRuntime,
    };

    use super::{envelope, visca_payload, MIRROR_OFF, MIRROR_ON};

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
    async fn async_noun_clears_the_horizontal_mirror() {
        let (transport, writes) = ProbeTransport::new();
        let session = open(
            transport,
            ProfileSpec::from_compile_time::<SonyFR7>().expect("FR7 profile"),
        )
        .await;
        let camera = session.camera::<SonyFR7>().expect("camera");

        camera
            .image()
            .enable_horizontal_flip()
            .await
            .expect("mirror on");
        assert_eq!(one_frame(&writes), MIRROR_ON);

        camera
            .image()
            .disable_horizontal_flip()
            .await
            .expect("mirror off");
        assert_eq!(one_frame(&writes), MIRROR_OFF);

        session.shutdown().await.expect("shutdown");
    }
}

#[cfg(all(feature = "dyn-api", feature = "runtime-tokio"))]
mod dyn_surface {
    use grafton_visca::{
        command::FlipState,
        dynapi::{DynSessionCamera, DynSessionCameraNouns},
        profile::ProfileSpec,
        profiles::{PtzOpticsG2, SonyFR7},
        state_cache::StateEntry,
        StateKey,
    };

    use super::{
        async_surface::{one_frame, open, ProbeTransport},
        MIRROR_OFF, MIRROR_ON,
    };

    #[tokio::test]
    async fn dynamic_noun_clears_the_horizontal_mirror() {
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
            .enable_horizontal_flip()
            .await
            .expect("mirror on");
        assert_eq!(one_frame(&writes), MIRROR_ON);

        nouns
            .image()
            .disable_horizontal_flip()
            .await
            .expect("mirror off");
        assert_eq!(one_frame(&writes), MIRROR_OFF);

        session.shutdown().await.expect("shutdown");
    }

    #[tokio::test]
    async fn dynamic_mirror_off_invalidates_the_cached_flip_pair() {
        let (transport, writes) = ProbeTransport::new();
        let session = open(
            transport,
            ProfileSpec::from_compile_time::<PtzOpticsG2>().expect("PTZOptics profile"),
        )
        .await;
        let camera = DynSessionCamera::from_session(&session).expect("dynamic camera");
        let nouns: &dyn DynSessionCameraNouns = &camera;

        nouns.image().set_flip_both().await.expect("combined flip");
        let _ = one_frame(&writes);
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
        assert_eq!(one_frame(&writes), MIRROR_OFF);
        assert_eq!(
            camera.state_cache().value(StateKey::Flip),
            StateEntry::Unknown,
        );

        session.shutdown().await.expect("shutdown");
    }
}
