//! Pure profile-aware lowering from typed requests to inert engine inputs.

use std::{
    marker::PhantomData,
    sync::Arc,
    time::{Duration, Instant},
};

use crate::session_config::ValidatedTuning;
#[cfg(test)]
use crate::CameraId;
use crate::{
    camera::{MotionQuery, MovementTolerance, PanTiltPosition},
    command::{
        inquiry::{
            FocusPositionInquiry, IrisInquiry, NdFilterInquiry, PanTiltPositionInquiry,
            ZoomPositionInquiry,
        },
        NdFilterValue,
    },
    completion,
    profile::{RetryBounds, DEFAULT_RETRY_BASE},
    target::TargetSlot,
    timeout::CommandCategory,
    types::{FocusPosition, IrisLevel, ZoomPosition},
    AffectedAxes, ControlClass, Inquiry, OperationCommand, OperationalTuning, PlainCommand,
    ProfileSpec, Request, ResponseDecoder, RetryClass, SubmissionClass, TimeoutClass,
};
use crate::{Error, Result};

use crate::raw::MAX_BYTES;
use crate::runtime::engine::{
    AppliedStateProjection, CancellationPolicy, ControlPolicy, EncodedMessage, ReplyShape,
    RequestContext, RetryPolicy, RuntimeRequest, TimeoutPolicy,
};

/// How one submission's scheduling lane is chosen.
///
/// Every request already classifies itself through [`Request::control_class`].
/// A camera view may carry ordinary submission QoS. It cannot represent or
/// demote the safety lane.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(crate) enum ClassSelection {
    /// Use the request's own classification.
    #[default]
    Request,
    /// A camera handle's ordinary-traffic default.
    Handle(SubmissionClass),
}

impl ClassSelection {
    /// Resolves the effective class for a request that classifies itself as
    /// `request`.
    ///
    /// The crate classifies stop and protocol-cancel requests as urgent. That
    /// intrinsic safety floor always wins; ordinary requests use the caller's
    /// selected QoS when one exists.
    pub(crate) const fn resolve(self, request: ControlClass) -> ControlClass {
        match self {
            Self::Request => request,
            Self::Handle(class) => match request {
                ControlClass::Urgent => ControlClass::Urgent,
                _ => class.control_class(),
            },
        }
    }

    /// Returns the handle default this selection carries, if any.
    pub(crate) const fn handle_default(self) -> Option<SubmissionClass> {
        match self {
            Self::Handle(class) => Some(class),
            Self::Request => None,
        }
    }

    /// Builds a handle selection from an optional default.
    pub(crate) const fn from_handle_default(class: Option<SubmissionClass>) -> Self {
        match class {
            Some(class) => Self::Handle(class),
            None => Self::Request,
        }
    }
}

/// Complete erased protocol-settlement selection made before admission.
// The poll plan is intentionally kept inline: moving it behind a box would
// add an allocation to every targeted preparation.
#[allow(clippy::large_enum_variant)]
#[derive(Debug, Clone)]
pub(crate) enum SettlementPlan {
    /// The profile declares the applied protocol completion as settlement evidence.
    CompletionIsSettled { default_budget: Duration },
    /// Settlement must poll the exact affected axes.
    Poll {
        queries: PositionQueryPlan,
        tolerance: MovementTolerance,
        interval: Duration,
        default_budget: Duration,
    },
}

/// Wire bytes are owned during ordinary preparation. A polling template shares
/// its already prepared bytes across repeated samples.
#[derive(Debug)]
enum PreparedWire {
    Owned(EncodedMessage),
    Shared(Arc<EncodedMessage>),
}

impl PreparedWire {
    #[cfg(test)]
    fn as_bytes(&self) -> &[u8] {
        match self {
            Self::Owned(message) => message.as_bytes(),
            Self::Shared(message) => message.as_bytes(),
        }
    }

    fn into_shared(self) -> Arc<EncodedMessage> {
        match self {
            Self::Owned(message) => Arc::new(message),
            Self::Shared(message) => message,
        }
    }
}

/// Reusable, inert lowering of one typed inquiry. Re-instantiation preserves
/// the exact wire, decoder, target, and protocol policy selected during
/// operation preparation.
#[derive(Debug, Clone)]
pub(crate) struct PreparedInquiryTemplate<R> {
    wire: Arc<EncodedMessage>,
    context: RequestContext,
    decoder: ResponseDecoder<R>,
}

impl<R> PreparedInquiryTemplate<R> {
    pub(crate) fn instantiate(&self) -> PreparedInquiry<R> {
        PreparedInquiry {
            wire: PreparedWire::Shared(Arc::clone(&self.wire)),
            context: self.context,
            decoder: self.decoder.clone(),
        }
    }
}

impl SettlementPlan {
    pub(crate) const fn default_budget(&self) -> Duration {
        match self {
            Self::CompletionIsSettled { default_budget, .. }
            | Self::Poll { default_budget, .. } => *default_budget,
        }
    }
}

/// One selected position inquiry and the answer from its preceding sample.
/// Absence means no baseline has been sampled yet, never an omitted axis.
#[derive(Debug, Clone)]
pub(crate) struct PositionQuery<R> {
    inquiry: PreparedInquiryTemplate<R>,
    previous: Option<R>,
}

impl<R> PositionQuery<R> {
    fn new(inquiry: PreparedInquiryTemplate<R>) -> Self {
        Self {
            inquiry,
            previous: None,
        }
    }

    pub(crate) fn instantiate(&self) -> PreparedInquiry<R> {
        self.inquiry.instantiate()
    }
}

pub(crate) trait PositionReading: Copy {
    fn moved(self, previous: Self, tolerance: MovementTolerance) -> bool;
}

impl PositionReading for PanTiltPosition {
    fn moved(self, previous: Self, tolerance: MovementTolerance) -> bool {
        let (pan, tilt) = self.raw_values();
        let (previous_pan, previous_tilt) = previous.raw_values();
        let tolerance = u64::from(tolerance.pan_tilt.unsigned_abs());
        (i64::from(pan) - i64::from(previous_pan)).unsigned_abs() > tolerance
            || (i64::from(tilt) - i64::from(previous_tilt)).unsigned_abs() > tolerance
    }
}

macro_rules! scalar_position_readings {
    ($($type:ty => $field:ident),+ $(,)?) => {$(
        impl PositionReading for $type {
            fn moved(self, previous: Self, tolerance: MovementTolerance) -> bool {
                u64::from(self.value().abs_diff(previous.value())) > u64::from(tolerance.$field)
            }
        }
    )+};
}

scalar_position_readings! {
    ZoomPosition => zoom, FocusPosition => focus, IrisLevel => iris, NdFilterValue => nd_filter,
}

impl<R: PositionReading> PositionQuery<R> {
    /// Records a successful reply. A first sample needs a second one before
    /// it can prove stability, so it conservatively reports movement.
    pub(crate) fn record(&mut self, value: R, tolerance: MovementTolerance) -> bool {
        self.previous
            .replace(value)
            .is_none_or(|previous| value.moved(previous, tolerance))
    }
}

/// Complete set of typed position inquiries selected for one operation.
#[derive(Debug, Clone)]
pub(crate) struct PositionQueryPlan {
    pub(crate) pan_tilt: Option<PositionQuery<PanTiltPosition>>,
    pub(crate) zoom: Option<PositionQuery<ZoomPosition>>,
    pub(crate) focus: Option<PositionQuery<FocusPosition>>,
    pub(crate) iris: Option<PositionQuery<IrisLevel>>,
    pub(crate) nd_filter: Option<PositionQuery<NdFilterValue>>,
}

impl PositionQueryPlan {
    /// Budget one complete sample from the exact inquiries the plan lowers.
    /// Each inquiry's observer covers its own retry budget, and pacing may
    /// delay its admission after the preceding query.
    fn snapshot_budget(&self) -> Option<Duration> {
        [
            self.pan_tilt.as_ref().map(|query| &query.inquiry.context),
            self.zoom.as_ref().map(|query| &query.inquiry.context),
            self.focus.as_ref().map(|query| &query.inquiry.context),
            self.iris.as_ref().map(|query| &query.inquiry.context),
            self.nd_filter.as_ref().map(|query| &query.inquiry.context),
        ]
        .into_iter()
        .flatten()
        .try_fold(Duration::ZERO, |budget, context| {
            budget
                .checked_add(observation_timeout(context, context.timeout.inquiry))?
                .checked_add(context.control.minimum_spacing)
        })
    }

    fn prepare(
        registration: &crate::TargetRegistration,
        tuning: ValidatedTuning,
        axes: AffectedAxes,
    ) -> Result<Self> {
        Ok(Self {
            pan_tilt: axes
                .contains(AffectedAxes::PAN_TILT)
                .then(|| {
                    prepare_inquiry(
                        &PanTiltPositionInquiry,
                        registration,
                        tuning,
                        ClassSelection::Request,
                    )
                })
                .transpose()?
                .map(|prepared| PositionQuery::new(prepared.into_template())),
            zoom: axes
                .contains(AffectedAxes::ZOOM)
                .then(|| {
                    prepare_inquiry(
                        &ZoomPositionInquiry,
                        registration,
                        tuning,
                        ClassSelection::Request,
                    )
                })
                .transpose()?
                .map(|prepared| PositionQuery::new(prepared.into_template())),
            focus: axes
                .contains(AffectedAxes::FOCUS)
                .then(|| {
                    prepare_inquiry(
                        &FocusPositionInquiry,
                        registration,
                        tuning,
                        ClassSelection::Request,
                    )
                })
                .transpose()?
                .map(|prepared| PositionQuery::new(prepared.into_template())),
            iris: axes
                .contains(AffectedAxes::IRIS)
                .then(|| {
                    prepare_inquiry(&IrisInquiry, registration, tuning, ClassSelection::Request)
                })
                .transpose()?
                .map(|prepared| PositionQuery::new(prepared.into_template())),
            nd_filter: axes
                .contains(AffectedAxes::ND_FILTER)
                .then(|| {
                    prepare_inquiry(
                        &NdFilterInquiry,
                        registration,
                        tuning,
                        ClassSelection::Request,
                    )
                })
                .transpose()?
                .map(|prepared| PositionQuery::new(prepared.into_template())),
        })
    }
}

/// Prepares the complete set of position inquiries for one explicitly
/// selected motion-axis set.
///
/// This is kept next to the settlement lowering so standalone motion
/// observation and targeted-operation settlement cannot drift in either their
/// profile admission checks or their selected wire queries.
pub(crate) fn prepare_position_queries(
    registration: &crate::TargetRegistration,
    tuning: ValidatedTuning,
    axes: AffectedAxes,
) -> Result<PositionQueryPlan> {
    let profile = registration.profile();
    if !profile.supports_axes(axes) {
        return Err(Error::FeatureNotSupported {
            feature: "selected motion axes are not supported by the profile",
        });
    }
    if !profile.position_inquiries().supports(axes) {
        return Err(Error::FeatureNotSupported {
            feature: "position inquiries required for the selected motion axes",
        });
    }
    PositionQueryPlan::prepare(registration, tuning, axes)
}

/// Temporal contract for one `is_moving` observation, shared by every facade
/// (#781).
///
/// The facade supplies owner-clock instants; this type decides what they
/// prove. The baseline's readings were all taken no later than the instant it
/// was received, and the final snapshot may start only once `window` has
/// elapsed after that. Every selected axis is therefore compared across at
/// least `window`, however quickly the camera answers each inquiry.
#[derive(Debug, Clone, Copy)]
pub(crate) struct MotionWindow {
    window: Duration,
    tolerance: MovementTolerance,
}

/// A [`MotionWindow`] whose baseline snapshot was received: the final
/// snapshot may start at [`Self::final_not_before`].
#[derive(Debug, Clone, Copy)]
pub(crate) struct OpenMotionWindow {
    final_not_before: Instant,
}

impl MotionWindow {
    /// Validates the query before any I/O.
    pub(crate) fn new(query: MotionQuery) -> Result<Self> {
        if query.window.is_zero() {
            return Err(Error::InvalidParameter {
                parameter: "MotionQuery::window",
                value: "0ns".into(),
                reason: "a movement observation needs a positive window".into(),
            });
        }
        Ok(Self {
            window: query.window,
            tolerance: query.tolerance,
        })
    }

    pub(crate) const fn tolerance(&self) -> MovementTolerance {
        self.tolerance
    }

    /// The single absolute budget for the whole observation: both snapshots'
    /// admission and replies plus the intervening wait.
    pub(crate) fn budget(&self, queries: &PositionQueryPlan) -> Result<Duration> {
        queries
            .snapshot_budget()
            .and_then(|snapshot| snapshot.checked_mul(2))
            .and_then(|snapshots| self.window.checked_add(snapshots))
            .ok_or_else(|| Error::InvalidParameter {
                parameter: "MotionQuery::window",
                value: format!("{:?}", self.window).into(),
                reason: "the observation deadline is not representable".into(),
            })
    }

    /// Records the baseline received at `received_at`.
    ///
    /// A window that cannot elapse before `deadline` is insufficient evidence
    /// and fails with [`Error::Timeout`] rather than reporting no movement.
    pub(crate) fn observe_baseline(
        self,
        received_at: Instant,
        deadline: Instant,
    ) -> Result<OpenMotionWindow> {
        let final_not_before = received_at
            .checked_add(self.window)
            .filter(|not_before| *not_before < deadline)
            .ok_or_else(Error::query_timeout)?;
        Ok(OpenMotionWindow { final_not_before })
    }
}

impl OpenMotionWindow {
    /// The earliest instant the final snapshot may start.
    pub(crate) const fn final_not_before(&self) -> Instant {
        self.final_not_before
    }

    /// Accepts the plan's final comparison, whose first inquiry started at
    /// `started_at`, only once the observation window has elapsed.
    pub(crate) fn observe_final(self, moving: bool, started_at: Instant) -> Result<bool> {
        if started_at < self.final_not_before {
            return Err(Error::InvalidState(
                "final movement snapshot started before the observation window elapsed".into(),
            ));
        }
        Ok(moving)
    }
}

/// What a settlement poll's driver does after one snapshot.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum SettlementStep {
    /// Two consecutive snapshots agreed within tolerance.
    Settled,
    /// Sleep this long on the owner clock, then take the next snapshot.
    SampleAfter(Duration),
}

/// Position-polling proof that the selected axes are idle, shared by
/// targeted-operation settlement and `wait_until_idle` on every facade (#802).
///
/// A baseline snapshot, then one every `interval`, until two consecutive
/// snapshots agree within tolerance, all before one absolute `deadline`. The
/// selected query slots compare their answers. The driver supplies that
/// movement verdict and its owner-clock instant; this type selects the next
/// pause or the deadline failure.
#[derive(Debug, Clone, Copy)]
pub(crate) struct SettlementPoll {
    interval: Duration,
    deadline: Instant,
}

impl SettlementPoll {
    pub(crate) const fn new(interval: Duration, deadline: Instant) -> Self {
        Self { interval, deadline }
    }

    /// A plan's first sample always reports movement, so only two successful
    /// agreeing samples can settle. Replies are compared in their own slots.
    pub(crate) fn observe(&mut self, moving: bool, now: Instant) -> Result<SettlementStep> {
        if !moving {
            return Ok(SettlementStep::Settled);
        }
        let remaining = self.deadline.saturating_duration_since(now);
        if remaining.is_zero() {
            return Err(Error::query_timeout());
        }
        Ok(SettlementStep::SampleAfter(self.interval.min(remaining)))
    }
}

/// A fully lowered ordinary command.
#[derive(Debug)]
pub(crate) struct PreparedCommand {
    wire: PreparedWire,
    context: RequestContext,
    reply_shape: ReplyShape,
    applied_state: Option<AppliedStateProjection>,
}

/// A fully lowered typed inquiry. Decoding remains outside the engine.
#[derive(Debug)]
pub(crate) struct PreparedInquiry<R> {
    wire: PreparedWire,
    context: RequestContext,
    decoder: ResponseDecoder<R>,
}

/// A fully lowered operation with compile-time completion semantics.
#[derive(Debug)]
pub(crate) struct PreparedOperation<K>
where
    K: completion::Kind,
{
    wire: PreparedWire,
    context: RequestContext,
    reply_shape: ReplyShape,
    affected_axes: AffectedAxes,
    settlement: completion::Settlement<K>,
    applied_state: Option<AppliedStateProjection>,
    marker: PhantomData<fn() -> K>,
}

pub(crate) fn lower_targeted_settlement(
    registration: &crate::TargetRegistration,
    tuning: ValidatedTuning,
    axes: AffectedAxes,
    default_budget: Duration,
) -> Result<SettlementPlan> {
    let profile = registration.profile();
    if !profile.supports_axes(axes) {
        return Err(Error::FeatureNotSupported {
            feature: "affected operation axes are not supported by the profile",
        });
    }
    if profile.supports_operation_complete() {
        return Ok(SettlementPlan::CompletionIsSettled { default_budget });
    }
    if !profile.position_inquiries().supports(axes) {
        return Err(Error::FeatureNotSupported {
            feature: "position inquiries required to settle the affected axes",
        });
    }
    Ok(SettlementPlan::Poll {
        queries: prepare_position_queries(registration, tuning, axes)?,
        tolerance: MovementTolerance::default(),
        interval: tuning
            .get()
            .inquiry_spacing_override()
            .unwrap_or_else(|| profile.timing().minimum_inquiry_spacing())
            .max(Duration::from_millis(25)),
        default_budget,
    })
}

pub(crate) fn lower_applied_only_settlement(
    registration: &crate::TargetRegistration,
    _tuning: ValidatedTuning,
    axes: AffectedAxes,
    _default_budget: Duration,
) -> Result<()> {
    let profile = registration.profile();
    if profile.supports_axes(axes) {
        Ok(())
    } else {
        Err(Error::FeatureNotSupported {
            feature: "affected operation axes are not supported by the profile",
        })
    }
}

/// Prepares a generic plain command through the shared lowering path.
pub(crate) fn prepare_command<C>(
    command: &C,
    registration: &crate::TargetRegistration,
    tuning: ValidatedTuning,
    class: ClassSelection,
) -> Result<PreparedCommand>
where
    C: PlainCommand + ?Sized,
{
    prepare_command_with_state(
        command,
        registration,
        tuning,
        class,
        crate::requests::applied_state_projection(command),
    )
}

/// Prepares a crate built-in command with its closed applied-state selection.
#[cfg(test)]
pub(crate) fn prepare_builtin_command<C>(
    command: &C,
    registration: &crate::TargetRegistration,
    tuning: ValidatedTuning,
) -> Result<PreparedCommand>
where
    C: PlainCommand + crate::request::builtin::BuiltinValidation + ?Sized,
{
    prepare_command(command, registration, tuning, ClassSelection::Request)
}

fn prepare_command_with_state<C>(
    command: &C,
    registration: &crate::TargetRegistration,
    tuning: ValidatedTuning,
    class: ClassSelection,
    applied_state: Option<AppliedStateProjection>,
) -> Result<PreparedCommand>
where
    C: PlainCommand + ?Sized,
{
    validate_timeout_class(command.timeout_class(), false)?;
    command.validate_for_target(registration)?;
    let wire = encode(command, registration)?;
    let mut context = request_context(command, registration, tuning, class, false)?;
    context.motion = command
        .motion_axes()
        .map(|axes| crate::runtime::engine::MotionEffect { axes, stop: false });
    Ok(PreparedCommand {
        wire,
        context,
        reply_shape: lower_reply_shape(command.reply_shape()),
        applied_state,
    })
}

