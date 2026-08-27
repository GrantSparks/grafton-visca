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
    ReceiptCore, ReceiptObservation, RequestId, RuntimeOutcome, RuntimeRequest, ShutdownReason,
    TransmissionMeta, WaitSelection, WireWrite,
};
use crate::runtime::engine::{DecodedFrame, Effect, FirstDispatch, Input, TransportKind};

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
pub(crate) struct BlockingSessionCore<'a> {
    parts: RefCell<BlockingSessionParts<'a>>,
}

struct BlockingSessionParts<'a> {
    owner: &'a mut BlockingOwner,
    driver: &'a mut dyn BlockingWireDriver,
    reader: &'a mut dyn BlockingReadDriver,
    decoder: &'a mut dyn BlockingFrameDecoder,
}

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

    pub(crate) fn control(&'a self) -> BlockingReceiptControl<'a> {
        BlockingReceiptControl::shared(self)
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
}

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

    fn cancel_operation(&self, receipt: ReceiptCore) -> Result<BlockingCancellationReceipt, Error>;
}

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

    fn cancel_operation(&self, receipt: ReceiptCore) -> Result<BlockingCancellationReceipt, Error> {
        self.with_parts(|owner, driver, _, _| owner.cancel_core(driver, receipt))
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
        Ok(Self {
            parts: RefCell::new(BlockingOwnedSessionParts {
                owner,
                driver: Box::new(driver),
                reader: Box::new(reader),
                decoder: Box::new(decoder),
            }),
            state_cache,
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
        self.with_parts(|owner, driver, _, _| owner.submit_operation(driver, prepared))
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

    fn cancel_operation(&self, receipt: ReceiptCore) -> Result<BlockingCancellationReceipt, Error> {
        self.with_parts(|owner, driver, _, _| owner.cancel_core(driver, receipt))
    }
}

/// The caller-thread transport/reader control needed while a blocking receipt
/// is observed. It has no lifecycle identity of its own.
///
/// The `Borrowed` variant preserves the owner-level receipt tests and the
/// narrow pre-facade adapter seam. Public handles are created with `Shared`,
/// which is the only variant that can outlive an individual method call.
pub(crate) struct BlockingReceiptControl<'a> {
    kind: BlockingControlKind<'a>,
}

enum BlockingControlKind<'a> {
    Shared(&'a dyn BlockingControlHost),
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
            BlockingControlKind::Borrowed { .. } => Instant::now(),
        }
    }

    pub(crate) fn sleep(&self, duration: Duration) {
        match &self.kind {
            BlockingControlKind::Shared(host) => host.sleep(duration),
            BlockingControlKind::Borrowed { .. } => std::thread::sleep(duration),
        }
    }

    pub(crate) fn deadline_after(&self, timeout: Duration) -> Result<Instant, Error> {
        match &self.kind {
            BlockingControlKind::Shared(host) => host.deadline_after(timeout),
            BlockingControlKind::Borrowed { .. } => observer_deadline(self.now(), timeout),
        }
    }

    fn owner_matches(&mut self, origin: &Arc<()>) -> Result<bool, Error> {
        match &self.kind {
            BlockingControlKind::Shared(host) => host.owner_matches(origin),
            BlockingControlKind::Borrowed { .. } => {
                self.with_parts(|owner, _, _, _| Ok(Arc::ptr_eq(origin, &owner.state.origin)))
            }
        }
    }

    fn pump_once_until(&mut self, deadline: Option<Instant>) -> Result<usize, Error> {
        match &self.kind {
            BlockingControlKind::Shared(host) => host.pump_once_until(deadline),
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
            BlockingControlKind::Borrowed { .. } => self.with_parts(|owner, driver, _, _| {
                owner.submit_inquiry_until(driver, prepared, deadline)
            }),
        }
    }

    pub(crate) fn cancel_operation<K>(
        &mut self,
        receipt: BlockingOperationReceipt<K>,
    ) -> Result<BlockingCancellationReceipt, Error>
    where
        K: completion::Kind,
    {
        match &self.kind {
            BlockingControlKind::Shared(host) => host.cancel_operation(receipt.into_core()),
            BlockingControlKind::Borrowed { .. } => {
                self.with_parts(|owner, driver, _, _| receipt.cancel(owner, driver))
            }
        }
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
    pub(crate) id: RequestId,
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

    pub(crate) fn wait_with_timeout(
        self,
        control: &mut BlockingReceiptControl<'_>,
        timeout: Duration,
    ) -> Result<(), Error> {
        wait_core_for(self.core, control, timeout).and_then(normalize_command_outcome)
    }

    pub(crate) fn detach(self) {}
}

impl<R> BlockingInquiryReceipt<R> {
    pub(crate) fn wait(self, control: &mut BlockingReceiptControl<'_>) -> Result<R, Error> {
        let timeout = self.core.configured_timeout();
        let outcome = wait_core_for(self.core, control, timeout)?;
        normalize_inquiry_outcome(outcome, &self.decoder)
    }

