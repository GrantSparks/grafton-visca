//! Validated runtime camera-profile specifications.

crate::camera::profiles::profile_registry::builtin_profile_registry!(
    crate::camera::profiles::profile_registry::reexport_builtin_profiles
);
pub use crate::camera::profiles::ProfileId;
pub use crate::profile_selection::{
    AnyProfile, CameraProfile, ProfileSelection, TypedProfileSelection,
};

use std::{
    fmt,
    time::{Duration, Instant},
};

use crate::config::OperationalTuning;

use crate::{capabilities, AffectedAxes, Error, Result};

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
    Error::InvalidRequest {
        reason: format!("profile {label} {names}: {reason}").into(),
    }
}

/// Largest duration an explicit retry timing override may set.
///
/// The one-hour override limit keeps caller-selected replay policy bounded.
/// It is not a limit on valid profile response deadlines or retry windows
/// derived from them: a two-hour completion deadline may legitimately require
/// a four-hour total budget. Profile validation checks representability;
/// engine admission checks the final contextual intervals against the owner's
/// sampled clock. Deadline overflow saturates toward the latest instant,
/// never back to the start of the wait (#636, #861).
const MAXIMUM_RETRY_TIMING: Duration = Duration::from_secs(60 * 60);

/// Returns whether adding a duration to the monotonic clock can be represented.
///
/// Profile and operational-tuning validation give callers early errors for
/// impossible intervals. The engine checks the completed request again at its
/// actual admission instant; its defensive saturation never rewinds a future
/// deadline to the start of the wait.
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

/// Wire envelope required by a camera profile.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
#[cfg_attr(feature = "ts-rs", derive(ts_rs::TS), ts(export))]
#[non_exhaustive]
#[cfg_attr(feature = "serde", serde(rename_all = "snake_case"))]
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
        serde(deserialize_with = "crate::profiles::required_option")
    )]
    #[cfg_attr(feature = "schemars", schemars(required, transform = nullable_schema))]
    tcp_port: Option<u16>,
    #[cfg_attr(
        feature = "serde",
        serde(deserialize_with = "crate::profiles::required_option")
    )]
    #[cfg_attr(feature = "schemars", schemars(required, transform = nullable_schema))]
    udp_port: Option<u16>,
    serial: bool,
}

impl TransportCompatibility {
    /// Creates transport facts with no enabled transport.
    ///
    /// Add at least one transport before [`ProfileSpecBuilder::build`].
    #[must_use]
    pub const fn new() -> Self {
        Self {
            tcp_port: None,
            udp_port: None,
            serial: false,
        }
    }

    /// Enables TCP with its nonzero default port, checked at profile build.
    #[must_use]
    pub const fn with_tcp(mut self, port: u16) -> Self {
        self.tcp_port = Some(port);
        self
    }

    /// Enables UDP with its nonzero default port, checked at profile build.
    #[must_use]
    pub const fn with_udp(mut self, port: u16) -> Self {
        self.udp_port = Some(port);
        self
    }

    /// Enables serial transport.
    #[must_use]
    pub const fn with_serial(mut self) -> Self {
        self.serial = true;
        self
    }

    /// Projects the enabled standard transport kinds.
    #[must_use]
    pub const fn transport_set(self) -> capabilities::TransportSet {
        use crate::transport::StandardTransport;
        let mut set = capabilities::TransportSet::empty();
        if self.supports(StandardTransport::Tcp) {
            set = set.with(StandardTransport::Tcp);
        }
        if self.supports(StandardTransport::Udp) {
            set = set.with(StandardTransport::Udp);
        }
        if self.supports(StandardTransport::Serial) {
            set = set.with(StandardTransport::Serial);
        }
        set
    }

    /// Answers the standard profile support matrix in one place.
    pub(crate) const fn supports(self, transport: crate::transport::StandardTransport) -> bool {
        match transport {
            crate::transport::StandardTransport::Tcp => self.tcp_port().is_some(),
            crate::transport::StandardTransport::Udp => self.udp_port().is_some(),
            crate::transport::StandardTransport::Serial => self.serial,
        }
    }

    /// Returns the profile-selected port of a network transport.
    pub(crate) const fn default_port(
        self,
        transport: crate::transport::NetworkTransport,
    ) -> Option<u16> {
        match transport {
            crate::transport::NetworkTransport::Tcp => self.tcp_port,
            crate::transport::NetworkTransport::Udp => self.udp_port,
        }
    }

    /// Returns the default TCP port when TCP is supported.
    #[must_use]
    pub const fn tcp_port(self) -> Option<u16> {
        self.default_port(crate::transport::NetworkTransport::Tcp)
    }

    /// Returns the default UDP port when UDP is supported.
    #[must_use]
    pub const fn udp_port(self) -> Option<u16> {
        self.default_port(crate::transport::NetworkTransport::Udp)
    }

    /// Returns whether serial transport is supported.
    #[must_use]
    pub const fn supports_serial(self) -> bool {
        self.serial
    }
}

impl Default for TransportCompatibility {
    fn default() -> Self {
        Self::new()
    }
}

/// Immutable protocol timings owned by a validated profile.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
#[cfg_attr(feature = "serde", serde(deny_unknown_fields))]
#[cfg_attr(feature = "serde", serde(try_from = "ProfileTimingSerde"))]
#[cfg_attr(feature = "ts-rs", derive(ts_rs::TS))]
pub struct ProfileTiming {
    #[cfg_attr(feature = "ts-rs", ts(type = "{ secs: number, nanos: number }"))]
    pub(crate) ack_timeout: Duration,
    pub(crate) command_timeouts: CommandTimeouts,
    #[cfg_attr(feature = "ts-rs", ts(type = "{ secs: number, nanos: number }"))]
    pub(crate) inquiry_timeout: Duration,
    #[cfg_attr(feature = "ts-rs", ts(type = "{ secs: number, nanos: number }"))]
    pub(crate) cancellation_timeout: Duration,
    #[cfg_attr(feature = "ts-rs", ts(type = "{ secs: number, nanos: number }"))]
    pub(crate) ambiguity_timeout: Duration,
    #[cfg_attr(feature = "ts-rs", ts(type = "{ secs: number, nanos: number }"))]
    pub(crate) busy_timeout: Duration,
    #[cfg_attr(feature = "ts-rs", ts(type = "{ secs: number, nanos: number }"))]
    pub(crate) raw_inquiry_reply_skew: Duration,
    #[cfg_attr(feature = "ts-rs", ts(type = "{ secs: number, nanos: number }"))]
    pub(crate) inquiry_spacing: Duration,
    #[cfg_attr(feature = "ts-rs", ts(type = "{ secs: number, nanos: number }"))]
    pub(crate) command_spacing: Duration,
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
    inquiry_spacing: Duration,
    command_spacing: Duration,
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
            inquiry_spacing: value.inquiry_spacing,
            command_spacing: value.command_spacing,
        }
        .validate()
    }
}

/// Builder for validated [`crate::profiles::ProfileTiming`] facts.
///
/// Every timing fact is required.  The builder does not supply protocol
/// defaults because timing is part of a profile's safety contract; callers
/// should use [`crate::profiles::CommandTimeouts::default`] explicitly when the standard
/// command-category deadlines are appropriate.  [`crate::profiles::ProfileTiming`] itself is
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
    inquiry_spacing: Option<Duration>,
    command_spacing: Option<Duration>,
}

/// Profile-specified pan/tilt coordinate conversion owned by a validated profile.
///
/// This is the crate's single implementation of the degrees ↔ raw-unit
/// conversion. Request preparation, physical position readback, and capability
/// degree ranges all use the conversion stored in the target's profile.
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

#[derive(Debug, Clone, Copy, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(
    feature = "serde",
    serde(try_from = "PanTiltCoordinateConversionSerde")
)]
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
#[cfg_attr(feature = "serde", serde(deny_unknown_fields))]
#[cfg_attr(feature = "ts-rs", derive(ts_rs::TS))]
pub struct PanTiltCoordinateConversion {
    coordinate_system: capabilities::CoordinateSystem,
    wire_codec: capabilities::PanTiltWireCodec,
    pan_degrees_to_units: f32,
    tilt_degrees_to_units: f32,
}

