//! Deferred polling keeps the operation's submission-time tuning on every facade.
#![cfg(any(
    feature = "runtime-tokio",
    all(
        feature = "test-utils",
        any(feature = "blocking", feature = "runtime-smol")
    )
))]

use grafton_visca::{
    config::{OperationalTuning, SessionConfig},
    operation::Settlement,
    profiles::GenericVisca,
    types::CameraId,
};
use grafton_visca_test_support::{
    facade_matrix,
    fake_camera::{self, FakeCamera},
};
use std::time::Duration;

facade_matrix! {
    paused:

    fn lazy_settlement_keeps_tuning_from_submission_after_live_reconfiguration() {
        let fake = FakeCamera::visca(|request, answer| {
            if request == [0x81, 9, 6, 0x12, 0xFF] {
                answer.reply([0x90, 0x50, 0, 0, 0, 0, 0, 0, 0, 0, 0xFF]);
            } else {
                answer.reply(fake_camera::frames::ack(1)).reply(fake_camera::frames::complete(1));
            }
        });
        let old_spacing = Duration::from_millis(40);
        let config = SessionConfig::from_compile_time::<GenericVisca>().expect("config")
            .with_tuning(OperationalTuning::new().with_inquiry_spacing(old_spacing));
        let session = open!(fake, config).expect("session");
        let camera = session.camera::<GenericVisca>(CameraId::CAMERA_1).expect("camera");
        let mut operation = wait!(camera.pan_tilt().home()).expect("home admitted");
        wait!(operation.applied()).expect("home applied");
        assert_eq!(fake.write_count(), 1, "application does not send settlement inquiries");
        let new_spacing = Duration::from_millis(250);
        wait!(session.set_tuning(OperationalTuning::new().with_inquiry_spacing(new_spacing)
            .with_inquiry_timeout(Duration::from_secs(20)))).expect("updated tuning");
        let settled = wait!(operation.settled()).expect("old operation settles");
        assert!(matches!(settled, Settlement::ObservedStable { window, .. } if window == old_spacing));
        assert_eq!(fake.write_count(), 3, "exactly two pan/tilt samples");
        wait!(operation.settled()).expect("cached settlement evidence");
        assert_eq!(fake.write_count(), 3, "settled cache requires no more samples");
        let mut next = wait!(camera.pan_tilt().home()).expect("new operation");
        let settled = wait!(next.settled()).expect("new operation settles");
        assert!(matches!(settled, Settlement::ObservedStable { window, .. } if window == new_spacing));
        wait!(session.close()).expect("close");
    }
}
