//! Caller-thread owner for the blocking mode.

use std::{
    cell::RefCell,
    collections::VecDeque,
    fmt,
    marker::PhantomData,
    sync::Arc,
    time::{Duration, Instant},
};

use crate::{
    completion, protocol::framer::RawIncompletePrefix, AffectedAxes, CancellationOutcome, Error,
    ResponseDecoder,
};

use super::{
    cancellation_receipt_for, clamp_receive_pause, normalize_cancellation_observation,
    normalize_command_outcome, normalize_inquiry_outcome, prepend_effects, transient_receive_pause,
    AppliedEffect, BlockingTransportAdapter, CancellationCore, CompletionObserver, DiagnosticEvent,
    OwnerInputTurn, OwnerPolicy, OwnerState, ReceiptCore, ReceiptObservation, RejectedCancellation,
    RequestId, RequestLane, RuntimeOutcome, RuntimeRequest, ShutdownReason, TransientFaultRun,
    TransmissionMeta, WaitSelection, WireWrite,
};

#[cfg(all(test, not(feature = "async")))]
use super::{
    MAXIMUM_TRANSIENT_RECEIVE_PAUSE, TRANSIENT_RECEIVE_FAULT_LIMIT, TRANSIENT_RECEIVE_FAULT_RESET,
    TRANSIENT_RECEIVE_FAULT_SPAN, TRANSIENT_RECEIVE_PAUSE,
};
use crate::runtime::engine::{
    DecodedFrame, Effect, FirstDispatch, FirstDispatchWait, IgnoreReason, Input,
    RawCorrelationReleaseSet, RawPrefixEvidence, RawReleaseGateAction, TransportKind,
};

/// Exact blocking write seam. Envelope encoding and sequence allocation belong
/// in the adapter; its returned metadata is fed to the engine before any other
/// boundary input can be observed.
pub(crate) trait BlockingWireDriver {
    fn write(&mut self, write: WireWrite<'_>) -> Result<TransmissionMeta, Error>;
}

/// Raw read seam. Framing and typed decoding are deliberately separate from
/// both transport I/O and the mutable engine borrow.
pub(crate) trait BlockingReadDriver {
    /// Wait no later than `owner_deadline`; the adapter also applies its own
    /// configured transport timeout and uses the earlier instant.
    fn receive(
        &mut self,
        receive_buffer: &mut [u8],
        owner_deadline: Option<Instant>,
    ) -> Result<BlockingReceive, Error>;
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum BlockingReceive {
    Bytes(usize),
    TimedOut,
}

pub(crate) trait BlockingFrameDecoder {
    fn decode(
        &mut self,
        buffers: &mut super::OwnerBuffers,
        received: usize,
        frame_limit: usize,
    ) -> Result<Vec<DecodedFrame>, Error>;

    /// Whether the stream decoder retains any unconsumed input. Datagram
    /// decoders and stateless test decoders retain nothing by default.
    fn has_buffered_stream_input(&mut self) -> Result<bool, Error> {
        Ok(false)
    }

    /// Evidence visible in the first retained raw stream input. Complete
    /// frames stay on the normal decode path; incomplete prefixes are
    /// classified just far enough for the shared correlation engine to decide
    /// whether a due release may discard or preserve them.
    fn buffered_raw_prefix_evidence(&mut self) -> Result<Option<RawPrefixEvidence>, Error> {
        Ok(self
            .buffered_stream_input_target()?
            .map(|target| RawPrefixEvidence::Incomplete {
                target,
                // Older test-only decoders model the historical stale-frame
                // seam, whose only valid action is one-fragment discard.
                // Production adapters override this method with evidence from
                // the actual first two raw bytes, so they never inherit this
                // compatibility classification.
                kind: RawIncompletePrefix::NamedCompletionOrError(crate::ViscaSocket::S1),
            }))
    }

    /// Target-only compatibility seam for stateless test decoders. Production
    /// raw transports override [`Self::buffered_raw_prefix_evidence`] and must
    /// never reduce their first two bytes to this coarse answer.
    fn buffered_stream_input_target(&mut self) -> Result<Option<crate::CameraId>, Error> {
        Ok(None)
    }

    /// Discard exactly the first retained raw stream frame or incomplete
    /// fragment. Production decoders preserve all later target input.
    fn discard_buffered_stream_input(&mut self) -> Result<(), Error> {
        Ok(())
    }
}

/// Blocking-mode linear command observation right.
#[derive(Debug)]
pub(crate) struct BlockingCommandReceipt {
    core: ReceiptCore,
}

/// Blocking-mode linear inquiry observation right and its exact decoder.
#[derive(Debug)]
pub(crate) struct BlockingInquiryReceipt<R> {
    core: ReceiptCore,
    decoder: ResponseDecoder<R>,
}

/// Blocking-mode linear operation observation right and its exact settlement
/// metadata. Only this receipt class exposes cancellation.
#[derive(Debug)]
pub(crate) struct BlockingOperationReceipt<K>
where
    K: completion::Kind,
{
    core: ReceiptCore,
    affected_axes: AffectedAxes,
    settlement: completion::Settlement<K>,
    marker: PhantomData<fn() -> K>,
}

/// Blocking-mode observation of one exact cancellation request.
#[derive(Debug)]
pub(crate) struct BlockingCancellationReceipt {
    core: CancellationCore,
}

/// The caller-thread owner and adapter parts shared by all blocking receipts
/// in one session.
///
/// The parts are borrowed once by the session core and are only mutably
/// borrowed for the duration of an individual handle method. This is what
/// permits a caller to retain multiple operation handles and observe them in
/// any order without making the first handle's lifetime an exclusive borrow of
/// the entire session. `RefCell` is intentional: blocking sessions are
/// caller-driven and non-`Sync`; a re-entrant method call is rejected as
/// [`Error::TransportBusy`] rather than recursively entering the owner.
#[cfg(all(test, not(feature = "async")))]
pub(crate) struct BlockingSessionCore<'a> {
    parts: RefCell<BlockingSessionParts<'a>>,
}

#[cfg(all(test, not(feature = "async")))]
struct BlockingSessionParts<'a> {
    owner: &'a mut BlockingOwner,
    driver: &'a mut dyn BlockingWireDriver,
    reader: &'a mut dyn BlockingReadDriver,
    decoder: &'a mut dyn BlockingFrameDecoder,
}

#[cfg(all(test, not(feature = "async")))]
impl<'a> BlockingSessionCore<'a> {
    /// Borrows one blocking session's owner and adapter seams into a shared,
    /// caller-thread control core. No operation handle allocation is needed.
    pub(crate) fn new<D, R, F>(
        owner: &'a mut BlockingOwner,
        driver: &'a mut D,
        reader: &'a mut R,
        decoder: &'a mut F,
    ) -> Self
    where
        D: BlockingWireDriver,
        R: BlockingReadDriver,
        F: BlockingFrameDecoder,
    {
        Self {
            parts: RefCell::new(BlockingSessionParts {
                owner,
                driver,
                reader,
                decoder,
            }),
        }
    }

    fn with_parts<T>(
        &self,
        operation: impl FnOnce(
            &mut BlockingOwner,
            &mut dyn BlockingWireDriver,
            &mut dyn BlockingReadDriver,
            &mut dyn BlockingFrameDecoder,
        ) -> Result<T, Error>,
    ) -> Result<T, Error> {
        let mut parts = self
            .parts
            .try_borrow_mut()
            .map_err(|_| Error::TransportBusy)?;
        let BlockingSessionParts {
            owner,
            driver,
            reader,
            decoder,
        } = &mut *parts;
        operation(owner, *driver, *reader, *decoder)
    }

    /// Runs one cancellation turn, returning the receipt with the reason when
    /// the turn itself cannot be taken (#612).
    fn with_parts_cancelling<T>(
        &self,
        receipt: ReceiptCore,
        operation: impl FnOnce(
            &mut BlockingOwner,
            &mut dyn BlockingWireDriver,
            ReceiptCore,
        ) -> Result<T, RejectedCancellation>,
    ) -> Result<T, RejectedCancellation> {
        let Ok(mut parts) = self.parts.try_borrow_mut() else {
            return Err(RejectedCancellation::kept(receipt, Error::TransportBusy));
        };
        let BlockingSessionParts { owner, driver, .. } = &mut *parts;
        operation(owner, *driver, receipt)
    }
}

#[cfg(all(test, not(feature = "async")))]
impl fmt::Debug for BlockingSessionCore<'_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.debug_struct("BlockingSessionCore").finish()
    }
}

/// Object-safe caller-thread host used by public blocking handles.
///
/// A borrowed [`BlockingSessionCore`] and an owning blocking session host both
/// implement this seam.  Handles therefore retain only a shared reference to
/// the host; the host's interior mutability serializes each individual method
/// and rejects re-entrant calls immediately.
pub(crate) trait BlockingControlHost {
    /// Returns the caller-thread owner's monotonic clock instant.
    fn now(&self) -> Instant;

    /// Sleeps on the caller-thread owner's clock seam.
    fn sleep(&self, duration: Duration);

    /// Creates one observer deadline from the owner clock.
    fn deadline_after(&self, timeout: Duration) -> Result<Instant, Error>;

    fn owner_matches(&self, origin: &Arc<()>) -> Result<bool, Error>;

    fn pump_once_until(&self, deadline: Option<Instant>) -> Result<usize, Error>;

    fn submit_inquiry_until(
        &self,
        request: RuntimeRequest,
        configured_timeout: Duration,
        deadline: Instant,
    ) -> Result<ReceiptCore, Error>;

    /// Records cancellation intent, handing the receipt back when the owner
    /// refuses so the caller keeps the original request's observer (#612).
    fn cancel_operation(
        &self,
        receipt: ReceiptCore,
    ) -> Result<BlockingCancellationReceipt, RejectedCancellation>;
}

#[cfg(all(test, not(feature = "async")))]
impl BlockingControlHost for BlockingSessionCore<'_> {
    fn now(&self) -> Instant {
        Instant::now()
    }

    fn sleep(&self, duration: Duration) {
        std::thread::sleep(duration);
    }

    fn deadline_after(&self, timeout: Duration) -> Result<Instant, Error> {
        observer_deadline(self.now(), timeout)
    }

    fn owner_matches(&self, origin: &Arc<()>) -> Result<bool, Error> {
        self.with_parts(|owner, _, _, _| Ok(Arc::ptr_eq(origin, &owner.state.origin)))
    }

    fn pump_once_until(&self, deadline: Option<Instant>) -> Result<usize, Error> {
        self.with_parts(|owner, driver, reader, decoder| {
            owner.pump_once_until(driver, reader, decoder, deadline)
        })
    }

    fn submit_inquiry_until(
        &self,
        request: RuntimeRequest,
        configured_timeout: Duration,
        deadline: Instant,
    ) -> Result<ReceiptCore, Error> {
        self.with_parts(|owner, driver, reader, decoder| {
            owner.submit_request_until_with_pump(
                driver,
                reader,
                decoder,
                request,
                configured_timeout,
                deadline,
            )
        })
    }

    fn cancel_operation(
        &self,
        receipt: ReceiptCore,
    ) -> Result<BlockingCancellationReceipt, RejectedCancellation> {
        self.with_parts_cancelling(receipt, |owner, driver, receipt| {
            owner.cancel_core(driver, receipt)
        })
    }
}

/// Owning caller-thread host used by the minimal public blocking session.
///
/// The transport adapter is split once at construction and all three views are
/// retained inside this one `RefCell`.  Handles borrow this host immutably and
/// each operation obtains one mutable host turn, so an owning session does not
/// require a self-referential `BlockingSessionCore`.
pub(crate) struct BlockingSessionHost {
    parts: RefCell<BlockingOwnedSessionParts>,
    state_cache: Arc<[std::sync::Mutex<super::TargetStateCache>; 9]>,
    /// The owner's live operational tuning (#631). The blocking owner runs on
    /// the caller thread, so the update and every subsequent preparation are
    /// already ordered by the one owner turn each takes.
    tuning: super::LiveTuning,
}

struct BlockingOwnedSessionParts {
    owner: BlockingOwner,
    driver: Box<dyn BlockingWireDriver + Send>,
    reader: Box<dyn BlockingReadDriver + Send>,
    decoder: Box<dyn BlockingFrameDecoder + Send>,
}

impl fmt::Debug for BlockingSessionHost {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.debug_struct("BlockingSessionHost").finish()
    }
}

impl BlockingSessionHost {
    pub(crate) fn from_adapter<T>(adapter: BlockingTransportAdapter<T>) -> Result<Self, Error>
    where
        T: crate::transport::BlockingTransport + crate::transport::HasTransportConfig + 'static,
    {
        let policy = adapter.policy().clone();
        let (driver, reader, decoder) = adapter.into_parts();
        let owner = BlockingOwner::new(policy)?;
        let state_cache = owner.state().state_cache_registry();
        let tuning = owner.state().live_tuning();
        Ok(Self {
            parts: RefCell::new(BlockingOwnedSessionParts {
                owner,
                driver: Box::new(driver),
                reader: Box::new(reader),
                decoder: Box::new(decoder),
            }),
            state_cache,
            tuning,
        })
    }

    fn with_parts<T>(
        &self,
        operation: impl FnOnce(
            &mut BlockingOwner,
            &mut dyn BlockingWireDriver,
            &mut dyn BlockingReadDriver,
            &mut dyn BlockingFrameDecoder,
        ) -> Result<T, Error>,
    ) -> Result<T, Error> {
        let mut parts = self
            .parts
            .try_borrow_mut()
            .map_err(|_| Error::TransportBusy)?;
        let BlockingOwnedSessionParts {
            owner,
            driver,
            reader,
            decoder,
        } = &mut *parts;
        operation(owner, driver.as_mut(), reader.as_mut(), decoder.as_mut())
    }

    /// Runs one cancellation turn, returning the receipt with the reason when
    /// the turn itself cannot be taken (#612).
    fn with_parts_cancelling<T>(
        &self,
        receipt: ReceiptCore,
        operation: impl FnOnce(
            &mut BlockingOwner,
            &mut dyn BlockingWireDriver,
            ReceiptCore,
        ) -> Result<T, RejectedCancellation>,
    ) -> Result<T, RejectedCancellation> {
        let Ok(mut parts) = self.parts.try_borrow_mut() else {
            return Err(RejectedCancellation::kept(receipt, Error::TransportBusy));
        };
        let BlockingOwnedSessionParts { owner, driver, .. } = &mut *parts;
        operation(owner, driver.as_mut(), receipt)
    }

    pub(crate) fn submit_command(
        &self,
        prepared: crate::prepared::PreparedCommand,
    ) -> Result<BlockingCommandReceipt, Error> {
        self.with_parts(|owner, driver, reader, decoder| {
            owner.submit_command_with_pump(driver, reader, decoder, prepared)
        })
    }

    pub(crate) fn submit_inquiry<R>(
        &self,
        prepared: crate::prepared::PreparedInquiry<R>,
    ) -> Result<BlockingInquiryReceipt<R>, Error> {
        self.with_parts(|owner, driver, reader, decoder| {
            owner.submit_inquiry_with_pump(driver, reader, decoder, prepared)
        })
    }

    pub(crate) fn submit_operation<K>(
        &self,
        prepared: crate::prepared::PreparedOperation<K>,
    ) -> Result<BlockingOperationReceipt<K>, Error>
    where
        K: completion::Kind,
    {
        self.with_parts(|owner, driver, reader, decoder| {
            // Global admission capacity is independent of the raw target
            // socket gate below. Probe it before entering the bounded #673
            // drain so a full session returns its dedicated capacity error
            // without waiting for an ACK that cannot make room for this
            // request. The owner turn is serialized by `parts`, so the probe
            // and the subsequent admission cannot race another submission.
            let target = prepared.target();
            owner.ensure_admission_capacity(target)?;
            // Issue #673: before an ordinary first-write submit, drain the raw
            // single-candidate pre-ACK gate if that alone blocks this target.
            // The peer's ACK is pumped under this request's own ACK budget so a
            // command socket frees and the subsequent first write wins.
            // CompletionOnly still needs target idleness and never meets that
            // sole-obstacle rule. Intrinsic Urgent operations also skip the
            // drain: #714 lets their safety lane cross one positional candidate
            // and makes any resulting two-candidate ACK bind to neither.
            if let Some(ack_budget) = prepared.preack_drain_hint() {
                owner.drain_raw_preack_gate(driver, reader, decoder, target, ack_budget)?;
            }
            owner.submit_operation_with_pump(driver, reader, decoder, prepared)
        })
    }

    pub(crate) fn shutdown(&self) -> Result<(), Error> {
        self.with_parts(|owner, driver, _, _| owner.shutdown(driver))
    }

    pub(crate) fn drain_diagnostics(&self) -> Result<Vec<DiagnosticEvent>, Error> {
        self.with_parts(|owner, _, _, _| Ok(owner.drain_diagnostics()))
    }

    pub(crate) fn metrics(&self) -> Result<crate::observability::MetricsSnapshot, Error> {
        self.with_parts(|owner, _, _, _| Ok(owner.state().metrics_snapshot()))
    }

    pub(crate) fn state_cache(&self, target: crate::CameraId) -> crate::state_cache::StateCache {
        crate::state_cache::StateCache::from_registry(Arc::clone(&self.state_cache), target)
    }

    /// Reads the tuning the owner is currently preparing requests under.
    pub(crate) fn tuning(&self) -> crate::OperationalTuning {
        self.tuning.get()
    }

    /// Installs new session tuning on the caller-thread owner (#631).
    ///
    /// This is the blocking counterpart of the async control-boundary message:
    /// the owner turn this takes is the same exclusive turn a submission takes,
    /// so an update can never interleave with one. A re-entrant call — one made
    /// from inside another owner turn — is rejected as
    /// [`Error::TransportBusy`] rather than corrupting that turn, and a call on
    /// a session that has already reached its terminal boundary returns that
    /// boundary error rather than silently succeeding (issue #690).
    pub(crate) fn reconfigure(
        &self,
        validated_tuning: Result<crate::OperationalTuning, Error>,
    ) -> Result<(), Error> {
        self.with_parts(|owner, _, _, _| owner.reconfigure(validated_tuning))
    }

    /// Returns the caller-thread owner's monotonic clock instant.
    pub(crate) fn now(&self) -> Instant {
        Instant::now()
    }

    /// Sleeps through the caller-thread owner's clock seam.
    pub(crate) fn sleep(&self, duration: Duration) {
        std::thread::sleep(duration);
    }

    /// Creates one observer deadline from the owner clock.
    pub(crate) fn deadline_after(&self, timeout: Duration) -> Result<Instant, Error> {
        observer_deadline(self.now(), timeout)
    }
}

impl BlockingControlHost for BlockingSessionHost {
    fn now(&self) -> Instant {
        self.now()
    }

    fn sleep(&self, duration: Duration) {
        self.sleep(duration)
    }

    fn deadline_after(&self, timeout: Duration) -> Result<Instant, Error> {
        self.deadline_after(timeout)
    }

    fn owner_matches(&self, origin: &Arc<()>) -> Result<bool, Error> {
        self.with_parts(|owner, _, _, _| Ok(Arc::ptr_eq(origin, &owner.state.origin)))
    }

    fn pump_once_until(&self, deadline: Option<Instant>) -> Result<usize, Error> {
        self.with_parts(|owner, driver, reader, decoder| {
            owner.pump_once_until(driver, reader, decoder, deadline)
        })
    }

    fn submit_inquiry_until(
        &self,
        request: RuntimeRequest,
        configured_timeout: Duration,
        deadline: Instant,
    ) -> Result<ReceiptCore, Error> {
        self.with_parts(|owner, driver, reader, decoder| {
            owner.submit_request_until_with_pump(
                driver,
                reader,
                decoder,
                request,
                configured_timeout,
                deadline,
            )
        })
    }

