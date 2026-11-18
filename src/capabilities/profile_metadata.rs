//! Evidence-only domain and transport markers.

/// A capability domain declared in a compile-time profile.
///
/// There is one variant per domain inherited by a noun-table row, including
/// domains whose every row also refines its gate with typed support.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum Domain {
    /// `HasPower`.
    Power,
    /// `HasZoom`.
    Zoom,
    /// `HasPanTilt`.
    PanTilt,
    /// `HasFocus`.
    Focus,
    /// `HasWhiteBalance`.
    WhiteBalance,
    /// `HasPresets`.
    Presets,
    /// `HasMenu`.
    Menu,
    /// Exposure domain.
    Exposure,
    /// Image processing domain.
    Image,
}

impl Domain {
    const ALL: [Self; 9] = [
        Self::Power,
        Self::Zoom,
        Self::PanTilt,
        Self::Focus,
        Self::WhiteBalance,
        Self::Presets,
        Self::Menu,
        Self::Exposure,
        Self::Image,
    ];

    /// Whether a runtime profile has the domain behind this marker.
    ///
    /// Domain presence is represented once in the validated capability records.
    /// Optional method grants are checked independently by `RowGate`.
    pub(crate) fn permits(self, profile: &crate::ProfileSpec) -> bool {
        self.permits_capabilities(profile.capabilities())
    }

    /// Tests the domain while profile facts are still being validated.
    pub(crate) const fn permits_capabilities(
        self,
        capabilities: &crate::capabilities::Capabilities,
    ) -> bool {
        match self {
            Self::Power => capabilities.power.is_some(),
            Self::Zoom => capabilities.zoom.is_some(),
            Self::PanTilt => capabilities.pan_tilt.is_some(),
            Self::Focus => capabilities.focus.is_some(),
            Self::WhiteBalance => capabilities.white_balance.is_some(),
            Self::Presets => capabilities.presets.is_some(),
            Self::Menu => capabilities.menu.is_some(),
            Self::Exposure => capabilities.exposure.is_some(),
            Self::Image => capabilities.image.is_some(),
        }
    }
}

/// The exact set of capability records declared by a compile-time profile.
///
/// ```
/// use grafton_visca::capabilities::{Domain, DomainSet};
/// const DOMAINS: DomainSet = DomainSet::from_domains(&[Domain::Zoom]);
/// assert!(DOMAINS.contains(Domain::Zoom));
/// ```
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct DomainSet(u16);

impl DomainSet {
    /// Creates a set with no declared domains.
    #[must_use]
    pub const fn empty() -> Self {
        Self(0)
    }

    /// Collects the declared domains into a set.
    #[must_use]
    pub const fn from_domains(domains: &[Domain]) -> Self {
        let mut set = Self::empty();
        let mut index = 0;
        while index < domains.len() {
            set = set.with(domains[index]);
            index += 1;
        }
        set
    }

    /// Adds a domain to the set.
    #[must_use]
    pub const fn with(self, domain: Domain) -> Self {
        Self(self.0 | 1 << domain as u8)
    }

    /// Checks whether the set contains this domain.
    #[must_use]
    pub const fn contains(self, domain: Domain) -> bool {
        self.0 & (1 << domain as u8) != 0
    }

    pub(crate) const fn from_capabilities(capabilities: &super::Capabilities) -> Self {
        let mut set = Self::empty();
        let mut index = 0;
        while index < Domain::ALL.len() {
            let domain = Domain::ALL[index];
            if domain.permits_capabilities(capabilities) {
                set = set.with(domain);
            }
            index += 1;
        }
        set
    }
}

/// The exact set of standard transports declared by a compile-time profile.
///
/// ```
/// use grafton_visca::{capabilities::TransportSet, transport::StandardTransport};
/// const TRANSPORTS: TransportSet = TransportSet::from_transports(&[StandardTransport::Tcp]);
/// assert!(TRANSPORTS.contains(StandardTransport::Tcp));
/// ```
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct TransportSet(u8);

impl TransportSet {
    /// Creates a set with no declared transports.
    #[must_use]
    pub const fn empty() -> Self {
        Self(0)
    }

    /// Collects the permitted standard transports into a set.
    #[must_use]
    pub const fn from_transports(transports: &[crate::transport::StandardTransport]) -> Self {
        let mut set = Self::empty();
        let mut index = 0;
        while index < transports.len() {
            set = set.with(transports[index]);
            index += 1;
        }
        set
    }

    /// Adds a standard transport to the set.
    #[must_use]
    pub const fn with(self, transport: crate::transport::StandardTransport) -> Self {
        Self(self.0 | 1 << transport as u8)
    }

    /// Checks whether the set contains this transport.
    #[must_use]
    pub const fn contains(self, transport: crate::transport::StandardTransport) -> bool {
        self.0 & (1 << transport as u8) != 0
    }
}

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
///
/// The marker requires the matching entry in
/// [`CompileTimeProfile::TRANSPORTS`](crate::profiles::CompileTimeProfile::TRANSPORTS)
/// and a compatible transport in the profile specification.
#[diagnostic::on_unimplemented(
    message = "profile `{Self}` does not support TCP transport construction"
)]
pub trait SupportsTcp {}

