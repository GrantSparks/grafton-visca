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
        strict_unconfirmed_poison: false,
    }
}

fn target_policy_for_validation() -> TargetPolicy {
    TargetPolicy {
        command_sockets: 1,
        cancellation: CancellationPolicy::Supported,
    }
}

#[cfg(all(feature = "blocking", not(feature = "async")))]
fn cached_projection(
    owner: &OwnerState,
    target: CameraId,
    state: WriteOnlyState,
) -> Option<AppliedStateProjection> {
    owner
        .target_cache
        .get(usize::from(target.id()))
        .and_then(|slot| slot.lock().ok().and_then(|cache| cache.get(state)))
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

/// Issue #631: the owner state used by the reconfiguration tests below.
///
/// The profile baseline is deliberately non-zero on both axes so a
/// re-derivation that quietly kept a previous override would be visible.
fn reconfigurable_owner_state() -> OwnerState {
    let mut protocol = policy_for_target_validation();
    protocol.command_spacing = Duration::from_millis(10);
    protocol.inquiry_spacing = Duration::from_millis(20);
    let policy = OwnerPolicy::single_target(
        protocol,
        CameraId::CAMERA_1,
        TargetPolicy {
            command_sockets: 2,
            cancellation: CancellationPolicy::Supported,
        },
    )
    .expect("single-target owner policy");
    OwnerState::new(policy).expect("owner state")
}

fn command_sockets(state: &OwnerState) -> u8 {
    state.policy().targets[usize::from(CameraId::CAMERA_1.id())]
        .expect("camera one is registered")
        .command_sockets
}

/// Issue #631: pacing and socket capacity are re-derived from the *profile*
/// baseline, so an override can be relaxed again instead of becoming the new
/// floor.
#[test]
fn retuning_re_derives_pacing_from_the_profile_baseline() {
    let mut state = reconfigurable_owner_state();
    assert_eq!(
        state.policy().protocol.command_spacing,
        Duration::from_millis(10)
    );
    assert_eq!(
        state.policy().protocol.inquiry_spacing,
        Duration::from_millis(20)
    );
    assert_eq!(command_sockets(&state), 2);

    state
        .retune(
            crate::OperationalTuning::new()
                .command_spacing(Duration::from_millis(200))
                .inquiry_spacing(Duration::from_millis(300))
                .maximum_command_sockets(1),
        )
        .expect("widening pacing and lowering socket capacity is accepted");
    assert_eq!(
        state.policy().protocol.command_spacing,
        Duration::from_millis(200)
    );
    assert_eq!(
        state.policy().protocol.inquiry_spacing,
        Duration::from_millis(300)
    );
    assert_eq!(command_sockets(&state), 1);

    // Clearing every override must return to the profile facts. Deriving the
    // new value from the *tuned* one instead would leave 200 ms installed
    // forever.
    state
        .retune(crate::OperationalTuning::new())
        .expect("clearing the overrides is accepted");
    assert_eq!(
        state.policy().protocol.command_spacing,
        Duration::from_millis(10)
    );
    assert_eq!(
        state.policy().protocol.inquiry_spacing,
        Duration::from_millis(20)
    );
    assert_eq!(command_sockets(&state), 2);
}

/// Issue #631: the live tuning cell handed to request-preparing handles follows
/// the owner, and a rejected update leaves both the cell and the policy alone.
#[test]
fn a_rejected_retune_changes_neither_the_live_tuning_nor_the_policy() {
    let mut state = reconfigurable_owner_state();
    let live = state.live_tuning();
    assert_eq!(live.get(), crate::OperationalTuning::new());

    let accepted = crate::OperationalTuning::new().command_spacing(Duration::from_millis(50));
    state.retune(accepted).expect("accepted");
    assert_eq!(live.get(), accepted, "the shared cell follows the owner");

    // The facades reject this before it reaches the owner; the engine's own
    // socket bound is the second line of defence, and it must fail cleanly.
    let error = state
        .retune(crate::OperationalTuning::new().maximum_command_sockets(3))
        .expect_err("three command sockets is not a VISCA capacity");
    assert!(matches!(error, Error::InvalidRequest(_)), "got {error:?}");
    assert_eq!(
        live.get(),
        accepted,
        "a rejected update leaves the live tuning untouched"
    );
    assert_eq!(
        state.policy().protocol.command_spacing,
        Duration::from_millis(50),
        "a rejected update leaves the owner policy untouched"
    );
    assert_eq!(command_sockets(&state), 2);
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
        protocol::response::{decode_basic, BasicKind},
        transport::{builder::AddressingMode, Envelope, FrameSequence, RawVisca, SonyEncapsulated},
        CameraId, Error, ViscaSocket,
    };

    use super::super::*;
    use super::cached_projection;
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
            strict_unconfirmed_poison: false,
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
                inquiry: Duration::from_millis(20),
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

    /// Build the same low-level policy as [`policy`], but with Sony's
    /// sequence-bearing envelope.  Tests that intentionally exercise
    /// pre-ACK concurrency or safe retry must use this envelope: raw VISCA
    /// cannot identify two same-target commands until one ACK assigns a
    /// socket.
    fn sony_policy(capacity: usize, transport: TransportKind) -> OwnerPolicy {
        let mut owner = policy(capacity, transport);
        owner.protocol.envelope = EnvelopeKind::Sony;
        owner
    }

    /// Seed the fake driver's transmission metadata with the sequence values
    /// that a Sony envelope would have returned.  The fake driver does not
    /// frame bytes itself, so the test supplies the envelope metadata at the
    /// driver boundary explicitly.
    fn sony_driver(sequences: impl IntoIterator<Item = u32>) -> FakeDriver {
        FakeDriver {
            writes: Vec::new(),
            results: sequences
                .into_iter()
                .map(|sequence| {
                    Ok(TransmissionMeta {
                        sequence: Some(sequence),
                    })
                })
                .collect(),
        }
    }

    fn sony_frame(target: CameraId, sequence: u32, response: DecodedResponse) -> DecodedFrame {
        DecodedFrame {
            target,
            sequence: Some(EnvelopeSequence {
                value: sequence,
                width: SequenceWidth::Full32,
            }),
            response,
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
        /// The envelope sequence this driver stamped on each write, in write
        /// order, exactly as it was reported back to the owner.
        sequences: Vec<Option<u32>>,
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
                sequences: Vec::new(),
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
                TestEnvelope::Raw(envelope) => envelope.frame_into_with_sequence(
                    write.bytes,
                    kind,
                    write.requested_sequence,
                    write.frame_buffer,
                )?,
                TestEnvelope::Sony(envelope) => envelope.frame_into_with_sequence(
                    write.bytes,
                    kind,
                    write.requested_sequence,
                    write.frame_buffer,
                )?,
            };
            self.frame_pointers
                .push(write.frame_buffer.as_ptr() as usize);
            self.frame_capacities.push(write.frame_buffer.capacity());
            self.frames.push(write.frame_buffer.to_vec());
            self.sequences.push(meta.sequence.map(FrameSequence::value));
            Ok(TransmissionMeta {
                sequence: meta.sequence.map(FrameSequence::value),
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
            crate::prepared::ClassSelection::Request,
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
            crate::prepared::ClassSelection::Request,
        )
        .unwrap()
    }

    /// Out-of-order peer-receipt retention on the Sony sequence-bearing
    /// envelope: multiple in-flight blocking waits each keep their own exact
    /// terminal outcome. The 1.x oracle recorded this on raw `PtzOpticsG2`; the
    /// v2 replay uses a Sony policy, so the name says so rather than implying
    /// the raw pairing still holds.
    #[test]
    fn sony_typed_blocking_wait_pumps_and_retains_out_of_order_peer_results() {
        let profile =
            crate::ProfileSpec::from_compile_time::<crate::profiles::SonyBRC300>().unwrap();
        let mut owner = BlockingOwner::new(sony_policy(8, TransportKind::Datagram)).unwrap();
        let mut driver = sony_driver(0..4);
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
                    sony_frame(
                        CameraId::CAMERA_1,
                        0,
                        DecodedResponse::Ack {
                            socket: Some(ViscaSocket::S1),
                        },
                    ),
                    sony_frame(
                        CameraId::CAMERA_1,
                        1,
                        DecodedResponse::Ack {
                            socket: Some(ViscaSocket::S2),
                        },
                    ),
                    sony_frame(
                        CameraId::CAMERA_2,
                        2,
                        DecodedResponse::Ack {
                            socket: Some(ViscaSocket::S1),
                        },
                    ),
                    sony_frame(
                        CameraId::CAMERA_2,
                        3,
                        DecodedResponse::Ack {
                            socket: Some(ViscaSocket::S2),
                        },
                    ),
                ],
                vec![sony_frame(
                    CameraId::CAMERA_1,
                    0,
                    DecodedResponse::Completion {
                        socket: Some(ViscaSocket::S1),
                    },
                )],
                vec![sony_frame(
                    CameraId::CAMERA_2,
                    3,
                    DecodedResponse::Error {
                        socket: Some(ViscaSocket::S2),
                        code: 0x02,
                    },
                )],
                vec![sony_frame(
                    CameraId::CAMERA_1,
                    1,
                    DecodedResponse::Completion {
                        socket: Some(ViscaSocket::S2),
                    },
                )],
                vec![sony_frame(
                    CameraId::CAMERA_2,
                    2,
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
        let cancellation = operation.cancel_test(&mut owner, &mut driver).unwrap();
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
        let mut owner = BlockingOwner::new(sony_policy(2, TransportKind::Datagram)).unwrap();
        let mut driver = sony_driver(0..2);
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
        let mut owner = BlockingOwner::new(sony_policy(2, TransportKind::Datagram)).unwrap();
        let mut driver = sony_driver(0..2);
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
                sony_frame(
                    CameraId::CAMERA_1,
                    0,
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
                sony_frame(
                    CameraId::CAMERA_1,
                    1,
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
                sony_frame(
                    CameraId::CAMERA_1,
                    1,
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
                sony_frame(
                    CameraId::CAMERA_1,
                    0,
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
        let _peer = owner
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
        assert!(cached_projection(
            owner.state(),
            CameraId::CAMERA_1,
            WriteOnlyState::PanTiltLimits,
        )
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

    /// Issue #565: on the Sony sequence-bearing envelope a transient read
    /// failure retries the in-flight command and leaves the session running;
    /// the pump reports "no frames", not an error. The raw envelope has no
    /// request identity after a successful write, so the same fault poisons
    /// there (see the raw fault targets); this case is Sony-only by name.
    #[test]
    fn sony_transient_blocking_read_fault_retries_and_keeps_the_session() {
        let profile =
            crate::ProfileSpec::from_compile_time::<crate::profiles::SonyBRC300>().unwrap();
        let mut owner = BlockingOwner::new(sony_policy(1, TransportKind::Datagram)).unwrap();
        let mut driver = sony_driver([0, 0]);
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
        let mut owner = BlockingOwner::new(sony_policy(8, TransportKind::Datagram)).unwrap();
        let mut driver = sony_driver(0..2);
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
                sony_frame(
                    CameraId::CAMERA_1,
                    0,
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
                sony_frame(
                    CameraId::CAMERA_1,
                    1,
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
                sony_frame(
                    CameraId::CAMERA_1,
                    1,
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
                sony_frame(
                    CameraId::CAMERA_1,
                    0,
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

    /// Issue #542: a public operation handle cannot name an unwritten request.
    /// The rejection must travel through the normal terminal effect so the
    /// owner drops the engine entry and queue ticket, resolves the temporary
    /// observer, and returns the shared admission permit.
    #[test]
    fn blocking_operation_rejection_terminalizes_only_the_new_request() {
        let mut owner_policy = policy(2, TransportKind::Datagram);
        owner_policy.targets[usize::from(CameraId::CAMERA_1.id())] = Some(TargetPolicy {
            command_sockets: 1,
            cancellation: CancellationPolicy::Supported,
        });
        let mut owner = BlockingOwner::new(owner_policy).unwrap();
        let profile =
            crate::ProfileSpec::from_compile_time::<crate::profiles::SonyBRC300>().unwrap();
        let mut driver = FakeDriver::default();
        let first = owner
            .submit_operation(&mut driver, prepared_zoom_drive(&profile))
            .unwrap();
        let first_operation_id = first.id();
        let first_id = owner.state().diagnostics().find_map(|event| match event {
            DiagnosticEvent::Admitted { id, .. } => Some(*id),
            _ => None,
        });
        let first_id = first_id.expect("first operation has one admission diagnostic");
        let first_state = owner.state().request_state(first_id);

        let error = owner
            .submit_operation(&mut driver, prepared_zoom_drive(&profile))
            .unwrap_err();
        assert!(matches!(error, Error::TransportBusy));
        assert_eq!(driver.writes.len(), 1);
        assert_eq!(owner.state().active_len(), 1);
        assert_eq!(owner.state().permits().available(), 1);
        assert_eq!(owner.state().metrics().admitted, 2);
        assert_eq!(owner.state().metrics().terminal, 1);
        assert_eq!(owner.state().request_state(first_id), first_state);

        let rejected_id = owner.state().diagnostics().find_map(|event| match event {
            DiagnosticEvent::Terminal {
                id,
                outcome: OutcomeDiagnostic::Failed(ErrorKind::Busy),
                ..
            } => Some(*id),
            _ => None,
        });
        let rejected_id = rejected_id.expect("rejected operation has one terminal diagnostic");
        assert_ne!(rejected_id, first_id);
        assert_eq!(first_operation_id, first_id.get());
        assert!(owner.state().request_state(rejected_id).is_none());
        assert!(owner.state().engine.entry(rejected_id).is_none());
        owner.state().engine.assert_invariants().unwrap();
        drop(first);
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
        let mut owner = BlockingOwner::new(sony_policy(8, TransportKind::Datagram)).unwrap();
        let mut driver = sony_driver(0..3);
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
                    sony_frame(
                        CameraId::CAMERA_1,
                        u32::from(socket.as_socket_number() - 1),
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
                sony_frame(
                    CameraId::CAMERA_1,
                    0,
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
                sony_frame(
                    CameraId::CAMERA_1,
                    1,
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
                sony_frame(
                    CameraId::CAMERA_1,
                    2,
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
                sony_frame(
                    CameraId::CAMERA_1,
                    2,
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
        assert_eq!(
            driver.writes.len(),
            1,
            "raw same-target B waits for A's ACK before it can be written"
        );
        assert_eq!(owner.state().request_state(a_id), Some(before));
        assert!(a.terminal().is_none());
        assert_eq!(owner.state().active_len(), 2);

        let ack_deadline = match before.0 {
            Phase::AwaitingAck { deadline, .. } => deadline,
            phase => panic!("request A was not awaiting an ACK: {phase:?}"),
        };
        owner.wake(&mut driver, ack_deadline).unwrap();
        // Issue #671: A's lost ACK quarantines it per-request rather than
        // poisoning the session. A's own deadline still fired at exactly
        // `ack_deadline` (B's paced submission never advanced it), moving A into
        // its late-ACK quarantine while the session stays live and B keeps
        // waiting behind A's still-reserved unacknowledged slot.
        assert!(a.terminal().is_none());
        assert!(matches!(
            owner.state().request_state(a_id).map(|state| state.0),
            Some(Phase::AwaitingLateAck { .. })
        ));
        assert_eq!(owner.state().state(), SessionState::Running);
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
            cached_projection(
                owner.state(),
                CameraId::CAMERA_1,
                WriteOnlyState::PanTiltLimits,
            ),
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
        let request = prepare_command(
            &set,
            CameraId::CAMERA_1,
            &profile,
            OperationalTuning::new(),
            crate::prepared::ClassSelection::Request,
        )
        .expect("pan/tilt limit preparation")
        .admit_with(|request, _timeout| request);
        let mut owner = BlockingOwner::new(policy(2, TransportKind::Datagram)).unwrap();
        let mut driver = FakeDriver::default();
        let receipt = owner.submit(&mut driver, request).unwrap();
        assert_eq!(
            cached_projection(
                owner.state(),
                CameraId::CAMERA_1,
                WriteOnlyState::PanTiltLimits,
            ),
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
            cached_projection(
                owner.state(),
                CameraId::CAMERA_1,
                WriteOnlyState::PanTiltLimits,
            ),
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
        let cached = cached_projection(
            owner.state(),
            CameraId::CAMERA_1,
            WriteOnlyState::PanTiltLimits,
        )
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
            crate::prepared::ClassSelection::Request,
        )
        .expect("pan/tilt limit clear preparation")
        .admit_with(|request, _timeout| request);
        let clear_receipt = owner.submit(&mut driver, clear).unwrap();
        assert_eq!(
            cached_projection(
                owner.state(),
                CameraId::CAMERA_1,
                WriteOnlyState::PanTiltLimits,
            ),
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
            cached_projection(
                owner.state(),
                CameraId::CAMERA_1,
                WriteOnlyState::PanTiltLimits,
            ),
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
            cached_projection(
                owner.state(),
                CameraId::CAMERA_1,
                WriteOnlyState::PanTiltLimits,
            ),
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
            crate::prepared::ClassSelection::Request,
        )
        .expect("plain preparation")
        .admit_with(|request, _timeout| request);
        let inquiry = prepare_inquiry(
            &PowerInquiry,
            CameraId::CAMERA_1,
            &profile,
            OperationalTuning::new(),
            crate::prepared::ClassSelection::Request,
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
        let rejected = owner.cancel_test(&mut driver, operation).unwrap_err();
        assert!(matches!(rejected.error, Error::NotSupported));
        // A refused cancellation returns the receipt so the caller keeps the
        // observer for the request the engine deliberately left running (#612).
        let operation = rejected
            .receipt
            .expect("a refused cancellation returns the operation receipt");
        assert_eq!(operation.id, id);
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
            if envelope == EnvelopeKind::Sony {
                assert_eq!(driver.sequences, vec![Some(0), Some(0)]);
                assert_eq!(driver.frames[0], driver.frames[1]);
            }
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
        let base = Instant::now();
        let receipt = owner
            .submit(
                &mut driver,
                command(CameraId::CAMERA_1, CancellationPolicy::Supported, None),
            )
            .unwrap();
        drop(receipt);
        // Issue #671: a raw command whose ACK is lost quarantines at its ACK
        // deadline (10ms) rather than poisoning immediately, then fails at the
        // ambiguity deadline (a further 10ms). Wake once past each so the
        // detached request drains; the quarantine window is measured from the
        // wake that processes the ACK deadline. Diagnostics and buffers stay
        // bounded across both deadline events.
        owner
            .wake(&mut driver, base + Duration::from_millis(20))
            .unwrap();
        owner
            .wake(&mut driver, base + Duration::from_millis(50))
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

        // A repeated explicit shutdown is an idempotent no-op: it must not
        // write again or resolve another terminal event.
        let writes_after_shutdown = driver.writes.len();
        let terminals_after_shutdown = owner.state().metrics().terminal;
        owner.shutdown(&mut driver).unwrap();
        assert_eq!(driver.writes.len(), writes_after_shutdown);
        assert_eq!(owner.state().metrics().terminal, terminals_after_shutdown);

        // A different fatal boundary must remain visible to a later explicit
        // shutdown instead of being mistaken for an idempotent repeat.
        let writes_after_poison = stream_driver.writes.len();
        let error = stream_owner
            .shutdown(&mut stream_driver)
            .expect_err("stream poison must not be hidden by shutdown");
        assert!(matches!(error, Error::StreamPoisoned { .. }));
        assert_eq!(stream_driver.writes.len(), writes_after_poison);
    }

    #[test]
    fn blocking_shutdown_preserves_reentrancy_error() {
        let mut owner = BlockingOwner::new(policy(1, TransportKind::Datagram)).unwrap();
        owner.mark_pumping_for_test();
        let mut driver = FakeDriver::default();

        let error = owner
            .shutdown(&mut driver)
            .expect_err("re-entrant shutdown must be rejected");
        assert!(matches!(error, Error::TransportBusy));
        assert!(driver.writes.is_empty());
    }

    /// Issue #634: the blocking facade is replayed against every expectation
    /// column the facade can observe.
    ///
    /// `BlockingOwner::submit` admits, dispatches and completes the initial
    /// write as one operation, so the fixture's intermediate `ready`/`sending`
    /// transitions are not facade-observable; the record-for-record replay of
    /// the same fixture at the owner-state seam lives in `lifecycle_trace`.
    /// Everything the facade *can* see is asserted here against the fixture's
    /// own columns: the exact wire bytes of every transmission, the engine
    /// identity behind every ticket, the socket every ACK claimed, terminal
    /// removal and permit release, retention of an unwaited outcome, and the
    /// exact terminal outcome each wait observes.
    #[test]
    fn blocking_out_of_order_fixture_replays_through_production_owner() {
        const FIXTURE: &str =
            include_str!("../../../tests/fixtures/issue_542/lifecycle/blocking_out_of_order.trace");

        let mut owner: Option<BlockingOwner> = None;
        let mut driver = FakeDriver::default();
        let mut receipts = BTreeMap::<String, ReceiptCore>::new();
        let mut identities = BTreeMap::<u64, RequestId>::new();
        let mut waited: Option<String> = None;
        let mut transmits = 0_usize;
        let mut records = 0_usize;
        let base = Instant::now();

        for line in FIXTURE.lines().map(str::trim) {
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            records += 1;
            let fields: Vec<_> = line.split_ascii_whitespace().collect();
            let at = fields[0].parse::<u64>().unwrap();
            match (fields[1], fields[2]) {
                ("input", "session") => {
                    let capacity = trace_field(&fields, "capacity").parse().unwrap();
                    owner = Some(
                        BlockingOwner::new(policy(capacity, TransportKind::Datagram)).unwrap(),
                    );
                }
                ("effect", "session") => {
                    let owner = owner.as_ref().unwrap();
                    assert_eq!(
                        owner.state().state(),
                        SessionState::Running,
                        "fixture session state: {line}"
                    );
                    assert_eq!(
                        owner.state().policy().protocol.transport,
                        TransportKind::Datagram
                    );
                    assert_eq!(
                        owner.state().permits().capacity().to_string(),
                        trace_field(&fields, "capacity"),
                        "fixture admission capacity: {line}"
                    );
                    assert_eq!(
                        owner.state().policy().targets[1].unwrap().cancellation,
                        CancellationPolicy::Supported
                    );
                }
                ("input", "blocking-submit") => {
                    let owner = owner.as_mut().unwrap();
                    let label = fields[3].to_owned();
                    let target =
                        CameraId::new(trace_field(&fields, "target").parse::<u8>().unwrap())
                            .unwrap();
                    let mut request = command(target, CancellationPolicy::Supported, None);
                    if let RuntimeRequest::Command { wire, context, .. } = &mut request {
                        // The fixture's own wire bytes, so the transmission
                        // expectation below compares against the real write.
                        *wire = Arc::new(
                            EncodedMessage::new(&trace_bytes(trace_field(&fields, "wire")))
                                .unwrap(),
                        );
                        context.timeout.ack = Duration::from_secs(60);
                        context.timeout.completion = Duration::from_secs(60);
                    }
                    let receipt = owner.submit(&mut driver, request).unwrap();
                    assert!(receipts.insert(label, receipt).is_none());
                }
                ("effect", "admitted") => {
                    let owner = owner.as_ref().unwrap();
                    let receipt = &receipts[trace_field(&fields, "ticket")];
                    let id: u64 = trace_field(&fields, "id").parse().unwrap();
                    identities.insert(id, receipt.id);
                    assert_eq!(receipt.id.get(), id, "fixture engine identity: {line}");
                    assert_eq!(
                        receipt.target,
                        CameraId::new(trace_field(&fields, "target").parse::<u8>().unwrap())
                            .unwrap(),
                        "fixture admission target: {line}"
                    );
                    assert_eq!(
                        owner.state().permits().available(),
                        owner.state().permits().capacity() - owner.state().active_len(),
                        "an admitted request holds exactly one permit: {line}"
                    );
                    assert_eq!(trace_field(&fields, "observer"), "blocking");
                }
                ("outcome", "admission") => {
                    let receipt = &receipts[trace_field(&fields, "ticket")];
                    assert_eq!(
                        receipt.id.get().to_string(),
                        trace_field(&fields, "id"),
                        "fixture admission reply: {line}"
                    );
                }
                ("effect", "state") => {
                    let owner = owner.as_ref().unwrap();
                    let id = identities[&trace_field(&fields, "id").parse().unwrap()];
                    let to = trace_field(&fields, "to");
                    let phase = owner.state().request_state(id).map(|state| state.0);
                    match to {
                        // Admission, dispatch and the initial write are one
                        // facade operation; only their settled result is
                        // observable here.
                        "sending" | "awaiting-ack" => assert!(
                            matches!(phase, Some(Phase::AwaitingAck { .. })),
                            "fixture phase {to}: {line} (actual {phase:?})"
                        ),
                        other => {
                            let socket = trace_socket_suffix(other);
                            assert!(
                                matches!(phase, Some(Phase::Executing { socket: owned, .. }) if owned == socket),
                                "fixture phase {other}: {line} (actual {phase:?})"
                            );
                        }
                    }
                }
                ("effect", "transmit") => {
                    let index: usize = trace_field(&fields, "tx").parse().unwrap();
                    transmits = transmits.max(index);
                    let id = identities[&trace_field(&fields, "id").parse().unwrap()];
                    let (written, bytes, cancellation) = &driver.writes[index - 1];
                    assert_eq!(*written, id, "fixture transmission owner: {line}");
                    assert_eq!(
                        bytes.as_slice(),
                        trace_bytes(trace_field(&fields, "wire")).as_slice(),
                        "fixture transmission bytes: {line}"
                    );
                    assert!(!cancellation, "fixture request transmission: {line}");
                    assert_eq!(trace_field(&fields, "kind"), "request");
                }
                ("driver-input", "transmission-finished") => {
                    let index: usize = trace_field(&fields, "tx").parse().unwrap();
                    assert_eq!(trace_field(&fields, "result"), "ok");
                    assert!(driver.writes.len() >= index, "fixture write result: {line}");
                }
                ("outcome", "blocking-handle") => {
                    let receipt = &receipts[trace_field(&fields, "ticket")];
                    assert_eq!(
                        receipt.id.get().to_string(),
                        trace_field(&fields, "id"),
                        "fixture blocking handle identity: {line}"
                    );
                    assert_eq!(
                        driver.writes.len(),
                        transmits,
                        "a blocking handle is returned only after its initial write: {line}"
                    );
                    assert_eq!(trace_field(&fields, "initial-write"), "complete");
                }
                ("input", "frame") => {
                    let owner = owner.as_mut().unwrap();
                    let bytes = trace_bytes(trace_field(&fields, "bytes"));
                    let decoded = decode_basic(&bytes).expect("fixture frame bytes decode");
                    let response = match (fields[3], decoded.kind) {
                        ("ack", BasicKind::Ack) => DecodedResponse::Ack {
                            socket: decoded.socket,
                        },
                        ("complete", BasicKind::Completion) => DecodedResponse::Completion {
                            socket: decoded.socket,
                        },
                        ("error", BasicKind::Error(code)) => DecodedResponse::Error {
                            socket: decoded.socket,
                            code,
                        },
                        other => panic!("fixture frame {other:?} disagrees with its own bytes"),
                    };
                    owner
                        .inject_frame(
                            &mut driver,
                            frame(decoded.source, response),
                            base + Duration::from_millis(at),
                        )
                        .unwrap();
                }
                ("effect", "frame-routed") => {
                    let owner = owner.as_ref().unwrap();
                    let id = identities[&trace_field(&fields, "id").parse().unwrap()];
                    let socket = trace_socket(&fields);
                    let phase = owner.state().request_state(id).map(|state| state.0);
                    assert!(
                        match phase {
                            Some(Phase::Executing { socket: owned, .. }) => owned == socket,
                            // A routed completion or error terminalizes the
                            // request that owned the frame's socket.
                            None => true,
                            _ => false,
                        },
                        "fixture frame routing: {line} (actual {phase:?})"
                    );
                    assert_eq!(trace_field(&fields, "via"), "target-socket");
                }
                ("effect", "terminal") => {
                    let owner = owner.as_ref().unwrap();
                    let id = identities[&trace_field(&fields, "id").parse().unwrap()];
                    assert!(
                        owner.state().request_state(id).is_none(),
                        "fixture terminal removes the engine entry: {line}"
                    );
                    assert_eq!(trace_field(&fields, "state"), "removed");
                    assert_eq!(trace_field(&fields, "permit"), "released");
                    assert_eq!(
                        owner.state().permits().available(),
                        owner.state().permits().capacity() - owner.state().active_len(),
                        "a terminal request releases its permit: {line}"
                    );
                }
                ("effect", "outcome-retained") => {
                    let id: u64 = trace_field(&fields, "id").parse().unwrap();
                    let receipt = receipts
                        .values()
                        .find(|receipt| receipt.id.get() == id)
                        .expect("an attached blocking receipt");
                    assert!(
                        !receipt.completion.receiver.is_empty(),
                        "a blocking receipt retains its outcome until it waits: {line}"
                    );
                    assert_eq!(trace_field(&fields, "observer"), "blocking");
                }
                ("input", "blocking-wait") => waited = Some(fields[3].to_owned()),
                ("outcome", "blocking-wait") => {
                    let label = waited.take().expect("a preceding blocking wait");
                    let receipt = receipts.remove(&label).unwrap();
                    assert_eq!(
                        receipt.id.get().to_string(),
                        trace_field(&fields, "id"),
                        "fixture wait identity: {line}"
                    );
                    let outcome = receipt.terminal().expect("a settled blocking receipt");
                    let expected = fields[4];
                    let matched = matches!(
                        (&outcome, expected),
                        (RuntimeOutcome::Applied, "Applied")
                            | (RuntimeOutcome::Cancelled, "Cancelled")
                            | (
                                RuntimeOutcome::Failed(Error::SyntaxError),
                                "Error::SyntaxError"
                            )
                    );
                    assert!(matched, "fixture wait outcome: {line} (actual {outcome:?})");
                }
                other => panic!("unsupported blocking fixture record {other:?}"),
            }
        }

        assert_eq!(records, 70, "every fixture record must be consumed");
        assert!(receipts.is_empty());
        assert_eq!(owner.as_ref().unwrap().state().active_len(), 0);
    }

    fn trace_field<'a>(fields: &[&'a str], name: &str) -> &'a str {
        fields
            .iter()
            .find_map(|field| field.split_once('=').filter(|(key, _)| *key == name))
            .map(|(_, value)| value)
            .unwrap_or_else(|| panic!("fixture field {name}"))
    }

    fn trace_bytes(text: &str) -> Vec<u8> {
        assert!(text.len().is_multiple_of(2), "fixture byte string {text}");
        (0..text.len() / 2)
            .map(|index| u8::from_str_radix(&text[index * 2..index * 2 + 2], 16).unwrap())
            .collect()
    }

    fn trace_socket(fields: &[&str]) -> ViscaSocket {
        match trace_field(fields, "socket") {
            "1" => ViscaSocket::S1,
            "2" => ViscaSocket::S2,
            other => panic!("fixture socket {other}"),
        }
    }

    fn trace_socket_suffix(phase: &str) -> ViscaSocket {
        match phase {
            "executing(socket=1)" => ViscaSocket::S1,
            "executing(socket=2)" => ViscaSocket::S2,
            other => panic!("fixture phase {other}"),
        }
    }

    /// The envelope sequence is decoder-owned end to end: the owner carries the
    /// value its *driver* stamped on the write into the engine's correlation
    /// table, rather than inventing one or matching on arrival order. A
    /// socketless Sony reply naming a different sequence therefore belongs to
    /// some other write and must leave this request exactly where it was.
    #[test]
    fn a_sony_reply_is_correlated_by_the_sequence_the_driver_stamped() {
        let mut owner_policy = policy(1, TransportKind::Datagram);
        owner_policy.protocol.envelope = EnvelopeKind::Sony;
        let mut owner = BlockingOwner::new(owner_policy).unwrap();
        let mut driver = FramingDriver::new(EnvelopeKind::Sony);
        let receipt = owner
            .submit(
                &mut driver,
                command(CameraId::CAMERA_1, CancellationPolicy::Supported, None),
            )
            .unwrap();
        let stamped = driver.sequences[0].expect("the Sony envelope stamps every write");
        let now = Instant::now();

        let sequenced = |value: u32, response| DecodedFrame {
            target: CameraId::CAMERA_1,
            sequence: Some(EnvelopeSequence {
                value,
                width: SequenceWidth::Full32,
            }),
            response,
        };
        let phase = |owner: &BlockingOwner| {
            owner
                .state()
                .request_state(receipt.id)
                .map(|state| state.0)
                .expect("the request is still tracked")
        };

        owner
            .inject_frame(
                &mut driver,
                sequenced(
                    stamped.wrapping_add(1),
                    DecodedResponse::Ack { socket: None },
                ),
                now,
            )
            .unwrap();
        assert!(
            matches!(phase(&owner), Phase::AwaitingAck { .. }),
            "a foreign sequence must not acknowledge this request"
        );

        owner
            .inject_frame(
                &mut driver,
                sequenced(stamped, DecodedResponse::Ack { socket: None }),
                now,
            )
            .unwrap();
        assert!(
            matches!(phase(&owner), Phase::Executing { .. }),
            "the stamped sequence is this request's acknowledgement"
        );

        owner
            .inject_frame(
                &mut driver,
                sequenced(
                    stamped.wrapping_add(1),
                    DecodedResponse::Completion { socket: None },
                ),
                now,
            )
            .unwrap();
        assert!(
            matches!(phase(&owner), Phase::Executing { .. }),
            "a foreign sequence must not complete this request"
        );

        owner
            .inject_frame(
                &mut driver,
                sequenced(stamped, DecodedResponse::Completion { socket: None }),
                now,
            )
            .unwrap();
        assert!(
            matches!(receipt.terminal(), Some(RuntimeOutcome::Applied)),
            "the stamped sequence completes this request"
        );
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
            strict_unconfirmed_poison: false,
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
        let mut context = request_context(retry);
        context.timeout.inquiry = INQUIRY;
        RuntimeRequest::Inquiry {
            wire: wire(),
            context,
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
    fn expired_ack_deadline_counts_a_timeout_and_the_retry_it_scheduled_for_sony() {
        let start = Instant::now();
        let mut state = OwnerState::new(owner_policy(EnvelopeKind::Sony)).unwrap();
        let _observer = sent(&mut state, command(retrying()), Some(1), start);
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
    fn expired_completion_deadline_counts_separately_from_the_ack_deadline_for_sony() {
        let start = Instant::now();
        let mut state = OwnerState::new(owner_policy(EnvelopeKind::Sony)).unwrap();
        let _observer = sent(&mut state, command(retrying()), Some(1), start);
        apply(
            &mut state,
            frame(
                Some(1),
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
                frame(None, DecodedResponse::Error { socket: None, code }),
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
                    socket: None,
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

    /// A transient receive fault (#565) retries a sequenced Sony command while
    /// it awaits its ACK. Raw commands poison the session because they have no
    /// sequence key that can make replay safe.
    #[test]
    fn a_transient_receive_fault_retry_counts_as_a_retry_for_sony() {
        let start = Instant::now();
        let mut state = OwnerState::new(owner_policy(EnvelopeKind::Sony)).unwrap();
        let _observer = sent(&mut state, command(retrying()), Some(1), start);
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
        assert_eq!(size_of::<OwnerMetrics>(), 20 * size_of::<u64>());
    }
}

/// Issue #637: the receive-error contract both owners now share. A read error
/// that only reports "no bytes arrived" is an idle read, not a fault.
#[test]
fn an_idle_read_error_reports_no_data_rather_than_a_fault() {
    use std::io::ErrorKind;
    use std::sync::Arc;

    for idle in [
        Error::Timeout,
        Error::Io(Arc::new(std::io::Error::from(ErrorKind::TimedOut))),
        Error::Io(Arc::new(std::io::Error::from(ErrorKind::WouldBlock))),
        Error::Io(Arc::new(std::io::Error::from(ErrorKind::Interrupted))),
        Error::Timeout.with_context("idle poll"),
    ] {
        assert!(
            receive_reported_no_data(&idle),
            "an idle read must not be classified as a fault: {idle}"
        );
    }
    for fault in [
        Error::TransportError("ICMP port unreachable".into()),
        Error::Io(Arc::new(std::io::Error::from(ErrorKind::ConnectionRefused))),
        Error::ConnectionClosed { reason: None },
    ] {
        assert!(
            !receive_reported_no_data(&fault),
            "a real read failure must stay a fault: {fault}"
        );
    }
}

/// Issue #637: a datagram send failure fails one request while the session
/// keeps running, so the value the caller sees must never claim a replacement
/// session is required.
#[test]
fn a_datagram_send_failure_is_normalized_to_a_per_request_error() {
    let normalized = normalize_datagram_send_error(Error::ConnectionClosed {
        reason: Some("socket closed".into()),
    });
    assert!(matches!(normalized, Error::TransportError(_)));
    assert!(!normalized.requires_new_session());
    assert!(
        normalized.to_string().contains("socket closed"),
        "the original cause must survive normalization: {normalized}"
    );

    // Errors that are already per-request are passed through untouched.
    assert!(matches!(
        normalize_datagram_send_error(Error::Timeout),
        Error::Timeout
    ));
    assert!(matches!(
        normalize_datagram_send_error(Error::TransportError("serial encode failed".into())),
        Error::TransportError(_)
    ));
}
/// Issue #634: every normative issue-542 lifecycle fixture replays through the
/// production owner and engine.
///
/// The replay below is deliberately *not* a model of the runtime. It drives the
/// real [`OwnerState`] — which owns the real protocol engine, the real
/// admission permit pool, the real terminal observers and the real
/// applied-state subscribers — and renders what production actually produced
/// back into the fixture's own record language. Every fixture record is then
/// compared against that rendering, so an engine or owner behavior change fails
/// the fixture instead of a hand-written simulator.
///
/// Only [`Effect::DeadlineExpired`] has no fixture vocabulary: the trace format
/// names the *boundary* that resolved a request rather than the deadline
/// classification, so the effect is consumed to prove `source=scheduler` and
/// emits no record of its own.
mod lifecycle_trace {
    use std::{
        borrow::Cow,
        collections::{BTreeMap, BTreeSet, VecDeque},
        sync::Arc,
        time::{Duration, Instant},
    };

    use super::super::*;
    use crate::{
        protocol::response::{decode_basic, BasicKind},
        runtime::engine::{
            CancellationPolicy, ControlPolicy, EncodedMessage, EnvelopeKind, InquiryRoute,
            RequestContext, RetryPolicy, TimeoutPolicy, TransportKind,
        },
        CameraId, Error,
    };

    const OBSERVER_LATE_DELIVERY: &str =
        include_str!("../../../tests/fixtures/issue_542/lifecycle/observer_late_delivery.trace");
    const DEADLINE_CLASSES: &str =
        include_str!("../../../tests/fixtures/issue_542/lifecycle/deadline_classes.trace");
    const CAPACITY_AND_FAILURES: &str =
        include_str!("../../../tests/fixtures/issue_542/lifecycle/capacity_and_failures.trace");
    const PTZOPTICS_CANCEL: &str =
        include_str!("../../../tests/fixtures/issue_542/lifecycle/ptzoptics_cancel.trace");
    const BLOCKING_OUT_OF_ORDER: &str =
        include_str!("../../../tests/fixtures/issue_542/lifecycle/blocking_out_of_order.trace");

    /// No fixture request may reach a deadline it did not ask for.
    const UNREACHABLE: Duration = Duration::from_secs(3_600);

    #[derive(Clone, Copy, PartialEq, Eq)]
    enum ObserverKind {
        Blocking,
        Async,
    }

    struct ObserverSlot {
        kind: ObserverKind,
        /// Dropping the observer *is* the detach operation.
        observer: Option<CompletionObserver>,
        cell: Arc<ObserverCell>,
        terminal_source: Option<&'static str>,
    }

    struct Pending {
        label: String,
        kind: ObserverKind,
        observer: CompletionObserver,
        admission: flume::Receiver<Result<RequestId, Error>>,
        permits_before: usize,
        active_before: usize,
    }

    fn lines(fixture: &str) -> Vec<&str> {
        fixture
            .lines()
            .map(str::trim)
            .filter(|line| !line.is_empty() && !line.starts_with('#'))
            .collect()
    }

    fn tokens(line: &str) -> Vec<&str> {
        line.split_ascii_whitespace().collect()
    }

    fn field<'a>(tokens: &[&'a str], name: &str) -> Option<&'a str> {
        tokens.iter().find_map(|token| {
            token
                .split_once('=')
                .filter(|(key, _)| *key == name)
                .map(|(_, value)| value)
        })
    }

    fn required<'a>(tokens: &[&'a str], name: &str) -> &'a str {
        field(tokens, name).unwrap_or_else(|| panic!("fixture field {name} in {tokens:?}"))
    }

    fn hex(text: &str) -> Vec<u8> {
        assert!(text.len().is_multiple_of(2), "fixture byte string {text}");
        (0..text.len() / 2)
            .map(|index| u8::from_str_radix(&text[index * 2..index * 2 + 2], 16).unwrap())
            .collect()
    }

    fn hex_text(bytes: &[u8]) -> String {
        bytes.iter().fold(String::new(), |mut text, byte| {
            use std::fmt::Write as _;
            let _ = write!(text, "{byte:02x}");
            text
        })
    }

    fn phase_name(phase: Phase) -> String {
        match phase {
            Phase::Ready { .. } => "ready".to_owned(),
            Phase::Sending { .. } => "sending".to_owned(),
            Phase::AwaitingAck { .. } => "awaiting-ack".to_owned(),
            Phase::Executing { socket, .. } => {
                format!("executing(socket={})", socket.as_socket_number())
            }
            Phase::AwaitingReply { .. } => "awaiting-reply".to_owned(),
            Phase::Backoff { .. } => "backoff".to_owned(),
            Phase::AwaitingCancellationResolution { socket, .. } => {
                format!(
                    "awaiting-cancellation(socket={})",
                    socket.as_socket_number()
                )
            }
            Phase::AwaitingLateAck { .. } => "awaiting-late-ack".to_owned(),
        }
    }

    const fn session_name(state: SessionState) -> &'static str {
        match state {
            SessionState::Running => "running",
            SessionState::Closed => "closed",
            SessionState::Shutdown => "shutdown",
            SessionState::Poisoned => "poisoned",
        }
    }

    const fn transport_name(transport: TransportKind) -> &'static str {
        match transport {
            TransportKind::Datagram => "datagram",
            TransportKind::Stream => "stream",
        }
    }

    const fn cancellation_name(policy: CancellationPolicy) -> &'static str {
        match policy {
            CancellationPolicy::Supported => "supported",
            CancellationPolicy::Unsupported => "unsupported",
        }
    }

    /// Maps one engine error back onto the fixture's terminal vocabulary.
    ///
    /// A transport failure round-trips through its injected error, so an engine
    /// that substitutes or collapses errors renders a different label.
    fn error_label(error: &Error) -> String {
        match error {
            Error::Timeout => "Timeout".to_owned(),
            Error::SyntaxError => "SyntaxError".to_owned(),
            Error::MessageLengthError => "MessageLengthError".to_owned(),
            Error::CommandBufferFull => "CommandBufferFull".to_owned(),
            Error::CommandCanceled => "CommandCanceled".to_owned(),
            Error::NoSocket => "NoSocket".to_owned(),
            Error::CommandNotExecutable => "CommandNotExecutable".to_owned(),
            Error::NotSupported => "NotSupported".to_owned(),
            Error::RuntimeShutdown => "RuntimeShutdown".to_owned(),
            Error::StreamPoisoned { .. } => "StreamPoisoned".to_owned(),
            Error::RuntimeQueueFull { .. } => "Capacity".to_owned(),
            Error::TransportError(reason) => reason.to_string(),
            other => format!("{other:?}"),
        }
    }

    fn outcome_text(outcome: &RuntimeOutcome) -> String {
        match outcome {
            RuntimeOutcome::Applied => "Applied".to_owned(),
            RuntimeOutcome::Cancelled => "Cancelled".to_owned(),
            RuntimeOutcome::Failed(error) => format!("Error::{}", error_label(error)),
            RuntimeOutcome::Reply { route, payload } => {
                format!("Reply(route={route:?},payload={payload:?})")
            }
        }
    }

    fn write_result(label: &str) -> Result<TransmissionMeta, Error> {
        match label {
            "ok" => Ok(TransmissionMeta { sequence: None }),
            "Timeout" => Err(Error::Timeout),
            other => Err(Error::TransportError(Cow::Owned(other.to_owned()))),
        }
    }

    /// Recovers the fixture label of the write failure that poisoned a stream
    /// session from the owner's own boundary error.
    fn poison_cause(state: &OwnerState) -> String {
        match state.boundary_error() {
            Some(Error::StreamPoisoned { reason }) => ["WriteFailed", "WriteTimeout", "Timeout"]
                .into_iter()
                .find(|label| {
                    write_result(label)
                        .err()
                        .is_some_and(|error| error.to_string() == reason.as_ref())
                })
                .map_or_else(|| reason.to_string(), ToOwned::to_owned),
            other => panic!("a poisoned session without a stream-poison boundary error: {other:?}"),
        }
    }

    fn owner_policy(
        transport: TransportKind,
        capacity: usize,
        cancellation: CancellationPolicy,
    ) -> OwnerPolicy {
        let protocol = ProtocolPolicy {
            capacity,
            envelope: EnvelopeKind::Raw,
            transport,
            inquiry_capacity: capacity,
            command_spacing: Duration::ZERO,
            inquiry_spacing: Duration::ZERO,
            inquiry_cooldown: Duration::ZERO,
            strict_unconfirmed_poison: false,
        };
        let mut targets = [None; 9];
        for slot in targets.iter_mut().take(4).skip(1) {
            *slot = Some(TargetPolicy {
                command_sockets: 2,
                cancellation,
            });
        }
        OwnerPolicy::with_targets(protocol, targets).unwrap()
    }

    fn command(
        target: CameraId,
        wire: &str,
        cancellation: CancellationPolicy,
        ack: Duration,
    ) -> RuntimeRequest {
        RuntimeRequest::Command {
            wire: Arc::new(EncodedMessage::new(&hex(wire)).unwrap()),
            context: RequestContext {
                target,
                timeout: TimeoutPolicy {
                    ack,
                    completion: UNREACHABLE,
                    inquiry: UNREACHABLE,
                    cancellation: UNREACHABLE,
                    ambiguity: UNREACHABLE,
                },
                retry: RetryPolicy::NEVER,
                control: ControlPolicy::default(),
                cancellation,
            },
            // Applied-state delivery is one of the normative observations, and
            // an unsubscribed fixture simply produces no subscriber record.
            applied_state: Some(
                AppliedStateProjection::set(WriteOnlyState::Spotlight, &[1]).unwrap(),
            ),
        }
    }

    fn inquiry(
        target: CameraId,
        wire: &str,
        cancellation: CancellationPolicy,
        reply: Duration,
    ) -> RuntimeRequest {
        RuntimeRequest::Inquiry {
            wire: Arc::new(EncodedMessage::new(&hex(wire)).unwrap()),
            context: RequestContext {
                target,
                timeout: TimeoutPolicy {
                    ack: UNREACHABLE,
                    completion: UNREACHABLE,
                    inquiry: reply,
                    cancellation: UNREACHABLE,
                    ambiguity: UNREACHABLE,
                },
                retry: RetryPolicy::NEVER,
                control: ControlPolicy::default(),
                cancellation,
            },
            route: InquiryRoute::UNKNOWN,
        }
    }

    struct Replay {
        state: Option<OwnerState>,
        cancellation: CancellationPolicy,
        base: Instant,
        at: u64,
        now: Instant,
        scheduler_dispatch: BTreeMap<String, u64>,
        labels: BTreeMap<String, RequestId>,
        observers: BTreeMap<RequestId, ObserverSlot>,
        subscribers: Vec<(String, AppliedStateSubscription)>,
        pending: Option<Pending>,
        write_label: String,
        cancel_phase: Option<Phase>,
        next_transmission: u64,
        cancel_transmissions: usize,
    }

    impl Replay {
        fn new(scheduler_dispatch: BTreeMap<String, u64>) -> Self {
            let base = Instant::now();
            Self {
                state: None,
                cancellation: CancellationPolicy::Supported,
                base,
                at: 0,
                now: base,
                scheduler_dispatch,
                labels: BTreeMap::new(),
                observers: BTreeMap::new(),
                subscribers: Vec::new(),
                pending: None,
                write_label: "ok".to_owned(),
                cancel_phase: None,
                next_transmission: 0,
                cancel_transmissions: 0,
            }
        }

        fn state(&self) -> &OwnerState {
            self.state.as_ref().expect("fixture session")
        }

        fn state_mut(&mut self) -> &mut OwnerState {
            self.state.as_mut().expect("fixture session")
        }

        fn id(&self, label: &str) -> RequestId {
            *self
                .labels
                .get(label)
                .unwrap_or_else(|| panic!("unknown fixture request label {label}"))
        }

        fn step(&mut self, input: &[&str]) -> Vec<String> {
            self.at = input[0].parse().expect("fixture timestamp");
            self.now = self.base + Duration::from_millis(self.at);
            let mut out = Vec::new();
            match input[2] {
                "session" => self.session(input, &mut out),
                "subscribe-applied" => self.subscribe_applied(input, &mut out),
                "admit" => self.admit(input, &mut out),
                "blocking-submit" => self.blocking_submit(input, &mut out),
                "dispatch" => self.dispatch(self.id(input[3]), required(input, "result"), &mut out),
                "frame" => self.frame(input, &mut out),
                "observer-timeout" => self.observer_deadline(input, true, &mut out),
                "detach" => self.observer_deadline(input, false, &mut out),
                "wake" => self.wake(&mut out),
                "cancel" => self.cancel(input, &mut out),
                "shutdown" => self.shutdown(&mut out),
                "blocking-wait" => self.blocking_wait(input, &mut out),
                "inspect-transmissions" => self.inspect_transmissions(input, &mut out),
                other => panic!("unknown lifecycle fixture input {other}"),
            }
            out
        }

        fn session(&mut self, input: &[&str], out: &mut Vec<String>) {
            let transport = match required(input, "transport") {
                "datagram" => TransportKind::Datagram,
                "stream" => TransportKind::Stream,
                other => panic!("unknown fixture transport {other}"),
            };
            let capacity = required(input, "capacity")
                .parse()
                .expect("fixture admission capacity");
            let cancellation = match required(input, "cancel") {
                "supported" => CancellationPolicy::Supported,
                "unsupported" => CancellationPolicy::Unsupported,
                other => panic!("unknown fixture cancellation policy {other}"),
            };
            self.cancellation = cancellation;
            self.labels.clear();
            self.observers.clear();
            self.subscribers.clear();
            self.pending = None;
            self.next_transmission = 0;
            self.cancel_transmissions = 0;
            let state = OwnerState::new(owner_policy(transport, capacity, cancellation)).unwrap();
            let registered = state.policy().targets[1].expect("camera one is registered");
            out.push(format!(
                "{} effect session state={} transport={} capacity={} cancel={}",
                self.at,
                session_name(state.state()),
                transport_name(state.policy().protocol.transport),
                state.permits().capacity(),
                cancellation_name(registered.cancellation),
            ));
            self.state = Some(state);
        }

        fn subscribe_applied(&mut self, input: &[&str], out: &mut Vec<String>) {
            let name = input[3].to_owned();
            let target = CameraId::new(required(input, "target").parse().unwrap()).unwrap();
            let subscription = self
                .state_mut()
                .subscribe_applied(Some(target), 8)
                .expect("applied-state subscription");
            self.subscribers.push((name.clone(), subscription));
            out.push(format!(
                "{} effect subscriber-added observer={name} target={}",
                self.at,
                target.id()
            ));
        }

        fn admit(&mut self, input: &[&str], out: &mut Vec<String>) {
            let label = input[3].to_owned();
            let target = CameraId::new(required(input, "target").parse().unwrap()).unwrap();
            let wire = required(input, "wire");
            let kind = match required(input, "observer") {
                "async" => ObserverKind::Async,
                "blocking" => ObserverKind::Blocking,
                other => panic!("unknown fixture observer kind {other}"),
            };
            // A scheduler deadline in the fixture is an absolute trace time; the
            // engine computes it from the instant the request is actually sent.
            let ack = field(input, "scheduler").map_or(UNREACHABLE, |value| {
                let deadline: u64 = value.parse().expect("fixture scheduler deadline");
                let sent_at = self.scheduler_dispatch[&label];
                Duration::from_millis(deadline - sent_at)
            });

            let permits_before = self.state().permits().available();
            let active_before = self.state().active_len();
            let Some(permit) = self.state().permits().try_acquire() else {
                out.push(format!(
                    "{} outcome admission ticket={label} error=Capacity capacity={} engine-entry=none observer=none transmission=none",
                    self.at,
                    self.state().permits().capacity()
                ));
                assert_eq!(
                    self.state().active_len(),
                    active_before,
                    "a rejected admission never creates an engine entry"
                );
                return;
            };

            let request = match field(input, "kind").unwrap_or("command") {
                "command" => command(target, wire, self.cancellation, ack),
                "inquiry" => inquiry(target, wire, self.cancellation, ack),
                other => panic!("unknown fixture request kind {other}"),
            };
            let now = self.now;
            let (staged, observer, admission) = self.state_mut().stage_admission(request, permit);
            let Input::Admit { ticket, request } = staged else {
                panic!("staged admission did not produce an admit input")
            };
            self.pending = Some(Pending {
                label,
                kind,
                observer,
                admission,
                permits_before,
                active_before,
            });
            let effects = self.state_mut().admit_without_due(ticket, request, now);
            self.drain(effects, "admission", out);
            assert!(
                self.pending.is_none(),
                "admission produced neither an entry nor a rejection"
            );
        }

        fn blocking_submit(&mut self, input: &[&str], out: &mut Vec<String>) {
            self.admit(input, out);
            let label = input[3];
            let id = self.id(label);
            self.dispatch(id, "ok", out);
            assert!(
                matches!(
                    self.state().request_state(id),
                    Some((Phase::AwaitingAck { .. }, _))
                ),
                "a blocking handle is returned only after its initial write completes"
            );
            out.push(format!(
                "{} outcome blocking-handle ticket={label} id={} initial-write=complete",
                self.at,
                id.get()
            ));
        }

        fn dispatch(&mut self, id: RequestId, result: &str, out: &mut Vec<String>) {
            self.write_label = result.to_owned();
            let now = self.now;
            match self.state_mut().first_dispatch_without_due(id, now) {
                FirstDispatch::Effects(effects) => self.drain(effects.into(), "transport", out),
                other => panic!("fixture dispatch is not the scheduler winner: {other:?}"),
            }
        }

        fn frame(&mut self, input: &[&str], out: &mut Vec<String>) {
            let kind = input[3];
            let bytes = hex(required(input, "bytes"));
            let decoded = decode_basic(&bytes).expect("fixture frame bytes decode");
            let response = match (kind, decoded.kind) {
                ("ack", BasicKind::Ack) => DecodedResponse::Ack {
                    socket: decoded.socket,
                },
                ("complete", BasicKind::Completion) => DecodedResponse::Completion {
                    socket: decoded.socket,
                },
                ("error", BasicKind::Error(code)) => DecodedResponse::Error {
                    socket: decoded.socket,
                    code,
                },
                other => panic!("fixture frame {other:?} disagrees with its own bytes"),
            };
            if let Some(socket) = field(input, "socket") {
                assert_eq!(
                    decoded.socket.map(|socket| socket.as_socket_number()),
                    socket.parse::<u8>().ok(),
                    "fixture socket disagrees with its own bytes"
                );
            }
            if let Some(code) = field(input, "code") {
                assert_eq!(
                    decoded.kind,
                    BasicKind::Error(u8::from_str_radix(code, 16).unwrap()),
                    "fixture error code disagrees with its own bytes"
                );
            }
            // Every lifecycle fixture is a raw session, so correlation is by
            // target and socket rather than by envelope sequence.
            let target = decoded.source;
            let sequence: Option<EnvelopeSequence> = None;
            let now = self.now;
            let pre_socket = |state: &OwnerState, id: RequestId| match state.request_state(id) {
                Some((Phase::Executing { socket, .. }, _)) => Some(socket),
                _ => None,
            };
            let owned = self
                .labels
                .values()
                .copied()
                .filter_map(|id| pre_socket(self.state(), id).map(|socket| (id, socket)))
                .collect::<BTreeMap<_, _>>();

            let turn = self.state().begin_input_turn(now);
            let effects = self.state_mut().input_in_turn(
                &turn,
                Input::Frame(DecodedFrame {
                    target,
                    sequence,
                    response,
                }),
            );
            let routed = effects
                .iter()
                .find_map(effect_request)
                .expect("a fixture frame is always routed to one request");
            let socket = decoded
                .socket
                .or_else(|| owned.get(&routed).copied())
                .expect("a routed raw frame owns a socket");
            out.push(format!(
                "{} effect frame-routed id={} target={} via={} socket={} bytes={}",
                self.at,
                routed.get(),
                target.id(),
                if sequence.is_some() {
                    "sequence"
                } else {
                    "target-socket"
                },
                socket.as_socket_number(),
                hex_text(&bytes)
            ));
            self.drain(effects, "protocol", out);
        }

        fn observer_deadline(&mut self, input: &[&str], timed_out: bool, out: &mut Vec<String>) {
            let id = self.id(input[3]);
            let slot = self
                .observers
                .get_mut(&id)
                .expect("fixture observer is registered");
            let observer = slot.observer.as_ref().expect("an attached observer");
            assert!(
                observer.try_recv().is_none(),
                "an observer that gives up cannot already hold a terminal outcome"
            );
            slot.observer = None;
            assert!(
                !slot.cell.is_attached(),
                "dropping the observer detaches it"
            );
            if timed_out {
                out.push(format!(
                    "{} outcome operation id={} Error::{} source=observer",
                    self.at,
                    id.get(),
                    error_label(&Error::Timeout)
                ));
            }
            let (phase, _) = self
                .state()
                .request_state(id)
                .expect("observer detach never removes engine routing");
            out.push(format!(
                "{} effect observer-detached id={} protocol-state={} routing=retained",
                self.at,
                id.get(),
                phase_name(phase)
            ));
        }

        fn wake(&mut self, out: &mut Vec<String>) {
            let now = self.now;
            let effects = self.state_mut().advance(now);
            self.drain(effects, "scheduler", out);
        }

        fn cancel(&mut self, input: &[&str], out: &mut Vec<String>) {
            let id = self.id(input[3]);
            let (phase, _) = self
                .state()
                .request_state(id)
                .expect("cancellation target is active");
            self.cancel_phase = Some(phase);
            let cancels_before = self.cancel_transmissions;
            let now = self.now;
            let turn = self.state().begin_input_turn(now);
            let effects = self.state_mut().input_in_turn(&turn, Input::Cancel { id });
            let ignored = effects.iter().all(|effect| {
                matches!(
                    effect,
                    Effect::CancellationObservation {
                        observation: CancellationObservation::Failed(_),
                        ..
                    }
                )
            });
            self.drain(effects, "local", out);
            if ignored {
                let (phase, cancellation) = self
                    .state()
                    .request_state(id)
                    .expect("an ignored cancellation leaves the request able to complete");
                assert_eq!(
                    cancellation,
                    CancelState::None,
                    "an ignored cancellation records no cancellation substate"
                );
                assert_eq!(
                    self.cancel_transmissions, cancels_before,
                    "an ignored cancellation emits no cancel frame"
                );
                out.push(format!(
                    "{} effect cancellation-ignored id={} protocol-state={} cancel-frame=none",
                    self.at,
                    id.get(),
                    phase_name(phase)
                ));
            }
        }

        fn shutdown(&mut self, out: &mut Vec<String>) {
            let now = self.now;
            let turn = self.state().begin_input_turn(now);
            let effects = self
                .state_mut()
                .input_in_turn(&turn, Input::Shutdown(ShutdownReason::Explicit));
            self.drain(effects, "shutdown", out);
        }

        fn blocking_wait(&mut self, input: &[&str], out: &mut Vec<String>) {
            let id = self.id(input[3]);
            let slot = self
                .observers
                .get_mut(&id)
                .expect("fixture observer is registered");
            let observer = slot.observer.take().expect("a retained blocking observer");
            let outcome = observation_outcome(
                observer
                    .try_recv()
                    .expect("a blocking wait consumes its retained terminal outcome"),
            );
            out.push(format!(
                "{} outcome blocking-wait id={} {} source={}",
                self.at,
                id.get(),
                outcome_text(&outcome),
                slot.terminal_source.expect("recorded terminal source")
            ));
        }

        fn inspect_transmissions(&mut self, input: &[&str], out: &mut Vec<String>) {
            assert_eq!(required(input, "kind"), "cancel");
            out.push(format!(
                "{} outcome transmissions kind=cancel count={}",
                self.at, self.cancel_transmissions
            ));
        }

        fn drain(
            &mut self,
            effects: VecDeque<Effect>,
            boundary: &'static str,
            out: &mut Vec<String>,
        ) {
            let mut queue = effects;
            let mut deferred: Vec<(RequestId, String)> = Vec::new();
            let mut applied: Vec<(RequestId, String)> = Vec::new();
            let mut deadlines: BTreeSet<RequestId> = BTreeSet::new();
            while let Some(effect) = queue.pop_front() {
                match effect {
                    Effect::Admitted { id, .. } => {
                        self.record_admitted(id, effect, out);
                    }
                    Effect::AdmissionRejected { .. } => {
                        self.record_admission_rejected(effect, out);
                    }
                    Effect::Transition { id, from, to, .. } => {
                        self.state_mut().apply_effect(effect);
                        out.push(format!(
                            "{} effect state id={} from={} to={}",
                            self.at,
                            id.get(),
                            phase_name(from),
                            phase_name(to)
                        ));
                    }
                    Effect::Transmit { .. } => {
                        self.record_transmit(effect, &mut queue, out);
                    }
                    Effect::DeadlineExpired { id, .. } => {
                        deadlines.insert(id);
                        self.state_mut().apply_effect(effect);
                    }
                    Effect::CancellationRecorded { id } => {
                        self.state_mut().apply_effect(effect);
                        let queued = matches!(
                            self.cancel_phase,
                            Some(Phase::Ready { .. } | Phase::Backoff { .. })
                        );
                        out.push(format!(
                            "{} effect cancellation-recorded id={} disposition={} cancel-frame=none",
                            self.at,
                            id.get(),
                            if queued { "queued-local" } else { "sent" }
                        ));
                    }
                    Effect::CancellationObservation {
                        id,
                        ref observation,
                    } => {
                        let (text, source) = match observation {
                            CancellationObservation::Recorded => ("Recorded".to_owned(), "local"),
                            CancellationObservation::Cancelled => ("Cancelled".to_owned(), "local"),
                            CancellationObservation::Completed => {
                                ("Completed".to_owned(), "protocol")
                            }
                            CancellationObservation::Failed(error) => (
                                format!("Error::{}", error_label(error)),
                                if matches!(error, Error::NotSupported) {
                                    "profile"
                                } else {
                                    "engine"
                                },
                            ),
                        };
                        deferred.push((
                            id,
                            format!(
                                "{} outcome cancellation id={} {text} source={source}",
                                self.at,
                                id.get()
                            ),
                        ));
                        self.state_mut().apply_effect(effect);
                    }
                    Effect::AppliedState { effect: projection } => {
                        self.state_mut()
                            .apply_effect(Effect::AppliedState { effect: projection });
                        for (name, subscription) in &self.subscribers {
                            if let Some(AppliedStateEvent(delivered)) = subscription.try_recv() {
                                applied.push((
                                    projection.request,
                                    format!(
                                        "{} outcome applied-state observer={name} target={} id={} state={}",
                                        self.at,
                                        delivered.target.id(),
                                        delivered.request.get(),
                                        if delivered.projection.is_known() {
                                            "applied"
                                        } else {
                                            "invalidated"
                                        }
                                    ),
                                ));
                            }
                        }
                    }
                    Effect::Terminal { id, ref outcome } => {
                        let source = if deadlines.contains(&id) {
                            "scheduler"
                        } else if boundary == "protocol"
                            && matches!(outcome, RuntimeOutcome::Failed(_))
                        {
                            "camera"
                        } else {
                            boundary
                        };
                        let text = outcome_text(outcome);
                        let permits_before = self.state().permits().available();
                        self.state_mut().apply_effect(effect.clone());
                        let removed = self.state().request_state(id).is_none();
                        let released = self.state().permits().available() == permits_before + 1;
                        out.push(format!(
                            "{} effect terminal id={} outcome={text} source={source} state={} permit={}",
                            self.at,
                            id.get(),
                            if removed { "removed" } else { "retained" },
                            if released { "released" } else { "held" }
                        ));
                        self.record_observer(id, source, out);
                        applied.retain(|(request, record)| {
                            let mine = *request == id;
                            if mine {
                                out.push(record.clone());
                            }
                            !mine
                        });
                        deferred.retain(|(request, record)| {
                            let mine = *request == id;
                            if mine {
                                out.push(record.clone());
                            }
                            !mine
                        });
                    }
                    Effect::SessionChanged { from, to } => {
                        self.state_mut().apply_effect(effect);
                        let detail = match to {
                            SessionState::Poisoned => {
                                format!("cause={}", poison_cause(self.state()))
                            }
                            SessionState::Shutdown => "reason=explicit".to_owned(),
                            other => panic!("no fixture vocabulary for session state {other:?}"),
                        };
                        out.push(format!(
                            "{} effect session from={} to={} {detail}",
                            self.at,
                            session_name(from),
                            session_name(to)
                        ));
                    }
                    Effect::RetryScheduled { .. } | Effect::Ignored(_) => {
                        panic!("no fixture vocabulary for {effect:?}")
                    }
                }
            }
            for (_, record) in applied {
                out.push(record);
            }
            for (_, record) in deferred {
                out.push(record);
            }
        }

        fn record_admitted(&mut self, id: RequestId, effect: Effect, out: &mut Vec<String>) {
            let pending = self.pending.take().expect("a staged admission");
            self.state_mut().apply_effect(effect);
            let target = self
                .state()
                .diagnostics()
                .filter_map(|event| match event {
                    DiagnosticEvent::Admitted {
                        id: recorded,
                        target,
                        ..
                    } if *recorded == id => Some(*target),
                    _ => None,
                })
                .last()
                .expect("owner records every admission");
            let (phase, _) = self
                .state()
                .request_state(id)
                .expect("an admitted request owns an engine entry");
            let acquired = self.state().permits().available() == pending.permits_before - 1;
            out.push(format!(
                "{} effect admitted ticket={} id={} target={} permit={} state={} observer={}",
                self.at,
                pending.label,
                id.get(),
                target.id(),
                if acquired { "acquired" } else { "unclaimed" },
                phase_name(phase),
                match pending.kind {
                    ObserverKind::Async => "async",
                    ObserverKind::Blocking => "blocking",
                }
            ));
            let admitted = pending
                .admission
                .try_recv()
                .expect("admission reply")
                .expect("admission reply carries the engine identity");
            out.push(format!(
                "{} outcome admission ticket={} id={}",
                self.at,
                pending.label,
                admitted.get()
            ));
            self.labels.insert(pending.label, id);
            self.observers.insert(
                id,
                ObserverSlot {
                    kind: pending.kind,
                    cell: Arc::clone(&pending.observer.cell),
                    observer: Some(pending.observer),
                    terminal_source: None,
                },
            );
        }

        fn record_admission_rejected(&mut self, effect: Effect, out: &mut Vec<String>) {
            let pending = self.pending.take().expect("a staged admission");
            let AppliedEffect::AdmissionRejected(error) = self.state_mut().apply_effect(effect)
            else {
                panic!("a rejected admission did not report its error")
            };
            let capacity = match &error {
                Error::RuntimeQueueFull { capacity } => format!(" capacity={capacity}"),
                _ => String::new(),
            };
            assert_eq!(
                self.state().active_len(),
                pending.active_before,
                "a rejected admission never creates an engine entry"
            );
            assert_eq!(
                self.state().permits().available(),
                pending.permits_before,
                "a rejected admission releases its permit"
            );
            assert!(
                matches!(pending.admission.try_recv(), Ok(Err(_)) | Err(_)),
                "a rejected admission never replies with an engine identity"
            );
            out.push(format!(
                "{} outcome admission ticket={} error={}{capacity} engine-entry=none observer=none transmission=none",
                self.at,
                pending.label,
                error_label(&error)
            ));
        }

        fn record_transmit(
            &mut self,
            effect: Effect,
            queue: &mut VecDeque<Effect>,
            out: &mut Vec<String>,
        ) {
            let Effect::Transmit {
                request, ref kind, ..
            } = effect
            else {
                unreachable!()
            };
            self.next_transmission += 1;
            let transmission = self.next_transmission;
            match kind {
                Transmission::Request { target, wire, .. } => out.push(format!(
                    "{} effect transmit tx={transmission} id={} kind=request target={} wire={}",
                    self.at,
                    request.get(),
                    target.id(),
                    hex_text(wire.as_bytes())
                )),
                Transmission::Cancel { target, socket, .. } => {
                    self.cancel_transmissions += 1;
                    out.push(format!(
                        "{} effect transmit tx={transmission} id={} kind=cancel target={} socket={}",
                        self.at,
                        request.get(),
                        target.id(),
                        socket.as_socket_number()
                    ));
                }
            }
            let AppliedEffect::Transmit(staged) = self.state_mut().apply_effect(effect) else {
                panic!("a transmit effect did not stage a write")
            };
            let label = self.write_label.clone();
            out.push(format!(
                "{} driver-input transmission-finished tx={transmission} result={label}",
                self.at
            ));
            let now = self.now;
            let produced =
                self.state_mut()
                    .finish_write_without_due(&staged, write_result(&label), now);
            prepend_effects(queue, produced);
        }

        fn record_observer(&mut self, id: RequestId, source: &'static str, out: &mut Vec<String>) {
            let at = self.at;
            let Some(slot) = self.observers.get_mut(&id) else {
                return;
            };
            slot.terminal_source = Some(source);
            let Some(observer) = slot.observer.as_ref() else {
                assert!(
                    !slot.cell.is_attached() && !slot.cell.resolved.load(Ordering::Acquire),
                    "a detached observer is never resolved"
                );
                out.push(format!(
                    "{at} effect delivery-discarded id={} observer=detached",
                    id.get()
                ));
                return;
            };
            match slot.kind {
                ObserverKind::Async => {
                    let outcome = observation_outcome(
                        observer
                            .try_recv()
                            .expect("an attached observer receives its terminal outcome"),
                    );
                    out.push(format!(
                        "{at} outcome operation id={} {} source={source}",
                        id.get(),
                        outcome_text(&outcome)
                    ));
                }
                ObserverKind::Blocking => {
                    assert!(
                        !observer.receiver.is_empty(),
                        "a blocking observer retains its terminal outcome until it waits"
                    );
                    out.push(format!(
                        "{at} effect outcome-retained id={} observer=blocking",
                        id.get()
                    ));
                }
            }
        }
    }

    fn effect_request(effect: &Effect) -> Option<RequestId> {
        match effect {
            Effect::Transition { id, .. }
            | Effect::Terminal { id, .. }
            | Effect::CancellationRecorded { id }
            | Effect::CancellationObservation { id, .. }
            | Effect::DeadlineExpired { id, .. }
            | Effect::Admitted { id, .. }
            | Effect::RetryScheduled { id, .. } => Some(*id),
            Effect::AppliedState { effect } => Some(effect.request),
            Effect::Transmit { request, .. } => Some(*request),
            Effect::AdmissionRejected { .. }
            | Effect::SessionChanged { .. }
            | Effect::Ignored(_) => None,
        }
    }

    /// Replays one fixture through the production owner and asserts that every
    /// `effect`, `outcome` and `driver-input` record it names is exactly what
    /// production produced.
    fn replay(fixture: &str) -> Vec<String> {
        let records = lines(fixture);
        let mut scheduler_dispatch = BTreeMap::new();
        for record in &records {
            let parsed = tokens(record);
            if parsed[1] == "input" && matches!(parsed[2], "dispatch" | "blocking-submit") {
                scheduler_dispatch.insert(parsed[3].to_owned(), parsed[0].parse().unwrap());
            }
        }

        let mut replay = Replay::new(scheduler_dispatch);
        let mut produced = Vec::new();
        let mut index = 0;
        while index < records.len() {
            let line = records[index];
            let parsed = tokens(line);
            assert_eq!(
                parsed[1], "input",
                "every fixture step begins with an input record: {line}"
            );
            index += 1;
            let mut expected = Vec::new();
            while index < records.len() {
                if tokens(records[index])[1] == "input" {
                    break;
                }
                expected.push(records[index].to_owned());
                index += 1;
            }
            let actual = replay.step(&parsed);
            assert_eq!(actual, expected, "production output diverged from {line}");
            produced.push(line.to_owned());
            produced.extend(actual);
        }
        assert_eq!(
            produced,
            records
                .iter()
                .map(|line| (*line).to_owned())
                .collect::<Vec<_>>(),
            "the fixture and the production replay must be the same record stream"
        );
        produced
    }

    #[test]
    fn observer_timeout_and_detach_preserve_late_routing_and_target_delivery() {
        let records = replay(OBSERVER_LATE_DELIVERY);
        assert!(records.iter().any(
            |line| line.contains("observer-detached id=1") && line.contains("routing=retained")
        ));
        assert!(records
            .iter()
            .any(|line| line.contains("applied-state observer=target-one")));
    }

    #[test]
    fn observer_scheduler_and_transport_deadlines_stay_distinct() {
        let records = replay(DEADLINE_CLASSES);
        for source in ["observer", "scheduler", "transport"] {
            assert!(
                records
                    .iter()
                    .any(|line| line.contains(&format!("source={source}"))),
                "the production replay must expose the {source} deadline boundary"
            );
        }
    }

    #[test]
    fn capacity_datagram_stream_poison_and_shutdown_boundaries_stay_distinct() {
        replay(CAPACITY_AND_FAILURES);
    }

    #[test]
    fn ptzoptics_g2_queued_cancel_is_local_but_sent_cancel_is_unsupported() {
        let records = replay(PTZOPTICS_CANCEL);
        assert!(records.iter().any(|line| line
            .contains("cancellation-recorded id=1 disposition=queued-local cancel-frame=none")));
        assert!(records
            .iter()
            .any(|line| line.contains("Error::NotSupported source=profile")));
        assert!(records
            .iter()
            .any(|line| line.ends_with("transmissions kind=cancel count=0")));
    }

    #[test]
    fn blocking_handles_follow_initial_write_and_retain_exact_out_of_order_outcomes() {
        let records = replay(BLOCKING_OUT_OF_ORDER);
        let first_write = records
            .iter()
            .position(|line| line.contains("transmission-finished tx=1 result=ok"))
            .expect("first initial write result");
        let first_handle = records
            .iter()
            .position(|line| line.contains("blocking-handle ticket=a"))
            .expect("first blocking handle");
        assert!(
            first_write < first_handle,
            "a blocking handle is returned only after its initial write completes"
        );
    }
}