    fn cancel_operation(
        &self,
        receipt: ReceiptCore,
    ) -> Result<BlockingCancellationReceipt, RejectedCancellation> {
        self.with_parts_cancelling(receipt, |owner, driver, receipt| {
            owner.cancel_core(driver, receipt)
        })
    }
}

/// The caller-thread transport/reader control needed while a blocking receipt
/// is observed. It has no lifecycle identity of its own.
///
/// The `Borrowed` variant exists only for the owner-level receipt tests: its
/// sole constructor is [`BlockingOwner::receipt_control`], and every caller of
/// that is in `src/runtime/owner/tests.rs`. No adapter or facade path builds
/// one. Public handles are created with `Shared`, which is the only variant
/// that can outlive an individual method call.
pub(crate) struct BlockingReceiptControl<'a> {
    kind: BlockingControlKind<'a>,
}

enum BlockingControlKind<'a> {
    Shared(&'a dyn BlockingControlHost),
    #[cfg(all(test, not(feature = "async")))]
    Borrowed {
        owner: &'a mut BlockingOwner,
        driver: &'a mut dyn BlockingWireDriver,
        reader: &'a mut dyn BlockingReadDriver,
        decoder: &'a mut dyn BlockingFrameDecoder,
    },
}

impl fmt::Debug for BlockingReceiptControl<'_> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.debug_struct("BlockingReceiptControl").finish()
    }
}

impl<'a> BlockingReceiptControl<'a> {
    pub(crate) fn shared(host: &'a dyn BlockingControlHost) -> Self {
        Self {
            kind: BlockingControlKind::Shared(host),
        }
    }

    #[cfg(all(test, not(feature = "async")))]
    fn with_parts<T>(
        &mut self,
        operation: impl FnOnce(
            &mut BlockingOwner,
            &mut dyn BlockingWireDriver,
            &mut dyn BlockingReadDriver,
            &mut dyn BlockingFrameDecoder,
        ) -> Result<T, Error>,
    ) -> Result<T, Error> {
        match &mut self.kind {
            BlockingControlKind::Shared(_) => Err(Error::InvalidState(
                "shared blocking controls do not expose mutable owner parts".into(),
            )),
            #[cfg(all(test, not(feature = "async")))]
            BlockingControlKind::Borrowed {
                owner,
                driver,
                reader,
                decoder,
            } => operation(owner, *driver, *reader, *decoder),
        }
    }

    pub(crate) fn now(&self) -> Instant {
        match &self.kind {
            BlockingControlKind::Shared(host) => host.now(),
            #[cfg(all(test, not(feature = "async")))]
            BlockingControlKind::Borrowed { .. } => Instant::now(),
        }
    }

    pub(crate) fn sleep(&self, duration: Duration) {
        match &self.kind {
            BlockingControlKind::Shared(host) => host.sleep(duration),
            #[cfg(all(test, not(feature = "async")))]
            BlockingControlKind::Borrowed { .. } => std::thread::sleep(duration),
        }
    }

    pub(crate) fn deadline_after(&self, timeout: Duration) -> Result<Instant, Error> {
        match &self.kind {
            BlockingControlKind::Shared(host) => host.deadline_after(timeout),
            #[cfg(all(test, not(feature = "async")))]
            BlockingControlKind::Borrowed { .. } => observer_deadline(self.now(), timeout),
        }
    }

    fn owner_matches(&mut self, origin: &Arc<()>) -> Result<bool, Error> {
        match &self.kind {
            BlockingControlKind::Shared(host) => host.owner_matches(origin),
            #[cfg(all(test, not(feature = "async")))]
            BlockingControlKind::Borrowed { .. } => {
                self.with_parts(|owner, _, _, _| Ok(Arc::ptr_eq(origin, &owner.state.origin)))
            }
        }
    }

    fn pump_once_until(&mut self, deadline: Option<Instant>) -> Result<usize, Error> {
        match &self.kind {
            BlockingControlKind::Shared(host) => host.pump_once_until(deadline),
            #[cfg(all(test, not(feature = "async")))]
            BlockingControlKind::Borrowed { .. } => {
                self.with_parts(|owner, driver, reader, decoder| {
                    owner.pump_once_until(driver, reader, decoder, deadline)
                })
            }
        }
    }

    fn submit_inquiry_until<R>(
        &mut self,
        prepared: crate::prepared::PreparedInquiry<R>,
        deadline: Instant,
    ) -> Result<BlockingInquiryReceipt<R>, Error> {
        match &self.kind {
            BlockingControlKind::Shared(host) => {
                let (request, decoder, timeout) = prepared.into_parts();
                host.submit_inquiry_until(request, timeout, deadline)
                    .map(|core| BlockingInquiryReceipt { core, decoder })
            }
            #[cfg(all(test, not(feature = "async")))]
            BlockingControlKind::Borrowed { .. } => {
                self.with_parts(|owner, driver, reader, decoder| {
                    let (request, response_decoder, timeout) = prepared.into_parts();
                    owner
                        .submit_request_until_with_pump(
                            driver, reader, decoder, request, timeout, deadline,
                        )
                        .map(|core| BlockingInquiryReceipt {
                            core,
                            decoder: response_decoder,
                        })
                })
            }
        }
    }

    /// Records cancellation intent, returning the operation receipt intact
    /// when the owner refuses it (#612).
    ///
    /// The large `Err` variant is the point: it is the caller's observation
    /// right travelling back rather than being destroyed. The public
    /// `blocking::Operation::cancel` boxes it into `CancelRejected` before it
    /// reaches a caller, so no public `Result` carries this size.
    #[allow(clippy::result_large_err)]
    pub(crate) fn cancel_operation<K>(
        &mut self,
        receipt: BlockingOperationReceipt<K>,
    ) -> Result<BlockingCancellationReceipt, (Option<BlockingOperationReceipt<K>>, Error)>
    where
        K: completion::Kind,
    {
        let BlockingOperationReceipt {
            core,
            affected_axes,
            settlement,
            marker,
        } = receipt;
        let rebuild = |core| BlockingOperationReceipt {
            core,
            affected_axes,
            settlement,
            marker,
        };
        let outcome = match &self.kind {
            BlockingControlKind::Shared(host) => host.cancel_operation(core),
            #[cfg(all(test, not(feature = "async")))]
            BlockingControlKind::Borrowed { .. } => {
                match self.with_parts(|owner, driver, _, _| Ok(owner.cancel_core(driver, core))) {
                    Ok(outcome) => outcome,
                    Err(error) => Err(RejectedCancellation::lost(error)),
                }
            }
        };
        outcome.map_err(|RejectedCancellation { receipt, error }| (receipt.map(rebuild), error))
    }
}

/// A targeted settled selection that has not yet created its one absolute
/// observer deadline. Phase 6 consumes this value to execute the exact plan.
#[derive(Debug)]
pub(crate) struct BlockingSettlementWait<'a> {
    receipt: BlockingOperationReceipt<completion::Targeted>,
    selection: WaitSelection,
    control: BlockingReceiptControl<'a>,
}

/// Result of the applied portion of a targeted settlement wait.
// Boxing the polling continuation would add an allocation at the settlement
// boundary; this enum is an internal, short-lived owner value.
#[allow(clippy::large_enum_variant)]
#[derive(Debug)]
pub(crate) enum BlockingAfterApplied<'a> {
    Settled(BlockingReceiptControl<'a>),
    Poll(BlockingPollingContinuation<'a>),
}

/// Exact polling work delegated to Phase 6 without claiming settlement.
#[derive(Debug)]
pub(crate) struct BlockingPollingContinuation<'a> {
    pub(crate) target: crate::CameraId,
    pub(crate) axes: AffectedAxes,
    pub(crate) plan: crate::prepared::SettlementPlan,
    pub(crate) deadline: Instant,
    pub(crate) control: BlockingReceiptControl<'a>,
}

impl BlockingCommandReceipt {
    pub(crate) fn wait(self, control: &mut BlockingReceiptControl<'_>) -> Result<(), Error> {
        let timeout = self.core.configured_timeout();
        wait_core_for(self.core, control, timeout).and_then(normalize_command_outcome)
    }

    // Used only by owner unit tests.
    #[cfg(all(test, not(feature = "async")))]
    pub(crate) fn wait_with_timeout(
        self,
        control: &mut BlockingReceiptControl<'_>,
        timeout: Duration,
    ) -> Result<(), Error> {
        wait_core_for(self.core, control, timeout).and_then(normalize_command_outcome)
    }
}

impl<R> BlockingInquiryReceipt<R> {
    pub(crate) fn wait(self, control: &mut BlockingReceiptControl<'_>) -> Result<R, Error> {
        let timeout = self.core.configured_timeout();
        let outcome = wait_core_for(self.core, control, timeout)?;
        normalize_inquiry_outcome(outcome, &self.decoder)
    }

    fn wait_until(
        self,
        control: &mut BlockingReceiptControl<'_>,
        deadline: Instant,
    ) -> Result<R, Error> {
        let outcome = wait_core_until(self.core, control, deadline)?;
        normalize_inquiry_outcome(outcome, &self.decoder)
    }
}

impl<K> BlockingOperationReceipt<K>
where
    K: completion::Kind,
{
    pub(crate) fn id(&self) -> u64 {
        self.core.id().get()
    }

    pub(crate) fn applied(self, control: &mut BlockingReceiptControl<'_>) -> Result<(), Error> {
        let timeout = self.core.configured_timeout();
        wait_core_for(self.core, control, timeout).and_then(normalize_command_outcome)
    }

    pub(crate) fn applied_with_timeout(
        self,
        control: &mut BlockingReceiptControl<'_>,
        timeout: Duration,
    ) -> Result<(), Error> {
        wait_core_for(self.core, control, timeout).and_then(normalize_command_outcome)
    }

    /// Owner-level cancellation used only by src/runtime/owner/tests.rs
    /// `mod blocking`, which compiles on the blocking-without-async leg (#636).
    /// The public path goes through `BlockingReceiptControl::cancel_operation`.
    #[cfg(all(test, not(feature = "async")))]
    pub(crate) fn cancel_test<D: BlockingWireDriver + ?Sized>(
        self,
        owner: &mut BlockingOwner,
        driver: &mut D,
    ) -> Result<BlockingCancellationReceipt, RejectedCancellation> {
        owner.cancel_core(driver, self.core)
    }
}

impl BlockingOperationReceipt<completion::Targeted> {
    pub(crate) fn settled<'a>(
        self,
        control: BlockingReceiptControl<'a>,
    ) -> BlockingSettlementWait<'a> {
        let selection = WaitSelection::Configured;
        BlockingSettlementWait {
            receipt: self,
            selection,
            control,
        }
    }

    pub(crate) fn settled_with_timeout<'a>(
        self,
        control: BlockingReceiptControl<'a>,
        timeout: Duration,
    ) -> BlockingSettlementWait<'a> {
        BlockingSettlementWait {
            receipt: self,
            selection: WaitSelection::Override(timeout),
            control,
        }
    }
}

impl<'a> BlockingSettlementWait<'a> {
    pub(crate) fn wait(self) -> Result<BlockingReceiptControl<'a>, Error> {
        let budget = match self.selection {
            WaitSelection::Configured => {
                self.receipt.settlement.default_budget().ok_or_else(|| {
                    Error::InvalidState(
                        "targeted operation omitted its configured settlement budget".into(),
                    )
                })?
            }
            WaitSelection::Override(timeout) => timeout,
        };
        let deadline = self.control.deadline_after(budget)?;
        match self.wait_applied_until(deadline)? {
            BlockingAfterApplied::Settled(control) => Ok(control),
            BlockingAfterApplied::Poll(continuation) => continuation.wait(),
        }
    }

    /// Waits for application using the Phase-6-selected absolute deadline. The
    /// same deadline must be retained by any returned polling continuation.
    pub(crate) fn wait_applied_until(
        mut self,
        deadline: Instant,
    ) -> Result<BlockingAfterApplied<'a>, Error> {
        let target = self.receipt.core.target();
        let outcome = wait_core_until(self.receipt.core, &mut self.control, deadline)?;
        normalize_command_outcome(outcome)?;
        match self.receipt.settlement.into_plan()?.into_inner() {
            crate::prepared::SettlementPlan::CompletionIsSettled {
                target: plan_target,
                ..
            } => {
                debug_assert_eq!(plan_target, target);
                Ok(BlockingAfterApplied::Settled(self.control))
            }
            plan @ crate::prepared::SettlementPlan::Poll {
                target: plan_target,
                axes,
                ..
            } => {
                debug_assert_eq!(plan_target, target);
                debug_assert_eq!(axes, self.receipt.affected_axes);
                Ok(BlockingAfterApplied::Poll(BlockingPollingContinuation {
                    target,
                    axes: self.receipt.affected_axes,
                    plan,
                    deadline,
                    control: self.control,
                }))
            }
        }
    }

    #[cfg(all(test, not(feature = "async")))]
    pub(crate) const fn selection(&self) -> WaitSelection {
        self.selection
    }
}

impl<'a> BlockingPollingContinuation<'a> {
    fn wait(mut self) -> Result<BlockingReceiptControl<'a>, Error> {
        let crate::prepared::SettlementPlan::Poll {
            target,
            queries,
            axes,
            tolerance,
            interval,
            ..
        } = self.plan
        else {
            return Err(Error::InvalidState(
                "polling continuation carried a non-poll settlement plan".into(),
            ));
        };
        debug_assert_eq!(target, self.target);
        debug_assert_eq!(axes, self.axes);
        let mut detector = crate::prepared::MotionDetector::new(axes, tolerance);
        let baseline = sample_positions_blocking(&mut self.control, &queries, self.deadline)?;
        if detector.observe(baseline)? != crate::prepared::MotionState::NeedSample {
            return Err(Error::InvalidState(
                "new movement detector rejected its baseline snapshot".into(),
            ));
        }
        loop {
            pump_until_sample_boundary(&mut self.control, interval, self.deadline)?;
            let snapshot = sample_positions_blocking(&mut self.control, &queries, self.deadline)?;
            match detector.observe(snapshot)? {
                crate::prepared::MotionState::Settled => return Ok(self.control),
                crate::prepared::MotionState::Moving => {}
                crate::prepared::MotionState::NeedSample => {
                    return Err(Error::InvalidState(
                        "movement detector lost its baseline snapshot".into(),
                    ));
                }
            }
        }
    }
}

fn pump_until_sample_boundary(
    control: &mut BlockingReceiptControl<'_>,
    interval: Duration,
    deadline: Instant,
) -> Result<(), Error> {
    let now = control.now();
    if now >= deadline {
        return Err(Error::Timeout);
    }
    let sample_at = now.checked_add(interval).unwrap_or(deadline).min(deadline);
    while control.now() < sample_at {
        // No receipt is being observed between two samples, so the pump's own
        // error is the only verdict available here. `pump_once_until` reports
        // the session boundary error whenever this turn ended the session, so a
        // settlement wait interrupted by a dead transport still classifies as
        // needing a replacement session (#629).
        control.pump_once_until(Some(sample_at))?;
    }
    if control.now() >= deadline {
        Err(Error::Timeout)
    } else {
        Ok(())
    }
}

pub(crate) fn sample_positions_blocking(
    control: &mut BlockingReceiptControl<'_>,
    queries: &crate::prepared::PositionQueryPlan,
    deadline: Instant,
) -> Result<crate::prepared::PositionSnapshot, Error> {
    let mut snapshot = crate::prepared::PositionSnapshot::default();
    if let Some(query) = &queries.pan_tilt {
        ensure_before_deadline(control, deadline)?;
        let receipt = control.submit_inquiry_until(query.instantiate(), deadline)?;
        snapshot.pan_tilt = Some(receipt.wait_until(control, deadline)?);
        ensure_before_deadline(control, deadline)?;
    }
    if let Some(query) = &queries.zoom {
        ensure_before_deadline(control, deadline)?;
        let receipt = control.submit_inquiry_until(query.instantiate(), deadline)?;
        snapshot.zoom = Some(receipt.wait_until(control, deadline)?);
        ensure_before_deadline(control, deadline)?;
    }
    if let Some(query) = &queries.focus {
        ensure_before_deadline(control, deadline)?;
        let receipt = control.submit_inquiry_until(query.instantiate(), deadline)?;
        snapshot.focus = Some(receipt.wait_until(control, deadline)?);
        ensure_before_deadline(control, deadline)?;
    }
    if let Some(query) = &queries.iris {
        ensure_before_deadline(control, deadline)?;
        let receipt = control.submit_inquiry_until(query.instantiate(), deadline)?;
        snapshot.iris = Some(receipt.wait_until(control, deadline)?);
        ensure_before_deadline(control, deadline)?;
    }
    if let Some(query) = &queries.nd_filter {
        ensure_before_deadline(control, deadline)?;
        let receipt = control.submit_inquiry_until(query.instantiate(), deadline)?;
        snapshot.nd_filter = Some(receipt.wait_until(control, deadline)?);
        ensure_before_deadline(control, deadline)?;
    }
    Ok(snapshot)
}

fn ensure_before_deadline(
    control: &BlockingReceiptControl<'_>,
    deadline: Instant,
) -> Result<(), Error> {
    if control.now() >= deadline {
        Err(Error::Timeout)
    } else {
        Ok(())
    }
}

impl BlockingCancellationReceipt {
    #[cfg(all(test, feature = "blocking"))]
    pub(crate) fn recv_test(
        self,
    ) -> Result<crate::runtime::engine::CancellationObservation, Error> {
        self.core.recv().map(test_cancellation_observation)
    }

    pub(crate) fn outcome(
        mut self,
        control: &mut BlockingReceiptControl<'_>,
        timeout: Duration,
    ) -> Result<CancellationOutcome, Error> {
        if !control.owner_matches(&self.core.origin)? {
            return Err(Error::InvalidState(
                "cancellation receipt belongs to a different owner".into(),
            ));
        }
        let deadline = control.deadline_after(timeout)?;
        loop {
            // An observation that was already buffered before this turn is
            // authoritative regardless of whether this caller's observer
            // deadline has subsequently elapsed.
            if let Some(observation) = self.core.try_observation() {
                return normalize_cancellation_observation(observation);
            }
            // Do not begin a fresh receive after an already-expired observer
            // deadline, but retain equality for the input-at-deadline rule.
            if control.now() > deadline {
                return Err(Error::Timeout);
            }
            let pump_result = control.pump_once_until(Some(deadline));
            // The receive turn may have fed a valid terminal frame to the
            // engine after the caller's exact observer bound. Snapshot once
            // after that turn: an input at equality wins, a strictly late
            // observation remains in the engine but is invisible to this
            // bounded observer.
            let observed_after_pump = control.now();
            // This cancellation's own terminal observation wins over the pump's
            // verdict; without one, the pump reports the session boundary error
            // rather than the raw transport cause (#629). That precedence
            // applies only while this observer is on time.
            if observed_after_pump <= deadline {
                if let Some(observation) = self.core.try_observation() {
                    return normalize_cancellation_observation(observation);
                }
            }
            // In particular, do not hide a session-replacement verdict behind
            // a local observer timeout (#629).
            pump_result?;
            if observed_after_pump >= deadline {
                return Err(Error::Timeout);
            }
        }
    }
}

#[cfg(test)]
fn test_cancellation_observation(
    observation: ReceiptObservation,
) -> crate::runtime::engine::CancellationObservation {
    match observation {
        ReceiptObservation::Terminal(RuntimeOutcome::Written) => {
            crate::runtime::engine::CancellationObservation::Failed(Error::InvalidState(
                "a local write outcome cannot authorize cancellation".into(),
            ))
        }
        ReceiptObservation::Terminal(RuntimeOutcome::Applied) => {
            crate::runtime::engine::CancellationObservation::Completed
        }
        ReceiptObservation::Terminal(RuntimeOutcome::Cancelled) => {
            crate::runtime::engine::CancellationObservation::Cancelled
        }
        ReceiptObservation::Terminal(RuntimeOutcome::Failed(error))
        | ReceiptObservation::CancellationFailed(error) => {
            crate::runtime::engine::CancellationObservation::Failed(error)
        }
        ReceiptObservation::Terminal(RuntimeOutcome::Reply { .. }) => {
            crate::runtime::engine::CancellationObservation::Failed(Error::InvalidState(
                "an inquiry outcome cannot authorize cancellation".into(),
            ))
        }
    }
}

