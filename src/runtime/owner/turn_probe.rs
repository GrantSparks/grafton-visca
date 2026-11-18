//! The real owner turn driver with deterministic in-memory transport I/O.

use std::{
    sync::Arc,
    time::{Duration, Instant},
};

use super::{
    adapter::AdapterFraming,
    boundary::{OwnerHandleCore, OwnerSnapshot},
    driver::owner_turn_driver,
    shell::{ClassifiedEvent, OwnerEvent, OwnerShellCore, TurnStep},
    turn::{Selection, TurnOutcome, TurnPlan},
    CommandOutcome, Effect, Observed, Observer, OwnerReceive, OwnerState, RetainedStreamInput,
    RuntimeRequest, TransmissionMeta, WireWrite, WriteOutcome,
};
use crate::{
    request::Request,
    runtime::engine::{
        CommandReply, ControlPolicy, DecodedFrame, DecodedResponse, EncodedMessage, EnvelopeKind,
        FrameSequence, RequestContext, RetryPolicy, TimeoutPolicy,
    },
    target::TargetSlot,
    transport::{AddressingMode, HasTransportConfig, IoBounds, SendSemantics, TransportIdentity},
    CameraId, Error, OperationalTuning, ProfileSpec, ViscaSocket,
};

/// A warmed engine and owner driver, with request lifecycle storage staged separately.
#[derive(Debug)]
pub(crate) struct TurnProbe {
    shell: ProbeShell,
    driver: ProbeTransport,
    // Keep all boundary channels connected for the probe's owner lifetime.
    _handle: OwnerHandleCore,
    wire: Arc<EncodedMessage>,
    pending: Option<Observer<CommandOutcome>>,
    submission: u64,
}

#[derive(Debug)]
struct ProbeShell {
    core: OwnerShellCore,
    io: ProbeIo,
}

#[allow(dead_code)] // The probe drives explicit events rather than the run/select loop.
impl ProbeShell {
    owner_turn_driver! {
        async: [],
        await: [],
        driver: ProbeDriver,
    }
}

trait ProbeDriver: RetainedStreamInput {
    fn record_write(&mut self, metadata: TransmissionMeta, bytes: &[u8]);
}

// Local fake: only the device write is replaced; framing, engine decisions,
// terminal delivery and recursive effect draining use the production owner.
#[derive(Debug)]
struct ProbeTransport {
    config: IoBounds,
    semantics: SendSemantics,
    metadata: Option<TransmissionMeta>,
    writes: usize,
    last_write: smallvec::SmallVec<[u8; 32]>,
}

impl HasTransportConfig for ProbeTransport {
    fn identity(&self) -> TransportIdentity<'_> {
        TransportIdentity::Custom {
            semantics: self.semantics,
            addressing: AddressingMode::Ip,
        }
    }

    fn io_bounds(&self) -> &IoBounds {
        &self.config
    }
}

impl RetainedStreamInput for ProbeTransport {}

impl ProbeDriver for ProbeTransport {
    fn record_write(&mut self, metadata: TransmissionMeta, bytes: &[u8]) {
        if metadata != TransmissionMeta::Known || self.metadata.is_none() {
            self.metadata = Some(metadata);
        }
        self.writes += 1;
        self.last_write.clear();
        self.last_write.extend_from_slice(bytes);
    }
}

#[derive(Debug)]
struct ProbeIo {
    now: Instant,
}

#[allow(dead_code)] // Required by the real driver macro's unused run/select path.
impl ProbeIo {
    const fn now(&self) -> Instant {
        self.now
    }

    fn cooperative_handoff(&mut self) {}

    fn select_event<'a, D: ProbeDriver>(
        &mut self,
        _state: &mut OwnerState,
        _receivers: &super::boundary::BoundaryReceivers,
        _selection: Selection,
        turn: super::turn::PlannedTurn<'a, super::boundary::RetainedBoundary>,
        _driver: &mut D,
    ) -> super::shell::SelectedOwnerInput<'a> {
        turn.into_other(super::shell::NonBoundaryEvent::Shutdown)
    }

    fn write<D: ProbeDriver>(
        &mut self,
        driver: &mut D,
        mut write: WireWrite<'_>,
        _timeout: Duration,
    ) -> WriteOutcome {
        match write.frame() {
            Ok(metadata) => {
                driver.record_write(metadata, write.frame_buffer);
                WriteOutcome::Sent(metadata)
            }
            Err(error) => WriteOutcome::NotSent(error),
        }
    }
}