/// Prepares a generic typed inquiry through the shared lowering path.
pub(crate) fn prepare_inquiry<Q>(
    inquiry: &Q,
    registration: &crate::TargetRegistration,
    tuning: ValidatedTuning,
    class: ClassSelection,
) -> Result<PreparedInquiry<Q::Response>>
where
    Q: Inquiry + ?Sized,
{
    let profile = registration.profile();
    // An inquiry has exactly one protocol lifecycle: its reply is the
    // terminal evidence. Do not merely ignore a downstream override here;
    // doing so would let an erased/custom inquiry advertise a reply shape the
    // engine never honors. This is deliberately shared by every preparation
    // entry point, not just raw::Inquiry's constructor.
    if !matches!(
        inquiry.reply_shape(),
        crate::raw::RawReplyShape::AckThenCompletion
    ) {
        return Err(Error::InvalidRequest(
            "inquiry reply shape must be RawReplyShape::AckThenCompletion; an inquiry always awaits its reply"
                .into(),
        ));
    }
    validate_timeout_class(inquiry.timeout_class(), true)?;
    if matches!(
        profile.capabilities().inquiry_support,
        crate::capabilities::InquirySupport::None
    ) {
        return Err(Error::FeatureNotSupported {
            feature: "VISCA inquiries",
        });
    }
    inquiry.validate_for_target(registration)?;
    let wire = encode(inquiry, registration)?;
    let mut context = request_context(inquiry, registration, tuning, class, true)?;
    context.retry.builtin_inquiry_syntax = crate::requests::builtin_inquiry_syntax_retry(inquiry);
    Ok(PreparedInquiry {
        wire,
        context,
        decoder: inquiry.decoder_for_profile(profile),
    })
}

/// Prepares a generic operation through the shared lowering path.
pub(crate) fn prepare_operation<K, O>(
    operation: &O,
    registration: &crate::TargetRegistration,
    tuning: ValidatedTuning,
    class: ClassSelection,
) -> Result<PreparedOperation<K>>
where
    K: completion::Kind,
    O: OperationCommand<K> + ?Sized,
{
    prepare_operation_with_state(
        operation,
        registration,
        tuning,
        class,
        crate::requests::applied_state_projection(operation),
    )
}

/// Prepares a crate built-in operation with its closed applied-state selection.
#[cfg(test)]
pub(crate) fn prepare_builtin_operation<K, O>(
    operation: &O,
    registration: &crate::TargetRegistration,
    tuning: ValidatedTuning,
) -> Result<PreparedOperation<K>>
where
    K: completion::Kind,
    O: OperationCommand<K> + crate::request::builtin::BuiltinValidation + ?Sized,
{
    prepare_operation::<K, _>(operation, registration, tuning, ClassSelection::Request)
}

fn prepare_operation_with_state<K, O>(
    operation: &O,
    registration: &crate::TargetRegistration,
    tuning: ValidatedTuning,
    class: ClassSelection,
    applied_state: Option<AppliedStateProjection>,
) -> Result<PreparedOperation<K>>
where
    K: completion::Kind,
    O: OperationCommand<K> + ?Sized,
{
    let profile = registration.profile();
    // An operation handle's `applied()` is a protocol-evidence promise. A
    // no-reply command can prove only a successful local write, so keep that
    // fire-and-forget shape on the plain-command path even for downstream or
    // dynamically erased operation implementations that bypass raw wrappers.
    if matches!(operation.reply_shape(), crate::raw::RawReplyShape::NoReply) {
        return Err(Error::InvalidRequest(
            "operations cannot use RawReplyShape::NoReply; operation application requires a terminal camera response"
                .into(),
        ));
    }
    validate_timeout_class(operation.timeout_class(), false)?;
    // Domain/profile admission must complete before lowering settlement
    // inquiries or encoding the command.  In particular, an unsupported
    // targeted built-in must not even prepare an inquiry plan as a side
    // effect of discovering that its operation class is unavailable.
    operation.validate_for_target(registration)?;
    let affected_axes = operation.affected_axes();
    let mut context = request_context(operation, registration, tuning, class, false)?;
    let stop = context.control.class == crate::runtime::engine::ControlClass::Urgent;
    context.motion = Some(crate::runtime::engine::MotionEffect {
        axes: affected_axes,
        // Urgent admission authority comes only from the sealed built-in STOP hook.
        stop,
    });
    if stop {
        // A STOP the camera finds not executable (`0x41`, for example a focus
        // STOP while auto-focus owns the lens) is refused for a standing
        // reason a resend cannot change, and its caller needs that verdict
        // promptly. Capacity rejections (`0x03`/`0x05`) still retry (#795).
        context.retry.movement_not_executable = false;
    }
    let settlement = completion::lower_settlement::<K>(
        registration,
        tuning,
        affected_axes,
        observation_timeout(
            &context,
            settlement_budget(operation.timeout_class(), profile, tuning.get()),
        ),
    )?;
    let wire = encode(operation, registration)?;
    Ok(PreparedOperation {
        wire,
        context,
        reply_shape: lower_reply_shape(operation.reply_shape()),
        affected_axes,
        settlement,
        applied_state,
        marker: PhantomData,
    })
}

fn encode<R>(request: &R, registration: &crate::TargetRegistration) -> Result<PreparedWire>
where
    R: Request + ?Sized,
{
    let target = registration.slot();
    let profile = registration.profile();
    let declared_max = crate::requests::declared_max_size(request);
    let capacity = declared_max.min(MAX_BYTES);
    if capacity < 2 {
        return Err(Error::InvalidRequest(
            "MAX_SIZE must allow a VISCA target address and message body".into(),
        ));
    }
    let message = EncodedMessage::encode(capacity, |bytes| {
        let written = match request.write_into(target.camera(), bytes) {
            Err(Error::BufferTooSmall { required, .. }) if required > capacity => {
                return Err(Error::InvalidRequest(
                format!("encoded request needs {required} bytes, exceeding MAX_SIZE ({declared_max}) or raw::MAX_BYTES ({MAX_BYTES})").into(),
            ));
            }
            result => result?,
        };
        if written > capacity {
            return Err(Error::InvalidRequest(
            format!("write_into reported {written} bytes, exceeding MAX_SIZE ({declared_max}) or raw::MAX_BYTES ({MAX_BYTES})").into(),
        ));
        }
        if written < 2 {
            return Err(Error::InvalidRequest(
                "encoded request must contain a VISCA target address and message body".into(),
            ));
        }
        if bytes[0] != target.camera().to_address_byte() {
            return Err(Error::InvalidRequest(
            format!(
                "request targets VISCA address 0x{:02x}, but the selected camera target is {} (0x{:02x})",
                bytes[0],
                target.camera(),
                target.camera().to_address_byte()
            )
            .into(),
        ));
        }
        Ok(written)
    })?;
    // The raw namespace and downstream `Request` implementations share this
    // final owner-only primitive guard.  Constructing the inert message first
    // preserves the existing terminator/size validation order; a rejection
    // here returns before the prepared request can be admitted or written to a
    // transport.
    crate::raw::reject_owner_only_primitive(message.as_bytes())?;
    let maximum = profile.maximum_request_bytes();
    if message.as_bytes().len() > maximum {
        return Err(Error::InvalidRequest(format!(
            "encoded request has {} bytes, exceeding this profile's supported envelope payload bound ({maximum})",
            message.as_bytes().len(),
        ).into()));
    }
    Ok(PreparedWire::Owned(message))
}

fn validate_timeout_class(class: TimeoutClass, inquiry: bool) -> Result<()> {
    let valid = CommandCategory::of(class).is_none() == inquiry;
    if valid {
        Ok(())
    } else {
        Err(Error::InvalidRequest(
            "request timeout class does not match its lifecycle kind".into(),
        ))
    }
}

fn request_context<R>(
    request: &R,
    registration: &crate::TargetRegistration,
    tuning: ValidatedTuning,
    class: ClassSelection,
    inquiry: bool,
) -> Result<RequestContext>
where
    R: Request + ?Sized,
{
    let target = registration.slot();
    let profile = registration.profile();
    let tuning = tuning.get();
    let timing = profile.timing();
    let completion = completion_timeout(request.timeout_class(), profile, tuning);
    let inquiry_timeout = tuning
        .inquiry_timeout_override()
        .unwrap_or(timing.inquiry_timeout());
    let timeout = TimeoutPolicy {
        ack: tuning
            .ack_timeout_override()
            .unwrap_or(timing.ack_timeout()),
        completion,
        inquiry: inquiry_timeout,
        cancellation: timing.cancellation_timeout(),
        ambiguity: timing.ambiguity_timeout(),
    };
    let spacing = if inquiry {
        tuning
            .inquiry_spacing_override()
            .unwrap_or(timing.minimum_inquiry_spacing())
    } else {
        tuning
            .command_spacing_override()
            .unwrap_or(timing.minimum_command_spacing())
    };
    let reply_shape = lower_reply_shape(request.reply_shape());
    let governing_deadline = if inquiry {
        timeout.inquiry
    } else {
        command_attempt_timeout(&timeout, reply_shape)
    };
    Ok(RequestContext {
        requires_extended_pan_tilt_speed: profile.pan_tilt_coordinates().is_some_and(
            |conversion| conversion.wire_codec() == crate::capabilities::PanTiltWireCodec::SonyFr7,
        ) && crate::requests::pan_tilt_max_speed(request)
            .is_some_and(|speed| speed > 24),
        motion: None,
        submission_order: 0,
        dispatch_deadline: None,
        target,
        timeout,
        retry: retry_policy(
            request.retry_class(),
            request.timeout_class(),
            tuning,
            timing.busy_timeout(),
            if inquiry {
                timeout.inquiry
            } else {
                timeout.completion
            },
            governing_deadline,
        ),
        control: ControlPolicy {
            class: lower_control(class.resolve(crate::requests::admission_control_class(request)?)),
            minimum_spacing: spacing,
        },
        cancellation: if profile.supports_command_cancel() {
            CancellationPolicy::Supported
        } else {
            CancellationPolicy::Unsupported
        },
    })
}

fn completion_timeout(
    class: TimeoutClass,
    profile: &ProfileSpec,
    tuning: OperationalTuning,
) -> Duration {
    let timeouts = profile.timing().command_timeouts();
    match CommandCategory::of(class) {
        Some(category) => tuning
            .command_override(category)
            .unwrap_or(timeouts.get(category)),
        // Inquiry requests use `TimeoutPolicy::inquiry`; keep the otherwise
        // unused completion field on the Quick timeout.
        None => timeouts.get(CommandCategory::Quick),
    }
}

fn settlement_budget(
    class: TimeoutClass,
    profile: &ProfileSpec,
    tuning: OperationalTuning,
) -> Duration {
    tuning
        .settlement_timeout_override()
        .unwrap_or_else(|| completion_timeout(class, profile, tuning))
}

/// Bounded retry count for one timeout category.
///
/// Budgets are keyed on the request's [`TimeoutClass`]: quick work gets two extra attempts because
/// it is cheap to replay, network work gets one fewer because a failing link rarely recovers within
/// a retry, and a long-running command gets exactly one attempt to spare the camera a second
/// multi-minute operation. Inquiries retain their own retry class while their response deadline is
/// selected from the inquiry timing fact.
const fn retry_budget(base: u32, class: TimeoutClass) -> u32 {
    match class {
        TimeoutClass::Quick | TimeoutClass::Inquiry => base.saturating_add(2),
        TimeoutClass::Movement | TimeoutClass::Preset => base,
        TimeoutClass::Network => {
            if base > 1 {
                base - 1
            } else {
                1
            }
        }
        TimeoutClass::LongRunning => 1,
    }
}

/// Lowers one request's retry policy.
///
/// `deadline` is the response window; `attempt` also includes the ACK window
/// for an ACK-bearing command. Preserve the established response-based retry
/// floor while ensuring a complete attempt fits within the budget. The
/// profile busy timeout covers camera-side recovery windows.
fn retry_policy(
    retry_class: RetryClass,
    timeout_class: TimeoutClass,
    tuning: OperationalTuning,
    busy_timeout: Duration,
    deadline: Duration,
    attempt: Duration,
) -> RetryPolicy {
    let bounds =
        tuning.lengthen_retry_bounds(RetryBounds::for_request(busy_timeout, deadline, attempt));
    let base = tuning.reduce_retry_base(DEFAULT_RETRY_BASE);
    let max_retries = if matches!(retry_class, RetryClass::Never) {
        0
    } else {
        retry_budget(base, timeout_class)
    };
    // `Never` is the sole policy opt-out from automatic replay. These flags
    // are only policy permissions: the engine narrows them further using
    // envelope evidence. Thus sequence-correlated Sony traffic may retry a
    // lost ACK (same-sequence retransmission, docs/visca_reference.md §5.3),
    // while a successfully sent raw command is never replayed on an ambiguous
    // outcome, and no command is written again after its ACK on either
    // envelope (#795): `completion_timeout` only governs a command that has no
    // ACK phase.
    let replayable = !matches!(retry_class, RetryClass::Never);
    RetryPolicy {
        max_retries,
        initial_backoff: bounds.initial_backoff,
        maximum_backoff: bounds.maximum_backoff,
        total_budget: bounds.total_budget,
        ack_timeout: replayable,
        completion_timeout: replayable,
        inquiry_timeout: matches!(retry_class, RetryClass::Inquiry),
        buffer_full: replayable,
        movement_not_executable: matches!(retry_class, RetryClass::Movement | RetryClass::Preset),
        builtin_inquiry_syntax: false,
    }
}

const fn lower_control(class: ControlClass) -> crate::runtime::engine::ControlClass {
    match class {
        ControlClass::Background => crate::runtime::engine::ControlClass::Background,
        ControlClass::Normal => crate::runtime::engine::ControlClass::Normal,
        ControlClass::User => crate::runtime::engine::ControlClass::User,
        ControlClass::Urgent => crate::runtime::engine::ControlClass::Urgent,
    }
}

const fn lower_reply_shape(shape: crate::raw::RawReplyShape) -> ReplyShape {
    match shape {
        crate::raw::RawReplyShape::AckThenCompletion => ReplyShape::AckThenCompletion,
        crate::raw::RawReplyShape::CompletionOnly => ReplyShape::CompletionOnly,
        crate::raw::RawReplyShape::NoReply => ReplyShape::NoReply,
    }
}

impl PreparedCommand {
    pub(crate) fn admit_with<T>(self, admit: impl FnOnce(RuntimeRequest, Duration) -> T) -> T {
        let timeout = observation_timeout(
            &self.context,
            command_attempt_timeout(&self.context.timeout, self.reply_shape),
        );
        let request = RuntimeRequest::Command {
            reply_shape: self.reply_shape,
            wire: self.wire.into_shared(),
            context: self.context,
            applied_state: self.applied_state,
        };
        admit(request, timeout)
    }
}

/// Configured observer deadlines of one admitted operation (#777).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct OperationTimeouts {
    /// Bounds a configured wait for application.
    pub(crate) applied: Duration,
    /// Bounds a configured wait for a cancellation's conclusion.
    pub(crate) cancellation: Duration,
}

/// Returns the default lifetime for a caller observer.
///
/// An observer that expires while its request can still be retried would turn
/// detach into an unintentional physical fire-and-forget operation. Keep the
/// command, operation, and inquiry paths on this one rule: an automatic retry
/// can never outlive the default observer. A zero total budget remains the
/// engine's explicit no-total-budget mode and therefore leaves the governing
/// deadline unchanged.
fn command_attempt_timeout(timeout: &TimeoutPolicy, shape: ReplyShape) -> Duration {
    match shape {
        ReplyShape::AckThenCompletion => timeout.ack.saturating_add(timeout.completion),
        ReplyShape::CompletionOnly | ReplyShape::NoReply => timeout.completion,
    }
}

fn observation_timeout(context: &RequestContext, governing_deadline: Duration) -> Duration {
    governing_deadline.max(context.retry.total_budget)
}

impl<R> PreparedInquiry<R> {
    fn into_template(self) -> PreparedInquiryTemplate<R> {
        PreparedInquiryTemplate {
            wire: self.wire.into_shared(),
            context: self.context,
            decoder: self.decoder,
        }
    }

    pub(crate) fn admit_with<T>(
        self,
        admit: impl FnOnce(RuntimeRequest, ResponseDecoder<R>, Duration) -> T,
    ) -> T {
        let timeout = observation_timeout(&self.context, self.context.timeout.inquiry);
        let request = RuntimeRequest::Inquiry {
            wire: self.wire.into_shared(),
            context: self.context,
        };
        admit(request, self.decoder, timeout)
    }
}

impl<K> PreparedOperation<K>
where
    K: completion::Kind,
{
    pub(crate) fn admit_with<T>(
        self,
        admit: impl FnOnce(
            RuntimeRequest,
            AffectedAxes,
            completion::Settlement<K>,
            OperationTimeouts,
        ) -> T,
    ) -> T {
        let applied = observation_timeout(
            &self.context,
            command_attempt_timeout(&self.context.timeout, self.reply_shape),
        );
        // Once cancellation is requested, the engine concludes the operation
        // within its cancellation ambiguity window plus the cancellation
        // observation window, and never before the original completion
        // deadline it already owned.
        let cancellation = applied.max(
            self.context
                .timeout
                .ambiguity
                .saturating_add(self.context.timeout.cancellation),
        );
        let request = RuntimeRequest::Command {
            reply_shape: self.reply_shape,
            wire: self.wire.into_shared(),
            context: self.context,
            applied_state: self.applied_state,
        };
        admit(
            request,
            self.affected_axes,
            self.settlement,
            OperationTimeouts {
                applied,
                cancellation,
            },
        )
    }
}

#[cfg(test)]
pub(crate) mod test_support {
    use super::*;

    fn validated(
        registration: &crate::TargetRegistration,
        tuning: OperationalTuning,
    ) -> Result<ValidatedTuning> {
        ValidatedTuning::validate(tuning, [registration.profile()])
    }

    pub(crate) fn prepare_command<C: PlainCommand + ?Sized>(
        command: &C,
        registration: &crate::TargetRegistration,
        tuning: OperationalTuning,
        class: ClassSelection,
    ) -> Result<PreparedCommand> {
        super::prepare_command(
            command,
            registration,
            validated(registration, tuning)?,
            class,
        )
    }

    pub(crate) fn prepare_inquiry<Q: Inquiry + ?Sized>(
        inquiry: &Q,
        registration: &crate::TargetRegistration,
        tuning: OperationalTuning,
        class: ClassSelection,
    ) -> Result<PreparedInquiry<Q::Response>> {
        super::prepare_inquiry(
            inquiry,
            registration,
            validated(registration, tuning)?,
            class,
        )
    }

    pub(crate) fn prepare_operation<K: completion::Kind, O: OperationCommand<K> + ?Sized>(
        operation: &O,
        registration: &crate::TargetRegistration,
        tuning: OperationalTuning,
        class: ClassSelection,
    ) -> Result<PreparedOperation<K>> {
        super::prepare_operation(
            operation,
            registration,
            validated(registration, tuning)?,
            class,
        )
    }

    pub(crate) fn prepare_builtin_command<
        C: PlainCommand + crate::request::builtin::BuiltinValidation + ?Sized,
    >(
        command: &C,
        registration: &crate::TargetRegistration,
        tuning: OperationalTuning,
    ) -> Result<PreparedCommand> {
        super::prepare_builtin_command(command, registration, validated(registration, tuning)?)
    }

    pub(crate) fn prepare_builtin_operation<
        K: completion::Kind,
        O: OperationCommand<K> + crate::request::builtin::BuiltinValidation + ?Sized,
    >(
        operation: &O,
        registration: &crate::TargetRegistration,
        tuning: OperationalTuning,
    ) -> Result<PreparedOperation<K>> {
        super::prepare_builtin_operation(operation, registration, validated(registration, tuning)?)
    }

    pub(crate) fn prepare_position_queries(
        registration: &crate::TargetRegistration,
        tuning: OperationalTuning,
        axes: AffectedAxes,
    ) -> Result<PositionQueryPlan> {
        super::prepare_position_queries(registration, validated(registration, tuning)?, axes)
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::panic, unused_qualifications)]
mod tests {
    use std::cell::Cell;

