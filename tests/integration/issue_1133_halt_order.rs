//! A halt's axis STOP keeps wire precedence after a camera buffer rejection.

#![cfg(any(
    feature = "runtime-tokio",
    all(
        feature = "test-utils",
        any(feature = "blocking", feature = "runtime-smol")
    )
))]
#![allow(clippy::expect_used)]

use grafton_visca::{
    config::{SessionConfig, TransportKind},
    operation::HaltOutcome,
    profiles::SonyEviH100,
    transport::{AddressingMode, SendSemantics},
    types::{IndividualCameraId, ZoomSpeed},
    Error,
};
use grafton_visca_test_support::{
    facade_matrix,
    fake_camera::{self, frames, FakeCamera, FOCUS_STOP, ZOOM_STOP},
};

const PAN_TILT_STOP: &[u8] = &[0x81, 0x01, 0x06, 0x01, 0x0c, 0x0a, 0x03, 0x03, 0xff];
const ZOOM_TELE: &[u8] = &[0x81, 0x01, 0x04, 0x07, 0x20, 0xff];

struct SerialCamera(FakeCamera);

impl SerialCamera {
    #[cfg(all(feature = "blocking", feature = "test-utils"))]
    fn blocking_wire(&self) -> fake_camera::BlockingWire {
        self.0
            .blocking_wire()
            .with_transport_kind(TransportKind::Serial)
            .with_semantics(SendSemantics::Stream)
            .with_addressing(AddressingMode::Serial)
    }

    #[cfg(any(feature = "runtime-tokio", feature = "runtime-smol"))]
    fn async_wire(&self) -> fake_camera::AsyncWire {
        self.0
            .async_wire()
            .with_transport_kind(TransportKind::Serial)
            .with_semantics(SendSemantics::Stream)
            .with_addressing(AddressingMode::Serial)
    }
}

facade_matrix! {
    paused:

    fn zoom_halt_orders_its_retry_before_the_later_zoom_drive() {
        for retryable in [true, false] {
            // Socket 1 is occupied before the transcript. Pan/tilt occupies
            // socket 2 until the zoom STOP receives the first refusal.
            let mut first_zoom_stop = true;
            let link = SerialCamera(FakeCamera::new(move |packet, answer| match packet {
                PAN_TILT_STOP => { answer.reply(frames::ack(2)); }
                ZOOM_STOP if std::mem::take(&mut first_zoom_stop) => {
                    if retryable {
                        answer.reply([0x90, 0x60, 0x03, 0xff]);
                    } else {
                        answer.reply([0x90, 0x60, 0x41, 0xff]);
                    }
                    answer.reply(frames::complete(2));
                }
                ZOOM_STOP | FOCUS_STOP | ZOOM_TELE => {
                    answer.reply(frames::ack(2)).reply(frames::complete(2));
                }
                _ => {}
            }));
            let config = SessionConfig::from_compile_time::<SonyEviH100>().expect("EVI config");
            let session = open!(link, config).expect("EVI serial session");
            let halt_view = session.camera::<SonyEviH100>(IndividualCameraId::CAMERA_1)
                .expect("halt producer");
            let drive_view = session.camera::<SonyEviH100>(IndividualCameraId::CAMERA_1)
                .expect("drive producer");
            let (halt, ()) = together!({
                wait!(halt_view.motion().stop_all()).expect("accepted halt")
            }, {
                wait_for_writes!(link.0, 2);
                let mut drive = wait!(drive_view.zoom().tele_variable(ZoomSpeed::ZERO))
                    .expect("later zoom drive admitted");
                wait!(drive.applied()).expect("later zoom drive applied");
            });

            assert!(matches!(halt.pan_tilt, HaltOutcome::Applied), "{halt:?}");
            assert!(matches!(halt.focus, HaltOutcome::Applied), "{halt:?}");
            let mut expected = vec![PAN_TILT_STOP, ZOOM_STOP, FOCUS_STOP];
            if retryable {
                assert!(matches!(halt.zoom, HaltOutcome::Applied), "{halt:?}");
                expected.push(ZOOM_STOP);
            } else {
                assert!(matches!(halt.zoom, HaltOutcome::Failed(Error::CommandNotExecutable)), "{halt:?}");
            }
            expected.push(ZOOM_TELE);
            assert_eq!(link.0.take_payloads(), expected);
            wait!(session.close()).expect("close EVI session");
        }
    }
}
