//! FR7 extended speeds require current, confirmed target-local mode state.

#![cfg(any(
    feature = "blocking",
    feature = "runtime-tokio",
    feature = "runtime-smol"
))]

use grafton_visca::{
    capabilities::{Capabilities, PanTiltCaps, TypedSupportSet, TypedSupportSurface},
    config::SessionConfig,
    error::{Certainty, FailureContext, FailureStage},
    operation::AppliedOnly,
    profiles::{
        AnyProfile, CommandTimeoutClass, ProfileEnvelope, ProfileSpec, SonyFr7,
        TransportCompatibility,
    },
    raw::{PlainRequest, RawReplyShape},
    request::{CommandPolicy, CommandRetryClass, SubmissionClass},
    transport::{AddressingMode, SendSemantics},
    types::{IndividualCameraId, PanSpeed, PanTiltDirection, PanTiltSpeedMode, TiltSpeed},
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

const STANDARD_MODE: [u8; 6] = [0x81, 0x01, 0x06, 0x45, 0x08, 0xff];
const FINE_MODE: [u8; 6] = [0x81, 0x01, 0x06, 0x45, 0x18, 0xff];
const SPEED_24: [u8; 9] = [0x81, 0x01, 0x06, 0x01, 0x18, 0x18, 0x02, 0x03, 0xff];
const SPEED_50: [u8; 9] = [0x81, 0x01, 0x06, 0x01, 0x32, 0x32, 0x02, 0x03, 0xff];

fn raw_command(bytes: [u8; 6], shape: RawReplyShape) -> PlainRequest {
    PlainRequest::with_policy(
        bytes,
        CommandPolicy::new(
            CommandTimeoutClass::Quick,
            CommandRetryClass::Never,
            SubmissionClass::Normal,
        )
        .with_reply_shape(shape),
    )
    .unwrap()
}

struct AddressedCamera(FakeCamera);

impl AddressedCamera {
    #[cfg(feature = "blocking")]
    fn blocking_wire(&self) -> fake_camera::BlockingWire {
        self.0
            .blocking_wire()
            .with_addressing(AddressingMode::Serial)
    }

    #[cfg(any(feature = "runtime-tokio", feature = "runtime-smol"))]
    fn async_wire(&self) -> fake_camera::AsyncWire {
        self.0.async_wire().with_addressing(AddressingMode::Serial)
    }
}

fn fast_drive() -> grafton_visca::command::pan_tilt::Drive {
    drive(50)
}

fn drive(speed: u8) -> grafton_visca::command::pan_tilt::Drive {
    grafton_visca::command::pan_tilt::Drive::new(
        PanTiltDirection::Right,
        grafton_visca::types::PanTiltSpeed {
            pan: PanSpeed::new(speed).unwrap(),
            tilt: TiltSpeed::new(speed).unwrap(),
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

    fn an_applied_raw_mode_setter_changes_the_extended_speed_guard() {
        let fake = FakeCamera::visca(|_, reply| {
            reply.reply(frames::ack(1)).reply(frames::complete(1));
        });
        let session = open!(fake, SessionConfig::from_compile_time::<SonyFr7>().unwrap()).unwrap();
        let camera = session.camera::<SonyFr7>(IndividualCameraId::CAMERA_1).unwrap();
        wait!(camera.pan_tilt().set_speed_mode(PanTiltSpeedMode::Fine50)).unwrap();

        wait!(camera.execute(&raw_command(STANDARD_MODE, RawReplyShape::AckThenCompletion))).unwrap();
        assert_eq!(camera.state_cache().pan_tilt().speed_mode(), Some(PanTiltSpeedMode::Standard24));
        let mut refused = wait!(camera.submit::<AppliedOnly, _>(&fast_drive())).unwrap();
        assert_unsent(&wait!(refused.applied()).unwrap_err());
        assert_eq!(fake.write_count(), 2);

        let mut standard = wait!(camera.submit::<AppliedOnly, _>(&drive(24))).unwrap();
        wait!(standard.applied()).unwrap();
        let mut vendor = STANDARD_MODE;
        vendor[3] += 1;
        let mut unknown_selector = STANDARD_MODE;
        unknown_selector[4] = 0x28;
        for bytes in [vendor, unknown_selector] {
            wait!(camera.execute(&raw_command(bytes, RawReplyShape::AckThenCompletion))).unwrap();
            assert_eq!(camera.state_cache().pan_tilt().speed_mode(), Some(PanTiltSpeedMode::Standard24));
        }
        wait!(camera.execute(&raw_command(FINE_MODE, RawReplyShape::AckThenCompletion))).unwrap();
        assert_eq!(camera.state_cache().pan_tilt().speed_mode(), Some(PanTiltSpeedMode::Fine50));
        let mut fast = wait!(camera.submit::<AppliedOnly, _>(&fast_drive())).unwrap();
        wait!(fast.applied()).unwrap();

        assert_eq!(fake.take_payloads(), [
            &FINE_MODE[..], &STANDARD_MODE, &SPEED_24, &vendor, &unknown_selector, &FINE_MODE, &SPEED_50
        ]);
        wait!(session.close()).unwrap();
    }

    fn a_raw_mode_write_invalidates_only_when_the_change_may_have_taken_effect() {
        let mut standard_writes = 0;
        let fake = FakeCamera::visca(move |command, reply| {
            if command == STANDARD_MODE {
                standard_writes += 1;
                match standard_writes {
                    1 => { reply.reply([0x90, 0x60, 0x02, 0xff]); }
                    2 => { reply.reply(frames::ack(1)).reply([0x90, 0x61, 0x41, 0xff]); }
                    3 => {}
                    _ => panic!("a new speed mode must be written at most once per request"),
                }
            } else {
                reply.reply(frames::ack(1)).reply(frames::complete(1));
            }
        });
        let session = open!(fake, SessionConfig::from_compile_time::<SonyFr7>().unwrap()).unwrap();
        let camera = session.camera::<SonyFr7>(IndividualCameraId::CAMERA_1).unwrap();
        let fine = raw_command(FINE_MODE, RawReplyShape::AckThenCompletion);
        let standard = raw_command(STANDARD_MODE, RawReplyShape::AckThenCompletion);

        wait!(camera.execute(&fine)).unwrap();
        let error = wait!(camera.execute(&standard)).unwrap_err();
        assert!(matches!(error, Error::SyntaxError), "{error:?}");
        assert_eq!(camera.state_cache().pan_tilt().speed_mode(), Some(PanTiltSpeedMode::Fine50));

        let error = wait!(camera.execute(&standard)).unwrap_err();
        assert_eq!(error.failure_context().unwrap().certainty, Certainty::Unconfirmed);
        assert_eq!(camera.state_cache().pan_tilt().speed_mode(), None);
        let mut refused = wait!(camera.submit::<AppliedOnly, _>(&fast_drive())).unwrap();
        assert_unsent(&wait!(refused.applied()).unwrap_err());
        assert_eq!(fake.write_count(), 3);

        wait!(camera.execute(&fine)).unwrap();
        wait!(camera.execute(&raw_command(STANDARD_MODE, RawReplyShape::NoReply))).unwrap();
        assert_eq!(camera.state_cache().pan_tilt().speed_mode(), None);
        let mut refused = wait!(camera.submit::<AppliedOnly, _>(&fast_drive())).unwrap();
        assert_unsent(&wait!(refused.applied()).unwrap_err());
        assert_eq!(fake.take_payloads(), [FINE_MODE, STANDARD_MODE, STANDARD_MODE, FINE_MODE, STANDARD_MODE]);
        wait!(session.close()).unwrap();
    }

    fn a_pending_raw_mode_setter_blocks_only_its_targets_extended_speed_drive() {
        use std::time::Duration;

        let fake = AddressedCamera(FakeCamera::visca(|command, reply| {
            let target_reply = 0x80 | ((command[0] & 0x0f) << 4);
            reply.reply([target_reply, 0x41, 0xff]);
            if command != STANDARD_MODE {
                reply.reply([target_reply, 0x51, 0xff]);
            }
        }));
        // A serial-addressed raw test bus supports the same exact setter on
        // two targets. Sony-encapsulated sessions allow only one target.
        let fr7 = ProfileSpec::from_compile_time::<SonyFr7>().unwrap();
        let capabilities = Capabilities::runtime_baseline("Addressed speed-mode camera", 1).unwrap();
        let profile = ProfileSpec::builder(capabilities)
            .pan_tilt(PanTiltCaps {
                geometry: None,
                motion_sync_speed_range: None,
                ..fr7.capabilities().pan_tilt.unwrap()
            })
            .typed_support(TypedSupportSet::from_surface(TypedSupportSurface::PanTiltSpeedMode))
            .transports(TransportCompatibility::new().with_serial())
            .envelope(ProfileEnvelope::RawVisca)
            .timing(fr7.timing())
            .maximum_command_sockets(fr7.maximum_command_sockets())
            .supports_operation_complete(true)
            .supports_command_cancel(false)
            .position_inquiries(None)
            .build().unwrap();
        let config = SessionConfig::new(profile.clone())
            .with_target(IndividualCameraId::CAMERA_2, profile).unwrap();
        let session = open!(fake, config).unwrap();
        let camera = session.camera::<AnyProfile>(IndividualCameraId::CAMERA_1).unwrap();
        let other = session.camera::<AnyProfile>(IndividualCameraId::CAMERA_2).unwrap();
        let mut other_fine = FINE_MODE;
        other_fine[0] = 0x82;
        wait!(camera.execute(&raw_command(FINE_MODE, RawReplyShape::AckThenCompletion))).unwrap();
        wait!(other.execute(&raw_command(other_fine, RawReplyShape::AckThenCompletion))).unwrap();
        let (setter, ()) = together!({
            wait!(camera.execute(&raw_command(STANDARD_MODE, RawReplyShape::AckThenCompletion)))
        }, {
            let writes = wait_for_writes!(fake.0, 3);
            assert_eq!(writes[2], STANDARD_MODE);
            let mut queued = wait!(camera.submit::<AppliedOnly, _>(&fast_drive())).unwrap();
            let pending = wait!(queued.applied_with_timeout(Duration::from_millis(100)));
            assert!(matches!(pending, Err(Error::ObservationTimeout { .. })));
            assert_eq!(fake.0.write_count(), 3);

            let mut unaffected = wait!(other.submit::<AppliedOnly, _>(&fast_drive())).unwrap();
            wait!(unaffected.applied()).unwrap();
            assert_eq!(other.state_cache().pan_tilt().speed_mode(), Some(PanTiltSpeedMode::Fine50));
            fake.0.push(frames::complete(1));
            assert_unsent(&wait!(queued.applied()).unwrap_err());
        });
        setter.unwrap();
        assert_eq!(camera.state_cache().pan_tilt().speed_mode(), Some(PanTiltSpeedMode::Standard24));
        let mut other_fast = SPEED_50;
        other_fast[0] = 0x82;
        let writes = fake.0.take_payloads();
        assert_eq!(writes, [&FINE_MODE[..], &other_fine, &STANDARD_MODE, &other_fast]);
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