    use super::test_support::{
        prepare_builtin_command, prepare_builtin_operation, prepare_command, prepare_inquiry,
        prepare_operation, prepare_position_queries,
    };
    use super::*;
    use crate::{
        capabilities::{Capabilities, InquirySupport, TypedSupportSet},
        command::{
            BrightnessInquiry, ExposureModeInquiry, FocusNearLimitInquiry, FocusZoneInquiry,
            IrisInquiry, NdFilterValue, PanTilt, PanTiltLimitCorner, PanTiltPositionInquiry,
            PictureEffectInquiry, PowerInquiry, UsbAudioInquiry, VersionInquiry,
            ZoomPositionInquiry, VISCA_TERMINATOR,
        },
        profile::MINIMUM_RETRY_BUDGET,
        request::builtin::{
            request_write_count, reset_request_write_count, FocusTrigger, IrisDirect, IrisReset,
            NdFilterStepUp, PanTiltAbsolute, PanTiltLimitClear, PanTiltLimitSet, PanTiltRelative,
            PresetSet, ZoomStop, ZoomTarget,
        },
        types::{IrisLevel, PanSpeed, TiltSpeed, ZoomPosition},
        units::Degrees,
        PositionInquirySupport, PresetNumber, ProfileEnvelope, ProfileTiming,
        TransportCompatibility,
    };

    std::thread_local! {
        static WRITE_COUNT: Cell<usize> = const { Cell::new(0) };
    }

    #[test]
    fn fr7_extended_pan_tilt_speeds_carry_a_dispatch_prerequisite() {
        use crate::request::builtin::PanTiltDrive;
        let registration = crate::TargetRegistration::new(
            CameraId::CAMERA_1,
            ProfileSpec::from_compile_time::<crate::profiles::SonyFR7>().expect("FR7"),
        )
        .expect("target");
        for speed in [1, 24, 25, 50] {
            let pan = PanSpeed::new(speed).expect("pan syntax");
            let tilt = TiltSpeed::new(speed).expect("tilt syntax");
            let drive =
                PanTiltDrive::new(crate::command::PanTiltDirection::Up, pan, tilt).expect("drive");
            let prepared = prepare_operation::<completion::AppliedOnly, _>(
                &drive,
                &registration,
                OperationalTuning::new(),
                ClassSelection::Request,
            )
            .expect("FR7 drive");
            assert_eq!(
                prepared.context.requires_extended_pan_tilt_speed,
                speed > 24
            );
            let absolute =
                PanTiltAbsolute::for_target(Degrees(1.0), Degrees(1.0), pan, tilt, &registration)
                    .expect("target");
            let prepared = prepare_operation::<completion::Targeted, _>(
                &absolute,
                &registration,
                OperationalTuning::new(),
                ClassSelection::Request,
            )
            .expect("FR7 position");
            assert_eq!(
                prepared.context.requires_extended_pan_tilt_speed,
                speed > 24
            );
            let relative =
                PanTiltRelative::for_target(Degrees(1.0), Degrees(1.0), pan, tilt, &registration)
                    .expect("offset");
            let prepared = prepare_operation::<completion::Targeted, _>(
                &relative,
                &registration,
                OperationalTuning::new(),
                ClassSelection::Request,
            )
            .expect("FR7 offset");
            assert_eq!(
                prepared.context.requires_extended_pan_tilt_speed,
                speed > 24
            );
        }
        let normal = crate::TargetRegistration::new(
            CameraId::CAMERA_1,
            ProfileSpec::from_compile_time::<crate::profiles::PtzOpticsG2>().expect("G2"),
        )
        .expect("target");
        let drive = PanTiltDrive::new(
            crate::command::PanTiltDirection::Up,
            PanSpeed::new(25).expect("syntax"),
            TiltSpeed::new(25).expect("syntax"),
        )
        .expect("drive");
        assert!(prepare_operation::<completion::AppliedOnly, _>(
            &drive,
            &normal,
            OperationalTuning::new(),
            ClassSelection::Request
        )
        .is_err());
    }

    fn reset_write_count() {
        WRITE_COUNT.with(|count| count.set(0));
    }

    fn write_count() -> usize {
        WRITE_COUNT.with(Cell::get)
    }

    fn increment_write_count() {
        WRITE_COUNT.with(|count| count.set(count.get().saturating_add(1)));
    }

    fn runtime_profile(
        mut capabilities: Capabilities,
        operation_complete: bool,
        position_inquiries: PositionInquirySupport,
        preset_axes: AffectedAxes,
    ) -> ProfileSpec {
        if let Some(presets) = &mut capabilities.presets {
            presets.recall_axes = preset_axes;
        }
        let transports = TransportCompatibility::new(Some(5678), Some(1259), false);
        ProfileSpec::builder(capabilities)
            .transports(transports)
            .envelope(ProfileEnvelope::RawVisca)
            .timing(
                ProfileTiming::builder()
                    .ack_timeout(Duration::from_millis(100))
                    .command_timeouts(crate::CommandTimeouts::default())
                    .inquiry_timeout(Duration::from_secs(1))
                    .cancellation_timeout(Duration::from_secs(1))
                    .ambiguity_timeout(Duration::from_secs(1))
                    .busy_timeout(Duration::ZERO)
                    .raw_inquiry_reply_skew(Duration::ZERO)
                    .minimum_inquiry_spacing(Duration::ZERO)
                    .minimum_command_spacing(Duration::ZERO)
                    .build()
                    .expect("valid timing"),
            )
            .maximum_command_sockets(1)
            .supports_operation_complete(operation_complete)
            .supports_command_cancel(false)
            .position_inquiries(position_inquiries)
            .build()
            .expect("valid test profile")
    }

    struct CountingInquiry;

    impl Request for CountingInquiry {
        type Class = crate::request::Inquiry;

        const MAX_SIZE: usize = 2;
        const TIMEOUT_CLASS: TimeoutClass = TimeoutClass::Inquiry;
        const RETRY_CLASS: RetryClass = RetryClass::Inquiry;
        const CONTROL_CLASS: ControlClass = ControlClass::Normal;

        fn write_into(&self, target: CameraId, buffer: &mut [u8]) -> Result<usize> {
            increment_write_count();
            buffer[..2].copy_from_slice(&[target.to_address_byte(), VISCA_TERMINATOR]);
            Ok(2)
        }
    }

    impl Inquiry for CountingInquiry {
        type Response = Vec<u8>;

        fn decoder(&self) -> ResponseDecoder<Self::Response> {
            ResponseDecoder::from_fn(|payload| Ok(payload.to_vec()))
        }
    }

    #[derive(Debug, Copy, Clone, crate::ViscaInquiry)]
    #[visca(opcode = 0x47, response = Raw)]
    struct DownstreamDerivedInquiry;

    struct CountingTargeted;

    impl Request for CountingTargeted {
        type Class = crate::request::Operation<completion::Targeted>;

        const MAX_SIZE: usize = 2;
        const TIMEOUT_CLASS: TimeoutClass = TimeoutClass::Movement;
        const RETRY_CLASS: RetryClass = RetryClass::Movement;
        const CONTROL_CLASS: ControlClass = ControlClass::User;

        fn write_into(&self, target: CameraId, buffer: &mut [u8]) -> Result<usize> {
            increment_write_count();
            buffer[..2].copy_from_slice(&[target.to_address_byte(), VISCA_TERMINATOR]);
            Ok(2)
        }
    }

    impl OperationCommand<completion::Targeted> for CountingTargeted {
        fn affected_axes(&self) -> AffectedAxes {
            AffectedAxes::ZOOM
        }
    }

    macro_rules! plain_timeout_request {
        ($name:ident, $timeout:expr) => {
            struct $name;

            impl Request for $name {
                type Class = crate::request::Plain;

                const MAX_SIZE: usize = 2;
                const TIMEOUT_CLASS: TimeoutClass = $timeout;
                const RETRY_CLASS: RetryClass = RetryClass::Never;
                const CONTROL_CLASS: ControlClass = ControlClass::Normal;

                fn write_into(&self, target: CameraId, buffer: &mut [u8]) -> Result<usize> {
                    buffer[..2].copy_from_slice(&[target.to_address_byte(), VISCA_TERMINATOR]);
                    Ok(2)
                }
            }
        };
    }

    plain_timeout_request!(QuickTimeoutCommand, TimeoutClass::Quick);
    plain_timeout_request!(MovementTimeoutCommand, TimeoutClass::Movement);
    plain_timeout_request!(PresetTimeoutCommand, TimeoutClass::Preset);
    plain_timeout_request!(LongRunningTimeoutCommand, TimeoutClass::LongRunning);
    plain_timeout_request!(NetworkTimeoutCommand, TimeoutClass::Network);

    /// A retrying command used to pin default observer lifetimes independently
    /// of the generated command inventory.
    struct RetryingQuickCommand;

    impl Request for RetryingQuickCommand {
        type Class = crate::request::Plain;

        const MAX_SIZE: usize = 2;
        const TIMEOUT_CLASS: TimeoutClass = TimeoutClass::Quick;
        const RETRY_CLASS: RetryClass = RetryClass::Standard;
        const CONTROL_CLASS: ControlClass = ControlClass::Normal;

        fn write_into(&self, target: CameraId, buffer: &mut [u8]) -> Result<usize> {
            buffer[..2].copy_from_slice(&[target.to_address_byte(), VISCA_TERMINATOR]);
            Ok(2)
        }
    }

    /// A retrying movement command used to pin the 30 s/60 s default window.
    struct RetryingMovementCommand;

    impl Request for RetryingMovementCommand {
        type Class = crate::request::Plain;

        const MAX_SIZE: usize = 2;
        const TIMEOUT_CLASS: TimeoutClass = TimeoutClass::Movement;
        const RETRY_CLASS: RetryClass = RetryClass::Movement;
        const CONTROL_CLASS: ControlClass = ControlClass::Normal;

        fn write_into(&self, target: CameraId, buffer: &mut [u8]) -> Result<usize> {
            buffer[..2].copy_from_slice(&[target.to_address_byte(), VISCA_TERMINATOR]);
            Ok(2)
        }
    }

    struct ClassifiedInquiry;

    impl Request for ClassifiedInquiry {
        type Class = crate::request::Inquiry;

        const MAX_SIZE: usize = 2;
        const TIMEOUT_CLASS: TimeoutClass = TimeoutClass::Quick;
        const RETRY_CLASS: RetryClass = RetryClass::Inquiry;
        const CONTROL_CLASS: ControlClass = ControlClass::Normal;

        fn write_into(&self, target: CameraId, buffer: &mut [u8]) -> Result<usize> {
            buffer[..2].copy_from_slice(&[target.to_address_byte(), VISCA_TERMINATOR]);
            Ok(2)
        }
    }

    impl Inquiry for ClassifiedInquiry {
        type Response = Vec<u8>;

        fn decoder(&self) -> ResponseDecoder<Self::Response> {
            ResponseDecoder::from_fn(|payload| Ok(payload.to_vec()))
        }
    }

    /// A downstream inquiry can override `Request::reply_shape` without
    /// passing through `raw::Inquiry`; shared preparation must reject both
    /// non-default command lifecycles rather than lower a declaration that an
    /// inquiry cannot honor.
    struct NonDefaultReplyShapeInquiry(crate::raw::RawReplyShape);

    impl Request for NonDefaultReplyShapeInquiry {
        type Class = crate::request::Inquiry;

        const MAX_SIZE: usize = 2;
        const TIMEOUT_CLASS: TimeoutClass = TimeoutClass::Inquiry;
        const RETRY_CLASS: RetryClass = RetryClass::Inquiry;
        const CONTROL_CLASS: ControlClass = ControlClass::Normal;

        fn reply_shape(&self) -> crate::raw::RawReplyShape {
            self.0
        }

        fn write_into(&self, target: CameraId, buffer: &mut [u8]) -> Result<usize> {
            buffer[..2].copy_from_slice(&[target.to_address_byte(), VISCA_TERMINATOR]);
            Ok(2)
        }
    }

    impl Inquiry for NonDefaultReplyShapeInquiry {
        type Response = Vec<u8>;

        fn decoder(&self) -> ResponseDecoder<Self::Response> {
            ResponseDecoder::from_fn(|payload| Ok(payload.to_vec()))
        }
    }

    struct CountingAppliedOnly;

    impl Request for CountingAppliedOnly {
        type Class = crate::request::Operation<completion::AppliedOnly>;

        const MAX_SIZE: usize = 2;
        const TIMEOUT_CLASS: TimeoutClass = TimeoutClass::Movement;
        const RETRY_CLASS: RetryClass = RetryClass::Movement;
        const CONTROL_CLASS: ControlClass = ControlClass::User;

        fn write_into(&self, target: CameraId, buffer: &mut [u8]) -> Result<usize> {
            increment_write_count();
            buffer[..2].copy_from_slice(&[target.to_address_byte(), VISCA_TERMINATOR]);
            Ok(2)
        }
    }

    impl OperationCommand<completion::AppliedOnly> for CountingAppliedOnly {
        fn affected_axes(&self) -> AffectedAxes {
            AffectedAxes::ND_FILTER
        }
    }

    /// A downstream operation can override `Request::reply_shape` without
    /// passing through `raw::AppliedOnly`; preparation must still refuse the
    /// no-reply lifecycle because it would otherwise manufacture an
    /// `applied()` handle without terminal camera evidence.
    struct NoReplyAppliedOnly;

    impl Request for NoReplyAppliedOnly {
        type Class = crate::request::Operation<completion::AppliedOnly>;

        const MAX_SIZE: usize = 2;
        const TIMEOUT_CLASS: TimeoutClass = TimeoutClass::Quick;
        const RETRY_CLASS: RetryClass = RetryClass::Never;
        const CONTROL_CLASS: ControlClass = ControlClass::Normal;

        fn reply_shape(&self) -> crate::raw::RawReplyShape {
            crate::raw::RawReplyShape::NoReply
        }

        fn write_into(&self, target: CameraId, buffer: &mut [u8]) -> Result<usize> {
            buffer[..2].copy_from_slice(&[target.to_address_byte(), VISCA_TERMINATOR]);
            Ok(2)
        }
    }

    impl OperationCommand<completion::AppliedOnly> for NoReplyAppliedOnly {
        fn affected_axes(&self) -> AffectedAxes {
            AffectedAxes::ZOOM
        }
    }

    struct WrongAddress;

    impl Request for WrongAddress {
        type Class = crate::request::Plain;

        const MAX_SIZE: usize = 2;
        const TIMEOUT_CLASS: TimeoutClass = TimeoutClass::Quick;
        const RETRY_CLASS: RetryClass = RetryClass::Never;
        const CONTROL_CLASS: ControlClass = ControlClass::Normal;

        fn write_into(&self, _target: CameraId, buffer: &mut [u8]) -> Result<usize> {
            buffer[..2].copy_from_slice(&[CameraId::CAMERA_1.to_address_byte(), VISCA_TERMINATOR]);
            Ok(2)
        }
    }

    /// A downstream-style plain request whose frame is supplied by the caller.
    /// The owner must validate the resulting bytes even though this type is not
    /// one of the crate's raw wrappers.
    struct DownstreamPlain {
        frame: &'static [u8],
    }

    impl Request for DownstreamPlain {
        type Class = crate::request::Plain;

        const MAX_SIZE: usize = 7;
        const TIMEOUT_CLASS: TimeoutClass = TimeoutClass::Quick;
        const RETRY_CLASS: RetryClass = RetryClass::Never;
        const CONTROL_CLASS: ControlClass = ControlClass::Normal;

        fn write_into(&self, target: CameraId, buffer: &mut [u8]) -> Result<usize> {
            buffer[..self.frame.len()].copy_from_slice(self.frame);
            buffer[0] = target.to_address_byte();
            Ok(self.frame.len())
        }
    }

    struct BoundedPlain<const MAX_SIZE: usize> {
        frame: &'static [u8],
        reported_length: Option<usize>,
    }

    impl<const MAX_SIZE: usize> Request for BoundedPlain<MAX_SIZE> {
        type Class = crate::request::Plain;

        const MAX_SIZE: usize = MAX_SIZE;
        const TIMEOUT_CLASS: TimeoutClass = TimeoutClass::Quick;
        const RETRY_CLASS: RetryClass = RetryClass::Never;
        const CONTROL_CLASS: ControlClass = ControlClass::Normal;

        fn write_into(&self, _: CameraId, buffer: &mut [u8]) -> Result<usize> {
            increment_write_count();
            let available = buffer.len();
            let destination = buffer
                .get_mut(..self.frame.len())
                .ok_or_else(|| Error::buffer_too_small(self.frame.len(), available))?;
            destination.copy_from_slice(self.frame);
            Ok(self.reported_length.unwrap_or(self.frame.len()))
        }
    }

    #[test]
    fn request_max_size_is_a_bound_and_written_length_is_authoritative() {
        const FRAME: &[u8] = &[0x81, 0x01, 0x04, 0x00, 0x02, 0xff];
        let profile = ProfileSpec::from_compile_time::<crate::profiles::GenericVisca>()
            .expect("generic profile");
        let check = |prepared: PreparedCommand| {
            assert_eq!(prepared.wire.as_bytes(), FRAME);
            assert_eq!(write_count(), 1, "preparation encodes exactly once");
        };
        reset_write_count();
        check(
            prepare_command(
                &BoundedPlain::<8> {
                    frame: FRAME,
                    reported_length: None,
                },
                &crate::TargetRegistration::new((TargetSlot::CAMERA_1).camera(), profile.clone())
                    .expect("valid target registration"),
                OperationalTuning::new(),
                ClassSelection::Request,
            )
            .expect("a frame shorter than MAX_SIZE prepares"),
        );
        reset_write_count();
        check(
            prepare_command(
                &BoundedPlain::<6> {
                    frame: FRAME,
                    reported_length: None,
                },
                &crate::TargetRegistration::new((TargetSlot::CAMERA_1).camera(), profile.clone())
                    .expect("valid target registration"),
                OperationalTuning::new(),
                ClassSelection::Request,
            )
            .expect("a frame exactly MAX_SIZE bytes long prepares"),
        );

        for request in [
            BoundedPlain::<8> {
                frame: &[0x81, 0x01, 0x04, 0, 0, 0, 0, 0, 0xff],
                reported_length: None,
            },
            BoundedPlain::<8> {
                frame: FRAME,
                reported_length: Some(9),
            },
        ] {
            let error = prepare_command(
                &request,
                &crate::TargetRegistration::new((TargetSlot::CAMERA_1).camera(), profile.clone())
                    .expect("valid target registration"),
                OperationalTuning::new(),
                ClassSelection::Request,
            )
            .expect_err("an encoder cannot exceed MAX_SIZE");
            assert!(
                matches!(error, Error::InvalidRequest(message) if message.contains("MAX_SIZE"))
            );
        }
        let error = prepare_command(
            &BoundedPlain::<8> {
                frame: &[0x81],
                reported_length: None,
            },
            &crate::TargetRegistration::new((TargetSlot::CAMERA_1).camera(), profile.clone())
                .expect("valid target registration"),
            OperationalTuning::new(),
            ClassSelection::Request,
        )
        .expect_err("a frame needs at least two bytes");
        assert!(matches!(error, Error::InvalidRequest(_)));
    }

