#![allow(clippy::expect_used, clippy::panic, clippy::unwrap_used)]

use std::time::Duration;

use super::*;
use crate::{
    runtime::engine::{CancellationPolicy, EnvelopeKind, TransportKind},
    CameraId,
};

fn policy_for_target_validation() -> ProtocolPolicy {
    ProtocolPolicy {
        capacity: 1,
        envelope: EnvelopeKind::Raw,
        transport: TransportKind::Datagram,
        inquiry_capacity: 1,
        command_spacing: Duration::ZERO,
        inquiry_spacing: Duration::ZERO,
        inquiry_cooldown: Duration::ZERO,
    }
}

fn target_policy_for_validation() -> TargetPolicy {
    TargetPolicy {
        command_sockets: 1,
        cancellation: CancellationPolicy::Supported,
    }
}

#[test]
fn single_target_rejects_broadcast_without_panicking() {
    let result = OwnerPolicy::single_target(
        policy_for_target_validation(),
        CameraId::BROADCAST,
        target_policy_for_validation(),
    );
    assert!(result.is_err());
}

#[test]
fn with_targets_rejects_broadcast_slot_without_panicking() {
    let mut targets = [None; 9];
    targets[8] = Some(target_policy_for_validation());
    let result = OwnerPolicy::with_targets(policy_for_target_validation(), targets);
    assert!(result.is_err());
}

#[cfg(all(feature = "blocking", not(feature = "async")))]
mod blocking {
    use std::{
        collections::{BTreeMap, VecDeque},
        sync::Arc,
        time::Duration,
    };

    use crate::{
        command::CommandKind,
        completion,
        transport::{builder::AddressingMode, Envelope, RawVisca, SonyEncapsulated},
        CameraId, Error, ViscaSocket,
    };

    use super::super::*;
    use crate::runtime::engine::{
        CancellationPolicy, ControlPolicy, DecodedResponse, EncodedMessage, EnvelopeKind,
        EnvelopeSequence, RequestContext, RetryPolicy, SequenceWidth, TimeoutPolicy, TransportKind,
    };

    fn policy(capacity: usize, transport: TransportKind) -> OwnerPolicy {
        let protocol = ProtocolPolicy {
            capacity,
            envelope: EnvelopeKind::Raw,
            transport,
            inquiry_capacity: capacity,
            command_spacing: Duration::ZERO,
            inquiry_spacing: Duration::ZERO,
            inquiry_cooldown: Duration::ZERO,
        };
        let mut owner = OwnerPolicy::single_target(
            protocol,
            CameraId::CAMERA_1,
            TargetPolicy {
                command_sockets: 2,
                cancellation: CancellationPolicy::Supported,
            },
        )
        .unwrap();
        owner.targets[usize::from(CameraId::CAMERA_2.id())] = Some(TargetPolicy {
            command_sockets: 2,
            cancellation: CancellationPolicy::Supported,
        });
        owner
    }

    fn context(target: CameraId, cancellation: CancellationPolicy) -> RequestContext {
        RequestContext {
            target,
            timeout: TimeoutPolicy {
                ack: Duration::from_millis(10),
                completion: Duration::from_millis(20),
                inquiry: Duration::from_millis(10),
                cancellation: Duration::from_millis(10),
                ambiguity: Duration::from_millis(10),
            },
            retry: RetryPolicy::NEVER,
            control: ControlPolicy::default(),
            cancellation,
        }
    }

    fn command(
        target: CameraId,
        cancellation: CancellationPolicy,
        applied_state: Option<AppliedStateProjection>,
    ) -> RuntimeRequest {
        RuntimeRequest::Command {
            wire: Arc::new(
                EncodedMessage::new(&[target.to_address_byte(), 0x01, 0x04, 0x00, 0xff]).unwrap(),
            ),
            context: context(target, cancellation),
            applied_state,
        }
    }

    fn frame(target: CameraId, response: DecodedResponse) -> DecodedFrame {
        DecodedFrame {
            target,
            sequence: None,
            response,
        }
    }

    #[derive(Debug, Default)]
    struct FakeDriver {
        writes: Vec<(RequestId, Vec<u8>, bool)>,
        results: VecDeque<Result<TransmissionMeta, Error>>,
    }

    impl BlockingWireDriver for FakeDriver {
        fn write(&mut self, write: WireWrite<'_>) -> Result<TransmissionMeta, Error> {
            self.writes
                .push((write.request, write.bytes.to_vec(), write.cancellation));
            self.results
                .pop_front()
                .unwrap_or(Ok(TransmissionMeta { sequence: None }))
        }
    }

    #[derive(Debug)]
    enum TestEnvelope {
        Raw(RawVisca),
        Sony(SonyEncapsulated),
    }

    #[derive(Debug)]
    struct FramingDriver {
        envelope: TestEnvelope,
        raw_pointers: Vec<usize>,
        frame_pointers: Vec<usize>,
        frame_capacities: Vec<usize>,
        frames: Vec<Vec<u8>>,
    }

    impl FramingDriver {
        fn new(envelope: EnvelopeKind) -> Self {
            let envelope = match envelope {
                EnvelopeKind::Raw => TestEnvelope::Raw(RawVisca::new(AddressingMode::Ip)),
                EnvelopeKind::Sony => TestEnvelope::Sony(SonyEncapsulated::new(AddressingMode::Ip)),
            };
            Self {
                envelope,
                raw_pointers: Vec::new(),
                frame_pointers: Vec::new(),
                frame_capacities: Vec::new(),
                frames: Vec::new(),
            }
        }
    }

    impl BlockingWireDriver for FramingDriver {
        fn write(&mut self, write: WireWrite<'_>) -> Result<TransmissionMeta, Error> {
            let kind = if write.inquiry {
                CommandKind::Inquiry
            } else {
                CommandKind::Command
            };
            self.raw_pointers.push(write.bytes.as_ptr() as usize);
            let meta = match &self.envelope {
                TestEnvelope::Raw(envelope) => {
                    envelope.frame_into(write.bytes, kind, write.frame_buffer)
                }
                TestEnvelope::Sony(envelope) => {
                    envelope.frame_into(write.bytes, kind, write.frame_buffer)
                }
            };
            self.frame_pointers
                .push(write.frame_buffer.as_ptr() as usize);
            self.frame_capacities.push(write.frame_buffer.capacity());
            self.frames.push(write.frame_buffer.to_vec());
            Ok(TransmissionMeta {
                sequence: meta.sequence,
            })
        }
    }

    #[derive(Debug, Default)]
    struct DeadlineReader {
        deadline: Option<Instant>,
        result: Option<Result<BlockingReceive, Error>>,
    }

    impl BlockingReadDriver for DeadlineReader {
        fn receive(
            &mut self,
            _receive_buffer: &mut [u8],
            owner_deadline: Option<Instant>,
        ) -> Result<BlockingReceive, Error> {
            self.deadline = owner_deadline;
            self.result.take().unwrap_or(Ok(BlockingReceive::TimedOut))
        }
    }

    #[derive(Debug, Default)]
    struct EmptyDecoder;

    impl BlockingFrameDecoder for EmptyDecoder {
        fn decode(
            &mut self,
            _buffers: &mut OwnerBuffers,
            _received: usize,
            _frame_limit: usize,
        ) -> Result<Vec<DecodedFrame>, Error> {
            Ok(Vec::new())
        }
    }

    #[derive(Debug, Default)]
    struct ScriptedReader;

    impl BlockingReadDriver for ScriptedReader {
        fn receive(
            &mut self,
            receive_buffer: &mut [u8],
            _owner_deadline: Option<Instant>,
        ) -> Result<BlockingReceive, Error> {
            receive_buffer[0] = 1;
            Ok(BlockingReceive::Bytes(1))
        }
    }

    #[derive(Debug)]
    struct ScriptedDecoder {
        batches: VecDeque<Vec<DecodedFrame>>,
    }

    impl BlockingFrameDecoder for ScriptedDecoder {
        fn decode(
            &mut self,
            _buffers: &mut OwnerBuffers,
            _received: usize,
            _frame_limit: usize,
        ) -> Result<Vec<DecodedFrame>, Error> {
            self.batches
                .pop_front()
                .ok_or_else(|| Error::InvalidState("scripted frame batch exhausted".into()))
        }
    }

    fn prepared_focus(
        profile: &crate::ProfileSpec,
        target: CameraId,
    ) -> crate::prepared::PreparedCommand {
        crate::prepared::prepare_command(
            &crate::request::builtin::FocusModeCommand::Manual,
            target,
            profile,
            crate::OperationalTuning::new(),
        )
        .unwrap()
    }

    fn prepared_limit_clear(
        profile: &crate::ProfileSpec,
        target: CameraId,
    ) -> crate::prepared::PreparedCommand {
        crate::prepared::prepare_command(
            &crate::request::builtin::PanTiltLimitClear::new(
                crate::command::PanTiltLimitCorner::DownLeft,
            ),
            target,
            profile,
            crate::OperationalTuning::new(),
        )
        .unwrap()
    }

    #[test]
    fn typed_blocking_wait_pumps_and_retains_out_of_order_peer_results() {
        let profile =
            crate::ProfileSpec::from_compile_time::<crate::profiles::SonyBRC300>().unwrap();
        let mut owner = BlockingOwner::new(policy(8, TransportKind::Datagram)).unwrap();
        let mut driver = FakeDriver::default();
        let a = owner
            .submit_command(&mut driver, prepared_focus(&profile, CameraId::CAMERA_1))
            .unwrap();
        let b = owner
            .submit_command(&mut driver, prepared_focus(&profile, CameraId::CAMERA_1))
            .unwrap();
        let c = owner
            .submit_command(&mut driver, prepared_focus(&profile, CameraId::CAMERA_2))
            .unwrap();
        let d = owner
            .submit_command(&mut driver, prepared_focus(&profile, CameraId::CAMERA_2))
            .unwrap();

        let mut reader = ScriptedReader;
        let mut decoder = ScriptedDecoder {
            batches: VecDeque::from([
                vec![
                    frame(
                        CameraId::CAMERA_1,
                        DecodedResponse::Ack {
                            socket: Some(ViscaSocket::S1),
                        },
                    ),
                    frame(
                        CameraId::CAMERA_1,
                        DecodedResponse::Ack {
                            socket: Some(ViscaSocket::S2),
                        },
                    ),
                    frame(
                        CameraId::CAMERA_2,
                        DecodedResponse::Ack {
                            socket: Some(ViscaSocket::S1),
                        },
                    ),
                    frame(
                        CameraId::CAMERA_2,
                        DecodedResponse::Ack {
                            socket: Some(ViscaSocket::S2),
                        },
                    ),
                ],
                vec![frame(
                    CameraId::CAMERA_1,
                    DecodedResponse::Completion {
                        socket: Some(ViscaSocket::S1),
                    },
                )],
                vec![frame(
                    CameraId::CAMERA_2,
                    DecodedResponse::Error {
                        socket: Some(ViscaSocket::S2),
                        code: 0x02,
                    },
                )],
                vec![frame(
                    CameraId::CAMERA_1,
                    DecodedResponse::Completion {
                        socket: Some(ViscaSocket::S2),
                    },
                )],
                vec![frame(
                    CameraId::CAMERA_2,
                    DecodedResponse::Completion {
                        socket: Some(ViscaSocket::S1),
                    },
                )],
            ]),
        };

        b.wait(&mut owner.receipt_control(&mut driver, &mut reader, &mut decoder))
            .unwrap();
        a.wait(&mut owner.receipt_control(&mut driver, &mut reader, &mut decoder))
            .unwrap();
        assert!(matches!(
            d.wait(&mut owner.receipt_control(&mut driver, &mut reader, &mut decoder,)),
            Err(Error::SyntaxError)
        ));
        c.wait(&mut owner.receipt_control(&mut driver, &mut reader, &mut decoder))
            .unwrap();
        assert!(decoder.batches.is_empty());
        assert_eq!(owner.state().active_len(), 0);
    }

    #[test]
    fn typed_inquiry_keeps_its_decoder_and_normalizes_the_reply() {
        let profile =
            crate::ProfileSpec::from_compile_time::<crate::profiles::GenericVisca>().unwrap();
        let prepared = crate::prepared::prepare_builtin_inquiry(
            &crate::command::PowerInquiry,
            CameraId::CAMERA_1,
            &profile,
            crate::OperationalTuning::new(),
        )
        .unwrap();
        let mut owner = BlockingOwner::new(policy(1, TransportKind::Datagram)).unwrap();
        let mut driver = FakeDriver::default();
        let receipt = owner.submit_inquiry(&mut driver, prepared).unwrap();
        let mut reader = ScriptedReader;
        let mut decoder = ScriptedDecoder {
            batches: VecDeque::from([vec![frame(
                CameraId::CAMERA_1,
                DecodedResponse::InquiryReply {
                    route: None,
                    payload: smallvec::smallvec![0x02],
                },
            )]]),
        };
        assert!(receipt
            .wait(&mut owner.receipt_control(&mut driver, &mut reader, &mut decoder,))
            .unwrap());
    }