    pub(crate) fn wait_with_timeout(
        self,
        control: &mut BlockingReceiptControl<'_>,
        timeout: Duration,
    ) -> Result<R, Error> {
        let outcome = wait_core_for(self.core, control, timeout)?;
        normalize_inquiry_outcome(outcome, &self.decoder)
    }

    pub(crate) fn detach(self) {}

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

    pub(crate) fn cancel<D: BlockingWireDriver + ?Sized>(
        self,
        owner: &mut BlockingOwner,
        driver: &mut D,
    ) -> Result<BlockingCancellationReceipt, Error> {
        if !Arc::ptr_eq(&self.core.origin, &owner.state.origin) {
            return Err(Error::InvalidState(
                "operation receipt belongs to a different owner".into(),
            ));
        }
        owner.cancel_core(driver, self.core)
    }

    pub(crate) fn into_core(self) -> ReceiptCore {
        self.core
    }

    pub(crate) fn detach(self) {}
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
        let id = self.receipt.core.id();
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
                    id,
                    target,
                    axes: self.receipt.affected_axes,
                    plan,
                    deadline,
                    control: self.control,
                }))
            }
        }
    }

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
    #[cfg(test)]
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

    pub(crate) fn detach(self) {}
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
}

impl BlockingOwner {
    pub(crate) fn new(policy: OwnerPolicy) -> Result<Self, Error> {
        Ok(Self {
            state: OwnerState::new(policy)?,
            pumping: false,
        })
    }

    pub(crate) const fn state(&self) -> &OwnerState {
        &self.state
    }

    pub(crate) fn state_cache(&self, target: crate::CameraId) -> crate::state_cache::StateCache {
        self.state.state_cache(target)
    }

