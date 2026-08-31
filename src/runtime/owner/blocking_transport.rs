//! Blocking production transport adapter for the Phase-6 owner.
//!
//! A blocking owner has separate write, read, and decode trait seams so the
//! owner can keep its mutable engine borrow short.  The three handles below
//! all point at one `BlockingTransportAdapter` state and therefore retain one
//! physical transport reader/writer, one envelope, and one protocol framer.

use std::{
    num::NonZeroUsize,
    sync::{Arc, Mutex, MutexGuard},
    time::Instant,
};

use crate::{
    profile::OperationalTuning,
    profile::ProfileSpec,
    protocol::framer::ProtocolFramer,
    runtime::engine::TransmissionMeta,
    transport::envelope::FrameSequence,
    transport::{builder::TransportConfig, BlockingTransport, HasTransportConfig},
    CameraId, Error,
};

use super::{
    adapter::{
        decode_frames_with_routing, owner_policy_for_targets_with_tuning,
        validate_profile_transport, OwnerEnvelope, RoutingState, TargetRegistry,
    },
    BlockingFrameDecoder, BlockingReadDriver, BlockingReceive, BlockingWireDriver, OwnerBuffers,
    OwnerPolicy, WireWrite,
};

#[derive(Debug)]
struct BlockingAdapterState<T> {
    transport: T,
    config: TransportConfig,
    envelope: OwnerEnvelope,
    framer: ProtocolFramer,
    routing: RoutingState,
}

/// One production blocking transport plus its owner-side envelope/framer.
#[derive(Debug)]
pub(crate) struct BlockingTransportAdapter<T> {
    state: Arc<Mutex<BlockingAdapterState<T>>>,
    policy: OwnerPolicy,
}

/// Writer view into a [`BlockingTransportAdapter`].
#[derive(Debug, Clone)]
pub(crate) struct BlockingTransportWriter<T> {
    state: Arc<Mutex<BlockingAdapterState<T>>>,
}

/// Reader view into a [`BlockingTransportAdapter`].
#[derive(Debug, Clone)]
pub(crate) struct BlockingTransportReader<T> {
    state: Arc<Mutex<BlockingAdapterState<T>>>,
}

/// Decoder view into a [`BlockingTransportAdapter`].
#[derive(Debug, Clone)]
pub(crate) struct BlockingTransportDecoder<T> {
    state: Arc<Mutex<BlockingAdapterState<T>>>,
}

impl<T> BlockingTransportAdapter<T>
where
    T: BlockingTransport + HasTransportConfig,
{
    /// Build an owner adapter from validated profile facts and the transport's
    /// immutable configuration.  No transport operation occurs here.
    // Test-only single-target convenience; production uses `new_with_targets`.
    #[cfg(test)]
    pub(crate) fn new(
        transport: T,
        profile: &ProfileSpec,
        target: CameraId,
    ) -> Result<Self, Error> {
        Self::new_with_tuning(transport, profile, target, OperationalTuning::new())
    }

    // Test-only single-target convenience; production uses `new_with_targets`.
    #[cfg(test)]
    pub(crate) fn new_with_tuning(
        transport: T,
        profile: &ProfileSpec,
        target: CameraId,
        tuning: OperationalTuning,
    ) -> Result<Self, Error> {
        Self::new_with_targets(
            transport,
            &[(target, profile)],
            tuning,
            crate::DEFAULT_ADMISSION_CAPACITY,
        )
    }

    /// Build an adapter for several immutable target/profile pairs on one
    /// physical transport.
    pub(crate) fn new_with_targets(
        transport: T,
        profiles: &[(CameraId, &ProfileSpec)],
        tuning: OperationalTuning,
        admission_capacity: NonZeroUsize,
    ) -> Result<Self, Error> {
        // Keep the blocking startup boundary identical to async: reject a
        // known standard transport before reading startup configuration or
        // constructing the owner policy. Custom transports (which report
        // `None`) remain an explicit profile-compatibility escape hatch.
        for (_, profile) in profiles {
            validate_profile_transport(profile, transport.standard_transport_kind())?;
        }
        let standard_kind = transport.standard_transport_kind();
        // Multi-target routing must be explicitly proven by a side-effect-free
        // transport hint. This runs before the first config read, actor spawn,
        // or transport operation; custom transports default to `None` and are
        // therefore rejected unless they opt into serial addressing.
        super::adapter::validate_profile_registry_topology(
            profiles,
            standard_kind,
            transport.addressing_mode_hint(),
        )?;
        let config = *transport.transport_config();
        super::adapter::validate_profile_registry_topology(
            profiles,
            standard_kind,
            Some(config.addressing),
        )?;
        let policy = owner_policy_for_targets_with_tuning(
            profiles,
            &config,
            transport.send_semantics(),
            tuning,
            admission_capacity,
        )?;
        let targets: Vec<_> = profiles.iter().map(|(target, _)| *target).collect();
        let registry = TargetRegistry::from_targets(&targets)?;
        let profile = profiles[0].1;
        let envelope = OwnerEnvelope::from_profile(profile, config.addressing)?;
        let framer =
            ProtocolFramer::new_with_config_and_mode(config.buffer_config, envelope.framing_mode());
        let routing = RoutingState::new(config.addressing, registry);
        Ok(Self {
            state: Arc::new(Mutex::new(BlockingAdapterState {
                transport,
                config,
                envelope,
                framer,
                routing,
            })),
            policy,
        })
    }

    /// Alias emphasizing that target/profile registration is immutable.
    pub(crate) fn new_with_profile_registry(
        transport: T,
        profiles: &[(CameraId, &ProfileSpec)],
        tuning: OperationalTuning,
        admission_capacity: NonZeroUsize,
    ) -> Result<Self, Error> {
        Self::new_with_targets(transport, profiles, tuning, admission_capacity)
    }

    pub(crate) fn policy(&self) -> &OwnerPolicy {
        &self.policy
    }

    /// Return write/read/decode views backed by this adapter's one transport.
    pub(crate) fn parts(
        &self,
    ) -> (
        BlockingTransportWriter<T>,
        BlockingTransportReader<T>,
        BlockingTransportDecoder<T>,
    ) {
        (
            BlockingTransportWriter {
                state: Arc::clone(&self.state),
            },
            BlockingTransportReader {
                state: Arc::clone(&self.state),
            },
            BlockingTransportDecoder {
                state: Arc::clone(&self.state),
            },
        )
    }

    /// Consume the adapter and return write/read/decode views.  The returned
    /// views still share exactly one underlying transport state.
    pub(crate) fn into_parts(
        self,
    ) -> (
        BlockingTransportWriter<T>,
        BlockingTransportReader<T>,
        BlockingTransportDecoder<T>,
    ) {
        let parts = self.parts();
        drop(self);
        parts
    }
}