    #[test]
    fn settlement_query_pacing_timeout_detaches_without_write_or_lifecycle_mutation() {
        let profile =
            crate::ProfileSpec::from_compile_time::<crate::profiles::GenericVisca>().unwrap();
        let tuning = crate::OperationalTuning::new().inquiry_spacing(Duration::from_millis(8));
        let prepare = || {
            crate::prepared::prepare_builtin_inquiry(
                &crate::command::PowerInquiry,
                CameraId::CAMERA_1,
                &profile,
                tuning,
            )
            .unwrap()
        };
        let mut owner = BlockingOwner::new(policy(1, TransportKind::Datagram)).unwrap();
        let mut driver = FakeDriver::default();
        let first = owner.submit_inquiry(&mut driver, prepare()).unwrap();
        owner
            .inject_frame(
                &mut driver,
                frame(
                    CameraId::CAMERA_1,
                    DecodedResponse::InquiryReply {
                        route: None,
                        payload: smallvec::smallvec![0x02],
                    },
                ),
                Instant::now(),
            )
            .unwrap();
        let mut reader = DeadlineReader::default();
        let mut decoder = EmptyDecoder;
        assert!(first
            .wait(&mut owner.receipt_control(&mut driver, &mut reader, &mut decoder))
            .unwrap());

        let deadline = Instant::now() + Duration::from_millis(1);
        assert!(matches!(
            owner.submit_inquiry_until(&mut driver, prepare(), deadline),
            Err(Error::Timeout)
        ));
        assert_eq!(driver.writes.len(), 1, "expired pacing writes no query");
        assert_eq!(
            owner.state().active_len(),
            1,
            "request remains engine-owned"
        );
        assert_eq!(
            owner.state().permits().available(),
            0,
            "permit remains held"
        );
        assert!(owner.state().diagnostics().all(|event| !matches!(
            event,
            DiagnosticEvent::CancellationRecorded { .. }
                | DiagnosticEvent::CancellationObserved { .. }
        )));

        std::thread::sleep(Duration::from_millis(9));
        owner
            .pump_once(&mut driver, &mut reader, &mut decoder)
            .unwrap();
        assert_eq!(
            driver.writes.len(),
            2,
            "ordinary engine later dispatches it"
        );
        owner
            .inject_frame(
                &mut driver,
                frame(
                    CameraId::CAMERA_1,
                    DecodedResponse::InquiryReply {
                        route: None,
                        payload: smallvec::smallvec![0x03],
                    },
                ),
                Instant::now(),
            )
            .unwrap();
        assert_eq!(owner.state().active_len(), 0);
        assert_eq!(owner.state().permits().available(), 1);
    }

    fn prepared_zoom<K>(profile: &crate::ProfileSpec) -> crate::prepared::PreparedOperation<K>
    where
        K: completion::Kind,
        crate::request::builtin::ZoomTarget: crate::OperationCommand<K>,
    {
        crate::prepared::prepare_builtin_operation::<K, _>(
            &crate::request::builtin::ZoomTarget::new(
                crate::types::ZoomPosition::new(0x0100).unwrap(),
            ),
            CameraId::CAMERA_1,
            profile,
            crate::OperationalTuning::new(),
        )
        .unwrap()
    }

    fn prepared_zoom_drive(
        profile: &crate::ProfileSpec,
    ) -> crate::prepared::PreparedOperation<completion::AppliedOnly> {
        crate::prepared::prepare_builtin_operation::<completion::AppliedOnly, _>(
            &crate::request::builtin::ZoomDrive::Tele,
            CameraId::CAMERA_1,
            profile,
            crate::OperationalTuning::new(),
        )
        .unwrap()
    }

    #[test]
    fn typed_operation_cancel_preserves_buffered_completion_and_exact_origin() {
        let profile =
            crate::ProfileSpec::from_compile_time::<crate::profiles::SonyBRC300>().unwrap();
        let mut owner = BlockingOwner::new(policy(2, TransportKind::Datagram)).unwrap();
        let mut driver = FakeDriver::default();
        let operation = owner
            .submit_operation(&mut driver, prepared_zoom::<completion::Targeted>(&profile))
            .unwrap();
        let now = Instant::now();
        owner
            .inject_frame(
                &mut driver,
                frame(
                    CameraId::CAMERA_1,
                    DecodedResponse::Ack {
                        socket: Some(ViscaSocket::S1),
                    },
                ),
                now,
            )
            .unwrap();
        owner
            .inject_frame(
                &mut driver,
                frame(
                    CameraId::CAMERA_1,
                    DecodedResponse::Completion {
                        socket: Some(ViscaSocket::S1),
                    },
                ),
                now,
            )
            .unwrap();
        let cancellation = operation.cancel(&mut owner, &mut driver).unwrap();
        let mut reader = DeadlineReader::default();
        let mut decoder = EmptyDecoder;
        assert_eq!(
            cancellation
                .outcome(
                    &mut owner.receipt_control(&mut driver, &mut reader, &mut decoder),
                    Duration::ZERO,
                )
                .unwrap(),
            CancellationOutcome::Completed
        );
        assert_eq!(
            driver.writes.len(),
            1,
            "buffered completion sends no cancel"
        );

        let mut first = BlockingOwner::new(policy(1, TransportKind::Datagram)).unwrap();
        let mut second = BlockingOwner::new(policy(1, TransportKind::Datagram)).unwrap();
        let mut first_driver = FakeDriver::default();
        let mut second_driver = FakeDriver::default();
        let operation = first
            .submit_operation(
                &mut first_driver,
                prepared_zoom::<completion::Targeted>(&profile),
            )
            .unwrap();
        let mut wrong_reader = DeadlineReader::default();
        let mut wrong_decoder = EmptyDecoder;
        assert!(matches!(
            operation.applied_with_timeout(
                &mut second.receipt_control(
                    &mut second_driver,
                    &mut wrong_reader,
                    &mut wrong_decoder,
                ),
                Duration::ZERO,
            ),
            Err(Error::InvalidState(_))
        ));
        assert!(wrong_reader.deadline.is_none(), "wrong owner never pumps");
    }

    #[test]
    fn shared_blocking_control_allows_two_public_handles() {
        let profile =
            crate::ProfileSpec::from_compile_time::<crate::profiles::SonyBRC300>().unwrap();
        let mut owner = BlockingOwner::new(policy(2, TransportKind::Datagram)).unwrap();
        let mut driver = FakeDriver::default();
        let first_receipt = owner
            .submit_operation(&mut driver, prepared_zoom_drive(&profile))
            .unwrap();
        let second_receipt = owner
            .submit_operation(&mut driver, prepared_zoom_drive(&profile))
            .unwrap();
        let mut reader = DeadlineReader::default();
        let mut decoder = EmptyDecoder;
        let core = BlockingSessionCore::new(&mut owner, &mut driver, &mut reader, &mut decoder);
        let first = crate::blocking::Operation::from_receipt(first_receipt, &core);
        let second = crate::blocking::Operation::from_receipt(second_receipt, &core);

        assert!(matches!(
            first.applied_with_timeout(Duration::ZERO),
            Err(Error::Timeout)
        ));
        assert!(matches!(
            second.applied_with_timeout(Duration::ZERO),
            Err(Error::Timeout)
        ));
    }

    #[test]
    fn shared_blocking_handles_retain_out_of_order_success_and_error() {
        let profile =
            crate::ProfileSpec::from_compile_time::<crate::profiles::SonyBRC300>().unwrap();
        let mut owner = BlockingOwner::new(policy(2, TransportKind::Datagram)).unwrap();
        let mut driver = FakeDriver::default();
        let first_receipt = owner
            .submit_operation(&mut driver, prepared_zoom_drive(&profile))
            .unwrap();
        let second_receipt = owner
            .submit_operation(&mut driver, prepared_zoom_drive(&profile))
            .unwrap();
        let now = Instant::now();

        // Route both ACKs before delivering terminals, then deliberately
        // complete the second request before failing the first one.  The
        // owner must retain each exact terminal outcome independently of the
        // order in which its handles are consumed.
        owner
            .inject_frame(
                &mut driver,
                frame(
                    CameraId::CAMERA_1,
                    DecodedResponse::Ack {
                        socket: Some(ViscaSocket::S1),
                    },
                ),
                now,
            )
            .unwrap();
        owner
            .inject_frame(
                &mut driver,
                frame(
                    CameraId::CAMERA_1,
                    DecodedResponse::Ack {
                        socket: Some(ViscaSocket::S2),
                    },
                ),
                now,
            )
            .unwrap();
        owner
            .inject_frame(
                &mut driver,
                frame(
                    CameraId::CAMERA_1,
                    DecodedResponse::Completion {
                        socket: Some(ViscaSocket::S2),
                    },
                ),
                now,
            )
            .unwrap();
        owner
            .inject_frame(
                &mut driver,
                frame(
                    CameraId::CAMERA_1,
                    DecodedResponse::Error {
                        socket: Some(ViscaSocket::S1),
                        code: 0x02,
                    },
                ),
                now,
            )
            .unwrap();

        let mut reader = DeadlineReader::default();
        let mut decoder = EmptyDecoder;
        let core = BlockingSessionCore::new(&mut owner, &mut driver, &mut reader, &mut decoder);
        let first = crate::blocking::Operation::from_receipt(first_receipt, &core);
        let second = crate::blocking::Operation::from_receipt(second_receipt, &core);

        assert!(matches!(
            first.applied_with_timeout(Duration::ZERO),
            Err(Error::SyntaxError)
        ));
        assert!(second.applied_with_timeout(Duration::ZERO).is_ok());
    }

    #[test]
    fn targeted_settlement_selection_retains_one_absolute_deadline() {
        let profile =
            crate::ProfileSpec::from_compile_time::<crate::profiles::SonyBRC300>().unwrap();
        let mut owner = BlockingOwner::new(policy(1, TransportKind::Datagram)).unwrap();
        let mut driver = FakeDriver::default();
        let operation = owner
            .submit_operation(&mut driver, prepared_zoom::<completion::Targeted>(&profile))
            .unwrap();
        let now = Instant::now();
        owner
            .inject_frame(
                &mut driver,
                frame(
                    CameraId::CAMERA_1,
                    DecodedResponse::Ack {
                        socket: Some(ViscaSocket::S1),
                    },
                ),
                now,
            )
            .unwrap();
        owner
            .inject_frame(
                &mut driver,
                frame(
                    CameraId::CAMERA_1,
                    DecodedResponse::Completion {
                        socket: Some(ViscaSocket::S1),
                    },
                ),
                now,
            )
            .unwrap();
        let mut reader = DeadlineReader::default();
        let mut decoder = EmptyDecoder;
        let selection = operation.settled_with_timeout(
            owner.receipt_control(&mut driver, &mut reader, &mut decoder),
            Duration::from_millis(321),
        );
        assert_eq!(
            selection.selection(),
            WaitSelection::Override(Duration::from_millis(321))
        );
        let deadline = Instant::now() + Duration::from_secs(1);
        let BlockingAfterApplied::Poll(continuation) =
            selection.wait_applied_until(deadline).unwrap()
        else {
            panic!("Sony BRC-300 targeted operation must delegate polling");
        };
        assert_eq!(continuation.target, CameraId::CAMERA_1);
        assert_eq!(continuation.axes, crate::AffectedAxes::ZOOM);
        assert_eq!(continuation.deadline, deadline);
        assert!(matches!(
            continuation.plan,
            crate::prepared::SettlementPlan::Poll { .. }
        ));
    }

    #[test]
    fn completion_is_settled_submits_no_position_inquiries() {
        let profile =
            crate::ProfileSpec::from_compile_time::<crate::profiles::PtzOpticsG3>().unwrap();
        let mut owner = BlockingOwner::new(policy(1, TransportKind::Datagram)).unwrap();
        let mut driver = FakeDriver::default();
        let operation = owner
            .submit_operation(&mut driver, prepared_zoom::<completion::Targeted>(&profile))
            .unwrap();
        let now = Instant::now();
        owner
            .inject_frame(
                &mut driver,
                frame(
                    CameraId::CAMERA_1,
                    DecodedResponse::Ack {
                        socket: Some(ViscaSocket::S1),
                    },
                ),
                now,
            )
            .unwrap();
        owner
            .inject_frame(
                &mut driver,
                frame(
                    CameraId::CAMERA_1,
                    DecodedResponse::Completion {
                        socket: Some(ViscaSocket::S1),
                    },
                ),
                now,
            )
            .unwrap();
        let mut reader = DeadlineReader::default();
        let mut decoder = EmptyDecoder;
        {
            let _settled = operation
                .settled(owner.receipt_control(&mut driver, &mut reader, &mut decoder))
                .wait()
                .unwrap();
        }
        assert_eq!(driver.writes.len(), 1);
        assert!(reader.deadline.is_none());
    }

    #[test]
    fn settlement_deadline_detaches_inquiry_and_late_reply_finishes_lifecycle() {
        let profile =
            crate::ProfileSpec::from_compile_time::<crate::profiles::SonyBRC300>().unwrap();
        let mut owner = BlockingOwner::new(policy(1, TransportKind::Datagram)).unwrap();
        let mut driver = FakeDriver::default();
        let operation = owner
            .submit_operation(&mut driver, prepared_zoom::<completion::Targeted>(&profile))
            .unwrap();
        let now = Instant::now();
        owner
            .inject_frame(
                &mut driver,
                frame(
                    CameraId::CAMERA_1,
                    DecodedResponse::Ack {
                        socket: Some(ViscaSocket::S1),
                    },
                ),
                now,
            )
            .unwrap();
        owner
            .inject_frame(
                &mut driver,
                frame(
                    CameraId::CAMERA_1,
                    DecodedResponse::Completion {
                        socket: Some(ViscaSocket::S1),
                    },
                ),
                now,
            )
            .unwrap();
        let mut reader = DeadlineReader::default();
        let mut decoder = EmptyDecoder;
        assert!(matches!(
            operation
                .settled_with_timeout(
                    owner.receipt_control(&mut driver, &mut reader, &mut decoder),
                    Duration::from_millis(1),
                )
                .wait(),
            Err(Error::Timeout)
        ));
        assert_eq!(driver.writes.len(), 2);
        assert_eq!(owner.state().active_len(), 1);
        assert_eq!(owner.state().permits().available(), 0);
        assert!(owner.state().diagnostics().all(|event| !matches!(
            event,
            DiagnosticEvent::CancellationRecorded { .. }
                | DiagnosticEvent::CancellationObserved { .. }
        )));
        owner
            .inject_frame(
                &mut driver,
                frame(
                    CameraId::CAMERA_1,
                    DecodedResponse::InquiryReply {
                        route: None,
                        payload: smallvec::smallvec![0x0, 0x1, 0x0, 0x0],
                    },
                ),
                Instant::now(),
            )
            .unwrap();
        assert_eq!(owner.state().active_len(), 0);
        assert_eq!(owner.state().permits().available(), 1);
    }