fn submission_observation_error(observation: ReceiptObservation) -> Error {
    match observation {
        ReceiptObservation::Terminal(RuntimeOutcome::Failed(error))
        | ReceiptObservation::CancellationFailed(error) => error,
        ReceiptObservation::Terminal(RuntimeOutcome::Cancelled) => Error::CommandCanceled,
        ReceiptObservation::Terminal(RuntimeOutcome::Applied) => {
            Error::InvalidState("unwritten blocking submission completed as applied".into())
        }
        ReceiptObservation::Terminal(RuntimeOutcome::Written) => {
            Error::InvalidState("unwritten blocking submission completed as locally written".into())
        }
        ReceiptObservation::Terminal(RuntimeOutcome::Reply { .. }) => Error::InvalidState(
            "unwritten blocking submission completed with an inquiry reply".into(),
        ),
    }
}

fn buffered_submission_error(completion: &CompletionObserver) -> Option<Error> {
    completion.try_recv().map(submission_observation_error)
}

/// Submission behavior at the caller's admission boundary.
///
/// Ordinary blocking receipts may be admitted before their first write and
/// remain in the owner's bounded ready queue. A public operation handle is
/// different: it must name a request whose initial write already succeeded,
/// so losing the first-dispatch race rejects that request immediately.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SubmitPolicy {
    QueueAllowed,
    RequireFirstWrite,
}

/// Controls whether a receive turn may hand a newly available socket to
/// ordinary queued work. Submission-side waits suppress that final scheduler
/// step until the submitting request's first-write result has been decided.
/// That includes the raw pre-ACK drain (issue #673) and a raw terminal
/// tombstone hold: both need input-first due work, but neither may hand a
/// freed lane to an ordinary peer before the submitting request is
/// re-evaluated.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PumpMode {
    Normal,
    PreAckDrain,
    FirstDispatchWait,
}

impl PumpMode {
    const fn allows_ordinary_dispatch(self) -> bool {
        matches!(self, Self::Normal)
    }

    /// A raw tombstone wait owns a stronger boundary than the other pump
    /// modes: no deadline work may release correlation until retained stream
    /// framing has either completed or been discarded.
    const fn defers_due(self) -> bool {
        matches!(self, Self::FirstDispatchWait)
    }
}

/// Internal result of one receive/decode turn. Public pump seams retain their
/// established `Result<usize>` contract; tombstone waits query retained stream
/// framing through [`BlockingFrameDecoder`] instead of inferring it from this
/// turn's byte count.
#[derive(Debug, Clone, Copy)]
struct PumpProgress {
    decoded_frames: usize,
}

/// A raw correlation deadline that has become visible to the caller-thread
/// owner, but has not yet earned its one receive-first release turn.
///
/// Unlike the async actor, a blocking owner has no independently running
/// receive task.  A synchronous write or control call can therefore return at
/// the deadline before its next explicit pump.  Keeping this exact projection
/// latched makes every non-receive path suppress due work until a later pump
/// has either consumed input or observed genuine post-boundary no-input.
#[derive(Debug, Clone, Copy)]
struct RawReleaseInputGate {
    releases: RawCorrelationReleaseSet,
    deferrals: usize,
    await_until: Option<Instant>,
}

/// A pathological nonblocking test adapter or an always-ready peer must not
/// turn one raw tombstone boundary into unbounded caller-thread work. Normal
/// blocking reads remain deadline-bounded; this cap is the second bound.
const RAW_TOMBSTONE_PUMP_WORK_LIMIT: usize = 64;

/// A past raw-release wake cannot be handed straight back to a blocking
/// adapter: many correctly return `TimedOut` without touching the wire. Give
/// the mandatory post-H probe a small positive ceiling so it performs one real
/// read while retaining the caller-thread work bound.
const RAW_RELEASE_PROBE_MAX_WAIT: Duration = Duration::from_millis(1);

fn wait_core_for(
    core: ReceiptCore,
    control: &mut BlockingReceiptControl<'_>,
    timeout: Duration,
) -> Result<RuntimeOutcome, Error> {
    let deadline = control.deadline_after(timeout)?;
    wait_core_until(core, control, deadline)
}

fn wait_core_until(
    core: ReceiptCore,
    control: &mut BlockingReceiptControl<'_>,
    deadline: Instant,
) -> Result<RuntimeOutcome, Error> {
    if !control.owner_matches(&core.origin)? {
        return Err(Error::InvalidState(
            "receipt belongs to a different owner".into(),
        ));
    }
    loop {
        // A receipt outcome already buffered before this turn is not a new
        // wait, so it remains observable even after the caller's deadline.
        if let Some(outcome) = core.try_outcome() {
            return Ok(outcome);
        }
        // Preserve equality so a frame supplied exactly at the observer
        // deadline is still an input that this observer may consume.
        if control.now() > deadline {
            return Err(Error::Timeout);
        }
        let pump_result = control.pump_once_until(Some(deadline));
        // One clock snapshot decides whether a newly produced outcome belongs
        // to this observer. The engine has already consumed every decoded
        // frame regardless, so a strictly late terminal result remains its
        // authoritative lifecycle result while this observer times out.
        let observed_after_pump = control.now();
        if observed_after_pump <= deadline {
            if let Some(outcome) = core.try_outcome() {
                return Ok(outcome);
            }
        }
        // Preserve the pump's boundary verdict before declaring an observer
        // timeout: a terminal session must still ask its caller to replace it
        // rather than report the raw cause or silently mask that state (#629).
        pump_result?;
        if observed_after_pump >= deadline {
            return Err(Error::Timeout);
        }
    }
}

fn observer_deadline(now: Instant, timeout: Duration) -> Result<Instant, Error> {
    now.checked_add(timeout)
        .ok_or_else(|| Error::InvalidParameter {
            parameter: "observer timeout",
            value: format!("{timeout:?}").into(),
            reason: "duration exceeds the monotonic clock range".into(),
        })
}

/// Sleeps until an owner deadline, tolerating an early platform wake without
/// turning a no-data fake into a busy loop.
fn sleep_until(deadline: Instant) {
    while let Some(remaining) = deadline.checked_duration_since(Instant::now()) {
        if remaining.is_zero() {
            break;
        }
        std::thread::sleep(remaining);
    }
}

#[derive(Debug, Default)]
pub(crate) struct DriveReport {
    writes: Vec<(RequestId, Result<(), Error>)>,
    /// A session boundary reached while draining this effect batch.  Writes
    /// can poison a stream after an otherwise successful receive or timer
    /// turn, so callers must not infer that an `Ok` transport read leaves the
    /// owner usable.
    boundary_error: Option<Error>,
}

impl DriveReport {
    fn first_write_for(&self, id: RequestId) -> Option<Result<(), Error>> {
        self.writes
            .iter()
            .find(|(request, _)| *request == id)
            .map(|(_, result)| result.clone())
    }

    fn boundary_error(&self) -> Option<Error> {
        self.boundary_error.clone()
    }

    fn observe_boundary(&mut self, boundary_error: Option<Error>) {
        if self.boundary_error.is_none() {
            self.boundary_error = boundary_error;
        }
    }
}

/// A serialized owner. It never creates a worker thread and therefore cannot
/// accidentally pump ACK/completion while returning from submission.
#[derive(Debug)]
pub(crate) struct BlockingOwner {
    state: OwnerState,
    pumping: bool,
    /// A due raw release may not advance through a synchronous write, control,
    /// or scheduler turn before the next receive has established the exact
    /// input-first boundary.
    raw_release_input_gate: Option<RawReleaseInputGate>,
    /// One consecutive run of transient receive faults. A persistent run is
    /// eventually a dead transport, not a condition a caller-thread pump can
    /// recover by retrying forever.
    faults: TransientFaultRun,
}

impl BlockingOwner {
    pub(crate) fn new(policy: OwnerPolicy) -> Result<Self, Error> {
        Ok(Self {
            state: OwnerState::new(policy)?,
            pumping: false,
            raw_release_input_gate: None,
            faults: TransientFaultRun::default(),
        })
    }

    pub(crate) const fn state(&self) -> &OwnerState {
        &self.state
    }

    /// Checks the session-wide admission bound without retaining a permit.
    ///
    /// Operation submission performs this non-waiting probe before the narrow
    /// raw pre-ACK drain. The owner is caller-thread serialized, so the actual
    /// admission immediately afterward cannot lose the permit to another
    /// blocking submission.
    fn ensure_admission_capacity(&mut self, target: crate::CameraId) -> Result<(), Error> {
        let permits = self.state.permits();
        let Some(probe) = permits.try_acquire() else {
            let error = Error::RuntimeQueueFull {
                capacity: permits.capacity(),
            };
            self.state
                .record_admission_rejection(target, RequestLane::Command, &error);
            return Err(error);
        };
        drop(probe);
        Ok(())
    }

    /// Mutably accesses the owner state for caller-thread control operations.
    pub(crate) fn state_mut(&mut self) -> &mut OwnerState {
        &mut self.state
    }

    /// Latches the complete raw-correlation release set visible at `now`.
    /// The latch is intentionally retained if a non-receive control path
    /// changes the engine enough that its *current* due projection becomes
    /// empty: that path still did not inspect the wire, so it has not earned a
    /// successor dispatch.
    fn raw_release_gate_at(&mut self, now: Instant) -> Option<RawCorrelationReleaseSet> {
        let releases = self.state.raw_correlation_releases_due(now);
        if let Some(gate) = self.raw_release_input_gate {
            // A control turn may change the current engine projection while
            // this exact older release still lacks a wire observation. Never
            // replace it here: the pump will classify it first, then start a
            // separate fresh probe for any newly visible distinct set.
            return Some(gate.releases);
        }
        if releases.is_empty() {
            return None;
        }
        self.raw_release_input_gate = Some(RawReleaseInputGate {
            releases,
            deferrals: 0,
            await_until: None,
        });
        Some(releases)
    }

    fn raw_release_gate_pending(&self) -> bool {
        self.raw_release_input_gate.is_some()
    }

    /// Counts bounded receive-first attempts only while one exact release set
    /// remains unresolved.  A malformed datagram or a transient read fault is
    /// not an empty-input proof, so it leaves this gate armed; after the fixed
    /// bound the session fails closed rather than letting a successor bind
    /// stale evidence.
    fn defer_raw_release_gate<D: BlockingWireDriver + ?Sized>(
        &mut self,
        driver: &mut D,
        reason: &'static str,
    ) -> Result<(), Error> {
        let Some(gate) = &mut self.raw_release_input_gate else {
            return Ok(());
        };
        gate.deferrals = gate.deferrals.saturating_add(1);
        if gate.deferrals < RAW_TOMBSTONE_PUMP_WORK_LIMIT {
            return Ok(());
        }
        Err(self.poison_raw_release_gate(driver, Error::InvalidState(reason.into())))
    }

    fn clear_raw_release_gate(&mut self) {
        self.raw_release_input_gate = None;
    }

