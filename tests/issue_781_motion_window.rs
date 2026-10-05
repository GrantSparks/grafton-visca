//! `is_moving` observes over its requested window on every facade (#781).
//!
//! The fake camera's zoom position is a quantized function of elapsed time,
//! so two back-to-back reads are almost always equal while reads separated by
//! the window are not. Each test records when each zoom inquiry reached the
//! transport and checks the separation directly, rather than relying on the
//! verdict alone.

#![cfg(any(
    feature = "blocking",
    all(feature = "async", feature = "runtime-tokio")
))]

#[path = "common/profile_fixtures.rs"]
mod profile_fixtures;

use std::{
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};

use grafton_visca::{
    camera::{MotionQuery, MovementTolerance},
    AffectedAxes, Error, ProfileSpec,
};

use profile_fixtures::MotionOwnerCompileTimeProfile;

/// One zoom unit per step: slow enough that an immediate pair reads equal.
const STEP: Duration = Duration::from_millis(40);
const WINDOW: Duration = Duration::from_millis(200);
const ZOOM_INQUIRY: [u8; 4] = [0x81, 0x09, 0x04, 0x47];

/// A camera whose zoom either creeps one unit per [`STEP`] or holds still.
#[derive(Debug, Clone)]
struct ZoomCamera {
    started: Instant,
    ramping: bool,
    inquiries: Arc<Mutex<Vec<Instant>>>,
    writes: Arc<Mutex<usize>>,
}

impl ZoomCamera {
    fn new(ramping: bool) -> Self {
        Self {
            started: Instant::now(),
            ramping,
            inquiries: Arc::new(Mutex::new(Vec::new())),
            writes: Arc::new(Mutex::new(0)),
        }
    }

    fn respond(&self, bytes: &[u8]) -> Vec<Vec<u8>> {
        *self.writes.lock().expect("writes lock") += 1;
        if !bytes.starts_with(&ZOOM_INQUIRY) {
            return vec![vec![0x90, 0x41, 0xff], vec![0x90, 0x51, 0xff]];
        }
        let now = Instant::now();
        self.inquiries.lock().expect("inquiries lock").push(now);
        let zoom = if self.ramping {
            let steps = now.duration_since(self.started).as_nanos() / STEP.as_nanos();
            u16::try_from(steps).unwrap_or(u16::MAX).min(0x4000)
        } else {
            0x0100
        };
        vec![vec![
            0x90,
            0x50,
            ((zoom >> 12) & 0x0f) as u8,
            ((zoom >> 8) & 0x0f) as u8,
            ((zoom >> 4) & 0x0f) as u8,
            (zoom & 0x0f) as u8,
            0xff,
        ]]
    }

    /// The two zoom inquiries of one `is_moving` call were separated by at
    /// least the requested window.
    fn assert_sampled_across(&self, window: Duration) {
        let inquiries = self.inquiries.lock().expect("inquiries lock");
        assert_eq!(inquiries.len(), 2, "exactly two zoom snapshots");
        let separation = inquiries[1].duration_since(inquiries[0]);
        assert!(
            separation >= window,
            "snapshots {separation:?} apart, window {window:?}"
        );
    }

    fn writes(&self) -> usize {
        *self.writes.lock().expect("writes lock")
    }
}

fn profile() -> ProfileSpec {
    ProfileSpec::from_compile_time::<MotionOwnerCompileTimeProfile>().expect("motion profile")
}

/// Any change of more than one unit is movement.
fn creeping_zoom_query(window: Duration) -> MotionQuery {
    MotionQuery::new(AffectedAxes::ZOOM)
        .with_tolerance({
            let mut config = MovementTolerance::default();
            config.zoom = 1;
            config
        })
        .with_window(window)
}

fn assert_zero_window_rejected(result: Result<bool, Error>, camera: &ZoomCamera) {
    assert!(matches!(
        result,
        Err(Error::InvalidParameter {
            parameter: "MotionQuery::window",
            ..
        })
    ));
    assert_eq!(camera.writes(), 0, "a zero window is rejected before I/O");
}