    #[test]
    fn settlement_query_propagates_capacity_and_transport_errors_exactly() {
        fn complete_operation(
            owner: &mut BlockingOwner,
            driver: &mut FakeDriver,
            profile: &crate::ProfileSpec,
        ) -> BlockingOperationReceipt<completion::Targeted> {
            let operation = owner
                .submit_operation(driver, prepared_zoom::<completion::Targeted>(profile))
                .unwrap();
            let now = Instant::now();
            owner
                .inject_frame(
                    driver,
                    frame(
                        CameraId::CAMERA_1,
                        DecodedResponse::Ack {
                            socket: Some(ViscaSocket::S1),
                        },
                    ),
                    now,
                )
                .unwrap();
            owner
                .inject_frame(
                    driver,
                    frame(
                        CameraId::CAMERA_1,
                        DecodedResponse::Completion {
                            socket: Some(ViscaSocket::S1),
                        },
                    ),
                    now,
                )
                .unwrap();
            operation
        }

        let profile =
            crate::ProfileSpec::from_compile_time::<crate::profiles::SonyBRC300>().unwrap();
        let mut owner = BlockingOwner::new(policy(1, TransportKind::Datagram)).unwrap();
        let mut driver = FakeDriver::default();
        let operation = complete_operation(&mut owner, &mut driver, &profile);
        let peer = owner
            .submit_command(&mut driver, prepared_focus(&profile, CameraId::CAMERA_1))
            .unwrap();
        let mut reader = DeadlineReader::default();
        let mut decoder = EmptyDecoder;
        assert!(matches!(
            operation
                .settled_with_timeout(
                    owner.receipt_control(&mut driver, &mut reader, &mut decoder),
                    Duration::from_secs(1),
                )
                .wait(),
            Err(Error::RuntimeQueueFull { capacity: 1 })
        ));
        assert_eq!(driver.writes.len(), 2, "capacity failure writes no query");
        assert_eq!(owner.state().active_len(), 1, "peer remains untouched");
        peer.detach();

        let mut owner = BlockingOwner::new(policy(1, TransportKind::Datagram)).unwrap();
        let mut driver = FakeDriver::default();
        let operation = complete_operation(&mut owner, &mut driver, &profile);
        driver
            .results
            .push_back(Err(Error::TransportError("settlement query write".into())));
        let mut reader = DeadlineReader::default();
        let mut decoder = EmptyDecoder;
        assert!(matches!(
            operation
                .settled_with_timeout(
                    owner.receipt_control(&mut driver, &mut reader, &mut decoder),
                    Duration::from_secs(1),
                )
                .wait(),
            Err(Error::TransportError(reason)) if reason == "settlement query write"
        ));

        let mut owner = BlockingOwner::new(policy(1, TransportKind::Datagram)).unwrap();
        let mut driver = FakeDriver::default();
        let operation = complete_operation(&mut owner, &mut driver, &profile);
        let mut reader = ScriptedReader;
        let mut decoder = ScriptedDecoder {
            batches: VecDeque::from([vec![frame(
                CameraId::CAMERA_1,
                DecodedResponse::InquiryReply {
                    route: None,
                    payload: smallvec::smallvec![0x0],
                },
            )]]),
        };
        assert!(matches!(
            operation
                .settled_with_timeout(
                    owner.receipt_control(&mut driver, &mut reader, &mut decoder),
                    Duration::from_secs(1),
                )
                .wait(),
            Err(Error::InvalidResponseLength {
                expected: 4,
                actual: 1,
                ..
            })
        ));
        assert_eq!(owner.state().active_len(), 0);
    }

    #[test]
    fn blocking_targeted_settlement_reuses_prepared_query_until_two_snapshots_settle() {
        #[derive(Debug)]
        struct SettlementReader {
            events: VecDeque<BlockingReceive>,
        }

        impl BlockingReadDriver for SettlementReader {
            fn receive(
                &mut self,
                receive_buffer: &mut [u8],
                owner_deadline: Option<Instant>,
            ) -> Result<BlockingReceive, Error> {
                let event = self.events.pop_front().ok_or_else(|| {
                    Error::InvalidState("settlement reader event exhausted".into())
                })?;
                if event == BlockingReceive::TimedOut {
                    if let Some(deadline) = owner_deadline {
                        let now = Instant::now();
                        if deadline > now {
                            std::thread::sleep(deadline.duration_since(now));
                        }
                    }
                } else {
                    receive_buffer[0] = 1;
                }
                Ok(event)
            }
        }

        let profile =
            crate::ProfileSpec::from_compile_time::<crate::profiles::SonyBRC300>().unwrap();
        let mut owner = BlockingOwner::new(policy(2, TransportKind::Datagram)).unwrap();
        let mut driver = FakeDriver::default();
        let operation = owner
            .submit_operation(&mut driver, prepared_zoom::<completion::Targeted>(&profile))
            .unwrap();
        let now = Instant::now();
        owner
            .inject_frame(
                &mut driver,
                frame(
                    CameraId::CAMERA_1,
                    DecodedResponse::Ack {
                        socket: Some(ViscaSocket::S1),
                    },
                ),
                now,
            )
            .unwrap();
        owner
            .inject_frame(
                &mut driver,
                frame(
                    CameraId::CAMERA_1,
                    DecodedResponse::Completion {
                        socket: Some(ViscaSocket::S1),
                    },
                ),
                now,
            )
            .unwrap();
        let mut reader = SettlementReader {
            events: VecDeque::from([
                BlockingReceive::Bytes(1),
                BlockingReceive::TimedOut,
                BlockingReceive::Bytes(1),
                BlockingReceive::TimedOut,
                BlockingReceive::Bytes(1),
            ]),
        };
        let mut decoder = ScriptedDecoder {
            batches: VecDeque::from([
                vec![frame(
                    CameraId::CAMERA_1,
                    DecodedResponse::InquiryReply {
                        route: None,
                        payload: smallvec::smallvec![0x0, 0x1, 0x0, 0x0],
                    },
                )],
                vec![frame(
                    CameraId::CAMERA_1,
                    DecodedResponse::InquiryReply {
                        route: None,
                        payload: smallvec::smallvec![0x0, 0x1, 0x2, 0x0],
                    },
                )],
                vec![frame(
                    CameraId::CAMERA_1,
                    DecodedResponse::InquiryReply {
                        route: None,
                        payload: smallvec::smallvec![0x0, 0x1, 0x2, 0x0],
                    },
                )],
            ]),
        };
        let control = operation.settled_with_timeout(
            owner.receipt_control(&mut driver, &mut reader, &mut decoder),
            Duration::from_secs(1),
        );
        {
            let _settled = control.wait().unwrap();
        }

        assert!(reader.events.is_empty());
        assert!(decoder.batches.is_empty());
        assert_eq!(driver.writes.len(), 4);
        assert_eq!(driver.writes[1].1, driver.writes[2].1);
        assert_eq!(driver.writes[2].1, driver.writes[3].1);
        assert_eq!(driver.writes[1].1, vec![0x81, 0x09, 0x04, 0x47, 0xff]);
        assert_eq!(owner.state().active_len(), 0);
    }

    #[test]
    fn blocking_poll_interval_pumps_peer_frame_at_peer_deadline() {
        #[derive(Debug)]
        struct PeerReader {
            events: VecDeque<(BlockingReceive, bool)>,
        }

        impl BlockingReadDriver for PeerReader {
            fn receive(
                &mut self,
                receive_buffer: &mut [u8],
                owner_deadline: Option<Instant>,
            ) -> Result<BlockingReceive, Error> {
                let (event, wait_for_deadline) = self
                    .events
                    .pop_front()
                    .ok_or_else(|| Error::InvalidState("peer reader event exhausted".into()))?;
                if wait_for_deadline || matches!(event, BlockingReceive::TimedOut) {
                    if let Some(deadline) = owner_deadline {
                        let now = Instant::now();
                        if deadline > now {
                            std::thread::sleep(deadline.duration_since(now));
                        }
                    }
                }
                if matches!(event, BlockingReceive::Bytes(_)) {
                    receive_buffer[0] = 1;
                }
                Ok(event)
            }
        }

        let profile =
            crate::ProfileSpec::from_compile_time::<crate::profiles::SonyBRC300>().unwrap();
        let mut owner = BlockingOwner::new(policy(3, TransportKind::Datagram)).unwrap();
        let mut driver = FakeDriver::default();
        let operation = owner
            .submit_operation(&mut driver, prepared_zoom::<completion::Targeted>(&profile))
            .unwrap();
        let now = Instant::now();
        owner
            .inject_frame(
                &mut driver,
                frame(
                    CameraId::CAMERA_1,
                    DecodedResponse::Ack {
                        socket: Some(ViscaSocket::S1),
                    },
                ),
                now,
            )
            .unwrap();
        owner
            .inject_frame(
                &mut driver,
                frame(
                    CameraId::CAMERA_1,
                    DecodedResponse::Completion {
                        socket: Some(ViscaSocket::S1),
                    },
                ),
                now,
            )
            .unwrap();

        let peer = owner
            .submit(
                &mut driver,
                command(CameraId::CAMERA_2, CancellationPolicy::Supported, None),
            )
            .unwrap();
        let mut reader = PeerReader {
            events: VecDeque::from([
                (BlockingReceive::Bytes(1), false),
                (BlockingReceive::Bytes(1), true),
                (BlockingReceive::TimedOut, false),
                (BlockingReceive::Bytes(1), false),
            ]),
        };
        let mut decoder = ScriptedDecoder {
            batches: VecDeque::from([
                vec![frame(
                    CameraId::CAMERA_1,
                    DecodedResponse::InquiryReply {
                        route: None,
                        payload: smallvec::smallvec![0x0, 0x1, 0x0, 0x0],
                    },
                )],
                vec![
                    frame(
                        CameraId::CAMERA_2,
                        DecodedResponse::Ack {
                            socket: Some(ViscaSocket::S1),
                        },
                    ),
                    frame(
                        CameraId::CAMERA_2,
                        DecodedResponse::Completion {
                            socket: Some(ViscaSocket::S1),
                        },
                    ),
                ],
                vec![frame(
                    CameraId::CAMERA_1,
                    DecodedResponse::InquiryReply {
                        route: None,
                        payload: smallvec::smallvec![0x0, 0x1, 0x0, 0x0],
                    },
                )],
            ]),
        };

        {
            let _settled = operation
                .settled_with_timeout(
                    owner.receipt_control(&mut driver, &mut reader, &mut decoder),
                    Duration::from_secs(1),
                )
                .wait()
                .unwrap();
        }
        assert!(matches!(peer.terminal(), Some(RuntimeOutcome::Applied)));
        assert_eq!(owner.state().active_len(), 0);
        assert_eq!(owner.state().permits().available(), 3);
        assert_eq!(driver.writes.len(), 4);
    }

    #[test]
    fn observer_timeout_detaches_without_cancel_and_late_applied_still_caches() {
        use crate::command::semantics::WriteOnlyState;

        let profile =
            crate::ProfileSpec::from_compile_time::<crate::profiles::GenericVisca>().unwrap();
        let mut owner = BlockingOwner::new(policy(1, TransportKind::Datagram)).unwrap();
        let mut driver = FakeDriver::default();
        let receipt = owner
            .submit_command(
                &mut driver,
                prepared_limit_clear(&profile, CameraId::CAMERA_1),
            )
            .unwrap();
        let mut reader = DeadlineReader::default();
        let mut decoder = EmptyDecoder;
        assert!(matches!(
            receipt.wait_with_timeout(
                &mut owner.receipt_control(&mut driver, &mut reader, &mut decoder),
                Duration::ZERO,
            ),
            Err(Error::Timeout)
        ));
        assert_eq!(driver.writes.len(), 1);
        assert!(!driver.writes[0].2);
        assert_eq!(owner.state().active_len(), 1);

        let now = Instant::now();
        owner
            .inject_frame(
                &mut driver,
                frame(
                    CameraId::CAMERA_1,
                    DecodedResponse::Ack {
                        socket: Some(ViscaSocket::S1),
                    },
                ),
                now,
            )
            .unwrap();
        owner
            .inject_frame(
                &mut driver,
                frame(
                    CameraId::CAMERA_1,
                    DecodedResponse::Completion {
                        socket: Some(ViscaSocket::S1),
                    },
                ),
                now,
            )
            .unwrap();
        assert!(owner
            .state()
            .cached(CameraId::CAMERA_1, WriteOnlyState::PanTiltLimits)
            .is_some());
    }