/// Evidence-backed support for standard UDP transport construction.
///
/// The marker requires the matching entry in
/// [`CompileTimeProfile::TRANSPORTS`](crate::profiles::CompileTimeProfile::TRANSPORTS)
/// and a compatible transport in the profile specification.
#[diagnostic::on_unimplemented(
    message = "profile `{Self}` does not support UDP transport construction"
)]
pub trait SupportsUdp {}

/// Evidence-backed support for standard serial transport construction.
///
/// The marker requires the matching entry in
/// [`CompileTimeProfile::TRANSPORTS`](crate::profiles::CompileTimeProfile::TRANSPORTS)
/// and a compatible transport in the profile specification.
#[diagnostic::on_unimplemented(
    message = "profile `{Self}` does not support SERIAL transport construction"
)]
pub trait SupportsSerial {}

/// Evidence-backed support for the Power capability domain.
///
/// Built-in profiles implement this marker exactly when their domain record
/// is present. A downstream implementation requires source evidence and a
/// matching entry in [`CompileTimeProfile::DOMAINS`](crate::profiles::CompileTimeProfile::DOMAINS).
#[diagnostic::on_unimplemented(message = "profile `{Self}` does not declare Power support")]
pub trait HasPower {}

/// Evidence-backed support for the Zoom capability domain.
///
/// Built-in profiles implement this marker exactly when their domain record
/// is present. A downstream implementation requires source evidence and a
/// matching entry in [`CompileTimeProfile::DOMAINS`](crate::profiles::CompileTimeProfile::DOMAINS).
#[diagnostic::on_unimplemented(message = "profile `{Self}` does not declare Zoom support")]
pub trait HasZoom {}

/// Evidence-backed support for the Focus capability domain.
///
/// Built-in profiles implement this marker exactly when their domain record
/// is present. A downstream implementation requires source evidence and a
/// matching entry in [`CompileTimeProfile::DOMAINS`](crate::profiles::CompileTimeProfile::DOMAINS).
#[diagnostic::on_unimplemented(message = "profile `{Self}` does not declare Focus support")]
pub trait HasFocus {}

/// Evidence-backed support for the PanTilt capability domain.
///
/// Built-in profiles implement this marker exactly when their domain record
/// is present. A downstream implementation requires source evidence and a
/// matching entry in [`CompileTimeProfile::DOMAINS`](crate::profiles::CompileTimeProfile::DOMAINS).
#[diagnostic::on_unimplemented(message = "profile `{Self}` does not declare PanTilt support")]
pub trait HasPanTilt {}

/// Evidence-backed support for the Presets capability domain.
///
/// Built-in profiles implement this marker exactly when their domain record
/// is present. A downstream implementation requires source evidence and a
/// matching entry in [`CompileTimeProfile::DOMAINS`](crate::profiles::CompileTimeProfile::DOMAINS).
#[diagnostic::on_unimplemented(message = "profile `{Self}` does not declare Presets support")]
pub trait HasPresets {}

/// Evidence-backed support for the Exposure capability domain.
///
/// Built-in profiles implement this marker exactly when their domain record
/// is present. A downstream implementation requires source evidence and a
/// matching entry in [`CompileTimeProfile::DOMAINS`](crate::profiles::CompileTimeProfile::DOMAINS).
#[diagnostic::on_unimplemented(message = "profile `{Self}` does not declare Exposure support")]
pub trait HasExposure {}

/// Evidence-backed support for the WhiteBalance capability domain.
///
/// Built-in profiles implement this marker exactly when their domain record
/// is present. A downstream implementation requires source evidence and a
/// matching entry in [`CompileTimeProfile::DOMAINS`](crate::profiles::CompileTimeProfile::DOMAINS).
#[diagnostic::on_unimplemented(message = "profile `{Self}` does not declare WhiteBalance support")]
pub trait HasWhiteBalance {}

/// Evidence-backed support for the Image capability domain.
///
/// Built-in profiles implement this marker exactly when their domain record
/// is present. A downstream implementation requires source evidence and a
/// matching entry in [`CompileTimeProfile::DOMAINS`](crate::profiles::CompileTimeProfile::DOMAINS).
#[diagnostic::on_unimplemented(message = "profile `{Self}` does not declare Image support")]
pub trait HasImage {}

/// Evidence-backed support for the Menu capability domain.
///
/// Built-in profiles implement this marker exactly when their domain record
/// is present. A downstream implementation requires source evidence and a
/// matching entry in [`CompileTimeProfile::DOMAINS`](crate::profiles::CompileTimeProfile::DOMAINS).
#[diagnostic::on_unimplemented(message = "profile `{Self}` does not declare Menu support")]
pub trait HasMenu {}