impl<T> BlockingWireDriver for BlockingTransportAdapter<T>
where
    T: BlockingTransport + HasTransportConfig,
{
    fn write(&mut self, write: WireWrite<'_>) -> Result<TransmissionMeta, Error> {
        write_state(&self.state, write)
    }
}

impl<T> BlockingReadDriver for BlockingTransportAdapter<T>
where
    T: BlockingTransport + HasTransportConfig,
{
    fn receive(
        &mut self,
        receive_buffer: &mut [u8],
        owner_deadline: Option<Instant>,
    ) -> Result<BlockingReceive, Error> {
        receive_state(&self.state, receive_buffer, owner_deadline)
    }
}

impl<T> BlockingFrameDecoder for BlockingTransportAdapter<T>
where
    T: BlockingTransport + HasTransportConfig,
{
    fn decode(
        &mut self,
        buffers: &mut OwnerBuffers,
        received: usize,
        frame_limit: usize,
    ) -> Result<Vec<crate::runtime::engine::DecodedFrame>, Error> {
        decode_state(&self.state, buffers, received, frame_limit)
    }
}

impl<T> BlockingWireDriver for BlockingTransportWriter<T>
where
    T: BlockingTransport + HasTransportConfig,
{
    fn write(&mut self, write: WireWrite<'_>) -> Result<TransmissionMeta, Error> {
        write_state(&self.state, write)
    }
}

impl<T> BlockingReadDriver for BlockingTransportReader<T>
where
    T: BlockingTransport + HasTransportConfig,
{
    fn receive(
        &mut self,
        receive_buffer: &mut [u8],
        owner_deadline: Option<Instant>,
    ) -> Result<BlockingReceive, Error> {
        receive_state(&self.state, receive_buffer, owner_deadline)
    }
}

impl<T> BlockingFrameDecoder for BlockingTransportDecoder<T>
where
    T: BlockingTransport + HasTransportConfig,
{
    fn decode(
        &mut self,
        buffers: &mut OwnerBuffers,
        received: usize,
        frame_limit: usize,
    ) -> Result<Vec<crate::runtime::engine::DecodedFrame>, Error> {
        decode_state(&self.state, buffers, received, frame_limit)
    }
}

fn lock_state<'a, T>(
    state: &'a Arc<Mutex<BlockingAdapterState<T>>>,
) -> Result<MutexGuard<'a, BlockingAdapterState<T>>, Error> {
    state
        .lock()
        .map_err(|_| Error::TransportError("blocking owner transport state mutex poisoned".into()))
}

fn write_state<T>(
    state: &Arc<Mutex<BlockingAdapterState<T>>>,
    write: WireWrite<'_>,
) -> Result<TransmissionMeta, Error>
where
    T: BlockingTransport + HasTransportConfig,
{
    let mut state = lock_state(state)?;
    if write.envelope != state.envelope.kind() {
        return Err(Error::InvalidState(
            "owner write envelope does not match transport adapter".into(),
        ));
    }
    let kind = if write.inquiry {
        crate::command::CommandKind::Inquiry
    } else {
        crate::command::CommandKind::Command
    };
    let datagram = matches!(
        state.transport.send_semantics(),
        crate::transport::SendSemantics::Datagram
    );
    let frame_meta = state.envelope.frame_into_with_sequence(
        write.bytes,
        kind,
        write.requested_sequence,
        write.frame_buffer,
    )?;
    state
        .transport
        .send_with_kind(write.frame_buffer.as_ref(), kind)
        .map_err(|error| {
            if datagram {
                super::normalize_datagram_send_error(error)
            } else {
                error
            }
        })?;
    Ok(TransmissionMeta {
        // Outgoing framing always returns Full32 metadata. Convert only at
        // this transport/engine boundary; receive-side provenance remains
        // typed on FrameMeta until adapter decoding constructs EnvelopeSequence.
        sequence: frame_meta.sequence.map(FrameSequence::value),
    })
}

fn receive_state<T>(
    state: &Arc<Mutex<BlockingAdapterState<T>>>,
    receive_buffer: &mut [u8],
    owner_deadline: Option<Instant>,
) -> Result<BlockingReceive, Error>
where
    T: BlockingTransport + HasTransportConfig,
{
    let mut state = lock_state(state)?;
    let timeout = match owner_deadline {
        Some(deadline) => {
            let now = Instant::now();
            if deadline <= now {
                return Ok(BlockingReceive::TimedOut);
            }
            state.config.read_timeout.min(deadline - now)
        }
        None => state.config.read_timeout,
    };
    match state
        .transport
        .recv_into_with_timeout(receive_buffer, timeout)
    {
        Ok(received) => Ok(BlockingReceive::Bytes(received)),
        // An expired idle read timeout is no data, not a failed read. The
        // raw `WouldBlock`/`Interrupted`/`TimedOut` spellings a custom
        // transport may forward mean the same thing (#637).
        Err(error) if super::receive_reported_no_data(&error) => Ok(BlockingReceive::TimedOut),
        Err(error) => Err(error),
    }
}