    #[test]
    fn envelope_bounds_are_checked_before_admission_and_follow_the_profile_codec() {
        const FRAME: &[u8] = &[0x81, 1, 6, 2, 1, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0xff];
        let sony =
            ProfileSpec::from_compile_time::<crate::profiles::SonyBRCH900>().expect("Sony profile");
        let request = BoundedPlain::<18> {
            frame: FRAME,
            reported_length: None,
        };
        let error = prepare_command(
            &request,
            &crate::TargetRegistration::from_profile(TargetSlot::CAMERA_1, sony.clone()),
            OperationalTuning::new(),
            ClassSelection::Request,
        )
        .expect_err("the ordinary Sony envelope capacity is checked during preparation");
        assert!(
            matches!(error, Error::InvalidRequest(message) if message.contains("envelope payload bound (16)"))
        );

        let mut capabilities = Capabilities::from_profile::<crate::profiles::GenericVisca>();
        capabilities.profile_id = None;
        let raw = runtime_profile(
            capabilities,
            true,
            PositionInquirySupport::new(true, true, true),
            AffectedAxes::PAN_TILT,
        );
        prepare_command(
            &request,
            &crate::TargetRegistration::from_profile(TargetSlot::CAMERA_1, raw.clone()),
            OperationalTuning::new(),
            ClassSelection::Request,
        )
        .expect("raw framing supports larger bounded requests");
        let mut fr7_caps = raw.capabilities().clone();
        fr7_caps.pan_tilt.as_mut().expect("pan/tilt").coordinates =
            crate::PanTiltCoordinateConversion::new(
                crate::capabilities::CoordinateSystem::SignedCentered,
                crate::capabilities::PanTiltWireCodec::SonyFr7,
                16.0,
                16.0,
            )
            .expect("FR7 framing");
        let fr7 = ProfileSpec::builder(fr7_caps)
            .transports(raw.transports())
            .envelope(ProfileEnvelope::SonyEncapsulated)
            .timing(raw.timing())
            .maximum_command_sockets(raw.maximum_command_sockets())
            .supports_operation_complete(raw.supports_operation_complete())
            .supports_command_cancel(raw.supports_command_cancel())
            .position_inquiries(raw.position_inquiries())
            .build()
            .expect("FR7-format runtime profile");
        prepare_command(
            &request,
            &crate::TargetRegistration::from_profile(TargetSlot::CAMERA_1, fr7.clone()),
            OperationalTuning::new(),
            ClassSelection::Request,
        )
        .expect("documented FR7 seventeen-byte frames fit");
        let oversize = BoundedPlain::<18> {
            frame: &[0x81, 1, 6, 2, 1, 1, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0xff],
            reported_length: None,
        };
        assert!(matches!(
            prepare_command(
                &oversize,
                &crate::TargetRegistration::from_profile(TargetSlot::CAMERA_1, fr7.clone()),
                OperationalTuning::new(),
                ClassSelection::Request
            ),
            Err(Error::InvalidRequest(_))
        ));
    }

    /// Owner-only byte patterns must be rejected for every typed request
    /// implementation, not only for values from `crate::raw`.
    #[test]
    fn generic_plain_request_rejects_owner_only_primitives_before_admission() {
        let profile = ProfileSpec::from_compile_time::<crate::profiles::GenericVisca>()
            .expect("generic profile");
        let target = TargetSlot::CAMERA_1;

        for (name, frame) in [
            ("socket cancel", &[0x81, 0x20, VISCA_TERMINATOR][..]),
            (
                "interface clear",
                &[0x81, 0x01, 0x00, 0x01, VISCA_TERMINATOR][..],
            ),
        ] {
            let error = prepare_command(
                &DownstreamPlain { frame },
                &crate::TargetRegistration::new((target).camera(), profile.clone())
                    .expect("valid target registration"),
                OperationalTuning::new(),
                ClassSelection::Request,
            )
            .expect_err("owner-only primitive must not be prepared");
            assert!(matches!(error, Error::InvalidRequest(_)), "{name}");
            // Encoding a custom request into preparation scratch is necessary
            // to inspect its bytes; no PreparedCommand is returned, so no
            // owner admission or transport write can follow this rejection.
        }

        // A nearby custom command with the same address/terminator contract is
        // still accepted; the guard is limited to the exact owner-only shapes.
        let prepared = prepare_command(
            &DownstreamPlain {
                frame: &[0x81, 0x2a, 0x02, 0xa0, 0x04, 0x02, VISCA_TERMINATOR],
            },
            &crate::TargetRegistration::new((target).camera(), profile.clone())
                .expect("valid target registration"),
            OperationalTuning::new(),
            ClassSelection::Request,
        )
        .expect("ordinary downstream command must remain admissible");
        assert_eq!(
            prepared.wire.as_bytes(),
            &[0x81, 0x2a, 0x02, 0xa0, 0x04, 0x02, VISCA_TERMINATOR]
        );
    }

    #[test]
    fn inquiry_support_and_settlement_fail_before_encoding() {
        reset_write_count();
        let mut no_inquiry = Capabilities::from_profile::<crate::profiles::GenericVisca>();
        no_inquiry.profile_id = None;
        no_inquiry.inquiry_support = InquirySupport::None;
        let no_inquiry = runtime_profile(
            no_inquiry,
            true,
            PositionInquirySupport::new(false, false, false),
            AffectedAxes::PAN_TILT,
        );
        assert!(prepare_inquiry(
            &CountingInquiry,
            &crate::TargetRegistration::new((TargetSlot::CAMERA_1).camera(), no_inquiry.clone())
                .expect("valid target registration"),
            OperationalTuning::new(),
            ClassSelection::Request,
        )
        .is_err());
        assert_eq!(write_count(), 0);

        let mut missing_custom_zoom = Capabilities::from_profile::<crate::profiles::GenericVisca>();
        missing_custom_zoom.profile_id = None;
        missing_custom_zoom.zoom.as_mut().expect("zoom").direct = false;
        missing_custom_zoom.typed_support = TypedSupportSet::empty();
        let missing_custom_zoom = runtime_profile(
            missing_custom_zoom,
            false,
            PositionInquirySupport::new(true, false, true),
            AffectedAxes::PAN_TILT,
        );
        assert!(prepare_operation::<completion::Targeted, _>(
            &CountingTargeted,
            &crate::TargetRegistration::new(
                (TargetSlot::CAMERA_1).camera(),
                missing_custom_zoom.clone()
            )
            .expect("valid target registration"),
            OperationalTuning::new(),
            ClassSelection::Request,
        )
        .is_err());
        assert_eq!(write_count(), 0);
    }

    #[test]
    fn unsupported_applied_only_axes_fail_before_encoding() {
        let profile = ProfileSpec::from_compile_time::<crate::profiles::GenericVisca>()
            .expect("generic profile");
        reset_write_count();
        let error = prepare_operation::<completion::AppliedOnly, _>(
            &CountingAppliedOnly,
            &crate::TargetRegistration::new((TargetSlot::CAMERA_1).camera(), profile.clone())
                .expect("valid target registration"),
            OperationalTuning::new(),
            ClassSelection::Request,
        )
        .expect_err("unsupported applied-only axis must not be admitted");
        assert!(matches!(error, Error::FeatureNotSupported { .. }));
        assert_eq!(write_count(), 0, "admission must precede encoding");
    }

    #[test]
    fn gated_builtin_inquiry_rejects_unsupported_runtime_surface() {
        let profile =
            ProfileSpec::from_compile_time::<crate::profiles::PtzOpticsG2>().expect("PTZ profile");
        assert!(!profile
            .capabilities()
            .supports_typed(crate::capabilities::TypedSupportSurface::FocusNearLimitInquiry));

        let error = prepare_inquiry(
            &FocusNearLimitInquiry,
            &crate::TargetRegistration::new((TargetSlot::CAMERA_1).camera(), profile.clone())
                .expect("valid target registration"),
            OperationalTuning::new(),
            ClassSelection::Request,
        )
        .expect_err("unsupported gated inquiry must not be prepared");
        assert!(matches!(error, Error::FeatureNotSupported { .. }));
    }

    #[test]
    fn profile_rejected_generated_inquiries_fail_before_encoding() {
        fn assert_rejected_before_encoding<Q>(inquiry: &Q, profile: &ProfileSpec, name: &str)
        where
            Q: Inquiry,
        {
            crate::command::inquiry_structs::reset_generated_inquiry_write_count();
            match prepare_inquiry(
                inquiry,
                &crate::TargetRegistration::new((TargetSlot::CAMERA_1).camera(), profile.clone())
                    .expect("valid target registration"),
                OperationalTuning::new(),
                ClassSelection::Request,
            ) {
                Err(error) => assert!(
                    matches!(error, Error::FeatureNotSupported { .. }),
                    "{name} must be rejected by its profile gate: {error:?}"
                ),
                Ok(_) => panic!("{name} must not be prepared for this profile"),
            }
            assert_eq!(
                crate::command::inquiry_structs::generated_inquiry_write_count(),
                0,
                "{name} must be rejected before generated inquiry encoding"
            );
        }

        let ptzoptics_g3 = ProfileSpec::from_compile_time::<crate::profiles::PtzOpticsG3>()
            .expect("PTZOptics G3 profile");
        assert_rejected_before_encoding(&FocusZoneInquiry, &ptzoptics_g3, "FocusZoneInquiry");
        assert_rejected_before_encoding(&UsbAudioInquiry, &ptzoptics_g3, "UsbAudioInquiry");

        let sony_fr7 =
            ProfileSpec::from_compile_time::<crate::profiles::SonyFR7>().expect("Sony FR7 profile");
        assert_rejected_before_encoding(&BrightnessInquiry, &sony_fr7, "BrightnessInquiry");
        assert_rejected_before_encoding(&PictureEffectInquiry, &sony_fr7, "PictureEffectInquiry");
        assert_rejected_before_encoding(&IrisInquiry, &sony_fr7, "IrisInquiry");
        assert_rejected_before_encoding(&ExposureModeInquiry, &sony_fr7, "ExposureModeInquiry");
    }

    #[test]
    fn fr7_raw_auto_iris_inquiry_and_stepped_iris_command_remain_available() {
        fn decode_raw(payload: &[u8]) -> crate::Result<Vec<u8>> {
            Ok(payload.to_vec())
        }

        let fr7 =
            ProfileSpec::from_compile_time::<crate::profiles::SonyFR7>().expect("Sony FR7 profile");
        let auto_iris = crate::raw::Inquiry::from_fn(
            [0x81, 0x09, 0x05, 0x34, VISCA_TERMINATOR],
            decode_raw,
            RetryClass::Inquiry,
            ControlClass::Normal,
        )
        .expect("valid raw FR7 auto-iris inquiry");
        let prepared_inquiry = prepare_inquiry(
            &auto_iris,
            &crate::TargetRegistration::new((TargetSlot::CAMERA_1).camera(), fr7.clone())
                .expect("valid target registration"),
            OperationalTuning::new(),
            ClassSelection::Request,
        )
        .expect("raw FR7 auto-iris inquiry remains profile-agnostic");
        assert_eq!(
            prepared_inquiry.wire.as_bytes(),
            &[0x81, 0x09, 0x05, 0x34, VISCA_TERMINATOR]
        );

        let iris_step = crate::raw::Plain::new(
            [
                0x81,
                0x01,
                0x7E,
                0x04,
                0x4B,
                0x02,
                0x00,
                0x01,
                VISCA_TERMINATOR,
            ],
            TimeoutClass::Quick,
            RetryClass::Never,
            ControlClass::Normal,
        )
        .expect("valid raw FR7 iris-step command");
        let prepared_command = prepare_command(
            &iris_step,
            &crate::TargetRegistration::new((TargetSlot::CAMERA_1).camera(), fr7.clone())
                .expect("valid target registration"),
            OperationalTuning::new(),
            ClassSelection::Request,
        )
        .expect("raw FR7 iris-step command remains profile-agnostic");
        assert_eq!(
            prepared_command.wire.as_bytes(),
            &[
                0x81,
                0x01,
                0x7E,
                0x04,
                0x4B,
                0x02,
                0x00,
                0x01,
                VISCA_TERMINATOR,
            ]
        );
    }

    /// Issue #684: a base-domain inquiry (`power().state()`, `zoom().position()`,
    /// ...) is gated on its noun's base-domain capability, so a runtime
    /// `ProfileSpec` that drops the domain refuses the erased inquiry exactly
    /// where the static `<noun>()` accessor could not be named. Built-ins always
    /// declare every base domain, so this only bites caller-built profiles.
    #[test]
    fn base_domain_inquiry_follows_the_static_noun_gate() {
        fn minimal_profile(mutate: impl FnOnce(&mut Capabilities)) -> ProfileSpec {
            let mut capabilities =
                Capabilities::runtime_baseline("Downstream capability probe", 1).expect("baseline");
            // Grant inquiry support so the base-domain gate under test is the
            // only thing that can refuse a query.
            capabilities.inquiry_support = InquirySupport::Full;
            mutate(&mut capabilities);
            ProfileSpec::builder(capabilities)
                .transports(TransportCompatibility::new(Some(5678), None, false))
                .envelope(ProfileEnvelope::RawVisca)
                .timing(
                    ProfileTiming::builder()
                        .ack_timeout(Duration::from_millis(100))
                        .command_timeouts(crate::CommandTimeouts::default())
                        .inquiry_timeout(Duration::from_secs(1))
                        .cancellation_timeout(Duration::from_secs(1))
                        .ambiguity_timeout(Duration::from_secs(1))
                        .busy_timeout(Duration::ZERO)
                        .raw_inquiry_reply_skew(Duration::ZERO)
                        .minimum_inquiry_spacing(Duration::ZERO)
                        .minimum_command_spacing(Duration::ZERO)
                        .build()
                        .expect("valid timing"),
                )
                .maximum_command_sockets(1)
                .supports_operation_complete(false)
                .supports_command_cancel(false)
                .position_inquiries(PositionInquirySupport::new(false, false, false))
                .build()
                .expect("valid minimal profile")
        }

        fn admits<Q>(query: &Q, profile: &ProfileSpec) -> bool
        where
            Q: crate::Inquiry,
        {
            prepare_inquiry(
                query,
                &crate::TargetRegistration::new((TargetSlot::CAMERA_1).camera(), profile.clone())
                    .expect("valid target registration"),
                OperationalTuning::new(),
                ClassSelection::Request,
            )
            .is_ok()
        }

        // A runtime profile that declares no base domain refuses every
        // base-domain inquiry before any I/O, exactly where the static noun
        // accessor could not be named.
        let bare = minimal_profile(|_| {});
        assert!(!admits(&PowerInquiry, &bare));
        assert!(!admits(&ZoomPositionInquiry, &bare));
        // `VersionInquiry` sits on the `System` noun (`gate: [always]` in its
        // `@noun` header in `crate::noun_table`),
        // so no base domain gates it; only its own `HasVersionInquiry` typed
        // surface does, because its decoder accepts only the Sony reply layout.
        assert!(!admits(&VersionInquiry, &bare));
        let version_only = minimal_profile(|capabilities| {
            capabilities.typed_support = crate::capabilities::TypedSupportSet::from_surface(
                crate::capabilities::TypedSupportSurface::VersionInquiry,
            );
        });
        assert!(admits(&VersionInquiry, &version_only));
        assert!(!admits(&PowerInquiry, &version_only));

        // Opting the power domain in flips the power inquiry to admitted while
        // the still-absent zoom domain keeps its inquiry refused: the erased
        // gate tracks each noun's base marker independently.
        let power_only = minimal_profile(|capabilities| {
            capabilities.power = Some(crate::capabilities::PowerCaps {
                on_time: Duration::from_millis(750),
                standby: None,
                wake_on_lan: false,
                retains_settings: false,
                home_on_up: false,
            });
        });
        assert!(admits(&PowerInquiry, &power_only));
        assert!(!admits(&ZoomPositionInquiry, &power_only));
        assert!(!admits(&VersionInquiry, &power_only));
    }

    #[test]
    fn targeted_poll_plan_preserves_exact_prepared_queries_target_and_budget() {
        let mut capabilities = Capabilities::from_profile::<crate::profiles::GenericVisca>();
        capabilities.profile_id = None;
        let profile = runtime_profile(
            capabilities,
            false,
            PositionInquirySupport::new_with_iris_nd(true, true, true, true, false),
            AffectedAxes::PAN_TILT,
        );
        let tuning = OperationalTuning::new()
            .inquiry_spacing(Duration::from_millis(40))
            .movement_timeout(Duration::from_secs(31))
            .settlement_timeout(Duration::from_secs(62));
        let prepared = prepare_operation::<completion::Targeted, _>(
            &CountingTargeted,
            &crate::TargetRegistration::new((TargetSlot::CAMERA_2).camera(), profile.clone())
                .expect("valid target registration"),
            tuning,
            ClassSelection::Request,
        )
        .expect("targeted poll preparation");

        assert_eq!(prepared.context.timeout.completion, Duration::from_secs(31));
        let SettlementPlan::Poll {
            queries,
            tolerance,
            interval,
            default_budget,
        } = prepared.settlement.plan().clone()
        else {
            panic!("profile without completion must choose polling");
        };
        assert_eq!(prepared.context.target.camera(), CameraId::CAMERA_2);
        assert_eq!(prepared.affected_axes, AffectedAxes::ZOOM);
        assert_eq!(tolerance, MovementTolerance::default());
        assert_eq!(interval, Duration::from_millis(40));
        assert_eq!(
            default_budget,
            Duration::from_secs(62),
            "the settlement observer also covers this movement operation's retry budget"
        );
        assert!(queries.pan_tilt.is_none());
        assert!(queries.focus.is_none());
        let zoom = queries.zoom.expect("selected zoom query").instantiate();
        assert_eq!(zoom.context.target.camera(), CameraId::CAMERA_2);
        assert_eq!(zoom.context.timeout.inquiry, Duration::from_secs(1));
        assert_eq!(
            zoom.wire.as_bytes(),
            &[
                CameraId::CAMERA_2.to_address_byte(),
                0x09,
                0x04,
                0x47,
                VISCA_TERMINATOR,
            ]
        );
        assert_eq!(
            zoom.decoder
                .decode(&[0x0, 0x1, 0x2, 0x3])
                .expect("template decoder"),
            ZoomPosition::new(0x0123).expect("zoom position")
        );
    }

    fn position_query<R>() -> PositionQuery<R> {
        let profile =
            ProfileSpec::from_compile_time::<crate::profiles::GenericVisca>().expect("profile");
        let registration =
            crate::TargetRegistration::new(CameraId::CAMERA_1, profile).expect("registration");
        let template = prepare_inquiry(
            &ZoomPositionInquiry,
            &registration,
            OperationalTuning::new(),
            ClassSelection::Request,
        )
        .expect("inquiry")
        .into_template();
        PositionQuery::new(PreparedInquiryTemplate {
            wire: template.wire,
            context: template.context,
            route: template.route,
            decoder: ResponseDecoder::from_fn(|_| Err(Error::InvalidResponseFormat)),
        })
    }

    /// Runs the shared #781 window contract over synthetic owner-clock
    /// instants: the baseline is received at `start`, and the final snapshot
    /// starts exactly when the window allows.
    fn windowed_zoom(query: MotionQuery, baseline: u16, last: u16) -> Result<bool> {
        let start = Instant::now();
        let deadline = start + Duration::from_secs(30);
        let mut position = position_query::<ZoomPosition>();
        assert!(position.record(ZoomPosition::new(baseline)?, query.tolerance));
        let window = MotionWindow::new(query)?.observe_baseline(start, deadline)?;
        let not_before = window.final_not_before();
        assert_eq!(not_before, start + query.window);
        window.observe_final(
            position.record(ZoomPosition::new(last)?, query.tolerance),
            not_before,
        )
    }