    // Builds the borrowed control used only by owner unit tests.
    #[cfg(all(test, not(feature = "async")))]
    pub(crate) fn receipt_control<'a, D, R, F>(
        &'a mut self,
        driver: &'a mut D,
        reader: &'a mut R,
        decoder: &'a mut F,
    ) -> BlockingReceiptControl<'a>
    where
        D: BlockingWireDriver,
        R: BlockingReadDriver,
        F: BlockingFrameDecoder,
    {
        BlockingReceiptControl {
            kind: BlockingControlKind::Borrowed {
                owner: self,
                driver,
                reader,
                decoder,
            },
        }
    }

    /// Class-specific typed admission seam for ordinary commands.
    #[cfg(test)]
    pub(crate) fn submit_command<D: BlockingWireDriver + ?Sized>(
        &mut self,
        driver: &mut D,
        prepared: crate::prepared::PreparedCommand,
    ) -> Result<BlockingCommandReceipt, Error> {
        prepared.admit_with(|request, timeout| {
            self.submit_with_timeout(driver, request, timeout)
                .map(|core| BlockingCommandReceipt { core })
        })
    }

    /// Class-specific typed admission seam retaining the external decoder.
    #[cfg(test)]
    #[allow(dead_code)]
    pub(crate) fn submit_inquiry<D, R>(
        &mut self,
        driver: &mut D,
        prepared: crate::prepared::PreparedInquiry<R>,
    ) -> Result<BlockingInquiryReceipt<R>, Error>
    where
        D: BlockingWireDriver + ?Sized,
    {
        prepared.admit_with(|request, decoder, timeout| {
            self.submit_with_timeout(driver, request, timeout)
                .map(|core| BlockingInquiryReceipt { core, decoder })
        })
    }

    #[cfg(all(test, not(feature = "async")))]
    pub(crate) fn submit_inquiry_until<D: BlockingWireDriver + ?Sized, R>(
        &mut self,
        driver: &mut D,
        prepared: crate::prepared::PreparedInquiry<R>,
        deadline: Instant,
    ) -> Result<BlockingInquiryReceipt<R>, Error> {
        prepared.admit_with(|request, decoder, timeout| {
            self.submit_with_timeout_until(driver, request, timeout, deadline)
                .map(|core| BlockingInquiryReceipt { core, decoder })
        })
    }

    /// Deadline-bound production submission through the authoritative input
    /// pump. Settlement polling uses this path, so a fresh inquiry cannot
    /// sleep through a preceding raw-correlation tombstone.
    pub(crate) fn submit_request_until_with_pump<D, R, F>(
        &mut self,
        driver: &mut D,
        reader: &mut R,
        decoder: &mut F,
        request: RuntimeRequest,
        configured_timeout: Duration,
        deadline: Instant,
    ) -> Result<ReceiptCore, Error>
    where
        D: BlockingWireDriver + ?Sized,
        R: BlockingReadDriver + ?Sized,
        F: BlockingFrameDecoder + ?Sized,
    {
        self.submit_with_timeout_until_pumped(
            driver,
            reader,
            decoder,
            request,
            configured_timeout,
            deadline,
        )
    }

    /// Drain the raw single-candidate pre-ACK gate before an
    /// ACK-then-completion operation's first-write submit, when that gate alone
    /// blocks a new command on `target` (issue #673).
    ///
    /// On a raw-VISCA target the engine normally keeps one *unacknowledged*
    /// command in flight so a socketless ACK cannot be misattributed. While a
    /// caller holds an un-awaited operation whose command is still awaiting its
    /// ACK, a second ordinary operation would lose the first-dispatch race even
    /// though a command socket is free the instant that ACK lands. This pumps
    /// the owner until the gate clears, bounded by the submitting request's own
    /// ACK budget, so the subsequent first write wins. An intrinsic `Urgent`
    /// operation never enters this drain: #714 gives it an explicit
    /// two-candidate safety lane whose ACK evidence fails closed.
    ///
    /// It is deliberately narrow. A completion-only successor still needs the
    /// target to be entirely idle after a predecessor ACK, so that ACK is not
    /// its sole obstacle and no pump is attempted. When the block is genuine
    /// socket-capacity contention — every command socket already occupied,
    /// independent of the pre-ACK gate —
    /// [`OwnerState::raw_preack_gate_frees_socket_on_ack`] is `false`, no pump
    /// is attempted, and the fail-fast rejection the caller then receives from
    /// the first-write submit stands. If the pump ends the session (a close or
    /// poison observed while waiting), the session's own boundary verdict is
    /// returned rather than the raw transport cause, so an auto-reconnect loop
    /// keyed on `requires_new_session()` still behaves (issue #629).
    fn drain_raw_preack_gate<D, R, F>(
        &mut self,
        driver: &mut D,
        reader: &mut R,
        decoder: &mut F,
        target: crate::CameraId,
        ack_budget: Duration,
    ) -> Result<(), Error>
    where
        D: BlockingWireDriver + ?Sized,
        R: BlockingReadDriver + ?Sized,
        F: BlockingFrameDecoder + ?Sized,
    {
        if !self.state.raw_preack_gate_frees_socket_on_ack(target) {
            return Ok(());
        }
        let Some(deadline) = Instant::now().checked_add(ack_budget) else {
            return Ok(());
        };
        self.enter()?;
        let result = self.drain_raw_preack_gate_inner(driver, reader, decoder, target, deadline);
        self.leave();
        result.map_err(|error| self.boundary_error_or(error))
    }

    /// Test-only seam for asserting that a submission path does (or does not)
    /// enter the bounded pre-ACK drain. Production submissions reach the
    /// private method through [`BlockingSessionHost::submit_operation`].
    #[cfg(all(test, not(feature = "async")))]
    pub(crate) fn drain_raw_preack_gate_for_test<D, R, F>(
        &mut self,
        driver: &mut D,
        reader: &mut R,
        decoder: &mut F,
        target: crate::CameraId,
        ack_budget: Duration,
    ) -> Result<(), Error>
    where
        D: BlockingWireDriver + ?Sized,
        R: BlockingReadDriver + ?Sized,
        F: BlockingFrameDecoder + ?Sized,
    {
        self.drain_raw_preack_gate(driver, reader, decoder, target, ack_budget)
    }

    fn drain_raw_preack_gate_inner<D, R, F>(
        &mut self,
        driver: &mut D,
        reader: &mut R,
        decoder: &mut F,
        target: crate::CameraId,
        deadline: Instant,
    ) -> Result<(), Error>
    where
        D: BlockingWireDriver + ?Sized,
        R: BlockingReadDriver + ?Sized,
        F: BlockingFrameDecoder + ?Sized,
    {
        // Each `pump_once_inner` blocks no later than `deadline`, so this loop
        // cannot spin; it exits when the pending ACK clears the gate, when the
        // budget elapses (the first write then fails fast), or when the pump
        // itself ends the session.
        while self.state.raw_preack_gate_frees_socket_on_ack(target) && Instant::now() < deadline {
            self.pump_once_inner(
                driver,
                reader,
                decoder,
                Some(deadline),
                PumpMode::PreAckDrain,
            )?;
        }
        Ok(())
    }

    /// Class-specific typed admission seam retaining operation semantics.
    #[cfg(test)]
    #[allow(dead_code)]
    pub(crate) fn submit_operation<D, K>(
        &mut self,
        driver: &mut D,
        prepared: crate::prepared::PreparedOperation<K>,
    ) -> Result<BlockingOperationReceipt<K>, Error>
    where
        D: BlockingWireDriver + ?Sized,
        K: completion::Kind,
    {
        prepared.admit_with(|request, affected_axes, settlement, timeout| {
            self.submit_with_timeout_policy(
                driver,
                request,
                timeout,
                SubmitPolicy::RequireFirstWrite,
            )
            .map(|core| BlockingOperationReceipt {
                core,
                affected_axes,
                settlement,
                marker: PhantomData,
            })
        })
    }

    /// Production operation admission with the input pump needed for an exact
    /// first-dispatch wake.
    pub(crate) fn submit_operation_with_pump<D, R, F, K>(
        &mut self,
        driver: &mut D,
        reader: &mut R,
        decoder: &mut F,
        prepared: crate::prepared::PreparedOperation<K>,
    ) -> Result<BlockingOperationReceipt<K>, Error>
    where
        D: BlockingWireDriver + ?Sized,
        R: BlockingReadDriver + ?Sized,
        F: BlockingFrameDecoder + ?Sized,
        K: completion::Kind,
    {
        prepared.admit_with(|request, affected_axes, settlement, timeout| {
            self.submit_with_timeout_policy_pumped(
                driver,
                reader,
                decoder,
                request,
                timeout,
                SubmitPolicy::RequireFirstWrite,
            )
            .map(|core| BlockingOperationReceipt {
                core,
                affected_axes,
                settlement,
                marker: PhantomData,
            })
        })
    }

    /// Production inquiry admission with the authoritative input pump.
    pub(crate) fn submit_inquiry_with_pump<D, R, F, Response>(
        &mut self,
        driver: &mut D,
        reader: &mut R,
        frame_decoder: &mut F,
        prepared: crate::prepared::PreparedInquiry<Response>,
    ) -> Result<BlockingInquiryReceipt<Response>, Error>
    where
        D: BlockingWireDriver + ?Sized,
        R: BlockingReadDriver + ?Sized,
        F: BlockingFrameDecoder + ?Sized,
    {
        prepared.admit_with(|request, decoder, timeout| {
            self.submit_with_timeout_pumped(driver, reader, frame_decoder, request, timeout)
                .map(|core| BlockingInquiryReceipt { core, decoder })
        })
    }

    /// Production command admission has the authoritative reader and decoder
    /// available when a first-write race reaches an exact scheduler wake.
    pub(crate) fn submit_command_with_pump<D, R, F>(
        &mut self,
        driver: &mut D,
        reader: &mut R,
        decoder: &mut F,
        prepared: crate::prepared::PreparedCommand,
    ) -> Result<BlockingCommandReceipt, Error>
    where
        D: BlockingWireDriver + ?Sized,
        R: BlockingReadDriver + ?Sized,
        F: BlockingFrameDecoder + ?Sized,
    {
        prepared.admit_with(|request, timeout| {
            self.submit_with_timeout_pumped(driver, reader, decoder, request, timeout)
                .map(|core| BlockingCommandReceipt { core })
        })
    }

    /// Admit and perform the first write when this exact request wins the
    /// global dispatch race. Ordinary receipts that cannot win yet stay
    /// queued in the engine and are written by a later owner turn. This
    /// owner-only test seam has no reader/decoder; production submission uses
    /// the pumped variants above if it reaches a deterministic `WaitUntil`.
    // Untyped admission seam used only by owner unit tests.
    #[cfg(test)]
    pub(crate) fn submit<D: BlockingWireDriver>(
        &mut self,
        driver: &mut D,
        request: RuntimeRequest,
    ) -> Result<ReceiptCore, Error> {
        let timeout = if request.is_inquiry() {
            request.context().timeout.inquiry
        } else {
            request.context().timeout.completion
        };
        self.submit_with_timeout(driver, request, timeout)
    }

    #[cfg(test)]
    fn submit_with_timeout<D: BlockingWireDriver + ?Sized>(
        &mut self,
        driver: &mut D,
        request: RuntimeRequest,
        configured_timeout: Duration,
    ) -> Result<ReceiptCore, Error> {
        self.submit_with_timeout_policy(
            driver,
            request,
            configured_timeout,
            SubmitPolicy::QueueAllowed,
        )
    }

    fn submit_with_timeout_pumped<D, R, F>(
        &mut self,
        driver: &mut D,
        reader: &mut R,
        decoder: &mut F,
        request: RuntimeRequest,
        configured_timeout: Duration,
    ) -> Result<ReceiptCore, Error>
    where
        D: BlockingWireDriver + ?Sized,
        R: BlockingReadDriver + ?Sized,
        F: BlockingFrameDecoder + ?Sized,
    {
        self.submit_with_timeout_policy_pumped(
            driver,
            reader,
            decoder,
            request,
            configured_timeout,
            SubmitPolicy::QueueAllowed,
        )
    }

    #[cfg(test)]
    fn submit_with_timeout_policy<D: BlockingWireDriver + ?Sized>(
        &mut self,
        driver: &mut D,
        request: RuntimeRequest,
        configured_timeout: Duration,
        submit_policy: SubmitPolicy,
    ) -> Result<ReceiptCore, Error> {
        self.submit_with_timeout_policy_with_wait(
            driver,
            request,
            configured_timeout,
            None,
            submit_policy,
            |owner, driver, reason, dispatch_at, observer_deadline| {
                owner.wait_for_first_dispatch_without_pump(
                    driver,
                    reason,
                    dispatch_at,
                    observer_deadline,
                )
            },
        )
    }

    fn submit_with_timeout_policy_pumped<D, R, F>(
        &mut self,
        driver: &mut D,
        reader: &mut R,
        decoder: &mut F,
        request: RuntimeRequest,
        configured_timeout: Duration,
        submit_policy: SubmitPolicy,
    ) -> Result<ReceiptCore, Error>
    where
        D: BlockingWireDriver + ?Sized,
        R: BlockingReadDriver + ?Sized,
        F: BlockingFrameDecoder + ?Sized,
    {
        self.submit_with_timeout_policy_with_wait(
            driver,
            request,
            configured_timeout,
            None,
            submit_policy,
            |owner, driver, reason, dispatch_at, observer_deadline| {
                owner.wait_for_first_dispatch_pumped(
                    driver,
                    reader,
                    decoder,
                    reason,
                    dispatch_at,
                    observer_deadline,
                )
            },
        )
    }

    #[cfg(test)]
    #[allow(dead_code)]
    fn submit_with_timeout_until<D: BlockingWireDriver + ?Sized>(
        &mut self,
        driver: &mut D,
        request: RuntimeRequest,
        configured_timeout: Duration,
        deadline: Instant,
    ) -> Result<ReceiptCore, Error> {
        self.submit_with_timeout_until_policy(
            driver,
            request,
            configured_timeout,
            deadline,
            SubmitPolicy::QueueAllowed,
        )
    }

    fn submit_with_timeout_until_pumped<D, R, F>(
        &mut self,
        driver: &mut D,
        reader: &mut R,
        decoder: &mut F,
        request: RuntimeRequest,
        configured_timeout: Duration,
        deadline: Instant,
    ) -> Result<ReceiptCore, Error>
    where
        D: BlockingWireDriver + ?Sized,
        R: BlockingReadDriver + ?Sized,
        F: BlockingFrameDecoder + ?Sized,
    {
        self.submit_with_timeout_policy_with_wait(
            driver,
            request,
            configured_timeout,
            Some(deadline),
            SubmitPolicy::QueueAllowed,
            |owner, driver, reason, dispatch_at, observer_deadline| {
                owner.wait_for_first_dispatch_pumped(
                    driver,
                    reader,
                    decoder,
                    reason,
                    dispatch_at,
                    observer_deadline,
                )
            },
        )
    }

    #[cfg(test)]
    #[allow(dead_code)]
    fn submit_with_timeout_until_policy<D: BlockingWireDriver + ?Sized>(
        &mut self,
        driver: &mut D,
        request: RuntimeRequest,
        configured_timeout: Duration,
        deadline: Instant,
        submit_policy: SubmitPolicy,
    ) -> Result<ReceiptCore, Error> {
        self.submit_with_timeout_policy_with_wait(
            driver,
            request,
            configured_timeout,
            Some(deadline),
            submit_policy,
            |owner, driver, reason, dispatch_at, observer_deadline| {
                owner.wait_for_first_dispatch_without_pump(
                    driver,
                    reason,
                    dispatch_at,
                    observer_deadline,
                )
            },
        )
    }

    /// Earliest time a first-dispatch wait may consume. The wait's own reason
    /// supplies one bound, but an already-admitted request can have an earlier
    /// retry budget, ACK/completion deadline, or cancellation wake. A caller
    /// observer bound remains an independent ceiling.
    fn first_dispatch_wait_deadline(
        &self,
        dispatch_at: Instant,
        observer_deadline: Option<Instant>,
    ) -> Instant {
        min_deadline(self.state.next_wake_without_dispatch(), observer_deadline)
            .map_or(dispatch_at, |earlier| dispatch_at.min(earlier))
    }

    /// Test-only fallback for the owner seams that have no reader/decoder.
    /// Pacing has no input authority and can safely use the same no-dispatch
    /// deadline service as production. A raw tombstone cannot: advancing it
    /// without first consuming a boundary frame could bind stale input to the
    /// successor, so this seam fails closed.
    #[cfg(test)]
    fn wait_for_first_dispatch_without_pump<D: BlockingWireDriver + ?Sized>(
        &mut self,
        driver: &mut D,
        reason: FirstDispatchWait,
        dispatch_at: Instant,
        observer_deadline: Option<Instant>,
    ) -> Result<(), Error> {
        match reason {
            FirstDispatchWait::Pacing => {
                self.wait_for_pacing_without_receive(driver, dispatch_at, observer_deadline)
            }
            FirstDispatchWait::RawCorrelationTombstone => Err(Error::TransportBusy),
        }
    }

    /// Authoritative production first-dispatch wait. Only a raw correlation
    /// tombstone receives from the wire; ordinary pacing must leave peer
    /// replies queued for their own receipt waits.
    fn wait_for_first_dispatch_pumped<D, R, F>(
        &mut self,
        driver: &mut D,
        reader: &mut R,
        decoder: &mut F,
        reason: FirstDispatchWait,
        dispatch_at: Instant,
        observer_deadline: Option<Instant>,
    ) -> Result<(), Error>
    where
        D: BlockingWireDriver + ?Sized,
        R: BlockingReadDriver + ?Sized,
        F: BlockingFrameDecoder + ?Sized,
    {
        match reason {
            FirstDispatchWait::RawCorrelationTombstone => self.wait_for_raw_correlation_tombstone(
                driver,
                reader,
                decoder,
                dispatch_at,
                observer_deadline,
            ),
            FirstDispatchWait::Pacing => {
                self.wait_for_pacing_without_receive(driver, dispatch_at, observer_deadline)
            }
        }
    }

    /// Sleeps through an ordinary pacing wait without receiving, then services
    /// due work and cancellations while ordinary dispatch stays suppressed.
    /// The caller re-evaluates the exact request afterwards, so an equal total
    /// budget terminalizes before that request can write.
    fn wait_for_pacing_without_receive<D: BlockingWireDriver + ?Sized>(
        &mut self,
        driver: &mut D,
        dispatch_at: Instant,
        observer_deadline: Option<Instant>,
    ) -> Result<(), Error> {
        let deadline = self.first_dispatch_wait_deadline(dispatch_at, observer_deadline);
        sleep_until(deadline);
        self.service_due_without_dispatch(driver)
    }

    /// Pumps a raw correlation tombstone at its bounded owner deadline. Frames
    /// are applied before the dispatch-suppressed due pass, so a stale frame at
    /// the exact hold expiry is inert before the target lane can release.
    fn wait_for_raw_correlation_tombstone<D, R, F>(
        &mut self,
        driver: &mut D,
        reader: &mut R,
        decoder: &mut F,
        dispatch_at: Instant,
        observer_deadline: Option<Instant>,
    ) -> Result<(), Error>
    where
        D: BlockingWireDriver + ?Sized,
        R: BlockingReadDriver + ?Sized,
        F: BlockingFrameDecoder + ?Sized,
    {
        let deadline = self.first_dispatch_wait_deadline(dispatch_at, observer_deadline);
        let mut turns = 0_usize;
        loop {
            let progress = self
                .pump_once_inner(
                    driver,
                    reader,
                    decoder,
                    Some(deadline),
                    PumpMode::FirstDispatchWait,
                )
                .map_err(|error| self.boundary_error_or(error))?;
            turns = turns.saturating_add(1);
            let now = Instant::now();
            // `pump_once_inner` owns both complete-frame replay and retained
            // prefix classification.  A gate still pending means the most
            // recent read was an early idle, a transient fault, or unresolved
            // stream evidence; never turn that into due work merely because a
            // caller-side sleep reached H.
            if self.raw_release_gate_pending() || self.raw_release_gate_at(now).is_some() {
                if let Some(await_until) = self
                    .raw_release_input_gate
                    .and_then(|gate| gate.await_until)
                    .filter(|await_until| *await_until > now)
                {
                    sleep_until(
                        observer_deadline.map_or(await_until, |observer| observer.min(await_until)),
                    );
                }
                if turns >= RAW_TOMBSTONE_PUMP_WORK_LIMIT {
                    return Err(self.poison_raw_release_gate(
                        driver,
                        Error::InvalidState(
                            "blocking raw release receive work cap exhausted before input-first fence"
                                .into(),
                        ),
                    ));
                }
                continue;
            }

            if now >= deadline {
                break;
            }
            if progress.decoded_frames > 0 {
                // A peer that keeps yielding complete stale frames cannot
                // monopolize the caller thread until H. The same fixed cap
                // applies before and after the deadline, and it is scoped to
                // this one raw correlation wait.
                if turns >= RAW_TOMBSTONE_PUMP_WORK_LIMIT {
                    return Err(self.poison_raw_release_gate(
                        driver,
                        Error::InvalidState(
                            "blocking raw release receive work cap exhausted while complete input remained pending"
                                .into(),
                        ),
                    ));
                }
                continue;
            }
            // A reader may return an idle result before its requested deadline
            // (for example because transport read_timeout is smaller). Sleep
            // only to pace the next *fresh* receive; that earlier idle is not
            // a boundary fence.
            sleep_until(deadline);
        }
        self.service_due_without_dispatch(driver)
    }

    /// Resolve every already-buffered fragment that the current release set
    /// can prove stale, without taking another wire read between fragments.
    /// This keeps an arrival after the discarded fragment on the normal
    /// successor path, while still handling several stale delimiter-framed
    /// inputs retained in one production framer buffer.
    fn resolve_due_raw_prefixes<D, F>(
        &mut self,
        driver: &mut D,
        decoder: &mut F,
        now: Instant,
    ) -> Result<RawReleaseGateAction, Error>
    where
        D: BlockingWireDriver + ?Sized,
        F: BlockingFrameDecoder + ?Sized,
    {
        let mut discarded = 0_usize;
        loop {
            let buffered = decoder
                .has_buffered_stream_input()
                .map_err(|error| self.poison_tombstone_decoder(driver, error))?;
            let evidence = decoder
                .buffered_raw_prefix_evidence()
                .map_err(|error| self.poison_tombstone_decoder(driver, error))?;
            if !buffered {
                if evidence.is_some() {
                    return Err(self.poison_tombstone_decoder(
                        driver,
                        Error::InvalidState(
                            "blocking stream decoder described absent buffered input".into(),
                        ),
                    ));
                }
                // Every matching stale fragment has been removed. It is safe
                // to release due work before consuming an as-yet-unread tail.
                return Ok(self.state.resolve_raw_release_gate(now, None));
            }
            let evidence = evidence.ok_or_else(|| {
                self.poison_tombstone_decoder(
                    driver,
                    Error::InvalidState(
                        "blocking raw stream input could not be classified before correlation release"
                            .into(),
                    ),
                )
            })?;
            match self.state.resolve_raw_release_gate(now, Some(evidence)) {
                RawReleaseGateAction::DiscardFirst => {
                    if discarded >= RAW_TOMBSTONE_PUMP_WORK_LIMIT {
                        return Err(self.poison_tombstone_decoder(
                            driver,
                            Error::InvalidState(
                                "blocking raw correlation release framing work cap exhausted"
                                    .into(),
                            ),
                        ));
                    }
                    decoder
                        .discard_buffered_stream_input()
                        .map_err(|error| self.poison_tombstone_decoder(driver, error))?;
                    let _ = self
                        .state
                        .apply_effect(Effect::Ignored(IgnoreReason::MalformedFrame));
                    discarded = discarded.saturating_add(1);
                }
                action => return Ok(action),
            }
        }
    }

    /// Losing access to framing state makes safe raw-correlation release
    /// impossible. End the stream through the owner rather than returning a
    /// decoder error while leaving a runnable session behind.
    fn poison_tombstone_decoder<D: BlockingWireDriver + ?Sized>(
        &mut self,
        driver: &mut D,
        error: Error,
    ) -> Error {
        self.poison_raw_release_gate(driver, error)
    }

    /// Terminalizes a release whose receive-first proof can no longer be made
    /// safely.  This is shared by decoder failures and the bounded ordinary
    /// pump gate, so neither path can return an apparently usable owner after
    /// abandoning stale raw evidence.
    fn poison_raw_release_gate<D: BlockingWireDriver + ?Sized>(
        &mut self,
        driver: &mut D,
        error: Error,
    ) -> Error {
        let effects = self.input_for_mode(
            Input::Poison {
                reason: error.to_string().into_boxed_str(),
            },
            Instant::now(),
            PumpMode::FirstDispatchWait,
        );
        let _ = self.drive_for_mode(driver, effects, PumpMode::FirstDispatchWait);
        self.boundary_error_or(error)
    }

    /// Runs only due/cancellation work if a no-dispatch wake has arrived.
    /// This is shared by pacing and raw tombstone waits so neither can locally
    /// bypass a total budget or hand a newly free socket to an ordinary peer.
    fn service_due_without_dispatch<D: BlockingWireDriver + ?Sized>(
        &mut self,
        driver: &mut D,
    ) -> Result<(), Error> {
        let now = Instant::now();
        if self
            .state
            .next_wake_without_dispatch()
            .is_some_and(|wake| wake <= now)
        {
            // A pacing sleep has no wire authority.  If a raw release became
            // due while sleeping, leave it latched for the receive-first pump
            // rather than releasing it through this control-only path.
            let effects = self.advance_for_mode(now, PumpMode::FirstDispatchWait);
            let report = self.drive_for_mode(driver, effects, PumpMode::FirstDispatchWait);
            if let Some(error) = report.boundary_error() {
                return Err(error);
            }
        }
        Ok(())
    }

    fn submit_with_timeout_policy_with_wait<D, W>(
        &mut self,
        driver: &mut D,
        request: RuntimeRequest,
        configured_timeout: Duration,
        observer_deadline: Option<Instant>,
        submit_policy: SubmitPolicy,
        wait_for_dispatch: W,
    ) -> Result<ReceiptCore, Error>
    where
        D: BlockingWireDriver + ?Sized,
        W: FnMut(
            &mut Self,
            &mut D,
            FirstDispatchWait,
            Instant,
            Option<Instant>,
        ) -> Result<(), Error>,
    {
        // Match the async owner boundary: once the caller-owned observer
        // deadline has elapsed, reject before staging admission or writing a
        // new inquiry.
        if observer_deadline.is_some_and(|deadline| Instant::now() >= deadline) {
            return Err(Error::Timeout);
        }
        self.enter()?;
        let result = self.submit_inner(
            driver,
            request,
            configured_timeout,
            observer_deadline,
            submit_policy,
            wait_for_dispatch,
        );
        self.leave();
        result
    }

    fn submit_inner<D, W>(
        &mut self,
        driver: &mut D,
        request: RuntimeRequest,
        configured_timeout: Duration,
        observer_deadline: Option<Instant>,
        submit_policy: SubmitPolicy,
        mut wait_for_dispatch: W,
    ) -> Result<ReceiptCore, Error>
    where
        D: BlockingWireDriver + ?Sized,
        W: FnMut(
            &mut Self,
            &mut D,
            FirstDispatchWait,
            Instant,
            Option<Instant>,
        ) -> Result<(), Error>,
    {
        let target = request.context().target;
        let lane = if request.is_inquiry() {
            RequestLane::Inquiry
        } else {
            RequestLane::Command
        };
        let permits = self.state.permits();
        let Some(permit) = permits.try_acquire() else {
            let error = Error::RuntimeQueueFull {
                capacity: permits.capacity(),
            };
            self.state.record_admission_rejection(target, lane, &error);
            return Err(error);
        };
        let origin = self.state.origin();
        let (input, completion, admission) = self.state.stage_admission(request, permit);
        let Input::Admit { ticket, request } = input else {
            return Err(Error::InvalidState(
                "staged blocking admission did not produce an admit input".into(),
            ));
        };
        let effects = self
            .state
            .admit_without_due(ticket, request, Instant::now());
        let mut report = self.drive_without_due(driver, effects);
        let id = admission.recv().map_err(|_| Error::RuntimeShutdown)??;

        // Session admission capacity bounds how much work may be outstanding.
        // Ordinary receipts keep the historical queueing
        // behavior when they lose the global dispatch race; operation handles
        // use the stricter first-write contract below.
        let mut queued = false;
        while report.first_write_for(id).is_none() {
            if let Some(error) = buffered_submission_error(&completion) {
                return Err(error);
            }
            let now = Instant::now();
            if observer_deadline.is_some_and(|deadline| now >= deadline) {
                if matches!(submit_policy, SubmitPolicy::RequireFirstWrite) {
                    // A public operation handle cannot be lost behind an
                    // expired caller bound. Unlike QueueAllowed settlement
                    // inquiries, it has no receipt to keep observing, so
                    // terminalize its still-unwritten entry and release the
                    // admission permit through the engine's one authority.
                    let rejection = self.state.reject_unwritten_without_due(id, Error::Timeout);
                    let _ = self.drive_without_due(driver, rejection);
                    return Err(buffered_submission_error(&completion).unwrap_or(Error::Timeout));
                }
                return Err(Error::Timeout);
            }
            // `first_dispatch_without_due` deliberately does not advance a
            // tombstone.  It can nevertheless dispatch an *unrelated* ready
            // target, so do not enter it while a prior synchronous write or
            // control turn has crossed any raw-release boundary.  The pumped
            // production path receives first; the no-reader test seam fails
            // closed through its existing tombstone wait fallback.
            if self.raw_release_gate_at(now).is_some() {
                if let Err(error) = wait_for_dispatch(
                    self,
                    driver,
                    FirstDispatchWait::RawCorrelationTombstone,
                    now,
                    observer_deadline,
                ) {
                    let rejection = self.state.reject_unwritten_without_due(id, error.clone());
                    let _ = self.drive_without_due(driver, rejection);
                    return Err(buffered_submission_error(&completion).unwrap_or(error));
                }
                continue;
            }
            match self.state.first_dispatch_without_due(id, now) {
                FirstDispatch::Effects(effects) => {
                    let advanced = self.drive_without_due(driver, effects.into());
                    report.writes.extend(advanced.writes);
                }
                FirstDispatch::WaitUntil { deadline, reason } => {
                    // A raw correlation tombstone is the only first-dispatch
                    // wait permitted to receive: its boundary needs an ordered
                    // input turn so an already-buffered stale reply is inert
                    // before due work frees the target. Ordinary pacing is a
                    // pure clock wait; receiving there would consume a peer's
                    // ACK/completion and violate the first-write admission
                    // boundary. Both paths suppress ordinary dispatch while
                    // servicing due/cancellation work, then re-evaluate this
                    // exact request.
                    if let Err(error) =
                        wait_for_dispatch(self, driver, reason, deadline, observer_deadline)
                    {
                        // A first-dispatch wait fails before this request has
                        // written or exposed a receipt, regardless of its
                        // submission policy. Terminalize the exact ready
                        // entry through the engine so its queue ticket and
                        // admission permit cannot outlive the returned error.
                        let rejection = self.state.reject_unwritten_without_due(id, error.clone());
                        let _ = self.drive_without_due(driver, rejection);
                        return Err(buffered_submission_error(&completion).unwrap_or(error));
                    }
                }
                FirstDispatch::Blocked => {
                    match submit_policy {
                        SubmitPolicy::QueueAllowed => {
                            // Another request owns the only eligible socket
                            // right now. Leave this ordinary receipt queued;
                            // no peer request, deadline, pacing, or
                            // cancellation state is mutated here.
                            queued = true;
                            break;
                        }
                        SubmitPolicy::RequireFirstWrite => {
                            // A public operation handle may not escape for an
                            // unwritten request. Terminalize this entry via
                            // the engine so its queue ticket and admission
                            // permit are released, then observe the exact
                            // rejection through the temporary completion
                            // observer. No peer is pumped or waited on.
                            let rejection = self
                                .state
                                .reject_unwritten_without_due(id, Error::TransportBusy);
                            let _ = self.drive_without_due(driver, rejection);
                            return Err(buffered_submission_error(&completion).unwrap_or_else(
                                || {
                                    Error::InvalidState(
                                        "unwritten blocking operation rejection was not observed"
                                            .into(),
                                    )
                                },
                            ));
                        }
                    }
                }
                FirstDispatch::Missing => {
                    if let Some(error) = buffered_submission_error(&completion) {
                        return Err(error);
                    }
                    return Err(Error::InvalidState(
                        "unwritten blocking submission disappeared before dispatch".into(),
                    ));
                }
            }
        }

        if !queued {
            let first_write = report.first_write_for(id).ok_or_else(|| {
                Error::InvalidState("blocking request lost its first-write result".into())
            })?;
            if let Err(error) = first_write {
                // A failed write can have terminalized the session (for
                // example a stream poison). Preserve that engine verdict over
                // the raw driver error, exactly as the prior submission path
                // did; only a successful write may retain an immediate
                // `NoReply` `Written` observation for the returned receipt.
                return Err(buffered_submission_error(&completion).unwrap_or(error));
            }
        } else if let Some(error) = buffered_submission_error(&completion) {
            // A queued receipt has not reached the wire, so any terminal
            // observation remains a failed submission. Once a first write has
            // succeeded, however, a `NoReply` command legitimately resolves
            // `Written` in that same owner turn; leave that observation for
            // the returned receipt to consume.
            return Err(error);
        }
        Ok(ReceiptCore::new(
            id,
            target,
            completion,
            configured_timeout,
            origin,
        ))
    }

    // Frame-replay seam used only by owner unit tests.
    #[cfg(all(test, not(feature = "async")))]
    pub(crate) fn inject_frame<D: BlockingWireDriver>(
        &mut self,
        driver: &mut D,
        frame: DecodedFrame,
        now: Instant,
    ) -> Result<(), Error> {
        self.enter()?;
        let effects = self.state.input(Input::Frame(frame), now);
        let _ = self.drive(driver, effects);
        self.leave();
        Ok(())
    }

    /// Perform one raw receive, then frame/decode completely outside the engine
    /// mutation, and finally replay decoded frames in source order.
    // Raw receive seam used only by owner unit tests.
    #[cfg(test)]
    pub(crate) fn pump_once<D, R, F>(
        &mut self,
        driver: &mut D,
        reader: &mut R,
        decoder: &mut F,
    ) -> Result<usize, Error>
    where
        D: BlockingWireDriver + ?Sized,
        R: BlockingReadDriver + ?Sized,
        F: BlockingFrameDecoder + ?Sized,
    {
        self.pump_once_until(driver, reader, decoder, None)
    }

    /// Pump with a caller-owned observer deadline. The deadline only bounds
    /// waiting; it never becomes an engine cancellation or scheduler input.
    pub(crate) fn pump_once_until<D, R, F>(
        &mut self,
        driver: &mut D,
        reader: &mut R,
        decoder: &mut F,
        observer_deadline: Option<Instant>,
    ) -> Result<usize, Error>
    where
        D: BlockingWireDriver + ?Sized,
        R: BlockingReadDriver + ?Sized,
        F: BlockingFrameDecoder + ?Sized,
    {
        self.enter()?;
        let result =
            self.pump_once_inner(driver, reader, decoder, observer_deadline, PumpMode::Normal);
        self.leave();
        // Issue #629: whatever ended the session inside this one pump turn, the
        // caller must be told the session's own boundary verdict and never the
        // raw transport or framing cause that produced it. A raw cause
        // classifies as survivable (`Error::requires_new_session() == false`)
        // while the session is already `Closed`/`Poisoned`, so an auto-reconnect
        // loop keyed on that predicate would not rebuild. This is the single
        // choke point every pump caller shares, so no observation path can
        // reintroduce the misclassification.
        result
            .map(|progress| progress.decoded_frames)
            .map_err(|error| self.boundary_error_or(error))
    }

    /// The session's terminal boundary verdict once a boundary input has been
    /// applied, falling back to `error` while the session is still usable.
    fn boundary_error_or(&self, error: Error) -> Error {
        self.state.boundary_error().unwrap_or(error)
    }

    fn pump_once_inner<D, R, F>(
        &mut self,
        driver: &mut D,
        reader: &mut R,
        decoder: &mut F,
        observer_deadline: Option<Instant>,
        mode: PumpMode,
    ) -> Result<PumpProgress, Error>
    where
        D: BlockingWireDriver + ?Sized,
        R: BlockingReadDriver + ?Sized,
        F: BlockingFrameDecoder + ?Sized,
    {
        // A blocking write/control turn may already have crossed H.  Arm the
        // gate before this read, but only a result sampled at/after H can
        // release it; an adapter is allowed to time out before owner_deadline.
        let receive_started_at = Instant::now();
        let raw_gate_before_receive = self.raw_release_gate_at(receive_started_at).is_some();
        let mut owner_deadline = min_deadline(self.next_wake_for_mode(mode), observer_deadline);
        let raw_release_wait = self
            .raw_release_input_gate
            .and_then(|gate| gate.await_until);
        if raw_gate_before_receive && raw_release_wait.is_some_and(|wait| wait > receive_started_at)
        {
            // The engine has retained an ambiguous prefix under the old scope.
            // Give its tail one real, shared grace interval instead of handing
            // the already-due protocol wake back to the reader (#713).
            owner_deadline = min_deadline(raw_release_wait, observer_deadline);
        } else if raw_gate_before_receive
            && owner_deadline.is_some_and(|deadline| deadline <= receive_started_at)
        {
            // `BlockingTransportReader` quite correctly returns TimedOut
            // without entering the transport when handed a past deadline. At
            // a latched raw release that would be a synthetic idle, not an
            // input probe. Replace the expired wake with one small positive
            // probe ceiling (or an earlier still-live observer ceiling), so
            // the driver really enters its bounded read path.
            let probe_deadline = receive_started_at
                .checked_add(RAW_RELEASE_PROBE_MAX_WAIT)
                .unwrap_or(receive_started_at);
            owner_deadline = min_deadline(
                observer_deadline.filter(|deadline| *deadline > receive_started_at),
                Some(probe_deadline),
            );
        }
        let read = reader.receive(self.state.buffers().receive_mut(), owner_deadline);
        // An error that only reports "no bytes arrived" is an idle read, not a
        // fault: it consumed nothing and must not burn any request's retry
        // budget. The adapter normalizes this too; doing it here as well keeps
        // every read driver on one contract (#637).
        let read = match read {
            Err(error) if super::receive_reported_no_data(&error) => Ok(BlockingReceive::TimedOut),
            other => other,
        };
        let (received, received_at, no_input) = match read {
            Ok(BlockingReceive::TimedOut) => {
                // A no-data return becomes a fence only after the stream
                // framer has also shown that no partial bytes are retained.
                (0, Instant::now(), true)
            }
            Ok(BlockingReceive::Bytes(0)) => {
                let effects = self.input_for_mode(
                    Input::Shutdown(ShutdownReason::TransportClosed { reason: None }),
                    Instant::now(),
                    mode,
                );
                let _ = self.drive_for_mode(driver, effects, mode);
                return Err(Error::ConnectionClosed { reason: None });
            }
            Ok(BlockingReceive::Bytes(received)) => {
                // Any successful read breaks a transient-fault run, even when
                // the bytes only complete a later frame.
                self.faults.reset();
                (received, Instant::now(), false)
            }
            Err(Error::ResponseTooLarge { .. })
                if self.state.policy().protocol.transport == TransportKind::Datagram =>
            {
                // Built-in UDP reports this only after consuming one datagram
                // whose tail did not fit in the receive buffer.  A legacy
                // custom datagram transport may report the same established
                // spelling.  In either case the copied prefix must never reach
                // framing; treat it exactly like a malformed atomic datagram,
                // keep the session running, and let a later packet decode.
                self.faults.reset();
                let _ = self
                    .state
                    .apply_effect(Effect::Ignored(IgnoreReason::MalformedFrame));
                // A malformed/oversized datagram was consumed; it cannot
                // certify that a due raw release saw no input.
                if self.raw_release_gate_at(Instant::now()).is_some() {
                    self.defer_raw_release_gate(
                        driver,
                        "blocking raw release remained unresolved after oversized datagram",
                    )?;
                }
                return Ok(PumpProgress { decoded_frames: 0 });
            }
            Err(error) if super::receive_fault_is_transient(&error) => {
                let received_at = Instant::now();
                let raw_gate = self.raw_release_gate_at(received_at).is_some();
                let (length, span) = self.faults.record(received_at);
                if TransientFaultRun::is_permanent(length, span) {
                    // Twelve consecutive faults spanning at least one second
                    // are a broken adapter rather than a transient condition.
                    // Close through the owner so every outstanding observer
                    // receives the session boundary error, retaining both the
                    // run count and the underlying transport cause.
                    let effects = self.input_for_mode(
                        Input::Close {
                            reason: Some(
                                format!("{length} consecutive receive faults: {error}")
                                    .into_boxed_str(),
                            ),
                        },
                        received_at,
                        mode,
                    );
                    let _ = self.drive_for_mode(driver, effects, mode);
                    return Err(self.boundary_error_or(error));
                }
                // The engine safely retries sequenced Sony work with its same
                // sequence; a raw command awaiting ACK is left to its own ACK
                // deadline (issue #671; the strict opt-in poisons instead)
                // rather than being replayed. The read consumed nothing, so
                // framing state is intact and this pump simply produced no
                // frames.
                let effects = self.input_for_mode(Input::ReceiveFault { error }, received_at, mode);
                let report = self.drive_for_mode(
                    driver,
                    effects,
                    if raw_gate {
                        PumpMode::FirstDispatchWait
                    } else {
                        mode
                    },
                );
                if let Some(error) = report.boundary_error() {
                    return Err(error);
                }
                if raw_gate {
                    // A genuine transient fault is not an idle fence: stale
                    // input may still be ready behind it.
                    self.defer_raw_release_gate(
                        driver,
                        "blocking raw release receive work cap exhausted after transient faults",
                    )?;
                }
                pause_after_transient_receive_fault(length, owner_deadline);
                return Ok(PumpProgress { decoded_frames: 0 });
            }
            Err(error) => {
                // A fatal read proves the connection is gone; it says
                // nothing about the byte-stream *position*, which is what
                // poison means. Framing failures below still poison a
                // stream, exactly as the async owner does.
                let effects = self.input_for_mode(
                    Input::Close {
                        reason: Some(error.to_string().into_boxed_str()),
                    },
                    Instant::now(),
                    mode,
                );
                let _ = self.drive_for_mode(driver, effects, mode);
                // Report the close, not the raw read fault that caused it —
                // the same verdict the zero-byte arm above returns. The
                // cause survives in the close reason.
                return Err(self.boundary_error_or(error));
            }
        };

        let frame_limit = self.state.policy().limits.frames_per_receive;
        let is_stream = self.state.policy().protocol.transport == TransportKind::Stream;
        // If H was reached before this receive completed, replay complete
        // frames without their normal due tail.  Retained stream evidence is
        // classified below, then exactly one fenced tail may release it.
        let raw_gate = self.raw_release_gate_at(received_at).is_some();
        let processing_mode = if raw_gate {
            PumpMode::FirstDispatchWait
        } else {
            mode
        };
        // The first pass decodes the bytes just read. On a stream, subsequent
        // passes drain (received == 0) any complete frames a receive that hit
        // the per-receive frame limit left buffered, so a burst larger than one
        // batch is fully attributed in this pump instead of stalling until more
        // bytes happen to arrive (#674).
        let mut input_len = received;
        let mut driven = 0usize;
        // Datagram input is atomic and a true zero-data receive retains none,
        // so it is already the exact empty-input fence. A stream must still
        // call its framer with zero new bytes: it may hold a complete frame or
        // a partial prefix from an earlier read.
        if !no_input || is_stream {
            loop {
                let frames = match decoder.decode(self.state.buffers(), input_len, frame_limit) {
                    Ok(frames) => frames,
                    Err(error) => {
                        if is_stream {
                            let effects = self.input_for_mode(
                                Input::Poison {
                                    reason: error.to_string().into_boxed_str(),
                                },
                                Instant::now(),
                                mode,
                            );
                            let _ = self.drive_for_mode(driver, effects, mode);
                            return Err(error);
                        }
                        // A datagram is an atomic receive boundary. Malformed
                        // framing/decoding discards that whole datagram and leaves
                        // the owner Running so the next datagram can be attempted.
                        let _ = self
                            .state
                            .apply_effect(Effect::Ignored(IgnoreReason::MalformedFrame));
                        if raw_gate {
                            self.defer_raw_release_gate(
                            driver,
                            "blocking raw release receive work cap exhausted after malformed datagram",
                        )?;
                        }
                        return Ok(PumpProgress {
                            decoded_frames: driven,
                        });
                    }
                };
                // #672: a stream tolerates a delimited frame that did not classify by
                // discarding it and staying Running, exactly as a datagram already
                // does and as 1.x did (log-and-continue). Record one Ignored per
                // discarded frame so the discard stays observable.
                let discarded_malformed = self.state.buffers().take_discarded_malformed();
                for _ in 0..discarded_malformed {
                    let _ = self
                        .state
                        .apply_effect(Effect::Ignored(IgnoreReason::MalformedFrame));
                }
                if let Err(error) = self.state.validate_frame_batch(&frames) {
                    if is_stream {
                        let effects = self.input_for_mode(
                            Input::Poison {
                                reason: error.to_string().into_boxed_str(),
                            },
                            Instant::now(),
                            mode,
                        );
                        let _ = self.drive_for_mode(driver, effects, mode);
                        return Err(error);
                    }
                    let _ = self
                        .state
                        .apply_effect(Effect::Ignored(IgnoreReason::MalformedFrame));
                    if raw_gate {
                        self.defer_raw_release_gate(
                        driver,
                        "blocking raw release receive work cap exhausted after invalid datagram batch",
                    )?;
                    }
                    return Ok(PumpProgress {
                        decoded_frames: driven,
                    });
                }
                let count = frames.len();
                let report = self.drive_decoded_batch_with_mode(
                    driver,
                    frames,
                    received_at,
                    processing_mode,
                );
                driven = driven.saturating_add(count);
                if let Some(error) = report.boundary_error() {
                    return Err(error);
                }
                // Only a stream buffers a remainder, and only a batch that filled the
                // limit can have left one; drain and drive it without reading again.
                if is_stream && frame_limit > 0 && count >= frame_limit {
                    input_len = 0;
                    continue;
                }
                break;
            }
        }

        if raw_gate {
            // A no-data read proves an empty transport only if stream framing
            // has no retained bytes.  Complete frames were applied above in
            // source order; a partial prefix is delegated to the engine's
            // exact raw scope classifier before the release tail runs.
            let Some(gate_releases) = self.raw_release_input_gate.map(|gate| gate.releases) else {
                return Err(self.poison_raw_release_gate(
                    driver,
                    Error::InvalidState(
                        "blocking raw release gate disappeared before retained input was resolved"
                            .into(),
                    ),
                ));
            };
            let gate_action = if is_stream
                && decoder
                    .has_buffered_stream_input()
                    .map_err(|error| self.poison_tombstone_decoder(driver, error))?
            {
                self.resolve_due_raw_prefixes(driver, decoder, received_at)?
            } else {
                self.state.resolve_raw_release_gate(received_at, None)
            };
            match gate_action {
                RawReleaseGateAction::DiscardFirst => {
                    return Err(self.poison_raw_release_gate(
                        driver,
                        Error::InvalidState(
                            "blocking raw prefix classifier returned an unresolved discard".into(),
                        ),
                    ));
                }
                RawReleaseGateAction::AwaitInputUntil(deadline) => {
                    if let Some(gate) = &mut self.raw_release_input_gate {
                        gate.await_until = Some(deadline);
                    }
                    return Ok(PumpProgress {
                        decoded_frames: driven,
                    });
                }
                RawReleaseGateAction::Advance => {}
            }

            // The old scope has now had a real input turn. If a non-receive
            // control path exposed a different due scope meanwhile, preserve
            // the old scope's completed proof but require a *new* input turn
            // for the replacement before releasing either through due work.
            let current_releases = self.state.raw_correlation_releases_due(received_at);
            if !current_releases.is_empty() && current_releases != gate_releases {
                self.raw_release_input_gate = Some(RawReleaseInputGate {
                    releases: current_releases,
                    deferrals: 0,
                    await_until: None,
                });
                return Ok(PumpProgress {
                    decoded_frames: driven,
                });
            }

            // A valid decoded frame, a positively classified stream prefix,
            // or true no-input with no retained stream bytes earns the one due
            // tail.  Non-frame datagram input (including an empty/malformed
            // packet) must take another receive turn; it is not an H fence.
            // An atomic datagram that fails framing/validation returned above
            // through the bounded deferral path; it cannot piggyback on an
            // unrelated valid frame. A discarded *stream* delimiter is
            // different: the framer has consumed and classified that exact
            // fragment, while any response-shaped retained prefix was already
            // handled by `raw_prefix_disposition` above. It is therefore safe
            // to release after stream-only discard processing.
            let safe_to_release = driven > 0 || no_input || is_stream;
            if safe_to_release {
                self.advance_after_raw_release_input(driver, received_at, mode)?;
            } else {
                self.defer_raw_release_gate(
                    driver,
                    "blocking raw release remained unresolved after non-frame datagram input",
                )?;
            }
            return Ok(PumpProgress {
                decoded_frames: driven,
            });
        }

        // The old no-data wake path is retained only for a read sampled before
        // every raw release. `advance_for_mode` re-samples and arms a later
        // release instead of allowing this early idle to certify it.
        if no_input
            && !mode.defers_due()
            && self
                .next_wake_for_mode(mode)
                .is_some_and(|wake| wake <= received_at)
        {
            let effects = self.advance_for_mode(received_at, mode);
            let report = self.drive_for_mode(driver, effects, mode);
            if let Some(error) = report.boundary_error() {
                return Err(error);
            }
        }
        Ok(PumpProgress {
            decoded_frames: driven,
        })
    }

    fn next_wake_for_mode(&self, mode: PumpMode) -> Option<Instant> {
        if mode.allows_ordinary_dispatch() {
            self.state.next_wake()
        } else {
            self.state.next_wake_without_dispatch()
        }
    }

    fn cancel_core<D: BlockingWireDriver + ?Sized>(
        &mut self,
        driver: &mut D,
        receipt: ReceiptCore,
    ) -> Result<BlockingCancellationReceipt, RejectedCancellation> {
        if !Arc::ptr_eq(&receipt.origin, &self.state.origin) {
            return Err(RejectedCancellation::kept(
                receipt,
                Error::InvalidState("operation receipt belongs to a different owner".into()),
            ));
        }
        if let Err(error) = self.enter() {
            return Err(RejectedCancellation::kept(receipt, error));
        }
        if let Some(observation) = receipt.completion.try_recv() {
            self.leave();
            return Ok(BlockingCancellationReceipt {
                core: cancellation_receipt_for(receipt, Some(observation)),
            });
        }
        let id = receipt.id;
        let registration = self.state.register_cancellation(id);
        // Cancellation is externally ordered state, not a receive proof.  If
        // a raw correlation deadline is visible, preserve it for the next
        // pump instead of letting this control turn release a successor.
        let effects = self.input_for_mode(Input::Cancel { id }, Instant::now(), PumpMode::Normal);
        let _ = self.drive(driver, effects);
        let acknowledged = registration
            .acknowledgement
            .recv()
            .map_err(|_| Error::RuntimeShutdown);
        self.leave();
        // A refusal leaves the original request scheduled and observable, so
        // the receipt goes back to the caller rather than dying here (#612).
        match acknowledged {
            Ok(Ok(())) => Ok(BlockingCancellationReceipt {
                core: cancellation_receipt_for(receipt, None),
            }),
            Ok(Err(error)) | Err(error) => Err(RejectedCancellation::kept(receipt, error)),
        }
    }

    #[cfg(test)]
    pub(crate) fn cancel_test<D: BlockingWireDriver>(
        &mut self,
        driver: &mut D,
        receipt: ReceiptCore,
    ) -> Result<BlockingCancellationReceipt, RejectedCancellation> {
        self.cancel_core(driver, receipt)
    }

    /// Run only due scheduler work. This is intentionally distinct from a
    /// receive pump and is used for scheduler deadlines and pacing wakes.
    // Driven only by owner unit tests.
    #[cfg(test)]
    pub(crate) fn wake<D: BlockingWireDriver>(
        &mut self,
        driver: &mut D,
        now: Instant,
    ) -> Result<(), Error> {
        self.enter()?;
        // This seam has no reader, so it cannot make a raw-release
        // input-first proof.  `advance_for_mode` leaves such a gate armed.
        let effects = self.advance_for_mode(now, PumpMode::Normal);
        let _ = self.drive(driver, effects);
        self.leave();
        Ok(())
    }

    /// Installs new operational tuning through the same boundary/terminal gate
    /// every other blocking entry point takes (issue #690).
    ///
    /// `reconfigure` previously mutated owner state directly, never consulting
    /// [`Self::enter`], so `set_tuning` returned `Ok(())` on a poisoned or
    /// closed session — contradicting its documented contract that it "returns
    /// the session's terminal error if the owner is gone." Taking an `enter`
    /// turn restores that: a terminal session yields its `boundary_error()` and
    /// a re-entrant call yields [`Error::TransportBusy`], exactly as a
    /// submission would, before any tuning is applied. Facade validation is
    /// carried into this turn as a result so [`Self::enter`] also precedes a
    /// proposed update's validation error.
    pub(crate) fn reconfigure(
        &mut self,
        validated_tuning: Result<crate::OperationalTuning, Error>,
    ) -> Result<(), Error> {
        self.enter()?;
        let result = validated_tuning.and_then(|tuning| self.state_mut().retune(tuning));
        self.leave();
        result
    }

    pub(crate) fn shutdown<D: BlockingWireDriver + ?Sized>(
        &mut self,
        driver: &mut D,
    ) -> Result<(), Error> {
        // An explicit shutdown is idempotent once this owner has reached its
        // own shutdown boundary. Keep the pumping check in `enter` for every
        // other path so a re-entrant call is still rejected, and preserve
        // close/poison errors rather than treating them as another shutdown.
        if !self.pumping && matches!(self.state.boundary_error(), Some(Error::RuntimeShutdown)) {
            return Ok(());
        }
        self.enter()?;
        let effects = self
            .state
            .input(Input::Shutdown(ShutdownReason::Explicit), Instant::now());
        let _ = self.drive(driver, effects);
        self.leave();
        Ok(())
    }

    pub(crate) fn drain_diagnostics(&mut self) -> Vec<DiagnosticEvent> {
        self.state.drain_diagnostics()
    }

    fn drive<D: BlockingWireDriver + ?Sized>(
        &mut self,
        driver: &mut D,
        mut effects: VecDeque<Effect>,
    ) -> DriveReport {
        let mut report = DriveReport::default();
        while let Some(effect) = effects.pop_front() {
            if let AppliedEffect::Transmit(staged) = self.state.apply_effect(effect) {
                let write_result = match self.state.prepare_write(&staged) {
                    Ok(write) => driver.write(write),
                    Err(error) => Err(error),
                };
                report
                    .writes
                    .push((staged.request, write_result.clone().map(|_| ())));
                // A blocking transport can return after an unrelated raw
                // target's ambiguity deadline.  Sampling the completion here
                // is the linearization point: defer due work if that deadline
                // is now visible, so stale input accumulated during the write
                // gets one receive turn before any successor can be staged.
                let finished_at = Instant::now();
                let produced = if self.raw_release_gate_at(finished_at).is_some() {
                    self.state
                        .finish_write_without_due(&staged, write_result, finished_at)
                } else {
                    self.state.finish_write(&staged, write_result, finished_at)
                };
                prepend_effects(&mut effects, produced);
            }
        }
        report.observe_boundary(self.state.boundary_error());
        report
    }

    fn drive_without_due<D: BlockingWireDriver + ?Sized>(
        &mut self,
        driver: &mut D,
        mut effects: VecDeque<Effect>,
    ) -> DriveReport {
        let mut report = DriveReport::default();
        while let Some(effect) = effects.pop_front() {
            if let AppliedEffect::Transmit(staged) = self.state.apply_effect(effect) {
                let write_result = match self.state.prepare_write(&staged) {
                    Ok(write) => driver.write(write),
                    Err(error) => Err(error),
                };
                report
                    .writes
                    .push((staged.request, write_result.clone().map(|_| ())));
                let produced =
                    self.state
                        .finish_write_without_due(&staged, write_result, Instant::now());
                prepend_effects(&mut effects, produced);
            }
        }
        report.observe_boundary(self.state.boundary_error());
        report
    }

    /// Applies one non-frame pump input using the selected scheduler boundary.
    /// The pre-ACK mode goes directly through the engine's input-turn seam so
    /// owner-side effects are still replayed, but ordinary dispatch remains
    /// withheld until the submitting operation has been admitted.
    fn input_for_mode(&mut self, input: Input, now: Instant, mode: PumpMode) -> VecDeque<Effect> {
        // A receive/control input can be applied at the boundary, but no mode
        // may finish it with due work while raw input still has to be checked.
        // This covers the pre-ACK drain too: suppressing ordinary dispatch
        // alone would still release correlation for a later submission.
        if self.raw_release_gate_at(now).is_some() {
            let turn = self.state.begin_input_turn(now);
            let mut effects = self.state.input_in_turn(&turn, input);
            effects.extend(self.state.engine.finish_input_turn_without_due(turn.0));
            return effects;
        }
        match mode {
            PumpMode::Normal => self.state.input(input, now),
            PumpMode::PreAckDrain => self.state.engine.handle_without_dispatch(input, now).into(),
            PumpMode::FirstDispatchWait => {
                let turn = self.state.begin_input_turn(now);
                let mut effects = self.state.input_in_turn(&turn, input);
                effects.extend(self.state.engine.finish_input_turn_without_due(turn.0));
                effects
            }
        }
    }

    fn advance_for_mode(&mut self, now: Instant, mode: PumpMode) -> VecDeque<Effect> {
        // No caller without a receive proof may run an expiry, even when it
        // would otherwise suppress ordinary dispatch.  Cancellation and a
        // synchronous write can still be applied through their no-due seams;
        // the next pump owns the eventual release.
        if self.raw_release_gate_at(now).is_some() {
            return VecDeque::new();
        }
        if mode.allows_ordinary_dispatch() {
            self.state.advance(now)
        } else {
            self.state.engine.advance_without_dispatch(now).into()
        }
    }

    /// Completes one raw-release boundary after a real input turn has proved
    /// there is no unsafe retained evidence.  This is the only path that
    /// clears the caller-thread gate and is deliberately separate from
    /// [`Self::advance_for_mode`], whose callers have no such proof.
    fn advance_after_raw_release_input<D: BlockingWireDriver + ?Sized>(
        &mut self,
        driver: &mut D,
        now: Instant,
        mode: PumpMode,
    ) -> Result<(), Error> {
        debug_assert!(self.raw_release_gate_pending());
        let effects = if mode.allows_ordinary_dispatch() {
            self.state.advance(now)
        } else {
            self.state.engine.advance_without_dispatch(now).into()
        };
        self.clear_raw_release_gate();
        let report = self.drive_for_mode(driver, effects, mode);
        if let Some(error) = report.boundary_error() {
            return Err(error);
        }
        Ok(())
    }

    fn finish_input_turn_for_mode(
        &mut self,
        turn: OwnerInputTurn,
        mode: PumpMode,
    ) -> VecDeque<Effect> {
        match mode {
            PumpMode::Normal => self.state.finish_input_turn(turn),
            PumpMode::PreAckDrain => self
                .state
                .engine
                .finish_input_turn_without_dispatch(turn.0)
                .into(),
            // The enclosing raw-tombstone wait runs the due pass only after it
            // has inspected and, if necessary, cleared retained framing.
            PumpMode::FirstDispatchWait => self
                .state
                .engine
                .finish_input_turn_without_due(turn.0)
                .into(),
        }
    }

    fn drive_for_mode<D: BlockingWireDriver + ?Sized>(
        &mut self,
        driver: &mut D,
        effects: VecDeque<Effect>,
        mode: PumpMode,
    ) -> DriveReport {
        if mode.allows_ordinary_dispatch() {
            self.drive(driver, effects)
        } else {
            self.drive_without_due(driver, effects)
        }
    }

    /// Applies one validated decoded batch at a single owner-sampled instant.
    /// Every frame's effects, including identified write completions, are
    /// drained recursively before the next frame is applied. Due work runs
    /// exactly once after the complete source-ordered batch.
    #[cfg(all(test, feature = "blocking", not(feature = "async")))]
    pub(crate) fn drive_decoded_batch<D: BlockingWireDriver + ?Sized>(
        &mut self,
        driver: &mut D,
        frames: Vec<DecodedFrame>,
        received_at: Instant,
    ) {
        let _ = self.drive_decoded_batch_with_mode(driver, frames, received_at, PumpMode::Normal);
    }

    /// Replays a decoded batch while retaining all due/cancellation effects but
    /// optionally withholding ordinary dispatch. The pre-ACK drain uses the
    /// latter mode so a queued request cannot take the freed socket before the
    /// submitting operation is admitted.
    fn drive_decoded_batch_with_mode<D: BlockingWireDriver + ?Sized>(
        &mut self,
        driver: &mut D,
        frames: Vec<DecodedFrame>,
        received_at: Instant,
        mode: PumpMode,
    ) -> DriveReport {
        let turn = self.state.begin_input_turn(received_at);
        for frame in frames {
            let effects = self.state.input_in_turn(&turn, Input::Frame(frame));
            self.drive_in_turn(driver, &turn, effects);
        }
        let due = self.finish_input_turn_for_mode(turn, mode);
        self.drive_for_mode(driver, due, mode)
    }

    fn drive_in_turn<D: BlockingWireDriver + ?Sized>(
        &mut self,
        driver: &mut D,
        turn: &OwnerInputTurn,
        mut effects: VecDeque<Effect>,
    ) {
        while let Some(effect) = effects.pop_front() {
            if let AppliedEffect::Transmit(staged) = self.state.apply_effect(effect) {
                let write_result = match self.state.prepare_write(&staged) {
                    Ok(write) => driver.write(write),
                    Err(error) => Err(error),
                };
                let produced = self.state.finish_write_in_turn(turn, &staged, write_result);
                prepend_effects(&mut effects, produced);
            }
        }
    }

    fn enter(&mut self) -> Result<(), Error> {
        if self.pumping {
            return Err(Error::TransportBusy);
        }
        if let Some(error) = self.state.boundary_error() {
            return Err(error);
        }
        self.pumping = true;
        Ok(())
    }

    fn leave(&mut self) {
        debug_assert!(self.pumping);
        self.pumping = false;
    }

    #[cfg(all(test, not(feature = "async")))]
    // Called by `blocking_reentrancy_fails_before_admission_or_write` in
    // src/runtime/owner/tests.rs `mod blocking` (cfg blocking-without-async) (#636).
    pub(super) fn mark_pumping_for_test(&mut self) {
        self.pumping = true;
    }
}

