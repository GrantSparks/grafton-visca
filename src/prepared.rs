//! Pure profile-aware lowering from typed requests to inert engine inputs.

use std::{marker::PhantomData, sync::Arc, time::Duration};

use smallvec::SmallVec;

use crate::{
    camera::{MovementTolerance, PanTiltPosition},
    command::{
        inquiry::{
            FocusPositionInquiry, IrisInquiry, NdFilterInquiry, PanTiltPositionInquiry,
            ZoomPositionInquiry,
        },
        resolution::NdFilterPosition,
    },
    completion,
    types::{FocusPosition, IrisLevel, ZoomPosition},
    AffectedAxes, CameraId, ControlClass, Inquiry, InquiryRoute, OperationCommand,
    OperationalTuning, PlainCommand, ProfileSpec, Request, ResponseDecoder, RetryClass,
    SubmissionClass, TimeoutClass,
};
use crate::{Error, Result};

use crate::raw::{INLINE_BYTES, MAX_BYTES};
use crate::runtime::engine::{
    AppliedStateProjection, CancellationPolicy, ControlPolicy, EncodedMessage, RequestContext,
    RetryPolicy, RuntimeRequest, TimeoutPolicy,
};

/// How one submission's scheduling lane is chosen.
///
/// Every request already classifies itself through [`Request::control_class`].
/// A camera handle may carry ordinary submission QoS, and a single submission
/// may name its own QoS. Neither form can represent or demote the safety lane.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub(crate) enum ClassSelection {
    /// Use the request's own classification.
    #[default]
    Request,
    /// A camera handle's ordinary-traffic default.
    Handle(SubmissionClass),
    /// One submission's explicit ordinary-traffic class.
    Explicit(SubmissionClass),
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
            Self::Handle(class) | Self::Explicit(class) => match request {
                ControlClass::Urgent => ControlClass::Urgent,
                _ => class.control_class(),
            },
        }
    }

    /// Returns the handle default this selection carries, if any.
    pub(crate) const fn handle_default(self) -> Option<SubmissionClass> {
        match self {
            Self::Handle(class) => Some(class),
            Self::Request | Self::Explicit(_) => None,
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

/// Complete erased physical-settlement selection made before admission.
// The poll plan is intentionally kept inline: moving it behind a box would
// add an allocation to every targeted preparation.
#[allow(clippy::large_enum_variant)]
#[derive(Debug, Clone)]
pub(crate) enum SettlementPlan {
    /// Applied protocol completion is also physical settlement.
    CompletionIsSettled {
        target: CameraId,
        default_budget: Duration,
    },
    /// Settlement must poll the exact affected axes.
    Poll {
        target: CameraId,
        queries: PositionQueryPlan,
        axes: AffectedAxes,
        tolerance: MovementTolerance,
        interval: Duration,
        default_budget: Duration,
    },
}

/// Reusable, inert lowering of one typed inquiry. Re-instantiation preserves
/// the exact wire, route, decoder, target, and protocol policy selected during
/// operation preparation.
#[derive(Debug, Clone)]
pub(crate) struct PreparedInquiryTemplate<R> {
    wire: Arc<EncodedMessage>,
    context: RequestContext,
    route: InquiryRoute,
    decoder: ResponseDecoder<R>,
}

impl<R> PreparedInquiryTemplate<R> {
    pub(crate) fn instantiate(&self) -> PreparedInquiry<R> {
        PreparedInquiry {
            wire: Arc::clone(&self.wire),
            context: self.context,
            route: self.route,
            decoder: self.decoder.clone(),
        }
    }
}

impl SettlementPlan {
    // Convenience accessor over the target both variants already carry; the
    // settlement poller in `crate::completion` is what will read it rather than
    // re-matching the plan. Left in place because #630 is extending this module
    // concurrently (#636).
    #[allow(dead_code)]
    pub(crate) const fn target(&self) -> Option<CameraId> {
        match self {
            Self::CompletionIsSettled { target, .. } | Self::Poll { target, .. } => Some(*target),
        }
    }

    pub(crate) const fn default_budget(&self) -> Option<Duration> {
        match self {
            Self::CompletionIsSettled { default_budget, .. }
            | Self::Poll { default_budget, .. } => Some(*default_budget),
        }
    }
}

/// Complete set of typed position inquiries selected for one operation.
#[derive(Debug, Clone)]
pub(crate) struct PositionQueryPlan {
    pub(crate) pan_tilt: Option<PreparedInquiryTemplate<PanTiltPosition>>,
    pub(crate) zoom: Option<PreparedInquiryTemplate<ZoomPosition>>,
    pub(crate) focus: Option<PreparedInquiryTemplate<FocusPosition>>,
    pub(crate) iris: Option<PreparedInquiryTemplate<IrisLevel>>,
    pub(crate) nd_filter: Option<PreparedInquiryTemplate<NdFilterPosition>>,
}

impl PositionQueryPlan {
    fn prepare(
        target: CameraId,
        profile: &ProfileSpec,
        tuning: OperationalTuning,
        axes: AffectedAxes,
    ) -> Result<Self> {
        Ok(Self {
            pan_tilt: axes
                .contains(AffectedAxes::PAN_TILT)
                .then(|| prepare_builtin_inquiry(&PanTiltPositionInquiry, target, profile, tuning))
                .transpose()?
                .map(PreparedInquiry::into_template),
            zoom: axes
                .contains(AffectedAxes::ZOOM)
                .then(|| prepare_builtin_inquiry(&ZoomPositionInquiry, target, profile, tuning))
                .transpose()?
                .map(PreparedInquiry::into_template),
            focus: axes
                .contains(AffectedAxes::FOCUS)
                .then(|| prepare_builtin_inquiry(&FocusPositionInquiry, target, profile, tuning))
                .transpose()?
                .map(PreparedInquiry::into_template),
            iris: axes
                .contains(AffectedAxes::IRIS)
                .then(|| prepare_builtin_inquiry(&IrisInquiry, target, profile, tuning))
                .transpose()?
                .map(PreparedInquiry::into_template),
            nd_filter: axes
                .contains(AffectedAxes::ND_FILTER)
                .then(|| prepare_builtin_inquiry(&NdFilterInquiry, target, profile, tuning))
                .transpose()?
                .map(PreparedInquiry::into_template),
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
    target: CameraId,
    profile: &ProfileSpec,
    tuning: OperationalTuning,
    axes: AffectedAxes,
) -> Result<PositionQueryPlan> {
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
    PositionQueryPlan::prepare(target, profile, tuning, axes)
}

/// One complete position sample for an explicitly selected axis set.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub(crate) struct PositionSnapshot {
    pub(crate) pan_tilt: Option<PanTiltPosition>,
    pub(crate) zoom: Option<ZoomPosition>,
    pub(crate) focus: Option<FocusPosition>,
    pub(crate) iris: Option<IrisLevel>,
    pub(crate) nd_filter: Option<NdFilterPosition>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum MotionState {
    NeedSample,
    Moving,
    Settled,
}

/// Pure selected-axis movement detector shared by both execution modes.
#[derive(Debug, Clone, Copy)]
pub(crate) struct MotionDetector {
    axes: AffectedAxes,
    tolerance: MovementTolerance,
    previous: Option<PositionSnapshot>,
}

impl MotionDetector {
    pub(crate) const fn new(axes: AffectedAxes, tolerance: MovementTolerance) -> Self {
        Self {
            axes,
            tolerance,
            previous: None,
        }
    }

    pub(crate) fn observe(&mut self, snapshot: PositionSnapshot) -> Result<MotionState> {
        snapshot.validate(self.axes)?;
        let Some(previous) = self.previous.replace(snapshot) else {
            return Ok(MotionState::NeedSample);
        };
        Ok(
            if snapshots_moved(self.axes, self.tolerance, previous, snapshot)? {
                MotionState::Moving
            } else {
                MotionState::Settled
            },
        )
    }
}

impl PositionSnapshot {
    fn validate(self, axes: AffectedAxes) -> Result<()> {
        let complete = (!axes.contains(AffectedAxes::PAN_TILT) || self.pan_tilt.is_some())
            && (!axes.contains(AffectedAxes::ZOOM) || self.zoom.is_some())
            && (!axes.contains(AffectedAxes::FOCUS) || self.focus.is_some())
            && (!axes.contains(AffectedAxes::IRIS) || self.iris.is_some())
            && (!axes.contains(AffectedAxes::ND_FILTER) || self.nd_filter.is_some());
        if complete {
            Ok(())
        } else {
            Err(Error::InvalidState(
                "position snapshot omitted a selected settlement axis".into(),
            ))
        }
    }
}

fn snapshots_moved(
    axes: AffectedAxes,
    tolerance: MovementTolerance,
    previous: PositionSnapshot,
    current: PositionSnapshot,
) -> Result<bool> {
    if axes.contains(AffectedAxes::PAN_TILT) {
        let (previous_pan, previous_tilt) = previous
            .pan_tilt
            .ok_or_else(|| {
                Error::InvalidState("previous snapshot omitted selected pan/tilt".into())
            })?
            .raw_values();
        let (current_pan, current_tilt) = current
            .pan_tilt
            .ok_or_else(|| {
                Error::InvalidState("current snapshot omitted selected pan/tilt".into())
            })?
            .raw_values();
        let tolerance = u32::from(tolerance.pan_tilt.unsigned_abs());
        if (i32::from(previous_pan) - i32::from(current_pan)).unsigned_abs() > tolerance
            || (i32::from(previous_tilt) - i32::from(current_tilt)).unsigned_abs() > tolerance
        {
            return Ok(true);
        }
    }
    if axes.contains(AffectedAxes::ZOOM) {
        let previous = previous
            .zoom
            .ok_or_else(|| Error::InvalidState("previous snapshot omitted selected zoom".into()))?
            .value();
        let current = current
            .zoom
            .ok_or_else(|| Error::InvalidState("current snapshot omitted selected zoom".into()))?
            .value();
        if previous.abs_diff(current) > tolerance.zoom {
            return Ok(true);
        }
    }
    if axes.contains(AffectedAxes::FOCUS) {
        let previous = previous
            .focus
            .ok_or_else(|| Error::InvalidState("previous snapshot omitted selected focus".into()))?
            .value();
        let current = current
            .focus
            .ok_or_else(|| Error::InvalidState("current snapshot omitted selected focus".into()))?
            .value();
        if previous.abs_diff(current) > tolerance.focus {
            return Ok(true);
        }
    }
    if axes.contains(AffectedAxes::IRIS) {
        let previous = previous
            .iris
            .ok_or_else(|| Error::InvalidState("previous snapshot omitted selected iris".into()))?
            .value();
        let current = current
            .iris
            .ok_or_else(|| Error::InvalidState("current snapshot omitted selected iris".into()))?
            .value();
        if u16::from(previous.abs_diff(current)) > tolerance.iris {
            return Ok(true);
        }
    }
    if axes.contains(AffectedAxes::ND_FILTER) {
        let previous = previous
            .nd_filter
            .ok_or_else(|| {
                Error::InvalidState("previous snapshot omitted selected ND filter".into())
            })?
            .as_byte();
        let current = current
            .nd_filter
            .ok_or_else(|| {
                Error::InvalidState("current snapshot omitted selected ND filter".into())
            })?
            .as_byte();
        if u16::from(previous.abs_diff(current)) > tolerance.nd_filter {
            return Ok(true);
        }
    }
    Ok(false)
}

/// A fully lowered ordinary command.
#[derive(Debug)]
pub(crate) struct PreparedCommand {
    wire: Arc<EncodedMessage>,
    context: RequestContext,
    applied_state: Option<AppliedStateProjection>,
}

/// A fully lowered typed inquiry. Decoding remains outside the engine.
#[derive(Debug)]
pub(crate) struct PreparedInquiry<R> {
    wire: Arc<EncodedMessage>,
    context: RequestContext,
    route: InquiryRoute,
    decoder: ResponseDecoder<R>,
}

/// A fully lowered operation with compile-time completion semantics.
#[derive(Debug)]
pub(crate) struct PreparedOperation<K>
where
    K: completion::Kind,
{
    wire: Arc<EncodedMessage>,
    context: RequestContext,
    affected_axes: AffectedAxes,
    settlement: completion::Settlement<K>,
    applied_state: Option<AppliedStateProjection>,
    marker: PhantomData<fn() -> K>,
}

pub(crate) fn lower_targeted_settlement(
    target: CameraId,
    profile: &ProfileSpec,
    tuning: OperationalTuning,
    axes: AffectedAxes,
    default_budget: Duration,
) -> Result<SettlementPlan> {
    if !profile.supports_axes(axes) {
        return Err(Error::FeatureNotSupported {
            feature: "affected operation axes are not supported by the profile",
        });
    }
    if profile.supports_operation_complete() {
        return Ok(SettlementPlan::CompletionIsSettled {
            target,
            default_budget,
        });
    }
    if !profile.position_inquiries().supports(axes) {
        return Err(Error::FeatureNotSupported {
            feature: "position inquiries required to settle the affected axes",
        });
    }
    Ok(SettlementPlan::Poll {
        target,
        queries: prepare_position_queries(target, profile, tuning, axes)?,
        axes,
        tolerance: MovementTolerance::default(),
        interval: tuning
            .inquiry_spacing_override()
            .unwrap_or_else(|| profile.timing().minimum_inquiry_spacing())
            .max(Duration::from_millis(25)),
        default_budget,
    })
}

pub(crate) fn lower_applied_only_settlement(
    _target: CameraId,
    profile: &ProfileSpec,
    _tuning: OperationalTuning,
    axes: AffectedAxes,
    _default_budget: Duration,
) -> Result<()> {
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
    target: CameraId,
    profile: &ProfileSpec,
    tuning: OperationalTuning,
    class: ClassSelection,
) -> Result<PreparedCommand>
where
    C: PlainCommand + ?Sized,
{
    prepare_command_with_state(
        command,
        target,
        profile,
        tuning,
        class,
        crate::requests::applied_state_projection(command),
    )
}

/// Prepares a crate built-in command with its closed applied-state selection.
#[cfg(test)]
pub(crate) fn prepare_builtin_command<C>(
    command: &C,
    target: CameraId,
    profile: &ProfileSpec,
    tuning: OperationalTuning,
) -> Result<PreparedCommand>
where
    C: PlainCommand + crate::request::builtin::BuiltinValidation + ?Sized,
{
    prepare_command(command, target, profile, tuning, ClassSelection::Request)
}

fn prepare_command_with_state<C>(
    command: &C,
    target: CameraId,
    profile: &ProfileSpec,
    tuning: OperationalTuning,
    class: ClassSelection,
    applied_state: Option<AppliedStateProjection>,
) -> Result<PreparedCommand>
where
    C: PlainCommand + ?Sized,
{
    profile.validate_tuning(tuning)?;
    validate_timeout_class(command.timeout_class(), false)?;
    command.validate_for_profile(profile)?;
    let wire = encode(command, target)?;
    let context = request_context(command, target, profile, tuning, class, false, false);
    Ok(PreparedCommand {
        wire,
        context,
        applied_state,
    })
}

/// Prepares a generic typed inquiry through the shared lowering path.
pub(crate) fn prepare_inquiry<Q>(
    inquiry: &Q,
    target: CameraId,
    profile: &ProfileSpec,
    tuning: OperationalTuning,
    class: ClassSelection,
) -> Result<PreparedInquiry<Q::Response>>
where
    Q: Inquiry + ?Sized,
{
    prepare_inquiry_with_policy(inquiry, target, profile, tuning, class, false)
}

/// Prepares a crate-generated inquiry with its closed protocol retry policy.
pub(crate) fn prepare_builtin_inquiry<Q>(
    inquiry: &Q,
    target: CameraId,
    profile: &ProfileSpec,
    tuning: OperationalTuning,
) -> Result<PreparedInquiry<Q::Response>>
where
    Q: Inquiry + BuiltinInquiryRequest + ?Sized,
{
    prepare_inquiry_with_policy(
        inquiry,
        target,
        profile,
        tuning,
        ClassSelection::Request,
        true,
    )
}

pub(crate) trait BuiltinInquiryRequest: Inquiry {}

fn prepare_inquiry_with_policy<Q>(
    inquiry: &Q,
    target: CameraId,
    profile: &ProfileSpec,
    tuning: OperationalTuning,
    class: ClassSelection,
    builtin_inquiry_syntax: bool,
) -> Result<PreparedInquiry<Q::Response>>
where
    Q: Inquiry + ?Sized,
{
    if target.is_broadcast() {
        return Err(Error::InvalidRequest(
            "inquiries require an individual camera target".into(),
        ));
    }
    validate_timeout_class(inquiry.timeout_class(), true)?;
    profile.validate_tuning(tuning)?;
    if matches!(
        profile.capabilities().inquiry_support,
        crate::capabilities::InquirySupport::None
    ) {
        return Err(Error::FeatureNotSupported {
            feature: "VISCA inquiries",
        });
    }
    inquiry.validate_for_profile(profile)?;
    let wire = encode(inquiry, target)?;
    let context = request_context(
        inquiry,
        target,
        profile,
        tuning,
        class,
        true,
        builtin_inquiry_syntax,
    );
    Ok(PreparedInquiry {
        wire,
        context,
        route: inquiry.route(),
        decoder: inquiry.decoder_for_profile(profile),
    })
}

/// Prepares a generic operation through the shared lowering path.
pub(crate) fn prepare_operation<K, O>(
    operation: &O,
    target: CameraId,
    profile: &ProfileSpec,
    tuning: OperationalTuning,
    class: ClassSelection,
) -> Result<PreparedOperation<K>>
where
    K: completion::Kind,
    O: OperationCommand<K> + ?Sized,
{
    prepare_operation_with_state(
        operation,
        target,
        profile,
        tuning,
        class,
        crate::requests::applied_state_projection(operation),
    )
}

/// Prepares a crate built-in operation with its closed applied-state selection.
#[cfg(test)]
pub(crate) fn prepare_builtin_operation<K, O>(
    operation: &O,
    target: CameraId,
    profile: &ProfileSpec,
    tuning: OperationalTuning,
) -> Result<PreparedOperation<K>>
where
    K: completion::Kind,
    O: OperationCommand<K> + crate::request::builtin::BuiltinValidation + ?Sized,
{
    prepare_operation::<K, _>(operation, target, profile, tuning, ClassSelection::Request)
}

fn prepare_operation_with_state<K, O>(
    operation: &O,
    target: CameraId,
    profile: &ProfileSpec,
    tuning: OperationalTuning,
    class: ClassSelection,
    applied_state: Option<AppliedStateProjection>,
) -> Result<PreparedOperation<K>>
where
    K: completion::Kind,
    O: OperationCommand<K> + ?Sized,
{
    if target.is_broadcast() {
        return Err(Error::InvalidRequest(
            "observable operations require an individual camera target".into(),
        ));
    }
    validate_timeout_class(operation.timeout_class(), false)?;
    profile.validate_tuning(tuning)?;
    // Domain/profile admission must complete before lowering settlement
    // inquiries or encoding the command.  In particular, an unsupported
    // targeted built-in must not even prepare an inquiry plan as a side
    // effect of discovering that its operation class is unavailable.
    operation.validate_for_profile(profile)?;
    let affected_axes = operation.affected_axes();
    let context = request_context(operation, target, profile, tuning, class, false, false);
    let settlement = K::lower_settlement(
        target,
        profile,
        tuning,
        affected_axes,
        settlement_budget(operation.timeout_class(), profile, tuning),
    )?;
    let wire = encode(operation, target)?;
    Ok(PreparedOperation {
        wire,
        context,
        affected_axes,
        settlement,
        applied_state,
        marker: PhantomData,
    })
}

fn encode<R>(request: &R, target: CameraId) -> Result<Arc<EncodedMessage>>
where
    R: Request + ?Sized,
{
    let size = request.encoded_size();
    if size == 0 || size > R::MAX_SIZE || size > MAX_BYTES {
        return Err(Error::InvalidRequest(
            "encoded_size must be non-zero and no larger than MAX_SIZE or the protocol bound"
                .into(),
        ));
    }
    let mut bytes = SmallVec::<[u8; INLINE_BYTES]>::from_elem(0, size);
    let written = request.write_into(target, &mut bytes)?;
    if written != size || written > bytes.len() {
        return Err(Error::InvalidRequest(
            "write_into must report exactly encoded_size bytes within the supplied buffer".into(),
        ));
    }
    if written < 2 || bytes[0] != target.to_address_byte() {
        return Err(Error::InvalidRequest(
            "write_into must encode the explicit target address and a non-empty VISCA message"
                .into(),
        ));
    }
    Ok(Arc::new(EncodedMessage::new(&bytes[..written])?))
}

fn validate_timeout_class(class: TimeoutClass, inquiry: bool) -> Result<()> {
    let valid = if inquiry {
        matches!(class, TimeoutClass::Inquiry)
    } else {
        matches!(
            class,
            TimeoutClass::Quick
                | TimeoutClass::Movement
                | TimeoutClass::Preset
                | TimeoutClass::LongRunning
                | TimeoutClass::Network
        )
    };
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
    target: CameraId,
    profile: &ProfileSpec,
    tuning: OperationalTuning,
    class: ClassSelection,
    inquiry: bool,
    builtin_inquiry_syntax: bool,
) -> RequestContext
where
    R: Request + ?Sized,
{
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
    RequestContext {
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
            builtin_inquiry_syntax,
        ),
        control: ControlPolicy {
            class: lower_control(class.resolve(request.control_class())),
            minimum_spacing: spacing,
        },
        cancellation: if profile.supports_command_cancel() {
            CancellationPolicy::Supported
        } else {
            CancellationPolicy::Unsupported
        },
    }
}

fn completion_timeout(
    class: TimeoutClass,
    profile: &ProfileSpec,
    tuning: OperationalTuning,
) -> Duration {
    let timing = profile.timing();
    match class {
        TimeoutClass::Quick => tuning
            .quick_timeout_override()
            .unwrap_or(timing.command_timeouts().quick_timeout()),
        TimeoutClass::Movement => tuning
            .movement_timeout_override()
            .unwrap_or(timing.command_timeouts().movement_timeout()),
        TimeoutClass::Preset => tuning
            .preset_timeout_override()
            .unwrap_or(timing.command_timeouts().preset_timeout()),
        TimeoutClass::LongRunning => tuning
            .long_running_timeout_override()
            .unwrap_or(timing.command_timeouts().long_running_timeout()),
        TimeoutClass::Network => tuning
            .network_timeout_override()
            .unwrap_or(timing.command_timeouts().network_timeout()),
        // Inquiry requests use `TimeoutPolicy::inquiry`; keep the otherwise
        // unused completion field aligned with 1.x's Quick category.
        TimeoutClass::Inquiry => timing.command_timeouts().quick_timeout(),
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

/// Base retry count every per-category budget is derived from.
///
/// This is 1.x's `RetryConfig::default().max_retries`, and
/// [`OperationalTuning::retry_limit`] overrides exactly this number — not the
/// final per-category count — because that is the knob 1.x exposed.
const DEFAULT_RETRY_BASE: u32 = 3;

/// Floor for the total wall-clock a request may spend retrying, counted from
/// admission.
///
/// 1.x's `RetryConfig::default().max_retry_duration`. The governing request
/// deadline and profile busy timeout can raise this floor for a request whose
/// first attempt is longer than ten seconds.
const MINIMUM_RETRY_BUDGET: Duration = Duration::from_secs(10);

/// Bounded retry count for one timeout category.
///
/// This is 1.x's `RetryBudget::from_base` (`main:src/runtime/core/mod.rs`)
/// restored verbatim: quick work gets two extra attempts because it is cheap
/// to replay, network work gets one fewer because a failing link rarely
/// recovers within a retry, and a long-running command gets exactly one
/// attempt to spare the camera a second multi-minute operation. 1.x keyed this
/// on the request's [`TimeoutClass`]. Inquiries retain their own retry class
/// while their response deadline is selected from the inquiry timing fact.
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
/// `deadline` is the request's own governing deadline — its completion
/// deadline for a command, its reply deadline for an inquiry — and is what
/// the total budget is sized against. A flat budget cannot work here: a
/// request must be allowed to finish one governing attempt and still have
/// room for a retry. The profile busy timeout is included for camera-side
/// recovery windows.
fn retry_policy(
    retry_class: RetryClass,
    timeout_class: TimeoutClass,
    tuning: OperationalTuning,
    busy_timeout: Duration,
    deadline: Duration,
    builtin_inquiry_syntax: bool,
) -> RetryPolicy {
    let default_initial = Duration::from_millis(50);
    let default_maximum = Duration::from_millis(500).max(busy_timeout);
    let default_budget = MINIMUM_RETRY_BUDGET
        .max(deadline.saturating_mul(2))
        .max(busy_timeout);
    let (initial, maximum, budget) = tuning.retry_timing_override();
    let base = tuning.retry_limit_override().unwrap_or(DEFAULT_RETRY_BASE);
    let max_retries = if matches!(retry_class, RetryClass::Never) {
        0
    } else {
        retry_budget(base, timeout_class)
    };
    // `Never` is the sole policy opt-out from automatic replay. These flags
    // are only policy permissions: the engine narrows them further using
    // envelope evidence. Thus sequence-correlated Sony traffic may retry a
    // lost ACK or post-ACK completion timeout, while a successfully sent raw
    // command is poisoned on an ambiguous outcome rather than replayed.
    let replayable = !matches!(retry_class, RetryClass::Never);
    RetryPolicy {
        max_retries,
        initial_backoff: initial.unwrap_or(default_initial),
        maximum_backoff: maximum.unwrap_or(default_maximum),
        total_budget: budget.unwrap_or(default_budget),
        ack_timeout: replayable,
        completion_timeout: replayable,
        inquiry_timeout: matches!(retry_class, RetryClass::Inquiry),
        buffer_full: replayable,
        movement_not_executable: matches!(retry_class, RetryClass::Movement | RetryClass::Preset),
        builtin_inquiry_syntax,
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

impl PreparedCommand {
    pub(crate) fn admit_with<T>(self, admit: impl FnOnce(RuntimeRequest, Duration) -> T) -> T {
        let timeout = self.context.timeout.completion;
        let request = RuntimeRequest::Command {
            wire: self.wire,
            context: self.context,
            applied_state: self.applied_state,
        };
        admit(request, timeout)
    }
}

impl<R> PreparedInquiry<R> {
    fn into_template(self) -> PreparedInquiryTemplate<R> {
        PreparedInquiryTemplate {
            wire: self.wire,
            context: self.context,
            route: self.route,
            decoder: self.decoder,
        }
    }

    // Consumed by the blocking owner's inquiry submission seam
    // (`runtime::owner::blocking`), which an async-only leg does not compile.
    #[allow(dead_code)]
    pub(crate) fn into_parts(self) -> (RuntimeRequest, ResponseDecoder<R>, Duration) {
        let timeout = self.context.timeout.inquiry;
        let request = RuntimeRequest::Inquiry {
            wire: self.wire,
            context: self.context,
            route: crate::runtime::engine::InquiryRoute(self.route.identifier()),
        };
        (request, self.decoder, timeout)
    }

    pub(crate) fn admit_with<T>(
        self,
        admit: impl FnOnce(RuntimeRequest, ResponseDecoder<R>, Duration) -> T,
    ) -> T {
        let timeout = self.context.timeout.inquiry;
        let request = RuntimeRequest::Inquiry {
            wire: self.wire,
            context: self.context,
            route: crate::runtime::engine::InquiryRoute(self.route.identifier()),
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
        admit: impl FnOnce(RuntimeRequest, AffectedAxes, completion::Settlement<K>, Duration) -> T,
    ) -> T {
        let timeout = self.context.timeout.completion;
        let request = RuntimeRequest::Command {
            wire: self.wire,
            context: self.context,
            applied_state: self.applied_state,
        };
        admit(request, self.affected_axes, self.settlement, timeout)
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::panic, unused_qualifications)]
mod tests {
    use std::cell::Cell;

    use super::*;
    use crate::{
        capabilities::{Capabilities, InquirySupport, TypedSupportSet},
        command::{
            FocusNearLimitInquiry, NdFilterPosition, PanTilt, PanTiltLimitCorner,
            PanTiltPositionInquiry, PowerInquiry, VersionInquiry, ZoomPositionInquiry,
            VISCA_TERMINATOR,
        },
        request::builtin::{
            request_write_count, reset_request_write_count, FocusTrigger, IrisReset,
            NdFilterStepUp, PanTiltAbsolute, PanTiltLimitClear, PanTiltLimitSet, PanTiltRelative,
            PresetSet, ZoomTarget,
        },
        types::{IrisLevel, PanSpeed, TiltSpeed, ZoomPosition},
        units::Degrees,
        PositionInquirySupport, PresetNumber, ProfileEnvelope, ProfileTiming,
        TransportCompatibility,
    };

    std::thread_local! {
        static WRITE_COUNT: Cell<usize> = const { Cell::new(0) };
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
        capabilities.supports_operation_complete = operation_complete;
        let transports = TransportCompatibility::new(
            capabilities.default_tcp_port,
            capabilities.default_udp_port,
            false,
        );
        ProfileSpec::builder(capabilities)
            .pan_tilt_coordinates(
                crate::capabilities::CoordinateSystem::SignedCentered,
                16.0,
                16.0,
            )
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
                    .minimum_inquiry_spacing(Duration::ZERO)
                    .minimum_command_spacing(Duration::ZERO)
                    .build()
                    .expect("valid timing"),
            )
            .maximum_command_sockets(1)
            .supports_operation_complete(operation_complete)
            .supports_command_cancel(false)
            .preset_recall_axes(Some(preset_axes))
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

        fn route(&self) -> InquiryRoute {
            InquiryRoute::RAW
        }

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

        fn route(&self) -> InquiryRoute {
            InquiryRoute::RAW
        }

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

    #[test]
    fn inquiry_support_and_settlement_fail_before_encoding() {
        reset_write_count();
        let mut no_inquiry = Capabilities::from_profile::<crate::profiles::GenericVisca>();
        no_inquiry.inquiry_support = InquirySupport::None;
        let no_inquiry = runtime_profile(
            no_inquiry,
            true,
            PositionInquirySupport::new(false, false, false),
            AffectedAxes::PAN_TILT,
        );
        assert!(prepare_inquiry(
            &CountingInquiry,
            CameraId::CAMERA_1,
            &no_inquiry,
            OperationalTuning::new(),
            ClassSelection::Request,
        )
        .is_err());
        assert_eq!(write_count(), 0);

        let mut missing_custom_zoom = Capabilities::from_profile::<crate::profiles::GenericVisca>();
        missing_custom_zoom.supports_direct_zoom = false;
        missing_custom_zoom.typed_support = TypedSupportSet::empty();
        let missing_custom_zoom = runtime_profile(
            missing_custom_zoom,
            false,
            PositionInquirySupport::new(true, false, true),
            AffectedAxes::PAN_TILT,
        );
        assert!(prepare_operation::<completion::Targeted, _>(
            &CountingTargeted,
            CameraId::CAMERA_1,
            &missing_custom_zoom,
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
            CameraId::CAMERA_1,
            &profile,
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
            CameraId::CAMERA_1,
            &profile,
            OperationalTuning::new(),
            ClassSelection::Request,
        )
        .expect_err("unsupported gated inquiry must not be prepared");
        assert!(matches!(error, Error::FeatureNotSupported { .. }));
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
                        .minimum_inquiry_spacing(Duration::ZERO)
                        .minimum_command_spacing(Duration::ZERO)
                        .build()
                        .expect("valid timing"),
                )
                .maximum_command_sockets(1)
                .supports_operation_complete(false)
                .supports_command_cancel(false)
                .preset_recall_axes(None)
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
                CameraId::CAMERA_1,
                profile,
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
        // `VersionInquiry` sits on the `System` noun (`noun_marker!` = `None`),
        // so it stays reachable regardless of the base domains.
        assert!(admits(&VersionInquiry, &bare));

        // Opting the power domain in flips the power inquiry to admitted while
        // the still-absent zoom domain keeps its inquiry refused: the erased
        // gate tracks each noun's base marker independently.
        let power_only = minimal_profile(|capabilities| {
            capabilities.has_power = true;
            capabilities.power_on_time = Duration::from_millis(750);
        });
        assert!(admits(&PowerInquiry, &power_only));
        assert!(!admits(&ZoomPositionInquiry, &power_only));
        assert!(admits(&VersionInquiry, &power_only));
    }

    #[test]
    fn targeted_poll_plan_preserves_exact_prepared_queries_target_and_budget() {
        let capabilities = Capabilities::from_profile::<crate::profiles::GenericVisca>();
        let profile = runtime_profile(
            capabilities,
            false,
            PositionInquirySupport::new(true, true, true),
            AffectedAxes::PAN_TILT,
        );
        let tuning = OperationalTuning::new()
            .inquiry_spacing(Duration::from_millis(40))
            .movement_timeout(Duration::from_secs(31))
            .settlement_timeout(Duration::from_secs(9));
        let prepared = prepare_operation::<completion::Targeted, _>(
            &CountingTargeted,
            CameraId::CAMERA_2,
            &profile,
            tuning,
            ClassSelection::Request,
        )
        .expect("targeted poll preparation");

        assert_eq!(prepared.context.timeout.completion, Duration::from_secs(31));
        let SettlementPlan::Poll {
            target,
            queries,
            axes,
            tolerance,
            interval,
            default_budget,
        } = prepared
            .settlement
            .into_plan()
            .expect("targeted settlement plan")
            .into_inner()
        else {
            panic!("profile without completion must choose polling");
        };
        assert_eq!(target, CameraId::CAMERA_2);
        assert_eq!(axes, AffectedAxes::ZOOM);
        assert_eq!(tolerance, MovementTolerance::default());
        assert_eq!(interval, Duration::from_millis(40));
        assert_eq!(default_budget, Duration::from_secs(9));
        assert!(queries.pan_tilt.is_none());
        assert!(queries.focus.is_none());
        let zoom = queries.zoom.expect("selected zoom query").instantiate();
        assert_eq!(zoom.context.target, CameraId::CAMERA_2);
        assert_eq!(zoom.context.timeout.inquiry, Duration::from_secs(1));
        assert_eq!(zoom.route, ZoomPositionInquiry.route());
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

    #[test]
    fn motion_detector_is_pure_selected_axis_tolerant_and_overflow_safe() {
        let tolerance = MovementTolerance {
            pan_tilt: 2,
            zoom: 10,
            focus: 5,
            iris: 0,
            nd_filter: 0,
        };
        let all = AffectedAxes::PAN_TILT
            .union(AffectedAxes::ZOOM)
            .union(AffectedAxes::FOCUS);
        let mut detector = MotionDetector::new(all, tolerance);
        assert_eq!(
            detector
                .observe(PositionSnapshot {
                    pan_tilt: Some(PanTiltPosition::new(i16::MIN, i16::MAX)),
                    zoom: Some(ZoomPosition::new(100).expect("zoom")),
                    focus: Some(FocusPosition::new(200)),
                    iris: None,
                    nd_filter: None,
                })
                .expect("complete baseline"),
            MotionState::NeedSample
        );
        assert_eq!(
            detector
                .observe(PositionSnapshot {
                    pan_tilt: Some(PanTiltPosition::new(i16::MAX, i16::MIN)),
                    zoom: Some(ZoomPosition::new(110).expect("zoom")),
                    focus: Some(FocusPosition::new(205)),
                    iris: None,
                    nd_filter: None,
                })
                .expect("complete moving sample"),
            MotionState::Moving
        );
        assert_eq!(
            detector
                .observe(PositionSnapshot {
                    pan_tilt: Some(PanTiltPosition::new(i16::MAX - 2, i16::MIN + 2)),
                    zoom: Some(ZoomPosition::new(100).expect("zoom")),
                    focus: Some(FocusPosition::new(200)),
                    iris: None,
                    nd_filter: None,
                })
                .expect("tolerance-bound sample"),
            MotionState::Settled
        );

        let mut zoom_only = MotionDetector::new(AffectedAxes::ZOOM, tolerance);
        assert_eq!(
            zoom_only
                .observe(PositionSnapshot {
                    pan_tilt: Some(PanTiltPosition::new(i16::MIN, i16::MAX)),
                    zoom: Some(ZoomPosition::new(10).expect("zoom")),
                    focus: None,
                    iris: None,
                    nd_filter: None,
                })
                .expect("zoom baseline"),
            MotionState::NeedSample
        );
        assert_eq!(
            zoom_only
                .observe(PositionSnapshot {
                    pan_tilt: Some(PanTiltPosition::new(i16::MAX, i16::MIN)),
                    zoom: Some(ZoomPosition::new(10).expect("zoom")),
                    focus: Some(FocusPosition::new(u16::MAX)),
                    iris: None,
                    nd_filter: None,
                })
                .expect("unselected changes are ignored"),
            MotionState::Settled
        );
    }

    #[test]
    fn iris_and_nd_position_plans_use_only_exact_supported_wires() {
        let fr7 =
            ProfileSpec::from_compile_time::<crate::profiles::SonyFR7>().expect("Sony FR7 profile");
        let axes = AffectedAxes::IRIS.union(AffectedAxes::ND_FILTER);
        let plan =
            prepare_position_queries(CameraId::CAMERA_2, &fr7, OperationalTuning::new(), axes)
                .expect("FR7 supports both scalar position inquiries");
        assert!(plan.pan_tilt.is_none());
        assert!(plan.zoom.is_none());
        assert!(plan.focus.is_none());
        assert_eq!(
            plan.iris
                .expect("iris inquiry")
                .instantiate()
                .wire
                .as_bytes(),
            &[
                CameraId::CAMERA_2.to_address_byte(),
                0x09,
                0x04,
                0x4B,
                VISCA_TERMINATOR,
            ]
        );
        assert_eq!(
            plan.nd_filter
                .expect("ND inquiry")
                .instantiate()
                .wire
                .as_bytes(),
            &[
                CameraId::CAMERA_2.to_address_byte(),
                0x09,
                0x04,
                0x64,
                VISCA_TERMINATOR,
            ]
        );

        let mixed = prepare_position_queries(
            CameraId::CAMERA_2,
            &fr7,
            OperationalTuning::new(),
            AffectedAxes::PAN_TILT.union(AffectedAxes::IRIS),
        )
        .expect("mixed plan");
        assert!(mixed.pan_tilt.is_some());
        assert!(mixed.iris.is_some());
        assert!(mixed.zoom.is_none());
        assert!(mixed.focus.is_none());
        assert!(mixed.nd_filter.is_none());
    }

    #[test]
    fn targeted_scalar_requests_lower_to_exact_profile_settlement_plans() {
        let fr7 =
            ProfileSpec::from_compile_time::<crate::profiles::SonyFR7>().expect("Sony FR7 profile");

        let iris = prepare_builtin_operation::<completion::Targeted, _>(
            &IrisReset,
            CameraId::CAMERA_2,
            &fr7,
            OperationalTuning::new(),
        )
        .expect("FR7 iris operation");
        let SettlementPlan::Poll {
            target,
            queries,
            axes,
            ..
        } = iris
            .settlement
            .into_plan()
            .expect("targeted iris settlement")
            .into_inner()
        else {
            panic!("FR7 must poll scalar positions without operation-complete support");
        };
        assert_eq!(target, CameraId::CAMERA_2);
        assert_eq!(axes, AffectedAxes::IRIS);
        assert!(queries.pan_tilt.is_none());
        assert!(queries.zoom.is_none());
        assert!(queries.focus.is_none());
        assert!(queries.iris.is_some());
        assert!(queries.nd_filter.is_none());

        let nd = prepare_builtin_operation::<completion::Targeted, _>(
            &NdFilterStepUp,
            CameraId::CAMERA_2,
            &fr7,
            OperationalTuning::new(),
        )
        .expect("FR7 ND operation");
        let SettlementPlan::Poll { queries, axes, .. } = nd
            .settlement
            .into_plan()
            .expect("targeted ND settlement")
            .into_inner()
        else {
            panic!("FR7 must poll ND position without operation-complete support");
        };
        assert_eq!(axes, AffectedAxes::ND_FILTER);
        assert!(queries.pan_tilt.is_none());
        assert!(queries.zoom.is_none());
        assert!(queries.focus.is_none());
        assert!(queries.iris.is_none());
        assert!(queries.nd_filter.is_some());

        let ptz =
            ProfileSpec::from_compile_time::<crate::profiles::PtzOpticsG2>().expect("PTZ profile");
        let completed = prepare_builtin_operation::<completion::Targeted, _>(
            &IrisReset,
            CameraId::CAMERA_2,
            &ptz,
            OperationalTuning::new(),
        )
        .expect("PTZ iris operation");
        assert!(matches!(
            completed
                .settlement
                .into_plan()
                .expect("PTZ settlement")
                .into_inner(),
            SettlementPlan::CompletionIsSettled { .. }
        ));
    }

    #[test]
    fn unsupported_nd_position_fails_preparation_before_any_write() {
        let generic = ProfileSpec::from_compile_time::<crate::profiles::GenericVisca>()
            .expect("generic profile");
        let error = prepare_position_queries(
            CameraId::CAMERA_1,
            &generic,
            OperationalTuning::new(),
            AffectedAxes::ND_FILTER,
        )
        .expect_err("generic profile has no ND position inquiry");
        assert!(matches!(error, Error::FeatureNotSupported { .. }));
    }

    #[test]
    fn scalar_position_detector_honors_iris_and_nd_tolerance() {
        let axes = AffectedAxes::IRIS.union(AffectedAxes::ND_FILTER);
        let mut detector = MotionDetector::new(axes, MovementTolerance::default());
        let first = PositionSnapshot {
            iris: Some(IrisLevel::new(4).expect("iris")),
            nd_filter: Some(NdFilterPosition::OneQuarter),
            ..PositionSnapshot::default()
        };
        assert_eq!(
            detector.observe(first).expect("baseline"),
            MotionState::NeedSample
        );
        let same = PositionSnapshot {
            iris: Some(IrisLevel::new(4).expect("iris")),
            nd_filter: Some(NdFilterPosition::OneQuarter),
            ..PositionSnapshot::default()
        };
        assert_eq!(
            detector.observe(same).expect("stable"),
            MotionState::Settled
        );
        let changed = PositionSnapshot {
            iris: Some(IrisLevel::new(5).expect("iris")),
            nd_filter: Some(NdFilterPosition::OneEighth),
            ..PositionSnapshot::default()
        };
        assert_eq!(
            detector.observe(changed).expect("moving"),
            MotionState::Moving
        );
    }

    #[test]
    fn motion_detector_rejects_an_incomplete_selected_snapshot_without_panicking() {
        let mut detector = MotionDetector::new(
            AffectedAxes::PAN_TILT.union(AffectedAxes::ZOOM),
            MovementTolerance::default(),
        );
        let error = detector
            .observe(PositionSnapshot {
                pan_tilt: Some(PanTiltPosition::new(0, 0)),
                zoom: None,
                focus: None,
                iris: None,
                nd_filter: None,
            })
            .expect_err("missing selected zoom must be rejected");
        assert!(matches!(error, Error::InvalidState(_)));
    }

    #[test]
    fn explicit_target_address_is_validated() {
        let profile = ProfileSpec::from_compile_time::<crate::profiles::GenericVisca>()
            .expect("built-in profile");
        assert!(prepare_command(
            &WrongAddress,
            CameraId::CAMERA_2,
            &profile,
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
            CameraId::CAMERA_1,
            &profile,
            OperationalTuning::new(),
            ClassSelection::Request,
        )
        .expect("custom inquiry preparation");
        assert!(custom.context.retry.inquiry_timeout);
        assert!(!custom.context.retry.builtin_inquiry_syntax);

        let builtin = prepare_builtin_inquiry(
            &PowerInquiry,
            CameraId::CAMERA_1,
            &profile,
            OperationalTuning::new(),
        )
        .expect("built-in inquiry preparation");
        assert!(builtin.context.retry.inquiry_timeout);
        assert!(builtin.context.retry.builtin_inquiry_syntax);
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
                    CameraId::CAMERA_1,
                    &profile,
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
                    CameraId::CAMERA_1,
                    &profile,
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
                    CameraId::CAMERA_1,
                    &profile,
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
                    CameraId::CAMERA_1,
                    &profile,
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
                    CameraId::CAMERA_1,
                    &profile,
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
            CameraId::CAMERA_1,
            &profile,
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
            CameraId::CAMERA_1,
            &profile,
            tuning,
            ClassSelection::Request,
        )
        .expect("inquiry preparation");
        assert_eq!(prepared.context.timeout.inquiry, Duration::from_secs(17));
    }

    #[test]
    fn preparation_rejects_timeout_classes_that_do_not_match_request_kind() {
        let profile = ProfileSpec::from_compile_time::<crate::profiles::PtzOpticsG2>()
            .expect("built-in profile");
        assert!(prepare_inquiry(
            &ClassifiedInquiry,
            CameraId::CAMERA_1,
            &profile,
            OperationalTuning::new(),
            ClassSelection::Request,
        )
        .is_err());
    }

    #[test]
    fn never_retry_class_ignores_retry_limit_tuning() {
        let tuning = OperationalTuning::new().retry_limit(7);
        let never = retry_policy(
            RetryClass::Never,
            TimeoutClass::Quick,
            tuning,
            Duration::ZERO,
            Duration::from_secs(5),
            false,
        );
        let standard = retry_policy(
            RetryClass::Standard,
            TimeoutClass::Movement,
            tuning,
            Duration::ZERO,
            Duration::from_secs(5),
            false,
        );

        assert_eq!(never.max_retries, 0);
        // `retry_limit` sets the base; a movement budget is the base itself.
        assert_eq!(standard.max_retries, 7);
    }

    /// Issue #566: the per-category retry budgets are 1.x's
    /// `RetryBudget::from_base`, not one flat number per retry class.
    #[test]
    fn retry_budgets_follow_the_1x_per_category_table() {
        let tuning = OperationalTuning::new();
        let budget = |timeout_class| {
            retry_policy(
                RetryClass::Standard,
                timeout_class,
                tuning,
                Duration::ZERO,
                Duration::from_secs(5),
                false,
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
                false,
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
                Duration::from_millis(25),
                Duration::from_millis(75),
                Duration::from_secs(2),
            ),
            Duration::ZERO,
            Duration::from_secs(1),
            false,
        );
        assert_eq!(tuned.initial_backoff, Duration::from_millis(25));
        assert_eq!(tuned.maximum_backoff, Duration::from_millis(75));
        assert_eq!(tuned.total_budget, Duration::from_secs(2));
    }

    /// A network budget never reaches zero, matching 1.x's clamp.
    #[test]
    fn a_network_budget_keeps_one_attempt_at_the_smallest_base() {
        for base in [0, 1, 2] {
            let policy = retry_policy(
                RetryClass::Standard,
                TimeoutClass::Network,
                OperationalTuning::new().retry_limit(base),
                Duration::ZERO,
                Duration::from_secs(5),
                false,
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
                false,
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
            false,
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
                false,
            )
            .movement_not_executable
        };

        assert!(policy(RetryClass::Movement));
        assert!(policy(RetryClass::Preset));
        assert!(!policy(RetryClass::Standard));
        assert!(!policy(RetryClass::Inquiry));
        assert!(!policy(RetryClass::Never));
    }

    /// Issue #566: the default retry budget is at least the 1.x ten-second
    /// budget and grows with the selected command deadline.
    #[test]
    fn retry_budget_follows_the_governing_response_deadline() {
        let profile = ProfileSpec::from_compile_time::<crate::profiles::GenericVisca>()
            .expect("built-in profile");
        let prepared = prepare_builtin_operation::<completion::AppliedOnly, _>(
            &crate::request::builtin::ZoomStop,
            CameraId::CAMERA_1,
            &profile,
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
            false,
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
            false,
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

        for profile in [&sony, &nearus] {
            let absolute = PanTiltAbsolute::for_profile(
                Degrees(45.0),
                Degrees(-15.0),
                pan_speed,
                tilt_speed,
                profile,
            )
            .expect("unsigned absolute");
            let relative = PanTiltRelative::for_profile(
                Degrees(45.0),
                Degrees(-15.0),
                pan_speed,
                tilt_speed,
                profile,
            )
            .expect("unsigned relative");
            let limit = PanTiltLimitSet::for_profile(
                PanTiltLimitCorner::UpRight,
                Degrees(45.0),
                Degrees(-15.0),
                profile,
            )
            .expect("unsigned limit");
            let prepared_absolute = prepare_builtin_operation::<completion::Targeted, _>(
                &absolute,
                CameraId::CAMERA_1,
                profile,
                OperationalTuning::new(),
            )
            .expect("prepared absolute");
            let prepared_relative = prepare_builtin_operation::<completion::Targeted, _>(
                &relative,
                CameraId::CAMERA_1,
                profile,
                OperationalTuning::new(),
            )
            .expect("prepared relative");
            let prepared_limit = prepare_builtin_command(
                &limit,
                CameraId::CAMERA_1,
                profile,
                OperationalTuning::new(),
            )
            .expect("prepared limit");

            assert_eq!(
                prepared_absolute.wire.as_bytes(),
                encoded(&PanTilt::AbsolutePositionRaw {
                    pan_u16: 0x8249,
                    tilt_u16: 0x7f3d,
                    pan_speed,
                    tilt_speed,
                })
            );
            assert_eq!(
                prepared_relative.wire.as_bytes(),
                encoded(&PanTilt::RelativePositionRaw {
                    pan_u16: 0x8249,
                    tilt_u16: 0x7f3d,
                    pan_speed,
                    tilt_speed,
                })
            );
            assert_eq!(
                prepared_limit.wire.as_bytes(),
                encoded(&PanTilt::LimitSetRaw {
                    corner: PanTiltLimitCorner::UpRight,
                    pan_u16: 0x8249,
                    tilt_u16: 0x7f3d,
                })
            );
            assert!(matches!(
                prepared_limit.applied_state,
                Some(AppliedStateProjection::Set {
                    key: crate::command::semantics::WriteOnlyState::PanTiltLimits,
                    value,
                }) if value.value_count == 3
            ));

            let inquiry = prepare_inquiry(
                &PanTiltPositionInquiry,
                CameraId::CAMERA_1,
                profile,
                OperationalTuning::new(),
                ClassSelection::Request,
            )
            .expect("prepared unsigned inquiry");
            assert_eq!(
                inquiry
                    .decoder
                    .decode(&[0x8, 0x2, 0x4, 0x9, 0x7, 0xf, 0x3, 0xd])
                    .expect("unsigned position decode"),
                crate::camera::PanTiltPosition::new(585, -195)
            );
        }

        let clear = PanTiltLimitClear::new(PanTiltLimitCorner::DownLeft);
        let prepared_clear = prepare_command(
            &clear,
            CameraId::CAMERA_1,
            &signed,
            OperationalTuning::new(),
            ClassSelection::Request,
        )
        .expect("prepared limit clear");
        assert!(matches!(
            prepared_clear.applied_state,
            Some(AppliedStateProjection::Clear {
                key: crate::command::semantics::WriteOnlyState::PanTiltLimits,
                ..
            })
        ));

        let absolute = PanTiltAbsolute::for_profile(
            Degrees(45.0),
            Degrees(-15.0),
            pan_speed,
            tilt_speed,
            &signed,
        )
        .expect("signed absolute");
        let prepared = prepare_builtin_operation::<completion::Targeted, _>(
            &absolute,
            CameraId::CAMERA_1,
            &signed,
            OperationalTuning::new(),
        )
        .expect("prepared signed absolute");
        assert_eq!(
            prepared.wire.as_bytes(),
            encoded(&PanTilt::AbsolutePositionRaw {
                pan_u16: 0x02d0,
                tilt_u16: 0xff10,
                pan_speed,
                tilt_speed,
            })
        );
        let inquiry = prepare_inquiry(
            &PanTiltPositionInquiry,
            CameraId::CAMERA_1,
            &signed,
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
    }

    #[test]
    fn every_homogeneous_builtin_family_validates_before_write() {
        let nearus = ProfileSpec::from_compile_time::<crate::profiles::NearusBRC300>()
            .expect("built-in profile");
        let generic = ProfileSpec::from_compile_time::<crate::profiles::GenericVisca>()
            .expect("generic built-in profile");
        let pan_tilt = PanTiltAbsolute::for_profile(
            Degrees(100.0),
            Degrees(0.0),
            PanSpeed::new(18).expect("conservative range"),
            TiltSpeed::new(17).expect("conservative range"),
            &generic,
        )
        .expect("valid for the source profile");
        reset_request_write_count();
        assert!(prepare_operation::<completion::Targeted, _>(
            &pan_tilt,
            CameraId::CAMERA_1,
            &nearus,
            OperationalTuning::new(),
            ClassSelection::Request,
        )
        .is_err());
        assert_eq!(request_write_count(), 0);

        let mut no_direct_caps = Capabilities::from_profile::<crate::profiles::GenericVisca>();
        no_direct_caps.supports_direct_zoom = false;
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
            CameraId::CAMERA_1,
            &no_direct,
            OperationalTuning::new(),
            ClassSelection::Request,
        )
        .is_err());
        assert_eq!(request_write_count(), 0);

        let mut bounded_zoom_caps = Capabilities::from_profile::<crate::profiles::GenericVisca>();
        bounded_zoom_caps.zoom_range_optical = 0..=0x4000;
        bounded_zoom_caps.has_digital_zoom = true;
        bounded_zoom_caps.zoom_range_digital = Some(0x4000..=0x5000);
        bounded_zoom_caps.supports_direct_zoom = true;
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
            CameraId::CAMERA_1,
            &bounded_zoom,
            OperationalTuning::new(),
        )
        .is_err());
        assert_eq!(request_write_count(), 0);

        reset_request_write_count();
        assert!(prepare_operation::<completion::AppliedOnly, _>(
            &FocusTrigger::Snap,
            CameraId::CAMERA_1,
            &no_direct,
            OperationalTuning::new(),
            ClassSelection::Request,
        )
        .is_err());
        assert_eq!(request_write_count(), 0);

        let preset = PresetSet::new(PresetNumber::new(255).expect("protocol preset"));
        reset_request_write_count();
        assert!(prepare_command(
            &preset,
            CameraId::CAMERA_1,
            &nearus,
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

        /// An applied-only operation classified urgent exactly as the crate's
        /// typed stops are.
        struct UrgentStop;

        impl Request for UrgentStop {
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

        impl OperationCommand<completion::AppliedOnly> for UrgentStop {
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
                CameraId::CAMERA_1,
                &profile(),
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
                CameraId::CAMERA_1,
                &profile(),
                OperationalTuning::new(),
                class,
            )
            .expect("inquiry prepares")
            .context
            .control
            .class
        }

        fn operation_class(class: ClassSelection) -> EngineControlClass {
            prepare_operation::<completion::AppliedOnly, _>(
                &UrgentStop,
                CameraId::CAMERA_1,
                &profile(),
                OperationalTuning::new(),
                class,
            )
            .expect("applied-only operation prepares")
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
                    ClassSelection::Explicit(SubmissionClass::Background).resolve(request),
                    expected,
                    "an explicit per-submission class preserves the safety floor",
                );
                assert_eq!(
                    ClassSelection::Handle(SubmissionClass::Background).resolve(request),
                    expected,
                    "a handle default never demotes an urgent request",
                );
            }
        }

        /// A handle default and a per-submission override both reach the
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
                submission_class(ClassSelection::Explicit(SubmissionClass::Normal)),
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
                inquiry_class(ClassSelection::Explicit(SubmissionClass::User)),
                EngineControlClass::User,
            );
        }

        /// The safety rule, on the lowered value rather than on `resolve`
        /// alone: neither public QoS route can demote an urgent stop.
        #[test]
        fn an_urgent_operation_is_preserved_by_every_submission_class() {
            assert_eq!(
                operation_class(ClassSelection::Request),
                EngineControlClass::Urgent
            );
            assert_eq!(
                operation_class(ClassSelection::Handle(SubmissionClass::Background)),
                EngineControlClass::Urgent,
                "issue #630: a handle default must never demote an urgent stop",
            );
            assert_eq!(
                operation_class(ClassSelection::Explicit(SubmissionClass::Background)),
                EngineControlClass::Urgent,
                "issue #542: per-submission QoS must never demote an urgent stop",
            );
        }
    }
}