    /// Issue #802: the one settlement-polling decision both owners drive.
    #[test]
    fn settlement_poll_settles_only_on_two_agreeing_samples_before_its_deadline() {
        let start = Instant::now();
        let interval = Duration::from_millis(100);
        let deadline = start + Duration::from_millis(250);
        let tolerance = MovementTolerance::default();
        let mut query = position_query::<ZoomPosition>();
        let mut poll = SettlementPoll::new(interval, deadline);
        assert_eq!(
            poll.observe(
                query.record(ZoomPosition::new(10).expect("zoom"), tolerance),
                start
            )
            .expect("baseline"),
            SettlementStep::SampleAfter(interval)
        );
        assert_eq!(
            poll.observe(
                query.record(ZoomPosition::new(500).expect("zoom"), tolerance),
                start + Duration::from_millis(200)
            )
            .expect("moving"),
            SettlementStep::SampleAfter(Duration::from_millis(50))
        );
        assert_eq!(
            poll.observe(
                query.record(ZoomPosition::new(500).expect("zoom"), tolerance),
                deadline
            )
            .expect("stable"),
            SettlementStep::Settled
        );
        let mut moving = SettlementPoll::new(interval, deadline);
        let timeout = moving
            .observe(true, deadline)
            .expect_err("still moving at the deadline");
        assert!(matches!(timeout, Error::Timeout { .. }));
        assert_eq!(
            timeout.failure_context(),
            Some(crate::FailureContext::new(
                crate::FailureStage::Observation,
                crate::Certainty::NotAccepted,
            ))
        );
    }

    #[test]
    fn motion_window_rejects_a_zero_window_before_io() {
        let query = MotionQuery::new(AffectedAxes::ZOOM).with_window(Duration::ZERO);
        assert!(matches!(
            MotionWindow::new(query),
            Err(Error::InvalidParameter {
                parameter: "MotionQuery::window",
                ..
            })
        ));
    }

    #[test]
    fn motion_window_cannot_be_established_by_zero_time_sampling() {
        let query = MotionQuery::new(AffectedAxes::ZOOM).with_window(Duration::from_millis(100));
        let start = Instant::now();
        let window = MotionWindow::new(query)
            .expect("valid window")
            .observe_baseline(start, start + Duration::from_secs(30))
            .expect("baseline");
        let not_before = window.final_not_before();
        // A pair taken back to back cannot report "no movement" for a window
        // it did not span, even when the readings are identical.
        let early = not_before - Duration::from_nanos(1);
        assert!(matches!(
            window.observe_final(false, early),
            Err(Error::InvalidState(_))
        ));
        assert!(!window
            .observe_final(false, not_before)
            .expect("a sample at the window edge is valid"));
    }

    #[test]
    fn motion_window_that_cannot_fit_the_deadline_is_insufficient_evidence() {
        let query = MotionQuery::new(AffectedAxes::ZOOM).with_window(Duration::from_secs(1));
        let start = Instant::now();
        let window = MotionWindow::new(query).expect("valid window");
        // Slow inquiries consumed the budget: the window would end exactly at
        // the deadline, so no final snapshot can start. That is a timeout,
        // never a report of no movement (and with no open window, no final
        // snapshot can be compared at all).
        assert!(matches!(
            window.observe_baseline(start, start + Duration::from_secs(1)),
            Err(Error::Timeout { .. })
        ));
    }

    #[test]
    fn motion_window_budget_covers_the_window_and_every_inquiry() {
        let profile =
            ProfileSpec::from_compile_time::<crate::profiles::PtzOpticsG2>().expect("G2 profile");
        let window = MotionWindow::new(
            MotionQuery::new(AffectedAxes::ZOOM).with_window(Duration::from_millis(250)),
        )
        .expect("valid window");
        for tuning in [
            OperationalTuning::new(),
            OperationalTuning::new()
                .inquiry_timeout(Duration::from_secs(20))
                .inquiry_spacing(
                    profile.timing().minimum_inquiry_spacing() + Duration::from_millis(50),
                ),
        ] {
            let prepared = prepare_inquiry(
                &ZoomPositionInquiry,
                &crate::TargetRegistration::from_profile(TargetSlot::CAMERA_1, profile.clone()),
                tuning,
                ClassSelection::Request,
            )
            .expect("prepared inquiry");
            let context = prepared.context;
            let per_axis = observation_timeout(&context, context.timeout.inquiry)
                + context.control.minimum_spacing;
            fn template<R>(context: RequestContext) -> PositionQuery<R> {
                PositionQuery::new(PreparedInquiryTemplate {
                    wire: Arc::new(EncodedMessage::new(&[0x81, 0x09, 0xff]).expect("inert wire")),
                    context,
                    decoder: ResponseDecoder::from_fn(|_| Err(Error::InvalidResponseFormat)),
                })
            }
            let one_axis = PositionQueryPlan {
                pan_tilt: None,
                zoom: Some(template(context)),
                focus: None,
                iris: None,
                nd_filter: None,
            };
            let all_axes = PositionQueryPlan {
                pan_tilt: Some(template(context)),
                zoom: Some(template(context)),
                focus: Some(template(context)),
                iris: Some(template(context)),
                nd_filter: Some(template(context)),
            };
            assert_eq!(
                window.budget(&one_axis).expect("one axis budget"),
                Duration::from_millis(250) + per_axis * 2
            );
            assert_eq!(
                window.budget(&all_axes).expect("five axis budget"),
                Duration::from_millis(250) + per_axis * 10
            );
            let huge =
                MotionWindow::new(MotionQuery::new(AffectedAxes::ZOOM).with_window(Duration::MAX))
                    .expect("positive window");
            assert!(matches!(
                huge.budget(&one_axis),
                Err(Error::InvalidParameter { .. })
            ));
        }
    }

    #[test]
    fn settlement_overrides_lengthen_each_operations_own_retry_floor() {
        let profile =
            ProfileSpec::from_compile_time::<crate::profiles::PtzOpticsG2>().expect("G2 profile");
        fn check<O: OperationCommand<completion::Targeted>>(
            operation: &O,
            profile: &ProfileSpec,
            seconds: u64,
        ) {
            let prepared = prepare_operation::<completion::Targeted, _>(
                operation,
                &crate::TargetRegistration::from_profile(TargetSlot::CAMERA_1, profile.clone()),
                OperationalTuning::new().settlement_timeout(Duration::from_secs(seconds)),
                ClassSelection::Request,
            )
            .expect("accepted settlement override");
            assert_eq!(
                prepared.settlement.default_budget(),
                Duration::from_secs(seconds).max(prepared.context.retry.total_budget)
            );
        }
        let preset = crate::request::builtin::PresetRecall::for_profile(
            PresetNumber::new(1).expect("preset"),
            &profile,
        )
        .expect("preset recall");
        for seconds in [60, 90, 120, 180] {
            check(&crate::request::builtin::PanTiltHome, &profile, seconds);
            check(&preset, &profile, seconds);
        }
    }

    #[test]
    fn motion_window_compares_endpoints_across_the_whole_window() {
        let tolerance = MovementTolerance {
            zoom: 10,
            ..MovementTolerance::default()
        };
        let query = MotionQuery::new(AffectedAxes::ZOOM)
            .with_tolerance(tolerance)
            .with_window(Duration::from_millis(500));
        // Slow cumulative drift: individually small steps that add up to more
        // than the tolerance over the window are movement.
        assert!(windowed_zoom(query, 100, 111).expect("drift"));
        // Quantized movement that has not yet crossed a step reads as equal.
        assert!(!windowed_zoom(query, 100, 100).expect("quantized"));
        // Stable noise within tolerance, in either direction, is not movement.
        assert!(!windowed_zoom(query, 100, 110).expect("noise up"));
        assert!(!windowed_zoom(query, 100, 90).expect("noise down"));
        assert!(windowed_zoom(query, 100, 89).expect("past tolerance"));
    }

    #[test]
    fn motion_window_ignores_unselected_axes() {
        let profile =
            ProfileSpec::from_compile_time::<crate::profiles::GenericVisca>().expect("profile");
        let registration =
            crate::TargetRegistration::new(CameraId::CAMERA_1, profile).expect("registration");
        let mut plan =
            prepare_position_queries(&registration, OperationalTuning::new(), AffectedAxes::ZOOM)
                .expect("selected zoom plan");
        assert!(
            plan.pan_tilt.is_none()
                && plan.focus.is_none()
                && plan.iris.is_none()
                && plan.nd_filter.is_none()
        );
        let zoom = plan.zoom.as_mut().expect("selected zoom");
        assert!(zoom.record(
            ZoomPosition::new(10).expect("zoom"),
            MovementTolerance::default()
        ));
        assert!(!zoom.record(
            ZoomPosition::new(10).expect("zoom"),
            MovementTolerance::default()
        ));
    }

    #[test]
    fn position_queries_compare_typed_values_without_overflow() {
        let tolerance = MovementTolerance {
            pan_tilt: 2,
            zoom: 10,
            focus: 5,
            iris: 0,
            nd_filter: 0,
        };
        let mut pan_tilt = position_query::<PanTiltPosition>();
        assert!(pan_tilt.record(PanTiltPosition::new(i32::MIN, i32::MAX), tolerance));
        assert!(pan_tilt.record(PanTiltPosition::new(i32::MAX, i32::MIN), tolerance));
        assert!(!pan_tilt.record(PanTiltPosition::new(i32::MAX - 2, i32::MIN + 2), tolerance));
        let mut zoom = position_query::<ZoomPosition>();
        assert!(zoom.record(ZoomPosition::new(100).expect("zoom"), tolerance));
        assert!(!zoom.record(ZoomPosition::new(110).expect("zoom"), tolerance));
        assert!(zoom.record(ZoomPosition::new(99).expect("zoom"), tolerance));
        let mut focus = position_query::<FocusPosition>();
        assert!(focus.record(FocusPosition::new(200), tolerance));
        assert!(!focus.record(FocusPosition::new(205), tolerance));
        assert!(focus.record(FocusPosition::new(199), tolerance));
    }

    #[test]
    fn position_queries_preserve_brc300_width_pan_tilt_tolerances() {
        let tolerance = MovementTolerance {
            pan_tilt: 40_000,
            ..MovementTolerance::default()
        };
        let endpoint = PanTiltPosition::new(0x08A58, 0x493D);
        let mut stable = position_query::<PanTiltPosition>();
        assert!(stable.record(endpoint, tolerance));
        assert!(!stable.record(PanTiltPosition::new(0x08A58 - 40_000, 0x493D), tolerance));
        let mut moving = position_query::<PanTiltPosition>();
        assert!(moving.record(endpoint, tolerance));
        assert!(moving.record(PanTiltPosition::new(0x08A58 - 40_001, 0x493D), tolerance));
    }

    #[test]
    fn fr7_rejects_iris_position_plans_but_keeps_its_nd_wire() {
        let fr7 =
            ProfileSpec::from_compile_time::<crate::profiles::SonyFR7>().expect("Sony FR7 profile");
        let error = prepare_position_queries(
            &crate::TargetRegistration::new((TargetSlot::CAMERA_2).camera(), fr7.clone())
                .expect("valid target registration"),
            OperationalTuning::new(),
            AffectedAxes::IRIS,
        )
        .expect_err("FR7 has no standard iris position inquiry");
        assert!(matches!(error, Error::FeatureNotSupported { .. }));

        let error = prepare_position_queries(
            &crate::TargetRegistration::new((TargetSlot::CAMERA_2).camera(), fr7.clone())
                .expect("valid target registration"),
            OperationalTuning::new(),
            AffectedAxes::IRIS.union(AffectedAxes::ND_FILTER),
        )
        .expect_err("FR7 cannot combine an unsupported iris inquiry with ND");
        assert!(matches!(error, Error::FeatureNotSupported { .. }));

        let plan = prepare_position_queries(
            &crate::TargetRegistration::new((TargetSlot::CAMERA_2).camera(), fr7.clone())
                .expect("valid target registration"),
            OperationalTuning::new(),
            AffectedAxes::ND_FILTER,
        )
        .expect("FR7 supports its ND position inquiry");
        assert!(plan.pan_tilt.is_none());
        assert!(plan.zoom.is_none());
        assert!(plan.focus.is_none());
        assert!(plan.iris.is_none());
        assert_eq!(
            plan.nd_filter
                .expect("ND inquiry")
                .instantiate()
                .wire
                .as_bytes(),
            &[
                CameraId::CAMERA_2.to_address_byte(),
                0x09,
                0x7E,
                0x04,
                0x42,
                VISCA_TERMINATOR,
            ]
        );
    }

    #[test]
    fn targeted_scalar_requests_lower_to_exact_profile_settlement_plans() {
        let fr7 =
            ProfileSpec::from_compile_time::<crate::profiles::SonyFR7>().expect("Sony FR7 profile");

        reset_request_write_count();
        let error = prepare_builtin_operation::<completion::Targeted, _>(
            &IrisReset,
            &crate::TargetRegistration::new((TargetSlot::CAMERA_2).camera(), fr7.clone())
                .expect("valid target registration"),
            OperationalTuning::new(),
        )
        .expect_err("FR7 does not support shared typed iris operations");
        assert!(matches!(error, Error::FeatureNotSupported { .. }));
        assert_eq!(request_write_count(), 0);

        let nd = prepare_builtin_operation::<completion::Targeted, _>(
            &NdFilterStepUp,
            &crate::TargetRegistration::new((TargetSlot::CAMERA_2).camera(), fr7.clone())
                .expect("valid target registration"),
            OperationalTuning::new(),
        )
        .expect("FR7 ND operation");
        let SettlementPlan::Poll { queries, .. } = nd.settlement.plan().clone() else {
            panic!("FR7 must poll ND position without operation-complete support");
        };
        assert_eq!(nd.affected_axes, AffectedAxes::ND_FILTER);
        assert!(queries.pan_tilt.is_none());
        assert!(queries.zoom.is_none());
        assert!(queries.focus.is_none());
        assert!(queries.iris.is_none());
        assert!(queries.nd_filter.is_some());

        let ptz =
            ProfileSpec::from_compile_time::<crate::profiles::PtzOpticsG2>().expect("PTZ profile");
        let completed = prepare_builtin_operation::<completion::Targeted, _>(
            &IrisReset,
            &crate::TargetRegistration::new((TargetSlot::CAMERA_2).camera(), ptz.clone())
                .expect("valid target registration"),
            OperationalTuning::new(),
        )
        .expect("PTZ iris operation");
        assert!(matches!(
            completed.settlement.plan().clone(),
            SettlementPlan::CompletionIsSettled { .. }
        ));
    }

    #[test]
    fn brc_h900_high_iris_direct_decodes_and_settles_through_polling() {
        let h900 =
            ProfileSpec::from_compile_time::<crate::profiles::SonyBRCH900>().expect("H900 profile");
        let level = IrisLevel::new(0x14).expect("BRC-H900 iris level");
        let prepared = prepare_builtin_operation::<completion::Targeted, _>(
            &IrisDirect::new(level),
            &crate::TargetRegistration::new((TargetSlot::CAMERA_2).camera(), h900.clone())
                .expect("valid target registration"),
            OperationalTuning::new(),
        )
        .expect("BRC-H900 high iris operation");

        let SettlementPlan::Poll {
            queries, tolerance, ..
        } = prepared.settlement.plan().clone()
        else {
            panic!("BRC-H900 must poll iris because it has no operation-complete reply");
        };
        assert_eq!(prepared.context.target.camera(), CameraId::CAMERA_2);
        assert_eq!(prepared.affected_axes, AffectedAxes::IRIS);
        assert!(queries.pan_tilt.is_none());
        assert!(queries.zoom.is_none());
        assert!(queries.focus.is_none());
        assert!(queries.nd_filter.is_none());

        let mut sampled_iris = queries.iris.expect("selected iris settlement inquiry");
        let iris = sampled_iris.instantiate();
        assert_eq!(
            iris.wire.as_bytes(),
            &[
                CameraId::CAMERA_2.to_address_byte(),
                0x09,
                0x04,
                0x4B,
                VISCA_TERMINATOR,
            ]
        );
        let decoded = iris
            .decoder
            .decode(&[0x00, 0x00, 0x01, 0x04])
            .expect("BRC-H900 iris readback 0x14");
        assert_eq!(decoded, level);

        assert!(sampled_iris.record(decoded, tolerance));
        assert!(!sampled_iris.record(decoded, tolerance));
    }

    #[test]
    fn unsupported_nd_position_fails_preparation_before_any_write() {
        let generic = ProfileSpec::from_compile_time::<crate::profiles::GenericVisca>()
            .expect("generic profile");
        let error = prepare_position_queries(
            &crate::TargetRegistration::new((TargetSlot::CAMERA_1).camera(), generic.clone())
                .expect("valid target registration"),
            OperationalTuning::new(),
            AffectedAxes::ND_FILTER,
        )
        .expect_err("generic profile has no ND position inquiry");
        assert!(matches!(error, Error::FeatureNotSupported { .. }));
    }

    #[test]
    fn scalar_position_queries_honor_iris_and_nd_tolerance() {
        let tolerance = MovementTolerance::default();
        let mut iris = position_query::<IrisLevel>();
        let mut nd = position_query::<NdFilterValue>();
        let iris_value = IrisLevel::new(4).expect("iris");
        let nd_value = NdFilterValue::new(0).expect("ND");
        assert!(iris.record(iris_value, tolerance));
        assert!(nd.record(nd_value, tolerance));
        assert!(!iris.record(iris_value, tolerance));
        assert!(!nd.record(nd_value, tolerance));
        assert!(iris.record(IrisLevel::new(5).expect("iris"), tolerance));
        assert!(nd.record(NdFilterValue::new(4).expect("ND"), tolerance));
    }

    #[test]
    fn each_selected_query_needs_two_successful_answers_to_prove_stability() {
        let tolerance = MovementTolerance::default();
        let mut pan_tilt = position_query::<PanTiltPosition>();
        let mut zoom = position_query::<ZoomPosition>();
        let pan_tilt_value = PanTiltPosition::new(0, 0);
        let zoom_value = ZoomPosition::new(0).expect("zoom");
        assert!(pan_tilt.record(pan_tilt_value, tolerance));
        assert!(!pan_tilt.record(pan_tilt_value, tolerance));
        assert!(
            zoom.record(zoom_value, tolerance),
            "another axis's baseline is not zoom evidence"
        );
        assert!(!zoom.record(zoom_value, tolerance));
    }

    #[test]
    fn explicit_target_address_is_validated() {
        let profile = ProfileSpec::from_compile_time::<crate::profiles::GenericVisca>()
            .expect("built-in profile");
        assert!(prepare_command(
            &WrongAddress,
            &crate::TargetRegistration::new((TargetSlot::CAMERA_2).camera(), profile.clone())
                .expect("valid target registration"),
            OperationalTuning::new(),
            ClassSelection::Request,
        )
        .is_err());
    }

    #[test]
    fn inquiry_syntax_retry_is_reserved_for_builtin_inquiries() {
        let profile = ProfileSpec::from_compile_time::<crate::profiles::GenericVisca>()
            .expect("built-in profile");
        let custom = prepare_inquiry(
            &DownstreamDerivedInquiry,
            &crate::TargetRegistration::new((TargetSlot::CAMERA_1).camera(), profile.clone())
                .expect("valid target registration"),
            OperationalTuning::new(),
            ClassSelection::Request,
        )
        .expect("custom inquiry preparation");
        assert!(custom.context.retry.inquiry_timeout);
        assert!(!custom.context.retry.builtin_inquiry_syntax);

        let builtin = prepare_inquiry(
            &PowerInquiry,
            &crate::TargetRegistration::new((TargetSlot::CAMERA_1).camera(), profile.clone())
                .expect("valid target registration"),
            OperationalTuning::new(),
            ClassSelection::Request,
        )
        .expect("generic built-in inquiry preparation");
        assert!(builtin.context.retry.inquiry_timeout);
        assert!(builtin.context.retry.builtin_inquiry_syntax);

        let settlement_builtin = prepare_inquiry(
            &PowerInquiry,
            &crate::TargetRegistration::new((TargetSlot::CAMERA_1).camera(), profile.clone())
                .expect("valid target registration"),
            OperationalTuning::new(),
            ClassSelection::Request,
        )
        .expect("settlement built-in inquiry preparation");
        assert!(settlement_builtin.context.retry.builtin_inquiry_syntax);
    }