#[cfg(feature = "blocking")]
mod blocking_facade {
    use std::collections::VecDeque;

    use grafton_visca::{
        blocking::{Session, SessionConfig},
        camera::TransportKind,
        command::CommandKind,
        transport::{
            BlockingTransport, HasTransportConfig, ReceiveOutcome, SendSemantics, TransportConfig,
        },
    };

    use super::*;

    #[derive(Debug)]
    struct Transport {
        config: TransportConfig,
        camera: ZoomCamera,
        responses: VecDeque<Vec<u8>>,
    }

    impl HasTransportConfig for Transport {
        fn transport_config(&self) -> &TransportConfig {
            &self.config
        }

        fn standard_transport_kind(&self) -> Option<TransportKind> {
            None
        }
    }

    impl BlockingTransport for Transport {
        fn send_with_timeout(
            &mut self,
            bytes: &[u8],
            _kind: CommandKind,
            _timeout: Duration,
        ) -> Result<(), Error> {
            let responses = self.camera.respond(bytes);
            self.responses.extend(responses);
            Ok(())
        }

        fn recv_into_with_timeout(
            &mut self,
            dst: &mut [u8],
            _timeout: Duration,
        ) -> Result<ReceiveOutcome, Error> {
            let response = self.responses.pop_front().ok_or_else(Error::io_timeout)?;
            Ok(ReceiveOutcome::copy_message(&response, dst))
        }

        fn send_semantics(&self) -> SendSemantics {
            SendSemantics::Datagram
        }
    }

    fn open(camera: &ZoomCamera) -> Session {
        let transport = Transport {
            config: TransportConfig::default(),
            camera: camera.clone(),
            responses: VecDeque::new(),
        };
        Session::open(transport, SessionConfig::new(profile())).expect("blocking session")
    }

    #[test]
    fn typed_motion_observes_creep_across_the_window() {
        let camera = ZoomCamera::new(true);
        let session = open(&camera);
        let moving = session
            .camera::<MotionOwnerCompileTimeProfile>()
            .expect("camera")
            .motion()
            .is_moving_axes(creeping_zoom_query(WINDOW))
            .expect("motion query");
        camera.assert_sampled_across(WINDOW);
        assert!(moving, "creep over the window is movement");
        session.shutdown().expect("shutdown");
    }

    #[test]
    fn typed_motion_reports_no_movement_only_after_the_window() {
        let camera = ZoomCamera::new(false);
        let session = open(&camera);
        let moving = session
            .camera::<MotionOwnerCompileTimeProfile>()
            .expect("camera")
            .motion()
            .is_moving_axes(creeping_zoom_query(WINDOW))
            .expect("motion query");
        camera.assert_sampled_across(WINDOW);
        assert!(!moving);
        session.shutdown().expect("shutdown");
    }

    #[test]
    fn typed_motion_rejects_a_zero_window() {
        let camera = ZoomCamera::new(true);
        let session = open(&camera);
        let result = session
            .camera::<MotionOwnerCompileTimeProfile>()
            .expect("camera")
            .motion()
            .is_moving_axes(creeping_zoom_query(Duration::ZERO));
        assert_zero_window_rejected(result, &camera);
        session.shutdown().expect("shutdown");
    }

    #[cfg(feature = "dyn-api")]
    #[test]
    fn dynamic_motion_observes_creep_across_the_window() {
        let camera = ZoomCamera::new(true);
        let session = open(&camera);
        let moving = session
            .camera_dyn()
            .expect("dynamic camera")
            .motion()
            .is_moving_axes(creeping_zoom_query(WINDOW))
            .expect("motion query");
        camera.assert_sampled_across(WINDOW);
        assert!(moving);
        session.shutdown().expect("shutdown");
    }
}

