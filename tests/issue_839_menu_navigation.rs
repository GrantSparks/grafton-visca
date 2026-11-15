//! Menu navigation is physical pan/tilt motion when the camera menu is closed.
//! Exercise its applied handle and halt fence through each owner facade.

#![cfg(any(
    feature = "blocking",
    feature = "runtime-tokio",
    feature = "runtime-smol"
))]

use std::time::Duration;

use grafton_visca::{
    command::MenuDirection, profiles::PtzOpticsG2, transport::SendSemantics, Certainty,
    FailureContext, FailureStage, ProfileSpec, SessionConfig,
};
use grafton_visca_test_support::{facade_matrix, fake_camera};

use fake_camera::{frames, FakeCamera, FOCUS_STOP, ZOOM_STOP};

const NAVIGATE_UP: &[u8] = &[0x81, 0x01, 0x06, 0x01, 0x0e, 0x0e, 0x03, 0x01, 0xff];

struct DatagramCamera(FakeCamera);

impl DatagramCamera {
    #[cfg(feature = "blocking")]
    fn blocking_wire(&self) -> fake_camera::BlockingWire {
        self.0
            .blocking_wire()
            .with_semantics(SendSemantics::Datagram)
    }

    #[cfg(any(feature = "runtime-tokio", feature = "runtime-smol"))]
    fn async_wire(&self) -> fake_camera::AsyncWire {
        self.0.async_wire().with_semantics(SendSemantics::Datagram)
    }
}

fn config() -> SessionConfig {
    SessionConfig::new(ProfileSpec::from_compile_time::<PtzOpticsG2>().expect("G2 profile"))
}

facade_matrix! {
    paused:

    fn navigation_has_an_applied_handle_and_needs_an_explicit_stop() {
        let fake = FakeCamera::acking(1);
        let session = open!(fake, config()).expect("session");
        let camera = session.camera::<PtzOpticsG2>().expect("camera");
        let mut navigation = wait!(camera.menu().navigate(MenuDirection::Up))
            .expect("navigation admitted");
        wait!(navigation.applied()).expect("navigation applied");
        assert_eq!(fake.writes(), vec![NAVIGATE_UP.to_vec()]);

        let mut stop = wait!(camera.pan_tilt().stop()).expect("explicit stop admitted");
        wait!(stop.applied()).expect("stop applied");
        let writes = fake.writes();
        assert_eq!(writes.len(), 2);
        assert_eq!(&writes[1][6..], &[0x03, 0x03, 0xff]);
        session.shutdown().expect("shutdown");
    }

    fn halt_fences_admitted_navigation_before_its_write() {
        // Withhold the first ordinary command's ACK so navigation must queue.
        // The independently dispatched halt STOPs each get their own answer.
        let fake = DatagramCamera(FakeCamera::new(|bytes, answer| {
            let pan_tilt_stop = bytes.get(2..4) == Some(&[0x06, 0x01])
                && bytes.get(6..8) == Some(&[0x03, 0x03]);
            if pan_tilt_stop || bytes == ZOOM_STOP || bytes == FOCUS_STOP {
                answer.reply(frames::ack(2)).reply(frames::complete(2));
            }
        }));
        let session = open!(fake, config()).expect("session");
        let camera = session.camera::<PtzOpticsG2>().expect("camera");
        let _first = wait!(camera.zoom().tele()).expect("first command admitted");
        wait_for_writes!(fake.0, 1);
        pause!(Duration::from_millis(50));
        let mut navigation = wait!(camera.menu().navigate(MenuDirection::Up))
            .expect("navigation admitted behind first command");

        let _halt = wait!(camera.motion().stop_all_motion()).expect("halt admitted");
        let error = wait!(navigation.applied()).expect_err("halt supersedes navigation");
        assert!(matches!(error, grafton_visca::Error::MotionSuperseded { .. }));
        assert_eq!(
            error.failure_context(),
            Some(FailureContext::new(FailureStage::Terminal, Certainty::NotAccepted)),
        );
        assert!(fake.0.writes().iter().all(|bytes| bytes != NAVIGATE_UP));
        session.shutdown().expect("shutdown");
    }
}