    #[test]
    fn terminalized_stream_pump_outcome_precedes_local_pump_error() {
        let profile =
            crate::ProfileSpec::from_compile_time::<crate::profiles::GenericVisca>().unwrap();
        let mut owner = BlockingOwner::new(policy(1, TransportKind::Stream)).unwrap();
        let mut driver = FakeDriver::default();
        let receipt = owner
            .submit_command(&mut driver, prepared_focus(&profile, CameraId::CAMERA_1))
            .unwrap();
        let mut reader = DeadlineReader {
            deadline: None,
            result: Some(Err(Error::ConnectionClosed {
                reason: Some("peer closed".into()),
            })),
        };
        let mut decoder = EmptyDecoder;
        assert!(matches!(
            receipt.wait(&mut owner.receipt_control(&mut driver, &mut reader, &mut decoder,)),
            Err(Error::ConnectionClosed { .. })
        ));
    }

    /// Issue #565: a transient read failure retries the in-flight command and
    /// leaves the session running; the pump reports "no frames", not an error.
    #[test]
    fn transient_blocking_read_fault_retries_and_keeps_the_session() {
        let profile =
            crate::ProfileSpec::from_compile_time::<crate::profiles::GenericVisca>().unwrap();
        let mut owner = BlockingOwner::new(policy(1, TransportKind::Datagram)).unwrap();
        let mut driver = FakeDriver::default();
        let _receipt = owner
            .submit_command(&mut driver, prepared_focus(&profile, CameraId::CAMERA_1))
            .unwrap();
        let mut reader = DeadlineReader {
            deadline: None,
            result: Some(Err(Error::Io(std::sync::Arc::new(std::io::Error::from(
                std::io::ErrorKind::ConnectionRefused,
            ))))),
        };
        let mut decoder = EmptyDecoder;
        assert_eq!(
            owner
                .pump_once(&mut driver, &mut reader, &mut decoder)
                .unwrap(),
            0,
            "a transient read fault produces no frames and no error"
        );
        assert_eq!(owner.state().state(), SessionState::Running);
        assert_eq!(
            owner.state().active_len(),
            1,
            "the in-flight command is retried, not failed"
        );
        // The retry is dispatched by an ordinary later owner turn.
        for _ in 0..4 {
            if driver.writes.len() >= 2 {
                break;
            }
            let Some(wake) = owner.state().next_wake() else {
                break;
            };
            owner.wake(&mut driver, wake).unwrap();
        }
        assert_eq!(driver.writes.len(), 2, "the same request is written again");
        assert_eq!(driver.writes[0].0, driver.writes[1].0);
    }

    /// A fatal read ends the session on a stream transport as a close, not as
    /// a byte-stream poison: the read consumed nothing to desynchronize.
    #[test]
    fn fatal_blocking_read_fault_closes_the_stream_session() {
        let profile =
            crate::ProfileSpec::from_compile_time::<crate::profiles::GenericVisca>().unwrap();
        let mut owner = BlockingOwner::new(policy(1, TransportKind::Stream)).unwrap();
        let mut driver = FakeDriver::default();
        let _receipt = owner
            .submit_command(&mut driver, prepared_focus(&profile, CameraId::CAMERA_1))
            .unwrap();
        let mut reader = DeadlineReader {
            deadline: None,
            result: Some(Err(Error::Io(std::sync::Arc::new(std::io::Error::from(
                std::io::ErrorKind::BrokenPipe,
            ))))),
        };
        let mut decoder = EmptyDecoder;
        let error = owner
            .pump_once(&mut driver, &mut reader, &mut decoder)
            .expect_err("a fatal read ends the session");
        // Issue #629: the caller is told the session's verdict, not the raw
        // read fault. `Error::Io` classifies as survivable, so returning it here
        // would tell an auto-reconnect loop to keep using a dead session.
        let Error::ConnectionClosed { reason } = &error else {
            panic!("a fatal read must report the session close, got {error:?}");
        };
        let reason = reason.as_ref().expect("the read fault names the close");
        assert!(
            reason.contains("broken pipe"),
            "the transport cause must survive in the close reason: {reason}"
        );
        assert!(
            error.requires_new_session(),
            "a closed session must classify as needing a replacement"
        );
        assert_eq!(owner.state().state(), SessionState::Closed);
    }

    /// Issue #629 probe: a fatal read during settlement polling escaped through
    /// `pump_until_sample_boundary`, which has no receipt to consult, and
    /// reached the caller as the raw `Error::Io` while the session was already
    /// `Closed`.
    #[test]
    fn fatal_read_during_settlement_polling_reports_the_session_boundary_error() {
        #[derive(Debug)]
        struct PollingFaultReader {
            events: VecDeque<Result<BlockingReceive, Error>>,
        }

        impl BlockingReadDriver for PollingFaultReader {
            fn receive(
                &mut self,
                receive_buffer: &mut [u8],
                _owner_deadline: Option<Instant>,
            ) -> Result<BlockingReceive, Error> {
                let event = self.events.pop_front().ok_or_else(|| {
                    Error::InvalidState("polling fault reader event exhausted".into())
                })?;
                if matches!(event, Ok(BlockingReceive::Bytes(_))) {
                    receive_buffer[0] = 1;
                }
                event
            }
        }

        let profile =
            crate::ProfileSpec::from_compile_time::<crate::profiles::SonyBRC300>().unwrap();
        let mut owner = BlockingOwner::new(policy(2, TransportKind::Stream)).unwrap();
        let mut driver = FakeDriver::default();
        let operation = owner
            .submit_operation(&mut driver, prepared_zoom::<completion::Targeted>(&profile))
            .unwrap();
        let now = Instant::now();
        owner
            .inject_frame(
                &mut driver,
                frame(
                    CameraId::CAMERA_1,
                    DecodedResponse::Ack {
                        socket: Some(ViscaSocket::S1),
                    },
                ),
                now,
            )
            .unwrap();
        owner
            .inject_frame(
                &mut driver,
                frame(
                    CameraId::CAMERA_1,
                    DecodedResponse::Completion {
                        socket: Some(ViscaSocket::S1),
                    },
                ),
                now,
            )
            .unwrap();

        // The baseline snapshot succeeds; the connection dies in the interval
        // pump before the next sample can be taken.
        let mut reader = PollingFaultReader {
            events: VecDeque::from([
                Ok(BlockingReceive::Bytes(1)),
                Err(Error::Io(std::sync::Arc::new(std::io::Error::from(
                    std::io::ErrorKind::ConnectionReset,
                )))),
            ]),
        };
        let mut decoder = ScriptedDecoder {
            batches: VecDeque::from([vec![frame(
                CameraId::CAMERA_1,
                DecodedResponse::InquiryReply {
                    route: None,
                    payload: smallvec::smallvec![0x0, 0x1, 0x0, 0x0],
                },
            )]]),
        };
        let error = operation
            .settled_with_timeout(
                owner.receipt_control(&mut driver, &mut reader, &mut decoder),
                Duration::from_secs(1),
            )
            .wait()
            .expect_err("a fatal read during polling ends the settlement wait");
        assert!(
            matches!(error, Error::ConnectionClosed { .. }),
            "settlement must report the session close, got {error:?}"
        );
        assert!(
            error.requires_new_session(),
            "the settlement error must classify as needing a replacement session"
        );
        assert_eq!(owner.state().state(), SessionState::Closed);
    }

    /// Issue #629: the cancellation observer's pump is the other escape site.
    /// Its own terminal observation wins; without one the session's boundary
    /// error must be reported, never the raw read fault.
    #[test]
    fn fatal_read_while_observing_cancellation_reports_the_session_boundary_error() {
        let mut owner = BlockingOwner::new(policy(1, TransportKind::Stream)).unwrap();
        let mut driver = FakeDriver::default();
        let operation = owner
            .submit(
                &mut driver,
                command(CameraId::CAMERA_1, CancellationPolicy::Supported, None),
            )
            .unwrap();
        let cancellation = owner.cancel_test(&mut driver, operation).unwrap();
        let mut reader = DeadlineReader {
            deadline: None,
            result: Some(Err(Error::Io(std::sync::Arc::new(std::io::Error::from(
                std::io::ErrorKind::ConnectionReset,
            ))))),
        };
        let mut decoder = EmptyDecoder;
        let error = cancellation
            .outcome(
                &mut owner.receipt_control(&mut driver, &mut reader, &mut decoder),
                Duration::from_secs(1),
            )
            .expect_err("a fatal read ends the cancellation observation");
        assert!(
            matches!(error, Error::ConnectionClosed { .. }),
            "cancellation must report the session close, got {error:?}"
        );
        assert!(
            error.requires_new_session(),
            "the cancellation error must classify as needing a replacement session"
        );
        assert_eq!(owner.state().state(), SessionState::Closed);
    }

    #[test]
    fn decoded_batch_drains_ack_effects_before_equal_completion_deadline() {
        let mut a_request = command(CameraId::CAMERA_1, CancellationPolicy::Supported, None);
        match &mut a_request {
            RuntimeRequest::Command { context, .. } => {
                context.timeout.ack = Duration::from_secs(2);
                context.timeout.completion = Duration::from_secs(1);
            }
            RuntimeRequest::Inquiry { .. } => unreachable!(),
        }
        let mut b_request = command(CameraId::CAMERA_1, CancellationPolicy::Supported, None);
        match &mut b_request {
            RuntimeRequest::Command { context, .. } => {
                context.timeout.ack = Duration::from_secs(2);
            }
            RuntimeRequest::Inquiry { .. } => unreachable!(),
        }

        let mut owner = BlockingOwner::new(policy(2, TransportKind::Datagram)).unwrap();
        let mut driver = FakeDriver::default();
        let a = owner.submit(&mut driver, a_request).unwrap();
        let a_id = a.id();
        let a_ack_at = Instant::now();
        owner
            .inject_frame(
                &mut driver,
                frame(
                    CameraId::CAMERA_1,
                    DecodedResponse::Ack {
                        socket: Some(ViscaSocket::S1),
                    },
                ),
                a_ack_at,
            )
            .unwrap();
        let deadline = a_ack_at + Duration::from_secs(1);

        let b = owner.submit(&mut driver, b_request).unwrap();
        let b_id = b.id();
        let _cancellation = owner.cancel_test(&mut driver, b).unwrap();
        let _ = owner.drain_diagnostics();

        // This is the exact validated-batch replay seam used by pump_once: B's
        // ACK must be fully drained (including its cancel write/result) before
        // A's completion, and only then may due work run at the shared instant.
        owner.drive_decoded_batch(
            &mut driver,
            vec![
                frame(
                    CameraId::CAMERA_1,
                    DecodedResponse::Ack {
                        socket: Some(ViscaSocket::S2),
                    },
                ),
                frame(
                    CameraId::CAMERA_1,
                    DecodedResponse::Completion {
                        socket: Some(ViscaSocket::S1),
                    },
                ),
            ],
            deadline,
        );

        assert!(matches!(a.terminal(), Some(RuntimeOutcome::Applied)));
        let diagnostics: Vec<_> = owner.state().diagnostics().copied().collect();
        let ack_b = diagnostics
            .iter()
            .position(|event| {
                matches!(
                    event,
                    DiagnosticEvent::FrameReceived {
                        response: ResponseDiagnostic::Ack(Some(ViscaSocket::S2)),
                        ..
                    }
                )
            })
            .expect("B ACK diagnostic");
        let cancel_write = diagnostics
            .iter()
            .position(|event| {
                matches!(
                    event,
                    DiagnosticEvent::WriteFinished {
                        id,
                        cancellation: true,
                        ..
                    } if *id == b_id
                )
            })
            .expect("recursive B cancel write diagnostic");
        let completion_a = diagnostics
            .iter()
            .position(|event| {
                matches!(
                    event,
                    DiagnosticEvent::FrameReceived {
                        response: ResponseDiagnostic::Completion(Some(ViscaSocket::S1)),
                        ..
                    }
                )
            })
            .expect("A completion diagnostic");
        let applied_a = diagnostics
            .iter()
            .position(|event| {
                matches!(
                    event,
                    DiagnosticEvent::Terminal {
                        id,
                        outcome: OutcomeDiagnostic::Applied,
                        ..
                    } if *id == a_id
                )
            })
            .expect("A applied diagnostic");
        assert!(ack_b < cancel_write);
        assert!(cancel_write < completion_a);
        assert!(completion_a < applied_a);
        assert!(!diagnostics.iter().any(|event| matches!(
            event,
            DiagnosticEvent::RetryScheduled { id, .. } if *id == a_id
        )));
    }

