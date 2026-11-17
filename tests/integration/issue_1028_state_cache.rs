//! Write-only WDR and motion-sync setters preserve exactly their applied values.

#![cfg(any(
    feature = "blocking",
    feature = "runtime-tokio",
    feature = "runtime-smol"
))]

use grafton_visca::{
    config::SessionConfig,
    profiles::PtzOpticsG2,
    transport::SendSemantics,
    types::{IndividualCameraId, WideDynamicRangeLevel},
};
use grafton_visca_test_support::{
    facade_matrix,
    fake_camera::{self, frames, FakeCamera},
    profile_fixtures::MotionSyncTypedSupport,
};

#[derive(Clone, Copy, Debug)]
enum Outcome {
    Applied,
    Rejected,
    Unconfirmed,
}

struct Wire(FakeCamera);

impl Wire {
    fn new(outcome: Outcome) -> Self {
        Self(FakeCamera::visca(move |_, answer| match outcome {
            Outcome::Applied => {
                answer.reply(frames::ack(1)).reply(frames::complete(1));
            }
            Outcome::Rejected => {
                answer.reply([0x90, 0x60, 0x02, 0xff]);
            }
            Outcome::Unconfirmed => {
                answer.fail_send(std::io::Error::other("uncertain camera write").into());
            }
        }))
    }

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

facade_matrix! {
    fn wide_dynamic_range_cache_records_only_an_applied_setter() {
        for outcome in [Outcome::Applied, Outcome::Rejected, Outcome::Unconfirmed] {
            let wire = Wire::new(outcome);
            let config = SessionConfig::from_compile_time::<PtzOpticsG2>().unwrap();
            let session = open!(wire, config).unwrap();
            let camera = session.camera::<PtzOpticsG2>(IndividualCameraId::CAMERA_1).unwrap();
            let cache = camera.state_cache();
            assert_eq!(cache.exposure().wide_dynamic_range(), None);
            for level in [0, 4, 8] {
                let level = WideDynamicRangeLevel::new(level).unwrap();
                let result = wait!(camera.exposure().set_wide_dynamic_range(level));
                if matches!(outcome, Outcome::Applied) {
                    result.unwrap();
                    assert_eq!(cache.exposure().wide_dynamic_range(), Some(level));
                } else {
                    assert!(result.is_err(), "{outcome:?}: {result:?}");
                    assert_eq!(cache.exposure().wide_dynamic_range(), None);
                }
            }
            wait!(session.close()).unwrap();
        }
    }

    fn motion_sync_cache_records_only_an_applied_setter() {
        for outcome in [Outcome::Applied, Outcome::Rejected, Outcome::Unconfirmed] {
            let wire = Wire::new(outcome);
            let config = SessionConfig::from_compile_time::<MotionSyncTypedSupport>().unwrap();
            let session = open!(wire, config).unwrap();
            let camera = session.camera::<MotionSyncTypedSupport>(IndividualCameraId::CAMERA_1).unwrap();
            let cache = camera.state_cache();
            assert_eq!(cache.motion_sync().enabled(), None);
            for enabled in [true, false] {
                let result = wait!(camera.motion_sync().set_enabled(enabled));
                if matches!(outcome, Outcome::Applied) {
                    result.unwrap();
                    assert_eq!(cache.motion_sync().enabled(), Some(enabled));
                } else {
                    assert!(result.is_err(), "{outcome:?}: {result:?}");
                    assert_eq!(cache.motion_sync().enabled(), None);
                }
            }
            wait!(session.close()).unwrap();
        }
    }
}