fn decode_state<T>(
    state: &Arc<Mutex<BlockingAdapterState<T>>>,
    buffers: &mut OwnerBuffers,
    received: usize,
    frame_limit: usize,
) -> Result<Vec<crate::runtime::engine::DecodedFrame>, Error>
where
    T: BlockingTransport + HasTransportConfig,
{
    let mut state = lock_state(state)?;
    let transport = match state.transport.send_semantics() {
        crate::transport::SendSemantics::Datagram => {
            crate::runtime::engine::TransportKind::Datagram
        }
        crate::transport::SendSemantics::Stream => crate::runtime::engine::TransportKind::Stream,
    };
    let BlockingAdapterState {
        envelope,
        framer,
        routing,
        ..
    } = &mut *state;
    decode_frames_with_routing(
        envelope,
        framer,
        *routing,
        buffers,
        received,
        frame_limit,
        transport,
    )
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used, unused_qualifications)]
mod tests {
    use std::{
        collections::VecDeque,
        sync::{Arc, Mutex},
        time::Duration,
    };

    use super::*;
    use crate::{
        command::CommandKind,
        prepared::{prepare_command, ClassSelection},
        profile::ProfileSpec,
        profiles::{GenericVisca, SonyFR7},
        runtime::engine::{
            CancellationObservation, CancellationPolicy, ControlPolicy, DecodedResponse,
            EncodedMessage, EnvelopeSequence, ReplyShape, RequestContext, RetryPolicy,
            RuntimeOutcome, RuntimeRequest, SequenceWidth, TimeoutPolicy,
        },
        transport::{
            builder::{AddressingMode, TransportConfig},
            SendSemantics,
        },
    };

    #[derive(Debug)]
    struct ScriptedTransport {
        config: TransportConfig,
        sent: Arc<Mutex<Vec<Vec<u8>>>>,
        receives: VecDeque<Result<Vec<u8>, Error>>,
        semantics: SendSemantics,
    }

    impl ScriptedTransport {
        fn new(
            config: TransportConfig,
            receives: impl IntoIterator<Item = Result<Vec<u8>, Error>>,
        ) -> Self {
            Self {
                config,
                sent: Arc::new(Mutex::new(Vec::new())),
                receives: receives.into_iter().collect(),
                semantics: SendSemantics::Datagram,
            }
        }
    }

    impl HasTransportConfig for ScriptedTransport {
        fn transport_config(&self) -> &TransportConfig {
            &self.config
        }
    }

    impl BlockingTransport for ScriptedTransport {
        fn send_with_kind(&mut self, bytes: &[u8], _kind: CommandKind) -> Result<(), Error> {
            self.sent.lock().unwrap().push(bytes.to_vec());
            Ok(())
        }

        fn recv_into(&mut self, dst: &mut [u8]) -> Result<usize, Error> {
            self.recv_into_with_timeout(dst, Duration::from_secs(1))
        }

        fn recv_into_with_timeout(
            &mut self,
            dst: &mut [u8],
            _timeout: Duration,
        ) -> Result<usize, Error> {
            let next = self.receives.pop_front().unwrap_or(Ok(Vec::new()))?;
            let n = next.len().min(dst.len());
            dst[..n].copy_from_slice(&next[..n]);
            Ok(n)
        }

        fn send_semantics(&self) -> SendSemantics {
            self.semantics
        }
    }

    #[derive(Debug, Default)]
    struct SerialIo {
        sent: Vec<Vec<u8>>,
        receives: VecDeque<Result<Vec<u8>, Error>>,
    }

    #[derive(Debug)]
    struct ScriptedSerialTransport {
        config: TransportConfig,
        io: Arc<Mutex<SerialIo>>,
        semantics: SendSemantics,
    }

    impl ScriptedSerialTransport {
        fn new(
            receives: impl IntoIterator<Item = Result<Vec<u8>, Error>>,
            semantics: SendSemantics,
        ) -> (Self, Arc<Mutex<SerialIo>>) {
            let io = Arc::new(Mutex::new(SerialIo {
                sent: Vec::new(),
                receives: receives.into_iter().collect(),
            }));
            let transport = Self {
                config: TransportConfig {
                    addressing: AddressingMode::Serial,
                    ..TransportConfig::default()
                },
                io: Arc::clone(&io),
                semantics,
            };
            (transport, io)
        }
    }

    impl HasTransportConfig for ScriptedSerialTransport {
        fn transport_config(&self) -> &TransportConfig {
            &self.config
        }

        fn standard_transport_kind(&self) -> Option<crate::camera::TransportKind> {
            Some(crate::camera::TransportKind::Serial)
        }
    }

    impl BlockingTransport for ScriptedSerialTransport {
        fn send_with_kind(&mut self, bytes: &[u8], _kind: CommandKind) -> Result<(), Error> {
            self.io.lock().unwrap().sent.push(bytes.to_vec());
            Ok(())
        }

        fn recv_into(&mut self, dst: &mut [u8]) -> Result<usize, Error> {
            self.recv_into_with_timeout(dst, Duration::from_secs(1))
        }

        fn recv_into_with_timeout(
            &mut self,
            dst: &mut [u8],
            _timeout: Duration,
        ) -> Result<usize, Error> {
            let next = self
                .io
                .lock()
                .unwrap()
                .receives
                .pop_front()
                .unwrap_or(Ok(Vec::new()))?;
            let received = next.len().min(dst.len());
            dst[..received].copy_from_slice(&next[..received]);
            Ok(received)
        }

        fn addressing_mode_hint(&self) -> Option<AddressingMode> {
            Some(AddressingMode::Serial)
        }

        fn send_semantics(&self) -> SendSemantics {
            self.semantics
        }
    }

    fn config() -> TransportConfig {
        TransportConfig {
            addressing: AddressingMode::Ip,
            ..TransportConfig::default()
        }
    }

    fn profile() -> ProfileSpec {
        ProfileSpec::from_compile_time::<GenericVisca>().unwrap()
    }

    fn sony_reply(sequence: u32) -> Vec<u8> {
        let payload = [0x90, 0x50, 0x02, 0xff];
        let mut framed = crate::protocol::sony::SonyHeader::new_reply(payload.len(), sequence)
            .encode()
            .to_vec();
        framed.extend_from_slice(&payload);
        framed
    }