    #[test]
    fn command_completion_uses_category_overrides_then_profile_values() {
        let profile = ProfileSpec::from_compile_time::<crate::profiles::PtzOpticsG2>()
            .expect("built-in profile");
        let tuning = OperationalTuning::new()
            .quick_timeout(Duration::from_secs(11))
            .movement_timeout(Duration::from_secs(32))
            .preset_timeout(Duration::from_secs(62))
            .long_running_timeout(Duration::from_secs(302))
            .network_timeout(Duration::from_secs(15));

        let cases = [
            (
                prepare_command(
                    &QuickTimeoutCommand,
                    &crate::TargetRegistration::new(
                        (TargetSlot::CAMERA_1).camera(),
                        profile.clone(),
                    )
                    .expect("valid target registration"),
                    tuning,
                    ClassSelection::Request,
                )
                .expect("quick preparation")
                .context
                .timeout
                .completion,
                Duration::from_secs(11),
            ),
            (
                prepare_command(
                    &MovementTimeoutCommand,
                    &crate::TargetRegistration::new(
                        (TargetSlot::CAMERA_1).camera(),
                        profile.clone(),
                    )
                    .expect("valid target registration"),
                    tuning,
                    ClassSelection::Request,
                )
                .expect("movement preparation")
                .context
                .timeout
                .completion,
                Duration::from_secs(32),
            ),
            (
                prepare_command(
                    &PresetTimeoutCommand,
                    &crate::TargetRegistration::new(
                        (TargetSlot::CAMERA_1).camera(),
                        profile.clone(),
                    )
                    .expect("valid target registration"),
                    tuning,
                    ClassSelection::Request,
                )
                .expect("preset preparation")
                .context
                .timeout
                .completion,
                Duration::from_secs(62),
            ),
            (
                prepare_command(
                    &LongRunningTimeoutCommand,
                    &crate::TargetRegistration::new(
                        (TargetSlot::CAMERA_1).camera(),
                        profile.clone(),
                    )
                    .expect("valid target registration"),
                    tuning,
                    ClassSelection::Request,
                )
                .expect("long-running preparation")
                .context
                .timeout
                .completion,
                Duration::from_secs(302),
            ),
            (
                prepare_command(
                    &NetworkTimeoutCommand,
                    &crate::TargetRegistration::new(
                        (TargetSlot::CAMERA_1).camera(),
                        profile.clone(),
                    )
                    .expect("valid target registration"),
                    tuning,
                    ClassSelection::Request,
                )
                .expect("network preparation")
                .context
                .timeout
                .completion,
                Duration::from_secs(15),
            ),
        ];
        for (actual, expected) in cases {
            assert_eq!(actual, expected);
        }

        let profile_default = OperationalTuning::new();
        let quick = prepare_command(
            &QuickTimeoutCommand,
            &crate::TargetRegistration::new((TargetSlot::CAMERA_1).camera(), profile.clone())
                .expect("valid target registration"),
            profile_default,
            ClassSelection::Request,
        )
        .expect("default quick preparation");
        assert_eq!(quick.context.timeout.completion, Duration::from_secs(5));
    }

    #[test]
    fn inquiries_use_the_inquiry_deadline_and_ignore_command_overrides() {
        let profile = ProfileSpec::from_compile_time::<crate::profiles::PtzOpticsG2>()
            .expect("built-in profile");
        let tuning = OperationalTuning::new()
            .quick_timeout(Duration::from_secs(11))
            .inquiry_timeout(Duration::from_secs(17));
        let prepared = prepare_inquiry(
            &CountingInquiry,
            &crate::TargetRegistration::new((TargetSlot::CAMERA_1).camera(), profile.clone())
                .expect("valid target registration"),
            tuning,
            ClassSelection::Request,
        )
        .expect("inquiry preparation");
        assert_eq!(prepared.context.timeout.inquiry, Duration::from_secs(17));
    }

    #[test]
    fn default_observers_cover_the_total_retry_budget_for_every_request_class() {
        fn prepared_with_budget(total_budget: Duration) -> PreparedInquiry<Vec<u8>> {
            let profile = ProfileSpec::from_compile_time::<crate::profiles::GenericVisca>()
                .expect("built-in profile");
            let mut prepared = prepare_inquiry(
                &CountingInquiry,
                &crate::TargetRegistration::new((TargetSlot::CAMERA_1).camera(), profile.clone())
                    .expect("valid target registration"),
                OperationalTuning::new(),
                ClassSelection::Request,
            )
            .expect("inquiry preparation");
            prepared.context.timeout.inquiry = Duration::from_secs(1);
            prepared.context.retry.total_budget = total_budget;
            prepared
        }

        for (budget, expected) in [
            (Duration::from_millis(500), Duration::from_secs(1)),
            (Duration::from_secs(3), Duration::from_secs(3)),
            (Duration::ZERO, Duration::from_secs(1)),
        ] {
            let admitted =
                prepared_with_budget(budget).admit_with(|_request, _decoder, timeout| timeout);
            assert_eq!(admitted, expected, "admission budget {budget:?}");
        }

        let profile = ProfileSpec::from_compile_time::<crate::profiles::GenericVisca>()
            .expect("built-in profile");

        // Every timeout class takes the common command path, including a
        // no-retry policy whose default total budget is still part of the
        // prepared request. This catches a future split between the command
        // and inquiry rules.
        for (name, prepared, budget) in [
            (
                "quick",
                prepare_command(
                    &QuickTimeoutCommand,
                    &crate::TargetRegistration::new(
                        (TargetSlot::CAMERA_1).camera(),
                        profile.clone(),
                    )
                    .expect("valid target registration"),
                    OperationalTuning::new(),
                    ClassSelection::Request,
                )
                .expect("quick preparation"),
                Duration::from_secs(10),
            ),
            (
                "movement",
                prepare_command(
                    &MovementTimeoutCommand,
                    &crate::TargetRegistration::new(
                        (TargetSlot::CAMERA_1).camera(),
                        profile.clone(),
                    )
                    .expect("valid target registration"),
                    OperationalTuning::new(),
                    ClassSelection::Request,
                )
                .expect("movement preparation"),
                Duration::from_secs(60),
            ),
            (
                "preset",
                prepare_command(
                    &PresetTimeoutCommand,
                    &crate::TargetRegistration::new(
                        (TargetSlot::CAMERA_1).camera(),
                        profile.clone(),
                    )
                    .expect("valid target registration"),
                    OperationalTuning::new(),
                    ClassSelection::Request,
                )
                .expect("preset preparation"),
                Duration::from_secs(120),
            ),
            (
                "long-running",
                prepare_command(
                    &LongRunningTimeoutCommand,
                    &crate::TargetRegistration::new(
                        (TargetSlot::CAMERA_1).camera(),
                        profile.clone(),
                    )
                    .expect("valid target registration"),
                    OperationalTuning::new(),
                    ClassSelection::Request,
                )
                .expect("long-running preparation"),
                Duration::from_secs(600),
            ),
            (
                "network",
                prepare_command(
                    &NetworkTimeoutCommand,
                    &crate::TargetRegistration::new(
                        (TargetSlot::CAMERA_1).camera(),
                        profile.clone(),
                    )
                    .expect("valid target registration"),
                    OperationalTuning::new(),
                    ClassSelection::Request,
                )
                .expect("network preparation"),
                Duration::from_secs(10),
            ),
        ] {
            let observer = prepared.admit_with(|_request, timeout| timeout);
            assert!(
                observer >= budget,
                "{name} observer must cover retry budget"
            );
        }

        // Operations use the same command lifecycle but reach a different
        // receipt constructor. Pin that seam separately from plain commands.
        let operation = prepare_operation::<completion::Targeted, _>(
            &CountingTargeted,
            &crate::TargetRegistration::new((TargetSlot::CAMERA_1).camera(), profile.clone())
                .expect("valid target registration"),
            OperationalTuning::new(),
            ClassSelection::Request,
        )
        .expect("targeted operation preparation");
        let operation_budget = operation.context.retry.total_budget;
        assert!(
            operation.settlement.default_budget() >= operation_budget,
            "targeted settlement observer must cover retry budget"
        );
        let request_timeouts = operation.context.timeout;
        let timeouts = operation.admit_with(|_request, _axes, _settlement, timeouts| timeouts);
        assert!(
            timeouts.applied >= operation_budget,
            "operation observer must cover retry budget"
        );
        // A cancellation observer outlives the application observer and the
        // engine's cancellation ambiguity plus observation windows (#777).
        assert!(timeouts.cancellation >= timeouts.applied);
        assert!(
            timeouts.cancellation >= request_timeouts.ambiguity + request_timeouts.cancellation
        );

        let applied_only = prepare_builtin_operation::<completion::AppliedOnly, _>(
            &ZoomStop,
            &crate::TargetRegistration::new((TargetSlot::CAMERA_1).camera(), profile.clone())
                .expect("valid target registration"),
            OperationalTuning::new(),
        )
        .expect("applied-only operation preparation");
        let applied_only_budget = applied_only.context.retry.total_budget;
        let observer =
            applied_only.admit_with(|_request, _axes, _settlement, timeouts| timeouts.applied);
        assert!(
            observer >= applied_only_budget,
            "applied-only observer must cover retry budget"
        );
    }

    /// Issue #749: these are the three retry triggers that can issue a
    /// physical replay. The test stays at preparation time so it is entirely
    /// deterministic while asserting the exact receipt timeout delivered to
    /// both owner implementations.
    #[test]
    fn physical_retries_cannot_outlive_their_default_observer() {
        let profile = ProfileSpec::from_compile_time::<crate::profiles::GenericVisca>()
            .expect("built-in profile");

        // A lost ACK for an absolute movement can consume the full 60 s
        // movement retry window. Previously its command observer returned at
        // 30 s, so a later replay could move the camera after Timeout.
        let movement = prepare_command(
            &RetryingMovementCommand,
            &crate::TargetRegistration::new((TargetSlot::CAMERA_1).camera(), profile.clone())
                .expect("valid target registration"),
            OperationalTuning::new().movement_timeout(Duration::from_secs(30)),
            ClassSelection::Request,
        )
        .expect("movement preparation");
        let movement_budget = movement.context.retry.total_budget;
        let movement_observer = movement.admit_with(|_request, timeout| timeout);
        assert_eq!(movement_budget, Duration::from_secs(60));
        assert!(
            movement_observer >= movement_budget,
            "a lost-ACK movement replay must stay within the observer window"
        );

        // A CommandBufferFull at t=2 s may schedule another write inside the
        // quick request's full retry window; the observer must remain alive
        // for that whole window, not merely the first completion deadline.
        let busy = prepare_command(
            &RetryingQuickCommand,
            &crate::TargetRegistration::new((TargetSlot::CAMERA_1).camera(), profile.clone())
                .expect("valid target registration"),
            OperationalTuning::new().retry_timing(
                Duration::from_millis(50),
                Duration::from_millis(500),
                Duration::from_secs(10),
            ),
            ClassSelection::Request,
        )
        .expect("busy-command preparation");
        let busy_budget = busy.context.retry.total_budget;
        let busy_observer = busy.admit_with(|_request, timeout| timeout);
        // A shorter budget override never shortens the request's own budget:
        // twice Generic VISCA's 10 s quick deadline (#828 M3).
        assert_eq!(busy_budget, Duration::from_secs(20));
        assert!(
            busy_observer > Duration::from_secs(2),
            "the t=2 s CommandBufferFull retry must precede observer Timeout"
        );
        assert!(busy_observer >= busy_budget);

        // The operation receipt observes through the whole 60 s retry budget
        // rather than detaching at the 30 s completion deadline, so any
        // retry the budget still permits (a lost ACK, a busy camera) precedes
        // the observer's Timeout. An acknowledged command is never rewritten
        // (#795).
        let sony =
            ProfileSpec::from_compile_time::<crate::profiles::SonyFR7>().expect("Sony FR7 profile");
        let sony_operation = prepare_operation::<completion::Targeted, _>(
            &CountingTargeted,
            &crate::TargetRegistration::new((TargetSlot::CAMERA_1).camera(), sony.clone())
                .expect("valid target registration"),
            OperationalTuning::new().movement_timeout(Duration::from_secs(30)),
            ClassSelection::Request,
        )
        .expect("Sony operation preparation");
        let sony_budget = sony_operation.context.retry.total_budget;
        let sony_observer =
            sony_operation.admit_with(|_request, _axes, _settlement, timeouts| timeouts.applied);
        assert_eq!(sony_budget, Duration::from_secs(60));
        assert!(
            sony_observer > Duration::from_secs(30),
            "a Sony retry within the budget must precede observer Timeout"
        );
        assert!(sony_observer >= sony_budget);
    }

    #[test]
    fn preparation_rejects_timeout_classes_that_do_not_match_request_kind() {
        let profile = ProfileSpec::from_compile_time::<crate::profiles::PtzOpticsG2>()
            .expect("built-in profile");
        assert!(prepare_inquiry(
            &ClassifiedInquiry,
            &crate::TargetRegistration::new((TargetSlot::CAMERA_1).camera(), profile.clone())
                .expect("valid target registration"),
            OperationalTuning::new(),
            ClassSelection::Request,
        )
        .is_err());
    }

    #[test]
    fn generic_inquiry_preparation_rejects_non_default_reply_shapes() {
        let profile = ProfileSpec::from_compile_time::<crate::profiles::GenericVisca>()
            .expect("built-in profile");
        for reply_shape in [
            crate::raw::RawReplyShape::CompletionOnly,
            crate::raw::RawReplyShape::NoReply,
        ] {
            let error = prepare_inquiry(
                &NonDefaultReplyShapeInquiry(reply_shape),
                &crate::TargetRegistration::new((TargetSlot::CAMERA_1).camera(), profile.clone())
                    .expect("valid target registration"),
                OperationalTuning::new(),
                ClassSelection::Request,
            )
            .expect_err("an inquiry cannot lower a command-only reply shape");
            assert!(matches!(error, Error::InvalidRequest(_)));
        }
    }

    #[test]
    fn never_retry_class_ignores_retry_limit_tuning() {
        let tuning = OperationalTuning::new().retry_limit(2);
        let never = retry_policy(
            RetryClass::Never,
            TimeoutClass::Quick,
            tuning,
            Duration::ZERO,
            Duration::from_secs(5),
            Duration::ZERO,
        );
        let standard = retry_policy(
            RetryClass::Standard,
            TimeoutClass::Movement,
            tuning,
            Duration::ZERO,
            Duration::from_secs(5),
            Duration::ZERO,
        );

        assert_eq!(never.max_retries, 0);
        // `retry_limit` sets the base; a movement budget is the base itself.
        assert_eq!(standard.max_retries, 2);
    }

    /// Issue #566: the retry budgets follow the per-category table derived
    /// from the base count, not one flat number per retry class.
    #[test]
    fn retry_budgets_follow_the_per_category_table() {
        let tuning = OperationalTuning::new();
        let budget = |timeout_class| {
            retry_policy(
                RetryClass::Standard,
                timeout_class,
                tuning,
                Duration::ZERO,
                Duration::from_secs(5),
                Duration::ZERO,
            )
            .max_retries
        };

        // base 3: quick +2, movement/preset = base, network -1, long-running 1.
        assert_eq!(budget(TimeoutClass::Quick), 5);
        assert_eq!(budget(TimeoutClass::Inquiry), 5);
        assert_eq!(budget(TimeoutClass::Movement), 3);
        assert_eq!(budget(TimeoutClass::Preset), 3);
        assert_eq!(budget(TimeoutClass::Network), 2);
        assert_eq!(budget(TimeoutClass::LongRunning), 1);
    }

    // #842: exercise actual preparation, including every profile, category,
    // supported command reply shape and an ACK override grid.
    #[test]
    fn retry_budget_covers_full_ack_attempt_without_changing_default_budgets() {
        fn check<P: crate::CompileTimeProfile>() {
            let profile = ProfileSpec::from_compile_time::<P>().expect("built-in profile");
            for class in [
                TimeoutClass::Quick,
                TimeoutClass::Movement,
                TimeoutClass::Preset,
                TimeoutClass::LongRunning,
                TimeoutClass::Network,
            ] {
                for shape in [
                    crate::raw::RawReplyShape::AckThenCompletion,
                    crate::raw::RawReplyShape::CompletionOnly,
                    crate::raw::RawReplyShape::NoReply,
                ] {
                    let command = crate::raw::Plain::with_policy(
                        [0x81, 0x01, 0x04, 0x00, 0x02, 0xff],
                        crate::raw::Policy::new(class, RetryClass::Standard, ControlClass::Normal)
                            .expect("policy")
                            .with_reply_shape(shape),
                    )
                    .expect("raw command");
                    for ack in [
                        profile.timing().ack_timeout(),
                        Duration::from_secs(8),
                        Duration::from_secs(10),
                        Duration::from_secs(60),
                        Duration::from_secs(3600),
                    ] {
                        let tuning = OperationalTuning::new().ack_timeout(ack);
                        let prepared = prepare_command(
                            &command,
                            &crate::TargetRegistration::new(
                                (TargetSlot::CAMERA_1).camera(),
                                profile.clone(),
                            )
                            .expect("valid target registration"),
                            tuning,
                            ClassSelection::Request,
                        )
                        .expect("valid preparation");
                        let completion = prepared.context.timeout.completion;
                        let attempt = if shape == crate::raw::RawReplyShape::AckThenCompletion {
                            ack + completion
                        } else {
                            completion
                        };
                        let budget = prepared.context.retry.total_budget;
                        let legacy = MINIMUM_RETRY_BUDGET
                            .max(completion * 2)
                            .max(profile.timing().busy_timeout());
                        assert_eq!(budget, legacy.max(attempt), "{class:?} {shape:?} {ack:?}");
                        prepared.admit_with(|_, observer| assert!(observer >= budget));
                        if ack == profile.timing().ack_timeout() {
                            assert_eq!(
                                budget,
                                legacy,
                                "untuned budget changed for {} {class:?}",
                                profile.capabilities().model_name
                            );
                        }
                    }
                }
            }
            let inquiry = crate::raw::Inquiry::from_fn(
                [0x81, 0x09, 0x04, 0x00, 0xff],
                |_payload: &[u8]| Ok(0_u8),
                RetryClass::Inquiry,
                ControlClass::Normal,
            )
            .expect("raw inquiry");
            for ack in [
                profile.timing().ack_timeout(),
                Duration::from_secs(8),
                Duration::from_secs(10),
                Duration::from_secs(60),
                Duration::from_secs(3600),
            ] {
                let prepared = prepare_inquiry(
                    &inquiry,
                    &crate::TargetRegistration::new(
                        (TargetSlot::CAMERA_1).camera(),
                        profile.clone(),
                    )
                    .expect("valid target registration"),
                    OperationalTuning::new().ack_timeout(ack),
                    ClassSelection::Request,
                )
                .expect("inquiries keep their single reply deadline");
                let budget = prepared.context.retry.total_budget;
                let expected = MINIMUM_RETRY_BUDGET
                    .max(prepared.context.timeout.inquiry * 2)
                    .max(profile.timing().busy_timeout());
                assert_eq!(budget, expected, "ACK tuning changed an inquiry budget");
                prepared.admit_with(|_, _, observer| assert!(observer >= budget));
            }
        }
        check::<crate::profiles::PtzOpticsG2>();
        check::<crate::profiles::PtzOpticsG3>();
        check::<crate::profiles::PtzOptics30X>();
        check::<crate::profiles::SonyFR7>();
        check::<crate::profiles::SonyBRCH900>();
        check::<crate::profiles::SonyEVIH100>();
        check::<crate::profiles::SonyBRC300>();
        check::<crate::profiles::NearusBRC300>();
        check::<crate::profiles::GenericVisca>();
    }

