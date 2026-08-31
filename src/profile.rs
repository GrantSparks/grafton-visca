//! Validated runtime camera-profile specifications.

use std::time::{Duration, Instant};

use crate::{capabilities, timeout::CommandTimeouts, AffectedAxes, Error, Result};

/// Largest retry backoff ceiling or total retry budget an override may set.
///
/// Issue #636: the engine turns a request's total retry budget into a deadline
/// with `submitted_at.checked_add(budget)` and falls back to `submitted_at`
/// itself when that addition overflows. A budget near [`Duration::MAX`] would
/// therefore *invert* into its own opposite — the deadline meant to mean
/// "keep retrying for practically ever" lands in the past, the budget reads as
/// already spent, and the request gets zero retries instead of unbounded ones.
/// The same saturation turns an absurd backoff ceiling into no backoff at all.
///
/// One hour is orders of magnitude beyond any real VISCA retry window (the
/// built-in LongRunning command deadline is 300 seconds) and cannot overflow
/// an `Instant` on any supported platform, so the inversion is unreachable
/// without silently rewriting a caller's value.
const MAXIMUM_RETRY_TIMING: Duration = Duration::from_secs(60 * 60);

/// Returns whether adding a duration to the monotonic clock can be represented.
///
/// Runtime deadline construction must not use the engine's saturating fallback
/// for an unrepresentable duration: that fallback turns a future deadline into
/// the current instant and can make a request expire immediately. Profile and
/// operational-tuning validation therefore performs this check before any
/// request can be admitted or any protocol I/O can occur.
fn monotonic_duration_is_representable(now: Instant, duration: Duration) -> bool {
    now.checked_add(duration).is_some()
}

/// The default retry budget is at least twice the governing command/inquiry
/// deadline. Check that derived deadline too, rather than accepting an input
/// that is individually representable but overflows when the retry budget is
/// constructed.
fn retry_budget_is_representable(now: Instant, deadline: Duration) -> bool {
    deadline
        .checked_mul(2)
        .is_some_and(|budget| monotonic_duration_is_representable(now, budget))
}

/// Position inquiries available for profile-aware physical settlement.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
#[cfg_attr(feature = "ts-rs", derive(ts_rs::TS), ts(export))]
pub struct PositionInquirySupport {
    pan_tilt: bool,
    zoom: bool,
    focus: bool,
    iris: bool,
    nd_filter: bool,
}

impl PositionInquirySupport {
    /// Creates explicit per-axis inquiry support facts.
    #[must_use]
    pub const fn new(pan_tilt: bool, zoom: bool, focus: bool) -> Self {
        Self {
            pan_tilt,
            zoom,
            focus,
            iris: false,
            nd_filter: false,
        }
    }

    /// Creates explicit support facts for all five physical inquiry axes.
    #[must_use]
    pub const fn new_with_iris_nd(
        pan_tilt: bool,
        zoom: bool,
        focus: bool,
        iris: bool,
        nd_filter: bool,
    ) -> Self {
        Self {
            pan_tilt,
            zoom,
            focus,
            iris,
            nd_filter,
        }
    }

    /// Adds explicit iris inquiry support to an existing fact set.
    #[must_use]
    pub const fn with_iris(mut self, supported: bool) -> Self {
        self.iris = supported;
        self
    }

    /// Adds explicit ND-filter inquiry support to an existing fact set.
    #[must_use]
    pub const fn with_nd_filter(mut self, supported: bool) -> Self {
        self.nd_filter = supported;
        self
    }

    /// Returns whether pan/tilt position can be queried.
    #[must_use]
    pub const fn pan_tilt(self) -> bool {
        self.pan_tilt
    }

    /// Returns whether zoom position can be queried.
    #[must_use]
    pub const fn zoom(self) -> bool {
        self.zoom
    }

    /// Returns whether focus position can be queried.
    #[must_use]
    pub const fn focus(self) -> bool {
        self.focus
    }

    /// Returns whether iris position can be queried.
    #[must_use]
    pub const fn iris(self) -> bool {
        self.iris
    }

    /// Returns whether ND-filter position can be queried.
    #[must_use]
    pub const fn nd_filter(self) -> bool {
        self.nd_filter
    }

    /// Returns whether every selected axis has an exact position inquiry.
    #[must_use]
    pub const fn supports(self, axes: AffectedAxes) -> bool {
        (!axes.contains(AffectedAxes::PAN_TILT) || self.pan_tilt)
            && (!axes.contains(AffectedAxes::ZOOM) || self.zoom)
            && (!axes.contains(AffectedAxes::FOCUS) || self.focus)
            && (!axes.contains(AffectedAxes::IRIS) || self.iris)
            && (!axes.contains(AffectedAxes::ND_FILTER) || self.nd_filter)
    }
}

/// Wire envelope required by a camera profile.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
#[cfg_attr(feature = "ts-rs", derive(ts_rs::TS), ts(export))]
#[non_exhaustive]
pub enum ProfileEnvelope {
    /// Unencapsulated VISCA bytes.
    RawVisca,
    /// Sony VISCA-over-IP framing with sequence numbers.
    SonyEncapsulated,
}

/// Standard transports a profile permits, including their default ports.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
#[cfg_attr(feature = "ts-rs", derive(ts_rs::TS), ts(export))]
pub struct TransportCompatibility {
    tcp_port: Option<u16>,
    udp_port: Option<u16>,
    serial: bool,
}

impl TransportCompatibility {
    /// Creates transport facts. [`ProfileSpecBuilder::build`] validates them.
    #[must_use]
    pub const fn new(tcp_port: Option<u16>, udp_port: Option<u16>, serial: bool) -> Self {
        Self {
            tcp_port,
            udp_port,
            serial,
        }
    }

    /// Returns the default TCP port when TCP is supported.
    #[must_use]
    pub const fn tcp_port(self) -> Option<u16> {
        self.tcp_port
    }

    /// Returns the default UDP port when UDP is supported.
    #[must_use]
    pub const fn udp_port(self) -> Option<u16> {
        self.udp_port
    }

    /// Returns whether serial transport is supported.
    #[must_use]
    pub const fn supports_serial(self) -> bool {
        self.serial
    }
}

/// Immutable protocol timings owned by a validated profile.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
pub struct ProfileTiming {
    ack_timeout: Duration,
    command_timeouts: CommandTimeouts,
    inquiry_timeout: Duration,
    cancellation_timeout: Duration,
    ambiguity_timeout: Duration,
    busy_timeout: Duration,
    minimum_inquiry_spacing: Duration,
    minimum_command_spacing: Duration,
}

/// Builder for validated [`ProfileTiming`] facts.
///
/// Every timing fact is required.  The builder does not supply protocol
/// defaults because timing is part of a profile's safety contract; callers
/// should use [`CommandTimeouts::default`] explicitly when the standard
/// command-category deadlines are appropriate.  [`ProfileTiming`] itself is
/// accepted by [`ProfileSpecBuilder::timing`] as one cohesive value so a
/// profile cannot accidentally mix fields from different timing policies.
#[derive(Debug, Clone, Copy, Default)]
pub struct ProfileTimingBuilder {
    ack_timeout: Option<Duration>,
    command_timeouts: Option<CommandTimeouts>,
    inquiry_timeout: Option<Duration>,
    cancellation_timeout: Option<Duration>,
    ambiguity_timeout: Option<Duration>,
    busy_timeout: Option<Duration>,
    minimum_inquiry_spacing: Option<Duration>,
    minimum_command_spacing: Option<Duration>,
}

/// Profile-specified pan/tilt coordinate conversion owned by a validated profile.
#[derive(Debug, Clone, Copy, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
pub struct PanTiltCoordinateConversion {
    coordinate_system: capabilities::CoordinateSystem,
    #[cfg_attr(feature = "serde", serde(default))]
    wire_codec: capabilities::PanTiltWireCodec,
    pan_degrees_to_units: f32,
    tilt_degrees_to_units: f32,
}

impl PanTiltCoordinateConversion {
    /// Returns the camera's signed- or unsigned-centered wire convention.
    #[must_use]
    pub const fn coordinate_system(self) -> capabilities::CoordinateSystem {
        self.coordinate_system
    }

    /// Returns the profile-owned position-command and inquiry framing.
    #[must_use]
    pub const fn wire_codec(self) -> capabilities::PanTiltWireCodec {
        self.wire_codec
    }

    /// Returns the signed pan degree-to-unit scale.
    ///
    /// A negative scale represents a raw pan axis whose increasing values move
    /// left while the library's positive degree convention is right.
    #[must_use]
    pub const fn pan_degrees_to_units(self) -> f32 {
        self.pan_degrees_to_units
    }

    /// Returns the signed tilt degree-to-unit scale.
    ///
    /// A negative scale represents a raw tilt axis whose increasing values move
    /// up while the library's negative degree convention is up.
    #[must_use]
    pub const fn tilt_degrees_to_units(self) -> f32 {
        self.tilt_degrees_to_units
    }

    pub(crate) fn camera_coordinates(self, pan_degrees: f32, tilt_degrees: f32) -> (i32, i32) {
        let pan = (pan_degrees * self.pan_degrees_to_units).round() as i32;
        let tilt = (tilt_degrees * self.tilt_degrees_to_units).round() as i32;
        (pan, tilt)
    }
}

impl ProfileTiming {
    /// Starts a builder for explicit, validated profile timing facts.
    #[must_use]
    pub const fn builder() -> ProfileTimingBuilder {
        ProfileTimingBuilder::new()
    }

    /// Returns the acknowledgement deadline.
    #[must_use]
    pub const fn ack_timeout(self) -> Duration {
        self.ack_timeout
    }

    /// Returns the validated per-category command deadlines.
    #[must_use]
    pub const fn command_timeouts(self) -> CommandTimeouts {
        self.command_timeouts
    }

    /// Returns the inquiry-response deadline.
    #[must_use]
    pub const fn inquiry_timeout(self) -> Duration {
        self.inquiry_timeout
    }

    /// Returns the socket-cancellation response deadline.
    #[must_use]
    pub const fn cancellation_timeout(self) -> Duration {
        self.cancellation_timeout
    }

    /// Returns the pre-ack cancellation ambiguity deadline.
    #[must_use]
    pub const fn ambiguity_timeout(self) -> Duration {
        self.ambiguity_timeout
    }

    /// Returns the camera-busy recovery deadline.
    #[must_use]
    pub const fn busy_timeout(self) -> Duration {
        self.busy_timeout
    }

    /// Returns the minimum interval between inquiry writes.
    #[must_use]
    pub const fn minimum_inquiry_spacing(self) -> Duration {
        self.minimum_inquiry_spacing
    }

    /// Returns the minimum interval between command writes.
    #[must_use]
    pub const fn minimum_command_spacing(self) -> Duration {
        self.minimum_command_spacing
    }

    fn validate(self) -> Result<Self> {
        if self.ack_timeout.is_zero()
            || self.inquiry_timeout.is_zero()
            || self.cancellation_timeout.is_zero()
            || self.ambiguity_timeout.is_zero()
        {
            return Err(Error::InvalidRequest(
                "all profile protocol timeouts must be non-zero".into(),
            ));
        }
        self.command_timeouts.validate()?;

        let now = Instant::now();
        let command_timeouts = self.command_timeouts;
        let timing_values = [
            self.ack_timeout,
            command_timeouts.quick_timeout(),
            command_timeouts.movement_timeout(),
            command_timeouts.preset_timeout(),
            command_timeouts.long_running_timeout(),
            command_timeouts.network_timeout(),
            self.inquiry_timeout,
            self.cancellation_timeout,
            self.ambiguity_timeout,
            self.busy_timeout,
            self.minimum_inquiry_spacing,
            self.minimum_command_spacing,
        ];
        if timing_values
            .into_iter()
            .any(|duration| !monotonic_duration_is_representable(now, duration))
        {
            return Err(Error::InvalidRequest(
                "profile timing values must be representable by the monotonic clock".into(),
            ));
        }

        // `retry_policy` derives its default total budget as at least twice
        // the governing command or inquiry deadline.  Validate those derived
        // values here as well as the stored timing facts.
        if [
            command_timeouts.quick_timeout(),
            command_timeouts.movement_timeout(),
            command_timeouts.preset_timeout(),
            command_timeouts.long_running_timeout(),
            command_timeouts.network_timeout(),
            self.inquiry_timeout,
        ]
        .into_iter()
        .any(|deadline| !retry_budget_is_representable(now, deadline))
        {
            return Err(Error::InvalidRequest(
                "profile retry deadlines must be representable by the monotonic clock".into(),
            ));
        }
        Ok(self)
    }
}

impl ProfileTimingBuilder {
    /// Creates an empty builder.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            ack_timeout: None,
            command_timeouts: None,
            inquiry_timeout: None,
            cancellation_timeout: None,
            ambiguity_timeout: None,
            busy_timeout: None,
            minimum_inquiry_spacing: None,
            minimum_command_spacing: None,
        }
    }

    /// Sets the acknowledgement deadline.
    #[must_use]
    pub const fn ack_timeout(mut self, timeout: Duration) -> Self {
        self.ack_timeout = Some(timeout);
        self
    }

    /// Sets the complete per-category command deadline table.
    #[must_use]
    pub const fn command_timeouts(mut self, timeouts: CommandTimeouts) -> Self {
        self.command_timeouts = Some(timeouts);
        self
    }

    /// Sets the inquiry-response deadline.
    #[must_use]
    pub const fn inquiry_timeout(mut self, timeout: Duration) -> Self {
        self.inquiry_timeout = Some(timeout);
        self
    }

    /// Sets the socket-cancellation response deadline.
    #[must_use]
    pub const fn cancellation_timeout(mut self, timeout: Duration) -> Self {
        self.cancellation_timeout = Some(timeout);
        self
    }

    /// Sets the pre-ack cancellation ambiguity deadline.
    #[must_use]
    pub const fn ambiguity_timeout(mut self, timeout: Duration) -> Self {
        self.ambiguity_timeout = Some(timeout);
        self
    }

    /// Sets the camera-busy recovery deadline.
    #[must_use]
    pub const fn busy_timeout(mut self, timeout: Duration) -> Self {
        self.busy_timeout = Some(timeout);
        self
    }

    /// Sets the minimum interval between inquiry writes.
    #[must_use]
    pub const fn minimum_inquiry_spacing(mut self, spacing: Duration) -> Self {
        self.minimum_inquiry_spacing = Some(spacing);
        self
    }

    /// Sets the minimum interval between command writes.
    #[must_use]
    pub const fn minimum_command_spacing(mut self, spacing: Duration) -> Self {
        self.minimum_command_spacing = Some(spacing);
        self
    }

    /// Validates and returns immutable profile timing facts.
    pub fn build(self) -> Result<ProfileTiming> {
        fn required<T>(value: Option<T>, name: &'static str) -> Result<T> {
            value.ok_or_else(|| Error::InvalidRequest(name.into()))
        }

        ProfileTiming {
            ack_timeout: required(
                self.ack_timeout,
                "profile acknowledgement timeout is required",
            )?,
            command_timeouts: required(
                self.command_timeouts,
                "profile command timeout table is required",
            )?,
            inquiry_timeout: required(self.inquiry_timeout, "profile inquiry timeout is required")?,
            cancellation_timeout: required(
                self.cancellation_timeout,
                "profile cancellation timeout is required",
            )?,
            ambiguity_timeout: required(
                self.ambiguity_timeout,
                "profile ambiguity timeout is required",
            )?,
            busy_timeout: required(self.busy_timeout, "profile busy timeout is required")?,
            minimum_inquiry_spacing: required(
                self.minimum_inquiry_spacing,
                "profile minimum inquiry spacing is required",
            )?,
            minimum_command_spacing: required(
                self.minimum_command_spacing,
                "profile minimum command spacing is required",
            )?,
        }
        .validate()
    }
}