fn pause_after_transient_receive_fault(run: u32, owner_deadline: Option<Instant>) {
    let pause = clamp_receive_pause(transient_receive_pause(run), owner_deadline, Instant::now());
    if !pause.is_zero() {
        std::thread::sleep(pause);
    }
}

fn min_deadline(left: Option<Instant>, right: Option<Instant>) -> Option<Instant> {
    match (left, right) {
        (Some(left), Some(right)) => Some(left.min(right)),
        (Some(value), None) | (None, Some(value)) => Some(value),
        (None, None) => None,
    }
}

#[cfg(all(test, not(feature = "async")))]
#[allow(clippy::expect_used)]
mod tests {
    use std::{
        cell::{Cell, RefCell},
        collections::VecDeque,
        rc::Rc,
        sync::{
            atomic::{AtomicUsize, Ordering},
            Arc,
        },
        time::{Duration, Instant},
    };

    use super::*;
    use crate::runtime::engine::{
        CancellationPolicy, ControlPolicy, DecodedResponse, EncodedMessage, EnvelopeKind,
        ProtocolPolicy, ReplyShape, RequestContext, RetryPolicy, SessionState, TargetPolicy,
        TimeoutPolicy,
    };
    use crate::{
        command::CommandKind,
        completion::AppliedOnly,
        prepared::{prepare_builtin_command, prepare_builtin_operation},
        profile::ProfileSpec,
        profiles::GenericVisca,
        request::builtin::{FocusModeCommand, ZoomDrive},
        transport::{BlockingTransport, HasTransportConfig, SendSemantics, TransportConfig},
        CameraId, ErrorKind, OperationalTuning, ViscaSocket,
    };

