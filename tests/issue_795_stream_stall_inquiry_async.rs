//! Async-facade twin of `issue_795_stream_stall_inquiry.rs` (#795, observed
//! on a PTZOptics G2 bench, 2026-10-04): a stale raw TCP inquiry reply,
//! delivered by retransmission after a silent outbound stall, must never be
//! bound to the next inquiry on the same camera, and a reply that never
//! arrives latches only that camera's inquiries.

#![cfg(feature = "runtime-tokio")]
#![allow(clippy::expect_used, clippy::unwrap_used, clippy::panic)]

use std::{
    future::Future,
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};

use grafton_visca::{
    command::FocusMode,
    profile::ProfileSpec,
    profiles::PtzOpticsG2,
    transport::{AsyncTransport, HasTransportConfig, SendSemantics, TransportConfig},
    CameraId, Certainty, Error, FailureContext, FailureStage, Session, SessionConfig, TokioRuntime,
};

const POWER_INQUIRY: &[u8] = &[0x81, 0x09, 0x04, 0x00, 0xff];
const FOCUS_MODE_INQUIRY: &[u8] = &[0x81, 0x09, 0x04, 0x38, 0xff];
const PAN_TILT_STOP: &[u8] = &[0x81, 0x01, 0x06, 0x01, 0x0c, 0x0a, 0x03, 0x03, 0xff];

/// Camera truth: power on (`02`), focus MANUAL (`03`). A muted camera never
/// answers the power inquiry.
fn camera_answer(bytes: &[u8], mute_power: bool) -> Vec<Vec<u8>> {
    match bytes {
        b if b == POWER_INQUIRY && !mute_power => vec![vec![0x90, 0x50, 0x02, 0xff]],
        b if b == FOCUS_MODE_INQUIRY => vec![vec![0x90, 0x50, 0x03, 0xff]],
        b if b == PAN_TILT_STOP => vec![vec![0x90, 0x41, 0xff], vec![0x90, 0x51, 0xff]],
        _ => Vec::new(),
    }
}

#[derive(Debug, Default)]
struct WireState {
    fault: bool,
    mute_power: bool,
    queued: Vec<Vec<u8>>,
    writes: Vec<Vec<u8>>,
}

/// The shared stalled TCP path. While faulted, writes "succeed" into a
/// kernel queue. Once lifted, the queue is delivered in order either by the
/// next write or by [`Wire::retransmit`], and the camera answers every copy.
#[derive(Debug, Clone)]
struct Wire {
    state: Arc<Mutex<WireState>>,
    replies: flume::Sender<Vec<u8>>,
}

impl Wire {
    fn set_fault(&self, on: bool) {
        self.state.lock().unwrap().fault = on;
    }

    fn deliver_queued(&self, state: &mut WireState) {
        for request in std::mem::take(&mut state.queued) {
            for reply in camera_answer(&request, state.mute_power) {
                let _ = self.replies.send(reply);
            }
        }
    }

    /// TCP's retransmission timer firing after the fault lifted.
    fn retransmit(&self) {
        let mut state = self.state.lock().unwrap();
        if !state.fault {
            self.deliver_queued(&mut state);
        }
    }
}

#[derive(Debug)]
struct StallingStream {
    config: TransportConfig,
    wire: Wire,
    replies: flume::Receiver<Vec<u8>>,
}

impl HasTransportConfig for StallingStream {
    fn transport_config(&self) -> &TransportConfig {
        &self.config
    }
}

impl AsyncTransport for StallingStream {
    fn send(&mut self, bytes: &[u8]) -> impl Future<Output = Result<(), Error>> + Send {
        {
            let mut state = self.wire.state.lock().unwrap();
            state.writes.push(bytes.to_vec());
            state.queued.push(bytes.to_vec());
            if !state.fault {
                self.wire.deliver_queued(&mut state);
            }
        }
        async { Ok(()) }
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
                .map_err(|_| Error::connection_closed(None))?;
            dst[..bytes.len()].copy_from_slice(&bytes);
            Ok(bytes.len())
        }
    }

    fn send_semantics(&self) -> SendSemantics {
        SendSemantics::Stream
    }
}

async fn open() -> (Session, Wire) {
    let (sender, receiver) = flume::unbounded();
    let wire = Wire {
        state: Arc::default(),
        replies: sender,
    };
    let transport = StallingStream {
        config: TransportConfig::default(),
        wire: wire.clone(),
        replies: receiver,
    };
    let session = Session::open(
        transport,
        SessionConfig::new(ProfileSpec::from_compile_time::<PtzOpticsG2>().expect("G2")),
        TokioRuntime::from_current().expect("Tokio runtime"),
    )
    .await
    .expect("session");
    (session, wire)
}

fn assert_correlation_lost<T: std::fmt::Debug>(result: &Result<T, Error>) {
    let Err(error) = result else {
        panic!("expected InquiryCorrelationLost, got {result:?}");
    };
    assert!(
        matches!(error, Error::InquiryCorrelationLost { camera, .. } if *camera == CameraId::CAMERA_1),
        "{error:?}"
    );
    assert!(!error.is_retryable());
    assert!(!error.requires_new_session());
}

#[tokio::test]
async fn stale_stream_inquiry_reply_never_binds_a_later_inquiry_async() {
    let (session, wire) = open().await;
    let camera = session.camera::<PtzOpticsG2>().expect("camera");

    wire.set_fault(true);
    let power = camera.power().state().await;
    assert_eq!(
        power.as_ref().err().and_then(Error::failure_context),
        Some(FailureContext::new(
            FailureStage::Terminal,
            Certainty::FailedConclusively
        )),
        "{power:?}"
    );
    wire.set_fault(false);

    // The queued power copies are retransmitted 300 ms after the fault lifts,
    // later than the 150 ms reply skew, while the focus inquiry is pending.
    let retransmit = async {
        tokio::time::sleep(Duration::from_millis(300)).await;
        wire.retransmit();
    };
    let focus = camera.focus();
    let (mode, ()) = tokio::join!(focus.mode(), retransmit);
    assert!(
        matches!(mode, Ok(FocusMode::Manual)),
        "a stale power reply must never be returned as the focus mode: {mode:?}"
    );
    assert!(camera.power().state().await.expect("power after recovery"));
}

/// A camera that never answers an inquiry: once the ambiguity window passes,
/// the queued inquiry and every later one to that camera fail with
/// `InquiryCorrelationLost` (the later ones at admission, promptly) without
/// being written, while a STOP to the same camera still applies.
#[tokio::test]
async fn unanswered_stream_inquiry_latches_the_cameras_inquiries_async() {
    let (session, wire) = open().await;
    wire.state.lock().unwrap().mute_power = true;
    let camera = session.camera::<PtzOpticsG2>().expect("camera");

    let power = camera.power().state().await;
    assert_eq!(
        power.as_ref().err().and_then(Error::failure_context),
        Some(FailureContext::new(
            FailureStage::Terminal,
            Certainty::FailedConclusively
        )),
        "{power:?}"
    );

    let queued = camera.focus().mode().await;
    assert_correlation_lost(&queued);
    let started = Instant::now();
    let rejected = camera.focus().mode().await;
    assert!(started.elapsed() < Duration::from_millis(200));
    assert_correlation_lost(&rejected);
    let focus_writes = wire
        .state
        .lock()
        .unwrap()
        .writes
        .iter()
        .filter(|w| *w == FOCUS_MODE_INQUIRY)
        .count();
    assert_eq!(focus_writes, 0);

    let mut stop = camera.pan_tilt().stop().await.expect("STOP admitted");
    stop.applied()
        .await
        .expect("STOP to the latched camera applies");
}