/// Operational overrides that cannot change request or profile semantics.
///
/// Tuning contains no request class, route, decoder, cancellation capability,
/// or completion-kind fields. Every value is validated against a profile before
/// it is lowered.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
pub struct OperationalTuning {
    command_spacing: Option<Duration>,
    inquiry_spacing: Option<Duration>,
    maximum_command_sockets: Option<u8>,
    ack_timeout: Option<Duration>,
    quick_timeout: Option<Duration>,
    movement_timeout: Option<Duration>,
    preset_timeout: Option<Duration>,
    long_running_timeout: Option<Duration>,
    network_timeout: Option<Duration>,
    settlement_timeout: Option<Duration>,
    inquiry_timeout: Option<Duration>,
    retry_limit: Option<u32>,
    initial_retry_backoff: Option<Duration>,
    maximum_retry_backoff: Option<Duration>,
    retry_budget: Option<Duration>,
    strict_unconfirmed_poison: Option<bool>,
}

impl OperationalTuning {
    /// Creates tuning with no overrides.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            command_spacing: None,
            inquiry_spacing: None,
            maximum_command_sockets: None,
            ack_timeout: None,
            quick_timeout: None,
            movement_timeout: None,
            preset_timeout: None,
            long_running_timeout: None,
            network_timeout: None,
            settlement_timeout: None,
            inquiry_timeout: None,
            retry_limit: None,
            initial_retry_backoff: None,
            maximum_retry_backoff: None,
            retry_budget: None,
            strict_unconfirmed_poison: None,
        }
    }

    /// Overrides command pacing, subject to the profile minimum.
    #[must_use]
    pub const fn command_spacing(mut self, spacing: Duration) -> Self {
        self.command_spacing = Some(spacing);
        self
    }

    /// Overrides inquiry pacing, subject to the profile minimum.
    #[must_use]
    pub const fn inquiry_spacing(mut self, spacing: Duration) -> Self {
        self.inquiry_spacing = Some(spacing);
        self
    }

    /// Lowers owner socket capacity without exceeding the profile limit.
    #[must_use]
    pub const fn maximum_command_sockets(mut self, maximum: u8) -> Self {
        self.maximum_command_sockets = Some(maximum);
        self
    }

    /// Overrides the non-zero acknowledgement deadline.
    #[must_use]
    pub const fn ack_timeout(mut self, timeout: Duration) -> Self {
        self.ack_timeout = Some(timeout);
        self
    }

    /// Overrides the non-zero quick-command completion deadline.
    ///
    /// Category overrides are independent; set each category that needs a
    /// different deadline.
    #[must_use]
    pub const fn quick_timeout(mut self, timeout: Duration) -> Self {
        self.quick_timeout = Some(timeout);
        self
    }

    /// Overrides the non-zero movement-command completion deadline.
    ///
    #[must_use]
    pub const fn movement_timeout(mut self, timeout: Duration) -> Self {
        self.movement_timeout = Some(timeout);
        self
    }

    /// Overrides the non-zero preset-command completion deadline.
    ///
    #[must_use]
    pub const fn preset_timeout(mut self, timeout: Duration) -> Self {
        self.preset_timeout = Some(timeout);
        self
    }

    /// Overrides the non-zero long-running-command completion deadline.
    ///
    #[must_use]
    pub const fn long_running_timeout(mut self, timeout: Duration) -> Self {
        self.long_running_timeout = Some(timeout);
        self
    }

    /// Overrides the non-zero network-command completion deadline.
    ///
    #[must_use]
    pub const fn network_timeout(mut self, timeout: Duration) -> Self {
        self.network_timeout = Some(timeout);
        self
    }

    /// Overrides the complete physical-settlement budget for targeted
    /// operations. This is independent of the protocol response deadline.
    #[must_use]
    pub const fn settlement_timeout(mut self, timeout: Duration) -> Self {
        self.settlement_timeout = Some(timeout);
        self
    }

    /// Overrides the non-zero inquiry deadline.
    #[must_use]
    pub const fn inquiry_timeout(mut self, timeout: Duration) -> Self {
        self.inquiry_timeout = Some(timeout);
        self
    }

    /// Overrides the base bounded retry count.
    ///
    /// Each request's own budget is derived from this base by its timeout
    /// category: quick and inquiry work gets two more attempts, network work
    /// one fewer, and a long-running command exactly one, so the effective
    /// count is not always the number given here. A request whose retry class
    /// is [`RetryClass::Never`](crate::RetryClass::Never) is never replayed
    /// regardless of this value.
    #[must_use]
    pub const fn retry_limit(mut self, maximum: u32) -> Self {
        self.retry_limit = Some(maximum);
        self
    }

    /// Overrides retry backoff and total budget.
    #[must_use]
    pub const fn retry_timing(
        mut self,
        initial: Duration,
        maximum: Duration,
        budget: Duration,
    ) -> Self {
        self.initial_retry_backoff = Some(initial);
        self.maximum_retry_backoff = Some(maximum);
        self.retry_budget = Some(budget);
        self
    }

    /// Selects strict whole-session poisoning for unconfirmable raw commands.
    ///
    /// The default (`false`) fails only the affected raw command with
    /// [`Error::UnsequencedCommandUnconfirmed`]
    /// and quarantines its socket or its unacknowledged-command slot for the
    /// ambiguity window, so a late ACK or completion cannot bind to a later
    /// command while the session and every unrelated request keep running.
    /// Setting `true` restores the conservative behavior in which any such
    /// event — a lost ACK/completion, a spent retry budget, or an expired
    /// cancellation-ambiguity window — poisons the whole session, for
    /// deployments that would rather hard-fail an entire session than risk a
    /// subtle correlation error. The flag has no effect on the Sony envelope,
    /// whose sequence correlation never needs the quarantine. Like the wire
    /// envelope, it is applied when the session is built and is not changed by a
    /// later runtime reconfiguration.
    #[must_use]
    pub const fn strict_unconfirmed_poison(mut self, enabled: bool) -> Self {
        self.strict_unconfirmed_poison = Some(enabled);
        self
    }

    pub(crate) const fn command_spacing_override(self) -> Option<Duration> {
        self.command_spacing
    }

    pub(crate) const fn inquiry_spacing_override(self) -> Option<Duration> {
        self.inquiry_spacing
    }

    pub(crate) const fn maximum_command_sockets_override(self) -> Option<u8> {
        self.maximum_command_sockets
    }

    pub(crate) const fn ack_timeout_override(self) -> Option<Duration> {
        self.ack_timeout
    }

    pub(crate) const fn quick_timeout_override(self) -> Option<Duration> {
        self.quick_timeout
    }

    pub(crate) const fn movement_timeout_override(self) -> Option<Duration> {
        self.movement_timeout
    }

    pub(crate) const fn preset_timeout_override(self) -> Option<Duration> {
        self.preset_timeout
    }

    pub(crate) const fn long_running_timeout_override(self) -> Option<Duration> {
        self.long_running_timeout
    }

    pub(crate) const fn network_timeout_override(self) -> Option<Duration> {
        self.network_timeout
    }

    pub(crate) const fn settlement_timeout_override(self) -> Option<Duration> {
        self.settlement_timeout
    }

    pub(crate) const fn inquiry_timeout_override(self) -> Option<Duration> {
        self.inquiry_timeout
    }

    pub(crate) const fn retry_limit_override(self) -> Option<u32> {
        self.retry_limit
    }

    pub(crate) const fn retry_timing_override(
        self,
    ) -> (Option<Duration>, Option<Duration>, Option<Duration>) {
        (
            self.initial_retry_backoff,
            self.maximum_retry_backoff,
            self.retry_budget,
        )
    }

    pub(crate) const fn strict_unconfirmed_poison_override(self) -> Option<bool> {
        self.strict_unconfirmed_poison
    }
}

/// Validated runtime form of compile-time and user-supplied profile facts.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(
    feature = "serde",
    serde(try_from = "ProfileSpecSerde", into = "ProfileSpecSerde")
)]
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
pub struct ProfileSpec {
    capabilities: capabilities::Capabilities,
    pan_tilt_coordinates: Option<PanTiltCoordinateConversion>,
    transports: TransportCompatibility,
    envelope: ProfileEnvelope,
    timing: ProfileTiming,
    maximum_command_sockets: u8,
    supports_operation_complete: bool,
    supports_command_cancel: bool,
    preset_recall_axes: Option<AffectedAxes>,
    position_inquiries: PositionInquirySupport,
}

#[cfg(feature = "serde")]
#[derive(serde::Serialize, serde::Deserialize)]
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
struct ProfileSpecSerde {
    capabilities: capabilities::Capabilities,
    pan_tilt_coordinates: Option<PanTiltCoordinateConversion>,
    transports: TransportCompatibility,
    envelope: ProfileEnvelope,
    timing: ProfileTiming,
    maximum_command_sockets: u8,
    supports_operation_complete: bool,
    supports_command_cancel: bool,
    preset_recall_axes: Option<AffectedAxes>,
    position_inquiries: PositionInquirySupport,
}

#[cfg(feature = "serde")]
impl From<ProfileSpec> for ProfileSpecSerde {
    fn from(spec: ProfileSpec) -> Self {
        Self {
            capabilities: spec.capabilities,
            pan_tilt_coordinates: spec.pan_tilt_coordinates,
            transports: spec.transports,
            envelope: spec.envelope,
            timing: spec.timing,
            maximum_command_sockets: spec.maximum_command_sockets,
            supports_operation_complete: spec.supports_operation_complete,
            supports_command_cancel: spec.supports_command_cancel,
            preset_recall_axes: spec.preset_recall_axes,
            position_inquiries: spec.position_inquiries,
        }
    }
}

#[cfg(feature = "serde")]
impl TryFrom<ProfileSpecSerde> for ProfileSpec {
    type Error = Error;

    fn try_from(spec: ProfileSpecSerde) -> Result<Self> {
        Self {
            capabilities: spec.capabilities,
            pan_tilt_coordinates: spec.pan_tilt_coordinates,
            transports: spec.transports,
            envelope: spec.envelope,
            timing: spec.timing,
            maximum_command_sockets: spec.maximum_command_sockets,
            supports_operation_complete: spec.supports_operation_complete,
            supports_command_cancel: spec.supports_command_cancel,
            preset_recall_axes: spec.preset_recall_axes,
            position_inquiries: spec.position_inquiries,
        }
        .validate()
    }
}

impl ProfileSpec {
    /// Starts a runtime builder from a complete capability inventory.
    ///
    /// Protocol safety facts have no defaults and must be supplied explicitly
    /// before [`ProfileSpecBuilder::build`] succeeds.
    #[must_use]
    pub fn builder(capabilities: capabilities::Capabilities) -> ProfileSpecBuilder {
        ProfileSpecBuilder::runtime(capabilities)
    }

    /// Builds the runtime spec emitted by a compile-time profile.
    pub fn from_compile_time<P>() -> Result<Self>
    where
        P: CompileTimeProfile,
    {
        ProfileSpecBuilder::from_compile_time::<P>().build()
    }

    /// Checks that this validated runtime inventory is exactly the inventory
    /// lowered from a compile-time profile.
    ///
    /// Static capability bounds are only sound when every protocol fact agrees,
    /// not merely the profile identifier or broad capability bits.  Keep this
    /// comparison pure so session projection can reject a mismatched view
    /// before owner admission or protocol I/O.
    #[cfg(any(feature = "async", feature = "blocking", test))]
    pub(crate) fn ensure_compile_time<P>(&self) -> Result<()>
    where
        P: CompileTimeProfile,
    {
        let expected = Self::from_compile_time::<P>()?;
        if self == &expected {
            Ok(())
        } else {
            Err(Error::InvalidRequest(
                "registered runtime profile does not match the requested compile-time profile"
                    .into(),
            ))
        }
    }

    /// Returns whether every profile fact matches the generated compile-time
    /// projection for `P`.
    ///
    /// This deliberately compares the unvalidated builder projection. It is
    /// used while validating a `ProfileSpec`, so constructing the validated
    /// projection here would recurse back into this check.
    pub(crate) fn matches_compile_time_profile<P>(&self) -> bool
    where
        P: CompileTimeProfile,
    {
        let expected = ProfileSpecBuilder::from_compile_time::<P>();
        self.capabilities == expected.capabilities
            && self.pan_tilt_coordinates == expected.pan_tilt_coordinates
            && Some(self.transports) == expected.transports
            && Some(self.envelope) == expected.envelope
            && Some(self.timing) == expected.timing
            && Some(self.maximum_command_sockets) == expected.maximum_command_sockets
            && Some(self.supports_operation_complete) == expected.supports_operation_complete
            && Some(self.supports_command_cancel) == expected.supports_command_cancel
            && Some(self.preset_recall_axes) == expected.preset_recall_axes
            && Some(self.position_inquiries) == expected.position_inquiries
    }

    /// Returns runtime feature and conversion facts.
    #[must_use]
    pub const fn capabilities(&self) -> &capabilities::Capabilities {
        &self.capabilities
    }

    /// Returns profile-specified pan/tilt coordinate facts when pan/tilt is supported.
    #[must_use]
    pub const fn pan_tilt_coordinates(&self) -> Option<PanTiltCoordinateConversion> {
        self.pan_tilt_coordinates
    }

    pub(crate) fn convert_pan_tilt_degrees(
        &self,
        pan_degrees: f32,
        tilt_degrees: f32,
    ) -> Result<(i32, i32)> {
        if !pan_degrees.is_finite()
            || !tilt_degrees.is_finite()
            || !self.capabilities.pan_range_degrees.contains(&pan_degrees)
            || !self.capabilities.tilt_range_degrees.contains(&tilt_degrees)
        {
            return Err(Error::InvalidRequest(
                "pan/tilt degrees are outside the validated profile range".into(),
            ));
        }
        let conversion = self.pan_tilt_coordinates.ok_or_else(|| {
            Error::InvalidRequest("profile has no pan/tilt coordinate conversion".into())
        })?;
        let converted = conversion.camera_coordinates(pan_degrees, tilt_degrees);
        if !self.capabilities.pan_range.contains(&converted.0)
            || !self.capabilities.tilt_range.contains(&converted.1)
        {
            return Err(Error::InvalidRequest(
                "converted pan/tilt units are outside the validated profile range".into(),
            ));
        }
        Ok(converted)
    }

    /// Returns compatible standard transports.
    #[must_use]
    pub const fn transports(&self) -> TransportCompatibility {
        self.transports
    }

    /// Returns the required wire envelope.
    #[must_use]
    pub const fn envelope(&self) -> ProfileEnvelope {
        self.envelope
    }