/// Unvalidated wire form of [`crate::profiles::PanTiltCoordinateConversion`].
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
                &["capabilities.pan_tilt.geometry.coordinates"],
                "units-per-degree scales must be finite and nonzero",
            ));
        }
        crate::command::pan_tilt_wire::PanTiltFraming::new(wire_codec, coordinate_system)?;
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

    /// Builds a registry row's cached conversion and checks its static facts.
    ///
    /// Public/custom profiles use `new`, which reports the same failures as
    /// crate errors. The registry's literals retain compile-time assertions.
    pub(crate) const fn from_registry(
        coordinate_system: capabilities::CoordinateSystem,
        wire_codec: capabilities::PanTiltWireCodec,
        pan_degrees_to_units: f32,
        tilt_degrees_to_units: f32,
    ) -> Self {
        assert!(
            pan_degrees_to_units.is_finite()
                && pan_degrees_to_units != 0.0
                && tilt_degrees_to_units.is_finite()
                && tilt_degrees_to_units != 0.0,
            "pan/tilt coordinate scales must be finite and nonzero"
        );
        assert!(
            crate::command::pan_tilt_wire::PanTiltFraming::is_consistent(
                wire_codec,
                coordinate_system
            ),
            "Sony position framing requires signed-centered coordinates"
        );
        Self {
            coordinate_system,
            wire_codec,
            pan_degrees_to_units,
            tilt_degrees_to_units,
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

    /// Converts raw axis coordinates to checked physical degrees.
    pub fn to_degrees(
        self,
        position: crate::camera::PanTiltUnits,
    ) -> Result<crate::types::PanTiltPosition> {
        crate::types::PanTiltPosition::from_degrees(
            self.pan_degrees(position.pan),
            self.tilt_degrees(position.tilt),
        )
    }
}

/// Coordinate conversion with the effective source ranges selected at registration.
#[derive(Clone, Copy)]
pub(crate) struct TargetPanTiltCoordinates {
    pub(crate) conversion: PanTiltCoordinateConversion,
    pub(crate) pan_units: capabilities::CapabilityRange<i32>,
    pub(crate) tilt_units: capabilities::CapabilityRange<i32>,
    pub(crate) endpoint_quantization: capabilities::EndpointQuantization,
}

#[derive(Clone, Copy)]
pub(crate) enum CoordinateKind {
    Position,
    Offset,
}

impl TargetPanTiltCoordinates {
    /// Converts a physical position or offset through the registered target's ranges.
    pub(crate) fn checked_units(
        mut self,
        position: crate::types::PanTiltPosition,
        kind: CoordinateKind,
    ) -> Result<crate::camera::PanTiltUnits> {
        let names = match kind {
            CoordinateKind::Position => ["pan position", "tilt position"],
            CoordinateKind::Offset => {
                let span = |range: capabilities::CapabilityRange<i32>| {
                    let span = range.max() - range.min();
                    capabilities::CapabilityRange::<i32>::new(-span, span)
                };
                self.pan_units = span(self.pan_units)?;
                self.tilt_units = span(self.tilt_units)?;
                ["pan offset", "tilt offset"]
            }
        };
        let convert =
            |parameter, degrees: f32, scale: f32, units: capabilities::CapabilityRange<i32>| {
                let nominal = self.nominal_range(scale, units);
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
                self.constrain_units(rounded, units)
                    .ok_or_else(|| Error::InvalidParameter {
                        parameter,
                        value: degrees.to_string().into(),
                        reason: format!("converted coordinate is outside {units:?} units").into(),
                    })
            };
        Ok(crate::camera::PanTiltUnits::new(
            convert(
                names[0],
                position.pan.value(),
                self.conversion.pan_degrees_to_units,
                self.pan_units,
            )?,
            convert(
                names[1],
                position.tilt.value(),
                self.conversion.tilt_degrees_to_units,
                self.tilt_units,
            )?,
        ))
    }

    /// Checks a reported raw position against the registered target's ranges.
    pub(crate) fn position_units(
        self,
        position: crate::camera::PanTiltUnits,
    ) -> Result<crate::camera::PanTiltUnits> {
        for (parameter, raw, units) in [
            ("pan position", position.pan, self.pan_units),
            ("tilt position", position.tilt, self.tilt_units),
        ] {
            if !units.contains(raw) {
                return Err(Error::InvalidParameter {
                    parameter,
                    value: raw.to_string().into(),
                    reason: format!("reported coordinate is outside {units:?} units").into(),
                });
            }
        }
        Ok(position)
    }

    /// Converts valid raw positions, preserving interior precision. A sourced
    /// endpoint just outside the nominal degree interval can be expressed at
    /// the nominal limit only if doing so preserves the same wire word.
    pub(crate) fn position_degrees(
        self,
        position: crate::camera::PanTiltUnits,
    ) -> Result<crate::types::PanTiltPosition> {
        let position = self.position_units(position)?;
        let decode = |raw, scale, units: capabilities::CapabilityRange<i32>| {
            let degrees = raw as f32 / scale;
            if self.endpoint_quantization == capabilities::EndpointQuantization::WholeDegrees
                && (raw == units.min() || raw == units.max())
            {
                let nominal = self.nominal_range(scale, units);
                if !nominal.contains(&degrees) {
                    let endpoint = degrees.clamp(*nominal.start(), *nominal.end());
                    if degrees_to_units(endpoint, scale)
                        .and_then(|word| self.constrain_units(word, units))
                        == Some(raw)
                    {
                        return endpoint;
                    }
                }
            }
            degrees
        };
        crate::types::PanTiltPosition::from_degrees(
            decode(
                position.pan,
                self.conversion.pan_degrees_to_units,
                self.pan_units,
            ),
            decode(
                position.tilt,
                self.conversion.tilt_degrees_to_units,
                self.tilt_units,
            ),
        )
    }

    fn nominal_range(
        self,
        scale: f32,
        units: capabilities::CapabilityRange<i32>,
    ) -> std::ops::RangeInclusive<f32> {
        self.endpoint_quantization.degree_range(ordered_degrees(
            units.min() as f32 / scale,
            units.max() as f32 / scale,
        ))
    }

    fn constrain_units(
        self,
        rounded: i32,
        units: capabilities::CapabilityRange<i32>,
    ) -> Option<i32> {
        let clamped = rounded.clamp(units.min(), units.max());
        (rounded == clamped
            || (self.endpoint_quantization == capabilities::EndpointQuantization::WholeDegrees
                && rounded.abs_diff(clamped) == 1))
            .then_some(clamped)
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
    /// On raw-VISCA streams, this bounds the wait for an owed inquiry reply
    /// when another inquiry has an overlapping payload declaration. Once the
    /// window expires, overlapping inquiries fail unwritten with
    /// [`Error::InquiryCorrelationLost`]. Disjoint inquiries can continue
    /// and settle older debts by write order. An owed `CompletionOnly` reply
    /// holds all inquiries to that camera. See the
    /// [raw stream correlation ledger](https://github.com/GrantSparks/grafton-visca/blob/main/docs/architecture_2_0.md#the-raw-byte-stream-correlation-ledger).
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
    /// cannot exceed [`Self::inquiry_spacing`].
    #[must_use]
    pub const fn raw_inquiry_reply_skew(self) -> Duration {
        self.raw_inquiry_reply_skew
    }

    /// Returns the minimum interval between inquiry writes.
    #[must_use]
    pub const fn inquiry_spacing(self) -> Duration {
        self.inquiry_spacing
    }

    /// Returns the minimum interval between command writes.
    #[must_use]
    pub const fn command_spacing(self) -> Duration {
        self.command_spacing
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
            CommandTimeoutClass::ALL.map(|category| self.command_timeouts.get(category));
        if [
            self.ack_timeout,
            self.inquiry_timeout,
            self.cancellation_timeout,
            self.ambiguity_timeout,
            self.busy_timeout,
            self.raw_inquiry_reply_skew,
            self.inquiry_spacing,
            self.command_spacing,
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
        if self.raw_inquiry_reply_skew > self.inquiry_spacing {
            return Err(invalid_profile_fields(
                &["timing.raw_inquiry_reply_skew", "timing.inquiry_spacing"],
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
            inquiry_spacing: None,
            command_spacing: None,
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
    pub const fn inquiry_spacing(mut self, spacing: Duration) -> Self {
        self.inquiry_spacing = Some(spacing);
        self
    }

    /// Sets the minimum interval between command writes.
    #[must_use]
    pub const fn command_spacing(mut self, spacing: Duration) -> Self {
        self.command_spacing = Some(spacing);
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
            inquiry_spacing: required(self.inquiry_spacing, "timing.inquiry_spacing")?,
            command_spacing: required(self.command_spacing, "timing.command_spacing")?,
        }
        .validate()
    }
}

/// Base retry count every request's per-category retry budget is derived
/// from. [`crate::config::OperationalTuning::with_retry_limit`] may only lower it.
#[cfg(any(feature = "async", feature = "blocking", test))]
pub(crate) const DEFAULT_RETRY_BASE: u32 = crate::tuning::RetryLimit::MAX.value() as u32;

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
/// [`crate::config::OperationalTuning::with_retry_budget`] may only lengthen each one.
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
    #[cfg_attr(feature = "serde", serde(skip))]
    position_speeds: Option<crate::types::PositionSpeeds>,
    #[cfg_attr(feature = "serde", serde(skip))]
    settlement_axes: Option<AffectedAxes>,
}

/// Every protocol fact of one profile.
///
/// [`crate::profiles::ProfileSpec`] is the validated wrapper; this is also its persisted shape,
/// and the compile-time identity check compares two of these with the derived
/// equality, so a new fact is always part of both.
#[derive(Debug, Clone, PartialEq)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
#[cfg_attr(feature = "serde", serde(deny_unknown_fields))]
#[cfg_attr(feature = "ts-rs", derive(ts_rs::TS))]
pub(crate) struct ProfileFacts {
    pub(crate) capabilities: capabilities::Capabilities,
    pub(crate) encodings: capabilities::ProfileEncodings,
    pub(crate) transports: TransportCompatibility,
    pub(crate) envelope: ProfileEnvelope,
    pub(crate) timing: ProfileTiming,
    pub(crate) maximum_command_sockets: crate::CommandSocketCount,
    pub(crate) supports_operation_complete: bool,
    pub(crate) supports_command_cancel: bool,
    // Each settlement axis uses its public inquiry's canonical gate. In
    // particular, polling iris requires the Iris typed-support grant.
    #[cfg_attr(feature = "serde", serde(deserialize_with = "required_option"))]
    #[cfg_attr(feature = "schemars", schemars(required, transform = nullable_schema))]
    pub(crate) position_inquiries: Option<AffectedAxes>,
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
    let cause = error.to_string();
    // A stale built-in support set already supplies its precise constructor.
    // Require the complete instruction at the end and a known profile name;
    // instruction-like text inside user-controlled keys is not guidance.
    let has_model_guide = cause
        .rsplit_once(
            "the spec was saved by another release, so regenerate it with \
             `ProfileSpec::from_compile_time::<grafton_visca::profiles::",
        )
        .and_then(|(_, tail)| tail.strip_suffix(">()` and persist the result"))
        .is_some_and(|name| {
            ProfileId::all()
                .iter()
                .any(|profile| profile.compile_time_type_name() == name)
        });
    if has_model_guide || cause.ends_with(REGENERATE_SPEC) {
        E::custom(cause)
    } else {
        E::custom(format_args!("{cause}; {REGENERATE_SPEC}"))
    }
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
        Self::from_facts(facts).map_err(persisted_spec_error::<D::Error>)
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

#[cfg(feature = "ts-rs")]
impl ts_rs::TS for ProfileSpec {
    type WithoutGenerics = Self;
    type OptionInnerType = Self;

    fn name(_config: &ts_rs::Config) -> String {
        "ProfileSpec".into()
    }
    fn output_path() -> Option<std::path::PathBuf> {
        Some("ProfileSpec.ts".into())
    }
    fn inline(config: &ts_rs::Config) -> String {
        <ProfileFacts as ts_rs::TS>::inline(config)
    }
    fn decl(config: &ts_rs::Config) -> String {
        format!(
            "type ProfileSpec = {};",
            <Self as ts_rs::TS>::inline(config)
        )
    }
    fn decl_concrete(config: &ts_rs::Config) -> String {
        <Self as ts_rs::TS>::decl(config)
    }
    fn visit_dependencies(visitor: &mut impl ts_rs::TypeVisitor)
    where
        Self: 'static,
    {
        <ProfileFacts as ts_rs::TS>::visit_dependencies(visitor);
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
        let profile = P::spec()?;
        let facts = profile.capabilities();
        if facts.profile_id != P::ID {
            return Err(invalid_profile_fields(
                &["CompileTimeProfile::ID", "capabilities.profile_id"],
                "compile-time identity must match the returned profile specification",
            ));
        }
        if facts.typed_support != <P as CompileTimeProfile>::TYPED_SUPPORT {
            return Err(invalid_profile_fields(
                &[
                    "CompileTimeProfile::TYPED_SUPPORT",
                    "capabilities.typed_support",
                ],
                "compile-time permission set must match the returned profile specification",
            ));
        }
        if capabilities::DomainSet::from_capabilities(facts) != P::DOMAINS {
            return Err(invalid_profile_fields(
                &["CompileTimeProfile::DOMAINS", "capabilities"],
                "compile-time domain set must match the present capability records",
            ));
        }
        if P::SETTLEMENT_SUPPORT.is_some_and(|declared| declared != profile.settlement_support()) {
            return Err(invalid_profile_fields(
                &[
                    "CompileTimeProfile::SETTLEMENT_SUPPORT",
                    "position_inquiries",
                    "supports_operation_complete",
                ],
                "compile-time settlement support must match the profile specification",
            ));
        }
        if profile.transports().transport_set() != P::TRANSPORTS {
            return Err(invalid_profile_fields(
                &["CompileTimeProfile::TRANSPORTS", "transports"],
                "compile-time transport set must match the standard transport compatibility",
            ));
        }
        Ok(profile)
    }

    /// The only constructor: every spec has passed validation.
    pub(crate) fn from_facts(facts: ProfileFacts) -> Result<Self> {
        let (facts, settlement_axes) = facts.validate()?;
        let position_speeds = match &facts.capabilities.pan_tilt {
            Some(domain) => crate::types::PositionSpeeds::from_domain(domain)?,
            None => None,
        };
        let profile = Self {
            facts,
            position_speeds,
            settlement_axes,
        };
        profile.validate_stop_paths()?;
        Ok(profile)
    }

    fn validate_stop_paths(&self) -> Result<()> {
        // A granted STOP must be preparable before any session can rely on its
        // reserved slot. Custom profiles retain their caller-owned authority.
        for stop in crate::command::semantics::StopCommand::ALL {
            if crate::command::surface::row_gate(stop.row()).permits(self.capabilities()) {
                stop.validate(self)?;
            }
        }
        Ok(())
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
    #[cfg(test)]
    pub(crate) fn ensure_compile_time<P>(&self) -> Result<()>
    where
        P: CompileTimeProfile,
    {
        let expected = Self::from_compile_time::<P>()?;
        if self.facts == expected.facts {
            Ok(())
        } else {
            Err(Error::InvalidRequest {
                reason:
                    "registered runtime profile does not match the requested compile-time profile"
                        .into(),
            })
        }
    }

    /// Returns runtime feature and conversion facts.
    #[must_use]
    pub const fn capabilities(&self) -> &capabilities::Capabilities {
        &self.facts.capabilities
    }

    /// Returns the selected wire encoding for each multi-encoding capability.
    ///
    /// A selection is present exactly when its corresponding typed surface is
    /// granted. Requests use these immutable selections during target lowering.
    #[must_use]
    pub const fn encodings(&self) -> &capabilities::ProfileEncodings {
        &self.facts.encodings
    }

    pub(crate) const fn pan_tilt_geometry(&self) -> Option<&capabilities::PanTiltGeometry> {
        match &self.facts.capabilities.pan_tilt {
            Some(domain) => domain.geometry.as_ref(),
            None => None,
        }
    }

    /// Returns the pan/tilt coordinate facts for profiles with sourced geometry.
    #[must_use]
    pub const fn pan_tilt_coordinates(&self) -> Option<PanTiltCoordinateConversion> {
        match self.pan_tilt_geometry() {
            Some(geometry) => Some(geometry.coordinates),
            None => None,
        }
    }

    pub(crate) fn position_speeds(&self) -> Result<crate::types::PositionSpeeds> {
        self.position_speeds.ok_or(Error::FeatureNotSupported {
            feature: "degree-based pan/tilt positioning",
        })
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
    pub const fn maximum_command_sockets(&self) -> crate::CommandSocketCount {
        self.facts.maximum_command_sockets
    }

    /// Returns whether exact operation-complete messages are supported.
    #[must_use]
    pub const fn supports_operation_complete(&self) -> bool {
        self.facts.supports_operation_complete
    }

    /// Returns the axes that targeted operations can settle.
    ///
    /// Exact operation completion can settle any axis admitted by its
    /// capability gate. Otherwise the available position inquiries establish
    /// the axes. Returns `None` when neither source is available.
    #[must_use]
    pub const fn settlement_axes(&self) -> Option<AffectedAxes> {
        self.settlement_axes
    }

    fn settlement_support(&self) -> capabilities::SettlementSupport {
        capabilities::SettlementSupport::from_axes(
            capabilities::DomainSet::from_capabilities(self.capabilities()),
            self.settlement_axes(),
            self.preset_recall_axes(),
        )
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
    pub const fn position_inquiries(&self) -> Option<AffectedAxes> {
        self.facts.position_inquiries
    }

    /// Returns whether the profile declares every selected physical axis.
    ///
    /// This checks the capability inventory, not position-inquiry availability;
    /// polling availability follows each axis's settlement inquiry gate (iris
    /// requires `Iris`, while an `IrisSteps` grant still counts as inventory).
    /// Callers that need to poll must check that [`Self::position_inquiries`]
    /// contains their complete selection.
    #[must_use]
    pub fn supports_axes(&self, axes: AffectedAxes) -> bool {
        self.facts
            .supported_axes()
            .is_some_and(|supported| supported.contains(axes))
    }

    /// Fully validated STOP paths, with one encoding per independent axis set.
    #[cfg(any(feature = "blocking", feature = "async"))]
    pub(crate) fn stop_paths(&self) -> crate::stop_request::StopPaths {
        let mut paths = crate::stop_request::StopPaths::default();
        for stop in crate::command::semantics::StopCommand::ALL {
            if stop.validate(self).is_err() {
                continue;
            }
            let path = if stop.axes() == AffectedAxes::PAN_TILT {
                &mut paths.pan_tilt
            } else if stop.axes() == AffectedAxes::ZOOM {
                &mut paths.zoom
            } else {
                &mut paths.focus
            };
            // Alternative encodings of one axis share its reserve slot.
            path.get_or_insert(*stop);
        }
        paths
    }

    #[cfg(any(feature = "blocking", feature = "async"))]
    /// Each supported STOP path gets a control-reserve admission slot (D26).
    pub(crate) fn typed_stop_paths(&self) -> u8 {
        self.stop_paths().count()
    }

    /// Validates operational overrides without changing profile safety facts.
    ///
    /// Tuning may only make a profile more conservative: pacing and every
    /// deadline, including the retry backoff and budget, may be lengthened
    /// but never shortened, and socket capacity may only be lowered.
    pub fn validate_tuning(&self, tuning: OperationalTuning) -> Result<()> {
        let now = Instant::now();
        let command_overrides =
            CommandTimeoutClass::ALL.map(|category| tuning.command_override(category));
        if [
            tuning.command_spacing(),
            tuning.inquiry_spacing(),
            tuning.ack_timeout(),
            tuning.settlement_timeout(),
            tuning.inquiry_timeout(),
        ]
        .into_iter()
        .chain(command_overrides)
        .flatten()
        .any(|duration| !monotonic_duration_is_representable(now, duration))
        {
            return Err(Error::InvalidRequest {
                reason: "operational timing overrides must be representable by the monotonic clock"
                    .into(),
            });
        }

        if tuning
            .command_spacing()
            .is_some_and(|value| value < self.facts.timing.command_spacing)
            || tuning
                .inquiry_spacing()
                .is_some_and(|value| value < self.facts.timing.inquiry_spacing)
        {
            return Err(Error::InvalidRequest {
                reason: "operational tuning cannot weaken profile pacing minima".into(),
            });
        }
        if tuning
            .maximum_command_sockets()
            .is_some_and(|value| value > self.facts.maximum_command_sockets)
        {
            return Err(Error::InvalidRequest {
                reason: "operational tuning cannot raise the profile socket limit".into(),
            });
        }
        let resolved = crate::timeout::ResolvedTiming::validate(tuning, &self.facts.timing)?;
        // Check the same effective retry budget preparation will lower.
        if CommandTimeoutClass::ALL.into_iter().any(|category| {
            !retry_budget_is_representable(
                now,
                resolved.command(category).get(),
                resolved.ack.get(),
            )
        }) || !retry_budget_is_representable(now, resolved.inquiry.get(), Duration::ZERO)
        {
            return Err(Error::InvalidRequest {
                reason: "operational retry deadlines must be representable by the monotonic clock"
                    .into(),
            });
        }

        let undercuts =
            |value: Option<Duration>, floor: Duration| value.is_some_and(|value| value < floor);
        if undercuts(tuning.ack_timeout(), self.facts.timing.ack_timeout)
            || undercuts(tuning.inquiry_timeout(), self.facts.timing.inquiry_timeout)
            || CommandTimeoutClass::ALL.into_iter().any(|category| {
                undercuts(
                    tuning.command_override(category),
                    self.facts.timing.command_timeouts.get(category),
                )
            })
        {
            return Err(Error::InvalidRequest {
                reason: "operational timeout overrides cannot undercut profile deadlines".into(),
            });
        }
        if let Some(settlement) = tuning.settlement_timeout() {
            let floor = resolved
                .retry_budget(CommandTimeoutClass::Movement)
                .get()
                .min(resolved.retry_budget(CommandTimeoutClass::Preset).get());
            if settlement < floor {
                return Err(Error::InvalidRequest { reason: format!(
                    "operational settlement timeout cannot undercut the smallest targeted retry budget ({floor:?})"
                ).into() });
            }
        }
        // Each request can raise the profile floor, so checking the ordering
        // here also proves it for the bounds lowered during preparation.
        let floor = RetryBounds::for_request(
            self.facts.timing.busy_timeout,
            Duration::ZERO,
            Duration::ZERO,
        );
        for (setter, value, minimum) in [
            (
                "with_initial_retry_backoff",
                tuning.initial_retry_backoff(),
                floor.initial_backoff,
            ),
            (
                "with_maximum_retry_backoff",
                tuning.maximum_retry_backoff(),
                floor.maximum_backoff,
            ),
            (
                "with_retry_budget",
                tuning.retry_budget(),
                floor.total_budget,
            ),
        ] {
            if undercuts(value, minimum) {
                return Err(Error::InvalidRequest {
                    reason: format!("{setter} cannot undercut the profile's {minimum:?} floor")
                        .into(),
                });
            }
            if value.is_some_and(|duration| duration > MAXIMUM_RETRY_TIMING) {
                return Err(Error::InvalidRequest {
                    reason: format!("{setter} cannot exceed {MAXIMUM_RETRY_TIMING:?}").into(),
                });
            }
        }
        let effective = tuning.lengthen_retry_bounds(floor);
        for (bound, maximum) in [
            ("backoff ceiling", effective.maximum_backoff),
            ("retry budget", effective.total_budget),
        ] {
            if effective.initial_backoff > maximum {
                return Err(Error::InvalidRequest {
                    reason: format!(
                        "with_initial_retry_backoff ({:?}) exceeds the effective {bound} ({maximum:?})",
                        effective.initial_backoff,
                    )
                    .into(),
                });
            }
        }
        Ok(())
    }
}

impl ProfileFacts {
    fn supported_axes(&self) -> Option<AffectedAxes> {
        let caps = &self.capabilities;
        AffectedAxes::new_with_iris_nd(
            caps.pan_tilt.is_some(),
            caps.zoom.is_some(),
            caps.focus.is_some(),
            caps.exposure
                .as_ref()
                .is_some_and(|domain| domain.iris_range.is_some())
                || caps.supports(capabilities::TypedSupportSurface::IrisSteps),
            caps.supports(capabilities::TypedSupportSurface::NdFilter),
        )
        .ok()
    }

    fn settlement_axes(&self) -> Option<AffectedAxes> {
        if self.supports_operation_complete {
            self.supported_axes()
        } else {
            self.position_inquiries
        }
    }

    fn validate(mut self) -> Result<(Self, Option<AffectedAxes>)> {
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
        if self.capabilities.model_name.trim().is_empty() {
            return Err(invalid_profile_fields(
                &["capabilities.model_name"],
                "must not be empty",
            ));
        }
        if let Some(profile_id) = self.capabilities.profile_id {
            let registered = profile_id.raw_facts();
            if self != registered {
                let stored = self.capabilities.typed_support;
                let registry = registered.capabilities.typed_support;
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
                        "encodings",
                    ],
                    "built-in identity does not match the runtime protocol identity facts",
                ));
            }
        }
        self.capabilities.validate_domains()?;
        self.encodings.validate(self.capabilities.typed_support)?;
        if let Some((range, encoding)) = self
            .capabilities
            .presets
            .as_ref()
            .and_then(|presets| presets.recall_speed_range)
            .zip(self.encodings.preset_recall_speed())
        {
            let wire_range = encoding.speed_range();
            if range.min() < wire_range.min() || range.max() > wire_range.max() {
                return Err(invalid_profile_fields(
                    &[
                        "capabilities.presets.recall_speed_range",
                        "encodings.preset_recall_speed",
                    ],
                    "advertised recall speeds must fit the selected packet encoding",
                ));
            }
        }
        if !self.capabilities.inquiries {
            let inquiries = self
                .capabilities
                .typed_support
                .iter()
                .filter(|surface| crate::command::surface::is_inquiry_only_surface(*surface))
                .map(|surface| format!("{surface:?}"))
                .collect::<Vec<_>>();
            if !inquiries.is_empty() {
                return Err(invalid_profile_fields(
                    &["capabilities.typed_support", "capabilities.inquiries"],
                    format_args!(
                        "inquiry-only typed support {} requires profile inquiry support",
                        inquiries.join(", ")
                    ),
                ));
            }
        }
        for surface in self.capabilities.typed_support.iter() {
            if !self.capabilities.surface_metadata(surface).permits() {
                let fields: &[&str] = if surface == capabilities::TypedSupportSurface::Gain {
                    &[
                        "capabilities.typed_support",
                        "capabilities.exposure.gain_range",
                    ]
                } else {
                    &["capabilities.typed_support"]
                };
                return Err(invalid_profile_fields(
                    fields,
                    format_args!(
                        "typed support {surface:?} cannot enable an absent physical capability"
                    ),
                ));
            }
        }
        let caps = &self.capabilities;
        let queries = self.position_inquiries;
        if !caps.inquiries && queries.is_some() {
            return Err(invalid_profile_fields(
                &["position_inquiries", "capabilities.inquiries"],
                "position inquiries require profile inquiry support",
            ));
        }
        if queries.is_some_and(|axes| !crate::position_inquiries::permits(caps, axes)) {
            return Err(invalid_profile_fields(
                &[
                    "position_inquiries",
                    "capabilities",
                    "capabilities.typed_support",
                ],
                "position inquiry support names an axis whose settlement inquiry is not permitted",
            ));
        }
        use crate::command::{
            semantics::{BuiltinCommand, BuiltinRequestClass},
            surface::{permits_settlement, row_gate},
        };
        let settlement_axes = self.settlement_axes();
        let recall_axes = caps.presets.as_ref().map(|presets| presets.recall_axes);
        for &command in BuiltinCommand::ALL {
            let gate = row_gate(command);
            if matches!(
                command.classification(),
                BuiltinRequestClass::Targeted { .. }
            ) && gate.requires_typed_support()
                && gate.permits(caps)
                && !permits_settlement(command, settlement_axes, recall_axes)
            {
                return Err(invalid_profile_fields(
                    &["supports_operation_complete", "position_inquiries"],
                    "a granted targeted surface needs settlement for every affected axis",
                ));
            }
        }
        if recall_axes.is_some_and(|axes| {
            !self
                .supported_axes()
                .is_some_and(|supported| supported.contains(axes))
                || (axes.contains(AffectedAxes::IRIS)
                    && !crate::position_inquiries::permits(caps, AffectedAxes::IRIS))
        }) {
            // Recall describes physical effects even if this profile cannot
            // settle those effects. Iris still needs its canonical grant
            // because an iris register alone grants no iris family.
            return Err(invalid_profile_fields(
                &[
                    "capabilities.presets.recall_axes",
                    "capabilities",
                    "capabilities.typed_support",
                ],
                "preset recall requires supported axes and permission for the iris position inquiry",
            ));
        }

        Ok((self, settlement_axes))
    }
}

/// A source-backed compile-time camera profile with one runtime specification.
///
/// Implementations build their facts through the same [`ProfileSpecBuilder`]
/// used by custom runtime profiles, or delegate to a built-in [`crate::profiles::ProfileId::spec`].
/// `TYPED_SUPPORT`, `DOMAINS`, `TRANSPORTS`, `ID` and any declared `SETTLEMENT_SUPPORT` must agree with the returned specification;
/// [`crate::profiles::ProfileSpec::from_compile_time`] checks that contract before registration.
/// Support marker implementations remain separate evidence-backed claims.
///
/// [`crate::profiles::ProfileId::spec`]: crate::profiles::ProfileId::spec
pub trait CompileTimeProfile {
    /// The exact optional typed-surface grants evidenced for this profile.
    const TYPED_SUPPORT: capabilities::TypedSupportSet;

    /// The exact set of present capability records.
    const DOMAINS: capabilities::DomainSet;

    /// The exact set of permitted standard transports.
    const TRANSPORTS: capabilities::TransportSet;

    /// Declares the targeted families the profile can settle.
    ///
    /// A profile without this declaration cannot implement settlement markers.
    const SETTLEMENT_SUPPORT: Option<capabilities::SettlementSupport> = None;

    /// Built-in identity, or `None` for a downstream/custom profile.
    const ID: Option<ProfileId>;

    /// Builds this profile's validated, immutable runtime facts.
    ///
    /// Use [`crate::profiles::ProfileSpec::builder`] for custom facts or a built-in id's
    /// `spec` method. Do not recursively call `from_compile_time::<Self>()`.
    fn spec() -> Result<ProfileSpec>;
}

/// Builder for a validated runtime [`crate::profiles::ProfileSpec`].
///
/// Transport, envelope, timing, socket, completion, cancellation,
/// and per-axis inquiry facts are mandatory. This prevents a generic default
/// from silently granting protocol behavior.
#[derive(Debug, Clone)]
pub struct ProfileSpecBuilder {
    capabilities: capabilities::Capabilities,
    encodings: capabilities::ProfileEncodings,
    transports: Option<TransportCompatibility>,
    envelope: Option<ProfileEnvelope>,
    timing: Option<ProfileTiming>,
    maximum_command_sockets: Option<crate::CommandSocketCount>,
    supports_operation_complete: Option<bool>,
    supports_command_cancel: Option<bool>,
    position_inquiries: Option<Option<AffectedAxes>>,
}

impl ProfileSpecBuilder {
    fn runtime(capabilities: capabilities::Capabilities) -> Self {
        Self {
            capabilities,
            encodings: capabilities::ProfileEncodings::EMPTY,
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
    pub fn maximum_command_sockets(mut self, maximum: crate::CommandSocketCount) -> Self {
        self.maximum_command_sockets = Some(maximum);
        self
    }

    /// Sets the axes with exact position inquiries for settlement.
    ///
    /// Pass `None` explicitly when the profile provides no position inquiries.
    /// This fact is required even for a profile with no inquiry axes.
    #[must_use]
    pub fn position_inquiries(mut self, supported: impl Into<Option<AffectedAxes>>) -> Self {
        self.position_inquiries = Some(supported.into());
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

    /// Declares the menu-control domain.
    #[must_use]
    pub fn menu(mut self, facts: capabilities::MenuCaps) -> Self {
        self.capabilities.menu = Some(facts);
        self
    }

    /// Sets the complete optional typed-surface permission set.
    #[must_use]
    pub fn typed_support(mut self, support: capabilities::TypedSupportSet) -> Self {
        self.capabilities.typed_support = support;
        self
    }

    /// Selects the documented wire encoding of a granted typed surface.
    ///
    /// Both the grant and its encoding are required for a multi-encoding
    /// surface. Repeating a selection replaces that surface's earlier choice;
    /// it never changes the capability grants themselves.
    #[must_use]
    pub fn encoding(mut self, selection: impl Into<capabilities::EncodingSelection>) -> Self {
        self.encodings = self.encodings.with_encoding(selection.into());
        self
    }

    /// Sets whether the camera answers VISCA inquiries.
    #[must_use]
    pub fn inquiries(mut self, supported: bool) -> Self {
        self.capabilities.inquiries = supported;
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

        ProfileSpec::from_facts(ProfileFacts {
            capabilities: self.capabilities,
            encodings: self.encodings,
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

pub use crate::timeout::{CommandTimeoutClass, CommandTimeouts};

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
            geometry: Some(crate::capabilities::PanTiltGeometry {
                pan_range: CapabilityRange::<i32>::new(-100, 100).expect("valid capability bounds"),
                tilt_ranges: TiltRanges::Single(
                    CapabilityRange::<i32>::new(-50, 50).expect("valid capability bounds"),
                ),
                coordinates: conversion(
                    CoordinateSystem::SignedCentered,
                    PanTiltWireCodec::StandardVisca,
                    10.0,
                    10.0,
                ),
                endpoint_quantization: EndpointQuantization::Exact,
            }),
            pan_speed: CapabilityRange::<u8>::new(1, 8).expect("valid capability bounds"),
            tilt_speed: CapabilityRange::<u8>::new(1, 8).expect("valid capability bounds"),
            variable_speed: false,
            motion_sync_speed_range: None,
        });
        caps.zoom = Some(ZoomCaps {
            optical_max: 1000,
            digital_max: None,
            speed: CapabilityRange::<u8>::new(0, 7).expect("valid capability bounds"),
            direct: false,
            variable: false,
            optical_ratio: Some(10.0),
        });
        caps.focus = Some(FocusCaps {
            range: CapabilityRange::<u16>::new(0, 2000).expect("valid capability bounds"),
            near_end: FocusNearEnd::Minimum,
            speed: CapabilityRange::<u8>::new(0, 7).expect("valid capability bounds"),
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
            recall_speed_range: Some(
                CapabilityRange::<u8>::new(1, 8).expect("valid capability bounds"),
            ),
        });
        caps.typed_support = TypedSupportSet::from_surface(TypedSupportSurface::PanTiltPositioning);
        caps.inquiries = true;
        caps
    }
    fn runtime_builder(mut capabilities: Capabilities) -> ProfileSpecBuilder {
        if capabilities
            .pan_tilt
            .is_some_and(|domain| domain.geometry.is_some())
        {
            capabilities.typed_support =
                capabilities
                    .typed_support
                    .union(TypedSupportSet::from_surface(
                        TypedSupportSurface::PanTiltPositioning,
                    ));
        }
        ProfileSpec::builder(capabilities)
            .transports(TransportCompatibility::new().with_tcp(5678))
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
                    .inquiry_spacing(Duration::from_millis(25))
                    .command_spacing(Duration::from_millis(25))
                    .build()
                    .expect("valid timing"),
            )
            .maximum_command_sockets(crate::types::CommandSocketCount::ONE)
            .supports_operation_complete(false)
            .supports_command_cancel(false)
            .position_inquiries(AffectedAxes::PAN_TILT | AffectedAxes::ZOOM | AffectedAxes::FOCUS)
    }
    fn conservative_runtime_builder(capabilities: Capabilities) -> ProfileSpecBuilder {
        ProfileSpec::builder(capabilities)
            .transports(TransportCompatibility::new().with_tcp(5678))
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
                    .inquiry_spacing(Duration::ZERO)
                    .command_spacing(Duration::ZERO)
                    .build()
                    .expect("valid timing"),
            )
            .maximum_command_sockets(crate::types::CommandSocketCount::ONE)
            .supports_operation_complete(false)
            .supports_command_cancel(false)
            .position_inquiries(None)
    }

    /// A community profile needs no metadata-trait implementation or registry row.
    struct CommunityProfile;

    impl CompileTimeProfile for CommunityProfile {
        const DOMAINS: capabilities::DomainSet = capabilities::DomainSet::from_domains(&[
            capabilities::Domain::PanTilt,
            capabilities::Domain::Zoom,
            capabilities::Domain::Focus,
            capabilities::Domain::Presets,
        ]);
        const TRANSPORTS: capabilities::TransportSet =
            TransportCompatibility::new().with_tcp(5678).transport_set();
        const TYPED_SUPPORT: TypedSupportSet =
            TypedSupportSet::from_surface(TypedSupportSurface::PanTiltPositioning);
        const ID: Option<crate::profiles::ProfileId> = None;

        fn spec() -> Result<ProfileSpec> {
            runtime_builder(valid_runtime_capabilities()).build()
        }
    }

    #[test]
    fn community_compile_time_profiles_use_the_same_validated_builder() {
        let declared = CommunityProfile::spec().expect("community specification");
        let lowered = ProfileSpec::from_compile_time::<CommunityProfile>()
            .expect("matching compile-time declaration");
        assert_eq!(declared, lowered);
        assert_eq!(lowered.capabilities().profile_id, None);
        assert_eq!(
            lowered.capabilities().typed_support,
            <CommunityProfile as CompileTimeProfile>::TYPED_SUPPORT
        );
        lowered
            .ensure_compile_time::<CommunityProfile>()
            .expect("the exact facts bind the typed view");
    }

    #[test]
    fn compile_time_declarations_must_match_their_returned_specification() {
        struct WrongIdentity;
        impl CompileTimeProfile for WrongIdentity {
            const DOMAINS: capabilities::DomainSet =
                <CommunityProfile as CompileTimeProfile>::DOMAINS;
            const TRANSPORTS: capabilities::TransportSet =
                <CommunityProfile as CompileTimeProfile>::TRANSPORTS;
            const TYPED_SUPPORT: TypedSupportSet = TypedSupportSet::EMPTY;
            const ID: Option<crate::profiles::ProfileId> =
                Some(crate::profiles::ProfileId::GenericVisca);

            fn spec() -> Result<ProfileSpec> {
                CommunityProfile::spec()
            }
        }

        struct WrongPermissionSet;
        impl CompileTimeProfile for WrongPermissionSet {
            const DOMAINS: capabilities::DomainSet =
                <CommunityProfile as CompileTimeProfile>::DOMAINS;
            const TRANSPORTS: capabilities::TransportSet =
                <CommunityProfile as CompileTimeProfile>::TRANSPORTS;
            const TYPED_SUPPORT: TypedSupportSet =
                TypedSupportSet::from_surface(TypedSupportSurface::DirectZoom);
            const ID: Option<crate::profiles::ProfileId> = None;

            fn spec() -> Result<ProfileSpec> {
                CommunityProfile::spec()
            }
        }

        struct WrongDomains;
        impl CompileTimeProfile for WrongDomains {
            const ID: Option<ProfileId> = CommunityProfile::ID;
            const TYPED_SUPPORT: TypedSupportSet =
                <CommunityProfile as CompileTimeProfile>::TYPED_SUPPORT;
            const DOMAINS: capabilities::DomainSet = capabilities::DomainSet::empty();
            const TRANSPORTS: capabilities::TransportSet =
                <CommunityProfile as CompileTimeProfile>::TRANSPORTS;

            fn spec() -> Result<ProfileSpec> {
                CommunityProfile::spec()
            }
        }

        struct WrongTransports;
        impl CompileTimeProfile for WrongTransports {
            const ID: Option<ProfileId> = CommunityProfile::ID;
            const TYPED_SUPPORT: TypedSupportSet =
                <CommunityProfile as CompileTimeProfile>::TYPED_SUPPORT;
            const DOMAINS: capabilities::DomainSet =
                <CommunityProfile as CompileTimeProfile>::DOMAINS;
            const TRANSPORTS: capabilities::TransportSet = capabilities::TransportSet::empty();

            fn spec() -> Result<ProfileSpec> {
                CommunityProfile::spec()
            }
        }

        struct WrongSettlement;
        impl CompileTimeProfile for WrongSettlement {
            const ID: Option<ProfileId> = CommunityProfile::ID;
            const TYPED_SUPPORT: TypedSupportSet =
                <CommunityProfile as CompileTimeProfile>::TYPED_SUPPORT;
            const DOMAINS: capabilities::DomainSet =
                <CommunityProfile as CompileTimeProfile>::DOMAINS;
            const TRANSPORTS: capabilities::TransportSet =
                <CommunityProfile as CompileTimeProfile>::TRANSPORTS;
            const SETTLEMENT_SUPPORT: Option<capabilities::SettlementSupport> =
                Some(capabilities::SettlementSupport::from_axes(
                    <Self as CompileTimeProfile>::DOMAINS,
                    None,
                    None,
                ));

            fn spec() -> Result<ProfileSpec> {
                CommunityProfile::spec()
            }
        }

        let settlement_error = ProfileSpec::from_compile_time::<WrongSettlement>()
            .expect_err("declared settlement must match the profile");
        assert!(
            settlement_error
                .to_string()
                .contains("CompileTimeProfile::SETTLEMENT_SUPPORT"),
            "{settlement_error}"
        );

        let domain_error = ProfileSpec::from_compile_time::<WrongDomains>()
            .expect_err("declared domains must match the present capability records");
        assert!(
            domain_error
                .to_string()
                .contains("CompileTimeProfile::DOMAINS"),
            "{domain_error}"
        );
        let transport_error = ProfileSpec::from_compile_time::<WrongTransports>()
            .expect_err("declared transports must match compatibility");
        assert!(
            transport_error
                .to_string()
                .contains("CompileTimeProfile::TRANSPORTS"),
            "{transport_error}"
        );

        let identity_error = ProfileSpec::from_compile_time::<WrongIdentity>()
            .expect_err("a built-in id cannot name custom facts")
            .to_string();
        assert!(
            identity_error.contains("CompileTimeProfile::ID"),
            "{identity_error}"
        );
        let support_error = ProfileSpec::from_compile_time::<WrongPermissionSet>()
            .expect_err("typed grants must match the declared facts")
            .to_string();
        assert!(
            support_error.contains("CompileTimeProfile::TYPED_SUPPORT"),
            "{support_error}"
        );
        let profile = CommunityProfile::spec().expect("valid custom facts");
        assert!(profile.ensure_compile_time::<WrongIdentity>().is_err());
        assert!(profile.ensure_compile_time::<WrongPermissionSet>().is_err());
    }

    #[cfg(any(feature = "blocking", feature = "async"))]
    #[test]
    fn halt_paths_and_control_reserve_share_every_validated_stop() {
        use crate::{config::OperationalTuning, session_config::ValidatedTuning};
        for id in ProfileId::all() {
            // Exercise both the source-backed registry and caller-owned
            // custom authority with the same facts, including FR7 aliases.
            for custom in [false, true] {
                let mut facts = id.raw_facts();
                if custom {
                    facts.capabilities.profile_id = None;
                }
                let profile = ProfileSpec::from_facts(facts).expect("valid profile");
                let target =
                    crate::TargetRegistration::new(crate::IndividualCameraId::CAMERA_1, profile);
                let tuning =
                    ValidatedTuning::validate(OperationalTuning::new(), [target.profile()])
                        .expect("valid tuning");
                let halt = crate::prepared::prepare_halt(&target, tuning);
                assert_eq!(
                    halt.requests
                        .iter()
                        .filter(|request| matches!(request, Some(Ok(_))))
                        .count(),
                    usize::from(target.profile().typed_stop_paths()),
                    "{id:?}, custom={custom}",
                );
                for (path, request) in target
                    .profile()
                    .stop_paths()
                    .rows()
                    .into_iter()
                    .zip(halt.requests)
                {
                    assert_eq!(path.is_some(), request.is_some());
                    if let (Some(stop), Some(request)) = (path, request) {
                        let request = request.expect("selected STOP prepares");
                        assert_eq!(
                            request.context().motion.map(|motion| motion.axes),
                            Some(stop.axes())
                        );
                        assert!(request.is_stop());
                    }
                }
            }
        }
    }

    #[test]
    fn stop_validation_refuses_missing_builtin_sources_but_keeps_custom_authority() {
        use crate::command::semantics::StopCommand;
        let mut facts = ProfileId::PtzOpticsG2.raw_facts();
        facts.capabilities.typed_support =
            facts
                .capabilities
                .typed_support
                .union(capabilities::TypedSupportSet::from_surface(
                    capabilities::TypedSupportSurface::HighResolutionZoom,
                ));
        // Bypass identity validation to isolate the precise gate/source
        // mismatch. The same check runs in from_facts before publishing a spec.
        let forged = ProfileSpec {
            settlement_axes: facts.settlement_axes(),
            facts: facts.clone(),
            position_speeds: None,
        };
        assert!(
            crate::command::surface::row_gate(StopCommand::Fr7ZoomHighResolutionStop.row())
                .permits(forged.capabilities())
        );
        assert!(matches!(
            StopCommand::Fr7ZoomHighResolutionStop.validate(&forged),
            Err(Error::FeatureNotSupported { .. })
        ));
        // Exercise the exact granted-STOP validation performed by from_facts,
        // without letting its separate built-in identity guard mask the cause.
        assert!(matches!(forged.validate_stop_paths(),
            Err(Error::FeatureNotSupported { feature })
                if feature == crate::command::surface::surface_entry(
                    StopCommand::Fr7ZoomHighResolutionStop.row()).feature()));
        facts.capabilities.profile_id = None;
        let custom = ProfileSpec {
            settlement_axes: facts.settlement_axes(),
            facts,
            position_speeds: None,
        };
        custom
            .validate_stop_paths()
            .expect("custom specs explicitly own their protocol authority");
    }

    #[test]
    fn every_builtin_spec_uses_its_one_raw_registry_fact_record() {
        for id in crate::profiles::ProfileId::all() {
            let profile = id.spec().expect("validated built-in registry row");
            assert_eq!(profile.capabilities().profile_id, Some(*id));
            assert_eq!(profile.facts, id.raw_facts());
        }
    }
    fn invalid_request_message<T>(result: Result<T>) -> Option<String> {
        match result {
            Err(Error::InvalidRequest { reason: message }) => Some(message.into_owned()),
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
            CommandTimeouts::DEFAULT
                .with_quick_timeout(Duration::from_secs(10))
                .with_movement_timeout(Duration::from_secs(30))
                .with_preset_timeout(Duration::from_secs(60))
                .with_long_running_timeout(Duration::from_secs(300))
                .with_network_timeout(Duration::from_secs(10))
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
            .inquiry_spacing(Duration::from_millis(25))
            .command_spacing(Duration::from_millis(50));

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
        missing_timing_field!(inquiry_spacing, "inquiry_spacing");
        missing_timing_field!(command_spacing, "command_spacing");

        let timing = complete.build().expect("complete timing facts");

        assert_eq!(timing.ack_timeout(), Duration::from_millis(100));
        assert_eq!(timing.command_timeouts(), CommandTimeouts::default());
        assert_eq!(timing.inquiry_timeout(), Duration::from_secs(1));
        assert_eq!(timing.cancellation_timeout(), Duration::from_secs(2));
        assert_eq!(timing.ambiguity_timeout(), Duration::from_secs(3));
        assert_eq!(timing.busy_timeout(), Duration::from_secs(4));
        assert_eq!(timing.raw_inquiry_reply_skew(), Duration::from_millis(25));
        assert_eq!(timing.inquiry_spacing(), Duration::from_millis(25));
        assert_eq!(timing.command_spacing(), Duration::from_millis(50));

        assert!(ProfileTiming::builder()
            .ack_timeout(Duration::ZERO)
            .command_timeouts(CommandTimeouts::default())
            .inquiry_timeout(Duration::from_secs(1))
            .cancellation_timeout(Duration::from_secs(1))
            .ambiguity_timeout(Duration::from_secs(1))
            .busy_timeout(Duration::ZERO)
            .raw_inquiry_reply_skew(Duration::ZERO)
            .inquiry_spacing(Duration::ZERO)
            .command_spacing(Duration::ZERO)
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
                .inquiry_spacing(Duration::ZERO)
                .command_spacing(Duration::ZERO)
        };

        for result in [
            complete().ack_timeout(Duration::MAX).build(),
            complete().inquiry_timeout(Duration::MAX).build(),
            complete().cancellation_timeout(Duration::MAX).build(),
            complete().ambiguity_timeout(Duration::MAX).build(),
            complete().busy_timeout(Duration::MAX).build(),
            complete().raw_inquiry_reply_skew(Duration::MAX).build(),
            complete().inquiry_spacing(Duration::MAX).build(),
            complete().command_spacing(Duration::MAX).build(),
        ] {
            assert!(result.is_err());
        }

        assert!(complete()
            .raw_inquiry_reply_skew(Duration::from_nanos(1))
            .build()
            .is_err());

        let default = CommandTimeouts::default();
        let command_timeout_cases = [
            CommandTimeouts::DEFAULT
                .with_quick_timeout(Duration::MAX)
                .with_movement_timeout(default.movement_timeout())
                .with_preset_timeout(default.preset_timeout())
                .with_long_running_timeout(default.long_running_timeout())
                .with_network_timeout(default.network_timeout()),
            CommandTimeouts::DEFAULT
                .with_quick_timeout(default.quick_timeout())
                .with_movement_timeout(Duration::MAX)
                .with_preset_timeout(default.preset_timeout())
                .with_long_running_timeout(default.long_running_timeout())
                .with_network_timeout(default.network_timeout()),
            CommandTimeouts::DEFAULT
                .with_quick_timeout(default.quick_timeout())
                .with_movement_timeout(default.movement_timeout())
                .with_preset_timeout(Duration::MAX)
                .with_long_running_timeout(default.long_running_timeout())
                .with_network_timeout(default.network_timeout()),
            CommandTimeouts::DEFAULT
                .with_quick_timeout(default.quick_timeout())
                .with_movement_timeout(default.movement_timeout())
                .with_preset_timeout(default.preset_timeout())
                .with_long_running_timeout(Duration::MAX)
                .with_network_timeout(default.network_timeout()),
            CommandTimeouts::DEFAULT
                .with_quick_timeout(default.quick_timeout())
                .with_movement_timeout(default.movement_timeout())
                .with_preset_timeout(default.preset_timeout())
                .with_long_running_timeout(default.long_running_timeout())
                .with_network_timeout(Duration::MAX),
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
            .command_timeouts(
                CommandTimeouts::DEFAULT
                    .with_quick_timeout(Duration::from_secs(5))
                    .with_movement_timeout(Duration::from_secs(30))
                    .with_preset_timeout(Duration::from_secs(60))
                    .with_long_running_timeout(Duration::from_secs(300))
                    .with_network_timeout(Duration::from_secs(5)),
            )
            .inquiry_timeout(Duration::from_secs(1))
            .cancellation_timeout(Duration::from_secs(1))
            .ambiguity_timeout(Duration::from_secs(1))
            .busy_timeout(Duration::ZERO)
            .raw_inquiry_reply_skew(Duration::ZERO)
            .inquiry_spacing(Duration::ZERO)
            .command_spacing(Duration::ZERO)
            .build()
            .expect("ordinary five-minute LongRunning deadline is representable");

        assert_eq!(
            timing.command_timeouts().long_running_timeout(),
            Duration::from_secs(300)
        );
    }

    #[test]
    fn runtime_command_timeout_values_are_explicit_and_nonzero_validated() {
        let exact = CommandTimeouts::DEFAULT
            .with_quick_timeout(Duration::from_secs(6))
            .with_movement_timeout(Duration::from_secs(7))
            .with_preset_timeout(Duration::from_secs(8))
            .with_long_running_timeout(Duration::from_secs(9))
            .with_network_timeout(Duration::from_secs(10));
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
                    .inquiry_spacing(Duration::from_millis(25))
                    .command_spacing(Duration::from_millis(25))
                    .build()
                    .expect("valid timing"),
            )
            .build()
            .expect("exact runtime category values");
        assert_eq!(profile.command_timeouts(), exact);

        assert!(CommandTimeouts::DEFAULT
            .with_quick_timeout(Duration::ZERO)
            .with_movement_timeout(Duration::from_secs(1))
            .with_preset_timeout(Duration::from_secs(1))
            .with_long_running_timeout(Duration::from_secs(1))
            .with_network_timeout(Duration::from_secs(1))
            .validate()
            .is_err());
    }

    #[test]
    fn extended_speed_ranges_require_the_variable_speed_fact() {
        for codec in [PanTiltWireCodec::StandardVisca, PanTiltWireCodec::SonyFr7] {
            for pan_axis in [true, false] {
                let mut caps = valid_runtime_capabilities();
                let pan_tilt = caps.pan_tilt.as_mut().expect("pan/tilt");
                pan_tilt.geometry.as_mut().expect("geometry").coordinates =
                    conversion(CoordinateSystem::SignedCentered, codec, 10.0, 10.0);
                let field = if pan_axis {
                    pan_tilt.pan_speed =
                        CapabilityRange::<u8>::new(1, 50).expect("valid capability bounds");
                    "capabilities.pan_tilt.pan_speed"
                } else {
                    pan_tilt.tilt_speed =
                        CapabilityRange::<u8>::new(1, 50).expect("valid capability bounds");
                    "capabilities.pan_tilt.tilt_speed"
                };
                let error = runtime_builder(caps.clone())
                    .build()
                    .expect_err("unreachable speed range");
                let message = error.to_string();
                assert!(message.contains(field), "{message}");
                assert!(
                    message.contains("capabilities.pan_tilt.variable_speed"),
                    "{message}"
                );
                caps.pan_tilt.as_mut().expect("pan/tilt").variable_speed = true;
                let message = runtime_builder(caps.clone())
                    .build()
                    .expect_err("the fine-mode setter needs its permission")
                    .to_string();
                assert!(message.contains(field), "{message}");
                assert!(message.contains("capabilities.typed_support"), "{message}");
                caps.typed_support = caps.typed_support.union(TypedSupportSet::from_surface(
                    TypedSupportSurface::PanTiltSpeedMode,
                ));
                runtime_builder(caps)
                    .build()
                    .expect("a variable-speed profile can reach the fine range");
            }
        }
    }

    #[cfg(feature = "serde")]
    #[test]
    fn persisted_extended_speed_ranges_require_the_variable_speed_fact() {
        let profile = runtime_builder(valid_runtime_capabilities())
            .build()
            .expect("valid profile");
        for field in ["pan_speed", "tilt_speed"] {
            let mut saved = serde_json::to_value(&profile).expect("serialize profile");
            saved["capabilities"]["pan_tilt"][field] = serde_json::json!({"min": 1, "max": 50});
            let message = serde_json::from_value::<ProfileSpec>(saved.clone())
                .expect_err("reject unreachable speed ranges on load")
                .to_string();
            assert!(
                message.contains(&format!("capabilities.pan_tilt.{field}")),
                "{message}"
            );
            assert!(
                message.contains("capabilities.pan_tilt.variable_speed"),
                "{message}"
            );
            saved["capabilities"]["pan_tilt"]["variable_speed"] = serde_json::json!(true);
            let message = serde_json::from_value::<ProfileSpec>(saved)
                .expect_err("reject a fine range without the speed-mode permission")
                .to_string();
            assert!(
                message.contains(&format!("capabilities.pan_tilt.{field}")),
                "{message}"
            );
            assert!(message.contains("capabilities.typed_support"), "{message}");
        }
    }

    #[test]
    fn tuning_cannot_weaken_pacing_or_raise_socket_capacity() {
        let profile = runtime_builder(valid_runtime_capabilities())
            .build()
            .expect("valid runtime profile");
        assert!(profile
            .validate_tuning(
                OperationalTuning::new().with_command_spacing(Duration::from_millis(1))
            )
            .is_err());
        assert!(profile
            .validate_tuning(
                OperationalTuning::new()
                    .with_maximum_command_sockets(crate::types::CommandSocketCount::TWO)
            )
            .is_err());
        assert!(profile
            .validate_tuning(OperationalTuning::new().with_ack_timeout(Duration::from_nanos(1)))
            .is_err());
        assert!(profile
            .validate_tuning(OperationalTuning::new().with_inquiry_timeout(Duration::from_nanos(1)))
            .is_err());
        for tuning in [
            OperationalTuning::new().with_quick_timeout(Duration::from_nanos(1)),
            OperationalTuning::new().with_movement_timeout(Duration::from_nanos(1)),
            OperationalTuning::new().with_preset_timeout(Duration::from_nanos(1)),
            OperationalTuning::new().with_long_running_timeout(Duration::from_nanos(1)),
            OperationalTuning::new().with_network_timeout(Duration::from_nanos(1)),
        ] {
            assert!(profile.validate_tuning(tuning).is_err());
        }
        assert!(profile
            .validate_tuning(OperationalTuning::new().with_quick_timeout(Duration::ZERO))
            .is_err());
    }

    #[test]
    fn settlement_tuning_must_lengthen_at_least_one_targeted_retry_budget() {
        let profile =
            ProfileSpec::from_compile_time::<crate::profiles::PtzOpticsG2>().expect("G2 profile");
        for seconds in [2, 20, 59] {
            let error = profile
                .validate_tuning(
                    OperationalTuning::new().with_settlement_timeout(Duration::from_secs(seconds)),
                )
                .expect_err("an override below the retry floor has no effect");
            assert!(
                matches!(error, Error::InvalidRequest { reason: message } if message.contains("60s"))
            );
        }
        for seconds in [60, 61, 90, 120] {
            profile
                .validate_tuning(
                    OperationalTuning::new().with_settlement_timeout(Duration::from_secs(seconds)),
                )
                .expect("an override at or above the floor is useful");
        }
        let tuned = OperationalTuning::new()
            .with_movement_timeout(Duration::from_secs(90))
            .with_preset_timeout(Duration::from_secs(70));
        assert!(profile
            .validate_tuning(tuned.with_settlement_timeout(Duration::from_secs(139)))
            .is_err());
        profile
            .validate_tuning(tuned.with_settlement_timeout(Duration::from_secs(140)))
            .expect("the smaller Preset retry budget sets the floor");
        let tuned_retry = OperationalTuning::new()
            .with_initial_retry_backoff(Duration::from_millis(50))
            .with_maximum_retry_backoff(Duration::from_millis(500))
            .with_retry_budget(Duration::from_secs(150));
        assert!(profile
            .validate_tuning(tuned_retry.with_settlement_timeout(Duration::from_secs(149)))
            .is_err());
        profile
            .validate_tuning(tuned_retry.with_settlement_timeout(Duration::from_secs(150)))
            .expect("retry tuning participates in the settlement floor");
    }

    #[test]
    fn tuning_rejects_unrepresentable_deadline_and_spacing_overrides() {
        let profile = runtime_builder(valid_runtime_capabilities())
            .build()
            .expect("valid runtime profile");

        for tuning in [
            OperationalTuning::new().with_command_spacing(Duration::MAX),
            OperationalTuning::new().with_inquiry_spacing(Duration::MAX),
            OperationalTuning::new().with_ack_timeout(Duration::MAX),
            OperationalTuning::new().with_quick_timeout(Duration::MAX),
            OperationalTuning::new().with_movement_timeout(Duration::MAX),
            OperationalTuning::new().with_preset_timeout(Duration::MAX),
            OperationalTuning::new().with_long_running_timeout(Duration::MAX),
            OperationalTuning::new().with_network_timeout(Duration::MAX),
            OperationalTuning::new().with_settlement_timeout(Duration::MAX),
            OperationalTuning::new().with_inquiry_timeout(Duration::MAX),
        ] {
            assert!(profile.validate_tuning(tuning).is_err());
        }
    }

    #[test]
    fn absurd_retry_timing_is_rejected_before_it_can_invert_into_zero_retries() {
        let profile = runtime_builder(valid_runtime_capabilities())
            .build()
            .expect("valid runtime profile");
        let sane = OperationalTuning::new()
            .with_initial_retry_backoff(Duration::from_millis(50))
            .with_maximum_retry_backoff(Duration::from_millis(500))
            .with_retry_budget(Duration::from_secs(10));
        assert!(profile.validate_tuning(sane).is_ok());

        for budget in [
            Duration::MAX,
            Duration::MAX - Duration::from_secs(1),
            MAXIMUM_RETRY_TIMING + Duration::from_nanos(1),
        ] {
            let inverted = OperationalTuning::new()
                .with_initial_retry_backoff(Duration::from_millis(50))
                .with_maximum_retry_backoff(Duration::from_millis(500))
                .with_retry_budget(budget);
            assert!(
                profile.validate_tuning(inverted).is_err(),
                "a {budget:?} retry budget must be rejected, not saturated into zero retries"
            );
        }

        // The same saturation turns an absurd ceiling into no backoff at all.
        assert!(profile
            .validate_tuning(
                OperationalTuning::new()
                    .with_initial_retry_backoff(Duration::from_millis(50))
                    .with_maximum_retry_backoff(Duration::MAX)
                    .with_retry_budget(Duration::from_secs(10))
            )
            .is_err());
        // Exactly at the bound is still accepted.
        assert!(profile
            .validate_tuning(
                OperationalTuning::new()
                    .with_initial_retry_backoff(Duration::from_millis(50))
                    .with_maximum_retry_backoff(MAXIMUM_RETRY_TIMING)
                    .with_retry_budget(MAXIMUM_RETRY_TIMING)
            )
            .is_ok());
    }

    #[test]
    fn retry_timing_tuning_may_only_lengthen_the_profile_bounds() {
        let profile = runtime_builder(valid_runtime_capabilities())
            .build()
            .expect("valid runtime profile");
        type Setter = fn(OperationalTuning, Duration) -> OperationalTuning;
        let fields: [(&str, Setter, u64); 3] = [
            (
                "with_initial_retry_backoff",
                OperationalTuning::with_initial_retry_backoff,
                50,
            ),
            (
                "with_maximum_retry_backoff",
                OperationalTuning::with_maximum_retry_backoff,
                500,
            ),
            (
                "with_retry_budget",
                OperationalTuning::with_retry_budget,
                10_000,
            ),
        ];
        for mask in 0..8 {
            let tuning = fields.iter().enumerate().fold(
                OperationalTuning::new(),
                |tuning, (index, &(_, setter, floor))| {
                    if mask & (1 << index) != 0 {
                        setter(tuning, Duration::from_millis(floor))
                    } else {
                        tuning
                    }
                },
            );
            assert!(
                profile.validate_tuning(tuning).is_ok(),
                "overrides {mask:03b}"
            );
            for (index, &(name, setter, floor)) in fields.iter().enumerate() {
                if mask & (1 << index) != 0 {
                    let error = invalid_request_message(
                        profile.validate_tuning(setter(tuning, Duration::from_millis(floor - 1))),
                    )
                    .expect("a shortened retry bound must be rejected");
                    assert!(error.contains(name), "{name}: {error}");
                }
            }
            if mask & 1 != 0 {
                let wider = tuning.with_initial_retry_backoff(Duration::from_secs(2));
                let error = invalid_request_message(profile.validate_tuning(wider))
                    .expect("the initial backoff must fit the unchanged ceiling");
                assert!(error.contains("with_initial_retry_backoff") && error.contains("500ms"));
                assert!(profile
                    .validate_tuning(wider.with_maximum_retry_backoff(Duration::from_secs(3)))
                    .is_ok());
            }
        }

        // A profile busy timeout raises the floors it governs.
        let busy = ProfileSpec::builder(valid_runtime_capabilities())
            .transports(TransportCompatibility::new().with_tcp(5678))
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
                    .inquiry_spacing(Duration::from_millis(25))
                    .command_spacing(Duration::from_millis(25))
                    .build()
                    .expect("valid timing"),
            )
            .maximum_command_sockets(crate::types::CommandSocketCount::ONE)
            .supports_operation_complete(false)
            .supports_command_cancel(false)
            .position_inquiries(AffectedAxes::PAN_TILT | AffectedAxes::ZOOM | AffectedAxes::FOCUS)
            .build()
            .expect("valid busy-timeout profile");
        let tuning = OperationalTuning::new();
        assert!(busy
            .validate_tuning(tuning.with_maximum_retry_backoff(Duration::from_millis(11_999)))
            .is_err());
        assert!(busy
            .validate_tuning(tuning.with_retry_budget(Duration::from_millis(11_999)))
            .is_err());
        assert!(busy
            .validate_tuning(tuning.with_retry_budget(Duration::from_secs(12)))
            .is_ok());
    }

    #[test]
    fn retry_limit_tuning_may_only_lower_the_default_base() {
        let profile = runtime_builder(valid_runtime_capabilities())
            .build()
            .expect("valid runtime profile");
        for count in 0..=crate::tuning::RetryLimit::MAX.value() {
            let limit = crate::tuning::RetryLimit::new(count).expect("valid retry limit");
            let tuning = OperationalTuning::new().with_retry_limit(limit);
            assert!(profile.validate_tuning(tuning).is_ok());
            assert_eq!(
                tuning.reduce_retry_base(DEFAULT_RETRY_BASE),
                u32::from(count)
            );
        }
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
        use crate::command::pan_tilt_wire::PanTiltFraming;
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
        let brc300 = crate::profiles::SonyBrc300::spec()
            .expect("BRC profile")
            .pan_tilt_coordinates()
            .expect("BRC coordinate conversion");
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
            .transports(TransportCompatibility::new().with_serial())
            .envelope(ProfileEnvelope::SonyEncapsulated)
            .build()
            .is_err());
        assert!(runtime_builder(valid_runtime_capabilities())
            .transports(TransportCompatibility::new().with_tcp(52381).with_serial())
            .envelope(ProfileEnvelope::SonyEncapsulated)
            .build()
            .is_err());
        assert!(runtime_builder(valid_runtime_capabilities())
            .transports(TransportCompatibility::new().with_tcp(52381))
            .envelope(ProfileEnvelope::SonyEncapsulated)
            .build()
            .is_ok());
    }

    #[test]
    fn grouped_ranges_fit_the_selected_wire_and_typed_domains() {
        let mut caps = valid_runtime_capabilities();
        let pt = caps
            .pan_tilt
            .as_mut()
            .expect("pan/tilt")
            .geometry
            .as_mut()
            .expect("geometry");
        pt.coordinates = conversion(
            CoordinateSystem::SignedCentered,
            PanTiltWireCodec::SonyBrc300,
            -1.0,
            1.0,
        );
        pt.pan_range =
            CapabilityRange::<i32>::new(-0x80000, 0x7FFFF).expect("valid capability bounds");
        pt.tilt_ranges = TiltRanges::Single(
            CapabilityRange::<i32>::new(-0x8000, 0x7FFF).expect("valid capability bounds"),
        );
        assert!(runtime_builder(caps.clone()).build().is_ok());
        caps.pan_tilt
            .as_mut()
            .expect("pan/tilt")
            .geometry
            .as_mut()
            .expect("geometry")
            .pan_range =
            CapabilityRange::<i32>::new(-0x80001, 0x7FFFF).expect("valid capability bounds");
        assert!(runtime_builder(caps).build().is_err());

        let mut caps = valid_runtime_capabilities();
        caps.pan_tilt
            .as_mut()
            .expect("pan/tilt")
            .geometry
            .as_mut()
            .expect("geometry")
            .tilt_ranges = TiltRanges::Single(
            CapabilityRange::<i32>::new(-0x8000, 0x8000).expect("valid capability bounds"),
        );
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

        for field in ["optical_max", "digital_max"] {
            let mut caps = valid_runtime_capabilities();
            let zoom = caps.zoom.as_mut().expect("zoom");
            if field == "optical_max" {
                zoom.optical_max = 0x8000;
            } else {
                zoom.digital_max = Some(0x8000);
            }
            let error = runtime_builder(caps)
                .build()
                .expect_err("out of ZoomPosition domain");
            assert!(error.to_string().contains(field), "{error}");
        }
        let mut caps = valid_runtime_capabilities();
        caps.zoom.as_mut().expect("zoom").speed =
            CapabilityRange::<u8>::new(0, 8).expect("valid capability bounds");
        assert!(runtime_builder(caps).build().is_err());
        let mut caps = valid_runtime_capabilities();
        caps.presets.as_mut().expect("presets").recall_speed_range =
            Some(CapabilityRange::<u8>::new(1, 200).expect("valid capability bounds"));
        assert!(runtime_builder(caps).build().is_err());
    }

    #[test]
    fn recall_speed_ranges_fit_the_selected_packet_encoding() {
        use capabilities::PresetRecallSpeedEncoding;
        for (encoding, last) in [
            (PresetRecallSpeedEncoding::PtzOptics, 0x18),
            (PresetRecallSpeedEncoding::SonyFr7, 50),
        ] {
            let mut facts = ProfileId::PtzOpticsG2.raw_facts();
            facts.capabilities.profile_id = None;
            facts.encodings = facts.encodings.with_encoding(encoding.into());
            facts
                .capabilities
                .presets
                .as_mut()
                .expect("presets")
                .recall_speed_range =
                Some(CapabilityRange::<u8>::new(1, last).expect("valid capability bounds"));
            ProfileSpec::from_facts(facts.clone()).expect("complete encoding range");
            for range in [
                CapabilityRange::<u8>::new(0, last).expect("valid capability bounds"),
                CapabilityRange::<u8>::new(1, last + 1).expect("valid capability bounds"),
            ] {
                facts
                    .capabilities
                    .presets
                    .as_mut()
                    .expect("presets")
                    .recall_speed_range = Some(range);
                let error = ProfileSpec::from_facts(facts.clone()).expect_err("outside encoding");
                assert!(error.to_string().contains("recall_speed_range"), "{error}");
            }
        }
    }

    #[test]
    fn position_speed_grammars_validate_shared_speed_before_lowering() {
        use crate::{command::pan_tilt_wire::PanTiltFraming, types::SpeedLevel};
        for codec in [
            PanTiltWireCodec::StandardVisca,
            PanTiltWireCodec::SonyBrc300,
            PanTiltWireCodec::SonyFr7,
        ] {
            for (pan_max, tilt_min) in [(4, 5), (5, 5), (8, 5)] {
                let mut facts = ProfileId::PtzOpticsG2.raw_facts();
                facts.capabilities.profile_id = None;
                let pan_tilt = facts.capabilities.pan_tilt.as_mut().expect("pan/tilt");
                let coordinates = conversion(CoordinateSystem::SignedCentered, codec, 14.4, 14.4);
                pan_tilt.geometry.as_mut().expect("geometry").coordinates = coordinates;
                pan_tilt.pan_speed = CapabilityRange::<u8>::new(1, pan_max).expect("pan range");
                pan_tilt.tilt_speed = CapabilityRange::<u8>::new(tilt_min, 24).expect("tilt range");

                let result = ProfileSpec::from_facts(facts.clone());
                #[cfg(feature = "serde")]
                let restored = serde_json::from_value::<ProfileSpec>(
                    serde_json::to_value(&facts).expect("profile JSON"),
                );
                if codec == PanTiltWireCodec::SonyBrc300 && pan_max < tilt_min {
                    let assert_refusal = |reason: &str| {
                        assert!(
                            reason.contains("capabilities.pan_tilt.pan_speed"),
                            "{reason}"
                        );
                        assert!(
                            reason.contains("capabilities.pan_tilt.tilt_speed"),
                            "{reason}"
                        );
                    };
                    assert_refusal(&result.expect_err("disjoint ranges").to_string());
                    #[cfg(feature = "serde")]
                    assert_refusal(&restored.expect_err("disjoint ranges").to_string());
                    continue;
                }
                let spec = result.expect("compatible ranges");
                #[cfg(feature = "serde")]
                assert_eq!(restored.expect("compatible persisted ranges"), spec);
                let speeds = spec.position_speeds().expect("position speeds");
                for level in [
                    SpeedLevel::Slowest,
                    SpeedLevel::Slow,
                    SpeedLevel::Medium,
                    SpeedLevel::Fast,
                    SpeedLevel::Fastest,
                ] {
                    let lowered = speeds.lower(level);
                    assert!(PanTiltFraming::for_conversion(coordinates)
                        .move_speeds(lowered.pan, lowered.tilt)
                        .is_ok());
                    let (baseline_pan, baseline_tilt, _, _) = level.baseline_speeds();
                    let expected = if codec == PanTiltWireCodec::SonyBrc300 {
                        let shared = baseline_pan.clamp(tilt_min, pan_max);
                        (shared, shared)
                    } else {
                        (
                            baseline_pan.clamp(1, pan_max),
                            baseline_tilt.clamp(tilt_min, 24),
                        )
                    };
                    assert_eq!(
                        (lowered.pan.value(), lowered.tilt.value()),
                        expected,
                        "{codec:?} {level:?}"
                    );
                }
            }
        }
    }

    #[test]
    fn sourced_pan_tilt_endpoints_prepare_to_their_wire_words() {
        use crate::{
            request::{builtin::PanTiltAbsolute, Request},
            types::{Degrees, PanSpeed, TiltSpeed},
            CameraId, IndividualCameraId, MountPosition, TargetRegistration,
        };

        #[derive(Clone, Copy)]
        struct Axis {
            degrees: [f32; 2],
            words: [u32; 2],
            nibbles: usize,
        }

        let axis = |degrees, words, nibbles| Axis {
            degrees,
            words,
            nibbles,
        };
        let ptzoptics_pan = axis([-170.0, 170.0], [0xF670, 0x0990], 4);
        let ptzoptics_tilt = axis([-30.0, 90.0], [0xFE50, 0x0510], 4);
        let fr7_pan = axis([-170.0, 170.0], [0x09CA7, 0xF6359], 5);
        // The BRC limits use exact wire words with approximate degree scales.
        let brc300_pan = axis([-35416.0 / 208.0, 35416.0 / 208.0], [0x08A58, 0xF75A8], 5);
        let brc300_tilt = axis([-6250.0 / 208.0, 18749.0 / 208.0], [0xE796, 0x493D], 4);
        let cases = [
            (ProfileId::PtzOpticsG2, None, ptzoptics_pan, ptzoptics_tilt),
            (ProfileId::PtzOpticsG3, None, ptzoptics_pan, ptzoptics_tilt),
            (ProfileId::PtzOptics30x, None, ptzoptics_pan, ptzoptics_tilt),
            (
                ProfileId::SonyFr7,
                None,
                fr7_pan,
                axis([-30.0, 195.0], [0xFE45B, 0x0B3B0], 5),
            ),
            (
                ProfileId::SonyFr7,
                Some(MountPosition::Ceiling),
                fr7_pan,
                axis([-210.0, 15.0], [0xF3E7D, 0x00DD2], 5),
            ),
            (
                ProfileId::SonyBrcH900,
                None,
                axis([-39902.0 / 236.0, 39902.0 / 236.0], [0x09BDE, 0xF6422], 5),
                axis([-7080.0 / 236.0, 21240.0 / 236.0], [0xE458, 0x52F8], 4),
            ),
            (
                ProfileId::SonyEviH100,
                None,
                axis([-170.0, 170.0], [0xE1E5, 0x1E1B], 4),
                axis([-20.0, 90.0], [0xFC75, 0x0FF0], 4),
            ),
            (ProfileId::SonyBrc300, None, brc300_pan, brc300_tilt),
            (ProfileId::NearusBrc300, None, brc300_pan, brc300_tilt),
        ];
        // Generic VISCA's member sources do not agree on degree endpoints.
        for (id, mount, pan, tilt) in cases {
            let spec = id.spec().expect("built-in spec");
            let mut target = TargetRegistration::new(IndividualCameraId::CAMERA_1, spec);
            if let Some(mount) = mount {
                target = target.with_mount(mount).expect("supported mount");
            }
            let pan_range = target
                .profile()
                .capabilities()
                .pan_tilt
                .as_ref()
                .expect("pan/tilt")
                .geometry
                .expect("geometry")
                .pan_range_degrees();
            let tilt_range = target.tilt_range_degrees().expect("tilt range");
            assert_eq!(
                pan_range,
                pan.degrees[0]..=pan.degrees[1],
                "{id:?} {mount:?}"
            );
            assert_eq!(
                tilt_range,
                tilt.degrees[0]..=tilt.degrees[1],
                "{id:?} {mount:?}"
            );

            let request_at = |pan, tilt| {
                PanTiltAbsolute::for_target(
                    Degrees::new(pan).expect("pan degrees"),
                    Degrees::new(tilt).expect("tilt degrees"),
                    PanSpeed::new(1).expect("pan speed"),
                    TiltSpeed::new(1).expect("tilt speed"),
                    &target,
                )
            };
            let word = |nibbles: &[u8]| {
                nibbles
                    .iter()
                    .fold(0, |word, &nibble| (word << 4) | u32::from(nibble))
            };
            for i in 0..2 {
                let request =
                    request_at(pan.degrees[i], tilt.degrees[i]).expect("source endpoints");
                let mut frame = [0; PanTiltAbsolute::MAX_SIZE];
                let len = request
                    .write_into(CameraId::CAMERA_1, &mut frame)
                    .expect("absolute frame");
                assert_eq!(len, 7 + pan.nibbles + tilt.nibbles);
                let tilt_start = 6 + pan.nibbles;
                assert_eq!(
                    word(&frame[6..tilt_start]),
                    pan.words[i],
                    "{id:?} {mount:?}"
                );
                assert_eq!(
                    word(&frame[tilt_start..len - 1]),
                    tilt.words[i],
                    "{id:?} {mount:?}"
                );
            }
            for (outside_pan, outside_tilt) in [
                (pan.degrees[0] - 0.5, 0.0),
                (pan.degrees[1] + 0.5, 0.0),
                (0.0, tilt.degrees[0] - 0.5),
                (0.0, tilt.degrees[1] + 0.5),
            ] {
                assert!(
                    matches!(
                        request_at(outside_pan, outside_tilt),
                        Err(Error::InvalidParameter { .. })
                    ),
                    "{id:?} {mount:?} admitted {outside_pan}° pan, {outside_tilt}° tilt"
                );
            }
        }
    }

    #[test]
    fn degree_readback_preserves_custom_endpoints_that_nominal_rounding_would_alias() {
        let mut caps = valid_runtime_capabilities();
        let geometry = caps
            .pan_tilt
            .as_mut()
            .expect("pan/tilt")
            .geometry
            .as_mut()
            .expect("geometry");
        geometry.pan_range = CapabilityRange::new(-101, 99).expect("pan range");
        geometry.coordinates = conversion(
            CoordinateSystem::SignedCentered,
            PanTiltWireCodec::StandardVisca,
            -10.0,
            10.0,
        );
        geometry.endpoint_quantization = EndpointQuantization::WholeDegrees;

        let target = crate::TargetRegistration::new(
            crate::IndividualCameraId::CAMERA_1,
            runtime_builder(caps).build().expect("custom geometry"),
        );
        let coordinates = target
            .pan_tilt_coordinates()
            .expect("registered coordinates");
        let read = |pan| coordinates.position_degrees(crate::types::PanTiltUnits::new(pan, 0));
        // The true upper endpoint is +10.1°. Reporting +10° instead would
        // move a caller's next command to the adjacent raw word -100.
        assert_eq!(read(-101).expect("sourced endpoint").pan.value(), 10.1);
        assert_eq!(
            target
                .convert_pan_tilt_degrees(10.0, 0.0)
                .expect("nominal endpoint"),
            (-100, 0)
        );
        // +100 fits the nominal degree interval but is outside the raw range.
        assert!(matches!(
            read(100),
            Err(Error::InvalidParameter {
                parameter: "pan position",
                ..
            })
        ));
    }

    #[test]
    fn coordinate_conversion_uses_selected_bounds_and_one_endpoint_policy() {
        let spec = ProfileSpec::from_compile_time::<crate::profiles::SonyFr7>().expect("FR7");
        let desktop = crate::TargetRegistration::new(crate::IndividualCameraId::CAMERA_1, spec);
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
        caps.pan_tilt
            .as_mut()
            .expect("pan/tilt")
            .geometry
            .as_mut()
            .expect("geometry")
            .coordinates = conversion(
            CoordinateSystem::UnsignedCentered,
            PanTiltWireCodec::StandardVisca,
            10.0,
            10.0,
        );
        let target = crate::TargetRegistration::new(
            crate::IndividualCameraId::CAMERA_1,
            runtime_builder(caps).build().expect("unsigned profile"),
        );
        assert_eq!(
            target
                .convert_pan_tilt_degrees(0.0, 0.0)
                .expect("logical center"),
            (0, 0)
        );
        let request = crate::request::builtin::PanTiltAbsolute::for_target(
            crate::units::Degrees::new(0.0).expect("finite angle"),
            crate::units::Degrees::new(0.0).expect("finite angle"),
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
            .position_inquiries(AffectedAxes::PAN_TILT | AffectedAxes::FOCUS)
            .build()
            .is_err());
        assert!(runtime_builder(valid_runtime_capabilities())
            .position_inquiries(AffectedAxes::ZOOM | AffectedAxes::FOCUS)
            .build()
            .is_err());
        assert!(runtime_builder(valid_runtime_capabilities())
            .supports_operation_complete(true)
            .position_inquiries(None)
            .build()
            .is_ok());
    }

    #[test]
    fn inquiry_only_grants_require_profile_inquiry_support() {
        let mut caps = Capabilities::runtime_baseline("Version fixture", 1).expect("baseline");
        caps.typed_support = TypedSupportSet::from_surface(TypedSupportSurface::VersionInquiry);
        assert!(!caps
            .surface_metadata(TypedSupportSurface::VersionInquiry)
            .permits());
        let error = conservative_runtime_builder(caps.clone())
            .build()
            .expect_err("an inquiry grant cannot override no inquiry support")
            .to_string();
        for field in [
            "capabilities.typed_support",
            "capabilities.inquiries",
            "VersionInquiry",
        ] {
            assert!(error.contains(field), "{error}");
        }
        caps.inquiries = true;
        assert!(caps
            .surface_metadata(TypedSupportSurface::VersionInquiry)
            .permits());
        conservative_runtime_builder(caps)
            .build()
            .expect("inquiry support");
    }

    #[test]
    fn a_shared_digital_zoom_range_grants_control_without_inquiries() {
        let mut caps = Capabilities::runtime_baseline("Digital zoom control", 1).expect("baseline");
        caps.zoom = Some(capabilities::ZoomCaps {
            optical_max: 0x4000,
            digital_max: Some(0x7000),
            speed: capabilities::CapabilityRange::<u8>::new(0, 7).expect("valid capability bounds"),
            direct: true,
            variable: false,
            optical_ratio: None,
        });
        caps.typed_support = TypedSupportSet::from_surfaces(&[
            TypedSupportSurface::DirectZoom,
            TypedSupportSurface::DigitalZoomRange,
        ]);
        assert!(!caps.inquiries);
        assert!(caps
            .surface_metadata(TypedSupportSurface::DigitalZoomRange)
            .permits());
        let profile = conservative_runtime_builder(caps)
            .supports_operation_complete(true)
            .build()
            .expect("command-only digital zoom profile");
        let target = crate::TargetRegistration::new(crate::IndividualCameraId::CAMERA_1, profile);
        let request = crate::command::zoom::SetNormalized::new(
            crate::units::UnitInterval::MAX,
            crate::ZoomDomain::OpticalPlusDigital,
        );
        crate::Request::validate_for_target(&request, &target).expect("control admission");
    }

    #[test]
    fn discovery_requires_inquiry_support_for_every_inquiry_only_grant() {
        let profile = ProfileSpec::from_compile_time::<crate::profiles::PtzOpticsG2>().expect("G2");
        let mut caps = profile.capabilities().clone();
        caps.inquiries = false;
        for surface in caps.typed_support.iter() {
            if crate::command::surface::is_inquiry_only_surface(surface) {
                assert!(!caps.surface_metadata(surface).permits(), "{surface:?}");
            }
        }
        assert!(caps.supports(TypedSupportSurface::DirectZoom));
    }

    #[cfg(feature = "serde")]
    #[test]
    fn persisted_inquiry_only_grants_cannot_disable_profile_inquiry_support() {
        let mut caps = Capabilities::runtime_baseline("Version fixture", 1).expect("baseline");
        caps.typed_support = TypedSupportSet::from_surface(TypedSupportSurface::VersionInquiry);
        caps.inquiries = true;
        let profile = conservative_runtime_builder(caps)
            .build()
            .expect("valid inquiries");
        let mut json = serde_json::to_value(profile).expect("serialize");
        json["capabilities"]["inquiries"] = serde_json::Value::Bool(false);
        let error = serde_json::from_value::<ProfileSpec>(json)
            .expect_err("contradictory persisted spec")
            .to_string();
        assert!(error.contains("capabilities.typed_support"), "{error}");
        assert!(error.contains("capabilities.inquiries"), "{error}");
    }

    #[test]
    fn typed_permissions_require_the_domain_and_each_value_inventory() {
        for surface in [
            TypedSupportSurface::DirectZoom,
            TypedSupportSurface::FocusZoneInquiry,
            TypedSupportSurface::PresetRecallSpeed,
        ] {
            let mut caps = valid_runtime_capabilities();
            caps.presets.as_mut().expect("presets").recall_speed_range = None;
            caps.typed_support = TypedSupportSet::from_surface(surface);
            assert!(runtime_builder(caps).build().is_err(), "{surface:?}");
        }
        // USB audio has no separate capability-domain record or numeric
        // inventory. Its explicit grant is the complete capability fact.
        let mut usb = Capabilities::runtime_baseline("USB audio fixture", 1).expect("baseline");
        usb.typed_support = TypedSupportSet::from_surface(TypedSupportSurface::UsbAudio);
        assert!(conservative_runtime_builder(usb).build().is_ok());
        for refinement in [
            TypedSupportSurface::TallyBrightness,
            TypedSupportSurface::TallyMode,
        ] {
            let mut caps = valid_runtime_capabilities();
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
            assert!(crate::IndividualCameraId::new(invalid).is_err());
        }
        for number in 1..=7 {
            let id = crate::IndividualCameraId::new(number).expect("individual");
            let caps =
                Capabilities::runtime_baseline("Individual default", number).expect("baseline");
            let profile = conservative_runtime_builder(caps).build().expect("profile");
            let config = crate::SessionConfig::new(profile);
            assert_eq!(config.sole_target(), Some(id));
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
            status_query: crate::capabilities::PowerStatusQuery::Standard,
            standby: true,
        });
        let profile = conservative_runtime_builder(caps)
            .build()
            .expect("power only");
        assert!(profile.pan_tilt_coordinates().is_none());
        assert!(profile.preset_recall_axes().is_none());
    }

    fn copy_builder(base: &ProfileSpec, caps: Capabilities) -> ProfileSpecBuilder {
        let mut builder = ProfileSpec::builder(caps)
            .transports(base.transports())
            .envelope(base.envelope())
            .timing(base.timing())
            .maximum_command_sockets(base.maximum_command_sockets())
            .supports_operation_complete(base.supports_operation_complete())
            .supports_command_cancel(base.supports_command_cancel())
            .position_inquiries(base.position_inquiries());
        for selection in base.encodings().iter() {
            builder = builder.encoding(selection);
        }
        builder
    }

    #[test]
    fn settlement_axes_follow_exact_completion_or_validated_inquiries() {
        let generic = ProfileId::GenericVisca.spec().expect("generic profile");
        assert_eq!(
            generic.settlement_axes(),
            Some(AffectedAxes::ZOOM | AffectedAxes::FOCUS | AffectedAxes::IRIS)
        );
        for id in ProfileId::all() {
            let profile = id.spec().expect("built-in profile");
            if profile.supports_operation_complete() {
                assert_eq!(
                    profile.settlement_axes(),
                    Some(AffectedAxes::MOVEMENT | AffectedAxes::IRIS)
                );
            }
        }

        let mut caps = Capabilities::runtime_baseline("Pan and tilt only", 1).expect("baseline");
        caps.pan_tilt = generic.capabilities().pan_tilt;
        let profile = conservative_runtime_builder(caps.clone())
            .build()
            .expect("pan/tilt without geometry or polling");
        assert_eq!(profile.settlement_axes(), None);
        let target = crate::TargetRegistration::new(crate::IndividualCameraId::CAMERA_1, profile);
        assert!(matches!(
            crate::Request::validate_for_target(&crate::command::pan_tilt::Home, &target),
            Err(Error::FeatureNotSupported { .. })
        ));

        let completing = conservative_runtime_builder(caps)
            .supports_operation_complete(true)
            .build()
            .expect("exact completion without geometry");
        assert_eq!(completing.settlement_axes(), Some(AffectedAxes::PAN_TILT));
        let target =
            crate::TargetRegistration::new(crate::IndividualCameraId::CAMERA_1, completing);
        crate::Request::validate_for_target(&crate::command::pan_tilt::Home, &target)
            .expect("home supported");
    }

    #[test]
    fn direct_zoom_grant_requires_zoom_settlement() {
        let built_in = ProfileId::PtzOpticsG2.spec().expect("G2 profile");
        let mut caps = Capabilities::runtime_baseline("Zoom camera", 1).expect("baseline");
        caps.zoom = built_in.capabilities().zoom;
        caps.typed_support = TypedSupportSet::from_surface(TypedSupportSurface::DirectZoom);
        let error = conservative_runtime_builder(caps)
            .build()
            .expect_err("no zoom settlement");
        assert!(error.to_string().contains("position_inquiries"));
    }

    #[test]
    fn iris_settlement_requires_the_inquiry_grant_for_queries_and_recall() {
        for recall in [false, true] {
            let mut facts = ProfileId::PtzOpticsG2.raw_facts();
            facts.capabilities.profile_id = None;
            facts.supports_operation_complete = false;
            facts.capabilities.typed_support = facts
                .capabilities
                .typed_support
                .without(TypedSupportSurface::Iris);
            if recall {
                facts.position_inquiries =
                    Some(AffectedAxes::PAN_TILT | AffectedAxes::ZOOM | AffectedAxes::FOCUS);
                facts
                    .capabilities
                    .presets
                    .as_mut()
                    .expect("presets")
                    .recall_axes = AffectedAxes::IRIS;
            }
            let error = facts
                .clone()
                .validate()
                .expect_err("iris grant is required")
                .to_string();
            assert!(error.contains("capabilities.typed_support"), "{error}");
            let field = if recall {
                "capabilities.presets.recall_axes"
            } else {
                "position_inquiries"
            };
            assert!(error.contains(field), "{error}");

            facts.capabilities.typed_support = facts
                .capabilities
                .typed_support
                .union(TypedSupportSet::from_surface(TypedSupportSurface::Iris));
            facts.position_inquiries =
                Some(facts.position_inquiries.expect("inquiries") | AffectedAxes::IRIS);
            facts
                .validate()
                .expect("the public iris inquiry is now admitted");
        }
    }

    #[test]
    fn every_position_axis_is_validated_against_its_public_inquiry() {
        use crate::command::inquiry::{
            FocusPositionInquiry, IrisInquiry, NdFilterInquiry, PanTiltPositionInquiry,
            ZoomPositionInquiry,
        };

        fn check<I: crate::Inquiry>(
            inquiry: I,
            axis: AffectedAxes,
            remove: impl FnOnce(&mut Capabilities),
        ) {
            let mut facts = ProfileId::PtzOpticsG2.raw_facts();
            facts.capabilities.profile_id = None;
            facts.capabilities.typed_support = TypedSupportSet::from_surfaces(&[
                TypedSupportSurface::Iris,
                TypedSupportSurface::NdFilter,
                TypedSupportSurface::PanTiltPositioning,
            ]);
            facts.capabilities.presets = None;
            facts.encodings = capabilities::ProfileEncodings::default();
            facts.supports_operation_complete = true;
            facts.position_inquiries = Some(axis);
            let profile = ProfileSpec::from_facts(facts.clone()).expect("permitted axis");
            let target = crate::TargetRegistration::from_profile(
                crate::target::TargetSlot::CAMERA_1,
                profile,
            );
            inquiry
                .validate_for_target(&target)
                .expect("public inquiry admits the axis");

            remove(&mut facts.capabilities);
            // Check the inquiry's independent admission rule even though this
            // deliberately invalid profile cannot be built by a caller.
            let target = crate::TargetRegistration::from_profile(
                crate::target::TargetSlot::CAMERA_1,
                ProfileSpec {
                    settlement_axes: facts.settlement_axes(),
                    facts: facts.clone(),
                    position_speeds: None,
                },
            );
            assert!(matches!(
                inquiry.validate_for_target(&target),
                Err(Error::FeatureNotSupported { .. })
            ));
            let error = facts
                .validate()
                .expect_err("the inquiry gate denies this axis")
                .to_string();
            assert!(error.contains("position_inquiries"), "{error}");
        }

        check(PanTiltPositionInquiry, AffectedAxes::PAN_TILT, |caps| {
            caps.pan_tilt = None;
            caps.typed_support = caps
                .typed_support
                .without(TypedSupportSurface::PanTiltPositioning);
        });
        check(ZoomPositionInquiry, AffectedAxes::ZOOM, |caps| {
            caps.zoom = None
        });
        check(FocusPositionInquiry, AffectedAxes::FOCUS, |caps| {
            caps.focus = None
        });
        check(IrisInquiry, AffectedAxes::IRIS, |caps| {
            caps.typed_support = TypedSupportSet::empty()
        });
        check(NdFilterInquiry, AffectedAxes::ND_FILTER, |caps| {
            caps.typed_support = TypedSupportSet::empty()
        });
    }

    #[test]
    fn iris_steps_remain_inventory_without_granting_position_polling() {
        let mut facts = ProfileId::PtzOpticsG2.raw_facts();
        facts.capabilities.profile_id = None;
        facts.capabilities.typed_support =
            TypedSupportSet::from_surface(TypedSupportSurface::IrisSteps);
        facts
            .capabilities
            .exposure
            .as_mut()
            .expect("exposure")
            .iris_range = None;
        facts.capabilities.presets = None;
        facts.encodings = capabilities::ProfileEncodings::default();
        facts.supports_operation_complete = true;
        facts.position_inquiries = None;
        let profile = ProfileSpec::from_facts(facts).expect("step-only iris inventory");
        assert!(profile.supports_axes(AffectedAxes::IRIS));
        assert!(!crate::position_inquiries::permits(
            profile.capabilities(),
            AffectedAxes::IRIS
        ));
    }

    #[test]
    fn encoding_selection_is_required_exactly_for_granted_multi_encoding_surfaces() {
        use capabilities::PresetRecallSpeedEncoding;
        let mut caps = valid_runtime_capabilities();
        caps.typed_support = TypedSupportSet::from_surface(TypedSupportSurface::PresetRecallSpeed);
        assert!(
            runtime_builder(caps.clone()).build().is_err(),
            "a grant cannot infer its encoding"
        );
        let selected = runtime_builder(caps)
            .encoding(PresetRecallSpeedEncoding::PtzOptics)
            .build()
            .expect("explicitly selected recall-speed encoding");
        assert_eq!(
            selected.encodings().preset_recall_speed(),
            Some(PresetRecallSpeedEncoding::PtzOptics)
        );
        assert!(
            runtime_builder(valid_runtime_capabilities())
                .encoding(PresetRecallSpeedEncoding::PtzOptics)
                .build()
                .is_err(),
            "an encoding cannot create a capability grant"
        );
    }

    #[test]
    fn builtin_encoding_selections_follow_source_backed_grants_and_identity() {
        use capabilities::{OnePushWhiteBalanceEncoding, PresetRecallSpeedEncoding};
        for id in crate::profiles::ProfileId::all() {
            let profile = id
                .spec()
                .expect("built-in facts select every required encoding");
            let support = profile.capabilities().typed_support;
            assert_eq!(
                profile.encodings().preset_recall_speed().is_some(),
                support.contains(TypedSupportSurface::PresetRecallSpeed)
            );
            assert_eq!(
                profile.encodings().one_push_white_balance().is_some(),
                support.contains(TypedSupportSurface::OnePushWhiteBalance)
            );
        }
        let ptzoptics = crate::profiles::ProfileId::PtzOpticsG2
            .spec()
            .expect("PTZOptics");
        assert_eq!(
            ptzoptics.encodings().preset_recall_speed(),
            Some(PresetRecallSpeedEncoding::PtzOptics)
        );
        assert_eq!(
            ptzoptics.encodings().one_push_white_balance(),
            Some(OnePushWhiteBalanceEncoding::Standard)
        );
        let sony = crate::profiles::ProfileId::SonyFr7.spec().expect("FR7");
        assert_eq!(
            sony.encodings().preset_recall_speed(),
            Some(PresetRecallSpeedEncoding::SonyFr7)
        );
        assert_eq!(
            sony.encodings().one_push_white_balance(),
            Some(OnePushWhiteBalanceEncoding::SonyFr7)
        );
        let recall_range = sony
            .capabilities()
            .presets
            .as_ref()
            .expect("FR7 presets")
            .recall_speed_range
            .as_ref()
            .expect("1C common recall-speed domain");
        assert_eq!((recall_range.min(), recall_range.max()), (1, 50));
        let mut changed = ptzoptics.facts.clone();
        changed.encodings = changed
            .encodings
            .with_encoding(PresetRecallSpeedEncoding::SonyFr7.into());
        let error =
            ProfileSpec::from_facts(changed).expect_err("an encoding is an exact identity fact");
        assert!(error.to_string().contains("encodings"));
    }

    #[cfg(feature = "serde")]
    #[test]
    fn persisted_encoding_selections_are_explicit_strict_and_round_trip() {
        use capabilities::PresetRecallSpeedEncoding;
        let mut caps = valid_runtime_capabilities();
        caps.typed_support = TypedSupportSet::from_surface(TypedSupportSurface::PresetRecallSpeed);
        let profile = runtime_builder(caps)
            .encoding(PresetRecallSpeedEncoding::PtzOptics)
            .build()
            .expect("runtime profile");
        let saved = serde_json::to_value(&profile).expect("serialize selected encoding");
        assert_eq!(
            saved["encodings"],
            serde_json::json!({"preset_recall_speed": "ptzoptics"})
        );
        assert_eq!(
            serde_json::from_value::<ProfileSpec>(saved.clone()).expect("round trip"),
            profile
        );
        for encodings in [
            serde_json::json!({}),
            serde_json::json!({"unknown_encoding":"standard"}),
            serde_json::json!({"preset_recall_speed":"unknown"}),
            serde_json::json!({"preset_recall_speed":"ptzoptics","one_push_white_balance":"standard"}),
        ] {
            let mut invalid = saved.clone();
            invalid["encodings"] = encodings;
            assert!(serde_json::from_value::<ProfileSpec>(invalid).is_err());
        }
        let mut missing = saved;
        missing.as_object_mut().expect("object").remove("encodings");
        assert!(
            serde_json::from_value::<ProfileSpec>(missing).is_err(),
            "old specs cannot silently infer encoding selections"
        );
    }

    #[test]
    fn built_in_identity_covers_every_protocol_fact() {
        use crate::profiles::PtzOpticsG2;

        let base = ProfileSpec::from_compile_time::<PtzOpticsG2>().expect("built-in profile");
        let rebuilt = |capabilities| copy_builder(&base, capabilities);
        let unclaimed_capabilities = || {
            let mut capabilities = base.capabilities().clone();
            capabilities.profile_id = None;
            capabilities
        };
        let altered_timing = ProfileTiming {
            command_spacing: base.timing().command_spacing + Duration::from_millis(1),
            ..base.timing()
        };
        // Toggle instead of adding IRIS: G2 already declares that inquiry.
        // A mutation fixture must actually differ from the registered facts.
        let altered_position_inquiries = AffectedAxes::from_bits(
            base.position_inquiries().expect("G2 inquiries").bits() ^ AffectedAxes::IRIS.bits(),
        )
        .ok();
        assert_ne!(altered_position_inquiries, base.position_inquiries());

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
            .transports(TransportCompatibility::new().with_tcp(5678).with_udp(1259)));
        assert_identity_mismatch!("timing", |builder: ProfileSpecBuilder| builder
            .timing(altered_timing));
        assert_identity_mismatch!("maximum_command_sockets", |builder: ProfileSpecBuilder| {
            builder.maximum_command_sockets(
                if base.maximum_command_sockets() == crate::CommandSocketCount::ONE {
                    crate::CommandSocketCount::TWO
                } else {
                    crate::CommandSocketCount::ONE
                },
            )
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
        let shutter = caps.exposure.as_mut().expect("exposure");
        let speed =
            capabilities::ShutterSpeedEntry::new(crate::units::ExposureTime::reciprocal(60), 1);
        shutter.shutter_speeds.extend([speed, speed]);
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
        caps.exposure.as_mut().expect("exposure").iris_range = Some(
            crate::capabilities::CapabilityDomain::<u16>::new(0, 256, &[][..])
                .expect("valid capability bounds"),
        );
        assert!(copy_builder(&base, caps).build().is_err());
        let mut caps = custom();
        caps.pan_tilt
            .as_mut()
            .expect("pan/tilt")
            .motion_sync_speed_range =
            Some(CapabilityRange::<u8>::new(0, 24).expect("valid capability bounds"));
        assert!(copy_builder(&base, caps).build().is_err());
        let mut caps = custom();
        caps.white_balance.as_mut().expect("WB").modes.clear();
        assert!(copy_builder(&base, caps).build().is_err());
        let mut caps = custom();
        caps.focus.as_mut().expect("focus").auto_focus = false;
        caps.focus.as_mut().expect("focus").af_sensitivity = true;
        assert!(copy_builder(&base, caps).build().is_err());
        let mut caps = custom();
        caps.exposure = None;
        caps.typed_support =
            TypedSupportSet::from_surface(TypedSupportSurface::BacklightCompensation);
        assert!(copy_builder(&base, caps).build().is_err());
    }

    #[test]
    fn zero_is_one_preset_and_the_highest_number_is_inclusive() {
        let mut caps = valid_runtime_capabilities();
        caps.typed_support = TypedSupportSet::from_surface(TypedSupportSurface::PresetMemory);
        caps.presets.as_mut().expect("presets").highest = 0;
        let profile = runtime_builder(caps).build().expect("preset zero");
        let registration =
            crate::TargetRegistration::new(crate::IndividualCameraId::CAMERA_1, profile);
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
        assert!(matches!(&builder_error, Error::InvalidRequest { .. }));
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
        let repeated: serde_json::Error = persisted_spec_error(&error);
        assert_eq!(repeated.to_string(), error);
    }

    #[cfg(feature = "serde")]
    #[test]
    fn saved_builtin_typed_support_keeps_one_model_regeneration_instruction() {
        let base = ProfileSpec::from_compile_time::<crate::profiles::SonyFr7>().expect("FR7");
        let mut caps = base.capabilities().clone();
        caps.typed_support = caps
            .typed_support
            .without(capabilities::TypedSupportSurface::VersionInquiry);
        let builder_error = copy_builder(&base, caps.clone())
            .build()
            .expect_err("the built-in identity must retain its current typed support");
        assert!(matches!(&builder_error, Error::InvalidRequest { .. }));
        let cause = builder_error.to_string();
        assert!(cause.contains("missing: VersionInquiry"), "{cause}");
        assert!(
            cause.ends_with(
                "`ProfileSpec::from_compile_time::<grafton_visca::profiles::SonyFr7>()` \
                 and persist the result"
            ),
            "{cause}"
        );

        let mut saved = serde_json::to_value(&base).expect("serialize current spec");
        saved["capabilities"] = serde_json::to_value(caps).expect("serialize stale support");
        let _: ProfileFacts = serde_json::from_value(saved.clone())
            .expect("a stale support set retains the complete persisted structure");
        let error = serde_json::from_value::<ProfileSpec>(saved)
            .expect_err("the saved built-in support must match the current registry")
            .to_string();
        assert_eq!(
            error
                .matches("the spec was saved by another release")
                .count(),
            1,
            "{error}"
        );
        assert_eq!(
            error, cause,
            "retain the full model-specific builder message"
        );
    }

    #[cfg(feature = "serde")]
    #[test]
    fn instruction_text_in_an_unknown_key_cannot_suppress_regeneration_guidance() {
        let base = ProfileSpec::from_compile_time::<crate::profiles::PtzOpticsG2>().expect("G2");
        let mut saved = serde_json::to_value(base).expect("serialize current spec");
        let unknown_key = "the spec was saved by another release, so regenerate it with \
            `ProfileSpec::from_compile_time::<grafton_visca::profiles::PtzOpticsG2>()` \
            and persist the result";
        saved
            .as_object_mut()
            .expect("saved spec is an object")
            .insert(unknown_key.into(), serde_json::Value::Null);
        let error = serde_json::from_value::<ProfileSpec>(saved)
            .expect_err("instruction text is still an unknown persisted key")
            .to_string();
        assert!(error.contains("unknown field"), "{error}");
        assert!(error.contains(unknown_key), "{error}");
        assert_eq!(error.matches(REGENERATE_SPEC).count(), 1, "{error}");
        assert!(error.ends_with(REGENERATE_SPEC), "{error}");
    }

    #[cfg(feature = "serde")]
    #[test]
    fn grouped_profile_persistence_requires_current_keys_and_validates_values() {
        let base = ProfileSpec::from_compile_time::<crate::profiles::PtzOpticsG2>().expect("G2");
        let value = serde_json::to_value(&base).expect("serialize");
        for count in [0, 3] {
            let mut invalid = value.clone();
            invalid["maximum_command_sockets"] = serde_json::json!(count);
            assert!(serde_json::from_value::<ProfileFacts>(invalid.clone()).is_err());
            assert!(serde_json::from_value::<ProfileSpec>(invalid).is_err());
        }
        for (path, key) in [
            ("/capabilities", "focus"),
            ("/capabilities/zoom", "optical_ratio"),
            ("/capabilities/image", "picture_effects"),
            ("/capabilities/pan_tilt/geometry/coordinates", "wire_codec"),
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
        invalid["capabilities"]["pan_tilt"]["geometry"]["coordinates"]["pan_degrees_to_units"] =
            serde_json::json!(0);
        assert!(serde_json::from_value::<ProfileSpec>(invalid).is_err());
        let restored: ProfileSpec = serde_json::from_value(value).expect("current grouped shape");
        assert_eq!(restored, base);
    }

    #[test]
    fn picture_effect_inventories_allow_checked_extensions_and_reject_duplicates() {
        let base = ProfileId::PtzOpticsG2.spec().expect("G2 profile");
        let mut caps = base.capabilities().clone();
        caps.profile_id = None;
        let extension = crate::types::PictureEffectMode::Unknown(
            crate::types::UnknownPictureEffect::new(0x05).expect("extension"),
        );
        caps.image
            .as_mut()
            .expect("image")
            .picture_effects
            .push(extension);
        let custom = copy_builder(&base, caps.clone())
            .build()
            .expect("custom inventory");
        assert!(custom
            .capabilities()
            .image
            .as_ref()
            .expect("image")
            .picture_effects
            .contains(&extension));
        caps.image
            .as_mut()
            .expect("image")
            .picture_effects
            .push(extension);
        let error = copy_builder(&base, caps)
            .build()
            .expect_err("duplicate effect");
        assert!(error
            .to_string()
            .contains("capabilities.image.picture_effects"));
    }

    #[cfg(feature = "serde")]
    #[test]
    fn picture_effect_inventory_has_a_required_round_tripping_persisted_key() {
        let base = ProfileId::PtzOpticsG2.spec().expect("G2 profile");
        let value = serde_json::to_value(&base).expect("serialize inventory");
        let image = &value["capabilities"]["image"];
        assert_eq!(
            image["picture_effects"],
            serde_json::json!(["off", "black_and_white"])
        );
        assert!(image.get("picture_effect").is_none());
        assert_eq!(
            serde_json::from_value::<ProfileSpec>(value.clone()).expect("round trip"),
            base
        );
        let mut obsolete = value.clone();
        let image = obsolete["capabilities"]["image"]
            .as_object_mut()
            .expect("image object");
        image.remove("picture_effects");
        image.insert("picture_effect".into(), serde_json::Value::Bool(true));
        let error =
            serde_json::from_value::<ProfileSpec>(obsolete).expect_err("obsolete inventory");
        assert!(error.to_string().contains(REGENERATE_SPEC));
        let mut empty = value;
        empty["capabilities"]["profile_id"] = serde_json::Value::Null;
        empty["capabilities"]["image"]["picture_effects"] = serde_json::json!([]);
        assert!(
            serde_json::from_value::<ProfileSpec>(empty).is_err(),
            "a typed grant requires a nonempty inventory"
        );
    }

    #[cfg(feature = "serde")]
    #[test]
    fn explicit_absence_of_inquiry_axes_is_required_and_round_trips() {
        let profile = runtime_builder(valid_runtime_capabilities())
            .supports_operation_complete(true)
            .position_inquiries(None)
            .build()
            .expect("exact completion can represent no position inquiries");
        assert_eq!(profile.position_inquiries(), None);
        let mut value = serde_json::to_value(&profile).expect("serialize no-query profile");
        assert_eq!(value["position_inquiries"], serde_json::Value::Null);
        assert_eq!(
            serde_json::from_value::<ProfileSpec>(value.clone()).expect("explicit absence"),
            profile
        );
        value
            .as_object_mut()
            .expect("profile object")
            .remove("position_inquiries");
        let error = serde_json::from_value::<ProfileSpec>(value)
            .expect_err("fact cannot be omitted")
            .to_string();
        assert!(
            error.contains("missing field `position_inquiries`"),
            "{error}"
        );
    }

    #[cfg(feature = "ts-rs")]
    #[test]
    fn profile_typescript_has_the_persisted_object_shape_without_a_facts_alias() {
        let config = ts_rs::Config::default();
        let declaration = <ProfileSpec as ts_rs::TS>::decl(&config);
        assert!(
            declaration.starts_with("type ProfileSpec = {"),
            "{declaration}"
        );
        assert!(!declaration.contains("ProfileFacts"), "{declaration}");
        for field in [
            "capabilities",
            "encodings",
            "transports",
            "envelope",
            "timing",
            "position_inquiries",
        ] {
            assert!(declaration.contains(&format!("{field}:")), "{declaration}");
        }
        let timing = <ProfileTiming as ts_rs::TS>::decl(&config);
        assert!(timing.contains("command_spacing:"), "{timing}");
        assert!(timing.contains("inquiry_spacing:"), "{timing}");
        assert!(!timing.contains("minimum_"), "{timing}");
        assert!(timing.contains("secs: number"), "{timing}");
        assert!(timing.contains("nanos: number"), "{timing}");
    }
}
