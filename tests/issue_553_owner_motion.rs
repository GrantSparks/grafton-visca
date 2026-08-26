//! Owner-backed motion surface coverage for the root async session camera.

#![cfg(all(
    feature = "async",
    any(feature = "runtime-tokio", feature = "runtime-smol")
))]

#[path = "common/profile_fixtures.rs"]
mod profile_fixtures;

use std::{
    collections::VecDeque,
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex,
    },
    time::Duration,
};

use grafton_visca::{
    camera::{IdleWait, MotionQuery},
    profile::{PositionInquirySupport, ProfileSpec},
    transport::{AsyncTransport, HasTransportConfig, SendSemantics, TransportConfig},
    AffectedAxes, Error, Executor, Session, SessionConfig,
};

use profile_fixtures::NonDefaultCompileTimeProfile;

#[derive(Debug, Clone, Copy)]
enum InquiryAxis {
    PanTilt,
    Zoom,
    Focus,
}

#[derive(Debug, Default)]
struct PositionScript {
    pan_tilt: VecDeque<(i16, i16)>,
    zoom: VecDeque<u16>,
    focus: VecDeque<u16>,
}

#[derive(Debug)]
struct MotionTransport {
    config: TransportConfig,
    responses: flume::Receiver<Vec<u8>>,
    response_tx: flume::Sender<Vec<u8>>,
    writes: Arc<Mutex<Vec<Vec<u8>>>>,
    positions: Arc<Mutex<PositionScript>>,
    fail_first_pan_stop: Arc<AtomicBool>,
}

impl MotionTransport {
    fn new(script: PositionScript, fail_first_pan_stop: bool) -> (Self, Arc<Mutex<Vec<Vec<u8>>>>) {
        let (response_tx, responses) = flume::unbounded();
        let writes = Arc::new(Mutex::new(Vec::new()));
        (
            Self {
                config: TransportConfig::default(),
                responses,
                response_tx,
                writes: Arc::clone(&writes),
                positions: Arc::new(Mutex::new(script)),
                fail_first_pan_stop: Arc::new(AtomicBool::new(fail_first_pan_stop)),
            },
            writes,
        )
    }

    fn next_position(&self, axis: InquiryAxis) -> Vec<u8> {
        let mut script = self.positions.lock().expect("position script lock");
        match axis {
            InquiryAxis::PanTilt => {
                let (pan, tilt) = script.pan_tilt.pop_front().unwrap_or((0, 0));
                position_frame_pan_tilt(pan, tilt)
            }
            InquiryAxis::Zoom => position_frame_value(script.zoom.pop_front().unwrap_or(0)),
            InquiryAxis::Focus => position_frame_value(script.focus.pop_front().unwrap_or(0)),
        }
    }

    fn response_for(&self, bytes: &[u8]) -> Vec<Vec<u8>> {
        if bytes.starts_with(&[0x81, 0x09, 0x06, 0x12]) {
            return vec![self.next_position(InquiryAxis::PanTilt)];
        }
        if bytes.starts_with(&[0x81, 0x09, 0x04, 0x47]) {
            return vec![self.next_position(InquiryAxis::Zoom)];
        }
        if bytes.starts_with(&[0x81, 0x09, 0x04, 0x48]) {
            return vec![self.next_position(InquiryAxis::Focus)];
        }
        if bytes.get(2) == Some(&0x06)
            && bytes.get(3) == Some(&0x01)
            && self.fail_first_pan_stop.swap(false, Ordering::AcqRel)
        {
            return vec![vec![0x90, 0x60, 0x02, 0xff]];
        }
        vec![vec![0x90, 0x41, 0xff], vec![0x90, 0x51, 0xff]]
    }
}

impl HasTransportConfig for MotionTransport {
    fn transport_config(&self) -> &TransportConfig {
        &self.config
    }
}

impl AsyncTransport for MotionTransport {
    async fn send(&mut self, bytes: &[u8]) -> Result<(), Error> {
        self.writes
            .lock()
            .expect("writes lock")
            .push(bytes.to_vec());
        for response in self.response_for(bytes) {
            self.response_tx
                .send_async(response)
                .await
                .map_err(|_| Error::ConnectionClosed { reason: None })?;
        }
        Ok(())
    }

    async fn recv_into(&mut self, dst: &mut [u8]) -> Result<usize, Error> {
        let response = self
            .responses
            .recv_async()
            .await
            .map_err(|_| Error::ConnectionClosed { reason: None })?;
        dst[..response.len()].copy_from_slice(&response);
        Ok(response.len())
    }

    fn send_semantics(&self) -> SendSemantics {
        SendSemantics::Datagram
    }
}

fn position_frame_pan_tilt(pan: i16, tilt: i16) -> Vec<u8> {
    vec![
        0x90,
        0x50,
        (pan as u16 >> 8) as u8,
        pan as u8,
        (tilt as u16 >> 8) as u8,
        tilt as u8,
        0xff,
    ]
}

fn position_frame_value(value: u16) -> Vec<u8> {
    vec![
        0x90,
        0x50,
        ((value >> 12) & 0x0f) as u8,
        ((value >> 8) & 0x0f) as u8,
        ((value >> 4) & 0x0f) as u8,
        (value & 0x0f) as u8,
        0xff,
    ]
}

fn runtime_profile(
    _position_inquiries: PositionInquirySupport,
    supports_operation_complete: bool,
) -> ProfileSpec {
    let _ = supports_operation_complete;
    ProfileSpec::from_compile_time::<NonDefaultCompileTimeProfile>()
        .expect("non-default runtime profile")
}