    #[derive(Debug, Default)]
    struct InteractionCounts {
        writes: AtomicUsize,
        receives: AtomicUsize,
    }

    #[derive(Debug)]
    struct CountingTransport {
        config: TransportConfig,
        counts: Arc<InteractionCounts>,
    }

    impl HasTransportConfig for CountingTransport {
        fn transport_config(&self) -> &TransportConfig {
            &self.config
        }
    }

    impl BlockingTransport for CountingTransport {
        fn send_with_kind(&mut self, _bytes: &[u8], _kind: CommandKind) -> Result<(), Error> {
            self.counts.writes.fetch_add(1, Ordering::SeqCst);
            Ok(())
        }

        fn recv_into(&mut self, _dst: &mut [u8]) -> Result<usize, Error> {
            self.counts.receives.fetch_add(1, Ordering::SeqCst);
            Err(Error::InvalidState(
                "unexpected receive in capacity test".into(),
            ))
        }

        fn recv_into_with_timeout(
            &mut self,
            _dst: &mut [u8],
            _timeout: Duration,
        ) -> Result<usize, Error> {
            self.counts.receives.fetch_add(1, Ordering::SeqCst);
            Err(Error::InvalidState(
                "unexpected receive in capacity test".into(),
            ))
        }

        fn send_semantics(&self) -> SendSemantics {
            SendSemantics::Datagram
        }
    }

    fn raw_owner_policy() -> OwnerPolicy {
        OwnerPolicy::single_target(
            ProtocolPolicy {
                capacity: 1,
                envelope: EnvelopeKind::Raw,
                transport: TransportKind::Datagram,
                inquiry_capacity: 1,
                command_spacing: Duration::ZERO,
                inquiry_spacing: Duration::ZERO,
                inquiry_cooldown: Duration::ZERO,
                raw_inquiry_release_hold: Duration::from_secs(1),
                raw_release_grace: Duration::from_millis(100),
                strict_unconfirmed_poison: false,
            },
            CameraId::CAMERA_1,
            TargetPolicy {
                command_sockets: 1,
                cancellation: CancellationPolicy::Supported,
            },
        )
        .expect("valid raw owner policy")
    }

    fn raw_two_target_owner_policy(transport: TransportKind) -> OwnerPolicy {
        let protocol = ProtocolPolicy {
            capacity: 3,
            envelope: EnvelopeKind::Raw,
            transport,
            inquiry_capacity: 1,
            command_spacing: Duration::ZERO,
            inquiry_spacing: Duration::ZERO,
            inquiry_cooldown: Duration::ZERO,
            raw_inquiry_release_hold: Duration::from_secs(1),
            raw_release_grace: Duration::from_millis(100),
            strict_unconfirmed_poison: false,
        };
        let mut targets = [None; 9];
        for target in [CameraId::CAMERA_1, CameraId::CAMERA_2] {
            targets[usize::from(target.id())] = Some(TargetPolicy {
                command_sockets: 1,
                cancellation: CancellationPolicy::Supported,
            });
        }
        OwnerPolicy::with_targets(protocol, targets).expect("valid two-target raw policy")
    }

