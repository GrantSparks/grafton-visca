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
    command::menu::Navigate,
    config::{SessionConfig, TransportKind},
    error::{Certainty, ErrorKind, FailureContext, FailureStage, Recovery},
    operation::CancellationOutcome,
    profiles::{AnyProfile, SonyBrcH900, SonyFr7},
    types::{IndividualCameraId, MenuDirection, PanTiltDirection, SpeedLevel},
    Error,
};
use grafton_visca_test_support::{
    facade_matrix,
    fake_camera::{self, frames, FakeCamera},
};

const RIGHT: &[u8] = &[0x81, 0x01, 0x06, 0x01, 0x01, 0x01, 0x02, 0x03, 0xff];
const CANCEL: &[u8] = &[0x81, 0x21, 0xff];
const STOP: &[u8] = &[0x81, 0x01, 0x06, 0x01, 0x0c, 0x0a, 0x03, 0x03, 0xff];

const NAVIGATE_UP: &[u8] = &[0x81, 0x01, 0x06, 0x01, 0x0e, 0x0e, 0x03, 0x01, 0xff];
const POWER_INQUIRY: &[u8] = &[0x81, 0x09, 0x04, 0x00, 0xff];
const DRIVE_INTERVAL: Duration = Duration::from_millis(200);

struct UdpCamera(FakeCamera);

impl UdpCamera {
    #[cfg(all(feature = "blocking", feature = "test-utils"))]
    fn blocking_wire(&self) -> fake_camera::BlockingWire {
        self.0
            .blocking_wire()
            .with_transport_kind(TransportKind::Udp)
    }

    #[cfg(any(feature = "runtime-tokio", feature = "runtime-smol"))]
    fn async_wire(&self) -> fake_camera::AsyncWire {
        self.0.async_wire().with_transport_kind(TransportKind::Udp)
    }
}