async fn run_motion_surface<E>(executor: E)
where
    E: Executor,
{
    let profile = runtime_profile(PositionInquirySupport::new(true, true, true), false);
    let script = PositionScript {
        zoom: [10, 10, 10, 10].into_iter().collect(),
        ..PositionScript::default()
    };
    let (transport, writes) = MotionTransport::new(script, false);
    let session = Session::open(transport, SessionConfig::new(profile), executor.clone())
        .await
        .expect("session");
    let camera = session
        .camera::<NonDefaultCompileTimeProfile>()
        .expect("camera");
    assert!(!camera
        .motion()
        .is_moving(MotionQuery::new(AffectedAxes::ZOOM))
        .await
        .expect("zoom motion query"));
    camera
        .motion()
        .wait_until_idle(
            IdleWait::new(AffectedAxes::ZOOM, Duration::from_secs(1)).with_interval(Duration::ZERO),
        )
        .await
        .expect("zoom idle wait");
    {
        let writes = writes.lock().expect("writes lock");
        assert_eq!(writes.len(), 4);
        assert!(writes
            .iter()
            .all(|bytes| bytes.starts_with(&[0x81, 0x09, 0x04, 0x47])));
    }
    session.shutdown().await.expect("shutdown");

    let profile = runtime_profile(PositionInquirySupport::new(true, true, false), true);
    let (transport, writes) = MotionTransport::new(PositionScript::default(), false);
    let session = Session::open(transport, SessionConfig::new(profile), executor.clone())
        .await
        .expect("unsupported-axis session");
    let camera = session
        .camera::<NonDefaultCompileTimeProfile>()
        .expect("camera");
    assert!(matches!(
        camera
            .motion()
            .is_moving(MotionQuery::new(AffectedAxes::ND_FILTER))
            .await,
        Err(Error::FeatureNotSupported { .. })
    ));
    assert!(matches!(
        camera
            .motion()
            .wait_until_idle(IdleWait::new(
                AffectedAxes::ND_FILTER,
                Duration::from_secs(1)
            ))
            .await,
        Err(Error::FeatureNotSupported { .. })
    ));
    assert!(writes.lock().expect("writes lock").is_empty());
    session.shutdown().await.expect("shutdown");

    let script = PositionScript {
        zoom: [0, 100, 100].into_iter().collect(),
        ..PositionScript::default()
    };
    let (transport, writes) = MotionTransport::new(script, false);
    let session = Session::open(
        transport,
        SessionConfig::new(runtime_profile(
            PositionInquirySupport::new(true, true, true),
            false,
        )),
        executor.clone(),
    )
    .await
    .expect("moving session");
    session
        .camera::<NonDefaultCompileTimeProfile>()
        .expect("camera")
        .motion()
        .wait_until_idle(
            IdleWait::new(AffectedAxes::ZOOM, Duration::from_secs(1)).with_interval(Duration::ZERO),
        )
        .await
        .expect("moving then settled");
    assert_eq!(writes.lock().expect("writes lock").len(), 3);
    session.shutdown().await.expect("shutdown");

    let script = PositionScript {
        zoom: [0].into_iter().collect(),
        ..PositionScript::default()
    };
    let (transport, writes) = MotionTransport::new(script, false);
    let session = Session::open(
        transport,
        SessionConfig::new(runtime_profile(
            PositionInquirySupport::new(true, true, true),
            false,
        )),
        executor.clone(),
    )
    .await
    .expect("deadline session");
    let result = session
        .camera::<NonDefaultCompileTimeProfile>()
        .expect("camera")
        .motion()
        .wait_until_idle(
            IdleWait::new(AffectedAxes::ZOOM, Duration::from_millis(5))
                .with_interval(Duration::from_secs(1)),
        )
        .await;
    assert!(matches!(result, Err(Error::Timeout)));
    assert_eq!(writes.lock().expect("writes lock").len(), 1);
    session.shutdown().await.expect("shutdown");

    let (transport, writes) = MotionTransport::new(PositionScript::default(), true);
    let session = Session::open(
        transport,
        SessionConfig::new(runtime_profile(
            PositionInquirySupport::new(true, true, true),
            false,
        )),
        executor,
    )
    .await
    .expect("stop session");
    assert!(matches!(
        session
            .camera::<NonDefaultCompileTimeProfile>()
            .expect("camera")
            .motion()
            .stop_all_motion()
            .await,
        Err(Error::SyntaxError)
    ));
    {
        let writes = writes.lock().expect("writes lock");
        assert_eq!(writes.len(), 3);
        assert!(writes[0].starts_with(&[0x81, 0x01, 0x06, 0x01]));
        assert!(writes[1].starts_with(&[0x81, 0x01, 0x04, 0x07]));
        assert!(writes[2].starts_with(&[0x81, 0x01, 0x04, 0x08]));
    }
    session.shutdown().await.expect("shutdown");
}

#[cfg(feature = "runtime-tokio")]
#[tokio::test]
async fn tokio_owner_motion_surface_is_exact_and_deadline_bound() {
    run_motion_surface(grafton_visca::TokioRuntime::from_current().expect("runtime")).await;
}

#[cfg(feature = "runtime-smol")]
#[test]
fn smol_owner_motion_surface_is_exact_and_deadline_bound() {
    smol::block_on(run_motion_surface(grafton_visca::SmolRuntime::new()));
}