    fn raw_request(
        target: CameraId,
        reply_shape: ReplyShape,
        ambiguity: Duration,
    ) -> RuntimeRequest {
        RuntimeRequest::Command {
            wire: Arc::new(
                EncodedMessage::new(&[target.to_address_byte(), 0x01, 0x04, 0x00, 0xff])
                    .expect("valid raw test command"),
            ),
            context: RequestContext {
                target,
                timeout: TimeoutPolicy {
                    ack: Duration::from_secs(1),
                    completion: Duration::from_secs(1),
                    inquiry: Duration::from_secs(1),
                    cancellation: Duration::from_secs(1),
                    ambiguity,
                },
                retry: RetryPolicy::NEVER,
                control: ControlPolicy::default(),
                cancellation: CancellationPolicy::Supported,
                reply_shape,
            },
            applied_state: None,
        }
    }

    #[derive(Debug, Default)]
    struct FaultDriver;

    impl BlockingWireDriver for FaultDriver {
        fn write(&mut self, _write: WireWrite<'_>) -> Result<TransmissionMeta, Error> {
            Ok(TransmissionMeta { sequence: None })
        }
    }

    /// Blocks exactly the second write, which is the unrelated target-C write
    /// in the raw-release regression below.  The test samples a real owner
    /// deadline; without the guarded `finish_write_without_due` call, its
    /// return immediately advances A's tombstone and writes queued B.
    #[derive(Debug)]
    struct ReleaseCrossingDriver {
        release_after: Instant,
        writes: Vec<Vec<u8>>,
    }

    impl BlockingWireDriver for ReleaseCrossingDriver {
        fn write(&mut self, write: WireWrite<'_>) -> Result<TransmissionMeta, Error> {
            self.writes.push(write.bytes.to_vec());
            if self.writes.len() == 2 {
                sleep_until(self.release_after);
            }
            Ok(TransmissionMeta { sequence: None })
        }
    }

    #[derive(Debug)]
    struct FaultReader {
        reads: VecDeque<Result<BlockingReceive, Error>>,
    }

    impl BlockingReadDriver for FaultReader {
        fn receive(
            &mut self,
            receive_buffer: &mut [u8],
            _owner_deadline: Option<Instant>,
        ) -> Result<BlockingReceive, Error> {
            let read = self.reads.pop_front().expect("scripted read");
            if matches!(read, Ok(BlockingReceive::Bytes(received)) if received > 0) {
                receive_buffer[0] = 0;
            }
            read
        }
    }

    #[derive(Debug, Default)]
    struct EarlyIdleThenDeadlineFrameReader {
        calls: usize,
    }

    impl BlockingReadDriver for EarlyIdleThenDeadlineFrameReader {
        fn receive(
            &mut self,
            receive_buffer: &mut [u8],
            owner_deadline: Option<Instant>,
        ) -> Result<BlockingReceive, Error> {
            self.calls = self.calls.saturating_add(1);
            match self.calls {
                1 => Ok(BlockingReceive::TimedOut),
                2 => {
                    let deadline = owner_deadline.expect("fresh raw probe has a deadline");
                    sleep_until(deadline);
                    receive_buffer[0] = 0;
                    Ok(BlockingReceive::Bytes(1))
                }
                _ => Err(Error::InvalidState("early-idle reader exhausted".into())),
            }
        }
    }

    #[derive(Debug, Default)]
    struct EmptyDecoder;

    impl BlockingFrameDecoder for EmptyDecoder {
        fn decode(
            &mut self,
            _buffers: &mut super::super::OwnerBuffers,
            _received: usize,
            _frame_limit: usize,
        ) -> Result<Vec<DecodedFrame>, Error> {
            Ok(Vec::new())
        }
    }

    #[derive(Debug, Default)]
    struct MalformedDatagramDecoder;

    impl BlockingFrameDecoder for MalformedDatagramDecoder {
        fn decode(
            &mut self,
            _buffers: &mut super::super::OwnerBuffers,
            _received: usize,
            _frame_limit: usize,
        ) -> Result<Vec<DecodedFrame>, Error> {
            Err(Error::InvalidState("scripted malformed datagram".into()))
        }
    }

    /// A caller-owned clock whose reader advances it only after producing a
    /// scripted frame batch. This makes the observer/engine race deterministic
    /// without teaching the engine about a test clock.
    #[derive(Clone, Debug)]
    struct ObserverClock(Rc<Cell<Instant>>);

    impl ObserverClock {
        fn new(now: Instant) -> Self {
            Self(Rc::new(Cell::new(now)))
        }

        fn now(&self) -> Instant {
            self.0.get()
        }

        fn set(&self, now: Instant) {
            self.0.set(now);
        }
    }

    #[derive(Debug)]
    struct DeadlineFrameReader {
        clock: ObserverClock,
        offset: Duration,
        delivered: bool,
        deadline: Option<Instant>,
    }

    impl DeadlineFrameReader {
        fn new(clock: ObserverClock, offset: Duration) -> Self {
            Self {
                clock,
                offset,
                delivered: false,
                deadline: None,
            }
        }
    }

    impl BlockingReadDriver for DeadlineFrameReader {
        fn receive(
            &mut self,
            receive_buffer: &mut [u8],
            owner_deadline: Option<Instant>,
        ) -> Result<BlockingReceive, Error> {
            assert!(!self.delivered, "one scripted receive per observer wait");
            let deadline = owner_deadline.expect("receipt pump carries observer deadline");
            self.deadline = Some(deadline);
            self.delivered = true;
            self.clock.set(
                deadline
                    .checked_add(self.offset)
                    .expect("test observer clock remains representable"),
            );
            receive_buffer[0] = 0;
            Ok(BlockingReceive::Bytes(1))
        }
    }

    #[derive(Debug)]
    struct DeadlineFaultReader {
        clock: ObserverClock,
        offset: Duration,
    }

    impl BlockingReadDriver for DeadlineFaultReader {
        fn receive(
            &mut self,
            _receive_buffer: &mut [u8],
            owner_deadline: Option<Instant>,
        ) -> Result<BlockingReceive, Error> {
            let deadline = owner_deadline.expect("receipt pump carries observer deadline");
            self.clock.set(
                deadline
                    .checked_add(self.offset)
                    .expect("test observer clock remains representable"),
            );
            Err(Error::Io(Arc::new(std::io::Error::from(
                std::io::ErrorKind::ConnectionReset,
            ))))
        }
    }

    #[derive(Debug)]
    struct OneBatchDecoder {
        batch: Option<Vec<DecodedFrame>>,
    }

    impl OneBatchDecoder {
        fn new(batch: Vec<DecodedFrame>) -> Self {
            Self { batch: Some(batch) }
        }
    }

    impl BlockingFrameDecoder for OneBatchDecoder {
        fn decode(
            &mut self,
            _buffers: &mut super::super::OwnerBuffers,
            _received: usize,
            _frame_limit: usize,
        ) -> Result<Vec<DecodedFrame>, Error> {
            self.batch
                .take()
                .ok_or_else(|| Error::InvalidState("scripted frame batch exhausted".into()))
        }
    }