    /// Returns immutable deadline and pacing minima.
    #[must_use]
    pub const fn timing(&self) -> ProfileTiming {
        self.timing
    }

    /// Returns the validated per-category command completion deadlines.
    #[must_use]
    pub const fn command_timeouts(&self) -> CommandTimeouts {
        self.timing.command_timeouts()
    }

    /// Returns the maximum number of camera command sockets.
    #[must_use]
    pub const fn maximum_command_sockets(&self) -> u8 {
        self.maximum_command_sockets
    }

    /// Returns whether exact operation-complete messages are supported.
    #[must_use]
    pub const fn supports_operation_complete(&self) -> bool {
        self.supports_operation_complete
    }

    /// Returns whether transmitted commands permit protocol cancellation.
    #[must_use]
    pub const fn supports_command_cancel(&self) -> bool {
        self.supports_command_cancel
    }

    /// Returns the exact axes affected by preset recall.
    #[must_use]
    pub const fn preset_recall_axes(&self) -> Option<AffectedAxes> {
        self.preset_recall_axes
    }

    /// Returns per-axis position-inquiry support used for settlement.
    #[must_use]
    pub const fn position_inquiries(&self) -> PositionInquirySupport {
        self.position_inquiries
    }

    /// Returns whether the profile declares every selected physical axis.
    ///
    /// This checks the capability inventory, not position-inquiry availability;
    /// callers that need to poll must additionally use
    /// [`PositionInquirySupport::supports`].
    #[must_use]
    pub const fn supports_axes(&self, axes: AffectedAxes) -> bool {
        (!axes.contains(AffectedAxes::PAN_TILT) || self.capabilities.has_pan_tilt)
            && (!axes.contains(AffectedAxes::ZOOM) || self.capabilities.has_zoom)
            && (!axes.contains(AffectedAxes::FOCUS) || self.capabilities.has_focus)
            && (!axes.contains(AffectedAxes::IRIS) || self.capabilities.has_iris_control)
            && (!axes.contains(AffectedAxes::ND_FILTER) || self.capabilities.has_nd_filter)
    }

    /// Validates operational overrides without changing profile safety facts.
    pub fn validate_tuning(&self, tuning: OperationalTuning) -> Result<()> {
        let now = Instant::now();
        let tuning_values = [
            tuning.command_spacing,
            tuning.inquiry_spacing,
            tuning.ack_timeout,
            tuning.quick_timeout,
            tuning.movement_timeout,
            tuning.preset_timeout,
            tuning.long_running_timeout,
            tuning.network_timeout,
            tuning.settlement_timeout,
            tuning.inquiry_timeout,
            tuning.initial_retry_backoff,
            tuning.maximum_retry_backoff,
            tuning.retry_budget,
        ];
        if tuning_values
            .into_iter()
            .flatten()
            .any(|duration| !monotonic_duration_is_representable(now, duration))
        {
            return Err(Error::InvalidRequest(
                "operational timing overrides must be representable by the monotonic clock".into(),
            ));
        }

        // Command and inquiry overrides feed the same derived retry-budget
        // calculation as their profile-owned counterparts.
        if [
            tuning.quick_timeout,
            tuning.movement_timeout,
            tuning.preset_timeout,
            tuning.long_running_timeout,
            tuning.network_timeout,
            tuning.inquiry_timeout,
        ]
        .into_iter()
        .flatten()
        .any(|deadline| !retry_budget_is_representable(now, deadline))
        {
            return Err(Error::InvalidRequest(
                "operational retry deadlines must be representable by the monotonic clock".into(),
            ));
        }

        if tuning
            .command_spacing
            .is_some_and(|value| value < self.timing.minimum_command_spacing)
            || tuning
                .inquiry_spacing
                .is_some_and(|value| value < self.timing.minimum_inquiry_spacing)
        {
            return Err(Error::InvalidRequest(
                "operational tuning cannot weaken profile pacing minima".into(),
            ));
        }
        if tuning
            .maximum_command_sockets
            .is_some_and(|value| value == 0 || value > self.maximum_command_sockets)
        {
            return Err(Error::InvalidRequest(
                "operational tuning cannot raise the profile socket limit".into(),
            ));
        }
        if [
            tuning.ack_timeout,
            tuning.quick_timeout,
            tuning.movement_timeout,
            tuning.preset_timeout,
            tuning.long_running_timeout,
            tuning.network_timeout,
            tuning.settlement_timeout,
            tuning.inquiry_timeout,
        ]
        .into_iter()
        .flatten()
        .any(|timeout| timeout.is_zero())
        {
            return Err(Error::InvalidRequest(
                "operational timeout overrides must be non-zero".into(),
            ));
        }
        if tuning
            .ack_timeout
            .is_some_and(|timeout| timeout < self.timing.ack_timeout)
            || tuning
                .quick_timeout
                .is_some_and(|timeout| timeout < self.timing.command_timeouts.quick_timeout())
            || tuning
                .movement_timeout
                .is_some_and(|timeout| timeout < self.timing.command_timeouts.movement_timeout())
            || tuning
                .preset_timeout
                .is_some_and(|timeout| timeout < self.timing.command_timeouts.preset_timeout())
            || tuning.long_running_timeout.is_some_and(|timeout| {
                timeout < self.timing.command_timeouts.long_running_timeout()
            })
            || tuning
                .network_timeout
                .is_some_and(|timeout| timeout < self.timing.command_timeouts.network_timeout())
            || tuning
                .inquiry_timeout
                .is_some_and(|timeout| timeout < self.timing.inquiry_timeout)
        {
            return Err(Error::InvalidRequest(
                "operational timeout overrides cannot undercut profile deadlines".into(),
            ));
        }
        if tuning.retry_limit.is_some_and(|limit| limit > 32) {
            return Err(Error::InvalidRequest(
                "operational retry limit exceeds the bounded maximum of 32".into(),
            ));
        }
        match (
            tuning.initial_retry_backoff,
            tuning.maximum_retry_backoff,
            tuning.retry_budget,
        ) {
            (None, None, None) => {}
            (Some(initial), Some(maximum), Some(budget))
                if !initial.is_zero() && maximum >= initial && budget >= initial => {}
            _ => {
                return Err(Error::InvalidRequest(
                    "retry timing requires non-zero ordered backoff and a sufficient budget".into(),
                ));
            }
        }
        // The ordering rule above keeps `initial` below both of these, so
        // bounding the ceiling and the budget bounds all three (see
        // [`MAXIMUM_RETRY_TIMING`]).
        if tuning
            .maximum_retry_backoff
            .is_some_and(|maximum| maximum > MAXIMUM_RETRY_TIMING)
            || tuning
                .retry_budget
                .is_some_and(|budget| budget > MAXIMUM_RETRY_TIMING)
        {
            return Err(Error::InvalidRequest(
                "retry backoff ceiling and budget cannot exceed one hour".into(),
            ));
        }
        Ok(())
    }

