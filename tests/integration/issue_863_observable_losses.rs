//! Loss metrics describe public delivery, independently of session age.

#![cfg(any(
    feature = "blocking",
    feature = "runtime-tokio",
    feature = "runtime-smol"
))]

use grafton_visca::{config::SessionConfig, profiles::GenericVisca};
use grafton_visca_test_support::{
    facade_matrix,
    fake_camera::{frames, FakeCamera},
};

facade_matrix! {
    fn diagnostic_loss_counts_only_staging_or_subscriber_overflow() {
        let peer = FakeCamera::new(|_, reply| {
            reply.reply(frames::ack(1)).reply(frames::complete(1));
        });
        let session = open!(peer, SessionConfig::from_compile_time::<GenericVisca>().unwrap())
            .unwrap();
        let camera = session.camera::<GenericVisca>(grafton_visca::types::IndividualCameraId::CAMERA_1).unwrap();
        let fast = wait!(session.subscribe_diagnostics(128)).unwrap();
        let mut delivered = 0;
        for _ in 0..40 {
            wait!(camera.execute(&grafton_visca::command::focus::SetMode::new(grafton_visca::types::FocusMode::Manual))).unwrap();
            // A control response follows the terminal diagnostic on the owner.
            let metrics = wait!(session.metrics()).unwrap();
            delivered += std::iter::from_fn(|| fast.try_recv()).count();
            assert_eq!(metrics.dropped_staged_diagnostics, 0);
            assert_eq!(metrics.dropped_diagnostic_events, 0);
        }
        assert!(delivered > 128, "a healthy session outlives the former history bound");
        drop(fast);

        // Events produced with no subscribers are not delivery losses.
        for _ in 0..40 {
            wait!(camera.execute(&grafton_visca::command::focus::SetMode::new(grafton_visca::types::FocusMode::Manual))).unwrap();
        }
        let metrics = wait!(session.metrics()).unwrap();
        assert_eq!(metrics.dropped_staged_diagnostics, 0);
        assert_eq!(metrics.dropped_diagnostic_events, 0);

        let slow = wait!(session.subscribe_diagnostics(1)).unwrap();
        assert!(slow.try_recv().is_none(), "a subscription does not replay history");
        wait!(camera.execute(&grafton_visca::command::focus::SetMode::new(grafton_visca::types::FocusMode::Manual))).unwrap();
        let metrics = wait!(session.metrics()).unwrap();
        assert_eq!(metrics.dropped_staged_diagnostics, 0);
        assert!(metrics.dropped_diagnostic_events > 0);
        assert_eq!(std::iter::from_fn(|| slow.try_recv()).count(), 1);
        wait!(session.close()).unwrap();
    }
}