    /// A test-only shared host that uses [`ObserverClock`] for caller bounds
    /// while continuing to feed all frames through a real [`BlockingOwner`].
    struct ClockedHost<'a> {
        clock: ObserverClock,
        owner: RefCell<&'a mut BlockingOwner>,
        driver: RefCell<&'a mut dyn BlockingWireDriver>,
        reader: RefCell<&'a mut dyn BlockingReadDriver>,
        decoder: RefCell<&'a mut dyn BlockingFrameDecoder>,
    }

    impl<'a> ClockedHost<'a> {
        fn new<D, R, F>(
            clock: ObserverClock,
            owner: &'a mut BlockingOwner,
            driver: &'a mut D,
            reader: &'a mut R,
            decoder: &'a mut F,
        ) -> Self
        where
            D: BlockingWireDriver,
            R: BlockingReadDriver,
            F: BlockingFrameDecoder,
        {
            Self {
                clock,
                owner: RefCell::new(owner),
                driver: RefCell::new(driver),
                reader: RefCell::new(reader),
                decoder: RefCell::new(decoder),
            }
        }
    }

    impl BlockingControlHost for ClockedHost<'_> {
        fn now(&self) -> Instant {
            self.clock.now()
        }

        fn sleep(&self, duration: Duration) {
            self.clock.set(
                self.clock
                    .now()
                    .checked_add(duration)
                    .expect("test observer clock remains representable"),
            );
        }

        fn deadline_after(&self, timeout: Duration) -> Result<Instant, Error> {
            observer_deadline(self.now(), timeout)
        }

        fn owner_matches(&self, origin: &Arc<()>) -> Result<bool, Error> {
            let owner = self.owner.borrow();
            Ok(Arc::ptr_eq(origin, &owner.state.origin))
        }

        fn pump_once_until(&self, deadline: Option<Instant>) -> Result<usize, Error> {
            let mut owner = self.owner.borrow_mut();
            let mut driver = self.driver.borrow_mut();
            let mut reader = self.reader.borrow_mut();
            let mut decoder = self.decoder.borrow_mut();
            owner.pump_once_until(&mut **driver, &mut **reader, &mut **decoder, deadline)
        }

        fn submit_inquiry_until(
            &self,
            request: RuntimeRequest,
            configured_timeout: Duration,
            deadline: Instant,
        ) -> Result<ReceiptCore, Error> {
            let mut owner = self.owner.borrow_mut();
            let mut driver = self.driver.borrow_mut();
            let mut reader = self.reader.borrow_mut();
            let mut decoder = self.decoder.borrow_mut();
            owner.submit_request_until_with_pump(
                &mut **driver,
                &mut **reader,
                &mut **decoder,
                request,
                configured_timeout,
                deadline,
            )
        }

        fn cancel_operation(
            &self,
            receipt: ReceiptCore,
        ) -> Result<BlockingCancellationReceipt, RejectedCancellation> {
            let mut owner = self.owner.borrow_mut();
            let mut driver = self.driver.borrow_mut();
            owner.cancel_core(&mut **driver, receipt)
        }
    }

    fn command_frames() -> Vec<DecodedFrame> {
        vec![
            DecodedFrame {
                target: CameraId::CAMERA_1,
                sequence: None,
                response: DecodedResponse::Ack {
                    socket: Some(ViscaSocket::S1),
                },
            },
            DecodedFrame {
                target: CameraId::CAMERA_1,
                sequence: None,
                response: DecodedResponse::Completion {
                    socket: Some(ViscaSocket::S1),
                },
            },
        ]
    }

    fn cancellation_frame() -> Vec<DecodedFrame> {
        vec![DecodedFrame {
            target: CameraId::CAMERA_1,
            sequence: None,
            response: DecodedResponse::Error {
                socket: Some(ViscaSocket::S1),
                code: 0x04,
            },
        }]
    }

    fn generic_profile() -> ProfileSpec {
        ProfileSpec::from_compile_time::<GenericVisca>().expect("built-in profile")
    }

    fn focus_receipt(
        owner: &mut BlockingOwner,
        driver: &mut FaultDriver,
        profile: &ProfileSpec,
    ) -> BlockingCommandReceipt {
        owner
            .submit_command(
                driver,
                prepare_builtin_command(
                    &FocusModeCommand::Manual,
                    CameraId::CAMERA_1,
                    profile,
                    OperationalTuning::new(),
                )
                .expect("prepared focus command"),
            )
            .expect("focus command is admitted")
    }

    fn cancelling_zoom_receipt(
        owner: &mut BlockingOwner,
        driver: &mut FaultDriver,
        profile: &ProfileSpec,
    ) -> BlockingCancellationReceipt {
        let operation = owner
            .submit_operation(
                driver,
                prepare_builtin_operation::<AppliedOnly, _>(
                    &ZoomDrive::Tele,
                    CameraId::CAMERA_1,
                    profile,
                    OperationalTuning::new(),
                )
                .expect("prepared zoom operation"),
            )
            .expect("zoom operation is admitted");
        owner
            .inject_frame(
                driver,
                command_frames()
                    .into_iter()
                    .next()
                    .expect("command ACK frame"),
                Instant::now(),
            )
            .expect("ACK is accepted");
        operation
            .cancel_test(owner, driver)
            .expect("cancellation is accepted")
    }

    fn stage_ready_without_dispatch(
        owner: &mut BlockingOwner,
        driver: &mut ReleaseCrossingDriver,
        request: RuntimeRequest,
    ) -> RequestId {
        let permit = owner
            .state()
            .permits()
            .try_acquire()
            .expect("test admission capacity");
        let (input, _completion, admission) = owner.state_mut().stage_admission(request, permit);
        let Input::Admit { ticket, request } = input else {
            unreachable!("staged admission must retain its ticket");
        };
        let effects = owner
            .state_mut()
            .admit_without_due(ticket, request, Instant::now());
        let report = owner.drive_without_due(driver, effects);
        assert!(report.writes.is_empty(), "staging itself cannot dispatch");
        admission
            .recv()
            .expect("admission reply sender remains live")
            .expect("admission accepted")
    }

    fn assert_raw_release_write_crossing_stays_input_first(transport: TransportKind) {
        const HOLD: Duration = Duration::from_millis(30);
        const WRITE_AFTER_H: Duration = Duration::from_millis(45);

        let started = Instant::now();
        let mut owner =
            BlockingOwner::new(raw_two_target_owner_policy(transport)).expect("two-target owner");
        let mut driver = ReleaseCrossingDriver {
            release_after: started + WRITE_AFTER_H,
            writes: Vec::new(),
        };

        // A locally-written A NoReply creates the broad raw tombstone. B is
        // queued behind it while unrelated C is still eligible to transmit.
        let _a = owner
            .submit(
                &mut driver,
                raw_request(CameraId::CAMERA_1, ReplyShape::NoReply, HOLD),
            )
            .expect("A local write");
        let b = stage_ready_without_dispatch(
            &mut owner,
            &mut driver,
            raw_request(
                CameraId::CAMERA_1,
                ReplyShape::AckThenCompletion,
                Duration::from_secs(1),
            ),
        );
        let _c = stage_ready_without_dispatch(
            &mut owner,
            &mut driver,
            raw_request(
                CameraId::CAMERA_2,
                ReplyShape::AckThenCompletion,
                Duration::from_secs(1),
            ),
        );

        // Normal owner due work selects C. Its blocking write returns after
        // A's H, precisely the historical hole: a full finish_write here
        // would expire A and dispatch B before any receive turn.
        let effects = owner.state_mut().advance(Instant::now());
        let report = owner.drive(&mut driver, effects);
        assert!(report.boundary_error().is_none());
        assert_eq!(driver.writes.len(), 2, "only A then unrelated C wrote");
        assert!(
            owner.raw_release_gate_pending(),
            "C completion crossed A's H"
        );
        assert!(
            !owner
                .state()
                .raw_correlation_releases_due(Instant::now())
                .is_empty(),
            "the due projection remains blocked behind the input-first gate"
        );

        // The stale A terminal is decoded before the gate clears. Its broad
        // tombstone makes it inert, and only then may the normal due tail
        // dispatch B. This assertion is a mutation proof: changing `drive`
        // back to full `finish_write` produces B's third write above.
        let mut reader = FaultReader {
            reads: VecDeque::from([Ok(BlockingReceive::Bytes(1))]),
        };
        let mut decoder = OneBatchDecoder::new(vec![DecodedFrame {
            target: CameraId::CAMERA_1,
            sequence: None,
            response: DecodedResponse::Completion {
                socket: Some(ViscaSocket::S1),
            },
        }]);
        owner
            .pump_once(&mut driver, &mut reader, &mut decoder)
            .expect("stale A frame is consumed before release");
        assert_eq!(driver.writes.len(), 3, "B writes only after stale A input");
        assert_eq!(driver.writes[2][0], CameraId::CAMERA_1.to_address_byte());
        assert!(
            owner
                .state()
                .raw_correlation_releases_due(Instant::now())
                .is_empty(),
            "the fenced due tail released exactly once"
        );
        let _ = b;
    }

    #[test]
    fn raw_datagram_write_crossing_release_waits_for_stale_input() {
        assert_raw_release_write_crossing_stays_input_first(TransportKind::Datagram);
    }

    #[test]
    fn raw_stream_write_crossing_release_waits_for_stale_input() {
        assert_raw_release_write_crossing_stays_input_first(TransportKind::Stream);
    }

    #[test]
    fn raw_tombstone_early_idle_is_not_the_release_fence() {
        const HOLD: Duration = Duration::from_millis(30);
        let started = Instant::now();
        let mut owner = BlockingOwner::new(raw_two_target_owner_policy(TransportKind::Datagram))
            .expect("two-target owner");
        let mut driver = ReleaseCrossingDriver {
            release_after: started,
            writes: Vec::new(),
        };
        let _a = owner
            .submit(
                &mut driver,
                raw_request(CameraId::CAMERA_1, ReplyShape::NoReply, HOLD),
            )
            .expect("A local write");
        let b = stage_ready_without_dispatch(
            &mut owner,
            &mut driver,
            raw_request(
                CameraId::CAMERA_1,
                ReplyShape::AckThenCompletion,
                Duration::from_secs(1),
            ),
        );
        let mut reader = EarlyIdleThenDeadlineFrameReader::default();
        let mut decoder = OneBatchDecoder::new(vec![DecodedFrame {
            target: CameraId::CAMERA_1,
            sequence: None,
            response: DecodedResponse::Completion {
                socket: Some(ViscaSocket::S1),
            },
        }]);

        // The first result is an adapter-level early idle; the second is the
        // stale A terminal delivered at H. The wait must read twice.  The old
        // early-idle -> sleep -> service_due sequence left this frame unread.
        owner
            .wait_for_raw_correlation_tombstone(
                &mut driver,
                &mut reader,
                &mut decoder,
                started + Duration::from_secs(1),
                None,
            )
            .expect("fresh H receive consumes stale A evidence");
        assert_eq!(reader.calls, 2, "post-H stale input was consumed");
        let FirstDispatch::Effects(effects) = owner
            .state_mut()
            .first_dispatch_without_due(b, Instant::now())
        else {
            unreachable!("B becomes dispatchable only after the stale frame was read");
        };
        let _ = owner.drive_without_due(&mut driver, effects.into());
        assert_eq!(driver.writes.len(), 2, "A then B after the real fence");
    }

    #[test]
    fn raw_release_malformed_datagrams_exhaust_the_same_64_turn_cap() {
        let mut owner = BlockingOwner::new(raw_two_target_owner_policy(TransportKind::Datagram))
            .expect("two-target owner");
        let mut driver = ReleaseCrossingDriver {
            release_after: Instant::now(),
            writes: Vec::new(),
        };
        let _a = owner
            .submit(
                &mut driver,
                raw_request(CameraId::CAMERA_1, ReplyShape::NoReply, Duration::ZERO),
            )
            .expect("A local write");
        let _b = stage_ready_without_dispatch(
            &mut owner,
            &mut driver,
            raw_request(
                CameraId::CAMERA_1,
                ReplyShape::AckThenCompletion,
                Duration::from_secs(1),
            ),
        );
        let mut reader = FaultReader {
            reads: std::iter::repeat_n(
                Ok(BlockingReceive::Bytes(1)),
                RAW_TOMBSTONE_PUMP_WORK_LIMIT,
            )
            .collect(),
        };
        let mut decoder = MalformedDatagramDecoder;
        for _ in 1..RAW_TOMBSTONE_PUMP_WORK_LIMIT {
            assert_eq!(
                owner
                    .pump_once(&mut driver, &mut reader, &mut decoder)
                    .expect("a malformed datagram remains unfenced before the cap"),
                0
            );
        }
        let error = owner
            .pump_once(&mut driver, &mut reader, &mut decoder)
            .expect_err("the 64th malformed datagram fails closed");
        assert!(matches!(error, Error::StreamPoisoned { .. }));
        assert_eq!(
            driver.writes.len(),
            1,
            "B never writes through malformed input"
        );
        assert!(
            reader.reads.is_empty(),
            "the exact bounded receive budget was consumed"
        );
    }

    #[test]
    fn raw_release_gate_retains_old_scope_until_a_distinct_scope_gets_its_own_probe() {
        let mut owner = BlockingOwner::new(raw_two_target_owner_policy(TransportKind::Datagram))
            .expect("two-target owner");
        let mut driver = ReleaseCrossingDriver {
            release_after: Instant::now(),
            writes: Vec::new(),
        };
        let _a = owner
            .submit(
                &mut driver,
                raw_request(CameraId::CAMERA_1, ReplyShape::NoReply, Duration::ZERO),
            )
            .expect("A local write");
        let first_scope = owner
            .raw_release_gate_at(Instant::now())
            .expect("A release is latched");

        // Deliberately use the no-due test seams to emulate a control turn
        // that made a second target's terminal release visible while A's gate
        // was still awaiting input. The old scope must not be overwritten.
        let c = stage_ready_without_dispatch(
            &mut owner,
            &mut driver,
            raw_request(CameraId::CAMERA_2, ReplyShape::NoReply, Duration::ZERO),
        );
        let FirstDispatch::Effects(effects) = owner
            .state_mut()
            .first_dispatch_without_due(c, Instant::now())
        else {
            unreachable!("unrelated C is eligible through the no-due seam");
        };
        let _ = owner.drive_without_due(&mut driver, effects.into());
        let both_scopes = owner.state().raw_correlation_releases_due(Instant::now());
        assert_ne!(both_scopes, first_scope, "C added a distinct due scope");
        assert_eq!(
            owner.raw_release_gate_at(Instant::now()),
            Some(first_scope),
            "a non-receive path cannot replace A's unprobed release"
        );

        // A true empty datagram probes A. Since the engine now exposes a
        // distinct combined set, the pump preserves the completed A proof,
        // swaps to that exact replacement, and requires one more probe rather
        // than releasing either scope through this turn.
        let mut reader = FaultReader {
            reads: VecDeque::from([Ok(BlockingReceive::TimedOut)]),
        };
        let mut decoder = EmptyDecoder;
        owner
            .pump_once(&mut driver, &mut reader, &mut decoder)
            .expect("first scope probe remains nonterminal");
        assert_eq!(
            owner.raw_release_input_gate.map(|gate| gate.releases),
            Some(both_scopes),
            "the replacement scope is latched for its own fresh input turn"
        );
        assert_eq!(
            driver.writes.len(),
            2,
            "no successor dispatch leaked through the scope swap"
        );
    }

    #[test]
    fn observer_deadline_accepts_pump_terminal_command_at_equality() {
        let profile = generic_profile();
        let mut owner = BlockingOwner::new(raw_owner_policy()).expect("blocking owner");
        let mut driver = FaultDriver;
        let receipt = focus_receipt(&mut owner, &mut driver, &profile);
        let start = Instant::now();
        let clock = ObserverClock::new(start);
        let timeout = Duration::from_millis(100);
        let deadline = start.checked_add(timeout).expect("test deadline");
        let mut reader = DeadlineFrameReader::new(clock.clone(), Duration::ZERO);
        let mut decoder = OneBatchDecoder::new(command_frames());

        {
            let host = ClockedHost::new(
                clock.clone(),
                &mut owner,
                &mut driver,
                &mut reader,
                &mut decoder,
            );
            let mut control = BlockingReceiptControl::shared(&host);
            receipt
                .wait_with_timeout(&mut control, timeout)
                .expect("a terminal frame at the exact observer deadline wins");
        }

        assert_eq!(reader.deadline, Some(deadline));
        assert_eq!(owner.state().state(), SessionState::Running);
        assert_eq!(owner.state().active_len(), 0);
    }

    #[test]
    fn observer_deadline_rejects_pump_terminal_command_one_ns_late() {
        let profile = generic_profile();
        let mut owner = BlockingOwner::new(raw_owner_policy()).expect("blocking owner");
        let mut driver = FaultDriver;
        let receipt = focus_receipt(&mut owner, &mut driver, &profile);
        let start = Instant::now();
        let clock = ObserverClock::new(start);
        let timeout = Duration::from_millis(100);
        let deadline = start.checked_add(timeout).expect("test deadline");
        let mut reader = DeadlineFrameReader::new(clock.clone(), Duration::from_nanos(1));
        let mut decoder = OneBatchDecoder::new(command_frames());

        let error = {
            let host = ClockedHost::new(
                clock.clone(),
                &mut owner,
                &mut driver,
                &mut reader,
                &mut decoder,
            );
            let mut control = BlockingReceiptControl::shared(&host);
            receipt
                .wait_with_timeout(&mut control, timeout)
                .expect_err("a terminal frame one nanosecond late is invisible to this observer")
        };

        assert!(matches!(error, Error::Timeout));
        assert_eq!(reader.deadline, Some(deadline));
        assert_eq!(owner.state().state(), SessionState::Running);
        assert_eq!(
            owner.state().active_len(),
            0,
            "the engine still consumed the late terminal frame"
        );
    }

    #[test]
    fn late_observer_deadline_does_not_mask_a_stream_session_boundary() {
        let profile = generic_profile();
        let mut stream_policy = raw_owner_policy();
        stream_policy.protocol.transport = TransportKind::Stream;
        let mut owner = BlockingOwner::new(stream_policy).expect("blocking owner");
        let mut driver = FaultDriver;
        let receipt = focus_receipt(&mut owner, &mut driver, &profile);
        let clock = ObserverClock::new(Instant::now());
        let timeout = Duration::from_millis(100);
        let mut reader = DeadlineFaultReader {
            clock: clock.clone(),
            offset: Duration::from_nanos(1),
        };
        let mut decoder = EmptyDecoder;

        let error = {
            let host = ClockedHost::new(clock, &mut owner, &mut driver, &mut reader, &mut decoder);
            let mut control = BlockingReceiptControl::shared(&host);
            receipt
                .wait_with_timeout(&mut control, timeout)
                .expect_err("a session boundary remains more important than observer timeout")
        };

        assert!(matches!(error, Error::ConnectionClosed { .. }));
        assert!(error.requires_new_session());
        assert_eq!(owner.state().state(), SessionState::Closed);
    }

    #[test]
    fn cancellation_observer_accepts_pump_terminal_at_equality() {
        let profile = generic_profile();
        let mut owner = BlockingOwner::new(raw_owner_policy()).expect("blocking owner");
        let mut driver = FaultDriver;
        let receipt = cancelling_zoom_receipt(&mut owner, &mut driver, &profile);
        let start = Instant::now();
        let clock = ObserverClock::new(start);
        let timeout = Duration::from_millis(100);
        let deadline = start.checked_add(timeout).expect("test deadline");
        let mut reader = DeadlineFrameReader::new(clock.clone(), Duration::ZERO);
        let mut decoder = OneBatchDecoder::new(cancellation_frame());

        let outcome = {
            let host = ClockedHost::new(
                clock.clone(),
                &mut owner,
                &mut driver,
                &mut reader,
                &mut decoder,
            );
            let mut control = BlockingReceiptControl::shared(&host);
            receipt
                .outcome(&mut control, timeout)
                .expect("a cancellation terminal frame at equality wins")
        };

        assert_eq!(outcome, CancellationOutcome::Cancelled);
        assert_eq!(reader.deadline, Some(deadline));
        assert_eq!(owner.state().state(), SessionState::Running);
        assert_eq!(owner.state().active_len(), 0);
    }

    #[test]
    fn cancellation_observer_rejects_pump_terminal_one_ns_late() {
        let profile = generic_profile();
        let mut owner = BlockingOwner::new(raw_owner_policy()).expect("blocking owner");
        let mut driver = FaultDriver;
        let receipt = cancelling_zoom_receipt(&mut owner, &mut driver, &profile);
        let start = Instant::now();
        let clock = ObserverClock::new(start);
        let timeout = Duration::from_millis(100);
        let deadline = start.checked_add(timeout).expect("test deadline");
        let mut reader = DeadlineFrameReader::new(clock.clone(), Duration::from_nanos(1));
        let mut decoder = OneBatchDecoder::new(cancellation_frame());

        let error = {
            let host = ClockedHost::new(
                clock.clone(),
                &mut owner,
                &mut driver,
                &mut reader,
                &mut decoder,
            );
            let mut control = BlockingReceiptControl::shared(&host);
            receipt
                .outcome(&mut control, timeout)
                .expect_err("a late cancellation observation must not widen the bound")
        };

        assert!(matches!(error, Error::Timeout));
        assert_eq!(reader.deadline, Some(deadline));
        assert_eq!(owner.state().state(), SessionState::Running);
        assert_eq!(
            owner.state().active_len(),
            0,
            "the engine still recorded the late cancellation terminal"
        );
    }

    #[test]
    fn buffered_command_outcome_survives_an_expired_observer_deadline() {
        let profile = generic_profile();
        let mut owner = BlockingOwner::new(raw_owner_policy()).expect("blocking owner");
        let mut driver = FaultDriver;
        let receipt = focus_receipt(&mut owner, &mut driver, &profile);
        let now = Instant::now();
        for frame in command_frames() {
            owner
                .inject_frame(&mut driver, frame, now)
                .expect("terminal command frame is accepted");
        }
        let clock = ObserverClock::new(Instant::now());
        let mut reader = DeadlineFrameReader::new(clock.clone(), Duration::ZERO);
        let mut decoder = OneBatchDecoder::new(Vec::new());

        {
            let host = ClockedHost::new(clock, &mut owner, &mut driver, &mut reader, &mut decoder);
            let mut control = BlockingReceiptControl::shared(&host);
            receipt
                .wait_with_timeout(&mut control, Duration::ZERO)
                .expect("a pre-buffered receipt outcome does not wait");
        }

        assert!(!reader.delivered, "the expired observer did not pump");
        assert!(
            decoder.batch.is_some(),
            "the frame decoder stayed untouched"
        );
    }

    #[test]
    fn ordinary_capacity_rejection_is_observable_without_admission() {
        let profile = ProfileSpec::from_compile_time::<GenericVisca>().expect("built-in profile");
        let mut owner = BlockingOwner::new(raw_owner_policy()).expect("blocking owner");
        let mut driver = FaultDriver;

        let first = prepare_builtin_operation::<AppliedOnly, _>(
            &ZoomDrive::Tele,
            CameraId::CAMERA_1,
            &profile,
            OperationalTuning::new(),
        )
        .expect("first operation");
        let _first = owner
            .submit_operation(&mut driver, first)
            .expect("first operation is active");
        let before = owner.state().metrics_snapshot();
        assert_eq!(before.active, 1);
        assert_eq!(before.pending, 0);
        let _ = owner.drain_diagnostics();

        let second = prepare_builtin_command(
            &FocusModeCommand::Manual,
            CameraId::CAMERA_1,
            &profile,
            OperationalTuning::new(),
        )
        .expect("second ordinary command");
        let error = owner
            .submit_command(&mut driver, second)
            .expect_err("full admission capacity rejects an ordinary submission");
        assert!(matches!(error, Error::RuntimeQueueFull { capacity: 1 }));

        let after = owner.state().metrics_snapshot();
        assert_eq!(after.admission_rejected, before.admission_rejected + 1);
        assert_eq!(after.admitted, before.admitted);
        assert_eq!(after.terminal, before.terminal);
        assert_eq!(after.active, before.active);
        assert_eq!(after.pending, before.pending);
        assert_eq!(
            owner.drain_diagnostics(),
            vec![DiagnosticEvent::AdmissionRejected {
                target: CameraId::CAMERA_1,
                lane: RequestLane::Command,
                error: ErrorKind::BufferFull,
            }]
        );
    }

    #[test]
    fn operation_preprobe_capacity_rejection_is_observable_before_raw_preack_drain() {
        let counts = Arc::new(InteractionCounts::default());
        let transport = CountingTransport {
            config: TransportConfig::default(),
            counts: Arc::clone(&counts),
        };
        let profile = ProfileSpec::from_compile_time::<GenericVisca>().expect("built-in profile");
        let adapter = BlockingTransportAdapter::new_with_targets(
            transport,
            &[(CameraId::CAMERA_1, &profile)],
            OperationalTuning::new(),
            std::num::NonZeroUsize::new(1).expect("non-zero capacity"),
        )
        .expect("blocking adapter");
        let host = BlockingSessionHost::from_adapter(adapter).expect("blocking host");

        let first = prepare_builtin_operation::<AppliedOnly, _>(
            &ZoomDrive::Tele,
            CameraId::CAMERA_1,
            &profile,
            OperationalTuning::new(),
        )
        .expect("first operation");
        let _first = host.submit_operation(first).expect("first write");
        assert_eq!(counts.writes.load(Ordering::SeqCst), 1);
        assert_eq!(counts.receives.load(Ordering::SeqCst), 0);
        assert!(host
            .with_parts(|owner, _, _, _| Ok(owner
                .state()
                .raw_preack_gate_frees_socket_on_ack(CameraId::CAMERA_1)))
            .expect("inspect pre-ACK gate"));
        let before = host.metrics().expect("metrics before rejection");
        assert_eq!(before.active, 1);
        assert_eq!(before.pending, 0);
        let _ = host.drain_diagnostics().expect("initial diagnostics");

        let second = prepare_builtin_operation::<AppliedOnly, _>(
            &ZoomDrive::Tele,
            CameraId::CAMERA_1,
            &profile,
            OperationalTuning::new(),
        )
        .expect("second operation");
        let error = host
            .submit_operation(second)
            .expect_err("full global admission must reject before draining the ACK");
        assert!(matches!(error, Error::RuntimeQueueFull { capacity: 1 }));
        let after = host.metrics().expect("metrics after rejection");
        assert_eq!(after.admission_rejected, before.admission_rejected + 1);
        assert_eq!(after.admitted, before.admitted);
        assert_eq!(after.terminal, before.terminal);
        assert_eq!(after.active, before.active);
        assert_eq!(after.pending, before.pending);
        assert_eq!(
            host.drain_diagnostics().expect("rejection diagnostics"),
            vec![DiagnosticEvent::AdmissionRejected {
                target: CameraId::CAMERA_1,
                lane: RequestLane::Command,
                error: ErrorKind::BufferFull,
            }]
        );
        assert_eq!(counts.writes.load(Ordering::SeqCst), 1);
        assert_eq!(counts.receives.load(Ordering::SeqCst), 0);
    }

    #[test]
    fn persistent_transient_faults_close_the_blocking_session_with_their_cause() {
        assert!(!TransientFaultRun::is_permanent(
            TRANSIENT_RECEIVE_FAULT_LIMIT,
            Duration::ZERO
        ));
        assert!(!TransientFaultRun::is_permanent(
            TRANSIENT_RECEIVE_FAULT_LIMIT - 1,
            TRANSIENT_RECEIVE_FAULT_SPAN
        ));
        assert!(TransientFaultRun::is_permanent(
            TRANSIENT_RECEIVE_FAULT_LIMIT,
            TRANSIENT_RECEIVE_FAULT_SPAN
        ));

        let now = Instant::now();
        let mut owner = BlockingOwner::new(raw_owner_policy()).expect("blocking owner");
        // Seed eleven consecutive faults spanning the documented minimum so
        // this pump proves the twelfth transitions the real owner boundary,
        // without turning the regression test into a multi-second sleep.
        owner.faults = TransientFaultRun {
            length: TRANSIENT_RECEIVE_FAULT_LIMIT - 1,
            first_at: Some(now - TRANSIENT_RECEIVE_FAULT_SPAN),
            last_at: Some(now),
        };
        let mut driver = FaultDriver;
        let mut reader = FaultReader {
            reads: VecDeque::from([Err(Error::TransportError("simulated ICMP fault".into()))]),
        };
        let mut decoder = EmptyDecoder;

        let error = owner
            .pump_once(&mut driver, &mut reader, &mut decoder)
            .expect_err("the twelfth sustained fault must close the session");
        assert!(
            matches!(&error, Error::ConnectionClosed { reason: Some(_) }),
            "persistent transient faults must report ConnectionClosed"
        );
        if let Error::ConnectionClosed {
            reason: Some(reason),
        } = error
        {
            assert!(reason.contains("12 consecutive receive faults"));
            assert!(reason.contains("simulated ICMP fault"));
        }
        assert_eq!(owner.state().state(), SessionState::Closed);
    }

    #[test]
    fn idle_reads_do_not_break_a_transient_fault_run() {
        let now = Instant::now();
        let mut owner = BlockingOwner::new(raw_owner_policy()).expect("blocking owner");
        // Seed eleven faults across the required span.  The idle read below
        // must leave that run intact, so the following twelfth fault closes
        // without a wall-clock multi-second test.
        owner.faults = TransientFaultRun {
            length: TRANSIENT_RECEIVE_FAULT_LIMIT - 1,
            first_at: Some(now - TRANSIENT_RECEIVE_FAULT_SPAN),
            last_at: Some(now),
        };
        let mut driver = FaultDriver;
        let mut reader = FaultReader {
            reads: VecDeque::from([
                Ok(BlockingReceive::TimedOut),
                Err(Error::TransportError("simulated ICMP fault".into())),
            ]),
        };
        let mut decoder = EmptyDecoder;

        assert_eq!(
            owner
                .pump_once(&mut driver, &mut reader, &mut decoder)
                .expect("an idle receive is not a boundary"),
            0
        );
        assert_eq!(owner.faults.length, TRANSIENT_RECEIVE_FAULT_LIMIT - 1);
        assert!(owner.faults.first_at.is_some());
        assert!(owner.faults.last_at.is_some());

        let error = owner
            .pump_once(&mut driver, &mut reader, &mut decoder)
            .expect_err("the fault following idle no-data is still the twelfth");
        assert!(matches!(error, Error::ConnectionClosed { .. }));
        assert_eq!(owner.state().state(), SessionState::Closed);
    }

    #[test]
    fn transient_fault_run_resets_after_a_successful_read_or_a_five_second_gap() {
        let start = Instant::now();
        let mut owner = BlockingOwner::new(raw_owner_policy()).expect("blocking owner");
        owner.faults.record(start);
        owner.faults.record(start + Duration::from_millis(10));

        // A real successful read reaches the owner reset path, even when it
        // contains only an incomplete frame.
        let mut driver = FaultDriver;
        let mut reader = FaultReader {
            reads: VecDeque::from([Ok(BlockingReceive::Bytes(1))]),
        };
        let mut decoder = EmptyDecoder;
        owner
            .pump_once(&mut driver, &mut reader, &mut decoder)
            .expect("successful read");
        assert_eq!(owner.faults.length, 0);
        assert!(owner.faults.first_at.is_none());
        assert!(owner.faults.last_at.is_none());

        // A five-second gap does the same without relying on a wall-clock
        // sleep in the test.
        let mut faults = TransientFaultRun::default();
        assert_eq!(faults.record(start).0, 1);
        assert_eq!(faults.record(start + Duration::from_millis(10)).0, 2);
        assert_eq!(
            faults
                .record(start + Duration::from_millis(10) + TRANSIENT_RECEIVE_FAULT_RESET)
                .0,
            1,
            "a five-second gap starts a fresh fault run"
        );
    }

    #[test]
    fn transient_fault_pause_escalates_and_stays_deadline_clamped() {
        assert_eq!(transient_receive_pause(1), TRANSIENT_RECEIVE_PAUSE);
        assert_eq!(transient_receive_pause(2), Duration::from_millis(20));
        assert_eq!(transient_receive_pause(3), Duration::from_millis(40));
        assert_eq!(
            transient_receive_pause(100),
            MAXIMUM_TRANSIENT_RECEIVE_PAUSE,
            "the escalating pause has the documented 250 ms ceiling"
        );

        let now = Instant::now();
        assert_eq!(
            clamp_receive_pause(
                MAXIMUM_TRANSIENT_RECEIVE_PAUSE,
                Some(now + Duration::from_millis(3)),
                now,
            ),
            Duration::from_millis(3),
            "a transient pause cannot delay an earlier owner deadline"
        );
    }
}
