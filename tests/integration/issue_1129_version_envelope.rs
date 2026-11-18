//! Sony device-setting framing through the owner on each facade.

#![cfg(any(
    feature = "runtime-tokio",
    all(
        feature = "test-utils",
        any(feature = "blocking", feature = "runtime-smol")
    )
))]
#![allow(clippy::expect_used)]

use grafton_visca::{
    command::system::VersionInquiry,
    config::SessionConfig,
    profiles::{GenericVisca, SonyFr7},
    types::{IndividualCameraId, PowerStatus, VersionInfo},
};
use grafton_visca_test_support::{
    facade_matrix,
    fake_camera::{frames, FakeCamera},
};

const VERSION_REPLY: [u8; 10] = [0x90, 0x50, 0, 1, 0x05, 0x1e, 0x12, 0x34, 2, 0xff];
const VERSION: VersionInfo = VersionInfo {
    vendor: 0x0001,
    model: 0x051e,
    rom_version: 0x1234,
    max_socket: 2,
};
const RAW_VERSION: [u8; 5] = [0x81, 0x09, 0x00, 0x02, 0xff];
const FACADE_VERSION: [u8; 13] = [1, 0x20, 0, 5, 0, 0, 0, 0, 0x81, 0x09, 0, 0x02, 0xff];
const DIRECT_VERSION: [u8; 13] = [1, 0x20, 0, 5, 0, 0, 0, 1, 0x81, 0x09, 0, 0x02, 0xff];
const POWER_INQUIRY: [u8; 13] = [1, 0x10, 0, 5, 0, 0, 0, 2, 0x81, 0x09, 0x04, 0, 0xff];
const POWER_ON: [u8; 14] = [1, 0x00, 0, 6, 0, 0, 0, 3, 0x81, 0x01, 0x04, 0, 2, 0xff];

facade_matrix! {
    paused:

    fn fr7_version_uses_device_setting_framing_on_first_write_and_retry() {
        let mut direct_attempts = 0;
        let fake = FakeCamera::new(move |write, answer| {
            // Each reply is conditional on the entire header and body matching.
            if write == FACADE_VERSION {
                answer.reply(frames::sony_reply(0, &VERSION_REPLY));
            } else if write == DIRECT_VERSION {
                direct_attempts += 1;
                if direct_attempts == 2 {
                    answer.reply(frames::sony_reply(1, &VERSION_REPLY));
                }
            } else if write == POWER_INQUIRY {
                answer.reply(frames::sony_reply(2, &frames::inquiry_reply(&[2])));
            } else if write == POWER_ON {
                answer
                    .reply(frames::sony_reply(3, &frames::ack(1)))
                    .reply(frames::sony_reply(3, &frames::complete(1)));
            }
        });
        let config = SessionConfig::from_compile_time::<SonyFr7>().expect("FR7 config");
        let session = open!(fake, config).expect("FR7 session");
        let camera = session.camera::<SonyFr7>(IndividualCameraId::CAMERA_1).expect("FR7 view");
        assert_eq!(wait!(camera.system().version()).expect("facade version"), VERSION);
        assert_eq!(fake.writes(), [FACADE_VERSION]);
        assert_eq!(wait!(camera.inquire(&VersionInquiry)).expect("direct version after retry"), VERSION);
        assert_eq!(wait!(camera.power().status()).expect("ordinary inquiry"), PowerStatus::On);
        wait!(camera.power().on()).expect("ordinary command");
        assert_eq!(fake.writes(), [
            FACADE_VERSION.to_vec(),
            DIRECT_VERSION.to_vec(),
            DIRECT_VERSION.to_vec(),
            POWER_INQUIRY.to_vec(),
            POWER_ON.to_vec(),
        ]);
        wait!(session.close()).expect("close FR7 session");
    }

    fn raw_version_has_no_sony_envelope() {
        let fake = FakeCamera::new(|write, answer| {
            if write == RAW_VERSION {
                answer.reply(VERSION_REPLY);
            }
        });
        let config = SessionConfig::from_compile_time::<GenericVisca>().expect("raw VISCA config");
        let session = open!(fake, config).expect("raw VISCA session");
        let camera = session.camera::<GenericVisca>(IndividualCameraId::CAMERA_1).expect("raw VISCA view");
        assert_eq!(wait!(camera.inquire(&VersionInquiry)).expect("raw version"), VERSION);
        assert_eq!(fake.writes(), [RAW_VERSION]);
        wait!(session.close()).expect("close raw VISCA session");
    }
}
