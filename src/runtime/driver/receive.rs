//! Shared receive-side VISCA classification and attribution.
//!
//! Transport adapters own framing and envelope extraction. This driver owns the
//! protocol response policy after a VISCA payload and optional envelope sequence
//! are available.

use std::borrow::Cow;

use tracing::{debug, trace, warn};

use crate::{
    camera::CommandId,
    camera_id::CameraId,
    capabilities::Profile,
    command::response::{lift_response_for_spec, Payload},
    error::Error,
    protocol::response::{decode_basic, BasicKind, BasicResponse},
    runtime::core::{ReplySource, SchedulerCore, SchedulerEvent},
};

/// Result of classifying one received VISCA payload.
#[derive(Debug)]
pub(crate) enum ReceiveDisposition {
    /// A scheduler event should be processed normally.
    Event(SchedulerEvent),
    /// A response was attributed to a command but failed semantic decoding.
    AttributedDecodeFailure { id: CommandId, error: Error },
    /// The frame was intentionally ignored without failing active work.
    Ignored { reason: IgnoreReason },
    /// The payload was not a syntactically valid VISCA response.
    Malformed(Error),
}

/// Why a receive payload was ignored.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum IgnoreReason {
    /// VISCA network-change notification.
    NetworkChange,
    /// Syntactically valid but unsupported/unknown response kind.
    UnknownResponse,
    /// Sony-sequenced traffic that did not resolve to an active command.
    UnmatchedSequence { sequence: u32 },
    /// A data reply could not be attributed to an active inquiry.
    UnattributedDataReply,
    /// A data reply used the sequence of an active non-inquiry command.
    UnexpectedDataReply { id: CommandId },
    /// A reply whose source address owns nothing in flight, on a transport
    /// carrying more than one camera.
    ///
    /// The address is authoritative there, so the frame is dropped rather than
    /// charged to a neighbour's command.
    UnownedSourceReply { camera: CameraId },
}

/// Classify and attribute one received VISCA payload.
pub(crate) fn receive_one<P: Profile>(
    core: &mut SchedulerCore,
    payload: &[u8],
    sequence: Option<u32>,
) -> ReceiveDisposition {
    let basic = match decode_basic(payload) {
        Some(basic) => basic,
        None => {
            return ReceiveDisposition::Malformed(Error::InvalidResponse {
                expected: Cow::Borrowed("Valid VISCA response"),
                actual: payload.to_vec(),
            });
        }
    };

    trace!(
        kind = ?basic.kind,
        socket = ?basic.socket,
        source = %basic.source,
        sequence,
        "Decoded VISCA response"
    );

    match basic.kind {
        BasicKind::Ack => {
            let cmd_id = sequence.and_then(|seq| core.get_command_by_sequence(seq));
            if let Some(reason) = ignore_unmatched_sequence(core, sequence, cmd_id) {
                return ReceiveDisposition::Ignored { reason };
            }

            let cmd_id =
                match attribute_to_source(core, &basic, sequence, cmd_id, |core, camera| {
                    core.find_ack_target_for(camera)
                }) {
                    Ok(cmd_id) => cmd_id,
                    Err(reason) => return ReceiveDisposition::Ignored { reason },
                };

            let source = ReplySource::from_fields(cmd_id, sequence, basic.socket);
            ReceiveDisposition::Event(SchedulerEvent::Ack { source })
        }
        BasicKind::Completion => classify_completion::<P>(core, &basic, sequence),
        BasicKind::Error(code) => classify_error(core, &basic, sequence, code),
        BasicKind::DataReply => classify_data_reply::<P>(core, &basic, sequence),
        BasicKind::NetworkChange => ReceiveDisposition::Ignored {
            reason: IgnoreReason::NetworkChange,
        },
        BasicKind::Unknown => ReceiveDisposition::Ignored {
            reason: IgnoreReason::UnknownResponse,
        },
    }
}

fn classify_completion<P: Profile>(
    core: &mut SchedulerCore,
    basic: &BasicResponse<'_>,
    sequence: Option<u32>,
) -> ReceiveDisposition {
    let cmd_id = sequence.and_then(|seq| core.get_command_by_sequence(seq));
    if let Some(reason) = ignore_unmatched_sequence(core, sequence, cmd_id) {
        return ReceiveDisposition::Ignored { reason };
    }

    let cmd_id = match attribute_to_source(core, basic, sequence, cmd_id, |core, camera| {
        basic
            .socket
            .and_then(|socket| core.find_command_on_socket_for(camera, socket))
    }) {
        Ok(cmd_id) => cmd_id,
        Err(reason) => return ReceiveDisposition::Ignored { reason },
    };

    let response_spec = cmd_id.and_then(|id| core.get_inquiry_response_spec(id));

    match lift_response_for_spec::<P>(basic, response_spec.as_ref()) {
        Ok(response) => {
            let source = ReplySource::from_fields(cmd_id, sequence, basic.socket);
            ReceiveDisposition::Event(SchedulerEvent::Completion { source, response })
        }
        Err(error) => attributed_decode_failure(cmd_id, error, "Completion"),
    }
}

