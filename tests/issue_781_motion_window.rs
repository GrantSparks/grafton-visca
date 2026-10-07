//! `is_moving` observes over its requested window on every facade (#781).
//!
//! The fake camera's zoom position is a quantized function of elapsed time,
//! so two back-to-back reads are almost always equal while reads separated by
//! the window are not. Each test records when each zoom inquiry reached the
//! transport and checks the separation directly, rather than relying on the
//! verdict alone.
//!
//! Every scenario runs on the blocking facade and on the async facade under
//! each enabled runtime.

#![cfg(any(
    feature = "blocking",
    all(
        feature = "async",
        any(feature = "runtime-tokio", feature = "runtime-smol")
    )
))]

use grafton_visca_test_support::{facade_matrix, fake_camera, profile_fixtures};

use std::{
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};

use grafton_visca::{
    camera::{MotionQuery, MovementTolerance},
    AffectedAxes, Error, ProfileSpec, SessionConfig,
};

use fake_camera::FakeCamera;
use profile_fixtures::MotionOwnerCompileTimeProfile;

/// One zoom unit per step: slow enough that an immediate pair reads equal.
const STEP: Duration = Duration::from_millis(40);
const WINDOW: Duration = Duration::from_millis(200);
const ZOOM_INQUIRY: [u8; 4] = [0x81, 0x09, 0x04, 0x47];

/// A camera whose zoom either creeps one unit per [`STEP`] or holds still,
/// and the instants its zoom inquiries arrived.
struct ZoomCamera {
    camera: FakeCamera,
    inquiries: Arc<Mutex<Vec<Instant>>>,
}

impl ZoomCamera {
    fn new(ramping: bool) -> Self {
        let started = Instant::now();
        let inquiries = Arc::new(Mutex::new(Vec::new()));
        let recorded = Arc::clone(&inquiries);
        let camera = FakeCamera::visca(move |bytes, answer| {
            if !bytes.starts_with(&ZOOM_INQUIRY) {
                answer
                    .reply(vec![0x90, 0x41, 0xff])
                    .reply(vec![0x90, 0x51, 0xff]);
                return;
            }
            let now = Instant::now();
            recorded.lock().expect("inquiries lock").push(now);
            let zoom = if ramping {
                let steps = now.duration_since(started).as_nanos() / STEP.as_nanos();
                u16::try_from(steps).unwrap_or(u16::MAX).min(0x4000)
            } else {
                0x0100
            };
            answer.reply(vec![
                0x90,
                0x50,
                ((zoom >> 12) & 0x0f) as u8,
                ((zoom >> 8) & 0x0f) as u8,
                ((zoom >> 4) & 0x0f) as u8,
                (zoom & 0x0f) as u8,
                0xff,
            ]);
        });
        Self { camera, inquiries }
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
        self.camera.write_count()
    }
}

fn config() -> SessionConfig {
    SessionConfig::new(
        ProfileSpec::from_compile_time::<MotionOwnerCompileTimeProfile>().expect("motion profile"),
    )
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

facade_matrix! {
    fn typed_motion_observes_creep_across_the_window() {
        let zoom = ZoomCamera::new(true);
        let session = open!(zoom.camera, config()).expect("session");
        let moving = wait!(session
            .camera::<MotionOwnerCompileTimeProfile>()
            .expect("camera")
            .motion()
            .is_moving(creeping_zoom_query(WINDOW)))
        .expect("motion query");
        zoom.assert_sampled_across(WINDOW);
        assert!(moving, "creep over the window is movement");
        session.shutdown().expect("shutdown");
    }

    fn typed_motion_reports_no_movement_only_after_the_window() {
        let zoom = ZoomCamera::new(false);
        let session = open!(zoom.camera, config()).expect("session");
        let moving = wait!(session
            .camera::<MotionOwnerCompileTimeProfile>()
            .expect("camera")
            .motion()
            .is_moving(creeping_zoom_query(WINDOW)))
        .expect("motion query");
        zoom.assert_sampled_across(WINDOW);
        assert!(!moving);
        session.shutdown().expect("shutdown");
    }

    fn typed_motion_rejects_a_zero_window() {
        let zoom = ZoomCamera::new(true);
        let session = open!(zoom.camera, config()).expect("session");
        let result = wait!(session
            .camera::<MotionOwnerCompileTimeProfile>()
            .expect("camera")
            .motion()
            .is_moving(creeping_zoom_query(Duration::ZERO)));
        assert_zero_window_rejected(result, &zoom);
        session.shutdown().expect("shutdown");
    }

    #[cfg(feature = "dyn-api")]
    fn dynamic_motion_observes_creep_across_the_window() {
        let zoom = ZoomCamera::new(true);
        let session = open!(zoom.camera, config()).expect("session");
        let moving = wait!(session
            .camera_dyn()
            .expect("dynamic camera")
            .motion()
            .is_moving(creeping_zoom_query(WINDOW)))
        .expect("motion query");
        zoom.assert_sampled_across(WINDOW);
        assert!(moving);
        session.shutdown().expect("shutdown");
    }
}