    #[test]
    fn blocking_pump_uses_recursive_turn_order_for_a_decoded_batch() {
        let mut owner = BlockingOwner::new(policy(2, TransportKind::Datagram)).unwrap();
        let mut driver = FakeDriver::default();
        let a = owner
            .submit(
                &mut driver,
                command(CameraId::CAMERA_1, CancellationPolicy::Supported, None),
            )
            .unwrap();
        let now = Instant::now();
        owner
            .inject_frame(
                &mut driver,
                frame(
                    CameraId::CAMERA_1,
                    DecodedResponse::Ack {
                        socket: Some(ViscaSocket::S1),
                    },
                ),
                now,
            )
            .unwrap();
        let b = owner
            .submit(
                &mut driver,
                command(CameraId::CAMERA_1, CancellationPolicy::Supported, None),
            )
            .unwrap();
        let b_id = b.id();
        let _cancellation = owner.cancel_test(&mut driver, b).unwrap();
        let _ = owner.drain_diagnostics();

        let mut reader = ScriptedReader;
        let mut decoder = ScriptedDecoder {
            batches: VecDeque::from([vec![
                frame(
                    CameraId::CAMERA_1,
                    DecodedResponse::Ack {
                        socket: Some(ViscaSocket::S2),
                    },
                ),
                frame(
                    CameraId::CAMERA_1,
                    DecodedResponse::Completion {
                        socket: Some(ViscaSocket::S1),
                    },
                ),
            ]]),
        };
        assert_eq!(
            owner
                .pump_once(&mut driver, &mut reader, &mut decoder)
                .unwrap(),
            2
        );
        assert!(matches!(a.terminal(), Some(RuntimeOutcome::Applied)));

        let diagnostics: Vec<_> = owner.state().diagnostics().copied().collect();
        let cancel_write = diagnostics
            .iter()
            .position(|event| {
                matches!(
                    event,
                    DiagnosticEvent::WriteFinished {
                        id,
                        cancellation: true,
                        ..
                    } if *id == b_id
                )
            })
            .expect("recursive cancel write");
        let completion = diagnostics
            .iter()
            .position(|event| {
                matches!(
                    event,
                    DiagnosticEvent::FrameReceived {
                        response: ResponseDiagnostic::Completion(Some(ViscaSocket::S1)),
                        ..
                    }
                )
            })
            .expect("wire-next completion");
        assert!(cancel_write < completion);
    }

    #[test]
    fn blocking_fixture_first_write_and_out_of_order_retention() {
        let mut owner = BlockingOwner::new(policy(8, TransportKind::Datagram)).unwrap();
        let mut driver = FakeDriver::default();
        let a = owner
            .submit(
                &mut driver,
                command(CameraId::CAMERA_1, CancellationPolicy::Supported, None),
            )
            .unwrap();
        let b = owner
            .submit(
                &mut driver,
                command(CameraId::CAMERA_1, CancellationPolicy::Supported, None),
            )
            .unwrap();
        assert_eq!(
            driver.writes.len(),
            2,
            "submission performs one exact write"
        );
        assert!(a.terminal().is_none());
        assert!(b.terminal().is_none());

        let now = Instant::now();
        owner
            .inject_frame(
                &mut driver,
                frame(
                    CameraId::CAMERA_1,
                    DecodedResponse::Ack {
                        socket: Some(ViscaSocket::S1),
                    },
                ),
                now,
            )
            .unwrap();
        owner
            .inject_frame(
                &mut driver,
                frame(
                    CameraId::CAMERA_1,
                    DecodedResponse::Ack {
                        socket: Some(ViscaSocket::S2),
                    },
                ),
                now,
            )
            .unwrap();
        owner
            .inject_frame(
                &mut driver,
                frame(
                    CameraId::CAMERA_1,
                    DecodedResponse::Completion {
                        socket: Some(ViscaSocket::S2),
                    },
                ),
                now,
            )
            .unwrap();
        owner
            .inject_frame(
                &mut driver,
                frame(
                    CameraId::CAMERA_1,
                    DecodedResponse::Completion {
                        socket: Some(ViscaSocket::S1),
                    },
                ),
                now,
            )
            .unwrap();

        assert!(matches!(b.terminal(), Some(RuntimeOutcome::Applied)));
        assert!(matches!(a.terminal(), Some(RuntimeOutcome::Applied)));
        assert_eq!(owner.state().permits().available(), 8);
    }

    #[test]
    fn blocking_owner_matches_canonical_lifecycle_trace() {
        let mut owner = BlockingOwner::new(policy(1, TransportKind::Datagram)).unwrap();
        let mut driver = FakeDriver::default();
        let receipt = owner
            .submit(
                &mut driver,
                command(CameraId::CAMERA_1, CancellationPolicy::Supported, None),
            )
            .unwrap();
        let now = Instant::now();
        owner
            .inject_frame(
                &mut driver,
                frame(
                    CameraId::CAMERA_1,
                    DecodedResponse::Ack {
                        socket: Some(ViscaSocket::S1),
                    },
                ),
                now,
            )
            .unwrap();
        owner
            .inject_frame(
                &mut driver,
                frame(
                    CameraId::CAMERA_1,
                    DecodedResponse::Completion {
                        socket: Some(ViscaSocket::S1),
                    },
                ),
                now,
            )
            .unwrap();
        assert!(matches!(receipt.terminal(), Some(RuntimeOutcome::Applied)));
        assert_eq!(
            canonical_owner_trace(owner.state().diagnostics().copied()),
            CANONICAL_OWNER_TRACE
        );
    }

    #[test]
    fn ready_effects_preserve_source_order_and_write_completion_is_recursive() {
        let mut state = OwnerState::new(policy(2, TransportKind::Datagram)).unwrap();
        let permit = state.permits().try_acquire().unwrap();
        let (input, _observer, _admission) = state.stage_admission(
            command(CameraId::CAMERA_1, CancellationPolicy::Supported, None),
            permit,
        );
        let mut effects = state.input(input, Instant::now());
        // Models an unrelated already-ready source following the engine batch.
        effects.push_back(Effect::Ignored(IgnoreReason::MalformedFrame));
        let mut driver = FakeDriver::default();
        while let Some(effect) = effects.pop_front() {
            if let AppliedEffect::Transmit(staged) = state.apply_effect(effect) {
                let result = match state.prepare_write(&staged) {
                    Ok(write) => driver.write(write),
                    Err(error) => Err(error),
                };
                let produced = state.finish_write(&staged, result, Instant::now());
                prepend_effects(&mut effects, produced);
            }
        }
        let diagnostics: Vec<_> = state.diagnostics().copied().collect();
        let write = diagnostics
            .iter()
            .position(|event| matches!(event, DiagnosticEvent::WriteFinished { .. }))
            .unwrap();
        let completion_transition = diagnostics
            .iter()
            .position(|event| {
                matches!(
                    event,
                    DiagnosticEvent::Transition {
                        to: Phase::AwaitingAck { .. },
                        ..
                    }
                )
            })
            .unwrap();
        let unrelated = diagnostics
            .iter()
            .position(|event| {
                matches!(
                    event,
                    DiagnosticEvent::Ignored(IgnoreReason::MalformedFrame)
                )
            })
            .unwrap();
        assert!(write < completion_transition);
        assert!(completion_transition < unrelated);
        assert_eq!(driver.writes.len(), 1);
    }

    #[test]
    fn capacity_failure_creates_no_id_observer_or_write() {
        let mut owner = BlockingOwner::new(policy(1, TransportKind::Datagram)).unwrap();
        let mut driver = FakeDriver::default();
        let first = owner
            .submit(
                &mut driver,
                command(CameraId::CAMERA_1, CancellationPolicy::Supported, None),
            )
            .unwrap();
        let error = owner
            .submit(
                &mut driver,
                command(CameraId::CAMERA_1, CancellationPolicy::Supported, None),
            )
            .unwrap_err();
        assert!(matches!(error, Error::RuntimeQueueFull { capacity: 1 }));
        assert_eq!(owner.state().active_len(), 1);
        assert_eq!(owner.state().metrics().admitted, 1);
        assert_eq!(driver.writes.len(), 1);
        drop(first);
        // Detach is observer-only; capacity remains owned by engine state.
        assert_eq!(owner.state().permits().available(), 0);
    }

    /// Issue #561: a submission that cannot win the socket queues instead of
    /// terminalizing, and it never waits on or disturbs the busy peer.
    #[test]
    fn blocking_submission_queues_instead_of_waiting_for_another_socket() {
        let mut owner_policy = policy(3, TransportKind::Datagram);
        owner_policy.targets[usize::from(CameraId::CAMERA_1.id())] = Some(TargetPolicy {
            command_sockets: 1,
            cancellation: CancellationPolicy::Supported,
        });
        let mut owner = BlockingOwner::new(owner_policy).unwrap();
        let mut driver = FakeDriver::default();
        let first = owner
            .submit(
                &mut driver,
                command(CameraId::CAMERA_1, CancellationPolicy::Supported, None),
            )
            .unwrap();
        let first_id = first.id();
        let first_before = owner.state().request_state(first_id).unwrap();
        let _ = owner.drain_diagnostics();
        let queued = owner
            .submit(
                &mut driver,
                command(CameraId::CAMERA_1, CancellationPolicy::Supported, None),
            )
            .expect("a busy socket queues rather than failing the submission");
        let queued_id = queued.id();
        assert!(queued.terminal().is_none());
        assert!(matches!(
            owner.state().request_state(queued_id),
            Some((Phase::Ready { .. }, _))
        ));
        assert_eq!(
            driver.writes.len(),
            1,
            "the queued request performs no write"
        );
        assert_eq!(owner.state().request_state(first_id), Some(first_before));
        assert_eq!(owner.state().active_len(), 2);
        assert_eq!(owner.state().permits().available(), 1);
        assert_eq!(
            driver
                .writes
                .iter()
                .filter(|(_, _, cancellation)| *cancellation)
                .count(),
            0
        );
        assert!(!owner.state().diagnostics().any(|event| matches!(
            event,
            DiagnosticEvent::CancellationRecorded { .. }
                | DiagnosticEvent::CancellationObserved { .. }
        )));

        // Freeing the only socket drains the queue on the very same turn.
        let now = Instant::now();
        owner
            .inject_frame(
                &mut driver,
                frame(
                    CameraId::CAMERA_1,
                    DecodedResponse::Ack {
                        socket: Some(ViscaSocket::S1),
                    },
                ),
                now,
            )
            .unwrap();
        owner
            .inject_frame(
                &mut driver,
                frame(
                    CameraId::CAMERA_1,
                    DecodedResponse::Completion {
                        socket: Some(ViscaSocket::S1),
                    },
                ),
                now,
            )
            .unwrap();
        assert!(matches!(first.terminal(), Some(RuntimeOutcome::Applied)));
        assert_eq!(driver.writes.len(), 2, "the queued request is written next");
        assert_eq!(driver.writes[1].0, queued_id);
        drop(queued);
    }

    /// Issue #561: queueing is bounded by admission capacity, not by sockets.
    #[test]
    fn blocking_queue_depth_still_rejects_beyond_admission_capacity() {
        let mut owner_policy = policy(2, TransportKind::Datagram);
        owner_policy.targets[usize::from(CameraId::CAMERA_1.id())] = Some(TargetPolicy {
            command_sockets: 1,
            cancellation: CancellationPolicy::Supported,
        });
        let mut owner = BlockingOwner::new(owner_policy).unwrap();
        let mut driver = FakeDriver::default();
        let written = owner
            .submit(
                &mut driver,
                command(CameraId::CAMERA_1, CancellationPolicy::Supported, None),
            )
            .unwrap();
        let queued = owner
            .submit(
                &mut driver,
                command(CameraId::CAMERA_1, CancellationPolicy::Supported, None),
            )
            .expect("the second request fills the bounded queue");
        let error = owner
            .submit(
                &mut driver,
                command(CameraId::CAMERA_1, CancellationPolicy::Supported, None),
            )
            .unwrap_err();
        assert!(matches!(error, Error::RuntimeQueueFull { capacity: 2 }));
        assert_eq!(driver.writes.len(), 1);
        assert_eq!(owner.state().active_len(), 2);
        drop(written);
        drop(queued);
    }

    /// Issue #561: three concurrent blocking submissions over two sockets all
    /// succeed, and the queued one is written and completed in submit order.
    #[test]
    fn blocking_submissions_beyond_the_socket_count_all_complete_in_order() {
        let mut owner = BlockingOwner::new(policy(8, TransportKind::Datagram)).unwrap();
        let mut driver = FakeDriver::default();
        let receipts: Vec<_> = (0..3)
            .map(|index| {
                owner
                    .submit(
                        &mut driver,
                        command(CameraId::CAMERA_1, CancellationPolicy::Supported, None),
                    )
                    .unwrap_or_else(|error| panic!("submission {index} must not fail: {error:?}"))
            })
            .collect();
        let ids: Vec<_> = receipts.iter().map(ReceiptCore::id).collect();
        assert_eq!(driver.writes.len(), 2, "only two sockets are available");
        assert_eq!(
            driver
                .writes
                .iter()
                .map(|(id, _, _)| *id)
                .collect::<Vec<_>>(),
            ids[..2]
        );
        assert!(receipts.iter().all(|receipt| receipt.terminal().is_none()));

        let now = Instant::now();
        for socket in [ViscaSocket::S1, ViscaSocket::S2] {
            owner
                .inject_frame(
                    &mut driver,
                    frame(
                        CameraId::CAMERA_1,
                        DecodedResponse::Ack {
                            socket: Some(socket),
                        },
                    ),
                    now,
                )
                .unwrap();
        }
        owner
            .inject_frame(
                &mut driver,
                frame(
                    CameraId::CAMERA_1,
                    DecodedResponse::Completion {
                        socket: Some(ViscaSocket::S1),
                    },
                ),
                now,
            )
            .unwrap();
        assert!(matches!(
            receipts[0].terminal(),
            Some(RuntimeOutcome::Applied)
        ));
        assert_eq!(driver.writes.len(), 3, "the queued request drains next");
        assert_eq!(driver.writes[2].0, ids[2]);

        owner
            .inject_frame(
                &mut driver,
                frame(
                    CameraId::CAMERA_1,
                    DecodedResponse::Completion {
                        socket: Some(ViscaSocket::S2),
                    },
                ),
                now,
            )
            .unwrap();
        assert!(matches!(
            receipts[1].terminal(),
            Some(RuntimeOutcome::Applied)
        ));
        owner
            .inject_frame(
                &mut driver,
                frame(
                    CameraId::CAMERA_1,
                    DecodedResponse::Ack {
                        socket: Some(ViscaSocket::S1),
                    },
                ),
                now,
            )
            .unwrap();
        owner
            .inject_frame(
                &mut driver,
                frame(
                    CameraId::CAMERA_1,
                    DecodedResponse::Completion {
                        socket: Some(ViscaSocket::S1),
                    },
                ),
                now,
            )
            .unwrap();
        assert!(matches!(
            receipts[2].terminal(),
            Some(RuntimeOutcome::Applied)
        ));
        assert_eq!(owner.state().active_len(), 0);
    }