    fn validate(mut self) -> Result<Self> {
        fn range_ordered<T: PartialOrd>(range: &std::ops::RangeInclusive<T>) -> bool {
            range.start() <= range.end()
        }

        fn optional_ordered<T: PartialOrd>(range: Option<&std::ops::RangeInclusive<T>>) -> bool {
            range.is_none_or(range_ordered)
        }

        if self.transports.tcp_port == Some(0) || self.transports.udp_port == Some(0) {
            return Err(Error::InvalidRequest(
                "profile default transport ports must be non-zero".into(),
            ));
        }
        if self.transports.tcp_port.is_none()
            && self.transports.udp_port.is_none()
            && !self.transports.serial
        {
            return Err(Error::InvalidRequest(
                "profile must support at least one standard transport".into(),
            ));
        }
        if self.envelope == ProfileEnvelope::SonyEncapsulated
            && (self.transports.serial
                || (self.transports.tcp_port.is_none() && self.transports.udp_port.is_none()))
        {
            return Err(Error::InvalidRequest(
                "Sony encapsulation requires an IP transport and is incompatible with serial"
                    .into(),
            ));
        }
        self.timing = self.timing.validate()?;
        if !(1..=2).contains(&self.maximum_command_sockets) {
            return Err(Error::InvalidRequest(
                "profile command socket limit must be one or two".into(),
            ));
        }
        crate::CameraId::new(self.capabilities.default_camera_id)?;
        if self.capabilities.model_name.trim().is_empty() {
            return Err(Error::InvalidRequest(
                "profile model name must not be empty".into(),
            ));
        }
        if let Some(profile_id) = self.capabilities.profile_id {
            if !profile_id.matches_profile_spec(&self) {
                return Err(Error::InvalidRequest(
                    "built-in profile identity does not match runtime profile facts".into(),
                ));
            }
        }
        let ordered = |start: f64, end: f64| start.is_finite() && end.is_finite() && start <= end;
        if self.capabilities.has_pan_tilt
            && (!ordered(
                f64::from(*self.capabilities.pan_range.start()),
                f64::from(*self.capabilities.pan_range.end()),
            ) || !ordered(
                f64::from(*self.capabilities.tilt_range.start()),
                f64::from(*self.capabilities.tilt_range.end()),
            ) || !ordered(
                f64::from(*self.capabilities.pan_range_degrees.start()),
                f64::from(*self.capabilities.pan_range_degrees.end()),
            ) || !ordered(
                f64::from(*self.capabilities.tilt_range_degrees.start()),
                f64::from(*self.capabilities.tilt_range_degrees.end()),
            ) || *self.capabilities.pan_speed.start() == 0
                || self.capabilities.pan_speed.start() > self.capabilities.pan_speed.end()
                || *self.capabilities.tilt_speed.start() == 0
                || self.capabilities.tilt_speed.start() > self.capabilities.tilt_speed.end()
                || self.pan_tilt_coordinates.is_none_or(|conversion| {
                    !conversion.pan_degrees_to_units.is_finite()
                        || conversion.pan_degrees_to_units == 0.0
                        || !conversion.tilt_degrees_to_units.is_finite()
                        || conversion.tilt_degrees_to_units == 0.0
                }))
        {
            return Err(Error::InvalidRequest(
                "profile pan/tilt ranges, converters, and speeds are invalid".into(),
            ));
        }
        if self.capabilities.has_pan_tilt {
            let Some(conversion) = self.pan_tilt_coordinates else {
                return Err(Error::InvalidRequest(
                    "pan/tilt coordinate conversion is required".into(),
                ));
            };
            if conversion.wire_codec == capabilities::PanTiltWireCodec::SonyBrc300
                && conversion.coordinate_system != capabilities::CoordinateSystem::SignedCentered
            {
                return Err(Error::InvalidRequest(
                    "Sony BRC-300 pan/tilt framing requires signed-centered coordinates".into(),
                ));
            }
            if conversion.wire_codec == capabilities::PanTiltWireCodec::SonyBrc300
                && (!(-0x080000..=0x07_FFFF).contains(self.capabilities.pan_range.start())
                    || !(-0x080000..=0x07_FFFF).contains(self.capabilities.pan_range.end())
                    || !(-0x8000..=0x7FFF).contains(self.capabilities.tilt_range.start())
                    || !(-0x8000..=0x7FFF).contains(self.capabilities.tilt_range.end()))
            {
                return Err(Error::InvalidRequest(
                    "Sony BRC-300 pan/tilt ranges must fit signed 20-bit pan and signed 16-bit tilt framing".into(),
                ));
            }
            if conversion.wire_codec == capabilities::PanTiltWireCodec::StandardVisca
                && (!(-0x8000..=0x7FFF).contains(self.capabilities.pan_range.start())
                    || !(-0x8000..=0x7FFF).contains(self.capabilities.pan_range.end())
                    || !(-0x8000..=0x7FFF).contains(self.capabilities.tilt_range.start())
                    || !(-0x8000..=0x7FFF).contains(self.capabilities.tilt_range.end()))
            {
                return Err(Error::InvalidRequest(
                    "standard VISCA pan/tilt ranges must fit signed 16-bit framing".into(),
                ));
            }
            let coherent = |degrees: f32, factor: f32, units: i32| {
                (degrees * factor - units as f32).abs() <= 0.5
            };
            let coherent_range =
                |degrees: &std::ops::RangeInclusive<f32>,
                 factor: f32,
                 units: &std::ops::RangeInclusive<i32>| {
                    let (start_units, end_units) = if factor.is_sign_negative() {
                        (*units.end(), *units.start())
                    } else {
                        (*units.start(), *units.end())
                    };
                    coherent(*degrees.start(), factor, start_units)
                        && coherent(*degrees.end(), factor, end_units)
                };
            if !coherent_range(
                &self.capabilities.pan_range_degrees,
                conversion.pan_degrees_to_units,
                &self.capabilities.pan_range,
            ) || !coherent_range(
                &self.capabilities.tilt_range_degrees,
                conversion.tilt_degrees_to_units,
                &self.capabilities.tilt_range,
            ) {
                return Err(Error::InvalidRequest(
                    "pan/tilt degree ranges must match their profile-specified unit scales".into(),
                ));
            }
        }
        if !self.capabilities.has_pan_tilt
            && (self.pan_tilt_coordinates.is_some()
                || self.capabilities.pan_speed != (0..=0)
                || self.capabilities.tilt_speed != (0..=0)
                || self.capabilities.pan_range != (0..=0)
                || self.capabilities.tilt_range != (0..=0)
                || self.capabilities.pan_range_degrees != (0.0..=0.0)
                || self.capabilities.tilt_range_degrees != (0.0..=0.0)
                || self.capabilities.pan_tilt_simultaneous
                || !self.capabilities.preset_recovery_time.is_zero())
        {
            return Err(Error::InvalidRequest(
                "a profile without pan/tilt must retain the conservative pan/tilt facts".into(),
            ));
        }
        if self.capabilities.has_zoom
            && (*self.capabilities.zoom_range_optical.start() != 0
                || self.capabilities.zoom_range_optical.start()
                    > self.capabilities.zoom_range_optical.end()
                || self.capabilities.zoom_speed.start() > self.capabilities.zoom_speed.end()
                || !self.capabilities.zoom_magnification_to_units.is_finite()
                || self.capabilities.zoom_magnification_to_units <= 0.0
                || self
                    .capabilities
                    .zoom_range_digital
                    .as_ref()
                    .is_some_and(|range| {
                        range.start() != self.capabilities.zoom_range_optical.end()
                            || range.start() > range.end()
                    }))
        {
            return Err(Error::InvalidRequest(
                "profile zoom ranges, converter, and speeds are invalid".into(),
            ));
        }
        if !self.capabilities.has_zoom
            && (self.capabilities.has_digital_zoom
                || self.capabilities.zoom_range_optical != (0..=0)
                || self.capabilities.zoom_range_digital.is_some()
                || self.capabilities.zoom_speed != (0..=0)
                || self.capabilities.supports_direct_zoom
                || self.capabilities.supports_variable_zoom
                || self.capabilities.zoom_magnification_to_units != 1.0)
        {
            return Err(Error::InvalidRequest(
                "a profile without zoom must retain the conservative zoom facts".into(),
            ));
        }
        if self.capabilities.has_focus
            && (self.capabilities.focus_range.start() > self.capabilities.focus_range.end()
                || self.capabilities.focus_speed.start() > self.capabilities.focus_speed.end())
        {
            return Err(Error::InvalidRequest(
                "profile focus ranges and speeds are invalid".into(),
            ));
        }
        let capabilities = &self.capabilities;
        if !optional_ordered(capabilities.exposure_comp_range.as_ref())
            || !range_ordered(&capabilities.exposure_comp_profile_range)
            || !optional_ordered(capabilities.iris_range.as_ref())
            || !range_ordered(&capabilities.gain_range)
            || !optional_ordered(capabilities.exposure_brightness_range.as_ref())
            || !optional_ordered(capabilities.color_temp_range.as_ref())
            || !optional_ordered(capabilities.rg_tuning_range.as_ref())
            || !optional_ordered(capabilities.bg_tuning_range.as_ref())
            || !optional_ordered(capabilities.red_gain_range.as_ref())
            || !optional_ordered(capabilities.blue_gain_range.as_ref())
            || !optional_ordered(capabilities.contrast_range.as_ref())
            || !optional_ordered(capabilities.sharpness_range.as_ref())
            || !optional_ordered(capabilities.saturation_range.as_ref())
            || !optional_ordered(capabilities.hue_range.as_ref())
            || !optional_ordered(capabilities.luminance_range.as_ref())
            || !optional_ordered(capabilities.gamma_range.as_ref())
            || !range_ordered(&capabilities.preset_speed_range)
        {
            return Err(Error::InvalidRequest(
                "profile contains an inverted optional capability range".into(),
            ));
        }
        if capabilities.has_digital_zoom != capabilities.zoom_range_digital.is_some()
            || capabilities.has_iris_control != capabilities.iris_range.is_some()
            || capabilities.exposure_comp_range.as_ref()
                != capabilities
                    .has_exposure_comp
                    .then_some(&capabilities.exposure_comp_profile_range)
            || capabilities.has_color_temp != capabilities.color_temp_range.is_some()
            || capabilities.red_gain_range.is_some() != capabilities.blue_gain_range.is_some()
            || capabilities.has_rgb_gain
                != (capabilities.red_gain_range.is_some() && capabilities.blue_gain_range.is_some())
            || capabilities.has_gamma != capabilities.gamma_range.is_some()
            || capabilities.has_luminance != capabilities.luminance_range.is_some()
        {
            return Err(Error::InvalidRequest(
                "profile capability flags and their ranges disagree".into(),
            ));
        }
        if (!capabilities.has_zoom
            && (capabilities.has_digital_zoom
                || capabilities.zoom_range_digital.is_some()
                || capabilities.supports_direct_zoom
                || capabilities.supports_variable_zoom))
            || (!capabilities.has_exposure
                && (capabilities.has_iris_control
                    || capabilities.iris_range.is_some()
                    || capabilities.has_backlight_comp
                    || capabilities.has_wdr
                    || capabilities.has_exposure_comp
                    || capabilities.exposure_comp_range.is_some()
                    || capabilities.exposure_brightness_range.is_some()))
            || (!capabilities.has_white_balance
                && (capabilities.has_one_push_wb
                    || capabilities.has_color_temp
                    || capabilities.color_temp_range.is_some()
                    || capabilities.rg_tuning_range.is_some()
                    || capabilities.bg_tuning_range.is_some()
                    || capabilities.has_rgb_gain
                    || capabilities.red_gain_range.is_some()
                    || capabilities.blue_gain_range.is_some()))
            || (!capabilities.has_image_processing
                && (capabilities.contrast_range.is_some()
                    || capabilities.sharpness_range.is_some()
                    || capabilities.saturation_range.is_some()
                    || capabilities.hue_range.is_some()
                    || capabilities.luminance_range.is_some()
                    || capabilities.gamma_range.is_some()
                    || capabilities.supports_flip
                    || capabilities.supports_mirror
                    || capabilities.has_noise_reduction
                    || capabilities.has_2d_nr
                    || capabilities.has_3d_nr
                    || capabilities.has_picture_effect
                    || capabilities.has_gamma
                    || capabilities.has_luminance))
        {
            return Err(Error::InvalidRequest(
                "profile leaf capabilities require their parent domain".into(),
            ));
        }
        let nd_facts_valid = match capabilities.nd_filter_mode {
            capabilities::NdFilterMode::None => capabilities.nd_filter_steps.is_none(),
            capabilities::NdFilterMode::Fixed(value) => {
                value != 0 && capabilities.nd_filter_steps.is_none()
            }
            capabilities::NdFilterMode::Stepped(steps) => {
                steps != 0 && capabilities.nd_filter_steps == Some(steps)
            }
            capabilities::NdFilterMode::Variable => capabilities.nd_filter_steps.is_none(),
        };
        let has_typed_nd_mode = !matches!(
            capabilities.nd_filter_mode,
            capabilities::NdFilterMode::None
        );
        if capabilities.has_nd_filter != has_typed_nd_mode
            || !nd_facts_valid
            || capabilities.max_motion_sync_speed.as_ref()
                != capabilities
                    .has_motion_sync
                    .then_some(&capabilities.max_motion_sync_speed_profile)
            || capabilities.max_motion_sync_speed_profile == 0
            || ((capabilities.has_motion_sync || capabilities.has_variable_speed)
                && !capabilities.has_pan_tilt)
        {
            return Err(Error::InvalidRequest(
                "profile ND filter, motion-sync, or variable-speed facts disagree".into(),
            ));
        }
        if (!capabilities.has_focus
            && (capabilities.has_auto_focus
                || capabilities.has_one_push_focus
                || capabilities.has_focus_zone
                || capabilities.has_focus_zone_inquiry
                || capabilities.has_af_sensitivity
                || capabilities.has_focus_near_limit_inquiry
                || capabilities.focus_range != (0..=0)
                || capabilities.focus_speed != (0..=0)))
            || (capabilities.has_af_sensitivity && !capabilities.has_auto_focus)
            || (!capabilities.has_pan_tilt
                && (capabilities.pan_tilt_simultaneous
                    || !capabilities.preset_recovery_time.is_zero()))
            || (!capabilities.has_presets
                && (capabilities.max_presets != 0
                    || capabilities.preset_speed_range != (0..=0)
                    || capabilities.supports_preset_tour
                    || capabilities.supports_preset_thumbnail
                    || !capabilities.preset_recall_delay.is_zero()
                    || capabilities.supports_preset_names
                    || capabilities.max_preset_name_length != 0))
            || (capabilities.has_presets
                && (capabilities.max_presets == 0 || *capabilities.preset_speed_range.start() == 0))
            || (capabilities.supports_preset_names != (capabilities.max_preset_name_length != 0))
            || (!capabilities.has_power
                && (capabilities.supports_standby
                    || capabilities.supports_wake_on_lan
                    || !capabilities.power_on_time.is_zero()
                    || !capabilities.standby_time.is_zero()
                    || capabilities.retains_settings_on_power_off
                    || capabilities.home_on_power_up))
            || (capabilities.has_power && capabilities.power_on_time.is_zero())
            || (capabilities.supports_standby && capabilities.standby_time.is_zero())
            || (!capabilities.has_exposure
                && (!capabilities.exposure_modes.is_empty()
                    || !capabilities.shutter_speeds.is_empty()
                    || capabilities.gain_range != (0..=0)))
            || (capabilities.has_exposure
                && (capabilities.exposure_modes.is_empty()
                    || capabilities.shutter_speeds.is_empty()))
            || (!capabilities.has_white_balance && !capabilities.white_balance_modes.is_empty())
            || (capabilities.has_white_balance && capabilities.white_balance_modes.is_empty())
            || (capabilities.supports_hue != capabilities.hue_range.is_some())
            || (capabilities.uses_combined_flip_command
                && !(capabilities.supports_flip && capabilities.supports_mirror))
            || (capabilities.uses_combined_flip_command
                != capabilities
                    .supports_typed(capabilities::TypedSupportSurface::CombinedImageFlip))
            || (capabilities.requires_settings_save_for_flip && !capabilities.supports_flip)
            || ((capabilities.has_2d_nr || capabilities.has_3d_nr)
                && !capabilities.has_noise_reduction)
        {
            return Err(Error::InvalidRequest(
                "profile aggregate capability facts disagree with their parent domains".into(),
            ));
        }
        if capabilities
            .shutter_speeds
            .iter()
            .any(|speed| speed.label.trim().is_empty())
            || capabilities
                .shutter_speeds
                .iter()
                .enumerate()
                .any(|(index, speed)| {
                    capabilities.shutter_speeds[..index]
                        .iter()
                        .any(|earlier| earlier.value == speed.value || earlier.label == speed.label)
                })
            || capabilities
                .exposure_modes
                .iter()
                .enumerate()
                .any(|(index, mode)| capabilities.exposure_modes[..index].contains(mode))
            || capabilities
                .white_balance_modes
                .iter()
                .enumerate()
                .any(|(index, mode)| capabilities.white_balance_modes[..index].contains(mode))
        {
            return Err(Error::InvalidRequest(
                "profile mode and shutter inventories must be non-empty and duplicate-free".into(),
            ));
        }
        for surface in capabilities.typed_support.iter() {
            let physically_supported = match surface {
                capabilities::TypedSupportSurface::DirectZoom => {
                    capabilities.has_zoom && capabilities.supports_direct_zoom
                }
                capabilities::TypedSupportSurface::DigitalZoomToggle => {
                    capabilities.has_zoom && capabilities.has_digital_zoom
                }
                capabilities::TypedSupportSurface::DigitalZoomRange => {
                    capabilities.has_zoom && capabilities.zoom_range_digital.is_some()
                }
                capabilities::TypedSupportSurface::IrisControl => {
                    capabilities.has_exposure && capabilities.iris_range.is_some()
                }
                capabilities::TypedSupportSurface::OnePushFocus => {
                    capabilities.has_focus && capabilities.has_one_push_focus
                }
                capabilities::TypedSupportSurface::PtzOpticsSnapFocus => capabilities.has_focus,
                capabilities::TypedSupportSurface::PtzOpticsAntiFlicker
                | capabilities::TypedSupportSurface::SonySpotlight
                | capabilities::TypedSupportSurface::SonyAutoSlowShutter => {
                    capabilities.has_exposure
                }
                capabilities::TypedSupportSurface::PtzOpticsPresetRecallSpeed => {
                    capabilities.has_presets
                }
                // The vendor settings-save and streaming controls have no
                // separate discovery metadata. Their typed-support fact is
                // the complete source-backed permission.
                capabilities::TypedSupportSurface::PtzOpticsSettingsSave
                | capabilities::TypedSupportSurface::PtzOpticsMulticastStreaming
                | capabilities::TypedSupportSurface::PtzOpticsNdiQuality => true,
                capabilities::TypedSupportSurface::FocusLock
                | capabilities::TypedSupportSurface::PushAutoFocus => capabilities.has_focus,
                capabilities::TypedSupportSurface::FocusZone => {
                    capabilities.has_focus && capabilities.has_focus_zone
                }
                capabilities::TypedSupportSurface::FocusZoneInquiry => {
                    capabilities.has_focus && capabilities.has_focus_zone_inquiry
                }
                capabilities::TypedSupportSurface::AutoFocusSensitivity => {
                    capabilities.has_focus
                        && capabilities.has_auto_focus
                        && capabilities.has_af_sensitivity
                }
                capabilities::TypedSupportSurface::FocusNearLimitInquiry => {
                    capabilities.has_focus && capabilities.has_focus_near_limit_inquiry
                }
                capabilities::TypedSupportSurface::BacklightCompensation => {
                    capabilities.has_exposure && capabilities.has_backlight_comp
                }
                capabilities::TypedSupportSurface::WideDynamicRange => {
                    capabilities.has_exposure && capabilities.has_wdr
                }
                capabilities::TypedSupportSurface::ExposureCompensation => {
                    capabilities.has_exposure && capabilities.exposure_comp_range.is_some()
                }
                capabilities::TypedSupportSurface::BrightnessControl => {
                    capabilities.has_exposure && capabilities.exposure_brightness_range.is_some()
                }
                capabilities::TypedSupportSurface::OnePushWhiteBalance => {
                    capabilities.has_white_balance
                        && capabilities.has_one_push_wb
                        && capabilities
                            .white_balance_modes
                            .contains(&crate::command::WhiteBalanceMode::OnePush)
                }
                capabilities::TypedSupportSurface::AutoTrackingWhiteBalance => {
                    capabilities.has_white_balance
                        && capabilities
                            .white_balance_modes
                            .contains(&crate::command::WhiteBalanceMode::ATW)
                }
                capabilities::TypedSupportSurface::AutoWhiteBalanceSensitivity => {
                    capabilities.has_white_balance
                }
                capabilities::TypedSupportSurface::ColorTemperature => {
                    capabilities.has_white_balance
                        && capabilities.color_temp_range.is_some()
                        && capabilities
                            .white_balance_modes
                            .contains(&crate::command::WhiteBalanceMode::ColorTemperature)
                }
                capabilities::TypedSupportSurface::RgbGain => {
                    capabilities.has_white_balance
                        && capabilities.has_rgb_gain
                        && capabilities.red_gain_range.is_some()
                        && capabilities.blue_gain_range.is_some()
                }
                capabilities::TypedSupportSurface::RgbTuning => {
                    capabilities.has_white_balance
                        && capabilities.rg_tuning_range.is_some()
                        && capabilities.bg_tuning_range.is_some()
                }
                capabilities::TypedSupportSurface::ImageFlip => {
                    capabilities.has_image_processing && capabilities.supports_flip
                }
                capabilities::TypedSupportSurface::ImageMirror => {
                    capabilities.has_image_processing && capabilities.supports_mirror
                }
                capabilities::TypedSupportSurface::CombinedImageFlip => {
                    capabilities.has_image_processing
                        && capabilities.supports_flip
                        && capabilities.supports_mirror
                        && capabilities.uses_combined_flip_command
                }
                capabilities::TypedSupportSurface::ContrastControl => {
                    capabilities.has_image_processing && capabilities.contrast_range.is_some()
                }
                capabilities::TypedSupportSurface::SharpnessControl => {
                    capabilities.has_image_processing && capabilities.sharpness_range.is_some()
                }
                capabilities::TypedSupportSurface::SaturationControl => {
                    capabilities.has_image_processing && capabilities.saturation_range.is_some()
                }
                capabilities::TypedSupportSurface::HueControl => {
                    capabilities.has_image_processing && capabilities.hue_range.is_some()
                }
                capabilities::TypedSupportSurface::LuminanceControl => {
                    capabilities.has_image_processing && capabilities.luminance_range.is_some()
                }
                capabilities::TypedSupportSurface::GammaControl => {
                    capabilities.has_image_processing && capabilities.gamma_range.is_some()
                }
                capabilities::TypedSupportSurface::NoiseReduction => {
                    capabilities.has_image_processing && capabilities.has_noise_reduction
                }
                capabilities::TypedSupportSurface::NoiseReduction2D => {
                    capabilities.has_image_processing && capabilities.has_2d_nr
                }
                capabilities::TypedSupportSurface::NoiseReduction3D => {
                    capabilities.has_image_processing && capabilities.has_3d_nr
                }
                capabilities::TypedSupportSurface::PictureEffect => {
                    capabilities.has_image_processing && capabilities.has_picture_effect
                }
                capabilities::TypedSupportSurface::Tally => capabilities.has_tally,
                capabilities::TypedSupportSurface::DirectMenu => {
                    capabilities.has_direct_menu_control
                }
                capabilities::TypedSupportSurface::NdFilter => capabilities.has_nd_filter,
                capabilities::TypedSupportSurface::VariableSpeed => {
                    capabilities.has_pan_tilt && capabilities.has_variable_speed
                }
                capabilities::TypedSupportSurface::MotionSync => {
                    capabilities.has_pan_tilt && capabilities.has_motion_sync
                }
                capabilities::TypedSupportSurface::UsbAudio => capabilities.has_usb_audio,
            };
            if !physically_supported {
                return Err(Error::InvalidRequest(
                    format!(
                        "typed support {surface:?} cannot enable an absent physical capability"
                    )
                    .into(),
                ));
            }
        }
        if self.capabilities.supports_operation_complete != self.supports_operation_complete {
            return Err(Error::InvalidRequest(
                "profile completion capability facts disagree".into(),
            ));
        }
        if matches!(
            self.capabilities.inquiry_support,
            capabilities::InquirySupport::None
        ) && (self.position_inquiries.pan_tilt
            || self.position_inquiries.zoom
            || self.position_inquiries.focus
            || self.position_inquiries.iris
            || self.position_inquiries.nd_filter)
        {
            return Err(Error::InvalidRequest(
                "position inquiries require profile inquiry support".into(),
            ));
        }
        if (self.position_inquiries.pan_tilt && !self.capabilities.has_pan_tilt)
            || (self.position_inquiries.zoom && !self.capabilities.has_zoom)
            || (self.position_inquiries.focus && !self.capabilities.has_focus)
            || (self.position_inquiries.iris && !self.capabilities.has_iris_control)
            || (self.position_inquiries.nd_filter && !self.capabilities.has_nd_filter)
        {
            return Err(Error::InvalidRequest(
                "position inquiry support names an unsupported axis".into(),
            ));
        }
        // Metadata such as `has_iris_control` and `has_nd_filter` describes
        // what discovery may report; it does not make a typed targeted
        // operation submit-able.  Only the typed support registry grants that
        // operation surface, so only those markers participate in this
        // completionless-settlement invariant.
        let typed_iris_requires_inquiry = self
            .capabilities
            .supports_typed(capabilities::TypedSupportSurface::IrisControl);
        let typed_nd_requires_inquiry = self
            .capabilities
            .supports_typed(capabilities::TypedSupportSurface::NdFilter);
        if !self.supports_operation_complete
            && ((self.capabilities.has_pan_tilt && !self.position_inquiries.pan_tilt)
                || (self.capabilities.has_focus && !self.position_inquiries.focus)
                || (self.capabilities.has_zoom
                    && self.capabilities.supports_direct_zoom
                    && self
                        .capabilities
                        .supports_typed(capabilities::TypedSupportSurface::DirectZoom)
                    && !self.position_inquiries.zoom)
                || (typed_iris_requires_inquiry && !self.position_inquiries.iris)
                || (typed_nd_requires_inquiry && !self.position_inquiries.nd_filter))
        {
            return Err(Error::InvalidRequest(
                "profile without exact completion needs inquiries for every targeted axis".into(),
            ));
        }
        if self.capabilities.has_presets != self.preset_recall_axes.is_some() {
            return Err(Error::InvalidRequest(
                "preset recall axes must be present exactly when presets are supported".into(),
            ));
        }
        if let Some(preset_axes) = self.preset_recall_axes {
            let available_axes = AffectedAxes::new_with_iris_nd(
                self.capabilities.has_pan_tilt,
                self.capabilities.has_zoom,
                self.capabilities.has_focus,
                self.capabilities.has_iris_control,
                self.capabilities.has_nd_filter,
            )?;
            if !available_axes.contains(preset_axes) {
                return Err(Error::InvalidRequest(
                    "preset recall axes include an unsupported camera axis".into(),
                ));
            }
            if !self.supports_operation_complete && !self.position_inquiries.supports(preset_axes) {
                return Err(Error::InvalidRequest(
                    "profile without exact completion needs every preset-recall position inquiry"
                        .into(),
                ));
            }
        }
        Ok(self)
    }
}