#[cfg(all(feature = "async", feature = "runtime-tokio"))]
mod async_facade {
    use grafton_visca::{
        runtime::TokioRuntime,
        transport::{
            AsyncTransport, HasTransportConfig, ReceiveOutcome, SendSemantics, TransportConfig,
        },
        Session, SessionConfig,
    };

    use super::*;

    #[derive(Debug)]
    struct Transport {
        config: TransportConfig,
        camera: ZoomCamera,
        responses: flume::Receiver<Vec<u8>>,
        response_tx: flume::Sender<Vec<u8>>,
    }

    impl HasTransportConfig for Transport {
        fn transport_config(&self) -> &TransportConfig {
            &self.config
        }
    }

    impl AsyncTransport for Transport {
        async fn send(&mut self, bytes: &[u8]) -> Result<(), Error> {
            for response in self.camera.respond(bytes) {
                self.response_tx
                    .send_async(response)
                    .await
                    .map_err(|_| Error::connection_closed(None))?;
            }
            Ok(())
        }

        async fn recv_into(&mut self, dst: &mut [u8]) -> Result<ReceiveOutcome, Error> {
            let response = self
                .responses
                .recv_async()
                .await
                .map_err(|_| Error::connection_closed(None))?;
            Ok(ReceiveOutcome::copy_message(&response, dst))
        }

        fn send_semantics(&self) -> SendSemantics {
            SendSemantics::Datagram
        }
    }

    async fn open(camera: &ZoomCamera) -> Session {
        let (response_tx, responses) = flume::unbounded();
        let transport = Transport {
            config: TransportConfig::default(),
            camera: camera.clone(),
            responses,
            response_tx,
        };
        Session::open(
            transport,
            SessionConfig::new(profile()),
            TokioRuntime::from_current().expect("tokio runtime"),
        )
        .await
        .expect("async session")
    }

    #[tokio::test]
    async fn typed_motion_observes_creep_across_the_window() {
        let camera = ZoomCamera::new(true);
        let session = open(&camera).await;
        let moving = session
            .camera::<MotionOwnerCompileTimeProfile>()
            .expect("camera")
            .motion()
            .is_moving_axes(creeping_zoom_query(WINDOW))
            .await
            .expect("motion query");
        camera.assert_sampled_across(WINDOW);
        assert!(moving, "creep over the window is movement");
        session.shutdown().expect("shutdown");
    }

    #[tokio::test]
    async fn typed_motion_reports_no_movement_only_after_the_window() {
        let camera = ZoomCamera::new(false);
        let session = open(&camera).await;
        let moving = session
            .camera::<MotionOwnerCompileTimeProfile>()
            .expect("camera")
            .motion()
            .is_moving_axes(creeping_zoom_query(WINDOW))
            .await
            .expect("motion query");
        camera.assert_sampled_across(WINDOW);
        assert!(!moving);
        session.shutdown().expect("shutdown");
    }

    #[tokio::test]
    async fn typed_motion_rejects_a_zero_window() {
        let camera = ZoomCamera::new(true);
        let session = open(&camera).await;
        let result = session
            .camera::<MotionOwnerCompileTimeProfile>()
            .expect("camera")
            .motion()
            .is_moving_axes(creeping_zoom_query(Duration::ZERO))
            .await;
        assert_zero_window_rejected(result, &camera);
        session.shutdown().expect("shutdown");
    }

    #[cfg(feature = "dyn-api")]
    #[tokio::test]
    async fn dynamic_motion_observes_creep_across_the_window() {
        let camera = ZoomCamera::new(true);
        let session = open(&camera).await;
        let moving = session
            .camera_dyn()
            .expect("dynamic camera")
            .motion()
            .is_moving_axes(creeping_zoom_query(WINDOW))
            .await
            .expect("motion query");
        camera.assert_sampled_across(WINDOW);
        assert!(moving);
        session.shutdown().expect("shutdown");
    }
}