    #[test]
    fn paced_submission_does_not_advance_an_unrelated_ack_deadline() {
        let mut owner_policy = policy(2, TransportKind::Datagram);
        owner_policy.protocol.command_spacing = Duration::from_millis(30);
        let mut owner = BlockingOwner::new(owner_policy).unwrap();
        let mut driver = FakeDriver::default();

        let mut a_request = command(CameraId::CAMERA_1, CancellationPolicy::Supported, None);
        match &mut a_request {
            RuntimeRequest::Command { context, .. } => {
                context.timeout.ack = Duration::from_millis(5);
            }
            RuntimeRequest::Inquiry { .. } => unreachable!(),
        }
        let a = owner.submit(&mut driver, a_request).unwrap();
        let a_id = a.id();
        let before = owner.state().request_state(a_id).unwrap();
        assert!(matches!(before.0, Phase::AwaitingAck { .. }));

        let b = owner
            .submit(
                &mut driver,
                command(CameraId::CAMERA_1, CancellationPolicy::Supported, None),
            )
            .unwrap();
        assert_eq!(driver.writes.len(), 2, "paced B performs its own write");
        assert_eq!(owner.state().request_state(a_id), Some(before));
        assert!(a.terminal().is_none());
        assert_eq!(owner.state().active_len(), 2);

        owner.wake(&mut driver, Instant::now()).unwrap();
        assert!(matches!(
            a.terminal(),
            Some(RuntimeOutcome::Failed(Error::Timeout))
        ));
        drop(b);
    }

    #[test]
    fn blocking_reentrancy_fails_before_admission_or_write() {
        let mut owner = BlockingOwner::new(policy(1, TransportKind::Datagram)).unwrap();
        owner.mark_pumping_for_test();
        let mut driver = FakeDriver::default();
        let error = owner
            .submit(
                &mut driver,
                command(CameraId::CAMERA_1, CancellationPolicy::Supported, None),
            )
            .unwrap_err();
        assert!(matches!(error, Error::TransportBusy));
        assert_eq!(owner.state().active_len(), 0);
        assert!(driver.writes.is_empty());
    }

    #[test]
    fn detached_late_completion_still_updates_cache_and_subscription() {
        let mut owner = BlockingOwner::new(policy(2, TransportKind::Datagram)).unwrap();
        let subscription = owner
            .state_mut()
            .subscribe_applied(Some(CameraId::CAMERA_1), 1)
            .unwrap();
        let projection =
            AppliedStateProjection::set(WriteOnlyState::PanTiltLimits, &[42, -3]).unwrap();
        let mut driver = FakeDriver::default();
        let receipt = owner
            .submit(
                &mut driver,
                command(
                    CameraId::CAMERA_1,
                    CancellationPolicy::Supported,
                    Some(projection),
                ),
            )
            .unwrap();
        drop(receipt);
        let now = Instant::now();
        owner
            .inject_frame(
                &mut driver,
                frame(
                    CameraId::CAMERA_1,
                    DecodedResponse::Ack {
                        socket: Some(ViscaSocket::S1),
                    },
                ),
                now,
            )
            .unwrap();
        owner
            .inject_frame(
                &mut driver,
                frame(
                    CameraId::CAMERA_1,
                    DecodedResponse::Completion {
                        socket: Some(ViscaSocket::S1),
                    },
                ),
                now,
            )
            .unwrap();
        assert_eq!(
            owner
                .state()
                .cached(CameraId::CAMERA_1, WriteOnlyState::PanTiltLimits,),
            Some(projection)
        );
        assert_eq!(subscription.try_recv().unwrap().0.projection, projection);
        assert_eq!(owner.state().active_len(), 0);
    }

    #[test]
    fn prepared_pan_tilt_limit_updates_target_cache_only_after_applied() {
        use crate::{
            command::{semantics::WriteOnlyState, PanTiltLimitCorner},
            prepared::prepare_command,
            request::builtin::{PanTiltLimitClear, PanTiltLimitSet},
            units::Degrees,
            OperationalTuning, ProfileSpec,
        };

        let profile = ProfileSpec::from_compile_time::<crate::profiles::GenericVisca>()
            .expect("built-in profile");
        let set = PanTiltLimitSet::for_profile(
            PanTiltLimitCorner::UpRight,
            Degrees(45.0),
            Degrees(-15.0),
            &profile,
        )
        .expect("pan/tilt limit preparation");
        let request = prepare_command(&set, CameraId::CAMERA_1, &profile, OperationalTuning::new())
            .expect("pan/tilt limit preparation")
            .admit_with(|request, _timeout| request);
        let mut owner = BlockingOwner::new(policy(2, TransportKind::Datagram)).unwrap();
        let mut driver = FakeDriver::default();
        let receipt = owner.submit(&mut driver, request).unwrap();
        assert_eq!(
            owner
                .state()
                .cached(CameraId::CAMERA_1, WriteOnlyState::PanTiltLimits),
            None
        );
        drop(receipt);

        let now = Instant::now();
        owner
            .inject_frame(
                &mut driver,
                frame(
                    CameraId::CAMERA_1,
                    DecodedResponse::Ack {
                        socket: Some(ViscaSocket::S1),
                    },
                ),
                now,
            )
            .unwrap();
        assert_eq!(
            owner
                .state()
                .cached(CameraId::CAMERA_1, WriteOnlyState::PanTiltLimits),
            None
        );
        owner
            .inject_frame(
                &mut driver,
                frame(
                    CameraId::CAMERA_1,
                    DecodedResponse::Completion {
                        socket: Some(ViscaSocket::S1),
                    },
                ),
                now,
            )
            .unwrap();
        let cached = owner
            .state()
            .cached(CameraId::CAMERA_1, WriteOnlyState::PanTiltLimits)
            .expect("pan/tilt limit cached after exact application");
        assert!(matches!(
            cached,
            AppliedStateProjection::Set { value, .. }
                if value.value_count == 3
        ));

        let clear = prepare_command(
            &PanTiltLimitClear::new(PanTiltLimitCorner::UpRight),
            CameraId::CAMERA_1,
            &profile,
            OperationalTuning::new(),
        )
        .expect("pan/tilt limit clear preparation")
        .admit_with(|request, _timeout| request);
        let clear_receipt = owner.submit(&mut driver, clear).unwrap();
        assert_eq!(
            owner
                .state()
                .cached(CameraId::CAMERA_1, WriteOnlyState::PanTiltLimits),
            Some(cached)
        );
        drop(clear_receipt);
        owner
            .inject_frame(
                &mut driver,
                frame(
                    CameraId::CAMERA_1,
                    DecodedResponse::Ack {
                        socket: Some(ViscaSocket::S1),
                    },
                ),
                now,
            )
            .unwrap();
        assert_eq!(
            owner
                .state()
                .cached(CameraId::CAMERA_1, WriteOnlyState::PanTiltLimits),
            Some(cached)
        );
        owner
            .inject_frame(
                &mut driver,
                frame(
                    CameraId::CAMERA_1,
                    DecodedResponse::Completion {
                        socket: Some(ViscaSocket::S1),
                    },
                ),
                now,
            )
            .unwrap();
        assert_eq!(
            owner
                .state()
                .cached(CameraId::CAMERA_1, WriteOnlyState::PanTiltLimits),
            Some(
                AppliedStateProjection::clear_with_values(
                    WriteOnlyState::PanTiltLimits,
                    &[i64::from(PanTiltLimitCorner::UpRight.to_byte())],
                )
                .expect("corner discriminator"),
            )
        );
    }

    #[test]
    fn target_cache_preserves_set_clear_and_invalidate_with_bounded_targets() {
        use crate::command::semantics::WriteOnlyState;

        let set = AppliedStateProjection::set(WriteOnlyState::Spotlight, &[1]).unwrap();
        let clear = AppliedStateProjection::clear(WriteOnlyState::ImageFreeze);
        let invalidate = AppliedStateProjection::invalidate(WriteOnlyState::TallyMode);
        let mut cache = TargetStateCache::default();

        cache.apply(set, 2);
        cache.apply(clear, 2);
        cache.apply(invalidate, 2);

        // The oldest key is evicted, while Clear and Invalidate remain
        // distinguishable entries rather than collapsing to `None`.
        assert_eq!(cache.get(WriteOnlyState::Spotlight), None);
        assert_eq!(cache.get(WriteOnlyState::ImageFreeze), Some(clear));
        assert_eq!(cache.get(WriteOnlyState::TallyMode), Some(invalidate));

        let mut target_one = [TargetStateCache::default(), TargetStateCache::default()];
        target_one[0].apply(set, 1);
        target_one[1].apply(clear, 1);
        assert_eq!(target_one[0].get(WriteOnlyState::Spotlight), Some(set));
        assert_eq!(target_one[0].get(WriteOnlyState::ImageFreeze), None);
        assert_eq!(target_one[1].get(WriteOnlyState::ImageFreeze), Some(clear));
        assert_eq!(target_one[1].get(WriteOnlyState::Spotlight), None);
    }

    #[test]
    fn prepared_plain_and_inquiry_admit_on_cancellation_capable_target() {
        use crate::{
            command::PowerInquiry,
            prepared::{prepare_command, prepare_inquiry},
            request::builtin::FocusModeCommand,
            OperationalTuning, ProfileSpec,
        };

        let profile = ProfileSpec::from_compile_time::<crate::profiles::GenericVisca>()
            .expect("built-in profile");
        let plain = prepare_command(
            &FocusModeCommand::Manual,
            CameraId::CAMERA_1,
            &profile,
            OperationalTuning::new(),
        )
        .expect("plain preparation")
        .admit_with(|request, _timeout| request);
        let inquiry = prepare_inquiry(
            &PowerInquiry,
            CameraId::CAMERA_1,
            &profile,
            OperationalTuning::new(),
        )
        .expect("inquiry preparation")
        .admit_with(|request, _decoder, _timeout| request);

        for request in [plain, inquiry] {
            let mut owner = BlockingOwner::new(policy(1, TransportKind::Datagram)).unwrap();
            let mut driver = FakeDriver::default();
            let receipt = owner.submit(&mut driver, request).unwrap();
            assert_eq!(driver.writes.len(), 1);
            drop(receipt);
        }
    }

    #[test]
    fn unsupported_sent_cancellation_emits_no_cancel_write() {
        let mut owner_policy = policy(2, TransportKind::Datagram);
        owner_policy.targets[usize::from(CameraId::CAMERA_1.id())] = Some(TargetPolicy {
            command_sockets: 2,
            cancellation: CancellationPolicy::Unsupported,
        });
        let mut owner = BlockingOwner::new(owner_policy).unwrap();
        let mut driver = FakeDriver::default();
        let operation = owner
            .submit(
                &mut driver,
                command(CameraId::CAMERA_1, CancellationPolicy::Unsupported, None),
            )
            .unwrap();
        let id = operation.id();
        let before = owner.state().request_state(id).unwrap();
        let error = owner.cancel_test(&mut driver, operation).unwrap_err();
        assert!(matches!(error, Error::NotSupported));
        assert_eq!(owner.state().request_state(id), Some(before));
        assert_eq!(owner.state().active_len(), 1);
        assert_eq!(
            driver
                .writes
                .iter()
                .filter(|(_, _, cancellation)| *cancellation)
                .count(),
            0
        );
        owner
            .inject_frame(
                &mut driver,
                frame(
                    CameraId::CAMERA_1,
                    DecodedResponse::Ack {
                        socket: Some(ViscaSocket::S1),
                    },
                ),
                Instant::now(),
            )
            .unwrap();
        assert_eq!(owner.state().active_len(), 1);
        owner
            .inject_frame(
                &mut driver,
                frame(
                    CameraId::CAMERA_1,
                    DecodedResponse::Completion {
                        socket: Some(ViscaSocket::S1),
                    },
                ),
                Instant::now(),
            )
            .unwrap();
        assert_eq!(owner.state().active_len(), 0);
    }

    #[test]
    fn datagram_cancel_write_failure_resolves_token_and_retains_late_routing() {
        let mut owner = BlockingOwner::new(policy(2, TransportKind::Datagram)).unwrap();
        let mut driver = FakeDriver::default();
        let operation = owner
            .submit(
                &mut driver,
                command(CameraId::CAMERA_1, CancellationPolicy::Supported, None),
            )
            .unwrap();
        owner
            .inject_frame(
                &mut driver,
                frame(
                    CameraId::CAMERA_1,
                    DecodedResponse::Ack {
                        socket: Some(ViscaSocket::S1),
                    },
                ),
                Instant::now(),
            )
            .unwrap();
        driver
            .results
            .push_back(Err(Error::TransportError("cancel write failed".into())));

        let cancellation = owner.cancel_test(&mut driver, operation).unwrap();
        match cancellation.recv_test().unwrap() {
            CancellationObservation::Failed(Error::TransportError(reason)) => {
                assert_eq!(reason, "cancel write failed");
            }
            other => panic!("unexpected cancellation observation: {other:?}"),
        }
        assert_eq!(owner.state().state(), SessionState::Running);
        assert_eq!(owner.state().active_len(), 1);

        owner
            .inject_frame(
                &mut driver,
                frame(
                    CameraId::CAMERA_1,
                    DecodedResponse::Completion {
                        socket: Some(ViscaSocket::S1),
                    },
                ),
                Instant::now(),
            )
            .unwrap();
        assert_eq!(owner.state().active_len(), 0);
    }

