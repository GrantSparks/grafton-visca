//! Sony UDP retransmission distinguishes a lost inquiry from a lost reply.

#![cfg(any(
    feature = "runtime-tokio",
    all(
        feature = "test-utils",
        any(feature = "blocking", feature = "runtime-smol")
    )
))]
#![allow(clippy::expect_used, clippy::panic)]

use std::time::Duration;

use grafton_visca::{
    command::zoom::Tele,
    config::{OperationalTuning, SessionConfig, TransportKind},
    operation::AppliedOnly,
    profiles::SonyFr7,
    raw::InquiryRequest,
    request::{InquiryRetryClass, SubmissionClass},
    tuning::RetryLimit,
    types::{IndividualCameraId, PowerStatus, VersionInfo},
    Error,
};
use grafton_visca_test_support::{
    facade_matrix,
    fake_camera::{frames, FakeCamera},
};

const POWER_0: [u8; 13] = [1, 0x10, 0, 5, 0, 0, 0, 0, 0x81, 0x09, 0x04, 0, 0xff];
const POWER_1: [u8; 13] = [1, 0x10, 0, 5, 0, 0, 0, 1, 0x81, 0x09, 0x04, 0, 0xff];
const POWER_2: [u8; 13] = [1, 0x10, 0, 5, 0, 0, 0, 2, 0x81, 0x09, 0x04, 0, 0xff];
const VERSION_0: [u8; 13] = [1, 0x20, 0, 5, 0, 0, 0, 0, 0x81, 0x09, 0, 2, 0xff];
const VERSION_1: [u8; 13] = [1, 0x20, 0, 5, 0, 0, 0, 1, 0x81, 0x09, 0, 2, 0xff];
const ZOOM_0: [u8; 14] = [1, 0, 0, 6, 0, 0, 0, 0, 0x81, 1, 4, 7, 2, 0xff];
const DUPLICATE_0: [u8; 10] = [2, 0, 0, 2, 0, 0, 0, 0, 0x0f, 1];
const TYPE_ERROR_99: [u8; 10] = [2, 0, 0, 2, 0, 0, 0, 99, 0x0f, 2];
const VERSION_REPLY: [u8; 10] = [0x90, 0x50, 0, 1, 5, 0x1e, 0x12, 0x34, 2, 0xff];
const VERSION: VersionInfo = VersionInfo {
    vendor: 1,
    model: 0x051e,
    rom_version: 0x1234,
    max_socket: 2,
};

struct SonyUdp(FakeCamera);

impl SonyUdp {
    #[cfg(feature = "blocking")]
    fn blocking_wire(&self) -> grafton_visca_test_support::fake_camera::BlockingWire {
        self.0
            .blocking_wire()
            .with_transport_kind(TransportKind::Udp)
    }

    #[cfg(any(feature = "runtime-tokio", feature = "runtime-smol"))]
    fn async_wire(&self) -> grafton_visca_test_support::fake_camera::AsyncWire {
        self.0.async_wire().with_transport_kind(TransportKind::Udp)
    }
}

fn config() -> SessionConfig {
    SessionConfig::from_compile_time::<SonyFr7>()
        .expect("FR7 config")
        .with_tuning(
            OperationalTuning::new()
                .with_retry_limit(RetryLimit::new(0).expect("zero base retry limit")),
        )
}

#[track_caller]
fn assert_inquiry_timeout(error: &Error) {
    assert!(
        matches!(error, Error::InquiryFailed { source, .. } if matches!(source.as_ref(), Error::Timeout { .. })),
        "{error:?}"
    );
}