/// Compile-time camera profile that lowers to the validated runtime form.
pub trait CompileTimeProfile: capabilities::Profile {
    /// Compatible standard transports and their default ports.
    const TRANSPORTS: TransportCompatibility;
    /// Inquiry response timeout.
    const INQUIRY_TIMEOUT: Duration;
    /// Socket-cancellation response timeout.
    const CANCELLATION_TIMEOUT: Duration;
    /// Pre-ack cancellation ambiguity timeout.
    const AMBIGUITY_TIMEOUT: Duration;
    /// Maximum number of command sockets per camera target.
    const MAXIMUM_COMMAND_SOCKETS: u8;
    /// Exact axes affected by a preset recall.
    const PRESET_RECALL_AXES: Option<AffectedAxes>;
    /// Per-axis position inquiry availability.
    const POSITION_INQUIRIES: PositionInquirySupport;
}

/// Builder for a validated runtime [`ProfileSpec`].
///
/// Transport, envelope, timing, socket, completion, cancellation, preset-axis,
/// and per-axis inquiry facts are mandatory. This prevents a generic default
/// from silently granting protocol behavior.
#[derive(Debug, Clone)]
pub struct ProfileSpecBuilder {
    capabilities: capabilities::Capabilities,
    tcp_port_inferred: bool,
    udp_port_inferred: bool,
    pan_tilt_coordinates: Option<PanTiltCoordinateConversion>,
    pan_tilt_wire_codec: capabilities::PanTiltWireCodec,
    transports: Option<TransportCompatibility>,
    envelope: Option<ProfileEnvelope>,
    timing: Option<ProfileTiming>,
    maximum_command_sockets: Option<u8>,
    supports_operation_complete: Option<bool>,
    supports_command_cancel: Option<bool>,
    preset_recall_axes: Option<Option<AffectedAxes>>,
    position_inquiries: Option<PositionInquirySupport>,
}

impl ProfileSpecBuilder {
    fn runtime(capabilities: capabilities::Capabilities) -> Self {
        let tcp_port_inferred = capabilities.default_tcp_port.is_none();
        let udp_port_inferred = capabilities.default_udp_port.is_none();
        Self {
            capabilities,
            tcp_port_inferred,
            udp_port_inferred,
            pan_tilt_coordinates: None,
            pan_tilt_wire_codec: capabilities::PanTiltWireCodec::StandardVisca,
            transports: None,
            envelope: None,
            timing: None,
            maximum_command_sockets: None,
            supports_operation_complete: None,
            supports_command_cancel: None,
            preset_recall_axes: None,
            position_inquiries: None,
        }
    }

    fn from_compile_time<P>() -> Self
    where
        P: CompileTimeProfile,
    {
        let mut capabilities = capabilities::Capabilities::from_profile::<P>();
        capabilities.default_tcp_port = P::TRANSPORTS.tcp_port();
        capabilities.default_udp_port = P::TRANSPORTS.udp_port();
        Self {
            capabilities,
            tcp_port_inferred: false,
            udp_port_inferred: false,
            pan_tilt_coordinates: Some(PanTiltCoordinateConversion {
                coordinate_system: P::COORDINATE_SYSTEM,
                wire_codec: P::PAN_TILT_WIRE_CODEC,
                pan_degrees_to_units: P::PAN_DEGREES_TO_UNITS,
                tilt_degrees_to_units: P::TILT_DEGREES_TO_UNITS,
            }),
            pan_tilt_wire_codec: P::PAN_TILT_WIRE_CODEC,
            transports: Some(P::TRANSPORTS),
            envelope: Some(
                if <P::Envelope as crate::transport::Envelope>::SUPPORTS_SEQUENCE_CORRELATION {
                    ProfileEnvelope::SonyEncapsulated
                } else {
                    ProfileEnvelope::RawVisca
                },
            ),
            timing: Some(ProfileTiming {
                ack_timeout: P::ACK_TIMEOUT,
                command_timeouts: P::COMMAND_TIMEOUTS,
                inquiry_timeout: P::INQUIRY_TIMEOUT,
                cancellation_timeout: P::CANCELLATION_TIMEOUT,
                ambiguity_timeout: P::AMBIGUITY_TIMEOUT,
                busy_timeout: P::BUSY_TIMEOUT,
                minimum_inquiry_spacing: P::MIN_INQUIRY_SPACING,
                minimum_command_spacing: P::MIN_COMMAND_SPACING,
            }),
            maximum_command_sockets: Some(P::MAXIMUM_COMMAND_SOCKETS),
            supports_operation_complete: Some(P::SUPPORTS_OPERATION_COMPLETE),
            supports_command_cancel: Some(P::SUPPORTS_COMMAND_CANCEL),
            preset_recall_axes: Some(P::PRESET_RECALL_AXES),
            position_inquiries: Some(P::POSITION_INQUIRIES),
        }
    }

    /// Sets the authoritative standard transport compatibility facts.
    ///
    /// Unspecified capability port mirrors are filled from this value. An
    /// explicitly populated capability port remains an assertion and a
    /// mismatch is rejected by [`Self::build`].
    #[must_use]
    pub fn transports(mut self, transports: TransportCompatibility) -> Self {
        if self.tcp_port_inferred {
            self.capabilities.default_tcp_port = transports.tcp_port();
        }
        if self.udp_port_inferred {
            self.capabilities.default_udp_port = transports.udp_port();
        }
        self.transports = Some(transports);
        self
    }

    /// Sets the required wire envelope.
    #[must_use]
    pub fn envelope(mut self, envelope: ProfileEnvelope) -> Self {
        self.envelope = Some(envelope);
        self
    }

    /// Sets the maximum command socket count.
    #[must_use]
    pub fn maximum_command_sockets(mut self, maximum: u8) -> Self {
        self.maximum_command_sockets = Some(maximum);
        self
    }

    /// Sets exact preset-recall axes.
    #[must_use]
    pub fn preset_recall_axes(mut self, axes: Option<AffectedAxes>) -> Self {
        self.preset_recall_axes = Some(axes);
        self
    }

    /// Sets exact per-axis position-inquiry support for settlement.
    #[must_use]
    pub fn position_inquiries(mut self, supported: PositionInquirySupport) -> Self {
        self.position_inquiries = Some(supported);
        self
    }

    /// Sets the default camera ID.
    #[must_use]
    pub fn default_camera_id(mut self, camera_id: u8) -> Self {
        self.capabilities.default_camera_id = camera_id;
        self
    }

    /// Sets pan/tilt range, speed, and signed coordinate-conversion facts.
    #[must_use]
    #[allow(clippy::too_many_arguments)]
    pub fn pan_tilt(
        mut self,
        pan_range: std::ops::RangeInclusive<i32>,
        tilt_range: std::ops::RangeInclusive<i32>,
        maximum_pan_speed: u8,
        maximum_tilt_speed: u8,
        pan_degrees_to_units: f32,
        tilt_degrees_to_units: f32,
        coordinate_system: capabilities::CoordinateSystem,
        simultaneous: bool,
    ) -> Self {
        self.capabilities.pan_range = pan_range.clone();
        self.capabilities.tilt_range = tilt_range.clone();
        self.capabilities.pan_speed = 1..=maximum_pan_speed;
        self.capabilities.tilt_speed = 1..=maximum_tilt_speed;
        let pan_start_degrees = *pan_range.start() as f32 / pan_degrees_to_units;
        let pan_end_degrees = *pan_range.end() as f32 / pan_degrees_to_units;
        let tilt_start_degrees = *tilt_range.start() as f32 / tilt_degrees_to_units;
        let tilt_end_degrees = *tilt_range.end() as f32 / tilt_degrees_to_units;
        self.capabilities.pan_range_degrees =
            pan_start_degrees.min(pan_end_degrees)..=pan_start_degrees.max(pan_end_degrees);
        self.capabilities.tilt_range_degrees =
            tilt_start_degrees.min(tilt_end_degrees)..=tilt_start_degrees.max(tilt_end_degrees);
        self.capabilities.pan_tilt_simultaneous = simultaneous;
        self.capabilities.has_pan_tilt = true;
        self.pan_tilt_coordinates = Some(PanTiltCoordinateConversion {
            coordinate_system,
            wire_codec: self.pan_tilt_wire_codec,
            pan_degrees_to_units,
            tilt_degrees_to_units,
        });
        self
    }

    /// Sets profile-specified signed coordinate facts for a pre-populated pan/tilt capability
    /// inventory.
    #[must_use]
    pub fn pan_tilt_coordinates(
        mut self,
        coordinate_system: capabilities::CoordinateSystem,
        pan_degrees_to_units: f32,
        tilt_degrees_to_units: f32,
    ) -> Self {
        self.pan_tilt_coordinates = Some(PanTiltCoordinateConversion {
            coordinate_system,
            wire_codec: self.pan_tilt_wire_codec,
            pan_degrees_to_units,
            tilt_degrees_to_units,
        });
        self
    }

    /// Selects the profile-owned pan/tilt position-command and inquiry framing.
    ///
    /// The standard VISCA framing remains the default for runtime profiles.
    /// Calling this after [`Self::pan_tilt`] or [`Self::pan_tilt_coordinates`]
    /// updates the already supplied conversion facts as well.
    #[must_use]
    pub fn pan_tilt_wire_codec(mut self, wire_codec: capabilities::PanTiltWireCodec) -> Self {
        self.pan_tilt_wire_codec = wire_codec;
        if let Some(conversion) = &mut self.pan_tilt_coordinates {
            conversion.wire_codec = wire_codec;
        }
        self
    }

    /// Sets zoom range, speed, positioning, and conversion facts.
    #[must_use]
    pub fn zoom(
        mut self,
        optical_maximum: u16,
        digital_maximum: Option<u16>,
        speed_range: std::ops::RangeInclusive<u8>,
        supports_direct: bool,
        supports_variable: bool,
        magnification_to_units: f32,
    ) -> Self {
        self.capabilities.zoom_range_optical = 0..=optical_maximum;
        self.capabilities.zoom_range_digital =
            digital_maximum.map(|maximum| optical_maximum..=maximum);
        self.capabilities.zoom_speed = speed_range;
        self.capabilities.supports_direct_zoom = supports_direct;
        self.capabilities.supports_variable_zoom = supports_variable;
        self.capabilities.zoom_magnification_to_units = magnification_to_units;
        self.capabilities.has_digital_zoom = digital_maximum.is_some();
        self.capabilities.has_zoom = true;
        self
    }

    /// Sets focus range, speed, and autofocus capability facts.
    #[must_use]
    pub fn focus(
        mut self,
        position_range: std::ops::RangeInclusive<u16>,
        speed_range: std::ops::RangeInclusive<u8>,
        supports_auto_focus: bool,
        supports_one_push: bool,
    ) -> Self {
        self.capabilities.focus_range = position_range;
        self.capabilities.focus_speed = speed_range;
        self.capabilities.has_auto_focus = supports_auto_focus;
        self.capabilities.has_one_push_focus = supports_one_push;
        self.capabilities.has_focus = true;
        self
    }