    pub(crate) fn state_mut(&mut self) -> &mut OwnerState {
        &mut self.state
    }

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
            self.submit_with_timeout(driver, request, timeout)
                .map(|core| BlockingOperationReceipt {
                    core,
                    affected_axes,
                    settlement,
                    marker: PhantomData,
                })
        })
    }

    /// Admit and, when this exact request already wins the global dispatch
    /// race, perform its first write. A request that cannot win yet stays
    /// queued in the engine and is written by a later owner turn. No receive
    /// method is called here, so ACK/completion can only be consumed by an
    /// explicit pump.
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
        self.enter()?;
        let result = self.submit_inner(driver, request, configured_timeout, None);
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
        // Match the async owner boundary: once the caller-owned observer
        // deadline has elapsed, reject before staging admission or writing a
        // new inquiry.
        if Instant::now() >= deadline {
            return Err(Error::Timeout);
        }
        self.enter()?;
        let result = self.submit_inner(driver, request, configured_timeout, Some(deadline));
        self.leave();
        result
    }

    fn submit_inner<D: BlockingWireDriver + ?Sized>(
        &mut self,
        driver: &mut D,
        request: RuntimeRequest,
        configured_timeout: Duration,
        observer_deadline: Option<Instant>,
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

        // Issue #561: losing the global dispatch race is not backpressure.
        // Admission capacity (`max_pending_queue_depth`) already bounds how much
        // work may be outstanding, so a request that cannot be written yet stays
        // in the engine's ready queue and is dispatched by a later owner turn —
        // exactly the way the async facade behaves.
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
                    // Another request owns the only eligible socket right now.
                    // Leave this one queued; no peer request, deadline, pacing,
                    // or cancellation state is mutated here.
                    queued = true;
                    break;
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

        if let Some(error) = buffered_submission_error(&completion) {
            return Err(error);
        }
        if !queued {
            let first_write = report.first_write_for(id).ok_or_else(|| {
                Error::InvalidState("blocking request lost its first-write result".into())
            })?;
            first_write?;
        }
        Ok(ReceiptCore::new(
            id,
            target,
            completion,
            configured_timeout,
            origin,
        ))
    }

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
        let result = self.pump_once_inner(driver, reader, decoder, observer_deadline);
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
    ) -> Result<usize, Error>
    where
        D: BlockingWireDriver + ?Sized,
        R: BlockingReadDriver + ?Sized,
        F: BlockingFrameDecoder + ?Sized,
    {
        let owner_deadline = min_deadline(self.state.next_wake(), observer_deadline);
        let (received, received_at) =
            match reader.receive(self.state.buffers().receive_mut(), owner_deadline) {
                Ok(BlockingReceive::TimedOut) => {
                    let now = Instant::now();
                    if self.state.next_wake().is_some_and(|wake| wake <= now) {
                        let effects = self.state.advance(now);
                        self.drive(driver, effects);
                    }
                    return Ok(0);
                }
                Ok(BlockingReceive::Bytes(0)) => {
                    let effects = self.state.input(
                        Input::Shutdown(ShutdownReason::TransportClosed { reason: None }),
                        Instant::now(),
                    );
                    let _ = self.drive(driver, effects);
                    return Err(Error::ConnectionClosed { reason: None });
                }
                Ok(BlockingReceive::Bytes(received)) => (received, Instant::now()),
                Err(error) if super::receive_fault_is_transient(&error) => {
                    // 1.x parity: retry every command still waiting for its
                    // ACK and keep pumping. The read consumed nothing, so
                    // framing state is intact and this pump simply produced no
                    // frames.
                    let effects = self
                        .state
                        .input(Input::ReceiveFault { error }, Instant::now());
                    let _ = self.drive(driver, effects);
                    pause_after_transient_receive_fault(owner_deadline);
                    return Ok(0);
                }
                Err(error) => {
                    // A fatal read proves the connection is gone; it says
                    // nothing about the byte-stream *position*, which is what
                    // poison means. Framing failures below still poison a
                    // stream, exactly as the async owner does.
                    let effects = self.state.input(
                        Input::Close {
                            reason: Some(error.to_string().into_boxed_str()),
                        },
                        Instant::now(),
                    );
                    let _ = self.drive(driver, effects);
                    // Report the close, not the raw read fault that caused it —
                    // the same verdict the zero-byte arm above returns. The
                    // cause survives in the close reason.
                    return Err(self.boundary_error_or(error));
                }
            };

        let frame_limit = self.state.policy().limits.frames_per_receive;
        let frames = match decoder.decode(self.state.buffers(), received, frame_limit) {
            Ok(frames) => frames,
            Err(error) => {
                if self.state.policy().protocol.transport == TransportKind::Stream {
                    let effects = self.state.input(
                        Input::Poison {
                            reason: error.to_string().into_boxed_str(),
                        },
                        Instant::now(),
                    );
                    let _ = self.drive(driver, effects);
                }
                return Err(error);
            }
        };
        if let Err(error) = self.state.validate_frame_batch(&frames) {
            if self.state.policy().protocol.transport == TransportKind::Stream {
                let effects = self.state.input(
                    Input::Poison {
                        reason: error.to_string().into_boxed_str(),
                    },
                    Instant::now(),
                );
                let _ = self.drive(driver, effects);
            }
            return Err(error);
        }
        let count = frames.len();
        self.drive_decoded_batch(driver, frames, received_at);
        Ok(count)
    }

    fn cancel_core<D: BlockingWireDriver + ?Sized>(
        &mut self,
        driver: &mut D,
        receipt: ReceiptCore,
    ) -> Result<BlockingCancellationReceipt, Error> {
        self.enter()?;
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
        acknowledged??;
        Ok(BlockingCancellationReceipt {
            core: cancellation_receipt_for(receipt, None),
        })
    }

    #[cfg(test)]
    pub(crate) fn cancel_test<D: BlockingWireDriver>(
        &mut self,
        driver: &mut D,
        receipt: ReceiptCore,
    ) -> Result<BlockingCancellationReceipt, Error> {
        self.cancel_core(driver, receipt)
    }

    /// Run only due scheduler work. This is intentionally distinct from a
    /// receive pump and is used for scheduler deadlines and pacing wakes.
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

    pub(crate) fn shutdown<D: BlockingWireDriver + ?Sized>(
        &mut self,
        driver: &mut D,
    ) -> Result<(), Error> {
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

    /// Applies one validated decoded batch at a single owner-sampled instant.
    /// Every frame's effects, including identified write completions, are
    /// drained recursively before the next frame is applied. Due work runs
    /// exactly once after the complete source-ordered batch.
    pub(crate) fn drive_decoded_batch<D: BlockingWireDriver + ?Sized>(
        &mut self,
        driver: &mut D,
        frames: Vec<DecodedFrame>,
        received_at: Instant,
    ) {
        let turn = self.state.begin_input_turn(received_at);
        for frame in frames {
            let effects = self.state.input_in_turn(&turn, Input::Frame(frame));
            self.drive_in_turn(driver, &turn, effects);
        }
        let due = self.state.finish_input_turn(turn);
        let _ = self.drive(driver, due);
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

    #[cfg(test)]
    pub(super) fn mark_pumping_for_test(&mut self) {
        self.pumping = true;
    }
}

/// Pause applied after a transient receive fault so a transport that fails
/// immediately cannot spin a caller's pump loop. 1.x used the same bound.
const TRANSIENT_RECEIVE_PAUSE: Duration = Duration::from_millis(10);

fn pause_after_transient_receive_fault(owner_deadline: Option<Instant>) {
    let now = Instant::now();
    let pause = match owner_deadline {
        Some(deadline) if deadline <= now => return,
        Some(deadline) => TRANSIENT_RECEIVE_PAUSE.min(deadline.duration_since(now)),
        None => TRANSIENT_RECEIVE_PAUSE,
    };
    std::thread::sleep(pause);
}

fn min_deadline(left: Option<Instant>, right: Option<Instant>) -> Option<Instant> {
    match (left, right) {
        (Some(left), Some(right)) => Some(left.min(right)),
        (Some(value), None) | (None, Some(value)) => Some(value),
        (None, None) => None,
    }
}