fn classify_error(
    core: &mut SchedulerCore,
    basic: &BasicResponse<'_>,
    sequence: Option<u32>,
    code: u8,
) -> ReceiveDisposition {
    let mut cmd_id = sequence.and_then(|seq| core.get_command_by_sequence(seq));
    if let Some(reason) = ignore_unmatched_sequence(core, sequence, cmd_id) {
        return ReceiveDisposition::Ignored { reason };
    }

    if cmd_id.is_none() && basic.socket.is_none() && sequence.is_none() {
        // An error frame carries no inquiry payload, so this resolves through
        // the inquiry FIFO rather than by content.
        cmd_id =
            core.resolve_raw_inquiry_id(Payload::new(&[]), source_scope(core, basic, sequence));
    }

    let cmd_id = match attribute_to_source(core, basic, sequence, cmd_id, |core, camera| {
        core.find_error_target_for(camera, basic.socket)
    }) {
        Ok(cmd_id) => cmd_id,
        Err(reason) => return ReceiveDisposition::Ignored { reason },
    };

    debug!(
        "Received error 0x{code:02X} from {} for socket {:?}, cmd_id {:?}",
        basic.source, basic.socket, cmd_id
    );

    let source = ReplySource::from_fields(cmd_id, sequence, basic.socket);
    ReceiveDisposition::Event(SchedulerEvent::Error { source, code })
}

fn classify_data_reply<P: Profile>(
    core: &mut SchedulerCore,
    basic: &BasicResponse<'_>,
    sequence: Option<u32>,
) -> ReceiveDisposition {
    let cmd_id = match sequence {
        Some(seq) => match core.get_command_by_sequence(seq) {
            Some(id) if core.is_awaiting_inquiry_reply(id) => Some(id),
            Some(id) => {
                debug!(
                    %id,
                    sequence = seq,
                    "Ignoring data reply for active non-inquiry command"
                );
                return ReceiveDisposition::Ignored {
                    reason: IgnoreReason::UnexpectedDataReply { id },
                };
            }
            None => {
                record_unmatched_sequence(core, seq);
                return ReceiveDisposition::Ignored {
                    reason: IgnoreReason::UnmatchedSequence { sequence: seq },
                };
            }
        },
        None => core.resolve_raw_inquiry_id(basic.payload, source_scope(core, basic, sequence)),
    };

    let Some(cmd_id) = cmd_id else {
        return ReceiveDisposition::Ignored {
            reason: IgnoreReason::UnattributedDataReply,
        };
    };

    let Some(response_spec) = core.get_inquiry_response_spec(cmd_id) else {
        return ReceiveDisposition::Ignored {
            reason: IgnoreReason::UnexpectedDataReply { id: cmd_id },
        };
    };

    match lift_response_for_spec::<P>(basic, Some(&response_spec)) {
        Ok(response) => {
            let source = ReplySource::from_fields(Some(cmd_id), sequence, None);
            ReceiveDisposition::Event(SchedulerEvent::InquiryReply { source, response })
        }
        Err(error) => attributed_decode_failure(Some(cmd_id), error, "DataReply"),
    }
}

/// The camera a raw VISCA reply must be correlated against, if its source
/// address is discriminating on this transport.
///
/// Sony-encapsulated replies are correlated by sequence number, which is
/// authoritative; the VISCA address must never override it, so a sequenced
/// frame is never scoped here.
fn source_scope(
    core: &SchedulerCore,
    basic: &BasicResponse<'_>,
    sequence: Option<u32>,
) -> Option<CameraId> {
    if sequence.is_some() {
        return None;
    }
    core.reply_scope(basic.source)
}

/// Attribute an unsequenced reply to the camera named by its source address.
///
/// On a serial daisy chain the reply address is the only thing that ties a
/// frame to one of up to seven devices, so it is resolved here rather than left
/// to the scheduler's camera-blind socket and FIFO heuristics.
///
/// Returns:
/// - `Ok(Some(id))` when `resolved` already named a command, or the source
///   camera owns one that can take the reply,
/// - `Ok(None)` when the address is not discriminating - one camera at most has
///   work outstanding - so the scheduler's existing heuristics run unchanged,
/// - `Err(reason)` when the address is authoritative but nothing on that camera
///   can own the frame; the reply is ignored instead of being charged to a
///   neighbour on the bus.
fn attribute_to_source(
    core: &SchedulerCore,
    basic: &BasicResponse<'_>,
    sequence: Option<u32>,
    resolved: Option<CommandId>,
    resolve: impl FnOnce(&SchedulerCore, CameraId) -> Option<CommandId>,
) -> Result<Option<CommandId>, IgnoreReason> {
    if resolved.is_some() {
        return Ok(resolved);
    }

    let Some(camera) = source_scope(core, basic, sequence) else {
        return Ok(None);
    };

    match resolve(core, camera) {
        Some(cmd_id) => Ok(Some(cmd_id)),
        None => {
            debug!(
                %camera,
                kind = ?basic.kind,
                socket = ?basic.socket,
                "Ignoring reply from a camera with no matching work in flight"
            );
            Err(IgnoreReason::UnownedSourceReply { camera })
        }
    }
}

fn attributed_decode_failure(
    cmd_id: Option<CommandId>,
    error: Error,
    response_kind: &'static str,
) -> ReceiveDisposition {
    if let Some(id) = cmd_id {
        ReceiveDisposition::AttributedDecodeFailure {
            id,
            error: error.with_context("Response decode failed"),
        }
    } else {
        warn!(
            ?error,
            response_kind, "Decode error for unattributed response, dropping frame"
        );
        ReceiveDisposition::Ignored {
            reason: IgnoreReason::UnattributedDataReply,
        }
    }
}