    /// Sets the complete optional typed-surface permission set.
    #[must_use]
    pub fn typed_support(mut self, support: capabilities::TypedSupportSet) -> Self {
        self.capabilities.typed_support = support;
        self
    }

    /// Sets the overall inquiry support level.
    #[must_use]
    pub fn inquiry_support(mut self, support: capabilities::InquirySupport) -> Self {
        self.capabilities.inquiry_support = support;
        self
    }

    /// Sets immutable timing and pacing facts.
    #[must_use]
    pub fn timing(mut self, timing: ProfileTiming) -> Self {
        self.timing = Some(timing);
        self
    }

    /// Sets whether the camera produces exact operation-complete messages.
    #[must_use]
    pub fn supports_operation_complete(mut self, supported: bool) -> Self {
        self.supports_operation_complete = Some(supported);
        self.capabilities.supports_operation_complete = supported;
        self
    }

    /// Sets whether socket-specific protocol cancellation is supported.
    #[must_use]
    pub fn supports_command_cancel(mut self, supported: bool) -> Self {
        self.supports_command_cancel = Some(supported);
        self
    }

    /// Validates and returns an immutable profile.
    pub fn build(self) -> Result<ProfileSpec> {
        fn required<T>(value: Option<T>, name: &'static str) -> Result<T> {
            value.ok_or_else(|| Error::InvalidRequest(name.into()))
        }

        let transports = required(self.transports, "profile transport facts are required")?;
        let capabilities = self.capabilities;
        if capabilities.default_tcp_port != transports.tcp_port
            || capabilities.default_udp_port != transports.udp_port
        {
            return Err(Error::InvalidRequest(
                "capability default ports must exactly match profile transports".into(),
            ));
        }
        let timing = required(self.timing, "profile timing facts are required")?;
        ProfileSpec {
            capabilities,
            pan_tilt_coordinates: self.pan_tilt_coordinates,
            transports,
            envelope: required(self.envelope, "profile envelope is required")?,
            timing,
            maximum_command_sockets: required(
                self.maximum_command_sockets,
                "profile command socket limit is required",
            )?,
            supports_operation_complete: required(
                self.supports_operation_complete,
                "profile operation-complete support is required",
            )?,
            supports_command_cancel: required(
                self.supports_command_cancel,
                "profile cancellation support is required",
            )?,
            preset_recall_axes: required(
                self.preset_recall_axes,
                "profile preset recall axes are required",
            )?,
            position_inquiries: required(
                self.position_inquiries,
                "profile position inquiry support is required",
            )?,
        }
        .validate()
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, unused_qualifications)]
mod tests {
    use super::*;
    use crate::{
        capabilities::{Capabilities, RuntimeShutterSpeed, TypedSupportSet, TypedSupportSurface},
        request::builtin::PresetRecall,
        ExposureMode, OperationCommand, PresetNumber,
    };

    fn valid_runtime_capabilities() -> Capabilities {
        let mut capabilities = Capabilities::runtime_baseline("Downstream runtime camera", 1)
            .expect("runtime baseline");
        capabilities.has_pan_tilt = true;
        capabilities.pan_speed = 1..=8;
        capabilities.tilt_speed = 1..=8;
        capabilities.pan_range = -100..=100;
        capabilities.tilt_range = -50..=50;
        capabilities.pan_range_degrees = -10.0..=10.0;
        capabilities.tilt_range_degrees = -5.0..=5.0;
        capabilities.has_zoom = true;
        capabilities.zoom_range_optical = 0..=1_000;
        capabilities.zoom_speed = 0..=7;
        capabilities.zoom_magnification_to_units = 100.0;
        capabilities.has_focus = true;
        capabilities.focus_range = 0..=2_000;
        capabilities.focus_speed = 0..=7;
        capabilities.has_presets = true;
        capabilities.max_presets = 10;
        capabilities.preset_speed_range = 1..=8;
        capabilities.inquiry_support = capabilities::InquirySupport::Partial;
        capabilities
    }

    fn runtime_white_balance_capabilities(modes: Vec<crate::WhiteBalanceMode>) -> Capabilities {
        let mut capabilities = valid_runtime_capabilities();
        capabilities.has_white_balance = true;
        capabilities.white_balance_modes = modes;
        capabilities
    }

    fn runtime_builder(capabilities: Capabilities) -> ProfileSpecBuilder {
        ProfileSpec::builder(capabilities)
            .pan_tilt_coordinates(capabilities::CoordinateSystem::SignedCentered, 10.0, 10.0)
            .transports(TransportCompatibility::new(Some(5678), None, false))
            .envelope(ProfileEnvelope::RawVisca)
            .timing(
                ProfileTiming::builder()
                    .ack_timeout(Duration::from_millis(100))
                    .command_timeouts(CommandTimeouts::default())
                    .inquiry_timeout(Duration::from_secs(1))
                    .cancellation_timeout(Duration::from_secs(1))
                    .ambiguity_timeout(Duration::from_secs(1))
                    .busy_timeout(Duration::ZERO)
                    .minimum_inquiry_spacing(Duration::from_millis(25))
                    .minimum_command_spacing(Duration::from_millis(25))
                    .build()
                    .expect("valid timing"),
            )
            .maximum_command_sockets(1)
            .supports_operation_complete(false)
            .supports_command_cancel(false)
            .preset_recall_axes(Some(AffectedAxes::PAN_TILT.union(AffectedAxes::ZOOM)))
            .position_inquiries(PositionInquirySupport::new(true, true, true))
    }

    #[test]
    fn runtime_brc300_codec_requires_ranges_encodable_by_its_position_fields() {
        let mut endpoints = valid_runtime_capabilities();
        endpoints.pan_range = -0x080000..=0x07_FFFF;
        endpoints.tilt_range = -0x8000..=0x7FFF;
        endpoints.pan_range_degrees = -524_288.0..=524_287.0;
        endpoints.tilt_range_degrees = -32_768.0..=32_767.0;

        assert!(runtime_builder(endpoints.clone())
            .pan_tilt_coordinates(capabilities::CoordinateSystem::SignedCentered, 1.0, 1.0)
            .pan_tilt_wire_codec(capabilities::PanTiltWireCodec::SonyBrc300)
            .build()
            .is_ok());

        let mut pan_out_of_range = endpoints.clone();
        pan_out_of_range.pan_range = -0x080001..=0x07_FFFF;
        pan_out_of_range.pan_range_degrees = -524_289.0..=524_287.0;
        assert!(runtime_builder(pan_out_of_range)
            .pan_tilt_coordinates(capabilities::CoordinateSystem::SignedCentered, 1.0, 1.0)
            .pan_tilt_wire_codec(capabilities::PanTiltWireCodec::SonyBrc300)
            .build()
            .is_err());

        let mut tilt_out_of_range = endpoints;
        tilt_out_of_range.tilt_range = -0x8001..=0x7FFF;
        tilt_out_of_range.tilt_range_degrees = -32_769.0..=32_767.0;
        assert!(runtime_builder(tilt_out_of_range)
            .pan_tilt_coordinates(capabilities::CoordinateSystem::SignedCentered, 1.0, 1.0)
            .pan_tilt_wire_codec(capabilities::PanTiltWireCodec::SonyBrc300)
            .build()
            .is_err());
    }

    #[test]
    fn runtime_standard_visca_codec_requires_ranges_encodable_by_its_position_fields() {
        let build = |pan_range: std::ops::RangeInclusive<i32>,
                     tilt_range: std::ops::RangeInclusive<i32>| {
            runtime_builder(valid_runtime_capabilities())
                .pan_tilt(
                    pan_range,
                    tilt_range,
                    8,
                    8,
                    -1.0,
                    1.0,
                    capabilities::CoordinateSystem::SignedCentered,
                    true,
                )
                .pan_tilt_wire_codec(capabilities::PanTiltWireCodec::StandardVisca)
                .build()
        };

        assert!(build(-0x8000..=0x7FFF, -0x8000..=0x7FFF).is_ok());

        for result in [
            build(-0x8001..=0x7FFF, -0x8000..=0x7FFF),
            build(-0x8000..=0x8000, -0x8000..=0x7FFF),
            build(-0x8000..=0x7FFF, -0x8001..=0x7FFF),
            build(-0x8000..=0x7FFF, -0x8000..=0x8000),
        ] {
            assert!(matches!(
                result,
                Err(Error::InvalidRequest(message))
                    if message == "standard VISCA pan/tilt ranges must fit signed 16-bit framing"
            ));
        }
    }

    #[test]
    fn reverse_axis_brc300_scales_keep_static_and_runtime_ranges_ordered() {
        let static_profile = ProfileSpec::from_compile_time::<crate::profiles::SonyBRC300>()
            .expect("Sony BRC-300 static profile");
        let static_capabilities = static_profile.capabilities();

        let runtime_profile = runtime_builder(valid_runtime_capabilities())
            .pan_tilt(
                -0x08A58..=0x08A58,
                -0x186A..=0x493D,
                0x18,
                0x18,
                -208.0,
                -208.0,
                capabilities::CoordinateSystem::SignedCentered,
                true,
            )
            .pan_tilt_wire_codec(capabilities::PanTiltWireCodec::SonyBrc300)
            .build()
            .expect("valid reverse-axis runtime BRC-300 profile");

        assert_eq!(
            runtime_profile.capabilities().pan_range_degrees,
            static_capabilities.pan_range_degrees
        );
        assert_eq!(
            runtime_profile.capabilities().tilt_range_degrees,
            static_capabilities.tilt_range_degrees
        );

        for profile in [&static_profile, &runtime_profile] {
            let capabilities = profile.capabilities();
            assert!(capabilities.pan_range_degrees.start() <= capabilities.pan_range_degrees.end());
            assert!(
                capabilities.tilt_range_degrees.start() <= capabilities.tilt_range_degrees.end()
            );

            let conversion = profile.pan_tilt_coordinates().expect("BRC-300 coordinates");
            let (pan, tilt) = profile
                .convert_pan_tilt_degrees(45.0, -15.0)
                .expect("right/up BRC-300 target");
            assert_eq!((pan, tilt), (-0x02490, 0x0C30));
            assert_eq!(pan as f32 / conversion.pan_degrees_to_units(), 45.0);
            assert_eq!(tilt as f32 / conversion.tilt_degrees_to_units(), -15.0);
        }
    }

    #[test]
    fn runtime_pan_tilt_scales_reject_zero_and_nonfinite_values() {
        for (name, pan_scale, tilt_scale) in [
            ("zero pan", 0.0, 10.0),
            ("negative zero tilt", 10.0, -0.0),
            ("NaN pan", f32::NAN, 10.0),
            ("NaN tilt", 10.0, f32::NAN),
            ("infinite pan", f32::INFINITY, 10.0),
            ("infinite tilt", 10.0, f32::NEG_INFINITY),
        ] {
            assert!(
                runtime_builder(valid_runtime_capabilities())
                    .pan_tilt(
                        -100..=100,
                        -50..=50,
                        8,
                        8,
                        pan_scale,
                        tilt_scale,
                        capabilities::CoordinateSystem::SignedCentered,
                        true,
                    )
                    .build()
                    .is_err(),
                "accepted {name} pan/tilt scale"
            );
        }
    }