impl TurnProbe {
    pub(crate) fn new(
        profile: &ProfileSpec,
        config: IoBounds,
        semantics: SendSemantics,
    ) -> Result<Self, Error> {
        let driver = ProbeTransport {
            config,
            semantics,
            metadata: None,
            writes: 0,
            last_write: smallvec::SmallVec::new(),
        };
        let tuning = OperationalTuning::new();
        let (_framing, policy) = AdapterFraming::for_targets(
            &driver,
            &[(TargetSlot::ALL[0], profile)],
            crate::session_config::ValidatedTuning::validate(tuning, [profile])?,
            crate::DEFAULT_ADMISSION_CAPACITY,
            false,
        )?;
        let now = Instant::now();
        let state = OwnerState::new(policy, now)?;
        let (handle, ends) = OwnerHandleCore::new(&state);
        let (wire, ()) = EncodedMessage::encode(crate::command::zoom::Stop::MAX_SIZE, |buffer| {
            let written = crate::command::zoom::Stop.write_into(CameraId::CAMERA_1, buffer)?;
            Ok((written, ()))
        })?;
        Ok(Self {
            shell: ProbeShell {
                core: OwnerShellCore::new(state, ends),
                io: ProbeIo { now },
            },
            driver,
            _handle: handle,
            wire: Arc::new(wire),
            pending: None,
            submission: 0,
        })
    }

    /// Allocate and admit one request before measuring its protocol turns.
    pub(crate) fn stage_command(&mut self) -> Result<(), Error> {
        self.stage_command_with_retry(false)
    }

    pub(crate) fn stage_retry(&mut self) -> Result<(), Error> {
        self.stage_command_with_retry(true)
    }

    fn stage_command_with_retry(&mut self, retry: bool) -> Result<(), Error> {
        if self.pending.is_some() {
            return Err(Error::InvalidState {
                reason: "probe command already staged".into(),
            });
        }
        // Advance the deterministic clock past the profile's pacing minimum;
        // the probe retains the production policy rather than weakening it.
        self.shell.io.now = self
            .shell
            .io
            .now
            .checked_add(self.shell.core.state.engine.command_spacing())
            .ok_or_else(|| Error::InvalidState {
                reason: "probe pacing instant overflowed".into(),
            })?;
        let context = RequestContext {
            requires_extended_pan_tilt_speed: false,
            motion: None,
            submission_order: self.submission,
            dispatch_deadline: None,
            target: TargetSlot::ALL[0],
            timeout: TimeoutPolicy::default(),
            retry: RetryPolicy {
                max_retries: u32::from(retry),
                initial_backoff: Duration::from_millis(u64::from(retry)),
                maximum_backoff: Duration::from_millis(u64::from(retry)),
                total_budget: None,
            },
            control: ControlPolicy::default(),
        };
        let request = RuntimeRequest::Command {
            not_executable: false,
            wire: Arc::clone(&self.wire),
            context,
            reply_shape: CommandReply::AckThenCompletion,
            applied_state: None,
        };
        let permit = self.shell.core.state.permits().try_acquire(&request)?;
        let (observer, cell) = Observer::<CommandOutcome>::pair();
        // flume lazily allocates its bounded one-slot queue on first delivery.
        // That per-request lifecycle storage is permitted by the contract;
        // reserve it here, outside the measured protocol turn, while retaining
        // the real observer and real terminal delivery in command_turn().
        cell.sender
            .try_send(Observed {
                at: self.shell.io.now,
                value: CommandOutcome::Applied,
            })
            .map_err(|_| Error::InvalidState {
                reason: "probe observer warm-up failed".into(),
            })?;
        observer
            .receiver
            .try_recv()
            .map_err(|_| Error::InvalidState {
                reason: "probe observer warm-up was not delivered".into(),
            })?;
        let turn = self.shell.core.state.begin_input_turn(self.shell.io.now);
        self.shell
            .core
            .state
            .admit_in_turn(&turn, request, permit, cell)?;
        self.pending = Some(observer);
        self.submission += 1;
        self.driver.metadata = None;
        Ok(())
    }

    /// Run dispatch, exact write completion, ACK and protocol completion.
    pub(crate) fn command_turn(&mut self) -> Result<(), Error> {
        self.write_command()?;
        self.complete_command()
    }

    /// Prove a conclusive pre-ACK rejection reuses the exact framed attempt.
    pub(crate) fn retry_turn(&mut self) -> Result<(), Error> {
        self.write_command()?;
        let original = self.driver.last_write.clone();
        let metadata = self.driver.metadata;
        let buffer = &self.shell.core.state.buffers.transmit;
        let storage = (buffer.as_ptr(), buffer.capacity());
        self.receive(
            &[DecodedResponse::Error {
                socket: None,
                code: 0x03.into(),
            }],
            self.sequence(),
        );
        self.shell.io.now += self
            .shell
            .core
            .state
            .engine
            .command_spacing()
            .max(Duration::from_millis(10));
        self.write_command()?;
        let buffer = &self.shell.core.state.buffers.transmit;
        if original != self.driver.last_write
            || metadata != self.driver.metadata
            || storage != (buffer.as_ptr(), buffer.capacity())
        {
            return Err(Error::InvalidState {
                reason: "safe retry changed the original frame, sequence or framing buffer".into(),
            });
        }
        self.complete_command()
    }

