//! Evidence-only domain and transport markers.

/// Level of VISCA inquiry command support for a camera profile.
///
/// VISCA cameras vary in their support for inquiry (status read-back) commands.
/// This enum lets consumers determine inquiry support per-profile without
/// matching on [`ProfileGroup`](crate::profiles::ProfileGroup).
///
/// # Hardware Evidence
///
/// PTZOptics G2/G3/30X cameras have baseline VISCA inquiry support confirmed by
/// hardware testing. Model-specific optional inquiries still require their own
/// source-backed typed-support gate; a profile-wide `Full` value is never a
/// fallback permission for an optional accessor. See
/// [issue #498](https://github.com/GrantSparks/grafton-visca/issues/498) for
/// the tested G2 baseline.
///
/// # Usage
///
/// ```
/// use grafton_visca::capabilities::InquirySupport;
/// use grafton_visca::profiles::ProfileId;
///
/// # fn main() -> grafton_visca::Result<()> {
/// let profile = ProfileId::PtzOpticsG2;
/// match profile.spec()?.capabilities().inquiry_support {
///     InquirySupport::Full => {
///         // Baseline inquiries are available; optional accessors still
///         // require their profile-specific typed gates.
///     }
///     InquirySupport::Partial => {
///         // Basic inquiries work; advanced ones may not
///     }
///     InquirySupport::None => {
///         // Camera does not support inquiry commands
///     }
///     _ => {
///         // Future inquiry support categories should be handled conservatively
///     }
/// }
/// # Ok(())
/// # }
/// ```
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "serde", derive(serde::Serialize, serde::Deserialize))]
#[cfg_attr(feature = "schemars", derive(schemars::JsonSchema))]
#[cfg_attr(feature = "ts-rs", derive(ts_rs::TS), ts(export))]
#[cfg_attr(feature = "serde", serde(rename_all = "snake_case"))]
#[non_exhaustive]
pub enum InquirySupport {
    /// Full baseline inquiry support for the documented profile family.
    ///
    /// Cameras in this category support the baseline VISCA inquiry set used by
    /// their profile. Optional model-specific inquiries remain independently
    /// gated by typed support markers and runtime capability facts.
    ///
    /// Confirmed profiles: PTZOptics G2, PTZOptics G3, PTZOptics 30X,
    /// Sony FR7, Sony BRC-H900.
    Full,

    /// Partial inquiry support — basic inquiries work, but some may not respond.
    ///
    /// Position queries (zoom, pan/tilt, focus) and basic status queries work,
    /// but some feature-specific inquiries (exposure mode, image processing, etc.)
    /// may time out or return errors. Consumers should handle inquiry failures
    /// gracefully for cameras at this level.
    Partial,

    /// No inquiry command support.
    ///
    /// The camera does not respond to any VISCA inquiry commands.
    /// All inquiry operations will fail.
    None,
}

/// Evidence-backed support for standard TCP transport construction.
#[diagnostic::on_unimplemented(
    message = "profile `{Self}` does not support TCP transport construction"
)]
pub trait SupportsTcp {}

/// Evidence-backed support for standard UDP transport construction.
#[diagnostic::on_unimplemented(
    message = "profile `{Self}` does not support UDP transport construction"
)]
pub trait SupportsUdp {}

/// Evidence-backed support for standard SERIAL transport construction.
#[diagnostic::on_unimplemented(
    message = "profile `{Self}` does not support SERIAL transport construction"
)]
pub trait SupportsSerial {}

/// Evidence-backed support for the Power capability domain.
///
/// Built-in profiles implement this marker exactly when their domain record
/// is present. Downstream profiles must supply evidence before implementing it.
#[diagnostic::on_unimplemented(message = "profile `{Self}` does not declare Power support")]
pub trait HasPower {}

/// Evidence-backed support for the Zoom capability domain.
///
/// Built-in profiles implement this marker exactly when their domain record
/// is present. Downstream profiles must supply evidence before implementing it.
#[diagnostic::on_unimplemented(message = "profile `{Self}` does not declare Zoom support")]
pub trait HasZoom {}

/// Evidence-backed support for the Focus capability domain.
///
/// Built-in profiles implement this marker exactly when their domain record
/// is present. Downstream profiles must supply evidence before implementing it.
#[diagnostic::on_unimplemented(message = "profile `{Self}` does not declare Focus support")]
pub trait HasFocus {}

/// Evidence-backed support for the PanTilt capability domain.
///
/// Built-in profiles implement this marker exactly when their domain record
/// is present. Downstream profiles must supply evidence before implementing it.
#[diagnostic::on_unimplemented(message = "profile `{Self}` does not declare PanTilt support")]
pub trait HasPanTilt {}

/// Evidence-backed support for the Presets capability domain.
///
/// Built-in profiles implement this marker exactly when their domain record
/// is present. Downstream profiles must supply evidence before implementing it.
#[diagnostic::on_unimplemented(message = "profile `{Self}` does not declare Presets support")]
pub trait HasPresets {}

/// Evidence-backed support for the Exposure capability domain.
///
/// Built-in profiles implement this marker exactly when their domain record
/// is present. Downstream profiles must supply evidence before implementing it.
#[diagnostic::on_unimplemented(message = "profile `{Self}` does not declare Exposure support")]
pub trait HasExposure {}

/// Evidence-backed support for the WhiteBalance capability domain.
///
/// Built-in profiles implement this marker exactly when their domain record
/// is present. Downstream profiles must supply evidence before implementing it.
#[diagnostic::on_unimplemented(message = "profile `{Self}` does not declare WhiteBalance support")]
pub trait HasWhiteBalance {}

/// Evidence-backed support for the Image capability domain.
///
/// Built-in profiles implement this marker exactly when their domain record
/// is present. Downstream profiles must supply evidence before implementing it.
#[diagnostic::on_unimplemented(message = "profile `{Self}` does not declare Image support")]
pub trait HasImage {}

/// Evidence-backed support for the Menu capability domain.
///
/// Built-in profiles implement this marker exactly when their domain record
/// is present. Downstream profiles must supply evidence before implementing it.
#[diagnostic::on_unimplemented(message = "profile `{Self}` does not declare Menu support")]
pub trait HasMenu {}