fn assert_restart_blocked(error: &Error, stage: FailureStage) {
    let context = FailureContext::new(stage, Certainty::NotAccepted);
    assert!(
        matches!(
            error,
            Error::PanTiltDriveRestartBlocked { camera, context: actual, .. }
                if *camera == IndividualCameraId::CAMERA_1 && *actual == context
        ),
        "{error:?}"
    );
    assert_eq!(error.failure_context(), Some(context));
    assert_eq!(error.kind(), ErrorKind::NotExecutable);
    assert_eq!(error.recovery(), Recovery::Abandon);
    assert!(!error.requires_new_session());
}

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

    fn fr7_menu_navigation_shares_both_drive_intervals() {
        for menu_first in [true, false] {
            let (first, second) = if menu_first { (NAVIGATE_UP, STOP) } else { (RIGHT, NAVIGATE_UP) };
            let fake = UdpCamera(FakeCamera::visca(move |write, answer| {
                if write == first {
                    answer.reply(frames::ack(1));
                } else if write == second {
                    answer.reply(frames::ack(1)).reply(frames::complete(1));
                } else if write == POWER_INQUIRY {
                    answer.reply(frames::inquiry_reply(&[0x02]));
                } else {
                    assert_eq!(write, CANCEL);
                }
            }));
            let config = SessionConfig::from_compile_time::<SonyFr7>().expect("FR7 config");
            let session = open!(fake, config).expect("FR7 session");
            let static_camera = session.camera::<SonyFr7>(IndividualCameraId::CAMERA_1)
                .expect("static FR7");
            let dynamic_camera = session.camera::<AnyProfile>(IndividualCameraId::CAMERA_1)
                .expect("dynamic FR7");

            let start = now!();
            let mut operation = if menu_first {
                wait!(static_camera.submit(&Navigate::new(MenuDirection::Up)))
            } else {
                wait!(static_camera.pan_tilt().drive(PanTiltDirection::Right, SpeedLevel::Slowest))
            }.expect("drive admitted");
            wait_for_reads!(fake.0, 1);
            assert!(matches!(
                wait!(operation.cancel_with_timeout(Duration::from_millis(45))),
                Err(Error::ObservationTimeout { .. })
            ));
            assert_eq!(fake.0.write_count(), 1, "cancel remains below the drive interval");

            let mut next = if menu_first {
                wait!(dynamic_camera.pan_tilt().stop())
            } else {
                wait!(dynamic_camera.menu().navigate(MenuDirection::Up))
            }.expect("next drive admitted");
            assert!(wait!(static_camera.power().status()).expect("unrelated inquiry").is_on());
            assert_eq!(fake.0.write_count(), 2, "only the unrelated inquiry was written");
            wait_for_writes!(fake.0, 3);
            let cancel_seen = now!();
            assert!(cancel_seen.duration_since(start) >= DRIVE_INTERVAL);
            let cancel_sequence = frames::sony_sequence(&fake.0.writes()[2])
                .expect("Sony-encapsulated cancel");

            if menu_first {
                fake.0.push(frames::sony_reply(cancel_sequence, &frames::canceled(1)));
                pause!(Duration::from_millis(140));
                assert_eq!(fake.0.write_count(), 3, "early confirmation leaves the restart interval");
            } else {
                pause!(Duration::from_millis(220));
                assert_eq!(fake.0.write_count(), 3, "restart interval alone is insufficient");
                fake.0.push(frames::sony_reply(cancel_sequence, &frames::canceled(1)));
            }

            assert!(matches!(wait!(operation.applied()), Err(Error::CommandCancelled)));
            wait!(next.applied()).expect("next drive permitted after interval and confirmation");
            // The write watcher can report the cancel up to one polling slice after it left.
            assert!(now!().duration_since(cancel_seen) >= Duration::from_millis(180));
            assert_eq!(fake.0.take_payloads(), [first, POWER_INQUIRY, CANCEL, second]);
            wait!(session.close()).expect("FR7 session closed");
        }
    }

    fn inconclusive_fr7_menu_cancel_refuses_queued_and_fresh_drives() {
        let fake = UdpCamera(FakeCamera::visca(|write, answer| match write {
            NAVIGATE_UP => {
                answer.reply(frames::ack(1));
            }
            CANCEL => {
                answer.reply(frames::error(1, 0x05));
            }
            POWER_INQUIRY => {
                answer.reply(frames::inquiry_reply(&[0x02]));
            }
            other => panic!("unexpected FR7 frame {other:02x?}"),
        }));
        let config = || SessionConfig::from_compile_time::<SonyFr7>().expect("FR7 config");
        let session = open!(fake, config()).expect("FR7 session");
        let camera = session.camera::<SonyFr7>(IndividualCameraId::CAMERA_1).expect("static FR7");
        let dynamic = session.camera::<AnyProfile>(IndividualCameraId::CAMERA_1).expect("dynamic FR7");
        let mut first = wait!(camera.menu().navigate(MenuDirection::Up)).expect("navigation admitted");
        wait_for_reads!(fake.0, 1);
        assert!(matches!(
            wait!(first.cancel_with_timeout(Duration::from_millis(45))),
            Err(Error::ObservationTimeout { .. })
        ));
        let mut queued = wait!(dynamic.menu().navigate(MenuDirection::Up)).expect("queued menu drive");
        let terminal = wait!(queued.applied()).expect_err("unconfirmed cancellation blocks dispatch");
        assert_restart_blocked(&terminal, FailureStage::Terminal);
        let refusal = wait!(camera.pan_tilt().stop()).expect_err("unconfirmed cancellation blocks admission");
        assert_restart_blocked(&refusal, FailureStage::PreAdmission);

        assert!(wait!(camera.power().status()).expect("unrelated inquiry still usable").is_on());
        assert_eq!(fake.0.take_payloads(), [NAVIGATE_UP, CANCEL, POWER_INQUIRY]);
        wait!(session.close()).expect("FR7 session closed");

        let reopened = UdpCamera(FakeCamera::visca(|write, answer| {
            assert_eq!(write, RIGHT);
            answer.reply(frames::ack(1)).reply(frames::complete(1));
        }));
        let session = open!(reopened, config()).expect("new FR7 session");
        let camera = session.camera::<SonyFr7>(IndividualCameraId::CAMERA_1).expect("reopened camera");
        let mut drive = wait!(camera.pan_tilt().drive(PanTiltDirection::Right, SpeedLevel::Slowest))
            .expect("drive admitted on new session");
        wait!(drive.applied()).expect("reopened drive applied");
        assert_eq!(reopened.0.take_payloads(), [RIGHT]);
        wait!(session.close()).expect("new FR7 session closed");
    }

    fn brc_h900_menu_uses_ordinary_cancel_spacing() {
        let fake = UdpCamera(FakeCamera::visca(|write, answer| match write {
            NAVIGATE_UP => {
                answer.reply(frames::ack(1));
            }
            CANCEL => {
                answer.reply(frames::canceled(1));
            }
            other => panic!("unexpected BRC-H900 frame {other:02x?}"),
        }));
        let config = SessionConfig::from_compile_time::<SonyBrcH900>().expect("BRC-H900 config");
        let session = open!(fake, config).expect("BRC-H900 session");
        let camera = session.camera::<SonyBrcH900>(IndividualCameraId::CAMERA_1).expect("BRC-H900");
        let start = now!();
        let mut operation = wait!(camera.menu().navigate(MenuDirection::Up)).expect("navigation admitted");
        wait_for_reads!(fake.0, 1);
        assert_eq!(wait!(operation.cancel()).expect("cancel confirmed"), CancellationOutcome::Cancelled);
        let elapsed = now!().duration_since(start);
        assert!(elapsed >= Duration::from_millis(35));
        assert!(elapsed < DRIVE_INTERVAL, "ordinary spacing: {elapsed:?}");
        assert_eq!(fake.0.take_payloads(), [NAVIGATE_UP, CANCEL]);
        wait!(session.close()).expect("BRC-H900 session closed");
    }
}