    proptest::proptest! {
        #[test]
        fn retry_budget_and_observer_cover_arbitrary_valid_ack_overrides(
            ack_ms in 500_u64..=3_600_000,
            quick_ms in 8_000_u64..=3_600_000,
        ) {
            let profile = ProfileSpec::from_compile_time::<crate::profiles::SonyFR7>()
                .expect("built-in profile");
            let tuning = OperationalTuning::new().ack_timeout(Duration::from_millis(ack_ms))
                .quick_timeout(Duration::from_millis(quick_ms));
            let command = crate::raw::Plain::new([0x81, 0x01, 0x04, 0x00, 0x02, 0xff],
                TimeoutClass::Quick, RetryClass::Standard, ControlClass::Normal).expect("command");
            let prepared = prepare_command(&command,
        &crate::TargetRegistration::new((TargetSlot::CAMERA_1).camera(), profile.clone()).expect("valid target registration"),
                tuning, ClassSelection::Request).expect("valid tuning");
            let attempt = prepared.context.timeout.ack + prepared.context.timeout.completion;
            let budget = prepared.context.retry.total_budget;
            proptest::prop_assert!(budget >= attempt);
            prepared.admit_with(|_, observer| assert!(observer >= budget));
        }
    }

    #[test]
    fn retry_timing_defaults_follow_the_approved_v2_model() {
        let expected = [
            (TimeoutClass::Quick, 5, Duration::from_secs(1)),
            (TimeoutClass::Inquiry, 5, Duration::from_secs(1)),
            (TimeoutClass::Movement, 3, Duration::from_secs(30)),
            (TimeoutClass::Preset, 3, Duration::from_secs(60)),
            (TimeoutClass::Network, 2, Duration::from_secs(5)),
            (TimeoutClass::LongRunning, 1, Duration::from_secs(300)),
        ];
        for (class, retries, deadline) in expected {
            let policy = retry_policy(
                RetryClass::Standard,
                class,
                OperationalTuning::new(),
                Duration::ZERO,
                deadline,
                Duration::ZERO,
            );
            assert_eq!(policy.initial_backoff, Duration::from_millis(50));
            assert_eq!(
                policy.maximum_backoff,
                Duration::from_millis(500),
                "{class:?}"
            );
            let expected_budget = Duration::from_secs(10).max(deadline.saturating_mul(2));
            assert_eq!(policy.total_budget, expected_budget, "{class:?}");
            assert_eq!(policy.max_retries, retries, "{class:?}");
        }

        let tuned = retry_policy(
            RetryClass::Standard,
            TimeoutClass::Quick,
            OperationalTuning::new().retry_timing(
                Duration::from_millis(100),
                Duration::from_millis(750),
                Duration::from_secs(30),
            ),
            Duration::ZERO,
            Duration::from_secs(1),
            Duration::ZERO,
        );
        assert_eq!(tuned.initial_backoff, Duration::from_millis(100));
        assert_eq!(tuned.maximum_backoff, Duration::from_millis(750));
        assert_eq!(tuned.total_budget, Duration::from_secs(30));
    }

    /// #828 M3: retry timing is a profile bound that tuning may only
    /// lengthen. An override shorter than a request's own bound (here even
    /// one that validation would reject) leaves that bound in force, and a
    /// budget override shorter than a long request's own budget does not
    /// shorten it.
    #[test]
    fn retry_timing_overrides_never_shorten_a_request_bound() {
        let shortened = OperationalTuning::new().retry_timing(
            Duration::from_millis(1),
            Duration::from_millis(2),
            Duration::from_secs(3),
        );
        let quick = retry_policy(
            RetryClass::Standard,
            TimeoutClass::Quick,
            shortened,
            Duration::ZERO,
            Duration::from_secs(1),
            Duration::ZERO,
        );
        assert_eq!(quick.initial_backoff, Duration::from_millis(50));
        assert_eq!(quick.maximum_backoff, Duration::from_millis(500));
        assert_eq!(quick.total_budget, MINIMUM_RETRY_BUDGET);

        let raised_budget = OperationalTuning::new().retry_timing(
            Duration::from_millis(50),
            Duration::from_millis(500),
            Duration::from_secs(20),
        );
        let long = retry_policy(
            RetryClass::Standard,
            TimeoutClass::LongRunning,
            raised_budget,
            Duration::ZERO,
            Duration::from_secs(300),
            Duration::ZERO,
        );
        assert_eq!(long.total_budget, Duration::from_secs(600));
        let short = retry_policy(
            RetryClass::Standard,
            TimeoutClass::Quick,
            raised_budget,
            Duration::ZERO,
            Duration::from_secs(1),
            Duration::ZERO,
        );
        assert_eq!(short.total_budget, Duration::from_secs(20));
    }

    /// A network budget never reaches zero: the clamp keeps one attempt.
    #[test]
    fn a_network_budget_keeps_one_attempt_at_the_smallest_base() {
        for base in [0, 1, 2] {
            let policy = retry_policy(
                RetryClass::Standard,
                TimeoutClass::Network,
                OperationalTuning::new().retry_limit(base),
                Duration::ZERO,
                Duration::from_secs(5),
                Duration::ZERO,
            );
            assert_eq!(policy.max_retries, 1, "base {base}");
        }
    }

    /// Issue #566: a lost ACK and a post-ACK completion timeout are retryable
    /// for every retry class except `Never`. The rewrite had gated ACK retries
    /// to `Standard` and disabled completion retries for everything.
    #[test]
    fn every_replayable_class_retries_lost_acks_and_completion_timeouts() {
        let tuning = OperationalTuning::new();
        for class in [
            RetryClass::Standard,
            RetryClass::Inquiry,
            RetryClass::Movement,
            RetryClass::Preset,
        ] {
            let policy = retry_policy(
                class,
                TimeoutClass::Movement,
                tuning,
                Duration::ZERO,
                Duration::from_secs(5),
                Duration::ZERO,
            );
            assert!(policy.ack_timeout, "{class:?} must retry a lost ACK");
            assert!(
                policy.completion_timeout,
                "{class:?} must retry a post-ACK completion timeout"
            );
            assert!(policy.buffer_full, "{class:?} must retry a busy camera");
            assert!(policy.max_retries > 0, "{class:?} must have a budget");
        }

        let never = retry_policy(
            RetryClass::Never,
            TimeoutClass::Movement,
            tuning,
            Duration::ZERO,
            Duration::from_secs(5),
            Duration::ZERO,
        );
        assert!(!never.ack_timeout);
        assert!(!never.completion_timeout);
        assert!(!never.buffer_full);
        assert_eq!(never.max_retries, 0);
    }

    /// Issue #566: `0x41` (`CommandNotExecutable`) stays a movement/preset
    /// distinction. The classes differ in the terminal error a camera-refused
    /// command produces, so this must not be widened along with the timeout
    /// retries above.
    #[test]
    fn movement_not_executable_retry_is_reserved_for_movement_and_preset() {
        let tuning = OperationalTuning::new();
        let policy = |class| {
            retry_policy(
                class,
                TimeoutClass::Movement,
                tuning,
                Duration::ZERO,
                Duration::from_secs(5),
                Duration::ZERO,
            )
            .movement_not_executable
        };

        assert!(policy(RetryClass::Movement));
        assert!(policy(RetryClass::Preset));
        assert!(!policy(RetryClass::Standard));
        assert!(!policy(RetryClass::Inquiry));
        assert!(!policy(RetryClass::Never));
    }

    /// A typed STOP the camera answers with `0x41` was refused for a standing
    /// reason (G2 focus STOP in auto-focus), so it is reported at once rather
    /// than rewritten; capacity rejections still retry. Ordinary movement
    /// keeps the #566 `0x41` retry.
    #[test]
    fn typed_stops_do_not_retry_command_not_executable() {
        use crate::request::builtin::{FocusStop, ZoomDrive, ZoomStop};
        let profile = ProfileSpec::from_compile_time::<crate::profiles::PtzOpticsG2>()
            .expect("built-in profile");
        let stop_policies = [
            prepare_builtin_operation::<completion::AppliedOnly, _>(
                &FocusStop,
                &crate::TargetRegistration::new((TargetSlot::CAMERA_1).camera(), profile.clone())
                    .expect("valid target registration"),
                OperationalTuning::new(),
            )
            .expect("focus stop"),
            prepare_builtin_operation::<completion::AppliedOnly, _>(
                &ZoomStop,
                &crate::TargetRegistration::new((TargetSlot::CAMERA_1).camera(), profile.clone())
                    .expect("valid target registration"),
                OperationalTuning::new(),
            )
            .expect("zoom stop"),
        ];
        for prepared in &stop_policies {
            assert!(prepared.context.motion.is_some_and(|motion| motion.stop));
            assert!(!prepared.context.retry.movement_not_executable);
            assert!(prepared.context.retry.buffer_full);
        }
        let drive = prepare_builtin_operation::<completion::AppliedOnly, _>(
            &ZoomDrive::Tele,
            &crate::TargetRegistration::new((TargetSlot::CAMERA_1).camera(), profile.clone())
                .expect("valid target registration"),
            OperationalTuning::new(),
        )
        .expect("zoom drive");
        assert!(drive.context.motion.is_some_and(|motion| !motion.stop));
        assert!(drive.context.retry.movement_not_executable);
    }

    /// Issue #566: the default retry budget is at least the ten-second
    /// floor and grows with the selected command deadline.
    #[test]
    fn retry_budget_follows_the_governing_response_deadline() {
        let profile = ProfileSpec::from_compile_time::<crate::profiles::GenericVisca>()
            .expect("built-in profile");
        let prepared = prepare_builtin_operation::<completion::AppliedOnly, _>(
            &crate::request::builtin::ZoomStop,
            &crate::TargetRegistration::new((TargetSlot::CAMERA_1).camera(), profile.clone())
                .expect("valid target registration"),
            OperationalTuning::new(),
        )
        .expect("preparation");

        let completion_deadline = prepared.context.timeout.completion;
        let expected_budget = Duration::from_secs(10).max(completion_deadline.saturating_mul(2));
        assert_eq!(prepared.context.retry.total_budget, expected_budget);

        // A short deadline still gets the ten-second floor, while a longer
        // deadline and a profile busy timeout can raise the total budget.
        let quick_deadline = Duration::from_secs(1);
        let quick = retry_policy(
            RetryClass::Standard,
            TimeoutClass::Quick,
            OperationalTuning::new(),
            Duration::ZERO,
            quick_deadline,
            Duration::ZERO,
        );
        assert_eq!(
            quick.total_budget,
            MINIMUM_RETRY_BUDGET.max(quick_deadline.saturating_mul(2))
        );
        let busy_timeout = Duration::from_secs(17);
        let long_deadline = Duration::from_secs(6);
        let long = retry_policy(
            RetryClass::Standard,
            TimeoutClass::Quick,
            OperationalTuning::new(),
            busy_timeout,
            long_deadline,
            Duration::ZERO,
        );
        assert_eq!(
            long.total_budget,
            MINIMUM_RETRY_BUDGET
                .max(long_deadline.saturating_mul(2))
                .max(busy_timeout)
        );
    }

    #[test]
    fn final_pan_tilt_paths_use_owned_profile_coordinates() {
        fn encoded<C: crate::command::encode::WireEncode>(command: &C) -> Vec<u8> {
            let mut bytes = [0; 32];
            let written = command
                .write_into(CameraId::CAMERA_1, &mut bytes)
                .expect("expected command bytes");
            bytes[..written].to_vec()
        }

        let sony =
            ProfileSpec::from_compile_time::<crate::profiles::SonyBRC300>().expect("Sony profile");
        let nearus = ProfileSpec::from_compile_time::<crate::profiles::NearusBRC300>()
            .expect("Nearus profile");
        let signed = ProfileSpec::from_compile_time::<crate::profiles::GenericVisca>()
            .expect("signed profile");
        let pan_speed = PanSpeed::new(9).expect("speed");
        let tilt_speed = TiltSpeed::new(7).expect("speed");
        let sony_tilt_speed = TiltSpeed::new(9).expect("matching Sony BRC-300 speed");

        let sony_absolute = PanTiltAbsolute::for_target(
            Degrees(45.0),
            Degrees(15.0),
            pan_speed,
            sony_tilt_speed,
            &crate::TargetRegistration::new(crate::CameraId::CAMERA_1, sony.clone())
                .expect("target registration"),
        )
        .expect("Sony BRC-300 absolute");
        let sony_relative = PanTiltRelative::for_target(
            Degrees(45.0),
            Degrees(15.0),
            pan_speed,
            sony_tilt_speed,
            &crate::TargetRegistration::new(crate::CameraId::CAMERA_1, sony.clone())
                .expect("target registration"),
        )
        .expect("Sony BRC-300 relative");
        let sony_limit = PanTiltLimitSet::for_target(
            PanTiltLimitCorner::UpRight,
            Degrees(45.0),
            Degrees(15.0),
            &crate::TargetRegistration::new(crate::CameraId::CAMERA_1, sony.clone())
                .expect("target registration"),
        )
        .expect("Sony BRC-300 limit");
        let prepared_sony_absolute = prepare_builtin_operation::<completion::Targeted, _>(
            &sony_absolute,
            &crate::TargetRegistration::new((TargetSlot::CAMERA_1).camera(), sony.clone())
                .expect("valid target registration"),
            OperationalTuning::new(),
        )
        .expect("prepared Sony BRC-300 absolute");
        let prepared_sony_relative = prepare_builtin_operation::<completion::Targeted, _>(
            &sony_relative,
            &crate::TargetRegistration::new((TargetSlot::CAMERA_1).camera(), sony.clone())
                .expect("valid target registration"),
            OperationalTuning::new(),
        )
        .expect("prepared Sony BRC-300 relative");
        let prepared_sony_limit = prepare_builtin_command(
            &sony_limit,
            &crate::TargetRegistration::new((TargetSlot::CAMERA_1).camera(), sony.clone())
                .expect("valid target registration"),
            OperationalTuning::new(),
        )
        .expect("prepared Sony BRC-300 limit");
        assert_eq!(
            prepared_sony_absolute.wire.as_bytes(),
            &[
                0x81, 0x01, 0x06, 0x02, 0x09, 0x00, 0x0F, 0x0D, 0x0B, 0x07, 0x00, 0x00, 0x0C, 0x03,
                0x00, 0xFF
            ]
        );
        assert_eq!(
            prepared_sony_relative.wire.as_bytes(),
            &[
                0x81, 0x01, 0x06, 0x03, 0x09, 0x00, 0x0F, 0x0D, 0x0B, 0x07, 0x00, 0x00, 0x0C, 0x03,
                0x00, 0xFF
            ]
        );
        assert_eq!(
            prepared_sony_limit.wire.as_bytes(),
            &[
                0x81, 0x01, 0x06, 0x07, 0x00, 0x01, 0x0F, 0x0D, 0x0B, 0x07, 0x00, 0x00, 0x0C, 0x03,
                0x00, 0xFF
            ]
        );
        let sony_inquiry = prepare_inquiry(
            &PanTiltPositionInquiry,
            &crate::TargetRegistration::new((TargetSlot::CAMERA_1).camera(), sony.clone())
                .expect("valid target registration"),
            OperationalTuning::new(),
            ClassSelection::Request,
        )
        .expect("prepared Sony BRC-300 inquiry");
        assert_eq!(
            sony_inquiry
                .decoder
                .decode(&[0x00, 0x02, 0x04, 0x09, 0x00, 0x0F, 0x03, 0x0D, 0x00])
                .expect("Sony BRC-300 position decode"),
            crate::camera::PanTiltPosition::new(9360, -3120)
        );

        // R21 documents the same one-speed, five-pan-nibble frame and limits
        // for the Nearus BRC-300, so it encodes and decodes exactly as the
        // Sony BRC-300 does.
        {
            let profile = &nearus;
            let prepared_absolute = prepare_builtin_operation::<completion::Targeted, _>(
                &PanTiltAbsolute::for_target(
                    Degrees(45.0),
                    Degrees(15.0),
                    pan_speed,
                    sony_tilt_speed,
                    &crate::TargetRegistration::new(crate::CameraId::CAMERA_1, profile.clone())
                        .expect("target registration"),
                )
                .expect("Nearus absolute"),
                &crate::TargetRegistration::new((TargetSlot::CAMERA_1).camera(), profile.clone())
                    .expect("valid target registration"),
                OperationalTuning::new(),
            )
            .expect("prepared Nearus absolute");
            let prepared_relative = prepare_builtin_operation::<completion::Targeted, _>(
                &PanTiltRelative::for_target(
                    Degrees(45.0),
                    Degrees(15.0),
                    pan_speed,
                    sony_tilt_speed,
                    &crate::TargetRegistration::new(crate::CameraId::CAMERA_1, profile.clone())
                        .expect("target registration"),
                )
                .expect("Nearus relative"),
                &crate::TargetRegistration::new((TargetSlot::CAMERA_1).camera(), profile.clone())
                    .expect("valid target registration"),
                OperationalTuning::new(),
            )
            .expect("prepared Nearus relative");
            let prepared_limit = prepare_builtin_command(
                &PanTiltLimitSet::for_target(
                    PanTiltLimitCorner::UpRight,
                    Degrees(45.0),
                    Degrees(15.0),
                    &crate::TargetRegistration::new(crate::CameraId::CAMERA_1, profile.clone())
                        .expect("target registration"),
                )
                .expect("Nearus limit"),
                &crate::TargetRegistration::new((TargetSlot::CAMERA_1).camera(), profile.clone())
                    .expect("valid target registration"),
                OperationalTuning::new(),
            )
            .expect("prepared Nearus limit");
            assert_eq!(
                prepared_absolute.wire.as_bytes(),
                prepared_sony_absolute.wire.as_bytes()
            );
            assert_eq!(
                prepared_relative.wire.as_bytes(),
                prepared_sony_relative.wire.as_bytes()
            );
            assert_eq!(
                prepared_limit.wire.as_bytes(),
                prepared_sony_limit.wire.as_bytes()
            );
            assert!(matches!(
                PanTiltAbsolute::for_target(
                    Degrees(45.0),
                    Degrees(15.0),
                    pan_speed,
                    tilt_speed,
                    &crate::TargetRegistration::new(crate::CameraId::CAMERA_1, profile.clone())
                        .expect("target registration"),
                ),
                Err(Error::InvalidRequest(_))
            ));
            let inquiry = prepare_inquiry(
                &PanTiltPositionInquiry,
                &crate::TargetRegistration::new((TargetSlot::CAMERA_1).camera(), profile.clone())
                    .expect("valid target registration"),
                OperationalTuning::new(),
                ClassSelection::Request,
            )
            .expect("prepared Nearus inquiry");
            assert_eq!(
                inquiry
                    .decoder
                    .decode(&[0x00, 0x02, 0x04, 0x09, 0x00, 0x0F, 0x03, 0x0D, 0x00])
                    .expect("Nearus position decode"),
                crate::camera::PanTiltPosition::new(9360, -3120)
            );
        }

        let clear = PanTiltLimitClear::for_profile(PanTiltLimitCorner::DownLeft, &sony)
            .expect("Sony BRC-300 limit clear");
        let prepared_clear = prepare_command(
            &clear,
            &crate::TargetRegistration::new((TargetSlot::CAMERA_1).camera(), sony.clone())
                .expect("valid target registration"),
            OperationalTuning::new(),
            ClassSelection::Request,
        )
        .expect("prepared limit clear");
        assert_eq!(
            prepared_clear.wire.as_bytes(),
            &[
                0x81, 0x01, 0x06, 0x07, 0x01, 0x00, 0x07, 0x0F, 0x0F, 0x0F, 0x0F, 0x07, 0x0F, 0x0F,
                0x0F, 0xFF
            ]
        );
        assert!(matches!(
            prepared_clear.applied_state,
            Some(AppliedStateProjection::Set(crate::KnownState::PanTiltLimits(update))) if update.is_cleared() && update.corner() == PanTiltLimitCorner::DownLeft
        ));

        let absolute = PanTiltAbsolute::for_target(
            Degrees(45.0),
            Degrees(-15.0),
            pan_speed,
            tilt_speed,
            &crate::TargetRegistration::new(crate::CameraId::CAMERA_1, signed.clone())
                .expect("target registration"),
        )
        .expect("signed absolute");
        let prepared = prepare_builtin_operation::<completion::Targeted, _>(
            &absolute,
            &crate::TargetRegistration::new((TargetSlot::CAMERA_1).camera(), signed.clone())
                .expect("valid target registration"),
            OperationalTuning::new(),
        )
        .expect("prepared signed absolute");
        assert_eq!(
            prepared.wire.as_bytes(),
            encoded(&PanTilt::AbsolutePosition {
                pan: 0x02d0_u16 as i16,
                tilt: 0xff10_u16 as i16,
                pan_speed,
                tilt_speed,
            })
        );
        let inquiry = prepare_inquiry(
            &PanTiltPositionInquiry,
            &crate::TargetRegistration::new((TargetSlot::CAMERA_1).camera(), signed.clone())
                .expect("valid target registration"),
            OperationalTuning::new(),
            ClassSelection::Request,
        )
        .expect("prepared signed inquiry");
        assert_eq!(
            inquiry
                .decoder
                .decode(&[0x0, 0x2, 0xd, 0x0, 0xf, 0xf, 0x1, 0x0])
                .expect("signed position decode"),
            crate::camera::PanTiltPosition::new(720, -240)
        );
        assert!(matches!(
            inquiry.decoder.decode(&[0x12, 0x34, 0x56, 0x78]),
            Err(Error::InvalidResponseLength {
                expected: 8,
                actual: 4,
                ..
            })
        ));
    }

