//! Construction rejects unrepresentable admission bounds before owner setup.
#![cfg(any(
    feature = "blocking",
    feature = "runtime-tokio",
    feature = "runtime-smol"
))]

use grafton_visca::{profiles::GenericVisca, Error, SessionConfig};
use grafton_visca_test_support::{facade_matrix, fake_camera::FakeCamera};
use std::num::NonZeroUsize;

facade_matrix! {
    fn admission_overflow_is_rejected_before_transport_access() {
        for capacity in [usize::MAX, usize::MAX - 1, usize::MAX - 3, usize::MAX / 5 + 1] {
            let camera = FakeCamera::acking(1);
            let config = SessionConfig::from_compile_time::<GenericVisca>().expect("profile")
                .with_admission_capacity(NonZeroUsize::new(capacity).expect("nonzero"));
            let error = open!(camera, config).expect_err("unrepresentable capacity");
            assert!(matches!(error, Error::InvalidRequest(_)), "{error:?}");
            assert_eq!(camera.config_reads(), 0);
            assert_eq!(camera.write_count(), 0);
            assert_eq!(camera.receive_calls(), 0);
        }
    }

    fn a_large_representable_capacity_opens_and_executes() {
        let camera = FakeCamera::acking(1);
        let config = SessionConfig::from_compile_time::<GenericVisca>().expect("profile")
            .with_admission_capacity(NonZeroUsize::new(1_048_576).expect("nonzero"));
        let session = open!(camera, config).expect("large representable capacity");
        let view = session.camera::<GenericVisca>().expect("camera");
        wait!(view.power().on()).expect("command");
        session.shutdown().expect("shutdown");
    }
}