    #[test]
    fn stream_cancel_write_failure_poisons_token_session_and_peers() {
        let mut owner = BlockingOwner::new(policy(2, TransportKind::Stream)).unwrap();
        let mut driver = FakeDriver::default();
        let operation = owner
            .submit(
                &mut driver,
                command(CameraId::CAMERA_1, CancellationPolicy::Supported, None),
            )
            .unwrap();
        owner
            .inject_frame(
                &mut driver,
                frame(
                    CameraId::CAMERA_1,
                    DecodedResponse::Ack {
                        socket: Some(ViscaSocket::S1),
                    },
                ),
                Instant::now(),
            )
            .unwrap();
        let peer = owner
            .submit(
                &mut driver,
                command(CameraId::CAMERA_2, CancellationPolicy::Supported, None),
            )
            .unwrap();
        driver.results.push_back(Err(Error::Timeout));

        let cancellation = owner.cancel_test(&mut driver, operation).unwrap();
        assert!(matches!(
            cancellation.recv_test().unwrap(),
            CancellationObservation::Failed(Error::StreamPoisoned { .. })
        ));
        assert!(matches!(
            peer.terminal(),
            Some(RuntimeOutcome::Failed(Error::StreamPoisoned { .. }))
        ));
        assert_eq!(owner.state().state(), SessionState::Poisoned);
        assert_eq!(owner.state().active_len(), 0);
    }

    #[test]
    fn cancellation_receipt_retains_terminal_observation_after_recorded() {
        let mut owner = BlockingOwner::new(policy(2, TransportKind::Datagram)).unwrap();
        let mut driver = FakeDriver::default();
        let operation = owner
            .submit(
                &mut driver,
                command(CameraId::CAMERA_1, CancellationPolicy::Supported, None),
            )
            .unwrap();
        owner
            .inject_frame(
                &mut driver,
                frame(
                    CameraId::CAMERA_1,
                    DecodedResponse::Ack {
                        socket: Some(ViscaSocket::S1),
                    },
                ),
                Instant::now(),
            )
            .unwrap();
        let cancellation = owner.cancel_test(&mut driver, operation).unwrap();
        assert_eq!(
            driver
                .writes
                .iter()
                .filter(|(_, _, cancellation)| *cancellation)
                .count(),
            1
        );
        owner
            .inject_frame(
                &mut driver,
                frame(
                    CameraId::CAMERA_1,
                    DecodedResponse::Error {
                        socket: Some(ViscaSocket::S1),
                        code: 0x04,
                    },
                ),
                Instant::now(),
            )
            .unwrap();
        assert!(matches!(
            cancellation.recv_test().unwrap(),
            CancellationObservation::Cancelled
        ));
        assert_eq!(owner.state().active_len(), 0);
    }

    #[test]
    fn already_buffered_completion_wins_cancellation_race() {
        let mut owner = BlockingOwner::new(policy(1, TransportKind::Datagram)).unwrap();
        let mut driver = FakeDriver::default();
        let operation = owner
            .submit(
                &mut driver,
                command(CameraId::CAMERA_1, CancellationPolicy::Supported, None),
            )
            .unwrap();
        let now = Instant::now();
        owner
            .inject_frame(
                &mut driver,
                frame(
                    CameraId::CAMERA_1,
                    DecodedResponse::Ack {
                        socket: Some(ViscaSocket::S1),
                    },
                ),
                now,
            )
            .unwrap();
        owner
            .inject_frame(
                &mut driver,
                frame(
                    CameraId::CAMERA_1,
                    DecodedResponse::Completion {
                        socket: Some(ViscaSocket::S1),
                    },
                ),
                now,
            )
            .unwrap();
        let cancellation = owner.cancel_test(&mut driver, operation).unwrap();
        assert!(matches!(
            cancellation.recv_test().unwrap(),
            CancellationObservation::Completed
        ));
    }

    #[test]
    fn blocking_receive_uses_earliest_observer_or_engine_deadline() {
        let mut owner = BlockingOwner::new(policy(1, TransportKind::Datagram)).unwrap();
        let mut driver = FakeDriver::default();
        let receipt = owner
            .submit(
                &mut driver,
                command(CameraId::CAMERA_1, CancellationPolicy::Supported, None),
            )
            .unwrap();
        let observer_deadline = Instant::now() + Duration::from_millis(2);
        let mut reader = DeadlineReader::default();
        let mut decoder = EmptyDecoder;
        assert_eq!(
            owner
                .pump_once_until(
                    &mut driver,
                    &mut reader,
                    &mut decoder,
                    Some(observer_deadline),
                )
                .unwrap(),
            0
        );
        assert!(reader
            .deadline
            .is_some_and(|deadline| deadline <= observer_deadline));
        assert_eq!(owner.state().active_len(), 1);
        drop(receipt);
    }

    #[test]
    fn diagnostics_have_bounded_stream_and_blocking_drain() {
        let mut owner_policy = policy(1, TransportKind::Datagram);
        owner_policy.limits.diagnostics = 4;
        owner_policy.limits.diagnostic_events_per_subscription = 1;
        let mut owner = BlockingOwner::new(owner_policy).unwrap();
        let subscription = owner.state_mut().subscribe_diagnostics(1).unwrap();
        let mut driver = FakeDriver::default();
        let receipt = owner
            .submit(
                &mut driver,
                command(CameraId::CAMERA_1, CancellationPolicy::Supported, None),
            )
            .unwrap();
        assert!(matches!(
            subscription.try_recv(),
            Some(DiagnosticEvent::Admitted {
                target: CameraId::CAMERA_1,
                ..
            })
        ));
        assert!(owner.state().metrics().dropped_diagnostic_events > 0);
        let drained = owner.drain_diagnostics();
        assert!(!drained.is_empty());
        assert_eq!(owner.state().diagnostics().count(), 0);
        drop(receipt);
    }

    #[test]
    fn warmed_raw_and_sony_retries_reuse_owner_buffers() {
        for envelope in [EnvelopeKind::Raw, EnvelopeKind::Sony] {
            let mut owner_policy = policy(1, TransportKind::Datagram);
            owner_policy.protocol.envelope = envelope;
            let mut owner = BlockingOwner::new(owner_policy).unwrap();
            let mut request = command(CameraId::CAMERA_1, CancellationPolicy::Supported, None);
            if let RuntimeRequest::Command { context, .. } = &mut request {
                context.retry = RetryPolicy {
                    max_retries: 1,
                    initial_backoff: Duration::ZERO,
                    maximum_backoff: Duration::ZERO,
                    total_budget: Duration::from_secs(1),
                    ack_timeout: false,
                    completion_timeout: false,
                    inquiry_timeout: false,
                    buffer_full: true,
                    movement_not_executable: false,
                    builtin_inquiry_syntax: false,
                };
            }
            let mut driver = FramingDriver::new(envelope);
            let receipt = owner.submit(&mut driver, request).unwrap();
            let sequence = match envelope {
                EnvelopeKind::Raw => None,
                EnvelopeKind::Sony => Some(EnvelopeSequence {
                    value: 0,
                    width: SequenceWidth::Full32,
                }),
            };
            owner
                .inject_frame(
                    &mut driver,
                    DecodedFrame {
                        target: CameraId::CAMERA_1,
                        sequence,
                        response: DecodedResponse::Error {
                            socket: None,
                            code: 0x03,
                        },
                    },
                    Instant::now(),
                )
                .unwrap();
            assert_eq!(driver.frames.len(), 2, "{envelope:?} retry write");
            assert_eq!(driver.raw_pointers[0], driver.raw_pointers[1]);
            assert_eq!(driver.frame_pointers[0], driver.frame_pointers[1]);
            assert_eq!(driver.frame_capacities[0], driver.frame_capacities[1]);
            drop(receipt);
        }
    }

    #[test]
    fn deadline_detach_diagnostics_and_buffers_remain_bounded() {
        let mut owner_policy = policy(2, TransportKind::Datagram);
        owner_policy.limits.diagnostics = 3;
        owner_policy.limits.state_keys_per_target = 2;
        let mut owner = BlockingOwner::new(owner_policy).unwrap();
        let send_ptr = owner.state_mut().buffers().send.as_ptr();
        let receive_ptr = owner.state_mut().buffers().receive_mut().as_ptr();
        let mut driver = FakeDriver::default();
        let receipt = owner
            .submit(
                &mut driver,
                command(CameraId::CAMERA_1, CancellationPolicy::Supported, None),
            )
            .unwrap();
        drop(receipt);
        owner
            .wake(&mut driver, Instant::now() + Duration::from_millis(11))
            .unwrap();
        assert!(owner.state().diagnostics().count() <= 3);
        assert!(owner.state().metrics().dropped_diagnostics > 0);
        assert_eq!(owner.state_mut().buffers().send.as_ptr(), send_ptr);
        assert_eq!(
            owner.state_mut().buffers().receive_mut().as_ptr(),
            receive_ptr
        );
        assert_eq!(owner.state().active_len(), 0);
    }

    #[test]
    fn write_failure_and_shutdown_resolve_and_release_once() {
        let mut owner = BlockingOwner::new(policy(2, TransportKind::Datagram)).unwrap();
        let mut driver = FakeDriver {
            results: VecDeque::from([Err(Error::Timeout)]),
            ..FakeDriver::default()
        };
        let error = owner
            .submit(
                &mut driver,
                command(CameraId::CAMERA_1, CancellationPolicy::Supported, None),
            )
            .unwrap_err();
        assert!(matches!(error, Error::Timeout));
        assert_eq!(owner.state().active_len(), 0);
        assert_eq!(owner.state().permits().available(), 2);

        let mut stream_owner = BlockingOwner::new(policy(1, TransportKind::Stream)).unwrap();
        let mut stream_driver = FakeDriver {
            results: VecDeque::from([Err(Error::Timeout)]),
            ..FakeDriver::default()
        };
        let stream_error = stream_owner
            .submit(
                &mut stream_driver,
                command(CameraId::CAMERA_1, CancellationPolicy::Supported, None),
            )
            .unwrap_err();
        assert!(matches!(stream_error, Error::StreamPoisoned { .. }));
        assert_eq!(stream_owner.state().state(), SessionState::Poisoned);
        assert_eq!(stream_owner.state().active_len(), 0);
        assert_eq!(stream_owner.state().permits().available(), 1);

        let receipt = owner
            .submit(
                &mut driver,
                command(CameraId::CAMERA_1, CancellationPolicy::Supported, None),
            )
            .unwrap();
        owner.shutdown(&mut driver).unwrap();
        assert!(matches!(
            receipt.terminal(),
            Some(RuntimeOutcome::Failed(Error::RuntimeShutdown))
        ));
        assert_eq!(owner.state().active_len(), 0);
        assert_eq!(owner.state().permits().available(), 2);
    }

    #[test]
    fn blocking_out_of_order_fixture_replays_through_production_owner() {
        let fixture =
            include_str!("../../../tests/fixtures/issue_542/lifecycle/blocking_out_of_order.trace");
        let mut owner = BlockingOwner::new(policy(8, TransportKind::Datagram)).unwrap();
        let mut driver = FakeDriver::default();
        let mut receipts = BTreeMap::<String, ReceiptCore>::new();
        let mut observed = Vec::new();
        let base = Instant::now();

        for line in fixture.lines() {
            let fields: Vec<_> = line.split_ascii_whitespace().collect();
            if fields.get(1) != Some(&"input") {
                continue;
            }
            let at = fields[0].parse::<u64>().unwrap();
            match fields[2] {
                "session" => {}
                "blocking-submit" => {
                    let label = fields[3].to_owned();
                    let target = trace_field(&fields, "target").parse::<u8>().unwrap();
                    let target = CameraId::new(target).unwrap();
                    let mut request = command(target, CancellationPolicy::Supported, None);
                    if let RuntimeRequest::Command { context, .. } = &mut request {
                        context.timeout.ack = Duration::from_secs(1);
                        context.timeout.completion = Duration::from_secs(1);
                    }
                    let receipt = owner.submit(&mut driver, request).unwrap();
                    assert!(receipts.insert(label, receipt).is_none());
                }
                "frame" => {
                    let target = CameraId::CAMERA_1;
                    let response = match fields[3] {
                        "ack" => DecodedResponse::Ack {
                            socket: Some(trace_socket(&fields)),
                        },
                        "complete" => DecodedResponse::Completion {
                            socket: Some(trace_completion_socket(&fields)),
                        },
                        "error" => DecodedResponse::Error {
                            socket: Some(trace_socket(&fields)),
                            code: u8::from_str_radix(trace_field(&fields, "code"), 16).unwrap(),
                        },
                        other => panic!("unsupported fixture frame {other}"),
                    };
                    owner
                        .inject_frame(
                            &mut driver,
                            frame(target, response),
                            base + Duration::from_millis(at),
                        )
                        .unwrap();
                }
                "blocking-wait" => {
                    let receipt = receipts.remove(fields[3]).unwrap();
                    observed.push(receipt.terminal().unwrap());
                }
                other => panic!("unsupported blocking fixture input {other}"),
            }
        }

        assert_eq!(observed.len(), 4);
        assert!(matches!(observed[0], RuntimeOutcome::Applied));
        assert!(matches!(observed[1], RuntimeOutcome::Applied));
        assert!(matches!(observed[2], RuntimeOutcome::Applied));
        assert!(
            matches!(observed[3], RuntimeOutcome::Failed(Error::SyntaxError)),
            "unexpected fixture outcomes: {observed:?}"
        );
        assert!(receipts.is_empty());
    }