    #[test]
    fn every_homogeneous_builtin_family_validates_before_write() {
        let nearus = ProfileSpec::from_compile_time::<crate::profiles::NearusBRC300>()
            .expect("built-in profile");
        let generic = ProfileSpec::from_compile_time::<crate::profiles::GenericVisca>()
            .expect("generic built-in profile");
        let pan_tilt = PanTiltAbsolute::for_target(
            Degrees(100.0),
            Degrees(0.0),
            PanSpeed::new(18).expect("conservative range"),
            TiltSpeed::new(17).expect("conservative range"),
            &crate::TargetRegistration::new(crate::CameraId::CAMERA_1, generic.clone())
                .expect("target registration"),
        )
        .expect("valid for the source profile");
        reset_request_write_count();
        assert!(prepare_operation::<completion::Targeted, _>(
            &pan_tilt,
            &crate::TargetRegistration::new((TargetSlot::CAMERA_1).camera(), nearus.clone())
                .expect("valid target registration"),
            OperationalTuning::new(),
            ClassSelection::Request,
        )
        .is_err());
        assert_eq!(request_write_count(), 0);

        let mut no_direct_caps = Capabilities::from_profile::<crate::profiles::GenericVisca>();
        no_direct_caps.profile_id = None;
        no_direct_caps.zoom.as_mut().expect("zoom").direct = false;
        no_direct_caps.typed_support = TypedSupportSet::empty();
        let no_direct = runtime_profile(
            no_direct_caps,
            true,
            PositionInquirySupport::new(true, true, true),
            AffectedAxes::PAN_TILT,
        );
        let zoom = ZoomTarget::new(ZoomPosition::new(0).expect("conservative range"));
        reset_request_write_count();
        assert!(prepare_operation::<completion::Targeted, _>(
            &zoom,
            &crate::TargetRegistration::new((TargetSlot::CAMERA_1).camera(), no_direct.clone())
                .expect("valid target registration"),
            OperationalTuning::new(),
            ClassSelection::Request,
        )
        .is_err());
        assert_eq!(request_write_count(), 0);

        let mut bounded_zoom_caps = Capabilities::from_profile::<crate::profiles::GenericVisca>();
        bounded_zoom_caps.profile_id = None;
        bounded_zoom_caps.zoom.as_mut().expect("zoom").optical_max = 0x4000;
        bounded_zoom_caps.zoom.as_mut().expect("zoom").digital_max = Some(0x5000);
        bounded_zoom_caps.zoom.as_mut().expect("zoom").direct = true;
        bounded_zoom_caps.typed_support = TypedSupportSet::from_surfaces(&[
            crate::capabilities::TypedSupportSurface::DirectZoom,
            crate::capabilities::TypedSupportSurface::DigitalZoomRange,
        ]);
        let bounded_zoom = runtime_profile(
            bounded_zoom_caps,
            true,
            PositionInquirySupport::new(true, true, true),
            AffectedAxes::PAN_TILT,
        );
        let outside_zoom =
            ZoomTarget::new(ZoomPosition::new(0x5001).expect("protocol zoom position"));
        reset_request_write_count();
        assert!(prepare_builtin_operation::<completion::Targeted, _>(
            &outside_zoom,
            &crate::TargetRegistration::new((TargetSlot::CAMERA_1).camera(), bounded_zoom.clone())
                .expect("valid target registration"),
            OperationalTuning::new(),
        )
        .is_err());
        assert_eq!(request_write_count(), 0);

        reset_request_write_count();
        assert!(prepare_operation::<completion::AppliedOnly, _>(
            &FocusTrigger::Snap,
            &crate::TargetRegistration::new((TargetSlot::CAMERA_1).camera(), no_direct.clone())
                .expect("valid target registration"),
            OperationalTuning::new(),
            ClassSelection::Request,
        )
        .is_err());
        assert_eq!(request_write_count(), 0);

        let preset = PresetSet::new(PresetNumber::new(255).expect("protocol preset"));
        reset_request_write_count();
        assert!(prepare_command(
            &preset,
            &crate::TargetRegistration::new((TargetSlot::CAMERA_1).camera(), nearus.clone())
                .expect("valid target registration"),
            OperationalTuning::new(),
            ClassSelection::Request,
        )
        .is_err());
        assert_eq!(request_write_count(), 0);
    }

    /// Issue #630: the submission class a camera handle selects reaches the
    /// engine's `ControlPolicy` through every preparation path.
    mod submission_class {
        use super::*;
        use crate::runtime::engine::ControlClass as EngineControlClass;

        /// A plain command the crate would ordinarily schedule as background
        /// work, so a promotion is visible and a demotion is not a no-op.
        struct BackgroundPlain;

        impl Request for BackgroundPlain {
            type Class = crate::request::Plain;

            const MAX_SIZE: usize = 2;
            const TIMEOUT_CLASS: TimeoutClass = TimeoutClass::Quick;
            const RETRY_CLASS: RetryClass = RetryClass::Standard;
            const CONTROL_CLASS: ControlClass = ControlClass::Background;

            fn write_into(&self, target: CameraId, buffer: &mut [u8]) -> Result<usize> {
                buffer[..2].copy_from_slice(&[target.to_address_byte(), VISCA_TERMINATOR]);
                Ok(2)
            }
        }

        /// A downstream applied-only operation attempting to claim the safety
        /// lane. Public `Request` remains extensible, but owner admission must
        /// refuse this unapproved `Urgent` declaration.
        struct DownstreamUrgentStop;

        impl Request for DownstreamUrgentStop {
            type Class = crate::request::Operation<completion::AppliedOnly>;

            const MAX_SIZE: usize = 2;
            const TIMEOUT_CLASS: TimeoutClass = TimeoutClass::Quick;
            const RETRY_CLASS: RetryClass = RetryClass::Movement;
            const CONTROL_CLASS: ControlClass = ControlClass::Urgent;

            fn write_into(&self, target: CameraId, buffer: &mut [u8]) -> Result<usize> {
                buffer[..2].copy_from_slice(&[target.to_address_byte(), VISCA_TERMINATOR]);
                Ok(2)
            }
        }

        impl OperationCommand<completion::AppliedOnly> for DownstreamUrgentStop {
            fn affected_axes(&self) -> AffectedAxes {
                AffectedAxes::ZOOM
            }
        }

        fn profile() -> ProfileSpec {
            ProfileSpec::from_compile_time::<crate::profiles::GenericVisca>()
                .expect("built-in profile")
        }

        fn submission_class(class: ClassSelection) -> EngineControlClass {
            prepare_command(
                &BackgroundPlain,
                &crate::TargetRegistration::new((TargetSlot::CAMERA_1).camera(), profile().clone())
                    .expect("valid target registration"),
                OperationalTuning::new(),
                class,
            )
            .expect("plain command prepares")
            .context
            .control
            .class
        }

        fn inquiry_class(class: ClassSelection) -> EngineControlClass {
            prepare_inquiry(
                &CountingInquiry,
                &crate::TargetRegistration::new((TargetSlot::CAMERA_1).camera(), profile().clone())
                    .expect("valid target registration"),
                OperationalTuning::new(),
                class,
            )
            .expect("inquiry prepares")
            .context
            .control
            .class
        }

        fn downstream_urgent_result(
            class: ClassSelection,
        ) -> Result<PreparedOperation<completion::AppliedOnly>> {
            prepare_operation::<completion::AppliedOnly, _>(
                &DownstreamUrgentStop,
                &crate::TargetRegistration::new((TargetSlot::CAMERA_1).camera(), profile().clone())
                    .expect("valid target registration"),
                OperationalTuning::new(),
                class,
            )
        }

        fn builtin_stop_class(class: ClassSelection) -> EngineControlClass {
            prepare_operation::<completion::AppliedOnly, _>(
                &ZoomStop,
                &crate::TargetRegistration::new((TargetSlot::CAMERA_1).camera(), profile().clone())
                    .expect("valid target registration"),
                OperationalTuning::new(),
                class,
            )
            .expect("crate-owned stop prepares")
            .context
            .control
            .class
        }

        /// Both public QoS sources preserve an urgent intrinsic class and
        /// replace only ordinary intrinsic classes.
        #[test]
        fn resolution_table_is_exact() {
            for request in [
                ControlClass::Background,
                ControlClass::Normal,
                ControlClass::User,
                ControlClass::Urgent,
            ] {
                assert_eq!(
                    ClassSelection::Request.resolve(request),
                    request,
                    "an unselected submission keeps the request's own class",
                );
                let expected = if matches!(request, ControlClass::Urgent) {
                    ControlClass::Urgent
                } else {
                    ControlClass::Background
                };
                assert_eq!(
                    ClassSelection::Handle(SubmissionClass::Background).resolve(request),
                    expected,
                    "a class-selected view preserves the safety floor",
                );
                assert_eq!(
                    ClassSelection::Handle(SubmissionClass::Background).resolve(request),
                    expected,
                    "a handle default never demotes an urgent request",
                );
            }
        }

        /// A handle default and a class-selected derived view both reach the
        /// lowered `ControlPolicy` of a plain command.
        #[test]
        fn plain_command_lowers_the_selected_class() {
            assert_eq!(
                submission_class(ClassSelection::Request),
                EngineControlClass::Background
            );
            assert_eq!(
                submission_class(ClassSelection::Handle(SubmissionClass::User)),
                EngineControlClass::User,
            );
            assert_eq!(
                submission_class(ClassSelection::Handle(SubmissionClass::Normal)),
                EngineControlClass::Normal,
            );
        }

        /// Inquiries share the same four lanes, so a demoted telemetry handle
        /// really does move its polling out of the way.
        #[test]
        fn inquiry_lowers_the_selected_class() {
            assert_eq!(
                inquiry_class(ClassSelection::Request),
                EngineControlClass::Normal
            );
            assert_eq!(
                inquiry_class(ClassSelection::Handle(SubmissionClass::Background)),
                EngineControlClass::Background,
            );
            assert_eq!(
                inquiry_class(ClassSelection::Handle(SubmissionClass::User)),
                EngineControlClass::User,
            );
        }

        /// A public downstream implementation cannot manufacture the urgent
        /// safety lane simply by setting its associated constant.
        #[test]
        fn downstream_urgent_operation_is_rejected_before_lowering() {
            for class in [
                ClassSelection::Request,
                ClassSelection::Handle(SubmissionClass::Background),
                ClassSelection::Handle(SubmissionClass::Background),
            ] {
                assert!(matches!(
                    downstream_urgent_result(class),
                    Err(Error::InvalidRequest(_))
                ));
            }
        }

        /// The safety rule, on the lowered value rather than on `resolve`
        /// alone: neither public QoS route can demote a crate-owned urgent
        /// stop.
        #[test]
        fn crate_owned_urgent_stop_is_preserved_by_every_submission_class() {
            assert_eq!(
                builtin_stop_class(ClassSelection::Request),
                EngineControlClass::Urgent
            );
            assert_eq!(
                builtin_stop_class(ClassSelection::Handle(SubmissionClass::Background)),
                EngineControlClass::Urgent,
                "issue #630: a handle default must never demote an urgent stop",
            );
            assert_eq!(
                builtin_stop_class(ClassSelection::Handle(SubmissionClass::Background)),
                EngineControlClass::Urgent,
                "issue #542: view QoS must never demote an urgent stop",
            );
        }
    }

    /// #700: a raw command's declared reply shape lowers into the engine request
    /// context as a protocol-policy fact. The engine reads it there rather than
    /// inferring it from the wire bytes.
    #[test]
    fn declared_raw_motion_lowers_axes_without_forging_stop_authority() {
        let profile =
            ProfileSpec::from_compile_time::<crate::profiles::GenericVisca>().expect("profile");
        let policy =
            crate::raw::Policy::new(TimeoutClass::Quick, RetryClass::Never, ControlClass::Normal)
                .expect("policy");
        let command = crate::raw::Plain::with_policy([0x81, 0x01, 0x04, 0x07, 0x02, 0xff], policy)
            .expect("plain");
        let undeclared = prepare_command(
            &command,
            &crate::TargetRegistration::new((TargetSlot::CAMERA_1).camera(), profile.clone())
                .expect("valid target registration"),
            OperationalTuning::new(),
            ClassSelection::Request,
        )
        .expect("prepares");
        assert!(undeclared.context.motion.is_none());
        let declared = prepare_command(
            &command.with_motion_axes(AffectedAxes::ZOOM),
            &crate::TargetRegistration::new((TargetSlot::CAMERA_1).camera(), profile.clone())
                .expect("valid target registration"),
            OperationalTuning::new(),
            ClassSelection::Request,
        )
        .expect("prepares");
        assert_eq!(
            declared.context.motion,
            Some(crate::runtime::engine::MotionEffect {
                axes: AffectedAxes::ZOOM,
                stop: false
            })
        );
        assert_eq!(
            declared.context.control.class,
            crate::runtime::engine::ControlClass::Normal
        );
    }

    #[test]
    fn raw_reply_shape_lowers_into_request_context() {
        let profile = ProfileSpec::from_compile_time::<crate::profiles::GenericVisca>()
            .expect("built-in profile");
        for (declared, expected) in [
            (
                crate::raw::RawReplyShape::AckThenCompletion,
                ReplyShape::AckThenCompletion,
            ),
            (
                crate::raw::RawReplyShape::CompletionOnly,
                ReplyShape::CompletionOnly,
            ),
            (crate::raw::RawReplyShape::NoReply, ReplyShape::NoReply),
        ] {
            let policy = crate::raw::Policy::new(
                TimeoutClass::Quick,
                RetryClass::Never,
                ControlClass::Normal,
            )
            .expect("policy")
            .with_reply_shape(declared);
            let command =
                crate::raw::Plain::with_policy([0x81, 0x01, 0x02, 0xff], policy).expect("plain");
            let prepared = prepare_command(
                &command,
                &crate::TargetRegistration::new((TargetSlot::CAMERA_1).camera(), profile.clone())
                    .expect("valid target registration"),
                OperationalTuning::new(),
                ClassSelection::Request,
            )
            .expect("prepares");
            assert_eq!(prepared.reply_shape, expected);
        }

        // A raw command built with no explicit shape — and, by the same trait
        // default, every built-in — lowers to AckThenCompletion.
        let default_command = crate::raw::Plain::new(
            [0x81, 0x01, 0x02, 0xff],
            TimeoutClass::Quick,
            RetryClass::Never,
            ControlClass::Normal,
        )
        .expect("plain");
        let prepared = prepare_command(
            &default_command,
            &crate::TargetRegistration::new((TargetSlot::CAMERA_1).camera(), profile.clone())
                .expect("valid target registration"),
            OperationalTuning::new(),
            ClassSelection::Request,
        )
        .expect("prepares");
        assert_eq!(prepared.reply_shape, ReplyShape::AckThenCompletion);
    }

    #[test]
    fn no_reply_operation_is_rejected_during_generic_preparation() {
        let profile = ProfileSpec::from_compile_time::<crate::profiles::GenericVisca>()
            .expect("built-in profile");
        let error = prepare_operation::<completion::AppliedOnly, _>(
            &NoReplyAppliedOnly,
            &crate::TargetRegistration::new((TargetSlot::CAMERA_1).camera(), profile.clone())
                .expect("valid target registration"),
            OperationalTuning::new(),
            ClassSelection::Request,
        )
        .expect_err("a no-reply operation must not create an applied handle");
        assert!(matches!(error, Error::InvalidRequest(_)));
    }
}

/// Three independent typed STOPs, lowered before entering the owner boundary.
#[cfg(any(feature = "async", feature = "blocking"))]
#[derive(Debug)]
pub(crate) struct PreparedHalt {
    pub(crate) target: TargetSlot,
    pub(crate) axes: Option<AffectedAxes>,
    pub(crate) requests: [Option<Result<RuntimeRequest>>; 3],
    pub(crate) budget: Duration,
}

#[cfg(any(feature = "async", feature = "blocking"))]
pub(crate) fn prepare_halt(
    registration: &crate::TargetRegistration,
    tuning: ValidatedTuning,
) -> PreparedHalt {
    let target = registration.slot();
    let profile = registration.profile();
    use crate::request::builtin::{FocusStop, ZoomStop};
    let mut budget = completion_timeout(TimeoutClass::Movement, profile, tuning.get());
    let mut lower = |prepared: Result<PreparedOperation<completion::AppliedOnly>>| {
        prepared.map(|prepared| {
            prepared.admit_with(|request, _, _, timeouts| {
                budget = budget.max(timeouts.applied);
                request
            })
        })
    };
    let pan_tilt = profile.supports_axes(AffectedAxes::PAN_TILT).then(|| {
        lower(
            crate::stop_request::pan_tilt_stop_request(profile).and_then(|stop| {
                prepare_operation(&stop, registration, tuning, ClassSelection::Request)
            }),
        )
    });
    let zoom = profile.supports_axes(AffectedAxes::ZOOM).then(|| {
        lower(prepare_operation(
            &ZoomStop,
            registration,
            tuning,
            ClassSelection::Request,
        ))
    });
    let focus = profile.supports_axes(AffectedAxes::FOCUS).then(|| {
        lower(prepare_operation(
            &FocusStop,
            registration,
            tuning,
            ClassSelection::Request,
        ))
    });
    let mut axes: Option<AffectedAxes> = None;
    for (supported, axis) in [
        (pan_tilt.is_some(), AffectedAxes::PAN_TILT),
        (zoom.is_some(), AffectedAxes::ZOOM),
        (focus.is_some(), AffectedAxes::FOCUS),
    ] {
        if supported {
            axes = Some(axes.map_or(axis, |old| old.union(axis)));
        }
    }
    PreparedHalt {
        target,
        axes,
        requests: [pan_tilt, zoom, focus],
        budget,
    }
}