    fn set_sony_sequence(
        adapter: &BlockingTransportAdapter<ScriptedTransport>,
        sequence: u32,
    ) -> Result<(), &'static str> {
        let state = adapter.state.lock().unwrap();
        let OwnerEnvelope::Sony(envelope) = &state.envelope else {
            return Err("Sony sequence seed requires a Sony owner envelope");
        };
        envelope.set_sequence_for_test(sequence);
        Ok(())
    }

    fn serial_owner_adapter(
        receives: impl IntoIterator<Item = Result<Vec<u8>, Error>>,
        semantics: SendSemantics,
    ) -> (
        BlockingTransportAdapter<ScriptedSerialTransport>,
        Arc<Mutex<SerialIo>>,
    ) {
        let (transport, io) = ScriptedSerialTransport::new(receives, semantics);
        let profile = profile();
        let profiles = [
            (CameraId::CAMERA_1, &profile),
            (CameraId::CAMERA_2, &profile),
        ];
        let adapter = BlockingTransportAdapter::new_with_targets(
            transport,
            &profiles,
            OperationalTuning::new(),
            crate::DEFAULT_ADMISSION_CAPACITY,
        )
        .unwrap();
        (adapter, io)
    }

    fn serial_request(target: CameraId) -> RuntimeRequest {
        RuntimeRequest::Command {
            wire: Arc::new(
                EncodedMessage::new(&[target.to_address_byte(), 0x01, 0x04, 0x00, 0xff]).unwrap(),
            ),
            context: RequestContext {
                target,
                timeout: TimeoutPolicy {
                    ack: Duration::from_secs(1),
                    completion: Duration::from_secs(1),
                    inquiry: Duration::from_secs(1),
                    cancellation: Duration::from_secs(1),
                    ambiguity: Duration::from_secs(1),
                },
                retry: RetryPolicy::NEVER,
                control: ControlPolicy::default(),
                cancellation: CancellationPolicy::Supported,
                reply_shape: ReplyShape::AckThenCompletion,
            },
            applied_state: None,
        }
    }

    fn serial_inquiry(target: CameraId) -> RuntimeRequest {
        RuntimeRequest::Inquiry {
            wire: Arc::new(
                EncodedMessage::new(&[target.to_address_byte(), 0x09, 0x04, 0x00, 0xff]).unwrap(),
            ),
            context: RequestContext {
                target,
                timeout: TimeoutPolicy {
                    ack: Duration::from_secs(1),
                    completion: Duration::from_secs(1),
                    inquiry: Duration::from_secs(1),
                    cancellation: Duration::from_secs(1),
                    ambiguity: Duration::from_secs(1),
                },
                retry: RetryPolicy::NEVER,
                control: ControlPolicy::default(),
                cancellation: CancellationPolicy::Supported,
                reply_shape: ReplyShape::AckThenCompletion,
            },
            route: crate::runtime::engine::InquiryRoute::UNKNOWN,
        }
    }

    fn try_terminal(receipt: &super::super::ReceiptCore) -> Option<RuntimeOutcome> {
        match receipt.completion.try_recv() {
            Some(super::super::ReceiptObservation::Terminal(outcome)) => Some(outcome),
            Some(super::super::ReceiptObservation::CancellationFailed(error)) => {
                Some(RuntimeOutcome::Failed(error))
            }
            None => None,
        }
    }

    #[test]
    fn policy_and_raw_write_use_profile_and_transport_facts() {
        let transport = ScriptedTransport::new(config(), std::iter::empty());
        let adapter =
            BlockingTransportAdapter::new(transport, &profile(), CameraId::CAMERA_1).unwrap();
        assert_eq!(
            adapter.policy().protocol.envelope,
            crate::runtime::engine::EnvelopeKind::Raw
        );
        assert_eq!(
            adapter.policy().protocol.transport,
            crate::runtime::engine::TransportKind::Datagram
        );
        assert_eq!(adapter.policy().protocol.inquiry_capacity, 1);

        let (mut writer, _reader, _decoder) = adapter.parts();
        let mut owner = super::super::BlockingOwner::new(adapter.policy().clone()).unwrap();
        let profile = profile();
        let prepared = prepare_command(
            &crate::request::builtin::FocusModeCommand::Manual,
            CameraId::CAMERA_1,
            &profile,
            crate::OperationalTuning::new(),
            ClassSelection::Request,
        )
        .unwrap();
        owner.submit_command(&mut writer, prepared).unwrap();
    }

    #[test]
    fn reader_maps_timeout_without_hiding_transport_errors() {
        let mut cfg = config();
        cfg.read_timeout = Duration::from_millis(1);
        let transport = ScriptedTransport::new(
            cfg,
            [
                Err(Error::Timeout),
                Err(Error::TransportError("boom".into())),
            ],
        );
        let adapter =
            BlockingTransportAdapter::new(transport, &profile(), CameraId::CAMERA_1).unwrap();
        let (_writer, mut reader, _decoder) = adapter.parts();
        let mut receive = [0; 32];
        assert_eq!(
            reader.receive(&mut receive, None).unwrap(),
            BlockingReceive::TimedOut
        );
        assert!(matches!(
            reader.receive(&mut receive, None),
            Err(Error::TransportError(_))
        ));
    }

    /// Issue #637: the raw I/O spellings of an idle read mean the same thing as
    /// `Error::Timeout`, so a custom transport that forwards them verbatim gets
    /// the same "no data" treatment on both owners.
    #[test]
    fn reader_maps_raw_idle_io_kinds_to_the_same_no_data_answer() {
        let mut cfg = config();
        cfg.read_timeout = Duration::from_millis(1);
        let transport = ScriptedTransport::new(
            cfg,
            [
                Err(Error::Io(Arc::new(std::io::Error::from(
                    std::io::ErrorKind::WouldBlock,
                )))),
                Err(Error::Io(Arc::new(std::io::Error::from(
                    std::io::ErrorKind::TimedOut,
                )))),
                Err(Error::Io(Arc::new(std::io::Error::from(
                    std::io::ErrorKind::Interrupted,
                )))),
                Err(Error::Io(Arc::new(std::io::Error::from(
                    std::io::ErrorKind::ConnectionRefused,
                )))),
            ],
        );
        let adapter =
            BlockingTransportAdapter::new(transport, &profile(), CameraId::CAMERA_1).unwrap();
        let (_writer, mut reader, _decoder) = adapter.parts();
        let mut receive = [0; 32];
        for _ in 0..3 {
            assert_eq!(
                reader.receive(&mut receive, None).unwrap(),
                BlockingReceive::TimedOut
            );
        }
        assert!(
            matches!(reader.receive(&mut receive, None), Err(Error::Io(_))),
            "a real read failure is still a fault the owner gets to classify"
        );
    }

    #[test]
    fn oversized_datagram_is_discarded_before_framing_and_later_ack_completes() {
        let transport = ScriptedTransport::new(
            config(),
            [
                // Built-in UDP uses this established spelling only after it
                // has consumed an over-size datagram.  Its copied prefix must
                // not enter the raw VISCA decoder as an ACK.
                Err(Error::ResponseTooLarge { max_size: 3 }),
                Ok(vec![0x90, 0x41, 0xff]),
                Ok(vec![0x90, 0x51, 0xff]),
            ],
        );
        let adapter =
            BlockingTransportAdapter::new(transport, &profile(), CameraId::CAMERA_1).unwrap();
        let mut owner = super::super::BlockingOwner::new(adapter.policy().clone()).unwrap();
        let (mut writer, mut reader, mut decoder) = adapter.parts();
        let receipt = owner
            .submit(&mut writer, serial_request(CameraId::CAMERA_1))
            .unwrap();

        assert_eq!(
            owner
                .pump_once(&mut writer, &mut reader, &mut decoder)
                .unwrap(),
            0,
            "the consumed oversized datagram is discarded before framing"
        );
        assert!(try_terminal(&receipt).is_none());
        assert_eq!(owner.state().metrics().ignored_malformed_frames, 1);

        assert_eq!(
            owner
                .pump_once(&mut writer, &mut reader, &mut decoder)
                .unwrap(),
            1,
            "a later exact ACK remains a valid frame"
        );
        assert!(try_terminal(&receipt).is_none());
        assert_eq!(
            owner
                .pump_once(&mut writer, &mut reader, &mut decoder)
                .unwrap(),
            1,
            "the following completion is still attributed normally"
        );
        assert!(matches!(
            try_terminal(&receipt),
            Some(RuntimeOutcome::Applied)
        ));
    }

    #[test]
    fn decoder_extracts_basic_frames_and_reuses_one_transport_state() {
        let transport =
            ScriptedTransport::new(config(), [Ok(vec![0x90, 0x41, 0xff, 0x90, 0x51, 0xff])]);
        let adapter =
            BlockingTransportAdapter::new(transport, &profile(), CameraId::CAMERA_1).unwrap();
        let (_writer, mut reader, mut decoder) = adapter.parts();
        let mut buffers = OwnerBuffers::new(adapter.policy().limits).unwrap();
        let mut receive = [0; 128];
        let received = match reader.receive(&mut receive, None).unwrap() {
            BlockingReceive::Bytes(n) => {
                buffers.receive_mut()[..n].copy_from_slice(&receive[..n]);
                n
            }
            BlockingReceive::TimedOut => 0,
        };
        let frames = decoder.decode(&mut buffers, received, 4).unwrap();
        assert_eq!(frames.len(), 2);
        assert!(matches!(frames[0].response, DecodedResponse::Ack { .. }));
        assert!(matches!(
            frames[1].response,
            DecodedResponse::Completion { .. }
        ));
    }

    /// A raw owner already knows its envelope from the selected profile. A
    /// malformed/noise prefix can resemble a Sony header, but its `FF` still
    /// delimits one discarded raw frame before the replies that follow it.
    /// These prefixes are deliberately malformed/noise, not valid raw replies.
    #[test]
    fn raw_owner_recovers_sony_looking_noise_before_following_replies() {
        for payload_type in [[0x01, 0x11], [0x02, 0x00]] {
            let noise = [
                payload_type[0],
                payload_type[1],
                0x00,
                0x05,
                0x12,
                0x34,
                0x56,
                0x78,
                0x55,
                0xff,
            ];
            let mut bytes = noise.to_vec();
            bytes.extend_from_slice(&[0x90, 0x41, 0xff]);
            bytes.extend_from_slice(&[0x90, 0x51, 0xff]);

            let mut transport = ScriptedTransport::new(config(), [Ok(bytes)]);
            transport.semantics = SendSemantics::Stream;
            let adapter =
                BlockingTransportAdapter::new(transport, &profile(), CameraId::CAMERA_1).unwrap();
            let (_writer, mut reader, mut decoder) = adapter.parts();
            let mut buffers = OwnerBuffers::new(adapter.policy().limits).unwrap();
            let mut receive = [0; 128];
            let received = match reader.receive(&mut receive, None).unwrap() {
                BlockingReceive::Bytes(n) => {
                    buffers.receive_mut()[..n].copy_from_slice(&receive[..n]);
                    n
                }
                BlockingReceive::TimedOut => 0,
            };

            let frames = decoder.decode(&mut buffers, received, 4).unwrap();
            assert_eq!(frames.len(), 2, "{payload_type:02x?}");
            assert!(matches!(frames[0].response, DecodedResponse::Ack { .. }));
            assert!(matches!(
                frames[1].response,
                DecodedResponse::Completion { .. }
            ));
            assert_eq!(buffers.take_discarded_malformed(), 1, "{payload_type:02x?}");
        }
    }

    #[test]
    fn sony_decoder_preserves_full_sequence_metadata() {
        let payload = [0x90, 0x41, 0xff];
        let header = crate::protocol::sony::SonyHeader::new_reply(payload.len(), 0x1234_5678);
        let mut framed = header.encode().to_vec();
        framed.extend_from_slice(&payload);
        let transport = ScriptedTransport::new(config(), [Ok(framed)]);
        let adapter = BlockingTransportAdapter::new(
            transport,
            &ProfileSpec::from_compile_time::<SonyFR7>().unwrap(),
            CameraId::CAMERA_1,
        )
        .unwrap();
        let (_writer, mut reader, mut decoder) = adapter.parts();
        let mut buffers = OwnerBuffers::new(adapter.policy().limits).unwrap();
        let mut receive = [0; 128];
        let received = match reader.receive(&mut receive, None).unwrap() {
            BlockingReceive::Bytes(n) => {
                buffers.receive_mut()[..n].copy_from_slice(&receive[..n]);
                n
            }
            BlockingReceive::TimedOut => 0,
        };
        let frames = decoder.decode(&mut buffers, received, 4).unwrap();
        assert_eq!(
            frames[0].sequence,
            Some(EnvelopeSequence {
                value: 0x1234_5678,
                width: SequenceWidth::Full32,
            })
        );
        assert!(matches!(frames[0].response, DecodedResponse::Ack { .. }));
    }

    #[test]
    fn sony_owner_buffers_a_fragmented_header_and_payload_by_declared_length() {
        let payload = [0x90, 0x41, 0xff];
        let header = crate::protocol::sony::SonyHeader::new_reply(payload.len(), 0xff00_ff00);
        let mut framed = header.encode().to_vec();
        framed.extend_from_slice(&payload);
        let mut transport = ScriptedTransport::new(
            config(),
            [Ok(framed[..6].to_vec()), Ok(framed[6..].to_vec())],
        );
        transport.semantics = SendSemantics::Stream;
        let adapter = BlockingTransportAdapter::new(
            transport,
            &ProfileSpec::from_compile_time::<SonyFR7>().unwrap(),
            CameraId::CAMERA_1,
        )
        .unwrap();
        let (_writer, mut reader, mut decoder) = adapter.parts();
        let mut buffers = OwnerBuffers::new(adapter.policy().limits).unwrap();
        let mut receive = [0; 128];

        let first = match reader.receive(&mut receive, None).unwrap() {
            BlockingReceive::Bytes(n) => {
                buffers.receive_mut()[..n].copy_from_slice(&receive[..n]);
                n
            }
            BlockingReceive::TimedOut => 0,
        };
        assert!(decoder.decode(&mut buffers, first, 4).unwrap().is_empty());

        let second = match reader.receive(&mut receive, None).unwrap() {
            BlockingReceive::Bytes(n) => {
                buffers.receive_mut()[..n].copy_from_slice(&receive[..n]);
                n
            }
            BlockingReceive::TimedOut => 0,
        };
        let frames = decoder.decode(&mut buffers, second, 4).unwrap();
        assert_eq!(frames.len(), 1);
        assert_eq!(
            frames[0].sequence,
            Some(EnvelopeSequence {
                value: 0xff00_ff00,
                width: SequenceWidth::Full32,
            })
        );
        assert!(matches!(frames[0].response, DecodedResponse::Ack { .. }));
    }

    #[test]
    fn sony_decoder_preserves_potentially_truncated_lower16_metadata() {
        let payload = [0x90, 0x41, 0xff];
        // A zero upper half is deliberately classified as potentially
        // truncated by the production envelope parser, even though 42 also
        // fits in a genuine full-width Sony sequence.
        let header = crate::protocol::sony::SonyHeader::new_reply(payload.len(), 42);
        let mut framed = header.encode().to_vec();
        framed.extend_from_slice(&payload);
        let transport = ScriptedTransport::new(config(), [Ok(framed)]);
        let adapter = BlockingTransportAdapter::new(
            transport,
            &ProfileSpec::from_compile_time::<SonyFR7>().unwrap(),
            CameraId::CAMERA_1,
        )
        .unwrap();
        let (_writer, mut reader, mut decoder) = adapter.parts();
        let mut buffers = OwnerBuffers::new(adapter.policy().limits).unwrap();
        let mut receive = [0; 128];
        let received = match reader.receive(&mut receive, None).unwrap() {
            BlockingReceive::Bytes(n) => {
                buffers.receive_mut()[..n].copy_from_slice(&receive[..n]);
                n
            }
            BlockingReceive::TimedOut => 0,
        };
        let frames = decoder.decode(&mut buffers, received, 4).unwrap();
        assert_eq!(
            frames[0].sequence,
            Some(EnvelopeSequence {
                value: 42,
                width: SequenceWidth::Lower16,
            })
        );
    }

    #[test]
    fn sony_owner_routes_unique_lower16_and_recovers_after_collision() {
        let first_sequence = 0x1234_beef;
        let second_sequence = 0x5678_beef;
        let transport = ScriptedTransport::new(
            config(),
            [
                Ok(sony_reply(0xbeef)),
                Ok(sony_reply(first_sequence)),
                Ok(sony_reply(0xbeef)),
            ],
        );
        let adapter = BlockingTransportAdapter::new(
            transport,
            &ProfileSpec::from_compile_time::<SonyFR7>().unwrap(),
            CameraId::CAMERA_1,
        )
        .unwrap();
        let mut owner = super::super::BlockingOwner::new(adapter.policy().clone()).unwrap();
        let (mut writer, mut reader, mut decoder) = adapter.parts();

        // Use two distinct full-width identities with one lower half. This is
        // only a sequence-allocator setup for the production owner path; all
        // receive bytes still pass through the real Sony envelope/framer.
        set_sony_sequence(&adapter, first_sequence).unwrap();
        let first = owner
            .submit(&mut writer, serial_inquiry(CameraId::CAMERA_1))
            .unwrap();
        set_sony_sequence(&adapter, second_sequence).unwrap();
        let second = owner
            .submit(&mut writer, serial_inquiry(CameraId::CAMERA_1))
            .unwrap();

        // Both lower-16 owners are target-compatible, so the truncated reply
        // is deliberately inert rather than being guessed to one request.
        assert_eq!(
            owner
                .pump_once(&mut writer, &mut reader, &mut decoder)
                .unwrap(),
            1
        );
        assert!(try_terminal(&first).is_none());
        assert!(try_terminal(&second).is_none());

        // A full-width exact response removes one owner. The same lower-16
        // observation is then uniquely attributable to the remaining owner.
        owner
            .pump_once(&mut writer, &mut reader, &mut decoder)
            .unwrap();
        assert!(matches!(
            try_terminal(&first),
            Some(RuntimeOutcome::Reply { .. })
        ));
        assert!(try_terminal(&second).is_none());
        owner
            .pump_once(&mut writer, &mut reader, &mut decoder)
            .unwrap();
        assert!(matches!(
            try_terminal(&second),
            Some(RuntimeOutcome::Reply { .. })
        ));
    }

    #[test]
    fn sony_retry_and_cancel_wire_capture_preserve_logical_sequence_identity() {
        let payload = [0x90, 0x41, 0xff];
        let first_reply = crate::protocol::sony::SonyHeader::new_reply(payload.len(), 0)
            .encode()
            .into_iter()
            .chain(payload)
            .collect::<Vec<_>>();
        let transport = ScriptedTransport::new(config(), [Ok(first_reply)]);
        let sent = Arc::clone(&transport.sent);
        let profile = ProfileSpec::from_compile_time::<SonyFR7>().unwrap();
        let adapter =
            BlockingTransportAdapter::new(transport, &profile, CameraId::CAMERA_1).unwrap();
        assert_eq!(
            adapter.policy().protocol.envelope,
            crate::runtime::engine::EnvelopeKind::Sony
        );
        let mut owner = super::super::BlockingOwner::new(adapter.policy().clone()).unwrap();
        let (mut writer, mut reader, mut decoder) = adapter.parts();
        let request = RuntimeRequest::Command {
            wire: Arc::new(EncodedMessage::new(&[0x81, 0x01, 0x04, 0x00, 0xff]).unwrap()),
            context: RequestContext {
                target: CameraId::CAMERA_1,
                timeout: TimeoutPolicy {
                    ack: Duration::from_secs(1),
                    // The retry is forced with a synthetic future timestamp
                    // below.  Keep every post-ACK deadline beyond that
                    // timestamp so the cancellation's pacing wake, rather
                    // than an artificial completion/ambiguity expiry, is the
                    // next owner event.
                    completion: Duration::from_secs(10),
                    inquiry: Duration::from_secs(10),
                    cancellation: Duration::from_secs(10),
                    ambiguity: Duration::from_secs(10),
                },
                retry: RetryPolicy {
                    max_retries: 1,
                    initial_backoff: Duration::ZERO,
                    maximum_backoff: Duration::ZERO,
                    total_budget: Duration::from_secs(10),
                    ack_timeout: true,
                    completion_timeout: false,
                    inquiry_timeout: false,
                    buffer_full: false,
                    movement_not_executable: false,
                    builtin_inquiry_syntax: false,
                },
                control: ControlPolicy::default(),
                cancellation: CancellationPolicy::Supported,
                reply_shape: ReplyShape::AckThenCompletion,
            },
            applied_state: None,
        };
        let receipt = owner.submit(&mut writer, request).unwrap();
        assert_eq!(sent.lock().unwrap().len(), 1);

        // Force the ACK deadline. The first write result was successful, so the
        // engine already owns sequence zero and the retry must be byte-identical.
        owner
            .wake(
                &mut writer,
                std::time::Instant::now() + Duration::from_secs(2),
            )
            .unwrap();
        let writes = sent.lock().unwrap().clone();
        assert_eq!(writes.len(), 2);
        let first_header = crate::protocol::sony::SonyHeader::decode(&writes[0][..8]).unwrap();
        let retry_header = crate::protocol::sony::SonyHeader::decode(&writes[1][..8]).unwrap();
        assert_eq!(first_header.sequence_number, 0);
        assert_eq!(retry_header.sequence_number, 0);
        assert_eq!(writes[0], writes[1]);

        // The retry is now the authoritative command. Its ACK causes a cancel
        // write, which is a separate logical message and therefore receives a
        // fresh Sony identity rather than inheriting the command's sequence.
        assert_eq!(
            owner
                .pump_once(&mut writer, &mut reader, &mut decoder)
                .unwrap(),
            1,
            "the queued Sony ACK should advance the retried command"
        );
        let _cancel = owner.cancel_test(&mut writer, receipt).unwrap();
        assert_eq!(
            sent.lock().unwrap().len(),
            2,
            "the urgent cancel is still subject to the shared Sony pacing floor"
        );
        let wake = owner
            .state()
            .next_wake()
            .expect("pacing queues the cancellation for the next owner wake");
        owner.wake(&mut writer, wake).unwrap();
        let writes = sent.lock().unwrap().clone();
        assert_eq!(writes.len(), 3);
        let cancel_header = crate::protocol::sony::SonyHeader::decode(&writes[2][..8]).unwrap();
        assert_eq!(cancel_header.sequence_number, 1);
    }

    #[test]
    fn decoder_maps_malformed_visca_to_invalid_response() {
        let transport = ScriptedTransport::new(config(), [Ok(vec![0x80, 0x41, 0xff])]);
        let adapter =
            BlockingTransportAdapter::new(transport, &profile(), CameraId::CAMERA_1).unwrap();
        let (_writer, mut reader, mut decoder) = adapter.parts();
        let mut buffers = OwnerBuffers::new(adapter.policy().limits).unwrap();
        let mut receive = [0; 128];
        let received = match reader.receive(&mut receive, None).unwrap() {
            BlockingReceive::Bytes(n) => {
                buffers.receive_mut()[..n].copy_from_slice(&receive[..n]);
                n
            }
            BlockingReceive::TimedOut => 0,
        };
        assert!(matches!(
            decoder.decode(&mut buffers, received, 4),
            Err(Error::InvalidResponse { .. })
        ));
    }

    #[test]
    fn serial_adapter_owner_routes_out_of_order_same_socket_frames_by_source() {
        let (adapter, io) = serial_owner_adapter(
            [Ok(vec![
                0xa0, 0x41, 0xff, // camera 2 ACK, socket 1
                0x90, 0x41, 0xff, // camera 1 ACK, socket 1
                0xa0, 0x51, 0xff, // camera 2 completion, socket 1
                0x90, 0x51, 0xff, // camera 1 completion, socket 1
            ])],
            SendSemantics::Stream,
        );
        let mut owner = super::super::BlockingOwner::new(adapter.policy().clone()).unwrap();
        let (mut writer, mut reader, mut decoder) = adapter.parts();
        let camera_one = owner
            .submit(&mut writer, serial_request(CameraId::CAMERA_1))
            .unwrap();
        let camera_two = owner
            .submit(&mut writer, serial_request(CameraId::CAMERA_2))
            .unwrap();

        let sent = io.lock().unwrap().sent.clone();
        assert_eq!(sent.len(), 2);
        assert_eq!(sent[0][0], CameraId::CAMERA_1.to_address_byte());
        assert_eq!(sent[1][0], CameraId::CAMERA_2.to_address_byte());
        assert_eq!(
            owner
                .pump_once(&mut writer, &mut reader, &mut decoder)
                .unwrap(),
            4
        );
        assert!(matches!(
            try_terminal(&camera_one),
            Some(RuntimeOutcome::Applied)
        ));
        assert!(matches!(
            try_terminal(&camera_two),
            Some(RuntimeOutcome::Applied)
        ));
    }

    #[test]
    fn serial_adapter_owner_does_not_attribute_unregistered_or_malformed_sources() {
        let (adapter, _io) = serial_owner_adapter(
            [
                Ok(vec![0xb0, 0x41, 0xff]), // valid camera 3 source, not registered
                Ok(vec![0xa1, 0x41, 0xff]), // malformed camera 2 source nibble
                Ok(vec![0x90, 0x41, 0xff, 0x90, 0x51, 0xff]),
            ],
            SendSemantics::Datagram,
        );
        let mut owner = super::super::BlockingOwner::new(adapter.policy().clone()).unwrap();
        let (mut writer, mut reader, mut decoder) = adapter.parts();
        let receipt = owner
            .submit(&mut writer, serial_request(CameraId::CAMERA_1))
            .unwrap();

        assert_eq!(
            owner
                .pump_once(&mut writer, &mut reader, &mut decoder)
                .unwrap(),
            1
        );
        assert!(try_terminal(&receipt).is_none());
        assert_eq!(
            owner
                .pump_once(&mut writer, &mut reader, &mut decoder)
                .unwrap(),
            0,
            "a malformed datagram is ignored at the owner boundary"
        );
        assert!(!matches!(
            try_terminal(&receipt),
            Some(RuntimeOutcome::Applied)
        ));
        assert_eq!(
            owner
                .pump_once(&mut writer, &mut reader, &mut decoder)
                .unwrap(),
            2,
            "a later valid datagram must still be decoded after the malformed one"
        );
        assert!(matches!(
            try_terminal(&receipt),
            Some(RuntimeOutcome::Applied)
        ));
    }

    #[test]
    fn serial_adapter_owner_cancel_routes_camera_two_socket_one_only() {
        let (adapter, io) = serial_owner_adapter(
            [
                Ok(vec![
                    0xa0, 0x41, 0xff, // camera 2 ACK, socket 1
                    0x90, 0x41, 0xff, // camera 1 ACK, socket 1
                ]),
                Ok(vec![
                    0xa0, 0x61, 0x04, 0xff, // camera 2 cancellation, socket 1
                ]),
                Ok(vec![0x90, 0x51, 0xff]), // camera 1 completion, socket 1
            ],
            SendSemantics::Stream,
        );
        let mut owner = super::super::BlockingOwner::new(adapter.policy().clone()).unwrap();
        let (mut writer, mut reader, mut decoder) = adapter.parts();
        let camera_one = owner
            .submit(&mut writer, serial_request(CameraId::CAMERA_1))
            .unwrap();
        let camera_two = owner
            .submit(&mut writer, serial_request(CameraId::CAMERA_2))
            .unwrap();
        assert_eq!(
            owner
                .pump_once(&mut writer, &mut reader, &mut decoder)
                .unwrap(),
            2
        );

        let cancellation = owner.cancel_test(&mut writer, camera_two).unwrap();
        let sent = io.lock().unwrap().sent.clone();
        assert_eq!(sent.last(), Some(&vec![0x82, 0x21, 0xff]));

        assert_eq!(
            owner
                .pump_once(&mut writer, &mut reader, &mut decoder)
                .unwrap(),
            1
        );
        assert!(matches!(
            cancellation.recv_test(),
            Ok(CancellationObservation::Cancelled)
        ));
        assert!(try_terminal(&camera_one).is_none());
        assert_eq!(
            owner
                .pump_once(&mut writer, &mut reader, &mut decoder)
                .unwrap(),
            1
        );
        assert!(matches!(
            try_terminal(&camera_one),
            Some(RuntimeOutcome::Applied)
        ));
    }

    #[test]
    fn serial_raw_inquiries_remain_globally_single_flight() {
        let (adapter, io) =
            serial_owner_adapter([Ok(vec![0x90, 0x50, 0x02, 0xff])], SendSemantics::Stream);
        assert_eq!(adapter.policy().protocol.inquiry_capacity, 1);
        let mut owner = super::super::BlockingOwner::new(adapter.policy().clone()).unwrap();
        let (mut writer, mut reader, mut decoder) = adapter.parts();
        let first = owner
            .submit(&mut writer, serial_inquiry(CameraId::CAMERA_1))
            .unwrap();
        // Issue #561: the second inquiry queues behind the single flight
        // instead of failing, and stays unwritten until the flight frees.
        let queued = owner
            .submit(&mut writer, serial_inquiry(CameraId::CAMERA_2))
            .expect("a busy inquiry flight queues rather than failing");
        assert_eq!(io.lock().unwrap().sent.len(), 1);
        assert!(try_terminal(&queued).is_none());

        assert_eq!(
            owner
                .pump_once(&mut writer, &mut reader, &mut decoder)
                .unwrap(),
            1
        );
        assert!(matches!(
            try_terminal(&first),
            Some(RuntimeOutcome::Reply { payload, .. }) if payload.as_slice() == [0x02]
        ));
        assert_eq!(
            io.lock().unwrap().sent.len(),
            2,
            "the queued inquiry is written once the single flight frees"
        );
    }
}
