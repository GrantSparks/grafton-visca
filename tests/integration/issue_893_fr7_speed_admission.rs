//! FR7 extended speeds require current, confirmed target-local mode state.

#![cfg(any(
    feature = "blocking",
    feature = "runtime-tokio",
    feature = "runtime-smol"
))]

use grafton_visca::{
    config::SessionConfig,
    error::{Certainty, FailureContext, FailureStage},
    operation::AppliedOnly,
    profiles::SonyFr7,
    transport::SendSemantics,
    types::{PanSpeed, PanTiltDirection, PanTiltSpeedMode, TiltSpeed},
    Error,
};
use grafton_visca_test_support::{
    facade_matrix,
    fake_camera::{self, frames, FakeCamera},
};

struct Fr7 {
    peer: FakeCamera,
}

impl Fr7 {
    fn new(reject_normal_after_ack: bool) -> Self {
        Self {
            peer: FakeCamera::visca(move |wire, reply| {
                if wire.first() != Some(&0x81) {
                    return; // Startup sequence reset is not a VISCA request.
                }
                reply.reply(frames::ack(1));
                if reject_normal_after_ack && wire == [0x81, 0x01, 0x06, 0x45, 0x08, 0xff] {
                    reply.reply(vec![0x90, 0x61, 0x41, 0xff]);
                } else {
                    reply.reply(frames::complete(1));
                }
            }),
        }
    }

    #[cfg(feature = "blocking")]
    fn blocking_wire(&self) -> fake_camera::BlockingWire {
        self.peer
            .blocking_wire()
            .with_semantics(SendSemantics::Stream)
    }

    #[cfg(any(feature = "runtime-tokio", feature = "runtime-smol"))]
    fn async_wire(&self) -> fake_camera::AsyncWire {
        self.peer.async_wire().with_semantics(SendSemantics::Stream)
    }
}

fn fast_drive() -> grafton_visca::command::pan_tilt::Drive {
    grafton_visca::command::pan_tilt::Drive::new(
        PanTiltDirection::Right,
        grafton_visca::types::PanTiltSpeed {
            pan: PanSpeed::new(50).unwrap(),
            tilt: TiltSpeed::new(50).unwrap(),
        },
    )
}

fn assert_unsent(error: &Error) {
    assert!(matches!(error, Error::TransmissionNotSent { source, .. }
        if matches!(source.as_ref(), Error::InvalidRequest { .. })));
    assert_eq!(
        error.failure_context(),
        Some(FailureContext::new(
            FailureStage::Terminal,
            Certainty::NotAccepted
        ))
    );
    assert!(!error.requires_new_session());
}

facade_matrix! {
    fn extended_speed_uses_the_latest_applied_mode_and_a_refusal_keeps_the_stream_live() {
        let wire = Fr7::new(false);
        let session = open!(wire, SessionConfig::from_compile_time::<SonyFr7>().unwrap()).unwrap();
        let camera = session.camera::<SonyFr7>(grafton_visca::types::IndividualCameraId::CAMERA_1).unwrap();
        let initial_writes = wire.peer.write_count();

        let mut refused = wait!(camera.submit::<AppliedOnly, _>(&fast_drive())).unwrap();
        assert_unsent(&wait!(refused.applied()).unwrap_err());
        let metrics = wait!(session.metrics()).unwrap();
        assert_eq!(metrics.writes, 0);
        assert_eq!(metrics.write_failures, 0);
        assert_eq!(wire.peer.write_count(), initial_writes);

        wait!(camera.pan_tilt().set_speed_mode(PanTiltSpeedMode::Fine50)).unwrap();
        assert_eq!(camera.state_cache().pan_tilt().speed_mode(), Some(PanTiltSpeedMode::Fine50));
        let mut accepted = wait!(camera.submit::<AppliedOnly, _>(&fast_drive())).unwrap();
        wait!(accepted.applied()).unwrap();

        wait!(camera.pan_tilt().set_speed_mode(PanTiltSpeedMode::Standard24)).unwrap();
        let before = wire.peer.write_count();
        let mut refused = wait!(camera.submit::<AppliedOnly, _>(&fast_drive())).unwrap();
        assert_unsent(&wait!(refused.applied()).unwrap_err());
        assert_eq!(wire.peer.write_count(), before);
        let mut stop = wait!(camera.submit::<AppliedOnly, _>(&grafton_visca::command::zoom::Stop)).unwrap();
        wait!(stop.applied()).expect("a rejected high speed cannot disable STOP delivery");
        wait!(session.close()).unwrap();
    }

    fn an_unconfirmed_mode_change_invalidates_the_extended_speed_prerequisite() {
        let wire = Fr7::new(true);
        let session = open!(wire, SessionConfig::from_compile_time::<SonyFr7>().unwrap()).unwrap();
        let camera = session.camera::<SonyFr7>(grafton_visca::types::IndividualCameraId::CAMERA_1).unwrap();
        wait!(camera.pan_tilt().set_speed_mode(PanTiltSpeedMode::Fine50)).unwrap();
        let error = wait!(camera.pan_tilt().set_speed_mode(PanTiltSpeedMode::Standard24))
            .unwrap_err();
        assert_eq!(error.failure_context().unwrap().certainty, Certainty::Unconfirmed);
        assert_eq!(camera.state_cache().pan_tilt().speed_mode(), None);
        let before = wire.peer.write_count();
        let mut refused = wait!(camera.submit::<AppliedOnly, _>(&fast_drive())).unwrap();
        assert_unsent(&wait!(refused.applied()).unwrap_err());
        assert_eq!(wire.peer.write_count(), before);
        let mut stop = wait!(camera.submit::<AppliedOnly, _>(&grafton_visca::command::zoom::Stop)).unwrap();
        wait!(stop.applied()).unwrap();
        wait!(session.close()).unwrap();
    }
}