fn ignore_unmatched_sequence(
    core: &mut SchedulerCore,
    sequence: Option<u32>,
    cmd_id: Option<CommandId>,
) -> Option<IgnoreReason> {
    let sequence = sequence?;
    if cmd_id.is_some() {
        return None;
    }

    record_unmatched_sequence(core, sequence);
    Some(IgnoreReason::UnmatchedSequence { sequence })
}

fn record_unmatched_sequence(core: &mut SchedulerCore, sequence: u32) {
    debug!(
        sequence,
        "Ignoring unmatched sequenced response without raw VISCA fallback"
    );
    core.record_unmatched_sequenced_reply();
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use std::{
        sync::Arc,
        time::{Duration, Instant},
    };

    use smallvec::SmallVec;

    use super::*;
    use crate::{
        camera::profiles::PtzOpticsG2,
        camera_id::CameraId,
        command::{
            bytes::VISCA_TERMINATOR, encode::EncodedCommand, inquiry_structs::InquiryKind,
            response::Response, CommandBehavior, InquiryData, InquiryResponseSpec,
        },
        runtime::core::{Priority, SchedulerAction},
        timeout::{CommandCategory, TimeoutConfig},
        visca_socket::ViscaSocket,
    };

    fn cmd_id(value: u32) -> CommandId {
        CommandId::from_raw(value).expect("test command ID must be non-zero")
    }

    fn command() -> Arc<EncodedCommand> {
        Arc::new(EncodedCommand {
            payload: SmallVec::from_slice(&[0x81, 0x01, 0x04, 0x00, 0x03, VISCA_TERMINATOR]),
            behavior: CommandBehavior::Command,
            category: CommandCategory::Movement,
        })
    }

    fn inquiry(kind: InquiryKind) -> Arc<EncodedCommand> {
        Arc::new(EncodedCommand {
            payload: SmallVec::from_slice(&[0x81, 0x09, 0x04, 0x00, VISCA_TERMINATOR]),
            behavior: CommandBehavior::Inquiry(InquiryResponseSpec::Builtin(kind)),
            category: CommandCategory::Quick,
        })
    }

    fn raw_inquiry() -> Arc<EncodedCommand> {
        Arc::new(EncodedCommand {
            payload: SmallVec::from_slice(&[0x81, 0x09, 0x7E, 0x55, VISCA_TERMINATOR]),
            behavior: CommandBehavior::Inquiry(InquiryResponseSpec::Raw),
            category: CommandCategory::Quick,
        })
    }

    fn start_inquiry(core: &mut SchedulerCore, id: CommandId, kind: InquiryKind) {
        core.start_inquiry(
            id,
            inquiry(kind),
            Priority::Normal,
            CameraId::CAMERA_1,
            Instant::now(),
        );
    }

    fn start_raw_inquiry(core: &mut SchedulerCore, id: CommandId) {
        core.start_inquiry(
            id,
            raw_inquiry(),
            Priority::Normal,
            CameraId::CAMERA_1,
            Instant::now(),
        );
    }

    fn start_command(core: &mut SchedulerCore, id: CommandId) {
        core.register_pending_ack(
            id,
            command(),
            Priority::Normal,
            CameraId::CAMERA_1,
            Instant::now(),
        );
    }

    fn start_command_for(core: &mut SchedulerCore, id: CommandId, camera: CameraId, at: Instant) {
        core.register_pending_ack(id, command(), Priority::Normal, camera, at);
    }

    fn start_inquiry_for(
        core: &mut SchedulerCore,
        id: CommandId,
        kind: InquiryKind,
        camera: CameraId,
        at: Instant,
    ) {
        core.start_inquiry(id, inquiry(kind), Priority::Normal, camera, at);
    }

    /// Reply address byte for a camera on a serial daisy chain: the device at
    /// address n answers with the high nibble `8 + n`.
    fn z0(camera: CameraId) -> u8 {
        (8 + camera.id()) << 4
    }

    #[track_caller]
    fn expect_event(disposition: ReceiveDisposition) -> SchedulerEvent {
        match disposition {
            ReceiveDisposition::Event(event) => event,
            other => panic!("expected a scheduler event, got {other:?}"),
        }
    }

    /// Feed one raw VISCA frame through the receive path and the scheduler.
    #[track_caller]
    fn deliver(core: &mut SchedulerCore, frame: &[u8], now: Instant) -> Vec<SchedulerAction> {
        let event = expect_event(receive_one::<PtzOpticsG2>(core, frame, None));
        core.process_event(event, now)
    }

    #[track_caller]
    fn completed_ids(actions: &[SchedulerAction]) -> Vec<CommandId> {
        actions
            .iter()
            .filter_map(|action| match action {
                SchedulerAction::CommandComplete { id, .. } => Some(*id),
                _ => None,
            })
            .collect()
    }

    #[test]
    fn ack_becomes_scheduler_event() {
        let mut core = SchedulerCore::new(TimeoutConfig::default());

        let disposition =
            receive_one::<PtzOpticsG2>(&mut core, &[0x90, 0x41, VISCA_TERMINATOR], None);

        match disposition {
            ReceiveDisposition::Event(SchedulerEvent::Ack { source }) => {
                assert!(matches!(
                    source,
                    ReplySource::BySocket {
                        socket: ViscaSocket::S1
                    }
                ));
            }
            other => panic!("expected ACK event, got {other:?}"),
        }
    }

    #[test]
    fn matched_sony_ack_uses_sequence_attribution() {
        let mut core = SchedulerCore::new(TimeoutConfig::default());
        start_command(&mut core, cmd_id(11));
        core.register_sequence(cmd_id(11), 111);

        let disposition =
            receive_one::<PtzOpticsG2>(&mut core, &[0x90, 0x41, VISCA_TERMINATOR], Some(111));

        match disposition {
            ReceiveDisposition::Event(SchedulerEvent::Ack { source }) => {
                assert_eq!(source.cmd_id(), Some(cmd_id(11)));
                assert_eq!(source.socket(), Some(ViscaSocket::S1));
            }
            other => panic!("expected sequenced ACK event, got {other:?}"),
        }
    }

    #[test]
    fn raw_completion_keeps_socket_attribution_for_scheduler() {
        let mut core = SchedulerCore::new(TimeoutConfig::default());

        let disposition =
            receive_one::<PtzOpticsG2>(&mut core, &[0x90, 0x51, VISCA_TERMINATOR], None);

        match disposition {
            ReceiveDisposition::Event(SchedulerEvent::Completion { source, response }) => {
                assert!(matches!(
                    source,
                    ReplySource::BySocket {
                        socket: ViscaSocket::S1
                    }
                ));
                assert!(matches!(
                    response,
                    Response::Completion {
                        socket: Some(ViscaSocket::S1)
                    }
                ));
            }
            other => panic!("expected raw completion event, got {other:?}"),
        }
    }

    #[test]
    fn malformed_payload_is_reported_without_state_mutation() {
        let mut core = SchedulerCore::new(TimeoutConfig::default());
        start_command(&mut core, cmd_id(1));

        let disposition = receive_one::<PtzOpticsG2>(&mut core, &[0x90, 0x41], None);

        assert!(matches!(disposition, ReceiveDisposition::Malformed(_)));
        assert!(core.is_command_pending(cmd_id(1)));
    }

    #[test]
    fn network_change_and_unknown_are_ignored() {
        let mut core = SchedulerCore::new(TimeoutConfig::default());

        let network = receive_one::<PtzOpticsG2>(&mut core, &[0x90, 0x38, VISCA_TERMINATOR], None);
        assert!(matches!(
            network,
            ReceiveDisposition::Ignored {
                reason: IgnoreReason::NetworkChange
            }
        ));

        let unknown = receive_one::<PtzOpticsG2>(&mut core, &[0x90, 0x70, VISCA_TERMINATOR], None);
        assert!(matches!(
            unknown,
            ReceiveDisposition::Ignored {
                reason: IgnoreReason::UnknownResponse
            }
        ));
    }

    #[test]
    fn unmatched_sony_ack_completion_and_error_are_ignored() {
        let mut core = SchedulerCore::new(TimeoutConfig::default());
        start_command(&mut core, cmd_id(1));

        let cases: &[(&[u8], u32)] = &[
            (&[0x90, 0x41, VISCA_TERMINATOR], 901),
            (&[0x90, 0x51, VISCA_TERMINATOR], 902),
            (&[0x90, 0x61, 0x41, VISCA_TERMINATOR], 903),
        ];

        for &(payload, sequence) in cases {
            let before = core.ignored_unmatched_sequenced_replies();
            let disposition = receive_one::<PtzOpticsG2>(&mut core, payload, Some(sequence));

            assert!(matches!(
                disposition,
                ReceiveDisposition::Ignored {
                    reason: IgnoreReason::UnmatchedSequence { sequence: got }
                } if got == sequence
            ));
            assert_eq!(core.ignored_unmatched_sequenced_replies(), before + 1);
            assert!(core.is_command_pending(cmd_id(1)));
        }
    }

    #[test]
    fn sequenced_data_reply_for_active_non_inquiry_is_ignored() {
        let mut core = SchedulerCore::new(TimeoutConfig::default());
        start_command(&mut core, cmd_id(4));
        core.register_sequence(cmd_id(4), 404);

        let before = core.ignored_unmatched_sequenced_replies();
        let disposition =
            receive_one::<PtzOpticsG2>(&mut core, &[0x90, 0x50, 0x02, VISCA_TERMINATOR], Some(404));

        assert!(matches!(
            disposition,
            ReceiveDisposition::Ignored {
                reason: IgnoreReason::UnexpectedDataReply { id }
            } if id == cmd_id(4)
        ));
        assert_eq!(core.ignored_unmatched_sequenced_replies(), before);
        assert!(core.is_command_pending(cmd_id(4)));
    }

    #[test]
    fn matched_sony_data_reply_uses_expected_inquiry_type() {
        let mut core = SchedulerCore::new(TimeoutConfig::default());
        start_inquiry(&mut core, cmd_id(7), InquiryKind::Power);
        core.register_sequence(cmd_id(7), 100);

        let disposition =
            receive_one::<PtzOpticsG2>(&mut core, &[0x90, 0x50, 0x02, VISCA_TERMINATOR], Some(100));

        match disposition {
            ReceiveDisposition::Event(SchedulerEvent::InquiryReply { source, response }) => {
                assert_eq!(source.cmd_id(), Some(cmd_id(7)));
                assert!(matches!(
                    response,
                    Response::Inquiry(InquiryData::Power { on: true })
                ));
            }
            other => panic!("expected inquiry event, got {other:?}"),
        }
    }

    #[test]
    fn matched_sony_raw_data_reply_uses_sequence_attribution() {
        let mut core = SchedulerCore::new(TimeoutConfig::default());
        start_raw_inquiry(&mut core, cmd_id(8));
        core.register_sequence(cmd_id(8), 808);

        let disposition = receive_one::<PtzOpticsG2>(
            &mut core,
            &[0x90, 0x50, 0x7A, 0x7B, VISCA_TERMINATOR],
            Some(808),
        );

        match disposition {
            ReceiveDisposition::Event(SchedulerEvent::InquiryReply { source, response }) => {
                assert_eq!(source.cmd_id(), Some(cmd_id(8)));
                match response {
                    Response::RawInquiry(payload) => assert_eq!(payload.as_slice(), &[0x7A, 0x7B]),
                    other => panic!("expected raw inquiry response, got {other:?}"),
                }
            }
            other => panic!("expected raw inquiry event, got {other:?}"),
        }
    }

    #[test]
    fn stale_sony_data_reply_does_not_consume_raw_inquiry() {
        let mut core = SchedulerCore::new(TimeoutConfig::default());
        let now = Instant::now();
        start_inquiry(&mut core, cmd_id(1), InquiryKind::Power);

        let before = core.ignored_unmatched_sequenced_replies();
        let stale =
            receive_one::<PtzOpticsG2>(&mut core, &[0x90, 0x50, 0x02, VISCA_TERMINATOR], Some(999));

        assert!(matches!(
            stale,
            ReceiveDisposition::Ignored {
                reason: IgnoreReason::UnmatchedSequence { sequence: 999 }
            }
        ));
        assert_eq!(core.ignored_unmatched_sequenced_replies(), before + 1);
        assert!(core.is_command_pending(cmd_id(1)));

        let valid =
            receive_one::<PtzOpticsG2>(&mut core, &[0x90, 0x50, 0x02, VISCA_TERMINATOR], None);
        let ReceiveDisposition::Event(event) = valid else {
            panic!("expected valid raw inquiry event, got {valid:?}");
        };
        let actions = core.process_event(event, now);
        assert!(actions.iter().any(|action| {
            matches!(
                action,
                SchedulerAction::CommandComplete { id, .. } if *id == cmd_id(1)
            )
        }));
    }

    #[test]
    fn stale_sony_raw_data_reply_does_not_consume_raw_fifo() {
        let mut core = SchedulerCore::new(TimeoutConfig::default());
        let now = Instant::now();
        start_raw_inquiry(&mut core, cmd_id(1));

        let stale =
            receive_one::<PtzOpticsG2>(&mut core, &[0x90, 0x50, 0x7A, VISCA_TERMINATOR], Some(999));

        assert!(matches!(
            stale,
            ReceiveDisposition::Ignored {
                reason: IgnoreReason::UnmatchedSequence { sequence: 999 }
            }
        ));
        assert!(core.is_command_pending(cmd_id(1)));

        let valid =
            receive_one::<PtzOpticsG2>(&mut core, &[0x90, 0x50, 0x7B, VISCA_TERMINATOR], None);
        let ReceiveDisposition::Event(event) = valid else {
            panic!("expected valid raw inquiry event, got {valid:?}");
        };
        let actions = core.process_event(event, now);
        assert!(actions.iter().any(|action| {
            matches!(
                action,
                SchedulerAction::CommandComplete {
                    id,
                    response: Response::RawInquiry(payload),
                    ..
                } if *id == cmd_id(1) && payload.as_slice() == [0x7B]
            )
        }));
    }

    #[test]
    fn stale_sony_malformed_data_reply_does_not_decode_against_raw_inquiry() {
        let mut core = SchedulerCore::new(TimeoutConfig::default());
        start_inquiry(&mut core, cmd_id(1), InquiryKind::Power);

        let before = core.ignored_unmatched_sequenced_replies();
        let stale = receive_one::<PtzOpticsG2>(
            &mut core,
            &[0x90, 0x50, 0x01, 0x02, 0x03, VISCA_TERMINATOR],
            Some(999),
        );

        assert!(matches!(
            stale,
            ReceiveDisposition::Ignored {
                reason: IgnoreReason::UnmatchedSequence { sequence: 999 }
            }
        ));
        assert_eq!(core.ignored_unmatched_sequenced_replies(), before + 1);
        assert!(core.is_command_pending(cmd_id(1)));
    }

    #[test]
    fn raw_data_reply_decode_failure_is_attributed() {
        let mut core = SchedulerCore::new(TimeoutConfig::default());
        start_inquiry(&mut core, cmd_id(9), InquiryKind::Power);

        let disposition = receive_one::<PtzOpticsG2>(
            &mut core,
            &[0x90, 0x50, 0x01, 0x02, 0x03, VISCA_TERMINATOR],
            None,
        );

        match disposition {
            ReceiveDisposition::AttributedDecodeFailure { id, error } => {
                assert_eq!(id, cmd_id(9));
                assert!(error.to_string().contains("Response decode failed"));
            }
            other => panic!("expected attributed decode failure, got {other:?}"),
        }
    }

    #[test]
    fn raw_data_reply_content_match_can_beat_fifo_order() {
        let mut core = SchedulerCore::new(TimeoutConfig::default());
        start_inquiry(&mut core, cmd_id(1), InquiryKind::ZoomPosition);
        start_inquiry(&mut core, cmd_id(2), InquiryKind::Power);

        let disposition =
            receive_one::<PtzOpticsG2>(&mut core, &[0x90, 0x50, 0x02, VISCA_TERMINATOR], None);

        match disposition {
            ReceiveDisposition::Event(SchedulerEvent::InquiryReply { source, response }) => {
                assert_eq!(source.cmd_id(), Some(cmd_id(2)));
                assert!(matches!(
                    response,
                    Response::Inquiry(InquiryData::Power { on: true })
                ));
            }
            other => panic!("expected content-matched inquiry event, got {other:?}"),
        }
    }

    #[test]
    fn raw_data_reply_builtin_match_skips_raw_inquiry_before_fifo() {
        let mut core = SchedulerCore::new(TimeoutConfig::default());
        start_raw_inquiry(&mut core, cmd_id(1));
        start_inquiry(&mut core, cmd_id(2), InquiryKind::Power);

        let disposition =
            receive_one::<PtzOpticsG2>(&mut core, &[0x90, 0x50, 0x02, VISCA_TERMINATOR], None);

        match disposition {
            ReceiveDisposition::Event(SchedulerEvent::InquiryReply { source, response }) => {
                assert_eq!(source.cmd_id(), Some(cmd_id(2)));
                assert!(matches!(
                    response,
                    Response::Inquiry(InquiryData::Power { on: true })
                ));
            }
            other => panic!("expected built-in content-matched inquiry event, got {other:?}"),
        }
    }

    #[test]
    fn raw_data_reply_for_raw_inquiry_uses_fifo_when_unsequenced() {
        let mut core = SchedulerCore::new(TimeoutConfig::default());
        start_raw_inquiry(&mut core, cmd_id(5));

        let disposition = receive_one::<PtzOpticsG2>(
            &mut core,
            &[0x90, 0x50, 0x33, 0x44, VISCA_TERMINATOR],
            None,
        );

        match disposition {
            ReceiveDisposition::Event(SchedulerEvent::InquiryReply { source, response }) => {
                assert_eq!(source.cmd_id(), Some(cmd_id(5)));
                match response {
                    Response::RawInquiry(payload) => assert_eq!(payload.as_slice(), &[0x33, 0x44]),
                    other => panic!("expected raw inquiry response, got {other:?}"),
                }
            }
            other => panic!("expected FIFO-attributed raw inquiry event, got {other:?}"),
        }
    }

    #[test]
    fn raw_data_reply_without_owner_is_ignored() {
        let mut core = SchedulerCore::new(TimeoutConfig::default());

        let disposition =
            receive_one::<PtzOpticsG2>(&mut core, &[0x90, 0x50, 0x02, VISCA_TERMINATOR], None);

        assert!(matches!(
            disposition,
            ReceiveDisposition::Ignored {
                reason: IgnoreReason::UnattributedDataReply
            }
        ));
    }

    #[test]
    fn raw_error_without_socket_uses_inquiry_fifo() {
        let mut core = SchedulerCore::new(TimeoutConfig::default());
        start_inquiry(&mut core, cmd_id(3), InquiryKind::Power);

        let disposition =
            receive_one::<PtzOpticsG2>(&mut core, &[0x90, 0x60, 0x41, VISCA_TERMINATOR], None);

        match disposition {
            ReceiveDisposition::Event(SchedulerEvent::Error { source, code }) => {
                assert_eq!(code, 0x41);
                assert_eq!(source.cmd_id(), Some(cmd_id(3)));
            }
            other => panic!("expected error event, got {other:?}"),
        }
    }

    // -----------------------------------------------------------------------
    // Serial daisy chain: replies carry the sending camera's address
    // -----------------------------------------------------------------------

    /// Every device on a chain answers with its own address, and each reply
    /// must drive that camera's own command through ACK and completion.
    ///
    /// Before source-address decoding, only camera 1's `0x90` replies parsed;
    /// `0xA0`-`0xF0` were rejected as malformed and those commands could only
    /// time out.
    #[test]
    fn chain_replies_drive_each_cameras_own_command() {
        let mut core = SchedulerCore::new(TimeoutConfig::default());
        let now = Instant::now();
        let chain = [
            (cmd_id(1), CameraId::CAMERA_1),
            (cmd_id(2), CameraId::CAMERA_2),
            (cmd_id(3), CameraId::CAMERA_3),
        ];

        for (offset, &(id, camera)) in chain.iter().enumerate() {
            start_command_for(
                &mut core,
                id,
                camera,
                now + Duration::from_millis(offset as u64),
            );
        }

        // ACKs come back in reverse order; each camera allocates its own S1.
        for &(id, camera) in chain.iter().rev() {
            let actions = deliver(&mut core, &[z0(camera), 0x41, VISCA_TERMINATOR], now);
            assert!(actions.is_empty(), "an ACK alone completes nothing");
            let (free, holder, _) = core.socket_state_for(camera, ViscaSocket::S1);
            assert!(!free, "{camera} should hold its own S1");
            assert_eq!(holder, Some(id), "{camera} S1 must hold its own command");
        }

        // Completions interleave; each finishes only its own camera's command.
        for &(id, camera) in &[chain[1], chain[2], chain[0]] {
            let actions = deliver(&mut core, &[z0(camera), 0x51, VISCA_TERMINATOR], now);
            assert_eq!(
                completed_ids(&actions),
                vec![id],
                "{camera}'s completion must finish {id} and nothing else"
            );
        }

        for &(id, _) in &chain {
            assert!(!core.is_command_pending(id), "{id} should be finished");
        }
    }

    /// An error frame from one camera fails that camera's command only.
    #[test]
    fn chain_error_fails_only_the_sending_cameras_command() {
        let mut core = SchedulerCore::new(TimeoutConfig::default());
        let now = Instant::now();
        let chain = [
            (cmd_id(1), CameraId::CAMERA_1),
            (cmd_id(2), CameraId::CAMERA_2),
            (cmd_id(3), CameraId::CAMERA_3),
        ];

        for (offset, &(id, camera)) in chain.iter().enumerate() {
            start_command_for(
                &mut core,
                id,
                camera,
                now + Duration::from_millis(offset as u64),
            );
            deliver(&mut core, &[z0(camera), 0x41, VISCA_TERMINATOR], now);
        }

        // Camera 2 rejects its command with a terminal syntax error.
        let actions = deliver(
            &mut core,
            &[z0(CameraId::CAMERA_2), 0x61, 0x02, VISCA_TERMINATOR],
            now,
        );
        let failed: Vec<_> = actions
            .iter()
            .filter_map(|action| match action {
                SchedulerAction::CommandFailed { id, .. } => Some(*id),
                _ => None,
            })
            .collect();
        assert_eq!(failed, vec![cmd_id(2)], "only camera 2's command fails");

        assert!(core.is_command_pending(cmd_id(1)), "camera 1 is untouched");
        assert!(core.is_command_pending(cmd_id(3)), "camera 3 is untouched");

        // The neighbours still complete normally afterwards.
        let actions = deliver(
            &mut core,
            &[z0(CameraId::CAMERA_3), 0x51, VISCA_TERMINATOR],
            now,
        );
        assert_eq!(completed_ids(&actions), vec![cmd_id(3)]);
    }

    /// A data reply resolves against the sending camera's inquiry, even when
    /// another camera has an identical inquiry outstanding.
    #[test]
    fn chain_data_reply_resolves_the_sending_cameras_inquiry() {
        let mut core = SchedulerCore::new(TimeoutConfig::default());
        let now = Instant::now();

        // Camera 1 asked first, so it owns the head of the FIFO order queue.
        start_inquiry_for(
            &mut core,
            cmd_id(1),
            InquiryKind::Power,
            CameraId::CAMERA_1,
            now,
        );
        start_inquiry_for(
            &mut core,
            cmd_id(2),
            InquiryKind::Power,
            CameraId::CAMERA_2,
            now + Duration::from_millis(1),
        );

        let actions = deliver(
            &mut core,
            &[z0(CameraId::CAMERA_2), 0x50, 0x02, VISCA_TERMINATOR],
            now,
        );
        assert_eq!(
            completed_ids(&actions),
            vec![cmd_id(2)],
            "camera 2's reply must not be charged to camera 1's identical inquiry"
        );
        assert!(core.is_command_pending(cmd_id(1)), "camera 1 still waiting");

        let actions = deliver(
            &mut core,
            &[z0(CameraId::CAMERA_1), 0x50, 0x02, VISCA_TERMINATOR],
            now,
        );
        assert_eq!(completed_ids(&actions), vec![cmd_id(1)]);
    }

    /// A reply from a camera with nothing in flight is ignored rather than
    /// charged to a neighbour, which is the existing policy for a syntactically
    /// valid frame that cannot be attributed.
    #[test]
    fn chain_reply_from_idle_camera_is_ignored() {
        let mut core = SchedulerCore::new(TimeoutConfig::default());
        let now = Instant::now();

        start_command_for(&mut core, cmd_id(1), CameraId::CAMERA_1, now);
        start_command_for(
            &mut core,
            cmd_id(2),
            CameraId::CAMERA_2,
            now + Duration::from_millis(1),
        );
        deliver(
            &mut core,
            &[z0(CameraId::CAMERA_1), 0x41, VISCA_TERMINATOR],
            now,
        );
        deliver(
            &mut core,
            &[z0(CameraId::CAMERA_2), 0x41, VISCA_TERMINATOR],
            now,
        );

        // Camera 5 is on the bus but has nothing outstanding.
        let idle = CameraId::CAMERA_5;
        for frame in [
            vec![z0(idle), 0x41, VISCA_TERMINATOR],
            vec![z0(idle), 0x51, VISCA_TERMINATOR],
            vec![z0(idle), 0x61, 0x02, VISCA_TERMINATOR],
            vec![z0(idle), 0x50, 0x02, VISCA_TERMINATOR],
        ] {
            let disposition = receive_one::<PtzOpticsG2>(&mut core, &frame, None);
            match disposition {
                ReceiveDisposition::Ignored {
                    reason: IgnoreReason::UnownedSourceReply { camera },
                } => assert_eq!(camera, idle),
                ReceiveDisposition::Ignored {
                    reason: IgnoreReason::UnattributedDataReply,
                } => {}
                other => panic!("expected an ignored frame for {frame:02X?}, got {other:?}"),
            }
        }

        assert!(core.is_command_pending(cmd_id(1)), "camera 1 untouched");
        assert!(core.is_command_pending(cmd_id(2)), "camera 2 untouched");
        let (free, holder, _) = core.socket_state_for(CameraId::CAMERA_1, ViscaSocket::S1);
        assert!(!free);
        assert_eq!(holder, Some(cmd_id(1)));
        let (free, holder, _) = core.socket_state_for(CameraId::CAMERA_2, ViscaSocket::S1);
        assert!(!free);
        assert_eq!(holder, Some(cmd_id(2)));
    }

    /// With one camera on the transport the reply address is not
    /// discriminating: IP cameras answer `0x90` whatever address they were
    /// configured with, so attribution stays exactly as it was in 1.x.
    #[test]
    fn single_camera_keeps_camera_blind_attribution() {
        let mut core = SchedulerCore::new(TimeoutConfig::default());
        let now = Instant::now();

        // The session addresses this camera as 2, but it replies as camera 1.
        start_command_for(&mut core, cmd_id(1), CameraId::CAMERA_2, now);

        deliver(&mut core, &[0x90, 0x41, VISCA_TERMINATOR], now);
        let (free, holder, _) = core.socket_state_for(CameraId::CAMERA_2, ViscaSocket::S1);
        assert!(
            !free,
            "the ACK still assigns the configured camera's socket"
        );
        assert_eq!(holder, Some(cmd_id(1)));

        let actions = deliver(&mut core, &[0x90, 0x51, VISCA_TERMINATOR], now);
        assert_eq!(completed_ids(&actions), vec![cmd_id(1)]);
    }

    /// Lead bytes that are not reply addresses stay malformed, and a chain
    /// address does not make a truncated frame decodable.
    #[test]
    fn non_reply_addresses_and_truncated_frames_are_still_malformed() {
        let mut core = SchedulerCore::new(TimeoutConfig::default());
        start_command(&mut core, cmd_id(1));

        let malformed: &[&[u8]] = &[
            &[0x80, 0x41, VISCA_TERMINATOR],       // controller address
            &[0x88, 0x30, 0x02, VISCA_TERMINATOR], // address-set reply
            &[0x81, 0x41, VISCA_TERMINATOR],       // command address, not a reply
            &[0x8F, 0x41, VISCA_TERMINATOR],
            &[0xA0, 0x41],                   // no terminator
            &[0xF0, 0x41],                   // no terminator
            &[0xA0, VISCA_TERMINATOR],       // too short
            &[0xF0, 0x61, VISCA_TERMINATOR], // error frame without its code
        ];

        for frame in malformed {
            assert!(
                matches!(
                    receive_one::<PtzOpticsG2>(&mut core, frame, None),
                    ReceiveDisposition::Malformed(_)
                ),
                "{frame:02X?} must stay malformed"
            );
        }
        assert!(core.is_command_pending(cmd_id(1)), "no state was disturbed");
    }

    /// Network-change notifications are ignored from every chain address.
    #[test]
    fn network_change_from_any_chain_address_is_ignored() {
        let mut core = SchedulerCore::new(TimeoutConfig::default());
        start_command(&mut core, cmd_id(1));

        for id in 1..=7u8 {
            let camera = CameraId::new(id).expect("1-7 are valid camera IDs");
            let frame = [z0(camera), 0x38, VISCA_TERMINATOR];
            assert!(
                matches!(
                    receive_one::<PtzOpticsG2>(&mut core, &frame, None),
                    ReceiveDisposition::Ignored {
                        reason: IgnoreReason::NetworkChange
                    }
                ),
                "network change from {camera} must be ignored"
            );
        }
        assert!(core.is_command_pending(cmd_id(1)));
    }

    /// A Sony sequence number stays authoritative: the VISCA reply address must
    /// never override it, and an unmatched sequence is still ignored.
    #[test]
    fn sequenced_replies_ignore_the_reply_address() {
        let mut core = SchedulerCore::new(TimeoutConfig::default());
        let now = Instant::now();
        start_command_for(&mut core, cmd_id(1), CameraId::CAMERA_1, now);
        start_command_for(
            &mut core,
            cmd_id(2),
            CameraId::CAMERA_2,
            now + Duration::from_millis(1),
        );
        core.register_sequence(cmd_id(1), 700);

        // Camera 2's address on a frame sequenced for camera 1's command:
        // the sequence wins.
        match receive_one::<PtzOpticsG2>(
            &mut core,
            &[z0(CameraId::CAMERA_2), 0x41, VISCA_TERMINATOR],
            Some(700),
        ) {
            ReceiveDisposition::Event(SchedulerEvent::Ack { source }) => {
                assert_eq!(source.cmd_id(), Some(cmd_id(1)));
            }
            other => panic!("expected a sequence-attributed ACK, got {other:?}"),
        }

        // An unmatched sequence is still dropped rather than falling back to
        // the reply address.
        let before = core.ignored_unmatched_sequenced_replies();
        let disposition = receive_one::<PtzOpticsG2>(
            &mut core,
            &[z0(CameraId::CAMERA_2), 0x41, VISCA_TERMINATOR],
            Some(999),
        );
        assert!(matches!(
            disposition,
            ReceiveDisposition::Ignored {
                reason: IgnoreReason::UnmatchedSequence { sequence: 999 }
            }
        ));
        assert_eq!(core.ignored_unmatched_sequenced_replies(), before + 1);
    }
}
