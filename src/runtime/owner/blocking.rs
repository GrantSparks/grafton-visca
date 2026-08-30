//! Caller-thread owner for the blocking mode.

use std::{
    cell::RefCell,
    collections::VecDeque,
    fmt,
    marker::PhantomData,
    sync::Arc,
    time::{Duration, Instant},
};

use crate::{completion, AffectedAxes, CancellationOutcome, Error, ResponseDecoder};

use super::{
    cancellation_receipt_for, normalize_cancellation_observation, normalize_command_outcome,
    normalize_inquiry_outcome, prepend_effects, AppliedEffect, BlockingTransportAdapter,
    CancellationCore, CompletionObserver, DiagnosticEvent, OwnerInputTurn, OwnerPolicy, OwnerState,
    ReceiptCore, ReceiptObservation, RejectedCancellation, RequestId, RuntimeOutcome,
    RuntimeRequest, ShutdownReason, TransmissionMeta, WaitSelection, WireWrite,
};
use crate::runtime::engine::{
    DecodedFrame, Effect, FirstDispatch, IgnoreReason, Input, TransportKind,
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
        self.with_parts(|owner, driver, _, _| {
            owner.submit_request_until(driver, request, configured_timeout, deadline)
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
        self.with_parts(|owner, driver, _, _| owner.submit_command(driver, prepared))
    }

    pub(crate) fn submit_inquiry<R>(
        &self,
        prepared: crate::prepared::PreparedInquiry<R>,
    ) -> Result<BlockingInquiryReceipt<R>, Error> {
        self.with_parts(|owner, driver, _, _| owner.submit_inquiry(driver, prepared))
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
            owner.ensure_admission_capacity()?;
            // Issue #673: before the first-write submit, drain the raw
            // single-candidate pre-ACK gate if that alone is what blocks this
            // target. Without it, an emergency `stop_all_motion`/`Urgent` stop —
            // or any second operation — submitted while a caller still holds an
            // un-awaited raw operation handle would lose the first-dispatch race
            // and be rejected `TransportBusy` with zero bytes on the wire while
            // the camera keeps moving. The drain pumps the peer's ACK (bounded
            // by this request's own ACK budget) so a command socket frees and
            // the subsequent first write wins.
            let (target, ack_budget) = prepared.preack_drain_hint();
            owner.drain_raw_preack_gate(driver, reader, decoder, target, ack_budget)?;
            owner.submit_operation(driver, prepared)
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
    pub(crate) fn reconfigure(&self, tuning: crate::OperationalTuning) -> Result<(), Error> {
        self.with_parts(|owner, _, _, _| owner.reconfigure(tuning))
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
        self.with_parts(|owner, driver, _, _| {
            owner.submit_request_until(driver, request, configured_timeout, deadline)
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
            BlockingControlKind::Borrowed { .. } => self.with_parts(|owner, driver, _, _| {
                owner.submit_inquiry_until(driver, prepared, deadline)
            }),
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
            if let Some(observation) = self.core.try_observation() {
                return normalize_cancellation_observation(observation);
            }
            let pump_result = control.pump_once_until(Some(deadline));
            // This cancellation's own terminal observation wins over the pump's
            // verdict; without one, the pump reports the session boundary error
            // rather than the raw transport cause (#629).
            if let Some(observation) = self.core.try_observation() {
                return normalize_cancellation_observation(observation);
            }
            pump_result?;
            if control.now() >= deadline {
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
/// ordinary queued work. The pre-ACK drain suppresses that final scheduler
/// step until its submitting operation has been admitted (issue #673).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum PumpMode {
    Normal,
    PreAckDrain,
}

impl PumpMode {
    const fn allows_ordinary_dispatch(self) -> bool {
        matches!(self, Self::Normal)
    }
}

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
        if let Some(outcome) = core.try_outcome() {
            return Ok(outcome);
        }
        let pump_result = control.pump_once_until(Some(deadline));
        if let Some(outcome) = core.try_outcome() {
            return Ok(outcome);
        }
        pump_result?;
        if control.now() >= deadline {
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

#[derive(Debug, Default)]
pub(crate) struct DriveReport {
    writes: Vec<(RequestId, Result<(), Error>)>,
}

impl DriveReport {
    fn first_write_for(&self, id: RequestId) -> Option<Result<(), Error>> {
        self.writes
            .iter()
            .find(|(request, _)| *request == id)
            .map(|(_, result)| result.clone())
    }
}

/// A serialized owner. It never creates a worker thread and therefore cannot
/// accidentally pump ACK/completion while returning from submission.
#[derive(Debug)]
pub(crate) struct BlockingOwner {
    state: OwnerState,
    pumping: bool,
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
    fn ensure_admission_capacity(&self) -> Result<(), Error> {
        let permits = self.state.permits();
        let Some(probe) = permits.try_acquire() else {
            return Err(Error::RuntimeQueueFull {
                capacity: permits.capacity(),
            });
        };
        drop(probe);
        Ok(())
    }

    /// Mutably accesses the owner state for caller-thread control operations.
    pub(crate) fn state_mut(&mut self) -> &mut OwnerState {
        &mut self.state
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

    pub(crate) fn submit_request_until<D: BlockingWireDriver + ?Sized>(
        &mut self,
        driver: &mut D,
        request: RuntimeRequest,
        configured_timeout: Duration,
        deadline: Instant,
    ) -> Result<ReceiptCore, Error> {
        self.submit_with_timeout_until(driver, request, configured_timeout, deadline)
    }

    /// Drain the raw single-candidate pre-ACK gate before an operation's
    /// first-write submit, when that gate alone blocks a new command on
    /// `target` (issue #673).
    ///
    /// On a raw-VISCA target the engine keeps at most one *unacknowledged*
    /// command in flight so a socketless ACK can never be misattributed to the
    /// wrong request. While a caller holds an un-awaited operation handle whose
    /// command is still awaiting its ACK, a second operation — including an
    /// emergency `stop_all_motion`/`Urgent` stop — loses the first-dispatch
    /// race and, under the `RequireFirstWrite` policy, would be rejected
    /// [`Error::TransportBusy`] with no write even though a command socket is
    /// free the instant that ACK lands. This pumps the owner (reading the
    /// peer's ACK off the socket) until the gate clears, bounded by the
    /// submitting request's own ACK budget, so the subsequent first write wins
    /// and the returned handle still names a request whose first write
    /// succeeded.
    ///
    /// It is deliberately narrow. When the block is genuine socket-capacity
    /// contention — every command socket already occupied, independent of the
    /// pre-ACK gate — [`OwnerState::raw_preack_gate_frees_socket_on_ack`] is
    /// `false`, no pump is attempted, and the fail-fast rejection the caller
    /// then receives from the first-write submit stands. If the pump ends the
    /// session (a close or poison observed while waiting), the session's own
    /// boundary verdict is returned rather than the raw transport cause, so an
    /// auto-reconnect loop keyed on `requires_new_session()` still behaves
    /// (issue #629).
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

    /// Admit and perform the first write when this exact request wins the
    /// global dispatch race. Ordinary receipts that cannot win yet stay
    /// queued in the engine and are written by a later owner turn. No receive
    /// method is called here, so ACK/completion can only be consumed by an
    /// explicit pump.
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

    fn submit_with_timeout_policy<D: BlockingWireDriver + ?Sized>(
        &mut self,
        driver: &mut D,
        request: RuntimeRequest,
        configured_timeout: Duration,
        submit_policy: SubmitPolicy,
    ) -> Result<ReceiptCore, Error> {
        self.enter()?;
        let result = self.submit_inner(driver, request, configured_timeout, None, submit_policy);
        self.leave();
        result
    }

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

    fn submit_with_timeout_until_policy<D: BlockingWireDriver + ?Sized>(
        &mut self,
        driver: &mut D,
        request: RuntimeRequest,
        configured_timeout: Duration,
        deadline: Instant,
        submit_policy: SubmitPolicy,
    ) -> Result<ReceiptCore, Error> {
        // Match the async owner boundary: once the caller-owned observer
        // deadline has elapsed, reject before staging admission or writing a
        // new inquiry.
        if Instant::now() >= deadline {
            return Err(Error::Timeout);
        }
        self.enter()?;
        let result = self.submit_inner(
            driver,
            request,
            configured_timeout,
            Some(deadline),
            submit_policy,
        );
        self.leave();
        result
    }

    fn submit_inner<D: BlockingWireDriver + ?Sized>(
        &mut self,
        driver: &mut D,
        request: RuntimeRequest,
        configured_timeout: Duration,
        observer_deadline: Option<Instant>,
        submit_policy: SubmitPolicy,
    ) -> Result<ReceiptCore, Error> {
        let target = request.context().target;
        let origin = self.state.origin();
        let permit = self
            .state
            .permits()
            .try_acquire()
            .ok_or(Error::RuntimeQueueFull {
                capacity: self.state.permits().capacity(),
            })?;
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
                return Err(Error::Timeout);
            }
            match self.state.first_dispatch_without_due(id, now) {
                FirstDispatch::Effects(effects) => {
                    let advanced = self.drive_without_due(driver, effects.into());
                    report.writes.extend(advanced.writes);
                }
                FirstDispatch::WaitUntil(dispatch_at) => {
                    if observer_deadline.is_some_and(|deadline| dispatch_at >= deadline) {
                        return Err(Error::Timeout);
                    }
                    let now = Instant::now();
                    if dispatch_at > now {
                        std::thread::sleep(dispatch_at.duration_since(now));
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
                // `NoReply` `Applied` observation for the returned receipt.
                return Err(buffered_submission_error(&completion).unwrap_or(error));
            }
        } else if let Some(error) = buffered_submission_error(&completion) {
            // A queued receipt has not reached the wire, so any terminal
            // observation remains a failed submission. Once a first write has
            // succeeded, however, a `NoReply` command legitimately resolves
            // `Applied` in that same owner turn; leave that observation for the
            // returned receipt to consume.
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
        result.map_err(|error| self.boundary_error_or(error))
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
    ) -> Result<usize, Error>
    where
        D: BlockingWireDriver + ?Sized,
        R: BlockingReadDriver + ?Sized,
        F: BlockingFrameDecoder + ?Sized,
    {
        let owner_deadline = min_deadline(self.next_wake_for_mode(mode), observer_deadline);
        let read = reader.receive(self.state.buffers().receive_mut(), owner_deadline);
        // An error that only reports "no bytes arrived" is an idle read, not a
        // fault: it consumed nothing and must not burn any request's retry
        // budget. The adapter normalizes this too; doing it here as well keeps
        // every read driver on one contract (#637).
        let read = match read {
            Err(error) if super::receive_reported_no_data(&error) => Ok(BlockingReceive::TimedOut),
            other => other,
        };
        let (received, received_at) = match read {
            Ok(BlockingReceive::TimedOut) => {
                // A clean idle read proves the adapter is responding again,
                // so it breaks any prior run of transient failures.
                self.faults.reset();
                let now = Instant::now();
                if self
                    .next_wake_for_mode(mode)
                    .is_some_and(|wake| wake <= now)
                {
                    let effects = self.advance_for_mode(now, mode);
                    let _ = self.drive_for_mode(driver, effects, mode);
                }
                return Ok(0);
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
                (received, Instant::now())
            }
            Err(error) if super::receive_fault_is_transient(&error) => {
                let received_at = Instant::now();
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
                let _ = self.drive_for_mode(driver, effects, mode);
                pause_after_transient_receive_fault(length, owner_deadline);
                return Ok(0);
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
        // The first pass decodes the bytes just read. On a stream, subsequent
        // passes drain (received == 0) any complete frames a receive that hit
        // the per-receive frame limit left buffered, so a burst larger than one
        // batch is fully attributed in this pump instead of stalling until more
        // bytes happen to arrive (#674).
        let mut input_len = received;
        let mut driven = 0usize;
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
                    return Ok(driven);
                }
            };
            // #672: a stream tolerates a delimited frame that did not classify by
            // discarding it and staying Running, exactly as a datagram already
            // does and as 1.x did (log-and-continue). Record one Ignored per
            // discarded frame so the discard stays observable.
            for _ in 0..self.state.buffers().take_discarded_malformed() {
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
                return Ok(driven);
            }
            let count = frames.len();
            self.drive_decoded_batch_with_mode(driver, frames, received_at, mode);
            driven = driven.saturating_add(count);
            // Only a stream buffers a remainder, and only a batch that filled the
            // limit can have left one; drain and drive it without reading again.
            if is_stream && frame_limit > 0 && count >= frame_limit {
                input_len = 0;
                continue;
            }
            break;
        }
        Ok(driven)
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
        let effects = self.state.input(Input::Cancel { id }, Instant::now());
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
        let effects = self.state.advance(now);
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
    /// submission would, before any tuning is applied.
    pub(crate) fn reconfigure(&mut self, tuning: crate::OperationalTuning) -> Result<(), Error> {
        self.enter()?;
        let result = self.state_mut().retune(tuning);
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
                let produced = self
                    .state
                    .finish_write(&staged, write_result, Instant::now());
                prepend_effects(&mut effects, produced);
            }
        }
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
        report
    }

    /// Applies one non-frame pump input using the selected scheduler boundary.
    /// The pre-ACK mode goes directly through the engine's input-turn seam so
    /// owner-side effects are still replayed, but ordinary dispatch remains
    /// withheld until the submitting operation has been admitted.
    fn input_for_mode(&mut self, input: Input, now: Instant, mode: PumpMode) -> VecDeque<Effect> {
        if mode.allows_ordinary_dispatch() {
            self.state.input(input, now)
        } else {
            self.state.engine.handle_without_dispatch(input, now).into()
        }
    }

    fn advance_for_mode(&mut self, now: Instant, mode: PumpMode) -> VecDeque<Effect> {
        if mode.allows_ordinary_dispatch() {
            self.state.advance(now)
        } else {
            self.state.engine.advance_without_dispatch(now).into()
        }
    }

    fn finish_input_turn_for_mode(
        &mut self,
        turn: OwnerInputTurn,
        mode: PumpMode,
    ) -> VecDeque<Effect> {
        if mode.allows_ordinary_dispatch() {
            self.state.finish_input_turn(turn)
        } else {
            self.state
                .engine
                .finish_input_turn_without_dispatch(turn.0)
                .into()
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
        self.drive_decoded_batch_with_mode(driver, frames, received_at, PumpMode::Normal);
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
    ) {
        let turn = self.state.begin_input_turn(received_at);
        for frame in frames {
            let effects = self.state.input_in_turn(&turn, Input::Frame(frame));
            self.drive_in_turn(driver, &turn, effects);
        }
        let due = self.finish_input_turn_for_mode(turn, mode);
        let _ = self.drive_for_mode(driver, due, mode);
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

/// Pause applied after the first transient receive fault so a transport that
/// fails immediately cannot spin a caller's pump loop. 1.x used the same
/// bound.
const TRANSIENT_RECEIVE_PAUSE: Duration = Duration::from_millis(10);

/// Ceiling on the escalating transient-fault pause.
const MAXIMUM_TRANSIENT_RECEIVE_PAUSE: Duration = Duration::from_millis(250);

/// Consecutive transient receive faults, with no successful read between them,
/// after which the session ends with the underlying transport error.
const TRANSIENT_RECEIVE_FAULT_LIMIT: u32 = 12;

/// Minimum wall-clock length of a fault run before it can end the session.
const TRANSIENT_RECEIVE_FAULT_SPAN: Duration = Duration::from_secs(1);

/// A gap this long between two transient faults proves the transport recovered
/// in between, so the run starts over rather than accumulating over hours.
const TRANSIENT_RECEIVE_FAULT_RESET: Duration = Duration::from_secs(5);

/// Escalating pause for the `run`-th consecutive transient receive fault.
fn transient_receive_pause(run: u32) -> Duration {
    let doublings = run.saturating_sub(1).min(6);
    TRANSIENT_RECEIVE_PAUSE
        .saturating_mul(1u32 << doublings)
        .min(MAXIMUM_TRANSIENT_RECEIVE_PAUSE)
}

/// Clamp a transient pause so it cannot delay an owner wake or caller deadline.
fn clamp_transient_pause(
    pause: Duration,
    owner_deadline: Option<Instant>,
    now: Instant,
) -> Duration {
    owner_deadline.map_or(pause, |deadline| {
        pause.min(deadline.saturating_duration_since(now))
    })
}

fn pause_after_transient_receive_fault(run: u32, owner_deadline: Option<Instant>) {
    let pause = clamp_transient_pause(transient_receive_pause(run), owner_deadline, Instant::now());
    if !pause.is_zero() {
        std::thread::sleep(pause);
    }
}

/// One run of consecutive transient receive faults.
#[derive(Debug, Default)]
struct TransientFaultRun {
    length: u32,
    first_at: Option<Instant>,
    last_at: Option<Instant>,
}

impl TransientFaultRun {
    /// Record one transient fault and report the run it belongs to.
    fn record(&mut self, at: Instant) -> (u32, Duration) {
        let continues = self
            .last_at
            .is_some_and(|last| at.saturating_duration_since(last) < TRANSIENT_RECEIVE_FAULT_RESET);
        if continues {
            self.length = self.length.saturating_add(1);
        } else {
            self.length = 1;
            self.first_at = Some(at);
        }
        self.last_at = Some(at);
        let span = self
            .first_at
            .map_or(Duration::ZERO, |first| at.saturating_duration_since(first));
        (self.length, span)
    }

    /// A successful or cleanly idle read proves the transport is answering
    /// again, so the next fault starts a fresh run.
    fn reset(&mut self) {
        self.length = 0;
        self.first_at = None;
        self.last_at = None;
    }

    /// Whether this run is long enough, and old enough, to be called permanent.
    const fn is_permanent(length: u32, span: Duration) -> bool {
        length >= TRANSIENT_RECEIVE_FAULT_LIMIT
            && span.as_nanos() >= TRANSIENT_RECEIVE_FAULT_SPAN.as_nanos()
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
        collections::VecDeque,
        sync::{
            atomic::{AtomicUsize, Ordering},
            Arc,
        },
    };

    use super::*;
    use crate::runtime::engine::{
        CancellationPolicy, EnvelopeKind, ProtocolPolicy, SessionState, TargetPolicy,
    };
    use crate::{
        command::CommandKind,
        completion::AppliedOnly,
        prepared::prepare_builtin_operation,
        profile::ProfileSpec,
        profiles::GenericVisca,
        request::builtin::ZoomDrive,
        transport::{BlockingTransport, HasTransportConfig, SendSemantics, TransportConfig},
        CameraId, OperationalTuning,
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

    #[derive(Debug, Default)]
    struct FaultDriver;

    impl BlockingWireDriver for FaultDriver {
        fn write(&mut self, _write: WireWrite<'_>) -> Result<TransmissionMeta, Error> {
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

    #[test]
    fn full_admission_rejects_before_raw_preack_drain() {
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
        assert_eq!(transient_receive_pause(1), Duration::from_millis(10));
        assert_eq!(transient_receive_pause(2), Duration::from_millis(20));
        assert_eq!(transient_receive_pause(3), Duration::from_millis(40));
        assert_eq!(
            transient_receive_pause(100),
            MAXIMUM_TRANSIENT_RECEIVE_PAUSE,
            "the escalating pause has the documented 250 ms ceiling"
        );

        let now = Instant::now();
        assert_eq!(
            clamp_transient_pause(
                MAXIMUM_TRANSIENT_RECEIVE_PAUSE,
                Some(now + Duration::from_millis(3)),
                now,
            ),
            Duration::from_millis(3),
            "a transient pause cannot delay an earlier owner deadline"
        );
    }
}