    fn write_command(&mut self) -> Result<(), Error> {
        if self.pending.is_none() {
            return Err(Error::InvalidState {
                reason: "probe command was not staged".into(),
            });
        }
        let writes = self.driver.writes;
        let effects = self.shell.core.state.advance(self.shell.io.now);
        self.shell.drive(&mut self.driver, effects);
        if self.driver.writes != writes + 1 {
            return Err(Error::InvalidState {
                reason: "probe command did not write exactly once".into(),
            });
        }
        Ok(())
    }

    fn complete_command(&mut self) -> Result<(), Error> {
        self.receive(
            &[
                DecodedResponse::Ack {
                    socket: Some(ViscaSocket::S1),
                },
                DecodedResponse::Completion {
                    socket: Some(ViscaSocket::S1),
                },
            ],
            self.sequence(),
        );
        let observer = self.pending.take().ok_or_else(|| Error::InvalidState {
            reason: "probe command observer disappeared".into(),
        })?;
        match observer.try_observed().map(|observed| observed.value) {
            Some(CommandOutcome::Applied) => Ok(()),
            Some(CommandOutcome::Failed(error)) => Err(error),
            _ => Err(Error::InvalidState {
                reason: "probe command did not apply".into(),
            }),
        }
    }

    /// Run a complete frame turn whose reply has no live request to alter.
    pub(crate) fn frame_turn(&mut self) -> Result<(), Error> {
        if self.pending.is_some() {
            return Err(Error::InvalidState {
                reason: "probe frame turn requires no staged command".into(),
            });
        }
        let sequence = matches!(
            self.shell.core.state.engine.envelope_kind(),
            EnvelopeKind::Sony
        )
        .then(|| {
            self.sequence()
                .unwrap_or(FrameSequence::from_reply_header(0))
        });
        self.receive(
            &[DecodedResponse::Completion {
                socket: Some(ViscaSocket::S1),
            }],
            sequence,
        );
        if self.shell.core.is_running() {
            Ok(())
        } else {
            Err(Error::InvalidState {
                reason: "inert probe frame ended the owner".into(),
            })
        }
    }

    fn sequence(&self) -> Option<FrameSequence> {
        match self.driver.metadata {
            Some(TransmissionMeta::Allocated { sequence }) => {
                Some(FrameSequence::from_reply_header(sequence))
            }
            _ => None,
        }
    }

    fn receive(&mut self, responses: &[DecodedResponse], sequence: Option<FrameSequence>) {
        let mut frames = self.shell.core.state.buffers().take_frames();
        for response in responses {
            assert!(
                frames
                    .try_push(DecodedFrame {
                        target: TargetSlot::ALL[0],
                        sequence,
                        response: response.clone(),
                    })
                    .is_ok(),
                "probe frames fit the receive bound"
            );
        }
        self.shell.handle_event(
            OwnerEvent::Receive {
                result: OwnerReceive::Frames(frames),
                received_at: self.shell.io.now,
            },
            &mut self.driver,
            self.shell.io.now,
            false,
        );
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn every_registered_profile_supports_command_and_frame_turns() {
        for id in crate::profiles::ProfileId::all() {
            let profile = id.spec().unwrap();
            for semantics in [SendSemantics::Stream, SendSemantics::Datagram] {
                let mut probe = TurnProbe::new(&profile, IoBounds::default(), semantics).unwrap();
                probe.stage_command().unwrap();
                probe.command_turn().unwrap();
                if probe.shell.core.state.engine.envelope_kind() == EnvelopeKind::Sony {
                    assert!(matches!(
                        probe.sequence(),
                        Some(FrameSequence::MaybeTruncated(_))
                    ));
                }
                probe.frame_turn().unwrap();
            }
        }
    }

    #[test]
    fn probe_pacing_reads_the_retuned_engine() {
        let profile = ProfileSpec::from_compile_time::<crate::profiles::GenericVisca>().unwrap();
        let mut probe =
            TurnProbe::new(&profile, IoBounds::default(), SendSemantics::Datagram).unwrap();
        let spacing = profile.timing().command_spacing() + Duration::from_millis(23);
        let tuning = OperationalTuning::new().with_command_spacing(spacing);
        let tuning = crate::session_config::ValidatedTuning::validate(tuning, [&profile]).unwrap();
        let before = probe.shell.io.now;
        probe.shell.core.state.retune(tuning, before).unwrap();
        probe.stage_command().unwrap();
        assert_eq!(probe.shell.io.now - before, spacing);
        probe.command_turn().unwrap();
    }
}
