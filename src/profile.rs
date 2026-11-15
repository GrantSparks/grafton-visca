//! Validated runtime camera-profile specifications.

use std::{
    fmt,
    time::{Duration, Instant},
};

use crate::{
    capabilities,
    timeout::{CommandCategory, CommandTimeouts},
    AffectedAxes, Error, Result,
};

/// Builds one actionable profile-construction error. Validation can involve
/// several coupled facts, so plural field names are retained instead of
/// collapsing the failure into an anonymous prose invariant.
pub(crate) fn invalid_profile_fields(fields: &[&str], reason: impl fmt::Display) -> Error {
    let names = fields
        .iter()
        .map(|field| format!("`{field}`"))
        .collect::<Vec<_>>()
        .join(", ");
    let label = if fields.len() == 1 { "field" } else { "fields" };
    Error::InvalidRequest(format!("profile {label} {names}: {reason}").into())
}

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

/// The derived budget covers twice the response window and the full attempt
/// (ACK plus completion for a command). Check their combined deadline rather
/// than accepting individually representable inputs that overflow together.
fn retry_budget_is_representable(now: Instant, deadline: Duration, ack: Duration) -> bool {
    deadline
        .checked_mul(2)
        .zip(deadline.checked_add(ack))
        .is_some_and(|(budget, attempt)| {
            monotonic_duration_is_representable(now, budget.max(attempt))
        })
}

/// Position inquiries available for profile-selected protocol settlement.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
#[cfg_attr(feature = "ts-rs", derive(ts_rs::TS), ts(export))]
#[cfg_attr(feature = "serde", serde(deny_unknown_fields))]
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
#[cfg_attr(feature = "serde", serde(deny_unknown_fields))]
pub struct TransportCompatibility {
    #[cfg_attr(
        feature = "serde",
        serde(deserialize_with = "crate::profile::required_option")
    )]
    #[cfg_attr(feature = "schemars", schemars(required, transform = nullable_schema))]
    tcp_port: Option<u16>,
    #[cfg_attr(
        feature = "serde",
        serde(deserialize_with = "crate::profile::required_option")
    )]
    #[cfg_attr(feature = "schemars", schemars(required, transform = nullable_schema))]
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
#[cfg_attr(feature = "serde", serde(deny_unknown_fields))]
#[cfg_attr(feature = "serde", serde(try_from = "ProfileTimingSerde"))]
pub struct ProfileTiming {
    ack_timeout: Duration,
    command_timeouts: CommandTimeouts,
    inquiry_timeout: Duration,
    cancellation_timeout: Duration,
    ambiguity_timeout: Duration,
    busy_timeout: Duration,
    raw_inquiry_reply_skew: Duration,
    minimum_inquiry_spacing: Duration,
    minimum_command_spacing: Duration,
}

/// Persisted timing facts before the builder's invariant is established.
#[cfg(feature = "serde")]
#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
struct ProfileTimingSerde {
    ack_timeout: Duration,
    command_timeouts: CommandTimeouts,
    inquiry_timeout: Duration,
    cancellation_timeout: Duration,
    ambiguity_timeout: Duration,
    busy_timeout: Duration,
    raw_inquiry_reply_skew: Duration,
    minimum_inquiry_spacing: Duration,
    minimum_command_spacing: Duration,
}

#[cfg(feature = "serde")]
impl TryFrom<ProfileTimingSerde> for ProfileTiming {
    type Error = Error;

    fn try_from(value: ProfileTimingSerde) -> Result<Self> {
        Self {
            ack_timeout: value.ack_timeout,
            command_timeouts: value.command_timeouts,
            inquiry_timeout: value.inquiry_timeout,
            cancellation_timeout: value.cancellation_timeout,
            ambiguity_timeout: value.ambiguity_timeout,
            busy_timeout: value.busy_timeout,
            raw_inquiry_reply_skew: value.raw_inquiry_reply_skew,
            minimum_inquiry_spacing: value.minimum_inquiry_spacing,
            minimum_command_spacing: value.minimum_command_spacing,
        }
        .validate()
    }
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
    raw_inquiry_reply_skew: Option<Duration>,
    minimum_inquiry_spacing: Option<Duration>,
    minimum_command_spacing: Option<Duration>,
}

/// Profile-specified pan/tilt coordinate conversion owned by a validated profile.
///
/// This is the crate's single implementation of the degrees ↔ raw-unit
/// conversion. Request preparation, [`PanTiltExt`], the position types'
/// profile-aware helpers, and the capability degree ranges all delegate here,
/// so every path produces the same units for the same angle.
///
/// The conversion rule, for each axis with its signed units-per-degree scale:
///
/// * degrees → units: `degrees × scale` in `f32`, rounded to the nearest
///   unit with halves rounded away from zero. Non-finite input, or a result
///   outside `i32`, has no unit value.
/// * units → degrees: `units ÷ scale` in `f32`.
///
/// For example, with the PTZOptics G2 profile's 14.4 units per degree,
/// `10.05°` is `145` units.
///
/// Deserialization rejects a zero or non-finite scale.
///
/// [`PanTiltExt`]: crate::capabilities::pan_tilt::PanTiltExt
#[derive(Debug, Clone, Copy, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(
    feature = "serde",
    serde(try_from = "PanTiltCoordinateConversionSerde")
)]
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
#[cfg_attr(feature = "serde", serde(deny_unknown_fields))]
pub struct PanTiltCoordinateConversion {
    coordinate_system: capabilities::CoordinateSystem,
    wire_codec: capabilities::PanTiltWireCodec,
    pan_degrees_to_units: f32,
    tilt_degrees_to_units: f32,
}

/// Unvalidated wire form of [`PanTiltCoordinateConversion`].
#[cfg(feature = "serde")]
#[derive(serde::Deserialize)]
#[serde(deny_unknown_fields)]
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
struct PanTiltCoordinateConversionSerde {
    coordinate_system: capabilities::CoordinateSystem,
    wire_codec: capabilities::PanTiltWireCodec,
    pan_degrees_to_units: f32,
    tilt_degrees_to_units: f32,
}

#[cfg(feature = "serde")]
impl TryFrom<PanTiltCoordinateConversionSerde> for PanTiltCoordinateConversion {
    type Error = Error;

    fn try_from(value: PanTiltCoordinateConversionSerde) -> Result<Self> {
        Self::new(
            value.coordinate_system,
            value.wire_codec,
            value.pan_degrees_to_units,
            value.tilt_degrees_to_units,
        )
    }
}

impl PanTiltCoordinateConversion {
    /// Creates validated coordinate and framing facts for a runtime domain.
    pub fn new(
        coordinate_system: capabilities::CoordinateSystem,
        wire_codec: capabilities::PanTiltWireCodec,
        pan_degrees_to_units: f32,
        tilt_degrees_to_units: f32,
    ) -> Result<Self> {
        if !pan_degrees_to_units.is_finite()
            || pan_degrees_to_units == 0.0
            || !tilt_degrees_to_units.is_finite()
            || tilt_degrees_to_units == 0.0
        {
            return Err(invalid_profile_fields(
                &["capabilities.pan_tilt.coordinates"],
                "units-per-degree scales must be finite and nonzero",
            ));
        }
        crate::command::pan_tilt::PanTiltFraming::new(wire_codec, coordinate_system)?;
        Ok(Self {
            coordinate_system,
            wire_codec,
            pan_degrees_to_units,
            tilt_degrees_to_units,
        })
    }

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
    /// down while the library's positive degree convention is up.
    #[must_use]
    pub const fn tilt_degrees_to_units(self) -> f32 {
        self.tilt_degrees_to_units
    }

    /// Returns the conversion a compile-time profile declares.
    ///
    /// A profile whose wire codec cannot frame its coordinate system (Sony
    /// BRC-300 framing with unsigned-centered coordinates) fails to build at
    /// this call; `tests/api_contract/fail/inconsistent_pan_tilt_framing_fails_to_build.rs`
    /// pins the error.
    #[must_use]
    pub const fn for_profile<P: capabilities::PanTilt + ?Sized>() -> Self {
        const {
            assert!(
                P::PAN_DEGREES_TO_UNITS.is_finite()
                    && P::PAN_DEGREES_TO_UNITS != 0.0
                    && P::TILT_DEGREES_TO_UNITS.is_finite()
                    && P::TILT_DEGREES_TO_UNITS != 0.0,
                "pan/tilt coordinate scales must be finite and nonzero"
            );
            assert!(
                crate::command::pan_tilt::PanTiltFraming::is_consistent(
                    P::PAN_TILT_WIRE_CODEC,
                    P::COORDINATE_SYSTEM,
                ),
                "Sony position framing requires signed-centered coordinates",
            );
        }
        Self {
            coordinate_system: P::COORDINATE_SYSTEM,
            wire_codec: P::PAN_TILT_WIRE_CODEC,
            pan_degrees_to_units: P::PAN_DEGREES_TO_UNITS,
            tilt_degrees_to_units: P::TILT_DEGREES_TO_UNITS,
        }
    }

    /// Converts a pan angle to raw units, or `None` when the angle is not
    /// finite or the result does not fit `i32`.
    #[must_use]
    pub fn pan_units(self, degrees: f32) -> Option<i32> {
        degrees_to_units(degrees, self.pan_degrees_to_units)
    }

    /// Converts a tilt angle to raw units, or `None` when the angle is not
    /// finite or the result does not fit `i32`.
    #[must_use]
    pub fn tilt_units(self, degrees: f32) -> Option<i32> {
        degrees_to_units(degrees, self.tilt_degrees_to_units)
    }

    /// Converts raw pan units to degrees.
    #[must_use]
    pub fn pan_degrees(self, units: i32) -> f32 {
        units as f32 / self.pan_degrees_to_units
    }

    /// Converts raw tilt units to degrees.
    #[must_use]
    pub fn tilt_degrees(self, units: i32) -> f32 {
        units as f32 / self.tilt_degrees_to_units
    }

    /// Returns the ordered degree range covered by a raw pan unit range.
    ///
    /// A negative scale reverses the endpoints, so the result is always
    /// ordered in the library's degree convention.
    #[must_use]
    pub fn pan_degree_range(
        self,
        units: &std::ops::RangeInclusive<i32>,
    ) -> std::ops::RangeInclusive<f32> {
        ordered_degrees(
            self.pan_degrees(*units.start()),
            self.pan_degrees(*units.end()),
        )
    }

    /// Returns the ordered degree range covered by a raw tilt unit range.
    ///
    /// A negative scale reverses the endpoints, so the result is always
    /// ordered in the library's degree convention.
    #[must_use]
    pub fn tilt_degree_range(
        self,
        units: &std::ops::RangeInclusive<i32>,
    ) -> std::ops::RangeInclusive<f32> {
        ordered_degrees(
            self.tilt_degrees(*units.start()),
            self.tilt_degrees(*units.end()),
        )
    }

    /// Converts a raw position to degrees.
    #[must_use]
    pub fn to_degrees(
        self,
        position: crate::camera::PanTiltPosition,
    ) -> crate::inquiry_conversions::PanTiltPositionDeg {
        crate::inquiry_conversions::PanTiltPositionDeg::new(
            crate::units::Degrees(self.pan_degrees(position.pan)),
            crate::units::Degrees(self.tilt_degrees(position.tilt)),
        )
    }

    /// Converts a position in degrees to raw units, or `None` when either
    /// axis has no unit value. Range validation is the caller's profile check.
    #[must_use]
    pub fn to_units(
        self,
        position: crate::inquiry_conversions::PanTiltPositionDeg,
    ) -> Option<crate::camera::PanTiltPosition> {
        Some(crate::camera::PanTiltPosition::new(
            self.pan_units(position.pan.0)?,
            self.tilt_units(position.tilt.0)?,
        ))
    }
}

