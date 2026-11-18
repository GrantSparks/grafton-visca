//! The registered FR7 cancellation rule is shared by static and dynamic views.

#![cfg(any(
    feature = "runtime-tokio",
    all(
        feature = "test-utils",
        any(feature = "blocking", feature = "runtime-smol")
    )
))]
#![allow(clippy::expect_used)]

use std::time::Duration;

use grafton_visca::{
    config::SessionConfig,
    operation::CancellationOutcome,
    profiles::{AnyProfile, SonyFr7},
    types::{IndividualCameraId, PanTiltDirection, SpeedLevel},
};
use grafton_visca_test_support::{
    facade_matrix,
    fake_camera::{frames, FakeCamera},
};

const RIGHT: &[u8] = &[0x81, 0x01, 0x06, 0x01, 0x01, 0x01, 0x02, 0x03, 0xff];
const CANCEL: &[u8] = &[0x81, 0x21, 0xff];
const STOP: &[u8] = &[0x81, 0x01, 0x06, 0x01, 0x0c, 0x0a, 0x03, 0x03, 0xff];

facade_matrix! {
    paused:

    fn fr7_static_drive_and_dynamic_stop_share_cancel_timing() {
        let fake = FakeCamera::visca(|write, answer| match write {
            RIGHT => {
                answer.reply(frames::ack(1));
            }
            CANCEL => {
                answer.reply(frames::canceled(1));
            }
            STOP => {
                answer.reply(frames::ack(1)).reply(frames::complete(1));
            }
            other => panic!("unexpected FR7 frame {other:02x?}"),
        });
        let config = SessionConfig::from_compile_time::<SonyFr7>().expect("FR7 config");
        let session = open!(fake, config).expect("FR7 session");
        let static_camera = session.camera::<SonyFr7>(IndividualCameraId::CAMERA_1)
            .expect("static FR7");
        let dynamic_camera = session.camera::<AnyProfile>(IndividualCameraId::CAMERA_1)
            .expect("dynamic FR7");
        let start = now!();
        let mut drive = wait!(static_camera.pan_tilt().drive(PanTiltDirection::Right, SpeedLevel::Slowest))
            .expect("FR7 drive admitted");
        wait_for_writes!(fake, 1);
        wait_for_reads!(fake, 1);

        assert_eq!(
            wait!(drive.cancel()).expect("drive cancel confirmed"),
            CancellationOutcome::Cancelled,
        );
        assert!(now!().duration_since(start) >= Duration::from_millis(200));
        let mut stop = wait!(dynamic_camera.pan_tilt().stop()).expect("FR7 stop admitted");
        wait!(stop.applied()).expect("FR7 stop applied");

        assert!(now!().duration_since(start) >= Duration::from_millis(400));
        assert_eq!(fake.take_payloads(), [RIGHT, CANCEL, STOP]);
        wait!(session.close()).expect("FR7 session closed");
    }
}
