//! Smol coverage for the final owner-backed dynamic noun facade.

#![cfg(all(feature = "dyn-api", feature = "runtime-smol"))]

use std::{
    future::Future,
    sync::{Arc, Mutex},
};

use grafton_visca::{
    dynapi::{DynSessionCamera, DynSessionCameraNouns},
    profile::ProfileSpec,
    profiles::PtzOpticsG2,
    state_cache::StateEntry,
    transport::{AsyncTransport, HasTransportConfig, SendSemantics, TransportConfig},
    CameraId, Error, Session, SessionConfig, SmolRuntime, StateKey,
};

#[derive(Debug)]
struct SmolTransport {
    config: TransportConfig,
    replies: flume::Receiver<Vec<u8>>,
    reply_tx: flume::Sender<Vec<u8>>,
    writes: Arc<Mutex<Vec<Vec<u8>>>>,
}

impl HasTransportConfig for SmolTransport {
    fn transport_config(&self) -> &TransportConfig {
        &self.config
    }
}

impl AsyncTransport for SmolTransport {
    fn send(&mut self, bytes: &[u8]) -> impl Future<Output = Result<(), Error>> + Send {
        let bytes = bytes.to_vec();
        self.writes
            .lock()
            .expect("smol writes lock")
            .push(bytes.clone());
        let source = bytes
            .first()
            .map(|address| 0x80 | ((address & 0x0f).saturating_add(8) << 4))
            .unwrap_or(0x90);
        let reply_tx = self.reply_tx.clone();
        async move {
            reply_tx
                .send_async(vec![source, 0x41, 0xff])
                .await
                .map_err(|_| Error::ConnectionClosed { reason: None })?;
            reply_tx
                .send_async(vec![source, 0x51, 0xff])
                .await
                .map_err(|_| Error::ConnectionClosed { reason: None })?;
            Ok(())
        }
    }

    #[allow(clippy::manual_async_fn)]
    fn recv_into<'a>(
        &'a mut self,
        dst: &'a mut [u8],
    ) -> impl Future<Output = Result<usize, Error>> + Send {
        async move {
            let bytes = self
                .replies
                .recv_async()
                .await
                .map_err(|_| Error::ConnectionClosed { reason: None })?;
            dst[..bytes.len()].copy_from_slice(&bytes);
            Ok(bytes.len())
        }
    }

    fn send_semantics(&self) -> SendSemantics {
        SendSemantics::Datagram
    }

    fn addressing_mode_hint(&self) -> Option<grafton_visca::transport::AddressingMode> {
        Some(self.config.addressing)
    }
}

fn transport(serial: bool) -> (SmolTransport, Arc<Mutex<Vec<Vec<u8>>>>) {
    let (reply_tx, replies) = flume::bounded(32);
    let mut config = TransportConfig::default();
    if serial {
        config.addressing = grafton_visca::transport::AddressingMode::Serial;
    }
    let writes = Arc::new(Mutex::new(Vec::new()));
    (
        SmolTransport {
            config,
            replies,
            reply_tx,
            writes: Arc::clone(&writes),
        },
        writes,
    )
}

fn profile() -> ProfileSpec {
    ProfileSpec::from_compile_time::<PtzOpticsG2>().expect("profile")
}

#[test]
fn smol_dynamic_noun_targeted_and_applied_handles_share_owner() {
    smol::block_on(async {
        let runtime = SmolRuntime::new();
        let (transport, _) = transport(false);
        let session = Session::open(transport, SessionConfig::new(profile()), runtime)
            .await
            .expect("owner-backed smol session");
        let camera = DynSessionCamera::from_session(&session).expect("dynamic camera");
        let nouns: &dyn DynSessionCameraNouns = &camera;

        nouns
            .zoom()
            .stop()
            .await
            .expect("applied operation admission")
            .applied()
            .await
            .expect("applied operation completion");
        nouns
            .pan_tilt()
            .home()
            .await
            .expect("targeted operation admission")
            .applied()
            .await
            .expect("targeted operation completion");

        session.shutdown().await.expect("smol shutdown");
    });
}