// The plain setter waits for completion. A second blocking caller holds it
// open while the first submits operations through the same session.
#[cfg(feature = "blocking")]
#[test]
fn a_pending_mode_setter_holds_fast_drives_and_leaves_the_second_socket_for_stop() {
    use std::time::Duration;

    use grafton_visca::types::IndividualCameraId;
    use grafton_visca_test_support::fake_camera::ZOOM_STOP;

    const NORMAL: &[u8] = &[0x81, 0x01, 0x06, 0x45, 0x08, 0xff];
    let wire = Fr7 {
        peer: FakeCamera::visca(|command, reply| {
            if command.first() != Some(&0x81) {
                return;
            }
            if command == NORMAL {
                reply.reply(frames::ack(1));
            } else {
                reply.reply(frames::ack(2)).reply(frames::complete(2));
            }
        }),
    };
    let config = SessionConfig::from_compile_time::<SonyFr7>().unwrap();
    let session = grafton_visca::blocking::Session::open(wire.blocking_wire(), &config).unwrap();
    let camera = session
        .camera::<SonyFr7>(IndividualCameraId::CAMERA_1)
        .unwrap();
    camera
        .pan_tilt()
        .set_speed_mode(PanTiltSpeedMode::Fine50)
        .unwrap();
    let before = wire.peer.write_count();

    std::thread::scope(|scope| {
        let setter = scope.spawn(|| {
            camera
                .pan_tilt()
                .set_speed_mode(PanTiltSpeedMode::Standard24)
        });
        let writes = wire.peer.wait_for_writes(before + 1);
        let (sequence, command) = frames::sony_split(writes.last().unwrap()).unwrap();
        assert_eq!(command, NORMAL);
        let mut drive = camera.submit::<AppliedOnly, _>(&fast_drive()).unwrap();
        // Observe beyond the FR7's 35 ms spacing, with the setter still pending.
        let pending = drive.applied_with_timeout(Duration::from_millis(150));
        let mut stop = camera
            .submit::<AppliedOnly, _>(&grafton_visca::command::zoom::Stop)
            .unwrap();
        stop.applied()
            .expect("the other socket serves an unrelated STOP");
        wire.peer
            .push(frames::sony_reply(sequence, &frames::complete(1)));
        setter.join().unwrap().unwrap();
        assert!(matches!(pending, Err(Error::ObservationTimeout { .. })));
        assert_unsent(&drive.applied().unwrap_err());
    });
    let payloads = wire.peer.take_payloads();
    assert_eq!(payloads[before..], [NORMAL, ZOOM_STOP]);
    assert_eq!(
        camera.state_cache().pan_tilt().speed_mode(),
        Some(PanTiltSpeedMode::Standard24)
    );
    let metrics = session.metrics().unwrap();
    assert_eq!(metrics.writes, 3);
    assert_eq!(metrics.write_failures, 0);
    session.close().unwrap();
}
