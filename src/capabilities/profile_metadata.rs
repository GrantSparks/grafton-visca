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

/// Targeted families a compile-time profile can settle.
///
/// A positive declaration requires an exact completion or sufficient position
/// inquiries and the corresponding domain record.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub struct SettlementSupport(u8);

impl SettlementSupport {
    /// Builds the settlement declaration from the profile's domains, its
    /// provable settlement axes, and its preset recall axes when present.
    #[must_use]
    pub const fn from_axes(
        domains: DomainSet,
        available: Option<crate::types::AffectedAxes>,
        recall: Option<crate::types::AffectedAxes>,
    ) -> Self {
        use crate::types::AffectedAxes;

        let Some(axes) = available else {
            return Self(0);
        };
        let mut bits = 0;
        if domains.contains(Domain::PanTilt) && axes.contains(AffectedAxes::PAN_TILT) {
            bits |= 1;
        }
        if domains.contains(Domain::Focus) && axes.contains(AffectedAxes::FOCUS) {
            bits |= 2;
        }
        if let Some(recall) = recall {
            if domains.contains(Domain::Presets) && axes.contains(recall) {
                bits |= 4;
            }
        }
        Self(bits)
    }

    /// Whether home and reset can settle.
    #[must_use]
    pub const fn pan_tilt(self) -> bool {
        self.0 & 1 != 0
    }

    /// Whether direct focus position can settle.
    #[must_use]
    pub const fn focus(self) -> bool {
        self.0 & 2 != 0
    }

    /// Whether recall can settle all axes that its preset affects.
    #[must_use]
    pub const fn preset_recall(self) -> bool {
        self.0 & 4 != 0
    }
}

/// Supports settlement of pan/tilt home and reset.
///
/// Downstream profiles declare the settlement proof in
/// [`CompileTimeProfile::SETTLEMENT_SUPPORT`](crate::profiles::CompileTimeProfile::SETTLEMENT_SUPPORT).
#[diagnostic::on_unimplemented(message = "profile `{Self}` cannot settle pan/tilt home or reset")]
pub trait HasPanTiltSettlement {}

/// Supports settlement of direct focus position.
///
/// Downstream profiles declare the settlement proof in
/// [`CompileTimeProfile::SETTLEMENT_SUPPORT`](crate::profiles::CompileTimeProfile::SETTLEMENT_SUPPORT).
#[diagnostic::on_unimplemented(message = "profile `{Self}` cannot settle direct focus position")]
pub trait HasFocusSettlement {}

/// Supports settlement of every axis affected by preset recall.
///
/// Downstream profiles declare the settlement proof in
/// [`CompileTimeProfile::SETTLEMENT_SUPPORT`](crate::profiles::CompileTimeProfile::SETTLEMENT_SUPPORT).
#[diagnostic::on_unimplemented(message = "profile `{Self}` cannot settle preset recall")]
pub trait HasPresetRecallSettlement {}

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