#[test]
fn smol_dynamic_motion_view_delegates_to_same_owner() {
    smol::block_on(async {
        let runtime = SmolRuntime::new();
        let (transport, _) = transport(false);
        let session = Session::open(transport, SessionConfig::new(profile()), runtime)
            .await
            .expect("owner-backed smol session");
        let camera = DynSessionCamera::from_session(&session).expect("dynamic camera");

        camera
            .motion()
            .stop_all_motion()
            .await
            .expect("motion safety delegation");
        session.shutdown().await.expect("smol shutdown");
    });
}

#[test]
fn smol_dynamic_unsupported_gate_and_target_selection_are_preflighted() {
    smol::block_on(async {
        let (first_transport, writes) = transport(false);
        let runtime = SmolRuntime::new();
        let session = Session::open(first_transport, SessionConfig::new(profile()), runtime)
            .await
            .expect("owner-backed smol session");
        let camera = DynSessionCamera::from_session(&session).expect("dynamic camera");
        let error = camera
            .zoom()
            .set_digital_zoom(true)
            .await
            .expect_err("unsupported digital zoom must fail before I/O");
        assert!(matches!(
            error,
            Error::FeatureNotSupported {
                feature: "digital zoom"
            }
        ));
        assert!(writes.lock().expect("writes lock").is_empty());
        session.shutdown().await.expect("smol shutdown");

        let (transport, writes) = transport(true);
        let mut config = SessionConfig::new(profile());
        config
            .register_target(CameraId::CAMERA_2, profile())
            .expect("camera 2 registration");
        let session = Session::open(transport, config, SmolRuntime::new())
            .await
            .expect("multi-target smol session");
        assert!(matches!(
            DynSessionCamera::from_session(&session),
            Err(Error::InvalidState(_))
        ));
        let selected = DynSessionCamera::from_session_target(&session, CameraId::CAMERA_2)
            .expect("explicit target selection");
        assert_eq!(selected.target(), CameraId::CAMERA_2);
        assert!(writes.lock().expect("writes lock").is_empty());
        session.shutdown().await.expect("smol shutdown");
    });
}

#[test]
fn smol_dynamic_cache_views_share_and_isolate_owner_state() {
    smol::block_on(async {
        let (transport, _) = transport(true);
        let mut config = SessionConfig::new(profile());
        config
            .register_target(CameraId::CAMERA_2, profile())
            .expect("camera 2 registration");
        let session = Session::open(transport, config, SmolRuntime::new())
            .await
            .expect("multi-target smol session");
        let first = DynSessionCamera::from_session_target(&session, CameraId::CAMERA_1)
            .expect("camera 1 view");
        let same = DynSessionCamera::from_session_target(&session, CameraId::CAMERA_1)
            .expect("same-target view");
        let second = DynSessionCamera::from_session_target(&session, CameraId::CAMERA_2)
            .expect("camera 2 view");
        assert_eq!(
            first.state_cache().value(StateKey::MulticastStreaming),
            StateEntry::Unknown
        );
        assert_eq!(
            second.state_cache().value(StateKey::MulticastStreaming),
            StateEntry::Unknown
        );
        first
            .advanced()
            .multicast_on()
            .await
            .expect("owner applied multicast state");
        assert!(matches!(
            first
                .state_cache()
                .value(StateKey::MulticastStreaming),
            StateEntry::Set(value) if value.get(0) == Some(1)
        ));
        assert!(matches!(
            same.state_cache()
                .value(StateKey::MulticastStreaming),
            StateEntry::Set(value) if value.get(0) == Some(1)
        ));
        assert_eq!(
            second.state_cache().value(StateKey::MulticastStreaming),
            StateEntry::Unknown
        );
        session.shutdown().await.expect("smol shutdown");
    });
}