facade_matrix! {
    paused:

    fn a_lost_inquiry_recovers_on_the_first_exact_replay() {
        let mut writes = 0;
        let peer = SonyUdp(FakeCamera::new(move |write, answer| {
            assert_eq!(write, POWER_0);
            writes += 1;
            if writes == 2 {
                answer.reply(frames::sony_reply(0, &frames::inquiry_reply(&[2])));
            }
        }));
        let session = open!(peer, config()).expect("Sony UDP session");
        let camera = session.camera::<SonyFr7>(IndividualCameraId::CAMERA_1).expect("FR7 view");
        assert_eq!(wait!(camera.power().status()).expect("replayed inquiry"), PowerStatus::On);
        assert_eq!(peer.0.writes(), [POWER_0, POWER_0]);
        wait!(session.close()).expect("close FR7 session");
    }

    fn a_lost_reply_recovers_on_a_fresh_sequence_amid_other_input() {
        let mut power_zero_writes = 0;
        let mut version_writes = 0;
        let peer = SonyUdp(FakeCamera::new(move |write, answer| {
            if write == POWER_0 {
                power_zero_writes += 1;
                if power_zero_writes == 1 {
                    // The camera performed the inquiry but only this unrelated
                    // diagnostic reaches the owner during the first attempt.
                    answer.reply(TYPE_ERROR_99);
                } else {
                    assert!(version_writes > 0);
                    answer.reply(DUPLICATE_0);
                }
            } else if write == VERSION_1 {
                version_writes += 1;
                if version_writes == 2 {
                    answer.reply(frames::sony_reply(1, &VERSION_REPLY));
                }
            } else if write == POWER_2 {
                // Deliver the stale reply after the fresh inquiry is on the
                // wire. Its different value detects accidental attribution.
                answer
                    .reply(frames::sony_reply(0, &frames::inquiry_reply(&[3])))
                    .reply(frames::sony_reply(2, &frames::inquiry_reply(&[2])));
            } else {
                panic!("unexpected Sony datagram {write:02x?}");
            }
        }));
        let session = open!(peer, config()).expect("Sony UDP session");
        let power = session.camera::<SonyFr7>(IndividualCameraId::CAMERA_1).expect("FR7 power view");
        let system = session.camera::<SonyFr7>(IndividualCameraId::CAMERA_1).expect("FR7 system view");
        let (status, version) = together!({
            wait!(power.power().status())
        }, {
            wait_for_writes!(peer.0, 1);
            wait!(system.system().version())
        });
        assert_eq!(status.expect("fresh power reply"), PowerStatus::On);
        assert_eq!(version.expect("other inquiry's reply"), VERSION);
        assert_eq!(peer.0.writes(), [POWER_0, VERSION_1, POWER_0, VERSION_1, POWER_2]);
        wait!(session.close()).expect("close FR7 session");
    }

    fn a_device_setting_inquiry_keeps_its_envelope_on_the_fresh_attempt() {
        let mut initial_writes = 0;
        let peer = SonyUdp(FakeCamera::new(move |write, answer| {
            if write == VERSION_0 {
                initial_writes += 1;
                if initial_writes == 2 {
                    answer.reply(DUPLICATE_0);
                }
            } else {
                assert_eq!(write, VERSION_1);
                answer.reply(frames::sony_reply(1, &VERSION_REPLY));
            }
        }));
        let session = open!(peer, config()).expect("Sony UDP session");
        let camera = session.camera::<SonyFr7>(IndividualCameraId::CAMERA_1).expect("FR7 view");
        assert_eq!(wait!(camera.system().version()).expect("fresh version reply"), VERSION);
        assert_eq!(peer.0.writes(), [VERSION_0, VERSION_0, VERSION_1]);
        wait!(session.close()).expect("close FR7 session");
    }

    fn a_fresh_attempt_uses_the_original_retry_count_and_deadline() {
        let peer = SonyUdp(FakeCamera::silent());
        let session = open!(peer, config()).expect("Sony UDP session");
        let camera = session.camera::<SonyFr7>(IndividualCameraId::CAMERA_1).expect("FR7 view");
        assert_inquiry_timeout(&wait!(camera.power().status()).expect_err("inquiry timed out"));
        assert_eq!(peer.0.writes(), [POWER_0, POWER_0, POWER_1], "initial write and two retries");
        wait!(session.close()).expect("close FR7 session");

        let peer = SonyUdp(FakeCamera::silent());
        // The first replay fits for the entire jitter range. The following
        // backoff would place even the earliest fresh write past the budget.
        let tuned = config().with_tuning(
            OperationalTuning::new()
                .with_inquiry_timeout(Duration::from_secs(3))
                .with_initial_retry_backoff(Duration::from_secs(3))
                .with_maximum_retry_backoff(Duration::from_secs(6))
                .with_retry_budget(Duration::from_secs(10)),
        );
        let session = open!(peer, tuned).expect("limited Sony UDP session");
        let camera = session.camera::<SonyFr7>(IndividualCameraId::CAMERA_1).expect("FR7 view");
        assert_inquiry_timeout(&wait!(camera.power().status()).expect_err("inquiry timed out"));
        assert_eq!(peer.0.writes(), [POWER_0, POWER_0], "deadline prevents a fresh attempt");
        wait!(session.close()).expect("close FR7 session");
    }

    fn an_unreviewed_inquiry_and_an_ack_lost_drive_never_get_fresh_sequences() {
        let mut writes = 0;
        let peer = SonyUdp(FakeCamera::new(move |write, answer| {
            assert_eq!(write, POWER_0);
            writes += 1;
            if writes > 1 {
                answer.reply(DUPLICATE_0);
            }
        }));
        let session = open!(peer, config()).expect("Sony UDP session");
        let camera = session.camera::<SonyFr7>(IndividualCameraId::CAMERA_1).expect("FR7 view");
        let unreviewed = InquiryRequest::from_fn(
            [0x81, 9, 4, 0, 0xff],
            |_| Ok(()),
            InquiryRetryClass::Inquiry,
            SubmissionClass::User,
        ).expect("raw inquiry");
        assert_inquiry_timeout(&wait!(camera.inquire(&unreviewed)).expect_err("raw inquiry timed out"));
        assert_eq!(peer.0.writes(), [POWER_0; 3]);
        wait!(session.close()).expect("close FR7 session");

        let mut writes = 0;
        let peer = SonyUdp(FakeCamera::new(move |write, answer| {
            assert_eq!(write, ZOOM_0);
            writes += 1;
            if writes > 1 {
                answer.reply(DUPLICATE_0);
            }
        }));
        let session = open!(peer, SessionConfig::from_compile_time::<SonyFr7>().expect("FR7 config")).expect("Sony UDP session");
        let camera = session.camera::<SonyFr7>(IndividualCameraId::CAMERA_1).expect("FR7 view");
        let mut drive = wait!(camera.submit::<AppliedOnly, _>(&Tele)).expect("movement admission");
        assert!(wait!(drive.applied()).is_err());
        let writes = peer.0.writes();
        assert!(writes.len() >= 3, "both first and subsequent retries must be exercised");
        assert!(writes.iter().all(|write| write == &ZOOM_0));
        wait!(session.close()).expect("close FR7 session");
    }
}