/// Effective source ranges selected by a target, without duplicate degree facts.
#[derive(Clone, Copy)]
pub(crate) struct PanTiltRanges {
    pub(crate) pan_units: capabilities::CapabilityRange<i32>,
    pub(crate) tilt_units: capabilities::CapabilityRange<i32>,
    pub(crate) endpoint_quantization: capabilities::EndpointQuantization,
}

#[derive(Clone, Copy)]
pub(crate) enum CoordinateKind {
    Position,
    Offset,
}

impl PanTiltCoordinateConversion {
    /// The one checked conversion for profile helpers and every target mount.
    pub(crate) fn checked_units(
        self,
        position: crate::inquiry_conversions::PanTiltPositionDeg,
        mut ranges: PanTiltRanges,
        kind: CoordinateKind,
    ) -> Result<crate::camera::PanTiltPosition> {
        let names = match kind {
            CoordinateKind::Position => ["pan position", "tilt position"],
            CoordinateKind::Offset => {
                let span = |range: capabilities::CapabilityRange<i32>| {
                    let span = range.max() - range.min();
                    capabilities::CapabilityRange::<i32>::new(-span, span)
                };
                ranges.pan_units = span(ranges.pan_units);
                ranges.tilt_units = span(ranges.tilt_units);
                ["pan offset", "tilt offset"]
            }
        };
        let convert =
            |parameter, degrees: f32, scale: f32, units: capabilities::CapabilityRange<i32>| {
                let exact = ordered_degrees(units.min() as f32 / scale, units.max() as f32 / scale);
                let nominal = ranges.endpoint_quantization.degree_range(exact);
                if !degrees.is_finite() || !nominal.contains(&degrees) {
                    return Err(Error::InvalidParameter {
                        parameter,
                        value: degrees.to_string().into(),
                        reason: format!(
                            "coordinate must lie within {nominal:?} degrees ({units:?} units)"
                        )
                        .into(),
                    });
                }
                let rounded =
                    degrees_to_units(degrees, scale).ok_or_else(|| Error::InvalidParameter {
                        parameter,
                        value: degrees.to_string().into(),
                        reason: "coordinate exceeds signed unit storage".into(),
                    })?;
                if units.contains(rounded) {
                    return Ok(rounded);
                }
                let clamped = rounded.clamp(units.min(), units.max());
                if ranges.endpoint_quantization == capabilities::EndpointQuantization::WholeDegrees
                    && i64::from(rounded).abs_diff(i64::from(clamped)) <= 1
                {
                    Ok(clamped)
                } else {
                    Err(Error::InvalidParameter {
                        parameter,
                        value: degrees.to_string().into(),
                        reason: format!("converted coordinate is outside {units:?} units").into(),
                    })
                }
            };
        Ok(crate::camera::PanTiltPosition::new(
            convert(
                names[0],
                position.pan.0,
                self.pan_degrees_to_units,
                ranges.pan_units,
            )?,
            convert(
                names[1],
                position.tilt.0,
                self.tilt_degrees_to_units,
                ranges.tilt_units,
            )?,
        ))
    }
}

/// The single nearest-unit rounding rule used by checked and unbounded conversions.
fn degrees_to_units(degrees: f32, units_per_degree: f32) -> Option<i32> {
    const LOWER: f32 = i32::MIN as f32;
    const UPPER: f32 = -(i32::MIN as f32);
    let units = (degrees * units_per_degree).round();
    (units.is_finite() && (LOWER..UPPER).contains(&units)).then_some(units as i32)
}