    fn trace_field<'a>(fields: &[&'a str], name: &str) -> &'a str {
        fields
            .iter()
            .find_map(|field| field.split_once('=').filter(|(key, _)| *key == name))
            .map(|(_, value)| value)
            .unwrap_or_else(|| panic!("fixture field {name}"))
    }

    fn trace_socket(fields: &[&str]) -> ViscaSocket {
        match trace_field(fields, "socket") {
            "1" => ViscaSocket::S1,
            "2" => ViscaSocket::S2,
            other => panic!("fixture socket {other}"),
        }
    }

    fn trace_completion_socket(fields: &[&str]) -> ViscaSocket {
        let bytes = trace_field(fields, "bytes");
        let response = u8::from_str_radix(&bytes[2..4], 16).unwrap();
        match response & 0x0f {
            1 => ViscaSocket::S1,
            2 => ViscaSocket::S2,
            other => panic!("fixture completion socket {other}"),
        }
    }

    #[test]
    fn sequenced_frame_type_stays_decoder_owned() {
        let decoded = DecodedFrame {
            target: CameraId::CAMERA_1,
            sequence: Some(EnvelopeSequence {
                value: 7,
                width: SequenceWidth::Full32,
            }),
            response: DecodedResponse::Unknown,
        };
        assert_eq!(decoded.sequence.unwrap().value, 7);
    }
}

/// Issue #571: the counters a field debugging session reaches for first.
///
/// Every scenario here is scripted at the owner boundary rather than asserted
/// against a retry constant, so the counts stay meaningful as retry policy
/// changes underneath them.
mod metrics {
    use std::{
        collections::VecDeque,
        sync::Arc,
        time::{Duration, Instant},
    };

    use super::super::*;
    use crate::{
        runtime::engine::{
            CancellationPolicy, ControlPolicy, DeadlineKind, DecodedFrame, DecodedResponse,
            EncodedMessage, EnvelopeKind, EnvelopeSequence, InquiryRoute, RequestContext,
            RetryPolicy, SequenceWidth, TimeoutPolicy, TransmissionMeta, TransportKind,
        },
        CameraId, Error, ViscaSocket,
    };

    const ACK: Duration = Duration::from_millis(10);
    const COMPLETION: Duration = Duration::from_millis(20);
    const INQUIRY: Duration = Duration::from_millis(15);

    fn retrying() -> RetryPolicy {
        RetryPolicy {
            max_retries: 3,
            initial_backoff: Duration::from_millis(5),
            maximum_backoff: Duration::from_millis(50),
            total_budget: Duration::from_secs(10),
            ack_timeout: true,
            completion_timeout: true,
            inquiry_timeout: true,
            buffer_full: true,
            movement_not_executable: true,
            builtin_inquiry_syntax: true,
        }
    }

    fn owner_policy(envelope: EnvelopeKind) -> OwnerPolicy {
        let protocol = ProtocolPolicy {
            capacity: 4,
            envelope,
            transport: TransportKind::Datagram,
            inquiry_capacity: 2,
            command_spacing: Duration::ZERO,
            inquiry_spacing: Duration::ZERO,
            inquiry_cooldown: Duration::ZERO,
        };
        OwnerPolicy::single_target(
            protocol,
            CameraId::CAMERA_1,
            TargetPolicy {
                command_sockets: 2,
                cancellation: CancellationPolicy::Supported,
            },
        )
        .unwrap()
    }

    fn request_context(retry: RetryPolicy) -> RequestContext {
        RequestContext {
            target: CameraId::CAMERA_1,
            timeout: TimeoutPolicy {
                ack: ACK,
                completion: COMPLETION,
                inquiry: INQUIRY,
                cancellation: Duration::from_millis(10),
                ambiguity: Duration::from_millis(10),
            },
            retry,
            control: ControlPolicy::default(),
            cancellation: CancellationPolicy::Supported,
        }
    }

    fn wire() -> Arc<EncodedMessage> {
        Arc::new(EncodedMessage::new(&[0x81, 0x01, 0x04, 0x00, 0xff]).unwrap())
    }

    fn command(retry: RetryPolicy) -> RuntimeRequest {
        RuntimeRequest::Command {
            wire: wire(),
            context: request_context(retry),
            applied_state: None,
        }
    }

    fn inquiry(retry: RetryPolicy) -> RuntimeRequest {
        RuntimeRequest::Inquiry {
            wire: wire(),
            context: request_context(retry),
            route: InquiryRoute(1),
        }
    }

    fn frame(sequence: Option<u32>, response: DecodedResponse) -> Input {
        OwnerState::frame_input(DecodedFrame {
            target: CameraId::CAMERA_1,
            sequence: sequence.map(|value| EnvelopeSequence {
                value,
                width: SequenceWidth::Full32,
            }),
            response,
        })
    }

    /// Applies one effect batch to quiescence, confirming every staged write.
    fn drain(
        state: &mut OwnerState,
        mut effects: VecDeque<Effect>,
        sequence: Option<u32>,
        now: Instant,
    ) {
        while let Some(effect) = effects.pop_front() {
            if let AppliedEffect::Transmit(staged) = state.apply_effect(effect) {
                let produced = state.finish_write(&staged, Ok(TransmissionMeta { sequence }), now);
                prepend_effects(&mut effects, produced);
            }
        }
    }

    /// Admits one request and leaves it waiting on the camera.
    fn sent(
        state: &mut OwnerState,
        request: RuntimeRequest,
        sequence: Option<u32>,
        now: Instant,
    ) -> CompletionObserver {
        let permit = state.permits().try_acquire().expect("admission permit");
        let (input, observer, _admission) = state.stage_admission(request, permit);
        let effects = state.input(input, now);
        drain(state, effects, sequence, now);
        observer
    }

    fn apply(state: &mut OwnerState, input: Input, now: Instant) {
        let effects = state.input(input, now);
        drain(state, effects, None, now);
    }

    fn advance(state: &mut OwnerState, now: Instant) {
        let effects = state.advance(now);
        drain(state, effects, None, now);
    }

    fn saw_deadline(state: &OwnerState, expected: DeadlineKind, retrying: bool) -> bool {
        state.diagnostics().any(|event| {
            matches!(
                event,
                DiagnosticEvent::DeadlineExpired { deadline, will_retry, .. }
                    if *deadline == expected && *will_retry == retrying
            )
        })
    }

    #[test]
    fn expired_ack_deadline_counts_a_timeout_and_the_retry_it_scheduled() {
        let start = Instant::now();
        let mut state = OwnerState::new(owner_policy(EnvelopeKind::Raw)).unwrap();
        let _observer = sent(&mut state, command(retrying()), None, start);
        assert_eq!(state.metrics().ack_timeouts, 0);
        advance(&mut state, start + ACK);
        let metrics = state.metrics();
        assert_eq!(metrics.ack_timeouts, 1);
        assert_eq!(metrics.retries_scheduled, 1);
        assert_eq!(metrics.completion_timeouts, 0);
        assert_eq!(metrics.inquiry_timeouts, 0);
        assert!(saw_deadline(&state, DeadlineKind::Ack, true));
    }

    #[test]
    fn expired_ack_deadline_without_a_retry_reports_the_decision_directly() {
        let start = Instant::now();
        let mut state = OwnerState::new(owner_policy(EnvelopeKind::Raw)).unwrap();
        let _observer = sent(&mut state, command(RetryPolicy::NEVER), None, start);
        advance(&mut state, start + ACK);
        let metrics = state.metrics();
        assert_eq!(metrics.ack_timeouts, 1);
        assert_eq!(metrics.retries_scheduled, 0);
        assert!(saw_deadline(&state, DeadlineKind::Ack, false));
    }

    #[test]
    fn expired_completion_deadline_counts_separately_from_the_ack_deadline() {
        let start = Instant::now();
        let mut state = OwnerState::new(owner_policy(EnvelopeKind::Raw)).unwrap();
        let _observer = sent(&mut state, command(retrying()), None, start);
        apply(
            &mut state,
            frame(
                None,
                DecodedResponse::Ack {
                    socket: Some(ViscaSocket::S1),
                },
            ),
            start,
        );
        advance(&mut state, start + COMPLETION);
        let metrics = state.metrics();
        assert_eq!(metrics.completion_timeouts, 1);
        assert_eq!(metrics.ack_timeouts, 0);
        assert!(saw_deadline(&state, DeadlineKind::Completion, true));
    }

    #[test]
    fn expired_inquiry_deadline_counts_as_an_inquiry_timeout() {
        let start = Instant::now();
        let mut state = OwnerState::new(owner_policy(EnvelopeKind::Raw)).unwrap();
        let _observer = sent(&mut state, inquiry(retrying()), None, start);
        advance(&mut state, start + INQUIRY);
        let metrics = state.metrics();
        assert_eq!(metrics.inquiry_timeouts, 1);
        assert_eq!(metrics.ack_timeouts, 0);
        assert_eq!(metrics.completion_timeouts, 0);
        assert!(saw_deadline(&state, DeadlineKind::InquiryReply, true));
    }

    #[test]
    fn camera_backpressure_codes_count_as_busy_errors() {
        for code in [0x03_u8, 0x05, 0x41] {
            let start = Instant::now();
            let mut state = OwnerState::new(owner_policy(EnvelopeKind::Raw)).unwrap();
            let _observer = sent(&mut state, command(retrying()), None, start);
            apply(
                &mut state,
                frame(
                    None,
                    DecodedResponse::Error {
                        socket: Some(ViscaSocket::S1),
                        code,
                    },
                ),
                start,
            );
            let metrics = state.metrics();
            assert_eq!(metrics.busy_errors, 1, "code {code:#04x}");
            assert_eq!(metrics.protocol_errors, 0, "code {code:#04x}");
            assert_eq!(metrics.retries_scheduled, 1, "code {code:#04x}");
        }
    }

    #[test]
    fn other_error_codes_count_as_protocol_errors() {
        let start = Instant::now();
        let mut state = OwnerState::new(owner_policy(EnvelopeKind::Raw)).unwrap();
        let _observer = sent(&mut state, command(retrying()), None, start);
        apply(
            &mut state,
            frame(
                None,
                DecodedResponse::Error {
                    socket: Some(ViscaSocket::S1),
                    code: 0x02,
                },
            ),
            start,
        );
        let metrics = state.metrics();
        assert_eq!(metrics.protocol_errors, 1);
        assert_eq!(metrics.busy_errors, 0);
        assert_eq!(metrics.terminal, 1);
    }

    /// `0x04` answers a cancel; counting it would make every successful
    /// cancellation look like a camera fault.
    #[test]
    fn the_cancellation_reply_code_is_neither_busy_nor_protocol() {
        let start = Instant::now();
        let mut state = OwnerState::new(owner_policy(EnvelopeKind::Raw)).unwrap();
        let _observer = sent(&mut state, command(retrying()), None, start);
        apply(
            &mut state,
            frame(
                None,
                DecodedResponse::Error {
                    socket: Some(ViscaSocket::S1),
                    code: 0x04,
                },
            ),
            start,
        );
        let metrics = state.metrics();
        assert_eq!(metrics.busy_errors, 0);
        assert_eq!(metrics.protocol_errors, 0);
    }

    #[test]
    fn a_sequenced_reply_matching_no_request_is_counted_as_ignored() {
        let start = Instant::now();
        let mut state = OwnerState::new(owner_policy(EnvelopeKind::Sony)).unwrap();
        let _observer = sent(&mut state, command(retrying()), Some(1), start);
        apply(
            &mut state,
            frame(
                Some(9_999),
                DecodedResponse::Completion {
                    socket: Some(ViscaSocket::S1),
                },
            ),
            start,
        );
        assert_eq!(state.metrics().ignored_unmatched_sequenced_replies, 1);
        assert_eq!(state.metrics().terminal, 0);
    }

    /// A transient receive fault (#565) retries in-flight work, and a retry is a
    /// retry whatever provoked it.
    #[test]
    fn a_transient_receive_fault_retry_counts_as_a_retry() {
        let start = Instant::now();
        let mut state = OwnerState::new(owner_policy(EnvelopeKind::Raw)).unwrap();
        let _observer = sent(&mut state, command(retrying()), None, start);
        apply(
            &mut state,
            Input::ReceiveFault {
                error: Error::Io(Arc::new(std::io::Error::from(
                    std::io::ErrorKind::ConnectionRefused,
                ))),
            },
            start,
        );
        let metrics = state.metrics();
        assert_eq!(metrics.retries_scheduled, 1);
        assert_eq!(metrics.ack_timeouts, 0);
    }

    /// The counters are scalars and the diagnostic vocabulary stays `Copy`, so
    /// an increment is a register add and observing a snapshot never allocates.
    #[test]
    fn owner_counters_stay_scalar_and_copyable() {
        const fn assert_copy<T: Copy>() {}
        assert_copy::<OwnerMetrics>();
        assert_copy::<DiagnosticEvent>();
        assert_eq!(size_of::<OwnerMetrics>(), 19 * size_of::<u64>());
    }
}