    fn conservative_runtime_builder(capabilities: Capabilities) -> ProfileSpecBuilder {
        ProfileSpec::builder(capabilities)
            .transports(TransportCompatibility::new(Some(5678), None, false))
            .envelope(ProfileEnvelope::RawVisca)
            .timing(
                ProfileTiming::builder()
                    .ack_timeout(Duration::from_millis(100))
                    .command_timeouts(CommandTimeouts::default())
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
    }

    #[test]
    fn runtime_only_baseline_builds_after_explicit_facts_and_keeps_exact_preset_axes() {
        let profile = runtime_builder(valid_runtime_capabilities())
            .build()
            .expect("valid runtime profile");
        let recall = PresetRecall::for_profile(PresetNumber::new(3).expect("preset"), &profile)
            .expect("profile supports presets");
        assert_eq!(
            recall.affected_axes(),
            AffectedAxes::PAN_TILT.union(AffectedAxes::ZOOM)
        );
    }

    #[test]
    fn completionless_scalar_settlement_uses_typed_support_not_general_metadata() {
        let mut iris_metadata = valid_runtime_capabilities();
        iris_metadata.has_exposure = true;
        iris_metadata.exposure_modes.push(ExposureMode::Auto);
        iris_metadata.shutter_speeds.push(RuntimeShutterSpeed {
            label: "1/60".into(),
            value: 1,
        });
        iris_metadata.gain_range = 0..=1;
        iris_metadata.has_iris_control = true;
        iris_metadata.iris_range = Some(0..=1);

        // Discovery metadata alone does not promise a future typed targeted
        // Iris request, so a completionless profile remains valid without the
        // scalar inquiry fact.
        assert!(runtime_builder(iris_metadata.clone()).build().is_ok());

        let mut typed_iris = iris_metadata;
        typed_iris.typed_support = TypedSupportSet::from_surface(TypedSupportSurface::IrisControl);
        assert!(runtime_builder(typed_iris).build().is_err());

        let mut nd_metadata = valid_runtime_capabilities();
        nd_metadata.has_nd_filter = true;
        nd_metadata.nd_filter_mode = capabilities::NdFilterMode::Variable;
        assert!(runtime_builder(nd_metadata.clone()).build().is_ok());

        let mut typed_nd = nd_metadata;
        typed_nd.typed_support = TypedSupportSet::from_surface(TypedSupportSurface::NdFilter);
        assert!(runtime_builder(typed_nd).build().is_err());
    }

    #[test]
    fn runtime_builder_requires_every_protocol_safety_fact() {
        assert!(ProfileSpec::builder(valid_runtime_capabilities())
            .build()
            .is_err());
    }

    #[test]
    fn command_timeout_values_match_defaults_and_registry_facts() {
        let defaults = CommandTimeouts::default();
        assert_eq!(defaults.quick_timeout(), Duration::from_secs(5));
        assert_eq!(defaults.movement_timeout(), Duration::from_secs(30));
        assert_eq!(defaults.preset_timeout(), Duration::from_secs(60));
        assert_eq!(defaults.long_running_timeout(), Duration::from_secs(300));
        assert_eq!(defaults.network_timeout(), Duration::from_secs(5));

        let g2 = ProfileSpec::from_compile_time::<crate::profiles::PtzOpticsG2>()
            .expect("built-in profile");
        assert_eq!(g2.command_timeouts(), defaults);
        let generic = ProfileSpec::from_compile_time::<crate::profiles::GenericVisca>()
            .expect("generic profile");
        assert_eq!(
            generic.command_timeouts(),
            CommandTimeouts::new(
                Duration::from_secs(10),
                Duration::from_secs(30),
                Duration::from_secs(60),
                Duration::from_secs(300),
                Duration::from_secs(10),
            )
        );
    }

    #[test]
    fn profile_timing_builder_requires_all_facts_and_preserves_values() {
        assert!(ProfileTiming::builder().build().is_err());

        let timing = ProfileTiming::builder()
            .ack_timeout(Duration::from_millis(100))
            .command_timeouts(CommandTimeouts::default())
            .inquiry_timeout(Duration::from_secs(1))
            .cancellation_timeout(Duration::from_secs(2))
            .ambiguity_timeout(Duration::from_secs(3))
            .busy_timeout(Duration::from_secs(4))
            .minimum_inquiry_spacing(Duration::from_millis(25))
            .minimum_command_spacing(Duration::from_millis(50))
            .build()
            .expect("complete timing facts");

        assert_eq!(timing.ack_timeout(), Duration::from_millis(100));
        assert_eq!(timing.command_timeouts(), CommandTimeouts::default());
        assert_eq!(timing.inquiry_timeout(), Duration::from_secs(1));
        assert_eq!(timing.cancellation_timeout(), Duration::from_secs(2));
        assert_eq!(timing.ambiguity_timeout(), Duration::from_secs(3));
        assert_eq!(timing.busy_timeout(), Duration::from_secs(4));
        assert_eq!(timing.minimum_inquiry_spacing(), Duration::from_millis(25));
        assert_eq!(timing.minimum_command_spacing(), Duration::from_millis(50));

        assert!(ProfileTiming::builder()
            .ack_timeout(Duration::ZERO)
            .command_timeouts(CommandTimeouts::default())
            .inquiry_timeout(Duration::from_secs(1))
            .cancellation_timeout(Duration::from_secs(1))
            .ambiguity_timeout(Duration::from_secs(1))
            .busy_timeout(Duration::ZERO)
            .minimum_inquiry_spacing(Duration::ZERO)
            .minimum_command_spacing(Duration::ZERO)
            .build()
            .is_err());
    }

    #[test]
    fn profile_timing_rejects_unrepresentable_deadlines_and_spacing() {
        let complete = || {
            ProfileTiming::builder()
                .ack_timeout(Duration::from_millis(100))
                .command_timeouts(CommandTimeouts::default())
                .inquiry_timeout(Duration::from_secs(1))
                .cancellation_timeout(Duration::from_secs(1))
                .ambiguity_timeout(Duration::from_secs(1))
                .busy_timeout(Duration::ZERO)
                .minimum_inquiry_spacing(Duration::ZERO)
                .minimum_command_spacing(Duration::ZERO)
        };

        for result in [
            complete().ack_timeout(Duration::MAX).build(),
            complete().inquiry_timeout(Duration::MAX).build(),
            complete().cancellation_timeout(Duration::MAX).build(),
            complete().ambiguity_timeout(Duration::MAX).build(),
            complete().busy_timeout(Duration::MAX).build(),
            complete().minimum_inquiry_spacing(Duration::MAX).build(),
            complete().minimum_command_spacing(Duration::MAX).build(),
        ] {
            assert!(result.is_err());
        }

        let default = CommandTimeouts::default();
        let command_timeout_cases = [
            CommandTimeouts::new(
                Duration::MAX,
                default.movement_timeout(),
                default.preset_timeout(),
                default.long_running_timeout(),
                default.network_timeout(),
            ),
            CommandTimeouts::new(
                default.quick_timeout(),
                Duration::MAX,
                default.preset_timeout(),
                default.long_running_timeout(),
                default.network_timeout(),
            ),
            CommandTimeouts::new(
                default.quick_timeout(),
                default.movement_timeout(),
                Duration::MAX,
                default.long_running_timeout(),
                default.network_timeout(),
            ),
            CommandTimeouts::new(
                default.quick_timeout(),
                default.movement_timeout(),
                default.preset_timeout(),
                Duration::MAX,
                default.network_timeout(),
            ),
            CommandTimeouts::new(
                default.quick_timeout(),
                default.movement_timeout(),
                default.preset_timeout(),
                default.long_running_timeout(),
                Duration::MAX,
            ),
        ];
        for command_timeouts in command_timeout_cases {
            assert!(complete()
                .command_timeouts(command_timeouts)
                .build()
                .is_err());
        }
    }

    #[test]
    fn ordinary_five_minute_long_running_deadline_remains_valid() {
        let timing = ProfileTiming::builder()
            .ack_timeout(Duration::from_millis(100))
            .command_timeouts(CommandTimeouts::new(
                Duration::from_secs(5),
                Duration::from_secs(30),
                Duration::from_secs(60),
                Duration::from_secs(300),
                Duration::from_secs(5),
            ))
            .inquiry_timeout(Duration::from_secs(1))
            .cancellation_timeout(Duration::from_secs(1))
            .ambiguity_timeout(Duration::from_secs(1))
            .busy_timeout(Duration::ZERO)
            .minimum_inquiry_spacing(Duration::ZERO)
            .minimum_command_spacing(Duration::ZERO)
            .build()
            .expect("ordinary five-minute LongRunning deadline is representable");

        assert_eq!(
            timing.command_timeouts().long_running_timeout(),
            Duration::from_secs(300)
        );
    }

    #[test]
    fn runtime_command_timeout_values_are_explicit_and_nonzero_validated() {
        let exact = CommandTimeouts::new(
            Duration::from_secs(6),
            Duration::from_secs(7),
            Duration::from_secs(8),
            Duration::from_secs(9),
            Duration::from_secs(10),
        );
        let profile = runtime_builder(valid_runtime_capabilities())
            .timing(
                ProfileTiming::builder()
                    .ack_timeout(Duration::from_millis(100))
                    .command_timeouts(exact)
                    .inquiry_timeout(Duration::from_secs(1))
                    .cancellation_timeout(Duration::from_secs(1))
                    .ambiguity_timeout(Duration::from_secs(1))
                    .busy_timeout(Duration::ZERO)
                    .minimum_inquiry_spacing(Duration::from_millis(25))
                    .minimum_command_spacing(Duration::from_millis(25))
                    .build()
                    .expect("valid timing"),
            )
            .build()
            .expect("exact runtime category values");
        assert_eq!(profile.command_timeouts(), exact);

        assert!(CommandTimeouts::new(
            Duration::ZERO,
            Duration::from_secs(1),
            Duration::from_secs(1),
            Duration::from_secs(1),
            Duration::from_secs(1),
        )
        .validate()
        .is_err());
    }

    #[test]
    fn tuning_cannot_weaken_pacing_or_raise_socket_capacity() {
        let profile = runtime_builder(valid_runtime_capabilities())
            .build()
            .expect("valid runtime profile");
        assert!(profile
            .validate_tuning(OperationalTuning::new().command_spacing(Duration::from_millis(1)))
            .is_err());
        assert!(profile
            .validate_tuning(OperationalTuning::new().maximum_command_sockets(2))
            .is_err());
        assert!(profile
            .validate_tuning(OperationalTuning::new().ack_timeout(Duration::from_nanos(1)))
            .is_err());
        assert!(profile
            .validate_tuning(OperationalTuning::new().inquiry_timeout(Duration::from_nanos(1)))
            .is_err());
        for tuning in [
            OperationalTuning::new().quick_timeout(Duration::from_nanos(1)),
            OperationalTuning::new().movement_timeout(Duration::from_nanos(1)),
            OperationalTuning::new().preset_timeout(Duration::from_nanos(1)),
            OperationalTuning::new().long_running_timeout(Duration::from_nanos(1)),
            OperationalTuning::new().network_timeout(Duration::from_nanos(1)),
        ] {
            assert!(profile.validate_tuning(tuning).is_err());
        }
        assert!(profile
            .validate_tuning(OperationalTuning::new().quick_timeout(Duration::ZERO))
            .is_err());
    }

    #[test]
    fn tuning_rejects_unrepresentable_deadline_and_spacing_overrides() {
        let profile = runtime_builder(valid_runtime_capabilities())
            .build()
            .expect("valid runtime profile");

        for tuning in [
            OperationalTuning::new().command_spacing(Duration::MAX),
            OperationalTuning::new().inquiry_spacing(Duration::MAX),
            OperationalTuning::new().ack_timeout(Duration::MAX),
            OperationalTuning::new().quick_timeout(Duration::MAX),
            OperationalTuning::new().movement_timeout(Duration::MAX),
            OperationalTuning::new().preset_timeout(Duration::MAX),
            OperationalTuning::new().long_running_timeout(Duration::MAX),
            OperationalTuning::new().network_timeout(Duration::MAX),
            OperationalTuning::new().settlement_timeout(Duration::MAX),
            OperationalTuning::new().inquiry_timeout(Duration::MAX),
        ] {
            assert!(profile.validate_tuning(tuning).is_err());
        }
    }

    /// Issue #636: an absurd retry budget used to invert into *fewer* retries
    /// than the default, because the engine's `submitted_at + budget` deadline
    /// saturates back to `submitted_at` and reads as already spent. Validation
    /// rejects the value instead of letting it silently mean its opposite.
    #[test]
    fn absurd_retry_timing_is_rejected_before_it_can_invert_into_zero_retries() {
        let profile = runtime_builder(valid_runtime_capabilities())
            .build()
            .expect("valid runtime profile");
        let sane = OperationalTuning::new().retry_timing(
            Duration::from_millis(50),
            Duration::from_millis(500),
            Duration::from_secs(10),
        );
        assert!(profile.validate_tuning(sane).is_ok());

        for budget in [
            Duration::MAX,
            Duration::MAX - Duration::from_secs(1),
            MAXIMUM_RETRY_TIMING + Duration::from_nanos(1),
        ] {
            let inverted = OperationalTuning::new().retry_timing(
                Duration::from_millis(50),
                Duration::from_millis(500),
                budget,
            );
            assert!(
                profile.validate_tuning(inverted).is_err(),
                "a {budget:?} retry budget must be rejected, not saturated into zero retries"
            );
        }

        // The same saturation turns an absurd ceiling into no backoff at all.
        assert!(profile
            .validate_tuning(OperationalTuning::new().retry_timing(
                Duration::from_millis(50),
                Duration::MAX,
                Duration::from_secs(10),
            ))
            .is_err());
        // Exactly at the bound is still accepted.
        assert!(profile
            .validate_tuning(OperationalTuning::new().retry_timing(
                Duration::from_millis(50),
                MAXIMUM_RETRY_TIMING,
                MAXIMUM_RETRY_TIMING,
            ))
            .is_ok());
    }

    /// The engine's own deadline arithmetic is what makes the bound necessary:
    /// an unbounded budget saturates instead of extending.
    #[test]
    fn the_retry_budget_bound_is_below_instant_addition_saturation() {
        let submitted = std::time::Instant::now();
        assert!(
            submitted.checked_add(MAXIMUM_RETRY_TIMING).is_some(),
            "the accepted maximum must still produce a real deadline"
        );
        assert!(
            submitted.checked_add(Duration::MAX).is_none(),
            "an unbounded budget saturates, which is exactly the inversion"
        );
    }

    #[test]
    fn compile_time_projection_requires_full_profile_equality() {
        let base = ProfileSpec::from_compile_time::<crate::profiles::PtzOpticsG2>()
            .expect("built-in profile");
        let coordinates = base.pan_tilt_coordinates().expect("coordinates");
        let timing = base.timing();
        let mut altered_capabilities = base.capabilities().clone();
        altered_capabilities.profile_id = None;
        let altered = ProfileSpec::builder(altered_capabilities)
            .pan_tilt_coordinates(
                coordinates.coordinate_system(),
                coordinates.pan_degrees_to_units(),
                coordinates.tilt_degrees_to_units(),
            )
            .transports(base.transports())
            .envelope(base.envelope())
            .timing(
                ProfileTiming::builder()
                    .ack_timeout(timing.ack_timeout())
                    .command_timeouts(timing.command_timeouts())
                    .inquiry_timeout(timing.inquiry_timeout())
                    .cancellation_timeout(timing.cancellation_timeout())
                    .ambiguity_timeout(timing.ambiguity_timeout())
                    .busy_timeout(timing.busy_timeout())
                    .minimum_inquiry_spacing(timing.minimum_inquiry_spacing())
                    .minimum_command_spacing(
                        timing.minimum_command_spacing() + Duration::from_millis(1),
                    )
                    .build()
                    .expect("valid altered timing"),
            )
            .maximum_command_sockets(base.maximum_command_sockets())
            .supports_operation_complete(base.supports_operation_complete())
            .supports_command_cancel(base.supports_command_cancel())
            .preset_recall_axes(base.preset_recall_axes())
            .position_inquiries(base.position_inquiries())
            .build()
            .expect("altered runtime profile");

        assert_ne!(altered, base);
        assert!(altered
            .ensure_compile_time::<crate::profiles::PtzOpticsG2>()
            .is_err());
    }

    #[test]
    fn built_in_identity_covers_non_capability_profile_facts() {
        let base =
            ProfileSpec::from_compile_time::<crate::profiles::SonyFR7>().expect("built-in profile");
        let coordinates = base.pan_tilt_coordinates().expect("coordinates");

        let rebuild = |envelope, coordinate_system, supports_command_cancel| {
            ProfileSpec::builder(base.capabilities().clone())
                .pan_tilt_coordinates(
                    coordinate_system,
                    coordinates.pan_degrees_to_units(),
                    coordinates.tilt_degrees_to_units(),
                )
                .transports(base.transports())
                .envelope(envelope)
                .timing(base.timing())
                .maximum_command_sockets(base.maximum_command_sockets())
                .supports_operation_complete(base.supports_operation_complete())
                .supports_command_cancel(supports_command_cancel)
                .preset_recall_axes(base.preset_recall_axes())
                .position_inquiries(base.position_inquiries())
                .build()
        };

        assert!(rebuild(
            base.envelope(),
            coordinates.coordinate_system(),
            base.supports_command_cancel(),
        )
        .is_ok());
        assert!(rebuild(
            ProfileEnvelope::RawVisca,
            coordinates.coordinate_system(),
            base.supports_command_cancel(),
        )
        .is_err());
        assert!(rebuild(
            base.envelope(),
            match coordinates.coordinate_system() {
                capabilities::CoordinateSystem::SignedCentered => {
                    capabilities::CoordinateSystem::UnsignedCentered
                }
                capabilities::CoordinateSystem::UnsignedCentered => {
                    capabilities::CoordinateSystem::SignedCentered
                }
            },
            base.supports_command_cancel(),
        )
        .is_err());
        assert!(rebuild(
            base.envelope(),
            coordinates.coordinate_system(),
            !base.supports_command_cancel(),
        )
        .is_err());
    }

    #[test]
    fn runtime_builder_rejects_exposure_image_and_typed_contradictions() {
        let mut exposure = valid_runtime_capabilities();
        exposure.typed_support = TypedSupportSet::from_surface(TypedSupportSurface::IrisControl);
        assert!(runtime_builder(exposure).build().is_err());

        let mut image = valid_runtime_capabilities();
        image.has_image_processing = true;
        image.contrast_range = Some(std::ops::RangeInclusive::new(10, 1));
        assert!(runtime_builder(image).build().is_err());

        let mut typed = valid_runtime_capabilities();
        typed.typed_support = TypedSupportSet::from_surface(TypedSupportSurface::DirectZoom);
        typed.supports_direct_zoom = false;
        assert!(runtime_builder(typed).build().is_err());

        let mut focus_zone_inquiry = valid_runtime_capabilities();
        focus_zone_inquiry.typed_support =
            TypedSupportSet::from_surface(TypedSupportSurface::FocusZoneInquiry);
        assert!(runtime_builder(focus_zone_inquiry).build().is_err());

        let mut usb_audio = valid_runtime_capabilities();
        usb_audio.typed_support = TypedSupportSet::from_surface(TypedSupportSurface::UsbAudio);
        assert!(runtime_builder(usb_audio).build().is_err());
    }

    #[test]
    fn runtime_builder_rejects_typed_white_balance_modes_missing_from_inventory() {
        let mut one_push = runtime_white_balance_capabilities(vec![crate::WhiteBalanceMode::Auto]);
        one_push.has_one_push_wb = true;
        one_push.typed_support =
            TypedSupportSet::from_surface(TypedSupportSurface::OnePushWhiteBalance);
        assert!(runtime_builder(one_push).build().is_err());

        let mut atw = runtime_white_balance_capabilities(vec![crate::WhiteBalanceMode::Auto]);
        atw.typed_support =
            TypedSupportSet::from_surface(TypedSupportSurface::AutoTrackingWhiteBalance);
        assert!(runtime_builder(atw).build().is_err());

        let mut color_temperature =
            runtime_white_balance_capabilities(vec![crate::WhiteBalanceMode::Manual]);
        color_temperature.has_color_temp = true;
        color_temperature.color_temp_range = Some(2_500..=8_000);
        color_temperature.typed_support =
            TypedSupportSet::from_surface(TypedSupportSurface::ColorTemperature);
        assert!(runtime_builder(color_temperature).build().is_err());
    }

    #[test]
    fn runtime_builder_accepts_coherent_partial_typed_white_balance_profiles() {
        let mut one_push =
            runtime_white_balance_capabilities(vec![crate::WhiteBalanceMode::OnePush]);
        one_push.has_one_push_wb = true;
        one_push.typed_support =
            TypedSupportSet::from_surface(TypedSupportSurface::OnePushWhiteBalance);
        assert!(runtime_builder(one_push).build().is_ok());

        let mut atw = runtime_white_balance_capabilities(vec![crate::WhiteBalanceMode::ATW]);
        atw.typed_support =
            TypedSupportSet::from_surface(TypedSupportSurface::AutoTrackingWhiteBalance);
        assert!(runtime_builder(atw).build().is_ok());

        let mut color_temperature =
            runtime_white_balance_capabilities(vec![crate::WhiteBalanceMode::ColorTemperature]);
        color_temperature.has_color_temp = true;
        color_temperature.color_temp_range = Some(2_500..=8_000);
        color_temperature.typed_support =
            TypedSupportSet::from_surface(TypedSupportSurface::ColorTemperature);
        assert!(runtime_builder(color_temperature).build().is_ok());

        // The metadata can describe an independent one-push trigger without
        // granting the typed mode-selection surface.
        let mut trigger_only =
            runtime_white_balance_capabilities(vec![crate::WhiteBalanceMode::Auto]);
        trigger_only.has_one_push_wb = true;
        assert!(runtime_builder(trigger_only).build().is_ok());
    }

    #[test]
    fn runtime_builder_rejects_parent_domain_and_aggregate_contradictions() {
        let mut focus = valid_runtime_capabilities();
        focus.has_focus = false;
        focus.focus_range = 0..=0;
        focus.focus_speed = 0..=0;
        focus.has_auto_focus = true;
        focus.has_one_push_focus = false;
        focus.has_focus_zone = false;
        focus.has_af_sensitivity = false;
        focus.has_focus_near_limit_inquiry = false;
        assert!(runtime_builder(focus).build().is_err());

        let mut preset = valid_runtime_capabilities();
        preset.has_presets = false;
        preset.max_presets = 0;
        preset.preset_speed_range = 0..=0;
        preset.supports_preset_tour = true;
        assert!(runtime_builder(preset).build().is_err());

        let mut power = valid_runtime_capabilities();
        power.supports_standby = true;
        assert!(runtime_builder(power).build().is_err());

        let mut exposure = valid_runtime_capabilities();
        exposure.exposure_modes.push(crate::ExposureMode::Auto);
        assert!(runtime_builder(exposure).build().is_err());

        let mut white_balance = valid_runtime_capabilities();
        white_balance
            .white_balance_modes
            .push(crate::WhiteBalanceMode::Auto);
        assert!(runtime_builder(white_balance).build().is_err());

        let mut nd_filter = valid_runtime_capabilities();
        nd_filter.nd_filter_mode = capabilities::NdFilterMode::Variable;
        assert!(runtime_builder(nd_filter).build().is_err());

        let mut motion = valid_runtime_capabilities();
        motion.max_motion_sync_speed = Some(24);
        assert!(runtime_builder(motion).build().is_err());
    }

    #[test]
    fn runtime_builder_rejects_incoherent_exact_inventory_facts() {
        let mut converter = valid_runtime_capabilities();
        converter.pan_range_degrees = -9.0..=10.0;
        assert!(runtime_builder(converter).build().is_err());

        let mut shutter = valid_runtime_capabilities();
        shutter.has_exposure = true;
        shutter.exposure_modes = vec![crate::ExposureMode::Auto];
        shutter.shutter_speeds = vec![
            capabilities::RuntimeShutterSpeed {
                label: "1/60".to_owned(),
                value: 1,
            },
            capabilities::RuntimeShutterSpeed {
                label: "1/60".to_owned(),
                value: 2,
            },
        ];
        shutter.gain_range = 0..=1;
        assert!(runtime_builder(shutter).build().is_err());

        let mut white_balance = valid_runtime_capabilities();
        white_balance.has_white_balance = true;
        white_balance.white_balance_modes =
            vec![crate::WhiteBalanceMode::Auto, crate::WhiteBalanceMode::Auto];
        assert!(runtime_builder(white_balance).build().is_err());

        let mut nd_filter = valid_runtime_capabilities();
        nd_filter.has_nd_filter = true;
        nd_filter.nd_filter_mode = capabilities::NdFilterMode::Stepped(3);
        nd_filter.nd_filter_steps = Some(2);
        assert!(runtime_builder(nd_filter).build().is_err());

        let mut combined_flip = valid_runtime_capabilities();
        combined_flip.has_image_processing = true;
        combined_flip.supports_flip = true;
        combined_flip.uses_combined_flip_command = true;
        assert!(runtime_builder(combined_flip).build().is_err());

        let mut combined_without_strategy = valid_runtime_capabilities();
        combined_without_strategy.has_image_processing = true;
        combined_without_strategy.supports_flip = true;
        combined_without_strategy.supports_mirror = true;
        combined_without_strategy.typed_support =
            TypedSupportSet::from_surface(TypedSupportSurface::CombinedImageFlip);
        assert!(runtime_builder(combined_without_strategy).build().is_err());

        let mut strategy_without_combined = valid_runtime_capabilities();
        strategy_without_combined.has_image_processing = true;
        strategy_without_combined.supports_flip = true;
        strategy_without_combined.supports_mirror = true;
        strategy_without_combined.uses_combined_flip_command = true;
        assert!(runtime_builder(strategy_without_combined).build().is_err());

        let mut rgb_flag_without_ranges = valid_runtime_capabilities();
        rgb_flag_without_ranges.has_white_balance = true;
        rgb_flag_without_ranges.white_balance_modes = vec![crate::WhiteBalanceMode::Auto];
        rgb_flag_without_ranges.has_rgb_gain = true;
        assert!(runtime_builder(rgb_flag_without_ranges).build().is_err());

        let mut rgb_ranges_without_flag = valid_runtime_capabilities();
        rgb_ranges_without_flag.has_white_balance = true;
        rgb_ranges_without_flag.white_balance_modes = vec![crate::WhiteBalanceMode::Auto];
        rgb_ranges_without_flag.red_gain_range = Some(0..=255);
        rgb_ranges_without_flag.blue_gain_range = Some(0..=255);
        assert!(runtime_builder(rgb_ranges_without_flag).build().is_err());

        let mut rgb_typed_without_ranges = valid_runtime_capabilities();
        rgb_typed_without_ranges.has_white_balance = true;
        rgb_typed_without_ranges.white_balance_modes = vec![crate::WhiteBalanceMode::Auto];
        rgb_typed_without_ranges.has_rgb_gain = true;
        rgb_typed_without_ranges.typed_support =
            TypedSupportSet::from_surface(TypedSupportSurface::RgbGain);
        assert!(runtime_builder(rgb_typed_without_ranges).build().is_err());

        let mut preset_names = valid_runtime_capabilities();
        preset_names.supports_preset_names = true;
        preset_names.max_preset_name_length = 0;
        assert!(runtime_builder(preset_names).build().is_err());
    }

    #[test]
    fn absent_motion_domains_require_exact_conservative_facts() {
        type Mutation = (&'static str, fn(&mut Capabilities));
        let mutations: &[Mutation] = &[
            ("pan speed", |caps| caps.pan_speed = 0..=1),
            ("tilt speed", |caps| caps.tilt_speed = 0..=1),
            ("pan units", |caps| caps.pan_range = -1..=0),
            ("tilt units", |caps| caps.tilt_range = 0..=1),
            ("pan degrees", |caps| caps.pan_range_degrees = -1.0..=0.0),
            ("tilt degrees", |caps| caps.tilt_range_degrees = 0.0..=1.0),
            ("simultaneous", |caps| caps.pan_tilt_simultaneous = true),
            ("preset recovery", |caps| {
                caps.preset_recovery_time = Duration::from_millis(1);
            }),
            ("optical range", |caps| caps.zoom_range_optical = 0..=1),
            ("digital flag", |caps| caps.has_digital_zoom = true),
            ("digital range", |caps| {
                caps.zoom_range_digital = Some(0..=1)
            }),
            ("zoom speed", |caps| caps.zoom_speed = 0..=1),
            ("direct zoom", |caps| caps.supports_direct_zoom = true),
            ("variable zoom", |caps| caps.supports_variable_zoom = true),
            ("zoom converter", |caps| {
                caps.zoom_magnification_to_units = 2.0
            }),
        ];
        let baseline =
            Capabilities::runtime_baseline("Conservative downstream camera", 1).expect("baseline");
        assert!(conservative_runtime_builder(baseline.clone())
            .build()
            .is_ok());
        assert!(conservative_runtime_builder(baseline.clone())
            .pan_tilt_coordinates(capabilities::CoordinateSystem::SignedCentered, 1.0, 1.0)
            .build()
            .is_err());
        for &(name, mutate) in mutations {
            let mut capabilities = baseline.clone();
            mutate(&mut capabilities);
            assert!(
                conservative_runtime_builder(capabilities).build().is_err(),
                "accepted non-conservative {name} without its parent domain"
            );
        }
    }

    #[test]
    fn zoom_ranges_are_zero_based_and_have_an_exact_digital_boundary() {
        let mut nonzero_optical = valid_runtime_capabilities();
        nonzero_optical.zoom_range_optical = 1..=1_000;
        assert!(runtime_builder(nonzero_optical).build().is_err());

        let mut digital_gap = valid_runtime_capabilities();
        digital_gap.has_digital_zoom = true;
        digital_gap.zoom_range_digital = Some(1_001..=2_000);
        assert!(runtime_builder(digital_gap).build().is_err());

        let mut digital_overlap = valid_runtime_capabilities();
        digital_overlap.has_digital_zoom = true;
        digital_overlap.zoom_range_digital = Some(999..=2_000);
        assert!(runtime_builder(digital_overlap).build().is_err());
    }

    #[test]
    fn transport_ports_fill_from_the_authority_and_explicit_mismatches_fail() {
        let baseline =
            Capabilities::runtime_baseline("Runtime transport camera", 1).expect("baseline");
        let profile = conservative_runtime_builder(baseline.clone())
            .build()
            .expect("transport setter fills an unspecified capability mirror");
        assert_eq!(profile.capabilities().profile_id, None);
        assert_eq!(profile.capabilities().default_tcp_port, Some(5678));
        assert_eq!(profile.transports().tcp_port(), Some(5678));

        let mut mismatched = baseline;
        mismatched.default_tcp_port = Some(9_999);
        assert!(conservative_runtime_builder(mismatched).build().is_err());
    }

    #[test]
    fn no_motion_power_only_runtime_profile_is_valid() {
        let mut capabilities =
            Capabilities::runtime_baseline("Power-only downstream camera", 1).expect("baseline");
        capabilities.has_power = true;
        capabilities.supports_standby = true;
        capabilities.supports_wake_on_lan = true;
        capabilities.power_on_time = Duration::from_millis(750);
        capabilities.standby_time = Duration::from_millis(250);
        capabilities.retains_settings_on_power_off = true;

        let profile = ProfileSpec::builder(capabilities)
            .transports(TransportCompatibility::new(Some(5678), None, false))
            .envelope(ProfileEnvelope::RawVisca)
            .timing(
                ProfileTiming::builder()
                    .ack_timeout(Duration::from_millis(100))
                    .command_timeouts(CommandTimeouts::default())
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
            .expect("power-only profile");

        assert!(profile.preset_recall_axes().is_none());
        assert!(profile.pan_tilt_coordinates().is_none());
    }

    #[test]
    fn sony_envelope_requires_ip_only_transport_compatibility() {
        assert!(runtime_builder(valid_runtime_capabilities())
            .transports(TransportCompatibility::new(None, None, true))
            .envelope(ProfileEnvelope::SonyEncapsulated)
            .build()
            .is_err());
        assert!(runtime_builder(valid_runtime_capabilities())
            .transports(TransportCompatibility::new(Some(52381), None, true))
            .envelope(ProfileEnvelope::SonyEncapsulated)
            .build()
            .is_err());
        assert!(runtime_builder(valid_runtime_capabilities())
            .transports(TransportCompatibility::new(Some(52381), None, false))
            .envelope(ProfileEnvelope::SonyEncapsulated)
            .build()
            .is_ok());
    }

    #[test]
    fn all_builtin_profiles_lower_from_registry_facts() {
        macro_rules! assert_profile {
            ($profile:ty) => {
                let result = ProfileSpec::from_compile_time::<$profile>();
                assert!(result.is_ok(), "{}: {result:?}", stringify!($profile));
            };
        }
        assert_profile!(crate::profiles::PtzOpticsG2);
        assert_profile!(crate::profiles::PtzOpticsG3);
        assert_profile!(crate::profiles::PtzOptics30X);
        assert_profile!(crate::profiles::SonyFR7);
        assert_profile!(crate::profiles::SonyBRCH900);
        assert_profile!(crate::profiles::SonyEVIH100);
        assert_profile!(crate::profiles::SonyBRC300);
        assert_profile!(crate::profiles::NearusBRC300);
        assert_profile!(crate::profiles::GenericVisca);
    }

    #[cfg(feature = "serde")]
    #[test]
    fn profile_deserialization_revalidates_invariants() {
        let profile = runtime_builder(valid_runtime_capabilities())
            .build()
            .expect("valid runtime profile");
        let mut value = serde_json::to_value(profile).expect("serialize profile");
        value["maximum_command_sockets"] = serde_json::json!(9);
        assert!(serde_json::from_value::<ProfileSpec>(value).is_err());
    }
}
