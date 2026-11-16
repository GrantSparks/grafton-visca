//! A one-millisecond idle read cannot starve ready owner boundary lanes (#979).

#![cfg(any(
    feature = "runtime-tokio",
    all(
        feature = "test-utils",
        any(feature = "blocking", feature = "runtime-smol")
    )
))]

use std::time::Duration;

use grafton_visca::{
    config::SessionConfig,
    operation::{AppliedOnly, CancellationOutcome},
    profiles::ProfileSpec,
    transport::TransportConfig,
    types::IndividualCameraId,
};
use grafton_visca_test_support::{
    facade_matrix,
    fake_camera::{self, frames, FakeCamera, FOCUS_STOP, ZOOM_STOP, ZOOM_TELE},
    profile_fixtures::NonDefaultCompileTimeProfile as Raw,
};

/// Gives both facade transports the same short read timeout.
struct ShortReads(FakeCamera);

impl ShortReads {
    fn config() -> TransportConfig {
        TransportConfig::default().with_read_timeout(Duration::from_millis(1))
    }

    #[cfg(all(feature = "blocking", feature = "test-utils"))]
    fn blocking_wire(&self) -> fake_camera::BlockingWire {
        self.0.blocking_wire().with_config(Self::config())
    }

    #[cfg(all(
        feature = "async",
        any(
            feature = "runtime-tokio",
            all(feature = "runtime-smol", feature = "test-utils")
        )
    ))]
    fn async_wire(&self) -> fake_camera::AsyncWire {
        self.0.async_wire().with_config(Self::config())
    }
}

facade_matrix! {
    paused:

    /// Exercise admission, cancellation, halt control and the close barrier
    /// while every silent receive exhausts its idle budget in one probe.
    fn short_reads_keep_command_cancel_stop_all_and_close_live() {
        const CANCEL: &[u8] = &[0x81, 0x21, 0xff];
        let fake = FakeCamera::new(|write, answer| {
            if write == ZOOM_TELE {
                // Keep this command executing until its cancellation boundary.
                answer.reply(frames::ack(1));
            } else if write == CANCEL {
                answer.reply(frames::canceled(1));
            } else {
                // Socket one can retain the cancellation's tombstone. The
                // independent second socket remains available for each STOP.
                answer.reply(fake_camera::ack_and_complete(2));
            }
        });
        let short_reads = ShortReads(fake.clone());
        let config = SessionConfig::new(ProfileSpec::from_compile_time::<Raw>().expect("raw profile"));
        let session = open!(short_reads, config).expect("owner session");
        let camera = session.camera::<Raw>(IndividualCameraId::CAMERA_1).expect("camera view");

        let mut completed = wait!(camera.zoom().stop()).expect("initial command admitted");
        wait!(completed.applied()).expect("initial command applied");

        let mut running = wait!(camera.submit::<AppliedOnly, _>(&grafton_visca::command::zoom::Tele))
            .expect("running command admitted");
        wait_for_writes!(fake, 2);
        wait_for_reads!(fake, 2);
        assert_eq!(wait!(running.cancel()).expect("cancellation boundary"), CancellationOutcome::Cancelled);

        wait!(camera.motion().stop_all()).expect("halt boundary accepted")
            .into_result().expect("all three STOP commands applied");
        let writes = fake.writes();
        assert_eq!(writes.len(), 6);
        assert_eq!(writes[0], ZOOM_STOP);
        assert_eq!(writes[1], ZOOM_TELE);
        assert_eq!(writes[2], CANCEL);
        assert!(writes[3].starts_with(&[0x81, 0x01, 0x06, 0x01]));
        assert_eq!(writes[4], ZOOM_STOP);
        assert_eq!(writes[5], FOCUS_STOP);

        wait!(session.close()).expect("close joins owner teardown");
        assert_eq!(fake.wires_dropped(), 1);
    }
}
