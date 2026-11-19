//! The FR7 drive timing depends on the encoded command.

use super::*;
use grafton_visca::{
    operation::AppliedOnly,
    profiles::CommandTimeoutClass,
    raw::{AppliedOnlyRequest, PlainRequest},
    request::{
        CommandRetryClass, OperationClass, OperationCommand, OperationPolicy, Request,
        SubmissionClass,
    },
    types::{AffectedAxes, CameraId},
};

const VENDOR: &[u8] = &[0x81, 0x01, 0x06, 0x7f, 0x01, 0x01, 0x02, 0x03, 0xff];

fn raw_operation(bytes: &[u8]) -> AppliedOnlyRequest {
    AppliedOnlyRequest::new(
        bytes,
        AffectedAxes::PAN_TILT,
        CommandTimeoutClass::Movement,
        CommandRetryClass::Never,
        SubmissionClass::User,
    )
    .expect("raw operation")
}

fn plain(bytes: &[u8]) -> PlainRequest {
    PlainRequest::new(
        bytes,
        CommandTimeoutClass::Movement,
        CommandRetryClass::Never,
        SubmissionClass::User,
    )
    .expect("raw plain request")
}

enum RequestKind {
    Typed,
    Raw,
    Custom,
}

struct CustomDrive(&'static [u8]);

impl Request for CustomDrive {
    type Class = OperationClass<AppliedOnly>;

    const MAX_SIZE: usize = 9;

    fn policy(&self) -> OperationPolicy {
        OperationPolicy::new(
            CommandTimeoutClass::Movement,
            CommandRetryClass::Never,
            SubmissionClass::User,
        )
    }

    fn write_into(&self, camera: CameraId, buffer: &mut [u8]) -> Result<usize, Error> {
        let Some(frame) = buffer.get_mut(..self.0.len()) else {
            return Err(Error::buffer_too_small(self.0.len(), buffer.len()));
        };
        frame.copy_from_slice(self.0);
        frame[0] = 0x80 | camera.value();
        Ok(frame.len())
    }
}

impl OperationCommand<AppliedOnly> for CustomDrive {
    fn affected_axes(&self) -> AffectedAxes {
        AffectedAxes::PAN_TILT
    }
}

facade_matrix! {
    paused:

    fn equivalent_drive_packets_share_the_cancel_boundary() {
        for packet in [RIGHT, STOP, NAVIGATE_UP] {
            for request_kind in [RequestKind::Typed, RequestKind::Raw, RequestKind::Custom] {
                let fake = UdpCamera(FakeCamera::visca(move |write, answer| {
                    if write == packet {
                        answer.reply(frames::ack(1));
                    } else {
                        assert_eq!(write, CANCEL);
                        answer.reply(frames::canceled(1));
                    }
                }));
                let config = SessionConfig::from_compile_time::<SonyFr7>().expect("FR7 config");
                let session = open!(fake, config).expect("FR7 session");
                let camera = session.camera::<SonyFr7>(IndividualCameraId::CAMERA_1)
                    .expect("static FR7");
                let dynamic = session.camera::<AnyProfile>(IndividualCameraId::CAMERA_1)
                    .expect("dynamic FR7");
                let start = now!();
                let mut drive = match request_kind {
                    RequestKind::Typed if packet == RIGHT => wait!(camera.pan_tilt().drive(PanTiltDirection::Right, SpeedLevel::Slowest)),
                    RequestKind::Typed if packet == STOP => wait!(camera.pan_tilt().stop()),
                    RequestKind::Typed => wait!(camera.menu().navigate(MenuDirection::Up)),
                    RequestKind::Raw => wait!(dynamic.submit(&raw_operation(packet))),
                    RequestKind::Custom => wait!(dynamic.submit(&CustomDrive(packet))),
                }.expect("drive admitted");
                wait_for_reads!(fake.0, 1);
                // Blocking owners can notice a deadline one receive slice later.
                let early = Duration::from_millis(if virtual_clock!() { 199 } else { 180 });
                assert!(matches!(
                    wait!(drive.cancel_with_timeout(early.saturating_sub(now!().duration_since(start)))),
                    Err(Error::ObservationTimeout { .. })
                ));
                assert_eq!(fake.0.write_count(), 1, "cancel is still waiting");
                assert_eq!(wait!(drive.cancel()).expect("cancel confirmed"), CancellationOutcome::Cancelled);
                let elapsed = now!().duration_since(start);
                if virtual_clock!() {
                    assert_eq!(elapsed, DRIVE_INTERVAL);
                } else {
                    assert!(elapsed >= DRIVE_INTERVAL);
                }
                assert_eq!(fake.0.take_payloads(), [packet, CANCEL]);
                wait!(session.close()).expect("FR7 session closed");
            }
        }
    }

    fn inconclusive_opaque_cancel_blocks_queued_and_new_drive_packets() {
        let fake = UdpCamera(FakeCamera::visca(|write, answer| match write {
            STOP => {
                answer.reply(frames::ack(1));
            }
            CANCEL => {
                answer.reply(frames::error(1, 0x05));
            }
            POWER_INQUIRY => {
                answer.reply(frames::inquiry_reply(&[0x02]));
            }
            VENDOR => {
                answer.reply(frames::ack(2)).reply(frames::complete(2));
            }
            other => panic!("unexpected FR7 frame {other:02x?}"),
        }));
        let config = SessionConfig::from_compile_time::<SonyFr7>().expect("FR7 config");
        let session = open!(fake, config).expect("FR7 session");
        let camera = session.camera::<AnyProfile>(IndividualCameraId::CAMERA_1).expect("dynamic FR7");
        let mut first = wait!(camera.submit(&raw_operation(STOP))).expect("raw STOP admitted");
        wait_for_reads!(fake.0, 1);
        assert!(matches!(
            wait!(first.cancel_with_timeout(Duration::from_millis(45))),
            Err(Error::ObservationTimeout { .. })
        ));
        let mut queued = wait!(camera.submit(&CustomDrive(NAVIGATE_UP))).expect("custom navigation queued");
        let plain_camera = camera.clone();
        let (plain_result, operation_result) = together!({
            wait!(plain_camera.execute(&plain(RIGHT)))
        }, {
            wait!(queued.applied())
        });
        assert_restart_blocked(&plain_result.expect_err("queued plain drive unwritten"), FailureStage::Terminal);
        assert_restart_blocked(&operation_result.expect_err("queued operation unwritten"), FailureStage::Terminal);
        let refusal = wait!(camera.execute(&plain(RIGHT))).expect_err("new plain drive refused");
        assert_restart_blocked(&refusal, FailureStage::PreAdmission);

        assert!(wait!(camera.power().status()).expect("unrelated inquiry").is_on());
        wait!(camera.execute(&plain(VENDOR))).expect("neighboring vendor opcode");
        assert_eq!(fake.0.take_payloads(), [STOP, CANCEL, POWER_INQUIRY, VENDOR]);
        wait!(session.close()).expect("FR7 session closed");
    }

    fn opaque_drive_waits_for_named_ack_and_observes_natural_completion() {
        for ack in [None, Some(0), Some(1)] {
            let fake = UdpCamera(FakeCamera::visca(move |write, answer| {
                assert_eq!(write, RIGHT);
                if let Some(socket) = ack {
                    answer.reply(frames::ack(socket));
                }
            }));
            let config = SessionConfig::from_compile_time::<SonyFr7>().expect("FR7 config");
            let session = open!(fake, config).expect("FR7 session");
            let camera = session.camera::<SonyFr7>(IndividualCameraId::CAMERA_1).expect("static FR7");
            let mut drive = wait!(camera.submit(&CustomDrive(RIGHT))).expect("custom drive admitted");
            wait_for_writes!(fake.0, 1);
            if ack.is_some() {
                wait_for_reads!(fake.0, 1);
            }
            // Hold the terminal past the earliest ordinary socket cancel time.
            assert!(matches!(
                wait!(drive.cancel_with_timeout(Duration::from_millis(60))),
                Err(Error::ObservationTimeout { .. })
            ));
            let sequence = frames::sony_sequence(&fake.0.writes()[0]).expect("Sony drive sequence");
            fake.0.push(frames::sony_reply(sequence, &frames::complete(1)));
            assert_eq!(wait!(drive.cancel()).expect("drive completed"), CancellationOutcome::Completed);
            wait!(drive.applied()).expect("drive applied");
            assert_eq!(fake.0.take_payloads(), [RIGHT]);
            wait!(session.close()).expect("FR7 session closed");
        }
    }

    fn brc_raw_pan_tilt_drive_has_ordinary_cancel_spacing() {
        let fake = UdpCamera(FakeCamera::visca(|write, answer| match write {
            RIGHT => {
                answer.reply(frames::ack(1));
            }
            CANCEL => {
                answer.reply(frames::canceled(1));
            }
            other => panic!("unexpected BRC-H900 frame {other:02x?}"),
        }));
        let config = SessionConfig::from_compile_time::<SonyBrcH900>().expect("BRC-H900 config");
        let session = open!(fake, config).expect("BRC-H900 session");
        let camera = session.camera::<SonyBrcH900>(IndividualCameraId::CAMERA_1).expect("BRC-H900");
        let start = now!();
        let mut drive = wait!(camera.submit(&raw_operation(RIGHT))).expect("raw drive admitted");
        wait_for_reads!(fake.0, 1);
        assert_eq!(wait!(drive.cancel()).expect("cancel confirmed"), CancellationOutcome::Cancelled);
        assert!(now!().duration_since(start) < DRIVE_INTERVAL);
        assert_eq!(fake.0.take_payloads(), [RIGHT, CANCEL]);
        wait!(session.close()).expect("BRC-H900 session closed");
    }
}