fn ordered_degrees(start: f32, end: f32) -> std::ops::RangeInclusive<f32> {
    start.min(end)..=start.max(end)
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

    /// Returns the bounded response-correlation ambiguity window.
    ///
    /// The owner uses this for socketless cancellation intent, late
    /// confirmation after an unconfirmed raw ACK/completion or late write
    /// result, and the target tombstone left by a successful raw command that
    /// declares no correlatable response. Once ACK establishes an exact socket,
    /// this window may extend but never shorten the command's completion
    /// deadline.
    ///
    /// On a raw-VISCA stream transport it also bounds how long a camera's next
    /// inquiries wait for a timed-out inquiry's reply, which the stream still
    /// owes. After it, they (and user `CompletionOnly` raw commands to that
    /// camera) fail unwritten with [`Error::InquiryCorrelationLost`] until the
    /// owed reply arrives or a new session is opened, because a later
    /// unidentifiable reply could otherwise be returned as another inquiry's
    /// data. Commands and stops to that camera are still sent, but its
    /// socketless error frames stay filtered, so a command rejected with one
    /// ends [`Error::UnsequencedCommandUnconfirmed`]; inquiry-based observation
    /// of that camera (settlement polling, `is_moving`, `wait_until_idle`)
    /// fails.
    #[must_use]
    pub const fn ambiguity_timeout(self) -> Duration {
        self.ambiguity_timeout
    }

    /// Returns the camera-busy recovery deadline.
    #[must_use]
    pub const fn busy_timeout(self) -> Duration {
        self.busy_timeout
    }

    /// Returns the bounded late-reply skew retained after a raw inquiry ends
    /// without a matched response.
    ///
    /// This fact is unused by sequence-bearing envelopes and may be zero. It
    /// cannot exceed [`Self::minimum_inquiry_spacing`].
    #[must_use]
    pub const fn raw_inquiry_reply_skew(self) -> Duration {
        self.raw_inquiry_reply_skew
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
            return Err(invalid_profile_fields(
                &[
                    "timing.ack_timeout",
                    "timing.inquiry_timeout",
                    "timing.cancellation_timeout",
                    "timing.ambiguity_timeout",
                ],
                "protocol timeouts must be non-zero",
            ));
        }
        self.command_timeouts
            .validate()
            .map_err(|error| invalid_profile_fields(&["timing.command_timeouts"], error))?;

        let now = Instant::now();
        let command_deadlines =
            CommandCategory::ALL.map(|category| self.command_timeouts.get(category));
        if [
            self.ack_timeout,
            self.inquiry_timeout,
            self.cancellation_timeout,
            self.ambiguity_timeout,
            self.busy_timeout,
            self.raw_inquiry_reply_skew,
            self.minimum_inquiry_spacing,
            self.minimum_command_spacing,
        ]
        .into_iter()
        .chain(command_deadlines)
        .any(|duration| !monotonic_duration_is_representable(now, duration))
        {
            return Err(invalid_profile_fields(
                &["timing"],
                "values must be representable by the monotonic clock",
            ));
        }
        if self.raw_inquiry_reply_skew > self.minimum_inquiry_spacing {
            return Err(invalid_profile_fields(
                &[
                    "timing.raw_inquiry_reply_skew",
                    "timing.minimum_inquiry_spacing",
                ],
                "raw inquiry reply skew cannot exceed minimum inquiry spacing",
            ));
        }

        // `RetryBounds::for_request` derives the default total budget as at
        // least twice the governing command or inquiry deadline. Validate
        // those derived values here as well as the stored timing facts.
        if command_deadlines
            .into_iter()
            .any(|deadline| !retry_budget_is_representable(now, deadline, self.ack_timeout))
            || !retry_budget_is_representable(now, self.inquiry_timeout, Duration::ZERO)
        {
            return Err(invalid_profile_fields(
                &["timing.command_timeouts", "timing.inquiry_timeout"],
                "derived retry deadlines must be representable by the monotonic clock",
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
            raw_inquiry_reply_skew: None,
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

    /// Sets the late-reply skew retained only after an uncertain raw inquiry
    /// release. The value may be zero and must not exceed the profile's minimum
    /// inquiry spacing.
    #[must_use]
    pub const fn raw_inquiry_reply_skew(mut self, skew: Duration) -> Self {
        self.raw_inquiry_reply_skew = Some(skew);
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
        fn required<T>(value: Option<T>, field: &'static str) -> Result<T> {
            value.ok_or_else(|| invalid_profile_fields(&[field], "is required"))
        }

        ProfileTiming {
            ack_timeout: required(self.ack_timeout, "timing.ack_timeout")?,
            command_timeouts: required(self.command_timeouts, "timing.command_timeouts")?,
            inquiry_timeout: required(self.inquiry_timeout, "timing.inquiry_timeout")?,
            cancellation_timeout: required(
                self.cancellation_timeout,
                "timing.cancellation_timeout",
            )?,
            ambiguity_timeout: required(self.ambiguity_timeout, "timing.ambiguity_timeout")?,
            busy_timeout: required(self.busy_timeout, "timing.busy_timeout")?,
            raw_inquiry_reply_skew: required(
                self.raw_inquiry_reply_skew,
                "timing.raw_inquiry_reply_skew",
            )?,
            minimum_inquiry_spacing: required(
                self.minimum_inquiry_spacing,
                "timing.minimum_inquiry_spacing",
            )?,
            minimum_command_spacing: required(
                self.minimum_command_spacing,
                "timing.minimum_command_spacing",
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
#[cfg_attr(feature = "serde", serde(deny_unknown_fields))]
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

    /// Lengthens the default protocol-settlement budget for targeted operations.
    ///
    /// The effective budget is the larger of this override and the request's
    /// retry budget. Values below every built-in targeted operation's retry
    /// budget are refused. Use `settled_with_timeout` for an exact shorter bound.
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

    /// Lowers the base bounded retry count.
    ///
    /// The default base is 3, and tuning may only reduce it:
    /// [`ProfileSpec::validate_tuning`] rejects a larger value, so tuning can
    /// never make retries more aggressive than the default. Each request's own
    /// budget is derived from this base by its timeout category: quick and
    /// inquiry work gets two more attempts, network work one fewer (at least
    /// one), and a long-running command exactly one, so the effective count is
    /// not always the number given here. A request whose retry class is
    /// [`RetryClass::Never`](crate::RetryClass::Never) is never replayed
    /// regardless of this value.
    #[must_use]
    pub const fn retry_limit(mut self, maximum: u32) -> Self {
        self.retry_limit = Some(maximum);
        self
    }

    /// Lengthens retry backoff and the total retry budget.
    ///
    /// Retry timing is a validated profile bound that tuning may only
    /// lengthen, like every other deadline. By default backoff begins at
    /// 50 ms and is capped at the larger of 500 ms and the profile busy
    /// timeout, and a request may retry for the largest of ten seconds, twice
    /// its own governing deadline, and the busy timeout.
    /// [`ProfileSpec::validate_tuning`] rejects an `initial` below 50 ms, a
    /// `maximum` below that ceiling, and a `budget` below the ten-second (or
    /// busy-timeout) floor. A request whose own default budget is longer than
    /// `budget` keeps its default.
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

    /// The completion-deadline override for one command category.
    pub(crate) const fn command_override(self, category: CommandCategory) -> Option<Duration> {
        match category {
            CommandCategory::Quick => self.quick_timeout,
            CommandCategory::Movement => self.movement_timeout,
            CommandCategory::Preset => self.preset_timeout,
            CommandCategory::LongRunning => self.long_running_timeout,
            CommandCategory::Network => self.network_timeout,
        }
    }

    /// Applies the retry-limit override to the default base retry count. The
    /// override may only reduce it.
    #[cfg(any(feature = "async", feature = "blocking", test))]
    pub(crate) fn reduce_retry_base(self, default: u32) -> u32 {
        self.retry_limit.map_or(default, |limit| limit.min(default))
    }
}

/// Owner-side reads of the remaining overrides; only preparation and the
/// owner policy consume them.
#[cfg(any(feature = "blocking", feature = "async"))]
impl OperationalTuning {
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

    pub(crate) const fn settlement_timeout_override(self) -> Option<Duration> {
        self.settlement_timeout
    }

    pub(crate) const fn inquiry_timeout_override(self) -> Option<Duration> {
        self.inquiry_timeout
    }
}

impl OperationalTuning {
    /// Applies the retry-timing overrides to a request's profile retry
    /// bounds. Each override may only lengthen its bound, so a value below the
    /// request's own bound leaves that bound in force.
    pub(crate) fn lengthen_retry_bounds(self, profile: RetryBounds) -> RetryBounds {
        let lengthen = |value: Option<Duration>, bound: Duration| {
            value.map_or(bound, |value| value.max(bound))
        };
        RetryBounds {
            initial_backoff: lengthen(self.initial_retry_backoff, profile.initial_backoff),
            maximum_backoff: lengthen(self.maximum_retry_backoff, profile.maximum_backoff),
            total_budget: lengthen(self.retry_budget, profile.total_budget),
        }
    }
}

/// Base retry count every request's per-category retry budget is derived
/// from. [`OperationalTuning::retry_limit`] may only lower it.
pub(crate) const DEFAULT_RETRY_BASE: u32 = 3;

/// Smallest first retry backoff.
const MINIMUM_INITIAL_RETRY_BACKOFF: Duration = Duration::from_millis(50);

/// Smallest retry backoff ceiling; a profile busy timeout can raise it.
const MINIMUM_MAXIMUM_RETRY_BACKOFF: Duration = Duration::from_millis(500);

/// Floor for the total wall-clock a request may spend retrying, counted from
/// admission. The governing request deadline and the profile busy timeout can
/// raise it.
pub(crate) const MINIMUM_RETRY_BUDGET: Duration = Duration::from_secs(10);

/// Retry pacing bounds for one request.
///
/// [`Self::for_request`] is the profile's bound for a request;
/// [`OperationalTuning::retry_timing`] may only lengthen each one.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct RetryBounds {
    /// Delay before the first retry.
    pub(crate) initial_backoff: Duration,
    /// Ceiling for every later backoff.
    pub(crate) maximum_backoff: Duration,
    /// Total retry wall-clock counted from admission.
    pub(crate) total_budget: Duration,
}

impl RetryBounds {
    /// The profile's retry bounds for a response deadline and full attempt.
    ///
    /// `deadline` is a command's completion window or an inquiry's reply
    /// window. `attempt` additionally includes the ACK window for an
    /// ACK-bearing command. The budget retains the established floor of ten
    /// seconds, twice the response deadline, and the busy timeout, and grows
    /// when needed to fit the complete attempt. This preserves ordinary
    /// untuned budgets while a longer ACK override cannot truncate healthy
    /// execution. Backoff starts at 50 ms, capped by 500 ms or the busy timeout.
    pub(crate) fn for_request(
        busy_timeout: Duration,
        deadline: Duration,
        attempt: Duration,
    ) -> Self {
        Self {
            initial_backoff: MINIMUM_INITIAL_RETRY_BACKOFF,
            maximum_backoff: MINIMUM_MAXIMUM_RETRY_BACKOFF.max(busy_timeout),
            total_budget: MINIMUM_RETRY_BUDGET
                .max(deadline.saturating_mul(2))
                .max(attempt)
                .max(busy_timeout),
        }
    }
}

/// Validated runtime form of compile-time and user-supplied profile facts.
///
/// With the `serde` feature a spec round-trips through JSON or another serde
/// format. Deserialization requires every current field and validates the
/// result. Unknown keys are refused at every level. A spec saved by another
/// release is refused with an instruction to
/// regenerate it, and a built-in identity must match the current registry
/// exactly.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize), serde(transparent))]
pub struct ProfileSpec {
    facts: ProfileFacts,
}

/// Every protocol fact of one profile.
///
/// [`ProfileSpec`] is the validated wrapper; this is also its persisted shape,
/// and the compile-time identity check compares two of these with the derived
/// equality, so a new fact is always part of both.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
#[cfg_attr(feature = "serde", serde(deny_unknown_fields))]
pub(crate) struct ProfileFacts {
    capabilities: capabilities::Capabilities,
    transports: TransportCompatibility,
    envelope: ProfileEnvelope,
    timing: ProfileTiming,
    maximum_command_sockets: u8,
    supports_operation_complete: bool,
    supports_command_cancel: bool,
    // Settlement query availability is a low-level protocol fact, distinct
    // from permission to expose a public typed inquiry.
    position_inquiries: PositionInquirySupport,
}

impl ProfileFacts {
    /// The facts a compile-time profile declares.
    fn compile_time<P>() -> Self
    where
        P: CompileTimeProfile,
    {
        Self {
            capabilities: capabilities::Capabilities::from_profile::<P>(),
            transports: P::TRANSPORTS,
            envelope: <P::Envelope as crate::transport::Envelope>::KIND,
            timing: ProfileTiming {
                ack_timeout: P::ACK_TIMEOUT,
                command_timeouts: P::COMMAND_TIMEOUTS,
                inquiry_timeout: P::INQUIRY_TIMEOUT,
                cancellation_timeout: P::CANCELLATION_TIMEOUT,
                ambiguity_timeout: P::AMBIGUITY_TIMEOUT,
                busy_timeout: P::BUSY_TIMEOUT,
                raw_inquiry_reply_skew: P::RAW_INQUIRY_REPLY_SKEW,
                minimum_inquiry_spacing: P::MIN_INQUIRY_SPACING,
                minimum_command_spacing: P::MIN_COMMAND_SPACING,
            },
            maximum_command_sockets: P::MAXIMUM_COMMAND_SOCKETS,
            supports_operation_complete: P::SUPPORTS_OPERATION_COMPLETE,
            supports_command_cancel: P::SUPPORTS_COMMAND_CANCEL,
            position_inquiries: P::POSITION_INQUIRIES,
        }
    }

    /// Returns whether every fact equals the compile-time declaration of `P`.
    ///
    /// This compares the unvalidated declaration: it runs while validating,
    /// so validating the declaration here would recurse back into this check.
    pub(crate) fn matches_compile_time<P>(&self) -> bool
    where
        P: CompileTimeProfile,
    {
        *self == Self::compile_time::<P>()
    }
}

/// Deserialize an optional fact whose key must still be present.
///
/// Attaching `deserialize_with` prevents serde's implicit missing-Option
/// default while retaining explicit `null` as the declared absent value.
#[cfg(feature = "serde")]
pub(crate) fn required_option<'de, D, T>(
    deserializer: D,
) -> std::result::Result<Option<T>, D::Error>
where
    D: serde::Deserializer<'de>,
    T: serde::Deserialize<'de>,
{
    serde::Deserialize::deserialize(deserializer)
}

/// Instruction appended to every refused persisted spec.
#[cfg(feature = "serde")]
const REGENERATE_SPEC: &str = "the spec was saved by another release, so regenerate it with \
     `ProfileSpec::from_compile_time::<P>()` for a built-in profile (or rebuild a custom \
     profile with `ProfileSpec::builder`) and persist the result";

/// Attach the same regeneration guidance to structural and semantic load failures.
#[cfg(feature = "serde")]
fn persisted_spec_error<E: serde::de::Error>(error: impl fmt::Display) -> E {
    E::custom(format_args!("{error}; {REGENERATE_SPEC}"))
}

/// `schemars(required)` also removes the null alternative from an `Option`.
/// Persisted facts require the key while retaining explicit null as absence.
#[cfg(feature = "schemars")]
pub(crate) fn nullable_schema(schema: &mut schemars::Schema) {
    let value = std::mem::replace(schema, true.into());
    *schema = schemars::json_schema!({ "anyOf": [value, { "type": "null" }] });
}

/// Close only the object schemas reachable from the persisted profile.
/// Serde's standard Duration and RangeInclusive reject unknown fields, but
/// their upstream schemas omit that constraint. Follow local references so
/// embedding ProfileSpec never changes unrelated definitions in a generator.
#[cfg(feature = "schemars")]
fn close_persisted_schema(
    mut schema: schemars::Schema,
    generator: &mut schemars::SchemaGenerator,
) -> schemars::Schema {
    fn close(value: &mut serde_json::Value, references: &mut Vec<String>) {
        match value {
            serde_json::Value::Object(object) => {
                if object.contains_key("properties") {
                    object.insert("additionalProperties".into(), false.into());
                }
                if let Some(reference) = object.get("$ref").and_then(serde_json::Value::as_str) {
                    references.push(reference.into());
                }
                for child in object.values_mut() {
                    close(child, references);
                }
            }
            serde_json::Value::Array(values) => {
                for child in values {
                    close(child, references);
                }
            }
            _ => {}
        }
    }

    let mut references = Vec::new();
    if let Some(object) = schema.as_object_mut() {
        for child in object.values_mut() {
            close(child, &mut references);
        }
        object.insert("additionalProperties".into(), false.into());
    }
    let prefix = format!("#{}", generator.settings().definitions_path);
    let mut visited = std::collections::BTreeSet::new();
    while let Some(reference) = references.pop() {
        let Some(name) = reference
            .strip_prefix(&prefix)
            .and_then(|rest| rest.strip_prefix('/'))
        else {
            continue;
        };
        let name = name.replace("~1", "/").replace("~0", "~");
        if visited.insert(name.clone()) {
            if let Some(definition) = generator.definitions_mut().get_mut(&name) {
                close(definition, &mut references);
            }
        }
    }
    schema
}

#[cfg(feature = "serde")]
impl<'de> serde::Deserialize<'de> for ProfileSpec {
    fn deserialize<D>(deserializer: D) -> std::result::Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let facts =
            ProfileFacts::deserialize(deserializer).map_err(persisted_spec_error::<D::Error>)?;
        Self::validated(facts).map_err(persisted_spec_error::<D::Error>)
    }
}

#[cfg(feature = "schemars")]
impl schemars::JsonSchema for ProfileSpec {
    fn schema_name() -> std::borrow::Cow<'static, str> {
        std::borrow::Cow::Borrowed("ProfileSpec")
    }

    fn json_schema(generator: &mut schemars::SchemaGenerator) -> schemars::Schema {
        close_persisted_schema(ProfileFacts::json_schema(generator), generator)
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
        Self::validated(ProfileFacts::compile_time::<P>())
    }

    /// The only constructor: every spec has passed validation.
    fn validated(facts: ProfileFacts) -> Result<Self> {
        facts.validate().map(|facts| Self { facts })
    }

    /// Returns the human-readable name of this runtime profile.
    ///
    /// This is the canonical accessor for the model name stored in the
    /// profile's capability inventory.
    #[must_use]
    pub fn name(&self) -> &str {
        self.facts.capabilities.model_name.as_str()
    }

    /// Checks that this validated runtime inventory has the same protocol
    /// identity as the inventory lowered from a compile-time profile.
    ///
    /// Static capability bounds are sound only when every protocol fact
    /// agrees, rather than merely the profile identifier or broad capability
    /// bits. Keep this comparison pure so session projection can reject a
    /// mismatched view before owner admission or protocol I/O.
    #[cfg(any(feature = "async", feature = "blocking", test))]
    pub(crate) fn ensure_compile_time<P>(&self) -> Result<()>
    where
        P: CompileTimeProfile,
    {
        if self.matches_compile_time_profile::<P>() {
            Ok(())
        } else {
            Err(Error::InvalidRequest(
                "registered runtime profile does not match the requested compile-time profile"
                    .into(),
            ))
        }
    }

    /// Returns whether every protocol identity fact matches the generated
    /// compile-time projection for `P`.
    #[cfg(any(feature = "async", feature = "blocking", test))]
    pub(crate) fn matches_compile_time_profile<P>(&self) -> bool
    where
        P: CompileTimeProfile,
    {
        self.facts.matches_compile_time::<P>()
    }

    /// Returns runtime feature and conversion facts.
    #[must_use]
    pub const fn capabilities(&self) -> &capabilities::Capabilities {
        &self.facts.capabilities
    }

    /// Returns profile-specified pan/tilt coordinate facts when pan/tilt is supported.
    #[must_use]
    pub const fn pan_tilt_coordinates(&self) -> Option<PanTiltCoordinateConversion> {
        match &self.facts.capabilities.pan_tilt {
            Some(domain) => Some(domain.coordinates),
            None => None,
        }
    }

    /// Returns compatible standard transports.
    #[must_use]
    pub const fn transports(&self) -> TransportCompatibility {
        self.facts.transports
    }

    /// Returns the required wire envelope.
    #[must_use]
    pub const fn envelope(&self) -> ProfileEnvelope {
        self.facts.envelope
    }

    /// The payload capacity supported for requests on this profile.
    ///
    /// The ordinary Sony command families fit sixteen bytes. R7's FR7
    /// position commands need seventeen, selected by their coordinate codec.
    /// This is the crate's supported framing capacity, not a vendor-wide
    /// maximum inferred from one model's command list.
    #[cfg(any(feature = "async", feature = "blocking"))]
    pub(crate) fn maximum_request_bytes(&self) -> usize {
        match self.envelope() {
            ProfileEnvelope::RawVisca => crate::raw::MAX_BYTES,
            ProfileEnvelope::SonyEncapsulated => {
                self.pan_tilt_coordinates().map_or(16, |conversion| {
                    16.max(conversion.wire_codec().maximum_position_frame_bytes())
                })
            }
        }
    }

    /// Returns immutable deadline and pacing minima.
    #[must_use]
    pub const fn timing(&self) -> ProfileTiming {
        self.facts.timing
    }

    /// Returns the validated per-category command completion deadlines.
    #[must_use]
    pub const fn command_timeouts(&self) -> CommandTimeouts {
        self.facts.timing.command_timeouts()
    }

    /// Returns the maximum number of camera command sockets.
    #[must_use]
    pub const fn maximum_command_sockets(&self) -> u8 {
        self.facts.maximum_command_sockets
    }

    /// Returns whether exact operation-complete messages are supported.
    #[must_use]
    pub const fn supports_operation_complete(&self) -> bool {
        self.facts.supports_operation_complete
    }

    /// Returns whether transmitted commands permit protocol cancellation.
    #[must_use]
    pub const fn supports_command_cancel(&self) -> bool {
        self.facts.supports_command_cancel
    }

    /// Returns the exact axes affected by preset recall.
    #[must_use]
    pub const fn preset_recall_axes(&self) -> Option<AffectedAxes> {
        match &self.facts.capabilities.presets {
            Some(domain) => Some(domain.recall_axes),
            None => None,
        }
    }

    /// Returns per-axis position-inquiry support used for settlement.
    #[must_use]
    pub const fn position_inquiries(&self) -> PositionInquirySupport {
        self.facts.position_inquiries
    }

    /// Returns whether the profile declares every selected physical axis.
    ///
    /// This checks the capability inventory, not position-inquiry availability;
    /// callers that need to poll must additionally use
    /// [`PositionInquirySupport::supports`].
    #[must_use]
    pub fn supports_axes(&self, axes: AffectedAxes) -> bool {
        let caps = &self.facts.capabilities;
        (!axes.contains(AffectedAxes::PAN_TILT) || caps.pan_tilt.is_some())
            && (!axes.contains(AffectedAxes::ZOOM) || caps.zoom.is_some())
            && (!axes.contains(AffectedAxes::FOCUS) || caps.focus.is_some())
            && (!axes.contains(AffectedAxes::IRIS)
                || caps
                    .exposure
                    .as_ref()
                    .is_some_and(|domain| domain.iris_range.is_some()))
            && (!axes.contains(AffectedAxes::ND_FILTER) || caps.has_nd_filter())
    }

    /// The number of typed STOP paths (pan/tilt, zoom, focus) this profile
    /// supports. Each one gets a control-reserve admission slot (D26, #778).
    #[cfg(any(feature = "blocking", feature = "async"))]
    pub(crate) fn typed_stop_paths(&self) -> u8 {
        [
            AffectedAxes::PAN_TILT,
            AffectedAxes::ZOOM,
            AffectedAxes::FOCUS,
        ]
        .into_iter()
        .filter(|axes| self.supports_axes(*axes))
        .fold(0, |paths, _| paths + 1)
    }

    /// Validates operational overrides without changing profile safety facts.
    ///
    /// Tuning may only make a profile more conservative: pacing and every
    /// deadline, including the retry backoff and budget, may be lengthened
    /// but never shortened, and socket capacity may only be lowered.
    pub fn validate_tuning(&self, tuning: OperationalTuning) -> Result<()> {
        let now = Instant::now();
        let command_overrides =
            CommandCategory::ALL.map(|category| tuning.command_override(category));
        if [
            tuning.command_spacing,
            tuning.inquiry_spacing,
            tuning.ack_timeout,
            tuning.settlement_timeout,
            tuning.inquiry_timeout,
            tuning.initial_retry_backoff,
            tuning.maximum_retry_backoff,
            tuning.retry_budget,
        ]
        .into_iter()
        .chain(command_overrides)
        .flatten()
        .any(|duration| !monotonic_duration_is_representable(now, duration))
        {
            return Err(Error::InvalidRequest(
                "operational timing overrides must be representable by the monotonic clock".into(),
            ));
        }

        // Command and inquiry overrides feed the same derived retry-budget
        // calculation as their profile-owned counterparts.
        let ack = tuning.ack_timeout.unwrap_or(self.facts.timing.ack_timeout);
        if CommandCategory::ALL.into_iter().any(|category| {
            let deadline = tuning
                .command_override(category)
                .unwrap_or(self.facts.timing.command_timeouts.get(category));
            !retry_budget_is_representable(now, deadline, ack)
        }) || !retry_budget_is_representable(
            now,
            tuning
                .inquiry_timeout
                .unwrap_or(self.facts.timing.inquiry_timeout),
            Duration::ZERO,
        ) {
            return Err(Error::InvalidRequest(
                "operational retry deadlines must be representable by the monotonic clock".into(),
            ));
        }

        if tuning
            .command_spacing
            .is_some_and(|value| value < self.facts.timing.minimum_command_spacing)
            || tuning
                .inquiry_spacing
                .is_some_and(|value| value < self.facts.timing.minimum_inquiry_spacing)
        {
            return Err(Error::InvalidRequest(
                "operational tuning cannot weaken profile pacing minima".into(),
            ));
        }
        if tuning
            .maximum_command_sockets
            .is_some_and(|value| value == 0 || value > self.facts.maximum_command_sockets)
        {
            return Err(Error::InvalidRequest(
                "operational tuning cannot raise the profile socket limit".into(),
            ));
        }
        if [
            tuning.ack_timeout,
            tuning.settlement_timeout,
            tuning.inquiry_timeout,
        ]
        .into_iter()
        .chain(command_overrides)
        .flatten()
        .any(|timeout| timeout.is_zero())
        {
            return Err(Error::InvalidRequest(
                "operational timeout overrides must be non-zero".into(),
            ));
        }
        let undercuts =
            |value: Option<Duration>, floor: Duration| value.is_some_and(|value| value < floor);
        if undercuts(tuning.ack_timeout, self.facts.timing.ack_timeout)
            || undercuts(tuning.inquiry_timeout, self.facts.timing.inquiry_timeout)
            || CommandCategory::ALL.into_iter().any(|category| {
                undercuts(
                    tuning.command_override(category),
                    self.facts.timing.command_timeouts.get(category),
                )
            })
        {
            return Err(Error::InvalidRequest(
                "operational timeout overrides cannot undercut profile deadlines".into(),
            ));
        }
        if let Some(settlement) = tuning.settlement_timeout {
            let budget = |category| {
                let deadline = tuning
                    .command_override(category)
                    .unwrap_or(self.facts.timing.command_timeouts.get(category));
                tuning
                    .lengthen_retry_bounds(RetryBounds::for_request(
                        self.facts.timing.busy_timeout,
                        deadline,
                        ack.saturating_add(deadline),
                    ))
                    .total_budget
            };
            let floor = budget(CommandCategory::Movement).min(budget(CommandCategory::Preset));
            if settlement < floor {
                return Err(Error::InvalidRequest(format!(
                    "operational settlement timeout cannot undercut the smallest targeted retry budget ({floor:?})"
                ).into()));
            }
        }
        if tuning
            .retry_limit
            .is_some_and(|limit| limit > DEFAULT_RETRY_BASE)
        {
            return Err(Error::InvalidRequest(
                format!(
                    "operational retry limit cannot exceed the default base retry count \
                     of {DEFAULT_RETRY_BASE}"
                )
                .into(),
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
        // The profile's request-independent retry floor: every request's own
        // bound is at least this, so an override below it could only shorten
        // retry timing.
        let floor = RetryBounds::for_request(
            self.facts.timing.busy_timeout,
            Duration::ZERO,
            Duration::ZERO,
        );
        if undercuts(tuning.initial_retry_backoff, floor.initial_backoff)
            || undercuts(tuning.maximum_retry_backoff, floor.maximum_backoff)
            || undercuts(tuning.retry_budget, floor.total_budget)
        {
            return Err(Error::InvalidRequest(
                format!(
                    "operational retry timing cannot undercut the profile retry bounds \
                     (initial backoff {:?}, backoff ceiling {:?}, budget {:?})",
                    floor.initial_backoff, floor.maximum_backoff, floor.total_budget,
                )
                .into(),
            ));
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
}

impl ProfileFacts {
    fn validate(mut self) -> Result<Self> {
        if self.transports.tcp_port == Some(0) || self.transports.udp_port == Some(0) {
            return Err(invalid_profile_fields(
                &["transports.tcp_port", "transports.udp_port"],
                "default ports must be non-zero",
            ));
        }
        if self.transports.tcp_port.is_none()
            && self.transports.udp_port.is_none()
            && !self.transports.serial
        {
            return Err(invalid_profile_fields(
                &["transports"],
                "must enable at least one standard transport",
            ));
        }
        if self.envelope == ProfileEnvelope::SonyEncapsulated
            && (self.transports.serial
                || (self.transports.tcp_port.is_none() && self.transports.udp_port.is_none()))
        {
            return Err(invalid_profile_fields(
                &["envelope", "transports"],
                "Sony encapsulation requires an IP transport and is incompatible with serial",
            ));
        }
        self.timing = self.timing.validate()?;
        if !(1..=2).contains(&self.maximum_command_sockets) {
            return Err(invalid_profile_fields(
                &["maximum_command_sockets"],
                "must be one or two",
            ));
        }
        if self.capabilities.model_name.trim().is_empty() {
            return Err(invalid_profile_fields(
                &["capabilities.model_name"],
                "must not be empty",
            ));
        }
        if let Some(profile_id) = self.capabilities.profile_id {
            if !profile_id.matches_profile_facts(&self) {
                let stored = self.capabilities.typed_support;
                let registry = profile_id.registry_typed_support();
                if stored != registry {
                    // The common case is a spec persisted by an earlier
                    // release whose registry granted a different typed
                    // surface set (#795). Name the difference and the fix.
                    let list =
                        |set: capabilities::TypedSupportSet,
                         other: capabilities::TypedSupportSet| {
                            let names = set
                                .iter()
                                .filter(|surface| !other.contains(*surface))
                                .map(|surface| format!("{surface:?}"))
                                .collect::<Vec<_>>();
                            if names.is_empty() {
                                "none".to_owned()
                            } else {
                                names.join(", ")
                            }
                        };
                    let type_name = profile_id.compile_time_type_name();
                    return Err(invalid_profile_fields(
                        &["capabilities.profile_id", "capabilities.typed_support"],
                        format_args!(
                            "built-in profile `{type_name}`: the stored typed-support set differs \
                             from the current built-in registry (missing: {}; not in registry: {}); \
                             the spec was saved by another release, so regenerate it with \
                             `ProfileSpec::from_compile_time::<grafton_visca::profiles::{type_name}>()` \
                             and persist the result",
                            list(registry, stored),
                            list(stored, registry),
                        ),
                    ));
                }
                return Err(invalid_profile_fields(
                    &[
                        "capabilities.profile_id",
                        "capabilities",
                        "transports",
                        "envelope",
                        "timing",
                        "maximum_command_sockets",
                        "supports_operation_complete",
                        "supports_command_cancel",
                        "position_inquiries",
                    ],
                    "built-in identity does not match the runtime protocol identity facts",
                ));
            }
        }
        self.capabilities.validate_domains()?;
        for surface in self.capabilities.typed_support.iter() {
            if !self.capabilities.surface_metadata(surface).permits() {
                return Err(invalid_profile_fields(
                    &["capabilities.typed_support"],
                    format_args!(
                        "typed support {surface:?} cannot enable an absent physical capability"
                    ),
                ));
            }
        }
        let caps = &self.capabilities;
        let queries = self.position_inquiries;
        if caps.inquiry_support == capabilities::InquirySupport::None
            && (queries.pan_tilt
                || queries.zoom
                || queries.focus
                || queries.iris
                || queries.nd_filter)
        {
            return Err(invalid_profile_fields(
                &["position_inquiries", "capabilities.inquiry_support"],
                "position inquiries require profile inquiry support",
            ));
        }
        if (queries.pan_tilt && caps.pan_tilt.is_none())
            || (queries.zoom && caps.zoom.is_none())
            || (queries.focus && caps.focus.is_none())
            || (queries.iris
                && caps
                    .exposure
                    .as_ref()
                    .is_none_or(|domain| domain.iris_range.is_none()))
            || (queries.nd_filter && !caps.has_nd_filter())
        {
            return Err(invalid_profile_fields(
                &["position_inquiries", "capabilities"],
                "position inquiry support names an unsupported axis",
            ));
        }
        if !self.supports_operation_complete
            && ((caps.pan_tilt.is_some() && !queries.pan_tilt)
                || (caps.focus.is_some() && !queries.focus)
                || (caps.supports_typed(capabilities::TypedSupportSurface::DirectZoom)
                    && !queries.zoom)
                || (caps.supports_typed(capabilities::TypedSupportSurface::IrisControl)
                    && !queries.iris)
                || (caps.supports_typed(capabilities::TypedSupportSurface::NdFilter)
                    && !queries.nd_filter))
        {
            return Err(invalid_profile_fields(
                &["supports_operation_complete", "position_inquiries"],
                "a profile without exact completion needs inquiries for every targeted axis",
            ));
        }
        if let Some(presets) = &caps.presets {
            if (presets.recall_axes.contains(AffectedAxes::PAN_TILT) && caps.pan_tilt.is_none())
                || (presets.recall_axes.contains(AffectedAxes::ZOOM) && caps.zoom.is_none())
                || (presets.recall_axes.contains(AffectedAxes::FOCUS) && caps.focus.is_none())
                || (presets.recall_axes.contains(AffectedAxes::IRIS)
                    && caps
                        .exposure
                        .as_ref()
                        .is_none_or(|domain| domain.iris_range.is_none()))
                || (presets.recall_axes.contains(AffectedAxes::ND_FILTER) && !caps.has_nd_filter())
            {
                return Err(invalid_profile_fields(
                    &["capabilities.presets.recall_axes"],
                    "preset recall must name present physical axes",
                ));
            }
            if !self.supports_operation_complete && !queries.supports(presets.recall_axes) {
                return Err(invalid_profile_fields(
                    &[
                        "supports_operation_complete",
                        "position_inquiries",
                        "capabilities.presets.recall_axes",
                    ],
                    "a profile without exact completion needs every preset-recall position inquiry",
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
    /// Late-reply skew retained after an uncertain raw inquiry release.
    const RAW_INQUIRY_REPLY_SKEW: Duration = Self::MIN_INQUIRY_SPACING;
    /// Maximum number of command sockets per camera target.
    const MAXIMUM_COMMAND_SOCKETS: u8;
    /// Exact axes affected by a preset recall.
    const PRESET_RECALL_AXES: Option<AffectedAxes>;
    /// Per-axis position inquiry availability.
    const POSITION_INQUIRIES: PositionInquirySupport;
}

/// Builder for a validated runtime [`ProfileSpec`].
///
/// Transport, envelope, timing, socket, completion, cancellation,
/// and per-axis inquiry facts are mandatory. This prevents a generic default
/// from silently granting protocol behavior.
#[derive(Debug, Clone)]
pub struct ProfileSpecBuilder {
    capabilities: capabilities::Capabilities,
    transports: Option<TransportCompatibility>,
    envelope: Option<ProfileEnvelope>,
    timing: Option<ProfileTiming>,
    maximum_command_sockets: Option<u8>,
    supports_operation_complete: Option<bool>,
    supports_command_cancel: Option<bool>,
    position_inquiries: Option<PositionInquirySupport>,
}

impl ProfileSpecBuilder {
    fn runtime(capabilities: capabilities::Capabilities) -> Self {
        Self {
            capabilities,
            transports: None,
            envelope: None,
            timing: None,
            maximum_command_sockets: None,
            supports_operation_complete: None,
            supports_command_cancel: None,
            position_inquiries: None,
        }
    }

    /// Sets the standard transport compatibility facts and their default
    /// ports.
    #[must_use]
    pub fn transports(mut self, transports: TransportCompatibility) -> Self {
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

    /// Sets exact per-axis position-inquiry support for settlement.
    #[must_use]
    pub fn position_inquiries(mut self, supported: PositionInquirySupport) -> Self {
        self.position_inquiries = Some(supported);
        self
    }

    /// Sets the validated individual default camera ID.
    #[must_use]
    pub fn default_camera_id(mut self, camera_id: crate::IndividualCameraId) -> Self {
        self.capabilities.default_camera_id = camera_id;
        self
    }

    /// Sets the complete pan/tilt domain facts.
    #[must_use]
    pub fn pan_tilt(mut self, facts: capabilities::PanTiltCaps) -> Self {
        self.capabilities.pan_tilt = Some(facts);
        self
    }

    /// Sets the complete zoom domain facts.
    #[must_use]
    pub fn zoom(mut self, facts: capabilities::ZoomCaps) -> Self {
        self.capabilities.zoom = Some(facts);
        self
    }

    /// Sets the complete focus domain facts.
    #[must_use]
    pub fn focus(mut self, facts: capabilities::FocusCaps) -> Self {
        self.capabilities.focus = Some(facts);
        self
    }

    /// Sets the complete exposure domain facts.
    #[must_use]
    pub fn exposure(mut self, facts: capabilities::ExposureCaps) -> Self {
        self.capabilities.exposure = Some(facts);
        self
    }

    /// Sets the complete white-balance domain facts.
    #[must_use]
    pub fn white_balance(mut self, facts: capabilities::WhiteBalanceCaps) -> Self {
        self.capabilities.white_balance = Some(facts);
        self
    }

    /// Sets the complete image domain facts.
    #[must_use]
    pub fn image(mut self, facts: capabilities::ImageCaps) -> Self {
        self.capabilities.image = Some(facts);
        self
    }

    /// Sets the complete presets domain facts.
    #[must_use]
    pub fn presets(mut self, facts: capabilities::PresetCaps) -> Self {
        self.capabilities.presets = Some(facts);
        self
    }

    /// Sets the complete power domain facts.
    #[must_use]
    pub fn power(mut self, facts: capabilities::PowerCaps) -> Self {
        self.capabilities.power = Some(facts);
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
        fn required<T>(value: Option<T>, field: &'static str) -> Result<T> {
            value.ok_or_else(|| invalid_profile_fields(&[field], "is required"))
        }

        ProfileSpec::validated(ProfileFacts {
            capabilities: self.capabilities,
            transports: required(self.transports, "transports")?,
            envelope: required(self.envelope, "envelope")?,
            timing: required(self.timing, "timing")?,
            maximum_command_sockets: required(
                self.maximum_command_sockets,
                "maximum_command_sockets",
            )?,
            supports_operation_complete: required(
                self.supports_operation_complete,
                "supports_operation_complete",
            )?,
            supports_command_cancel: required(
                self.supports_command_cancel,
                "supports_command_cancel",
            )?,
            position_inquiries: required(self.position_inquiries, "position_inquiries")?,
        })
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, unused_qualifications)]
mod tests {
    use super::*;
    use crate::capabilities::{
        Capabilities, CapabilityRange, CoordinateSystem, EndpointQuantization, FocusCaps,
        FocusNearEnd, PanTiltCaps, PanTiltWireCodec, PowerCaps, PresetCaps, TiltRanges,
        TypedSupportSet, TypedSupportSurface, ZoomCaps,
    };

    fn conversion(
        system: CoordinateSystem,
        codec: PanTiltWireCodec,
        pan: f32,
        tilt: f32,
    ) -> PanTiltCoordinateConversion {
        PanTiltCoordinateConversion::new(system, codec, pan, tilt).expect("valid conversion")
    }

    fn valid_runtime_capabilities() -> Capabilities {
        let mut caps =
            Capabilities::runtime_baseline("Downstream runtime camera", 1).expect("baseline");
        caps.pan_tilt = Some(PanTiltCaps {
            pan_range: CapabilityRange::<i32>::new(-100, 100),
            tilt_ranges: TiltRanges::Single(CapabilityRange::<i32>::new(-50, 50)),
            pan_speed: CapabilityRange::<u8>::new(1, 8),
            tilt_speed: CapabilityRange::<u8>::new(1, 8),
            coordinates: conversion(
                CoordinateSystem::SignedCentered,
                PanTiltWireCodec::StandardVisca,
                10.0,
                10.0,
            ),
            endpoint_quantization: EndpointQuantization::Exact,
            simultaneous: true,
            preset_recovery_time: Duration::ZERO,
            variable_speed: false,
            motion_sync_speed_range: None,
        });
        caps.zoom = Some(ZoomCaps {
            optical_max: 1000,
            digital_max: None,
            speed: CapabilityRange::<u8>::new(0, 7),
            direct: false,
            variable: false,
            optical_ratio: Some(10.0),
        });
        caps.focus = Some(FocusCaps {
            range: CapabilityRange::<u16>::new(0, 2000),
            near_end: FocusNearEnd::Minimum,
            speed: CapabilityRange::<u8>::new(0, 7),
            auto_focus: false,
            one_push: false,
            zone_inquiry: false,
            zones: Vec::new(),
            af_sensitivity: false,
            near_limit_inquiry: false,
        });
        caps.presets = Some(PresetCaps {
            highest: 10,
            recall_axes: AffectedAxes::PAN_TILT.union(AffectedAxes::ZOOM),
            speed_range: Some(CapabilityRange::<u8>::new(1, 8)),
            tour: false,
            thumbnail: false,
            recall_delay: Duration::ZERO,
            names: None,
        });
        caps.inquiry_support = capabilities::InquirySupport::Partial;
        caps
    }
    fn runtime_builder(capabilities: Capabilities) -> ProfileSpecBuilder {
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
                    .raw_inquiry_reply_skew(Duration::from_millis(25))
                    .minimum_inquiry_spacing(Duration::from_millis(25))
                    .minimum_command_spacing(Duration::from_millis(25))
                    .build()
                    .expect("valid timing"),
            )
            .maximum_command_sockets(1)
            .supports_operation_complete(false)
            .supports_command_cancel(false)
            .position_inquiries(PositionInquirySupport::new(true, true, true))
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
    }
    fn invalid_request_message<T>(result: Result<T>) -> Option<String> {
        match result {
            Err(Error::InvalidRequest(message)) => Some(message.into_owned()),
            _ => None,
        }
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
        let complete = ProfileTiming::builder()
            .ack_timeout(Duration::from_millis(100))
            .command_timeouts(CommandTimeouts::default())
            .inquiry_timeout(Duration::from_secs(1))
            .cancellation_timeout(Duration::from_secs(2))
            .ambiguity_timeout(Duration::from_secs(3))
            .busy_timeout(Duration::from_secs(4))
            .raw_inquiry_reply_skew(Duration::from_millis(25))
            .minimum_inquiry_spacing(Duration::from_millis(25))
            .minimum_command_spacing(Duration::from_millis(50));

        macro_rules! missing_timing_field {
            ($member:ident, $field:literal) => {{
                let mut builder = complete;
                builder.$member = None;
                let error = invalid_request_message(builder.build())
                    .expect("missing timing field must return InvalidRequest");
                assert!(
                    error.contains(concat!("`timing.", $field, "`")),
                    "missing timing.{} was not named by: {error}",
                    $field
                );
            }};
        }

        missing_timing_field!(ack_timeout, "ack_timeout");
        missing_timing_field!(command_timeouts, "command_timeouts");
        missing_timing_field!(inquiry_timeout, "inquiry_timeout");
        missing_timing_field!(cancellation_timeout, "cancellation_timeout");
        missing_timing_field!(ambiguity_timeout, "ambiguity_timeout");
        missing_timing_field!(busy_timeout, "busy_timeout");
        missing_timing_field!(raw_inquiry_reply_skew, "raw_inquiry_reply_skew");
        missing_timing_field!(minimum_inquiry_spacing, "minimum_inquiry_spacing");
        missing_timing_field!(minimum_command_spacing, "minimum_command_spacing");

        let timing = complete.build().expect("complete timing facts");

        assert_eq!(timing.ack_timeout(), Duration::from_millis(100));
        assert_eq!(timing.command_timeouts(), CommandTimeouts::default());
        assert_eq!(timing.inquiry_timeout(), Duration::from_secs(1));
        assert_eq!(timing.cancellation_timeout(), Duration::from_secs(2));
        assert_eq!(timing.ambiguity_timeout(), Duration::from_secs(3));
        assert_eq!(timing.busy_timeout(), Duration::from_secs(4));
        assert_eq!(timing.raw_inquiry_reply_skew(), Duration::from_millis(25));
        assert_eq!(timing.minimum_inquiry_spacing(), Duration::from_millis(25));
        assert_eq!(timing.minimum_command_spacing(), Duration::from_millis(50));

        assert!(ProfileTiming::builder()
            .ack_timeout(Duration::ZERO)
            .command_timeouts(CommandTimeouts::default())
            .inquiry_timeout(Duration::from_secs(1))
            .cancellation_timeout(Duration::from_secs(1))
            .ambiguity_timeout(Duration::from_secs(1))
            .busy_timeout(Duration::ZERO)
            .raw_inquiry_reply_skew(Duration::ZERO)
            .minimum_inquiry_spacing(Duration::ZERO)
            .minimum_command_spacing(Duration::ZERO)
            .build()
            .is_err());
    }

    #[test]
    fn retry_budget_representability_includes_the_ack_window() {
        let now = Instant::now();
        assert!(retry_budget_is_representable(
            now,
            Duration::from_secs(8),
            Duration::from_secs(10)
        ));
        assert!(!retry_budget_is_representable(
            now,
            Duration::from_secs(1),
            Duration::MAX
        ));
        assert!(!retry_budget_is_representable(
            now,
            Duration::MAX,
            Duration::from_secs(1)
        ));
    }

    #[test]
    fn individually_representable_ack_cannot_overflow_the_full_attempt() {
        let now = Instant::now();
        let (mut low, mut high) = (0_u64, u64::MAX);
        while low < high {
            let middle = low + (high - low) / 2 + (high - low) % 2;
            if now.checked_add(Duration::from_secs(middle)).is_some() {
                low = middle;
            } else {
                high = middle - 1;
            }
        }
        let ack = Duration::from_secs(low);
        let response = Duration::from_secs(1);
        assert!(monotonic_duration_is_representable(now, ack));
        assert!(monotonic_duration_is_representable(now, response * 2));
        assert!(!retry_budget_is_representable(now, response, ack));
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
                .raw_inquiry_reply_skew(Duration::ZERO)
                .minimum_inquiry_spacing(Duration::ZERO)
                .minimum_command_spacing(Duration::ZERO)
        };

        for result in [
            complete().ack_timeout(Duration::MAX).build(),
            complete().inquiry_timeout(Duration::MAX).build(),
            complete().cancellation_timeout(Duration::MAX).build(),
            complete().ambiguity_timeout(Duration::MAX).build(),
            complete().busy_timeout(Duration::MAX).build(),
            complete().raw_inquiry_reply_skew(Duration::MAX).build(),
            complete().minimum_inquiry_spacing(Duration::MAX).build(),
            complete().minimum_command_spacing(Duration::MAX).build(),
        ] {
            assert!(result.is_err());
        }

        assert!(complete()
            .raw_inquiry_reply_skew(Duration::from_nanos(1))
            .build()
            .is_err());

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
            .raw_inquiry_reply_skew(Duration::ZERO)
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
                    .raw_inquiry_reply_skew(Duration::from_millis(25))
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
    fn settlement_tuning_must_lengthen_at_least_one_targeted_retry_budget() {
        let profile =
            ProfileSpec::from_compile_time::<crate::profiles::PtzOpticsG2>().expect("G2 profile");
        for seconds in [2, 20, 59] {
            let error = profile
                .validate_tuning(
                    OperationalTuning::new().settlement_timeout(Duration::from_secs(seconds)),
                )
                .expect_err("an override below the retry floor has no effect");
            assert!(matches!(error, Error::InvalidRequest(message) if message.contains("60s")));
        }
        for seconds in [60, 61, 90, 120] {
            profile
                .validate_tuning(
                    OperationalTuning::new().settlement_timeout(Duration::from_secs(seconds)),
                )
                .expect("an override at or above the floor is useful");
        }
        let tuned = OperationalTuning::new()
            .movement_timeout(Duration::from_secs(90))
            .preset_timeout(Duration::from_secs(70));
        assert!(profile
            .validate_tuning(tuned.settlement_timeout(Duration::from_secs(139)))
            .is_err());
        profile
            .validate_tuning(tuned.settlement_timeout(Duration::from_secs(140)))
            .expect("the smaller Preset retry budget sets the floor");
        let tuned_retry = OperationalTuning::new().retry_timing(
            Duration::from_millis(50),
            Duration::from_millis(500),
            Duration::from_secs(150),
        );
        assert!(profile
            .validate_tuning(tuned_retry.settlement_timeout(Duration::from_secs(149)))
            .is_err());
        profile
            .validate_tuning(tuned_retry.settlement_timeout(Duration::from_secs(150)))
            .expect("retry tuning participates in the settlement floor");
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

    #[test]
    fn retry_timing_tuning_may_only_lengthen_the_profile_bounds() {
        let profile = runtime_builder(valid_runtime_capabilities())
            .build()
            .expect("valid runtime profile");
        let tuning = |initial: u64, maximum: u64, budget: u64| {
            OperationalTuning::new().retry_timing(
                Duration::from_millis(initial),
                Duration::from_millis(maximum),
                Duration::from_millis(budget),
            )
        };
        for (initial, maximum, budget) in [(49, 500, 10_000), (50, 499, 10_000), (50, 500, 9_999)] {
            let error =
                invalid_request_message(profile.validate_tuning(tuning(initial, maximum, budget)))
                    .expect("a shortened retry bound must be rejected");
            assert!(
                error.contains("cannot undercut the profile retry bounds"),
                "{initial}/{maximum}/{budget}: {error}"
            );
        }
        assert!(profile.validate_tuning(tuning(50, 500, 10_000)).is_ok());
        assert!(profile.validate_tuning(tuning(200, 2_000, 30_000)).is_ok());

        // A profile busy timeout raises the floors it governs.
        let busy = ProfileSpec::builder(valid_runtime_capabilities())
            .transports(TransportCompatibility::new(Some(5678), None, false))
            .envelope(ProfileEnvelope::RawVisca)
            .timing(
                ProfileTiming::builder()
                    .ack_timeout(Duration::from_millis(100))
                    .command_timeouts(CommandTimeouts::DEFAULT)
                    .inquiry_timeout(Duration::from_secs(1))
                    .cancellation_timeout(Duration::from_secs(1))
                    .ambiguity_timeout(Duration::from_secs(1))
                    .busy_timeout(Duration::from_secs(12))
                    .raw_inquiry_reply_skew(Duration::from_millis(25))
                    .minimum_inquiry_spacing(Duration::from_millis(25))
                    .minimum_command_spacing(Duration::from_millis(25))
                    .build()
                    .expect("valid timing"),
            )
            .maximum_command_sockets(1)
            .supports_operation_complete(false)
            .supports_command_cancel(false)
            .position_inquiries(PositionInquirySupport::new(true, true, true))
            .build()
            .expect("valid busy-timeout profile");
        assert!(busy.validate_tuning(tuning(50, 11_999, 12_000)).is_err());
        assert!(busy.validate_tuning(tuning(50, 12_000, 11_999)).is_err());
        assert!(busy.validate_tuning(tuning(50, 12_000, 12_000)).is_ok());
    }

    #[test]
    fn retry_limit_tuning_may_only_lower_the_default_base() {
        let profile = runtime_builder(valid_runtime_capabilities())
            .build()
            .expect("valid runtime profile");
        for limit in 0..=DEFAULT_RETRY_BASE {
            assert!(profile
                .validate_tuning(OperationalTuning::new().retry_limit(limit))
                .is_ok());
            assert_eq!(
                OperationalTuning::new()
                    .retry_limit(limit)
                    .reduce_retry_base(DEFAULT_RETRY_BASE),
                limit
            );
        }
        let error = invalid_request_message(
            profile.validate_tuning(OperationalTuning::new().retry_limit(DEFAULT_RETRY_BASE + 1)),
        )
        .expect("a raised retry limit must be rejected");
        assert!(
            error.contains("cannot exceed the default base retry count"),
            "{error}"
        );
        // Even unvalidated, an override never raises the base.
        assert_eq!(
            OperationalTuning::new()
                .retry_limit(32)
                .reduce_retry_base(DEFAULT_RETRY_BASE),
            DEFAULT_RETRY_BASE
        );
    }

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
    fn compile_time_conversions_satisfy_the_framing_rule() {
        use crate::command::pan_tilt::PanTiltFraming;
        use capabilities::{CoordinateSystem, PanTiltWireCodec};

        assert!(!PanTiltFraming::is_consistent(
            PanTiltWireCodec::SonyBrc300,
            CoordinateSystem::UnsignedCentered
        ));
        for (codec, system) in [
            (
                PanTiltWireCodec::SonyBrc300,
                CoordinateSystem::SignedCentered,
            ),
            (
                PanTiltWireCodec::StandardVisca,
                CoordinateSystem::SignedCentered,
            ),
            (
                PanTiltWireCodec::StandardVisca,
                CoordinateSystem::UnsignedCentered,
            ),
        ] {
            assert!(PanTiltFraming::is_consistent(codec, system));
            assert!(PanTiltFraming::new(codec, system).is_ok());
        }
        let brc300 = PanTiltCoordinateConversion::for_profile::<crate::profiles::SonyBRC300>();
        assert_eq!(
            PanTiltFraming::for_conversion(brc300),
            PanTiltFraming::new(
                crate::capabilities::PanTiltWireCodec::SonyBrc300,
                crate::capabilities::CoordinateSystem::SignedCentered
            )
            .expect("signed BRC framing")
        );
    }

    #[test]
    fn runtime_builder_requires_every_protocol_safety_fact() {
        let complete = runtime_builder(valid_runtime_capabilities());

        macro_rules! missing_field {
            ($member:ident, $field:literal) => {{
                let mut builder = complete.clone();
                builder.$member = None;
                let error = invalid_request_message(builder.build())
                    .expect("missing field must return InvalidRequest");
                assert!(
                    error.contains(concat!("`", $field, "`")),
                    "missing {} was not named by: {error}",
                    $field
                );
            }};
        }

        missing_field!(transports, "transports");
        missing_field!(envelope, "envelope");
        missing_field!(timing, "timing");
        missing_field!(maximum_command_sockets, "maximum_command_sockets");
        missing_field!(supports_operation_complete, "supports_operation_complete");
        missing_field!(supports_command_cancel, "supports_command_cancel");
        missing_field!(position_inquiries, "position_inquiries");
    }

    #[test]
    fn transport_ports_come_only_from_the_transport_facts() {
        let baseline =
            Capabilities::runtime_baseline("Runtime transport camera", 1).expect("baseline");
        let profile = conservative_runtime_builder(baseline)
            .build()
            .expect("runtime profile");
        assert_eq!(profile.capabilities().profile_id, None);
        assert_eq!(profile.transports().tcp_port(), Some(5678));
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
    fn grouped_ranges_fit_the_selected_wire_and_typed_domains() {
        let mut caps = valid_runtime_capabilities();
        let pt = caps.pan_tilt.as_mut().expect("pan/tilt");
        pt.coordinates = conversion(
            CoordinateSystem::SignedCentered,
            PanTiltWireCodec::SonyBrc300,
            -1.0,
            1.0,
        );
        pt.pan_range = CapabilityRange::<i32>::new(-0x80000, 0x7FFFF);
        pt.tilt_ranges = TiltRanges::Single(CapabilityRange::<i32>::new(-0x8000, 0x7FFF));
        assert!(runtime_builder(caps.clone()).build().is_ok());
        caps.pan_tilt.as_mut().expect("pan/tilt").pan_range =
            CapabilityRange::<i32>::new(-0x80001, 0x7FFFF);
        assert!(runtime_builder(caps).build().is_err());

        let mut caps = valid_runtime_capabilities();
        caps.pan_tilt.as_mut().expect("pan/tilt").tilt_ranges =
            TiltRanges::Single(CapabilityRange::<i32>::new(-0x8000, 0x8000));
        assert!(runtime_builder(caps).build().is_err());
        for (pan, tilt) in [
            (0.0, 1.0),
            (1.0, -0.0),
            (f32::NAN, 1.0),
            (1.0, f32::INFINITY),
        ] {
            assert!(PanTiltCoordinateConversion::new(
                CoordinateSystem::SignedCentered,
                PanTiltWireCodec::StandardVisca,
                pan,
                tilt
            )
            .is_err());
        }
        assert!(PanTiltCoordinateConversion::new(
            CoordinateSystem::UnsignedCentered,
            PanTiltWireCodec::SonyBrc300,
            1.0,
            1.0
        )
        .is_err());

        let mut caps = valid_runtime_capabilities();
        caps.zoom.as_mut().expect("zoom").speed = CapabilityRange::<u8>::new(0, 8);
        assert!(runtime_builder(caps).build().is_err());
        let mut caps = valid_runtime_capabilities();
        caps.presets.as_mut().expect("presets").speed_range =
            Some(CapabilityRange::<u8>::new(1, 200));
        assert!(runtime_builder(caps).build().is_err());
    }

    #[test]
    fn coordinate_conversion_uses_selected_bounds_and_one_endpoint_policy() {
        let spec = ProfileSpec::from_compile_time::<crate::profiles::SonyFR7>().expect("FR7");
        let desktop =
            crate::TargetRegistration::new(crate::CameraId::CAMERA_1, spec).expect("target");
        let ceiling = desktop
            .clone()
            .with_mount(crate::MountPosition::Ceiling)
            .expect("ceiling");
        assert_eq!(
            desktop
                .convert_pan_tilt_degrees(0.0, 195.0)
                .expect("nominal endpoint")
                .1,
            0xB3B0
        );
        assert!(desktop.convert_pan_tilt_degrees(0.0, 195.01).is_err());
        assert_eq!(
            ceiling
                .convert_pan_tilt_degrees(0.0, 15.0)
                .expect("nominal endpoint")
                .1,
            0xDD2
        );
        assert!(ceiling.convert_pan_tilt_degrees(0.0, 15.01).is_err());
        for target in [&desktop, &ceiling] {
            assert_eq!(
                target
                    .convert_pan_tilt_offset_degrees(0.0, 225.0)
                    .expect("full source span")
                    .1,
                53077
            );
            assert_eq!(
                target
                    .convert_pan_tilt_offset_degrees(0.0, -225.0)
                    .expect("negative source span")
                    .1,
                -53077
            );
            assert!(target.convert_pan_tilt_offset_degrees(0.0, 225.01).is_err());
            assert!(target
                .convert_pan_tilt_offset_degrees(0.0, -225.01)
                .is_err());
        }
        let mut caps = valid_runtime_capabilities();
        caps.pan_tilt.as_mut().expect("pan/tilt").coordinates = conversion(
            CoordinateSystem::UnsignedCentered,
            PanTiltWireCodec::StandardVisca,
            10.0,
            10.0,
        );
        let target = crate::TargetRegistration::new(
            crate::CameraId::CAMERA_1,
            runtime_builder(caps).build().expect("unsigned profile"),
        )
        .expect("target");
        assert_eq!(
            target
                .convert_pan_tilt_degrees(0.0, 0.0)
                .expect("logical center"),
            (0, 0)
        );
        let request = crate::request::builtin::PanTiltAbsolute::for_target(
            crate::units::Degrees(0.0),
            crate::units::Degrees(0.0),
            crate::types::PanSpeed::new(1).expect("speed"),
            crate::types::TiltSpeed::new(1).expect("speed"),
            &target,
        )
        .expect("absolute request");
        let mut wire = [0; 17];
        crate::Request::write_into(&request, crate::CameraId::CAMERA_1, &mut wire).expect("frame");
        assert_eq!(&wire[6..14], &[8, 0, 0, 0, 8, 0, 0, 0]);
        assert_eq!(
            target
                .convert_pan_tilt_offset_degrees(0.0, 0.0)
                .expect("offset"),
            (0, 0)
        );
    }

    #[test]
    fn optional_domains_have_no_orphan_facts_and_derive_ranges() {
        let baseline = Capabilities::runtime_baseline("Minimal camera", 1).expect("baseline");
        assert!(conservative_runtime_builder(baseline).build().is_ok());
        let mut caps = valid_runtime_capabilities();
        let zoom = caps.zoom.as_mut().expect("zoom");
        zoom.digital_max = Some(2000);
        assert_eq!(zoom.optical_range().as_inclusive(), 0..=1000);
        assert_eq!(
            zoom.digital_range().expect("digital").as_inclusive(),
            1000..=2000
        );
        assert!(runtime_builder(caps.clone()).build().is_ok());
        caps.zoom.as_mut().expect("zoom").digital_max = Some(1000);
        assert!(runtime_builder(caps).build().is_err());
        let mut caps = valid_runtime_capabilities();
        let focus = caps.focus.as_mut().expect("focus");
        focus.near_end = FocusNearEnd::Maximum;
        assert_eq!((focus.near(), focus.far()), (Some(2000), Some(0)));
        assert!(runtime_builder(caps).build().is_ok());
    }

    #[test]
    fn position_settlement_and_preset_axes_require_present_domains() {
        let mut caps = valid_runtime_capabilities();
        caps.focus = None;
        assert!(runtime_builder(caps).build().is_err());
        let mut caps = valid_runtime_capabilities();
        caps.zoom = None;
        assert!(runtime_builder(caps)
            .position_inquiries(PositionInquirySupport::new(true, false, true))
            .build()
            .is_err());
        assert!(runtime_builder(valid_runtime_capabilities())
            .position_inquiries(PositionInquirySupport::new(false, true, true))
            .build()
            .is_err());
        assert!(runtime_builder(valid_runtime_capabilities())
            .supports_operation_complete(true)
            .position_inquiries(PositionInquirySupport::new(false, false, false))
            .build()
            .is_ok());
    }

    #[test]
    fn typed_permissions_require_the_domain_and_each_value_inventory() {
        for surface in [
            TypedSupportSurface::DirectZoom,
            TypedSupportSurface::FocusZoneInquiry,
            TypedSupportSurface::UsbAudio,
            TypedSupportSurface::PtzOpticsPresetRecallSpeed,
        ] {
            let mut caps = valid_runtime_capabilities();
            caps.presets.as_mut().expect("presets").speed_range = None;
            caps.typed_support = TypedSupportSet::from_surface(surface);
            assert!(runtime_builder(caps).build().is_err(), "{surface:?}");
        }
        for refinement in [
            TypedSupportSurface::TallyBrightness,
            TypedSupportSurface::PtzOpticsTally,
        ] {
            let mut caps = valid_runtime_capabilities();
            caps.tally = true;
            caps.typed_support = TypedSupportSet::from_surface(refinement);
            assert!(runtime_builder(caps.clone()).build().is_err());
            caps.typed_support = caps
                .typed_support
                .union(TypedSupportSet::from_surface(TypedSupportSurface::Tally));
            assert!(runtime_builder(caps).build().is_ok());
        }
    }

    #[test]
    fn profile_defaults_are_proven_individual_targets() {
        for invalid in [0, 8, u8::MAX] {
            assert!(Capabilities::runtime_baseline("Invalid default", invalid).is_err());
            assert!(crate::IndividualCameraId::new(invalid).is_none());
        }
        for number in 1..=7 {
            let id = crate::IndividualCameraId::new(number).expect("individual");
            let caps =
                Capabilities::runtime_baseline("Individual default", number).expect("baseline");
            let profile = conservative_runtime_builder(caps).build().expect("profile");
            let config = crate::SessionConfig::new(profile);
            assert_eq!(config.sole_target(), Some(crate::CameraId::from(id)));
            assert_eq!(
                crate::target::TargetSlot::from(id).camera(),
                crate::CameraId::from(id)
            );
        }
    }

    #[test]
    fn power_only_profiles_need_no_movement_or_preset_facts() {
        let mut caps = Capabilities::runtime_baseline("Power camera", 1).expect("baseline");
        caps.power = Some(PowerCaps {
            on_time: Duration::from_millis(750),
            standby: Some(Duration::from_millis(250)),
            wake_on_lan: true,
            retains_settings: true,
            home_on_up: false,
        });
        let profile = conservative_runtime_builder(caps)
            .build()
            .expect("power only");
        assert!(profile.pan_tilt_coordinates().is_none());
        assert!(profile.preset_recall_axes().is_none());
    }

    fn copy_builder(base: &ProfileSpec, caps: Capabilities) -> ProfileSpecBuilder {
        ProfileSpec::builder(caps)
            .transports(base.transports())
            .envelope(base.envelope())
            .timing(base.timing())
            .maximum_command_sockets(base.maximum_command_sockets())
            .supports_operation_complete(base.supports_operation_complete())
            .supports_command_cancel(base.supports_command_cancel())
            .position_inquiries(base.position_inquiries())
    }

    #[test]
    fn built_in_identity_covers_every_protocol_fact() {
        use crate::profiles::PtzOpticsG2;

        let base = ProfileSpec::from_compile_time::<PtzOpticsG2>().expect("built-in profile");
        let rebuilt = |capabilities| {
            ProfileSpec::builder(capabilities)
                .transports(base.transports())
                .envelope(base.envelope())
                .timing(base.timing())
                .maximum_command_sockets(base.maximum_command_sockets())
                .supports_operation_complete(base.supports_operation_complete())
                .supports_command_cancel(base.supports_command_cancel())
                .position_inquiries(base.position_inquiries())
        };
        let unclaimed_capabilities = || {
            let mut capabilities = base.capabilities().clone();
            capabilities.profile_id = None;
            capabilities
        };
        let altered_timing = ProfileTiming {
            minimum_command_spacing: base.timing().minimum_command_spacing
                + Duration::from_millis(1),
            ..base.timing()
        };
        let altered_position_inquiries = PositionInquirySupport::new_with_iris_nd(
            base.position_inquiries().pan_tilt(),
            base.position_inquiries().zoom(),
            base.position_inquiries().focus(),
            !base.position_inquiries().iris(),
            base.position_inquiries().nd_filter(),
        );

        macro_rules! assert_identity_mismatch {
            ($field:literal, $configure:expr) => {{
                let configure = $configure;
                let error = invalid_request_message(
                    configure(rebuilt(base.capabilities().clone())).build(),
                )
                .expect("a built-in identity mismatch must be rejected");
                assert!(
                    error.contains(concat!("`", $field, "`")),
                    "the rejected identity mismatch must name {}: {error}",
                    $field
                );

                let unclaimed = configure(rebuilt(unclaimed_capabilities()))
                    .build()
                    .expect("the same override is valid without a built-in identity claim");
                assert!(
                    unclaimed.ensure_compile_time::<PtzOpticsG2>().is_err(),
                    "a runtime override of {} must not project the PtzOpticsG2 facade",
                    $field
                );
            }};
        }

        assert_identity_mismatch!("transports", |builder: ProfileSpecBuilder| builder
            .transports(TransportCompatibility::new(Some(5678), Some(1259), false)));
        assert_identity_mismatch!("timing", |builder: ProfileSpecBuilder| builder
            .timing(altered_timing));
        assert_identity_mismatch!("maximum_command_sockets", |builder: ProfileSpecBuilder| {
            builder.maximum_command_sockets(if base.maximum_command_sockets() == 1 {
                2
            } else {
                1
            })
        });
        assert_identity_mismatch!(
            "supports_operation_complete",
            |builder: ProfileSpecBuilder| builder
                .supports_operation_complete(!base.supports_operation_complete())
        );
        assert_identity_mismatch!("supports_command_cancel", |builder: ProfileSpecBuilder| {
            builder.supports_command_cancel(!base.supports_command_cancel())
        });
        assert_identity_mismatch!("capabilities", |builder: ProfileSpecBuilder| builder
            .presets(PresetCaps {
                recall_axes: AffectedAxes::PAN_TILT,
                ..base.capabilities().presets.expect("presets")
            }));
        assert_identity_mismatch!("capabilities", |builder: ProfileSpecBuilder| builder
            .pan_tilt(PanTiltCaps {
                simultaneous: !base.capabilities().pan_tilt.expect("pan/tilt").simultaneous,
                ..base.capabilities().pan_tilt.expect("pan/tilt")
            }));
        assert_identity_mismatch!("position_inquiries", |builder: ProfileSpecBuilder| builder
            .position_inquiries(altered_position_inquiries));
    }

    #[test]
    fn built_in_identity_covers_grouped_and_protocol_facts() {
        let base = ProfileSpec::from_compile_time::<crate::profiles::PtzOpticsG2>().expect("G2");
        let mut caps = base.capabilities().clone();
        caps.presets.as_mut().expect("presets").recall_axes = AffectedAxes::PAN_TILT;
        assert!(copy_builder(&base, caps.clone()).build().is_err());
        caps.profile_id = None;
        assert!(copy_builder(&base, caps).build().is_ok());
        assert!(copy_builder(&base, base.capabilities().clone())
            .supports_command_cancel(!base.supports_command_cancel())
            .build()
            .is_err());
        let mut custom = base.capabilities().clone();
        custom.profile_id = None;
        assert!(copy_builder(&base, custom)
            .supports_command_cancel(!base.supports_command_cancel())
            .build()
            .is_ok());
    }

    #[test]
    fn exact_inventories_reject_duplicates_and_missing_typed_modes() {
        let base = ProfileSpec::from_compile_time::<crate::profiles::PtzOpticsG2>().expect("G2");
        let custom = || {
            let mut caps = base.capabilities().clone();
            caps.profile_id = None;
            caps
        };
        let mut caps = custom();
        let zone = caps.focus.as_ref().expect("focus").zones[0];
        caps.focus.as_mut().expect("focus").zones.push(zone);
        assert!(copy_builder(&base, caps).build().is_err());
        let mut caps = custom();
        caps.focus.as_mut().expect("focus").zones.clear();
        assert!(copy_builder(&base, caps).build().is_err());
        let mut caps = custom();
        let mode = caps.white_balance.as_ref().expect("WB").modes[0];
        caps.white_balance.as_mut().expect("WB").modes.push(mode);
        assert!(copy_builder(&base, caps).build().is_err());
        let mut caps = custom();
        caps.white_balance
            .as_mut()
            .expect("WB")
            .modes
            .retain(|mode| *mode != crate::WhiteBalanceMode::ColorTemperature);
        assert!(copy_builder(&base, caps).build().is_err());
        let mut caps = custom();
        let speed = caps.exposure.as_ref().expect("exposure").shutter_speeds[0];
        caps.exposure
            .as_mut()
            .expect("exposure")
            .shutter_speeds
            .push(speed);
        assert!(copy_builder(&base, caps).build().is_err());
    }

    #[test]
    fn remaining_numeric_and_inventory_constraints_are_validated_at_build() {
        let base = ProfileSpec::from_compile_time::<crate::profiles::PtzOpticsG2>().expect("G2");
        let custom = || {
            let mut caps = base.capabilities().clone();
            caps.profile_id = None;
            caps
        };
        let mut caps = custom();
        caps.exposure.as_mut().expect("exposure").gain_range =
            Some(CapabilityRange::<u8>::new(0, 255));
        assert!(copy_builder(&base, caps).build().is_err());
        let mut caps = custom();
        caps.exposure.as_mut().expect("exposure").iris_range =
            Some(crate::capabilities::CapabilityDomain::<u16>::new(0, 255));
        assert!(copy_builder(&base, caps).build().is_err());
        let mut caps = custom();
        caps.pan_tilt
            .as_mut()
            .expect("pan/tilt")
            .motion_sync_speed_range = Some(CapabilityRange::<u8>::new(0, 24));
        assert!(copy_builder(&base, caps).build().is_err());
        let mut caps = custom();
        caps.white_balance.as_mut().expect("WB").modes.clear();
        assert!(copy_builder(&base, caps).build().is_err());
        let mut caps = custom();
        caps.focus.as_mut().expect("focus").auto_focus = false;
        caps.focus.as_mut().expect("focus").af_sensitivity = true;
        assert!(copy_builder(&base, caps).build().is_err());
        let mut caps = custom();
        caps.image = None;
        caps.typed_support =
            TypedSupportSet::from_surface(TypedSupportSurface::BacklightCompensation);
        assert!(copy_builder(&base, caps).build().is_err());
    }

    #[test]
    fn zero_is_one_preset_and_the_highest_number_is_inclusive() {
        let mut caps = valid_runtime_capabilities();
        caps.presets.as_mut().expect("presets").highest = 0;
        let profile = runtime_builder(caps).build().expect("preset zero");
        let registration =
            crate::TargetRegistration::new(crate::CameraId::CAMERA_1, profile).expect("target");
        let set = |number| {
            crate::Request::validate_for_target(
                &crate::request::builtin::PresetSet::new(
                    crate::command::PresetNumber::new(number).expect("preset"),
                ),
                &registration,
            )
        };
        set(0).expect("the sole preset");
        assert!(matches!(
            set(1),
            Err(Error::ParameterOutOfRange { min: 0, max: 0, .. })
        ));
    }

    #[cfg(feature = "serde")]
    #[test]
    fn saved_builtin_semantic_changes_keep_the_cause_and_one_regeneration_instruction() {
        let base = ProfileSpec::from_compile_time::<crate::profiles::PtzOpticsG2>().expect("G2");
        let mut saved = serde_json::to_value(&base).expect("serialize current spec");
        saved["capabilities"]["default_camera_id"] = serde_json::json!(2);

        let facts: ProfileFacts = serde_json::from_value(saved.clone())
            .expect("the individual ID and complete persisted shape are valid");
        let builder_error = copy_builder(&base, facts.capabilities)
            .build()
            .expect_err("the saved facts conflict with the current built-in identity");
        assert!(matches!(&builder_error, Error::InvalidRequest(_)));
        let cause = builder_error.to_string();
        assert!(cause.contains("`capabilities.profile_id`"), "{cause}");
        assert!(cause.contains("`capabilities`"), "{cause}");
        assert!(
            cause.contains("built-in identity does not match"),
            "{cause}"
        );

        let error = serde_json::from_value::<ProfileSpec>(saved)
            .expect_err("structurally valid stale facts still need regeneration")
            .to_string();
        assert!(
            error.contains(&cause),
            "original field context was lost: {error}"
        );
        assert_eq!(error.matches(REGENERATE_SPEC).count(), 1, "{error}");
    }

    #[cfg(feature = "serde")]
    #[test]
    fn grouped_profile_persistence_requires_current_keys_and_validates_values() {
        let base = ProfileSpec::from_compile_time::<crate::profiles::PtzOpticsG2>().expect("G2");
        let value = serde_json::to_value(&base).expect("serialize");
        for (path, key) in [
            ("/capabilities", "focus"),
            ("/capabilities/zoom", "optical_ratio"),
            ("/capabilities/pan_tilt/coordinates", "wire_codec"),
        ] {
            let mut missing = value.clone();
            missing
                .pointer_mut(path)
                .expect("object")
                .as_object_mut()
                .expect("record")
                .remove(key);
            let error = serde_json::from_value::<ProfileSpec>(missing)
                .expect_err("required current field")
                .to_string();
            assert!(error.contains(&format!("missing field `{key}`")), "{error}");
            assert!(error.contains(REGENERATE_SPEC), "{error}");
        }
        let mut invalid = value.clone();
        invalid["capabilities"]["default_camera_id"] = serde_json::json!(8);
        assert!(serde_json::from_value::<ProfileSpec>(invalid).is_err());
        let mut invalid = value.clone();
        invalid["capabilities"]["pan_tilt"]["coordinates"]["pan_degrees_to_units"] =
            serde_json::json!(0);
        assert!(serde_json::from_value::<ProfileSpec>(invalid).is_err());
        let restored: ProfileSpec = serde_json::from_value(value).expect("current grouped shape");
        assert_eq!(restored, base);
    }
}
