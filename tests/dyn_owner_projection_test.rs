//! Owner-backed dynamic operation projection coverage.

#![cfg(all(
    feature = "dyn-api",
    any(feature = "runtime-tokio", feature = "runtime-smol")
))]

use std::{future::Future, sync::Arc};

use grafton_visca::{
    dynapi::{DynSessionCamera, DynSessionCameraNouns},
    profile::{PositionInquirySupport, ProfileSpec},
    profiles::SonyBRC300,
    transport::{AsyncTransport, HasTransportConfig, SendSemantics, TransportConfig},
    CameraId, Error, ProfileTiming, Session, SessionConfig,
};

#[derive(Debug)]
struct ScriptedTransport {
    config: TransportConfig,
    replies: flume::Receiver<Vec<u8>>,
    reply_tx: flume::Sender<Vec<u8>>,
    writes: Arc<std::sync::Mutex<Vec<Vec<u8>>>>,
}

impl HasTransportConfig for ScriptedTransport {
    fn transport_config(&self) -> &TransportConfig {
        &self.config
    }
}

impl AsyncTransport for ScriptedTransport {
    fn send(&mut self, bytes: &[u8]) -> impl Future<Output = Result<(), Error>> + Send {
        let bytes = bytes.to_vec();
        let reply_tx = self.reply_tx.clone();
        self.writes
            .lock()
            .expect("scripted transport writes lock")
            .push(bytes.clone());
        async move {
            if bytes.get(1) == Some(&0x09) {
                // Four compact bytes are accepted by the pan/tilt decoder and
                // represent the same zero snapshot on both settlement samples.
                reply_tx
                    .send_async(vec![0x90, 0x50, 0x00, 0x00, 0x00, 0x00, 0xff])
                    .await
                    .map_err(|_| Error::ConnectionClosed { reason: None })?;
            } else {
                reply_tx
                    .send_async(vec![0x90, 0x41, 0xff])
                    .await
                    .map_err(|_| Error::ConnectionClosed { reason: None })?;
                reply_tx
                    .send_async(vec![0x90, 0x51, 0xff])
                    .await
                    .map_err(|_| Error::ConnectionClosed { reason: None })?;
            }
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
}

fn transport() -> (ScriptedTransport, Arc<std::sync::Mutex<Vec<Vec<u8>>>>) {
    let (reply_tx, replies) = flume::bounded(32);
    let writes = Arc::new(std::sync::Mutex::new(Vec::new()));
    (
        ScriptedTransport {
            config: TransportConfig::default(),
            replies,
            reply_tx,
            writes: Arc::clone(&writes),
        },
        writes,
    )
}

fn runtime_equivalent(static_profile: &ProfileSpec) -> ProfileSpec {
    let coordinates = static_profile
        .pan_tilt_coordinates()
        .expect("Sony BRC-300 pan/tilt conversion");
    let timing: ProfileTiming = static_profile.timing();
    ProfileSpec::builder(static_profile.capabilities().clone())
        .pan_tilt_coordinates(
            coordinates.coordinate_system(),
            coordinates.pan_degrees_to_units(),
            coordinates.tilt_degrees_to_units(),
        )
        .transports(static_profile.transports())
        .envelope(static_profile.envelope())
        .timing(
            ProfileTiming::builder()
                .ack_timeout(timing.ack_timeout())
                .command_timeouts(timing.command_timeouts())
                .inquiry_timeout(timing.inquiry_timeout())
                .cancellation_timeout(timing.cancellation_timeout())
                .ambiguity_timeout(timing.ambiguity_timeout())
                .busy_timeout(timing.busy_timeout())
                .minimum_inquiry_spacing(timing.minimum_inquiry_spacing())
                .minimum_command_spacing(timing.minimum_command_spacing())
                .build()
                .expect("valid timing"),
        )
        .maximum_command_sockets(static_profile.maximum_command_sockets())
        .supports_operation_complete(static_profile.supports_operation_complete())
        .supports_command_cancel(static_profile.supports_command_cancel())
        .preset_recall_axes(static_profile.preset_recall_axes())
        .position_inquiries(static_profile.position_inquiries())
        .build()
        .expect("runtime profile equivalent")
}

async fn dynamic_targeted_polling<E>(runtime: E)
where
    E: grafton_visca::Executor,
{
    let static_profile = ProfileSpec::from_compile_time::<SonyBRC300>().expect("static profile");
    assert!(!static_profile.supports_operation_complete());
    assert_eq!(
        static_profile.position_inquiries(),
        PositionInquirySupport::new(true, true, true)
    );
    let runtime_profile = runtime_equivalent(&static_profile);
    assert_eq!(static_profile, runtime_profile);

    let (transport, writes) = transport();
    let session = Session::open(
        transport,
        SessionConfig::new(runtime_profile.clone()),
        runtime,
    )
    .await
    .expect("owner-backed session");
    let camera = DynSessionCamera::from_session(&session).expect("dynamic camera");
    assert_eq!(camera.target(), CameraId::CAMERA_1);
    assert_eq!(camera.profile(), &runtime_profile);
    let typed = camera
        .camera::<SonyBRC300>()
        .expect("exact dynamic-to-typed projection");
    assert_eq!(typed.target(), camera.target());

    let object: &dyn DynSessionCameraNouns = &camera;
    object
        .pan_tilt()
        .home()
        .await
        .expect("targeted admission")
        .settled()
        .await
        .expect("targeted dynamic settlement");

    {
        let writes = writes.lock().expect("writes lock");
        assert_eq!(writes.len(), 3, "command plus two exact pan/tilt inquiries");
        assert_eq!(writes[0].get(1), Some(&0x01));
        assert!(writes[1].starts_with(&[0x81, 0x09, 0x06, 0x12]));
        assert!(writes[2].starts_with(&[0x81, 0x09, 0x06, 0x12]));
    }

    session.shutdown().await.expect("session shutdown");
}

#[cfg(feature = "runtime-tokio")]
#[tokio::test]
async fn tokio_dynamic_targeted_projection_uses_owner_settlement_wait() {
    dynamic_targeted_polling(grafton_visca::TokioRuntime::from_current().expect("runtime")).await;
}

#[cfg(feature = "runtime-smol")]
#[test]
fn smol_dynamic_targeted_projection_uses_owner_settlement_wait() {
    smol::block_on(dynamic_targeted_polling(grafton_visca::SmolRuntime::new()));
}
