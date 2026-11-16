//! Canonical noun metadata projected onto the closed protocol ledger.
//!
//! The noun table names public requests. Their exact `LEDGER_ROWS` describe
//! the protocol identities they can encode; query wrappers supply their raw
//! kind and source. Const evaluation proves exhaustive protocol coverage and
//! compatible shared rows, then builds an indexed lookup for preparation.

use crate::{capabilities::TypedSupportSet, noun_table::noun_table};

use super::semantics::{BuiltinCommand, BuiltinRequestClass, BuiltinRequestKind};

/// The capability gate of one noun-table row, including both its typed noun
/// header and any refining `where` marker.
///
/// This is the one gate vocabulary of the noun table.  The static facades
/// enforce it as trait bounds; the surface registry records it per command
/// and the built-in inquiries check it at runtime.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum RowGate {
    /// No capability is required (`gate: [always]`).
    Always,
    /// A base-domain marker is required (`gate: [domain <Marker>]`).
    Domain(DomainGate),
    /// Every typed-support marker in the set is required: a typed noun's
    /// header plus a row's refining `where` marker, when either is present.
    Typed(TypedSupportSet),
    /// A domain and every optional permission are required together.
    DomainAndTyped(DomainGate, TypedSupportSet),
}

impl RowGate {
    /// Tests a row before a validated profile exists, using the same capability
    /// contract that request and inquiry admission enforce.
    pub(crate) fn permits(self, capabilities: &crate::capabilities::Capabilities) -> bool {
        match self {
            Self::Always => true,
            Self::Domain(domain) => domain.permits_capabilities(capabilities),
            Self::Typed(surfaces) => surfaces
                .iter()
                .all(|surface| capabilities.supports(surface)),
            Self::DomainAndTyped(domain, surfaces) => {
                domain.permits_capabilities(capabilities)
                    && Self::Typed(surfaces).permits(capabilities)
            }
        }
    }

    /// Enforces the same noun-row contract for commands and inquiries.
    pub(crate) fn validate(
        self,
        profile: &crate::ProfileSpec,
        feature: &'static str,
    ) -> crate::Result<()> {
        let capabilities = profile.capabilities();
        if self.permits(capabilities) {
            return Ok(());
        }
        // Admission above uses the same predicate as profile validation. The
        // remaining lookup only chooses the established refusal diagnostic,
        // checking a missing domain before any refining typed permission.
        let typed = match self {
            Self::Typed(surfaces) => Some(surfaces),
            Self::DomainAndTyped(domain, surfaces) if domain.permits(profile) => Some(surfaces),
            _ => None,
        };
        let missing = typed.and_then(|surfaces| {
            surfaces
                .iter()
                .find(|surface| !capabilities.supports(*surface))
        });
        let denied = if missing == Some(crate::capabilities::TypedSupportSurface::ExposureMode) {
            crate::command::exposure_wire::SHARED_EXPOSURE_MODE_FEATURE
        } else {
            feature
        };
        Err(crate::Error::FeatureNotSupported { feature: denied })
    }

    /// Const comparison used by the generated request inventory.
    pub(crate) const fn same_as(self, other: Self) -> bool {
        match (self, other) {
            (Self::Always, Self::Always) => true,
            (Self::Domain(left), Self::Domain(right)) => left as u8 == right as u8,
            (Self::DomainAndTyped(ld, left), Self::DomainAndTyped(rd, right)) => {
                ld as u8 == rd as u8 && left.contains_all(right) && right.contains_all(left)
            }
            (Self::Typed(left), Self::Typed(right)) => {
                left.contains_all(right) && right.contains_all(left)
            }
            _ => false,
        }
    }
}

/// The base-domain marker of a noun header's `[domain <Marker>]` gate.
///
/// There is one variant per domain that some noun-table row inherits.  The
/// `Image` noun is gated on `HasImage`, but every one of its rows
/// narrows that with its own `where` marker, so it has no variant here. The exposure noun likewise refines every row.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum DomainGate {
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

impl DomainGate {
    /// Whether a runtime profile has the domain behind this marker.
    ///
    /// Domain presence is represented once in the validated capability records.
    /// Optional method grants are checked independently by `RowGate`.
    pub(crate) fn permits(self, profile: &crate::ProfileSpec) -> bool {
        self.permits_capabilities(profile.capabilities())
    }

    /// Tests the domain while profile facts are still being validated.
    pub(crate) fn permits_capabilities(
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

/// The runtime gate of a built-in inquiry, generated from its noun-table row.
///
/// The surface registry implements it for the request type of every
/// `inquiry` row, and the one untyped built-in inquiry gets
/// [`RowGate::Always`] from the inquiry table.  A typed built-in inquiry
/// without a noun row therefore has no implementation and fails to compile,
/// and an inquiry reached by two rows has two and fails the same way.
pub(crate) trait BuiltinInquiryGate {
    /// The inquiry row's gate.
    const GATE: RowGate;
}

/// Fixed domain requests require their original all-model evidence promise.
/// Value families instead prove each admitted profile/value against its exact
/// selected source row in the preparation-backed constructor inventory.
const fn assert_domain_source(gate: RowGate, source: crate::source_evidence::Source) {
    if matches!(gate, RowGate::Typed(_) | RowGate::DomainAndTyped(_, _)) {
        return;
    }
    let models = crate::profiles::ProfileId::all();
    let mut index = 0;
    while index < models.len() {
        assert!(
            source.covers_model(Some(models[index])),
            "a fixed domain or unconditional noun row requires evidence for every built-in profile"
        );
        index += 1;
    }
}

/// Maps a base-domain marker to its [`DomainGate`].
macro_rules! domain_gate {
    (HasPower) => {
        $crate::command::surface::DomainGate::Power
    };
    (HasZoom) => {
        $crate::command::surface::DomainGate::Zoom
    };
    (HasPanTilt) => {
        $crate::command::surface::DomainGate::PanTilt
    };
    (HasFocus) => {
        $crate::command::surface::DomainGate::Focus
    };
    (HasWhiteBalance) => {
        $crate::command::surface::DomainGate::WhiteBalance
    };
    (HasPresets) => {
        $crate::command::surface::DomainGate::Presets
    };
    (HasMenu) => {
        $crate::command::surface::DomainGate::Menu
    };
    (HasExposure) => {
        $crate::command::surface::DomainGate::Exposure
    };
    (HasImage) => {
        $crate::command::surface::DomainGate::Image
    };
}
pub(crate) use domain_gate;

/// Builds the full conjunction of noun-domain and method permissions.
macro_rules! noun_row_gate {
    ([], [always]) => { $crate::command::surface::RowGate::Always };
    ([], [domain $marker:ident]) => {
        $crate::command::surface::RowGate::Domain($crate::command::surface::domain_gate!($marker))
    };
    ([$($gate:ident),*], [typed $base:ident]) => {
        $crate::command::surface::RowGate::Typed($crate::capabilities::TypedSupportSet::from_surfaces(&[
            $crate::capabilities::typed_surface!($base),
            $($crate::capabilities::typed_surface!($gate),)*
        ]))
    };
    ([$($gate:ident),+], [domain $base:ident]) => {
        $crate::command::surface::RowGate::DomainAndTyped(
            $crate::command::surface::domain_gate!($base),
            $crate::capabilities::TypedSupportSet::from_surfaces(&[$($crate::capabilities::typed_surface!($gate)),+]),
        )
    };
    ([$($gate:ident),+], [always]) => {
        $crate::command::surface::RowGate::Typed(
            $crate::capabilities::TypedSupportSet::from_surfaces(&[$($crate::capabilities::typed_surface!($gate)),+]),
        )
    };
}

/// Skip domain headers that every row refines, without constructing an
/// unused domain gate merely to audit typed-header inheritance.
#[cfg(test)]
macro_rules! assert_typed_header {
    ($assertion:ident, [typed $base:ident], $row:tt, $noun:expr, $method:expr) => {
        $assertion(
            $noun,
            $method,
            noun_row_gate!([], [typed $base]),
            noun_row_gate!($row, [typed $base]),
        );
    };
    ($assertion:ident, $base:tt, $row:tt, $noun:expr, $method:expr) => {};
}

#[cfg(test)]
pub(crate) use noun_row_gate;

/// What kind of public surface disposition a built-in request has.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum StaticSurfaceDisposition {
    /// A normal target-facing noun method.
    Noun {
        /// Noun accessor containing the method.
        noun: StaticNoun,
        /// Canonical method spelling shared by all facades.
        method: &'static str,
        /// The row's capability gate.
        gate: RowGate,
    },
    /// A broadcast handshake, never a target camera method.
    BroadcastHandshake {
        /// Internal protocol spelling retained for the broadcast namespace.
        method: &'static str,
    },
    /// An internal cancellation primitive, never a noun method.
    InternalCancellation {
        /// Internal protocol spelling retained for owner cancellation.
        method: &'static str,
    },
}

/// One derived row in the closed static noun ledger.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) struct StaticSurfaceEntry {
    /// Exactly one source semantic row.
    pub(crate) command: BuiltinCommand,
    /// Noun mapping or explicit non-noun exception.
    pub(crate) disposition: StaticSurfaceDisposition,
    /// Semantic return class copied from the authoritative row.
    pub(crate) class: BuiltinRequestClass,
}

impl StaticSurfaceEntry {
    /// Canonical method spelling for a refused row's diagnostic.
    pub(crate) const fn feature(self) -> &'static str {
        match self.disposition {
            StaticSurfaceDisposition::Noun { method, .. }
            | StaticSurfaceDisposition::BroadcastHandshake { method }
            | StaticSurfaceDisposition::InternalCancellation { method } => method,
        }
    }

    /// Returns whether the row may become a target camera noun method.
    #[must_use]
    #[cfg(test)]
    pub(crate) const fn is_target_facing(self) -> bool {
        matches!(self.disposition, StaticSurfaceDisposition::Noun { .. })
    }
}

/// Resolve a noun-table row kind against the authoritative semantic
/// classification.
///
/// This is deliberately a const function with no fallback: a row whose kind
/// does not match its command ID fails while evaluating the arm's `CLASS`
/// constant in [`surface_entry`], before a facade can accidentally expose the
/// wrong operation handle.
#[allow(clippy::panic)]
const fn registry_class(command: BuiltinCommand, kind: BuiltinRequestKind) -> BuiltinRequestClass {
    let class = command.classification();
    if class.kind() as u8 == kind as u8 {
        class
    } else {
        panic!("noun-table request kind disagrees with BuiltinCommand::classification")
    }
}

/// Maps a command row's kind keyword to its [`BuiltinRequestKind`].
macro_rules! registry_class {
    (plain) => {
        BuiltinRequestKind::Plain
    };
    (applied) => {
        BuiltinRequestKind::AppliedOnly
    };
    (targeted) => {
        BuiltinRequestKind::Targeted
    };
}

/// Maps a protocol exception's keyword to its non-noun disposition.
macro_rules! exception_disposition {
    (broadcast, $method:expr) => {
        StaticSurfaceDisposition::BroadcastHandshake { method: $method }
    };
    (internal, $method:expr) => {
        StaticSurfaceDisposition::InternalCancellation { method: $method }
    };
}

/// Extracts a canonical query type while preserving one request-expression source.
/// Constructors and physical reply mapping do not change its request identity.
macro_rules! query_type {
    ($consumer:ident, [$($arguments:tt)*], [mapped [$($request:tt)*] |$reply:ident, $profile:ident| $mapping:expr]) => {
        query_type! {$consumer, [$($arguments)*], [$($request)*]}
    };
    ($consumer:ident, [$($arguments:tt)*], [checked $($request:tt)*]) => {
        query_type! {$consumer, [$($arguments)*], [$($request)*]}
    };
    ($consumer:ident, [$($arguments:tt)*], [with_target |$target:ident| $($request:tt)*]) => {
        query_type! {$consumer, [$($arguments)*], [$($request)*]}
    };
    ($consumer:ident, [$($arguments:tt)*], [with_profile |$profile:ident| $($request:tt)*]) => {
        query_type! {$consumer, [$($arguments)*], [$($request)*]}
    };
    ($consumer:ident, [$($arguments:tt)*], [$root:ident::$noun:ident::$request:ident::$constructor:ident($($value:tt)*)]) => {
        $consumer! {[$($arguments)*], $root::$noun::$request}
    };
    ($consumer:ident, [$($arguments:tt)*], [$root:ident::$noun:ident::$request:ident]) => {
        $consumer! {[$($arguments)*], $root::$noun::$request}
    };
}

macro_rules! implement_query_gate {
    ([$gate:tt $base:tt], $request:ty) => {
        impl BuiltinInquiryGate for $request {
            const GATE: RowGate = noun_row_gate!($gate, $base);
        }
        const _: () = assert_domain_source(
            noun_row_gate!($gate, $base),
            <$request as crate::command::nouns::NounInquiryContract>::SOURCE,
        );
    };
}

macro_rules! inquiry_gate {
    (inquiry $request:tt $gate:tt $base:tt) => {
        query_type!(implement_query_gate, [$gate $base], $request);
    };
    ($kind:ident $request:tt $gate:tt $base:tt) => {};
}

#[derive(Clone, Copy)]
struct CommandSurface {
    rows: &'static [BuiltinCommand],
    kind: BuiltinRequestKind,
    disposition: StaticSurfaceDisposition,
}

#[derive(Clone, Copy)]
struct QuerySurface {
    kind: crate::command::inquiry_structs::InquiryKind,
    gate: RowGate,
}

macro_rules! command_surface {
    (inquiry, $noun:ident, $method:ident, $request:ty, $gate:tt, $base:tt) => {
        None
    };
    ($kind:ident, $noun:ident, $method:ident, $request:ty, $gate:tt, $base:tt) => {
        Some(CommandSurface {
            rows: <$request as crate::command::semantics::BuiltinRequestContract>::LEDGER_ROWS,
            kind: registry_class!($kind),
            disposition: StaticSurfaceDisposition::Noun {
                noun: StaticNoun::$noun,
                method: stringify!($method),
                gate: noun_row_gate!($gate, $base),
            },
        })
    };
}

macro_rules! query_surface_entry {
    ([$gate:tt $base:tt], $request:ty) => {
        Some(QuerySurface {
            kind: <$request as crate::command::nouns::NounInquiryContract>::KIND,
            gate: noun_row_gate!($gate, $base),
        })
    };
}

macro_rules! query_surface {
    // This is an explicit typed view over the same position packet. Its own
    // wrapper gate is enforced separately; it cannot redefine the raw query.
    (inquiry [NormalizedInquiry] $request:tt $gate:tt $base:tt) => { None };
    (inquiry [$key:ident] $request:tt $gate:tt $base:tt) => {
        query_type!(query_surface_entry, [$gate $base], $request)
    };
    ($kind:ident $key:tt $request:tt $gate:tt $base:tt) => { None };
}

/// Projects canonical request identities, their exact protocol-row inventories,
/// and query provenance from the one noun table.
macro_rules! surface_registry {
    (
        $(
            @noun $noun:ident {
                accessor: $accessor:ident,
                getter: $getter:ident,
                gate: $base:tt,
                doc: $doc:literal $(,)?
            };
            $(
                $(#[$row_doc:meta])*
                $kind:ident [$key:ident] $method:ident($($(#[$coercion:ident])? $arg:ident: $ty:ty),*) -> $ret:ty
                    $(where $gate:ident $(+ $extra:ident)*)? = [$($request:tt)*];
            )*
        )*
        @exceptions; $( $exkind:ident [$excommand:ident] $exmethod:ident -> $exty:ty; )*
    ) => {
        /// The noun accessor owning a canonical method.
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
        pub(crate) enum StaticNoun {
            $( #[doc = $doc] $noun, )*
        }

        const COMMAND_SURFACES: &[Option<CommandSurface>] = {
            use crate::command;
            &[
                $( $( command_surface!($kind, $noun, $method, $ret, [$($gate $(, $extra)*)?], $base), )* )*
                $( Some(CommandSurface {
                    rows: &[BuiltinCommand::$excommand],
                    kind: BuiltinRequestKind::Plain,
                    disposition: exception_disposition!($exkind, stringify!($exmethod)),
                }), )*
            ]
        };

        const QUERY_SURFACES: &[Option<QuerySurface>] = {
            use crate::command;
            &[$( $( query_surface!($kind [$key] [$($request)*] [$($gate $(, $extra)*)?] $base), )* )*]
        };

        const _: () = {
            use crate::command;
            $( $( inquiry_gate!($kind [$($request)*] [$($gate $(, $extra)*)?] $base); )* )*
        };

        #[cfg(test)]
        #[test]
        fn typed_noun_headers_are_required_by_every_row() {
            fn assert_header(noun: &str, method: &str, header: RowGate, row: RowGate) {
                if let RowGate::Typed(required) = header {
                    let retained = match row {
                        RowGate::Typed(actual) | RowGate::DomainAndTyped(_, actual) => actual.contains_all(required),
                        RowGate::Always | RowGate::Domain(_) => false,
                    };
                    assert!(retained, "{noun}::{method} loses its noun header gate");
                }
            }
            $( $( assert_typed_header!(assert_header, $base, [$($gate $(, $extra)*)?], stringify!($noun), stringify!($method)); )* )*
        }
    };
}

noun_table!(surface_registry);

/// Compiles the protocol projection once; request preparation performs one
/// indexed lookup rather than searching canonical methods at runtime.
#[allow(clippy::panic)]
const fn build_surface_index() -> [StaticSurfaceEntry; BuiltinCommand::ALL.len()] {
    let first = BuiltinCommand::ALL[0];
    let mut index = [StaticSurfaceEntry {
        command: first,
        disposition: StaticSurfaceDisposition::InternalCancellation { method: "" },
        class: first.classification(),
    }; BuiltinCommand::ALL.len()];
    let mut covered = [false; BuiltinCommand::ALL.len()];
    let mut family = 0;
    while family < COMMAND_SURFACES.len() {
        if let Some(surface) = COMMAND_SURFACES[family] {
            assert!(
                !surface.rows.is_empty(),
                "a canonical command declares no protocol rows"
            );
            let mut row_index = 0;
            while row_index < surface.rows.len() {
                let row = surface.rows[row_index];
                let slot = row as usize;
                let class = registry_class(row, surface.kind);
                if let StaticSurfaceDisposition::Noun { gate, .. } = surface.disposition {
                    if surface.rows.len() == 1 {
                        assert_domain_source(gate, row.source());
                    }
                }
                if covered[slot] {
                    // A convenience method may reuse a canonical packet only
                    // when its complete admission and lifecycle contracts agree.
                    assert!(
                        class.same_contract(index[slot].class),
                        "shared protocol rows disagree on lifecycle"
                    );
                    match (index[slot].disposition, surface.disposition) {
                        (
                            StaticSurfaceDisposition::Noun { gate: left, .. },
                            StaticSurfaceDisposition::Noun { gate: right, .. },
                        ) => {
                            assert!(
                                left.same_as(right),
                                "shared protocol rows disagree on capability gates"
                            );
                        }
                        _ => panic!("a protocol exception is repeated or exposed as a noun method"),
                    }
                } else {
                    covered[slot] = true;
                    index[slot] = StaticSurfaceEntry {
                        command: row,
                        disposition: surface.disposition,
                        class,
                    };
                }
                row_index += 1;
            }
        }
        family += 1;
    }
    let mut row = 0;
    while row < covered.len() {
        assert!(
            BuiltinCommand::ALL[row] as usize == row,
            "protocol row ordinals must be dense"
        );
        assert!(
            covered[row],
            "a protocol row has no canonical noun or exception request"
        );
        row += 1;
    }
    index
}

const SURFACE_INDEX: [StaticSurfaceEntry; BuiltinCommand::ALL.len()] = build_surface_index();

/// The canonical surface of a protocol row, with aliases checked at compilation.
pub(crate) const fn surface_entry(command: BuiltinCommand) -> StaticSurfaceEntry {
    SURFACE_INDEX[command as usize]
}

/// The raw inquiry gate generated from canonical wrapper provenance.
/// Every old inquiry-table row forces this lookup during const evaluation.
#[allow(clippy::panic)]
pub(crate) const fn inquiry_kind_gate_if_present(
    kind: crate::command::inquiry_structs::InquiryKind,
) -> Option<RowGate> {
    let mut result = None;
    let mut index = 0;
    while index < QUERY_SURFACES.len() {
        if let Some(query) = QUERY_SURFACES[index] {
            if query.kind as usize == kind as usize {
                assert!(
                    result.is_none(),
                    "a raw inquiry kind has multiple canonical owners"
                );
                result = Some(query.gate);
            }
        }
        index += 1;
    }
    result
}

#[allow(clippy::panic)]
pub(crate) const fn inquiry_kind_gate(
    kind: crate::command::inquiry_structs::InquiryKind,
) -> RowGate {
    match inquiry_kind_gate_if_present(kind) {
        Some(gate) => gate,
        None => panic!("a raw inquiry kind has no canonical noun wrapper"),
    }
}

/// Returns the full capability gate of one static command row.
///
/// Every built-in request checks this projection before validating its values.
/// Dynamic admission therefore follows the same noun and row bounds as static
/// method resolution, including base-domain gates.
#[must_use]
pub(crate) const fn row_gate(command: BuiltinCommand) -> RowGate {
    match surface_entry(command).disposition {
        StaticSurfaceDisposition::Noun { gate, .. } => gate,
        StaticSurfaceDisposition::BroadcastHandshake { .. }
        | StaticSurfaceDisposition::InternalCancellation { .. } => RowGate::Always,
    }
}

/// Whether a typed grant exclusively enables inquiries. Derive command grants
/// from the ledger, so new surfaces cannot bypass inquiry-support validation.
pub(crate) const fn is_inquiry_only_surface(
    surface: crate::capabilities::TypedSupportSurface,
) -> bool {
    const COMMAND_SURFACES: TypedSupportSet = {
        let mut surfaces = TypedSupportSet::EMPTY;
        let mut index = 0;
        while index < BuiltinCommand::ALL.len() {
            match row_gate(BuiltinCommand::ALL[index]) {
                RowGate::Typed(required) | RowGate::DomainAndTyped(_, required) => {
                    surfaces = surfaces.union(required);
                }
                RowGate::Always | RowGate::Domain(_) => {}
            }
            index += 1;
        }
        surfaces
    };
    !COMMAND_SURFACES.contains(surface)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn refining_gates_retain_the_domain_and_every_marker() {
        let actual = noun_row_gate!([HasDirectZoom, HasDigitalZoomRange], [domain HasZoom]);
        assert_eq!(
            actual,
            RowGate::DomainAndTyped(
                DomainGate::Zoom,
                TypedSupportSet::from_surfaces(&[
                    crate::capabilities::TypedSupportSurface::DirectZoom,
                    crate::capabilities::TypedSupportSurface::DigitalZoomRange,
                ])
            )
        );
        let typed =
            noun_row_gate!([HasDirectZoom, HasDigitalZoomRange], [typed HasHighResolutionZoom]);
        assert_eq!(
            typed,
            RowGate::Typed(TypedSupportSet::from_surfaces(&[
                crate::capabilities::TypedSupportSurface::HighResolutionZoom,
                crate::capabilities::TypedSupportSurface::DirectZoom,
                crate::capabilities::TypedSupportSurface::DigitalZoomRange,
            ]))
        );
    }

    #[test]
    fn normalized_query_keeps_its_refining_gate_without_redefining_raw_position() {
        use crate::command::{inquiry_structs::InquiryKind, zoom::NormalizedInquiry};
        assert_eq!(
            inquiry_kind_gate(InquiryKind::ZoomPosition),
            RowGate::Domain(DomainGate::Zoom)
        );
        assert_eq!(
            <NormalizedInquiry as BuiltinInquiryGate>::GATE,
            RowGate::DomainAndTyped(
                DomainGate::Zoom,
                TypedSupportSet::from_surface(crate::capabilities::TypedSupportSurface::DirectZoom)
            )
        );
    }

    #[test]
    fn declared_query_views_keep_their_own_gate_before_private_alias_fallback() {
        use crate::command::inquiry_structs::{
            ExposureCompensationInquiry, Fr7AeLevelInquiry, GeneratedInquiry,
        };
        assert_eq!(
            <Fr7AeLevelInquiry as GeneratedInquiry>::GATE,
            <crate::command::exposure::AeLevelInquiry as BuiltinInquiryGate>::GATE
        );
        assert_ne!(
            <Fr7AeLevelInquiry as GeneratedInquiry>::GATE,
            <ExposureCompensationInquiry as GeneratedInquiry>::GATE
        );
    }

    #[test]
    fn query_type_projection_unwraps_fallible_and_mapped_construction() {
        use crate::command;
        macro_rules! name {
            ([], $request:ty) => {
                std::any::type_name::<$request>()
            };
        }
        assert_eq!(
            query_type!(
                name,
                [],
                [
                    mapped[with_target
                        | target
                        | command::zoom::NormalizedInquiry::for_target(target, domain)]
                        | reply,
                    profile | Ok(reply)
                ]
            ),
            std::any::type_name::<command::zoom::NormalizedInquiry>()
        );
        assert_eq!(
            query_type!(name, [], [checked command::presets::SeparateModeInquiry::new(preset)]),
            std::any::type_name::<command::presets::SeparateModeInquiry>()
        );
        assert_eq!(
            query_type!(
                name,
                [],
                [with_profile | profile | command::zoom::PositionInquiry]
            ),
            std::any::type_name::<command::zoom::PositionInquiry>()
        );
    }

    #[test]
    fn surface_ledger_is_exhaustive_unique_and_class_balanced() {
        let mut seen = std::collections::HashSet::new();
        let mut plain = 0;
        let mut applied_only = 0;
        let mut stops = 0;
        let mut targeted = 0;
        for command in BuiltinCommand::ALL {
            let entry = surface_entry(*command);
            assert!(seen.insert(*command), "duplicate surface ledger row");
            assert_eq!(entry.command, *command);
            assert_eq!(entry.class, command.classification());
            match entry.class {
                BuiltinRequestClass::Plain { .. } => plain += 1,
                BuiltinRequestClass::AppliedOnly { stop, .. } => {
                    applied_only += 1;
                    stops += usize::from(stop);
                }
                BuiltinRequestClass::Targeted { .. } => targeted += 1,
            }
            match entry.disposition {
                StaticSurfaceDisposition::Noun { method, .. }
                | StaticSurfaceDisposition::BroadcastHandshake { method }
                | StaticSurfaceDisposition::InternalCancellation { method } => {
                    assert!(!method.is_empty());
                }
            }
        }

        // The class distribution is pinned, not re-derived.  Summing the three
        // counters back to `ALL.len()` proves nothing — the match above is
        // exhaustive, so the sum is an identity — while the split between the
        // classes is exactly what a silent reclassification changes.
        //
        // Maintenance: adding a command row moves one of these three numbers by
        // one, and reclassifying a row moves two.  Update the triple in the
        // same commit as the ledger change and say in the commit message which
        // class the row landed in; a change here that nobody intended is the
        // regression this assertion exists to surface.
        assert_eq!(
            (plain, applied_only, stops, targeted),
            (164, 22, 4, 15),
            "the semantic class distribution of the closed ledger changed",
        );
        assert_eq!(seen.len(), BuiltinCommand::ALL.len());
    }

    /// Issue #651: paired on/off rows agree on their capability gate.
    ///
    /// Both halves of a toggle must be reachable from the same set of
    /// profiles, or a profile can reach one direction and get stuck there.
    ///
    /// `set_flip_both` and `set_flip_mode` are here for the same reason.  They
    /// send the *same* combined opcode, so a split gate would let a profile
    /// send that opcode's `Both` without being able to send its `Off`.
    #[test]
    fn paired_rows_share_one_capability_gate() {
        /// `None` marks a non-noun row, which the assertions below reject
        /// rather than skip: a pair that stopped being a noun pair would
        /// otherwise satisfy this test vacuously.
        fn facts(command: BuiltinCommand) -> (&'static str, Option<RowGate>) {
            match surface_entry(command).disposition {
                StaticSurfaceDisposition::Noun { method, gate, .. } => (method, Some(gate)),
                StaticSurfaceDisposition::BroadcastHandshake { method }
                | StaticSurfaceDisposition::InternalCancellation { method } => (method, None),
            }
        }

        for (on, off) in [
            (
                BuiltinCommand::ImageFlipHorizontal,
                BuiltinCommand::ImageFlipHorizontalOff,
            ),
            (
                BuiltinCommand::ImageFlipOff,
                BuiltinCommand::ImageFlipVertical,
            ),
            (
                BuiltinCommand::ImageFlipBoth,
                BuiltinCommand::ImageFlipCombined,
            ),
        ] {
            let (on_method, on_marker) = facts(on);
            let (off_method, off_marker) = facts(off);
            assert!(
                on_marker.is_some(),
                "`{on_method}` stopped being a noun row, so this pair no \
                 longer proves anything",
            );
            assert_eq!(
                on_marker, off_marker,
                "`{on_method}` and `{off_method}` are two directions of one \
                 toggle but are gated differently, so a profile can reach one \
                 direction and not the other",
            );
        }
    }

    #[test]
    fn broadcast_and_internal_rows_never_have_noun_dispositions() {
        assert!(matches!(
            surface_entry(BuiltinCommand::AddressSet).disposition,
            StaticSurfaceDisposition::BroadcastHandshake { .. }
        ));
        assert!(matches!(
            surface_entry(BuiltinCommand::InterfaceClear).disposition,
            StaticSurfaceDisposition::BroadcastHandshake { .. }
        ));
        assert!(matches!(
            surface_entry(BuiltinCommand::CommandCancel).disposition,
            StaticSurfaceDisposition::InternalCancellation { .. }
        ));

        // The target-facing total is derived: the ledger is closed, so the
        // noun rows are exactly the rows that are not one of the three named
        // non-noun exceptions above.
        let exceptions: Vec<BuiltinCommand> = BuiltinCommand::ALL
            .iter()
            .copied()
            .filter(|command| !surface_entry(*command).is_target_facing())
            .collect();
        assert_eq!(
            exceptions,
            vec![
                BuiltinCommand::AddressSet,
                BuiltinCommand::InterfaceClear,
                BuiltinCommand::CommandCancel,
            ]
        );

        // Counting the noun rows and adding the exceptions back is an identity,
        // because `is_target_facing` is what splits the two groups.  The
        // property worth asserting is that the two groups do not overlap in
        // method spelling: the broadcast and cancellation rows own protocol
        // names that must never resurface as a camera noun method on any of the
        // three facades.
        let reserved: Vec<&'static str> = BuiltinCommand::ALL
            .iter()
            .filter_map(|command| match surface_entry(*command).disposition {
                StaticSurfaceDisposition::BroadcastHandshake { method }
                | StaticSurfaceDisposition::InternalCancellation { method } => Some(method),
                StaticSurfaceDisposition::Noun { .. } => None,
            })
            .collect();
        assert_eq!(reserved.len(), exceptions.len());
        for command in BuiltinCommand::ALL {
            if let StaticSurfaceDisposition::Noun { noun, method, .. } =
                surface_entry(*command).disposition
            {
                assert!(
                    !reserved.contains(&method),
                    "noun {noun:?} reuses the reserved non-noun spelling {method}",
                );
            }
        }
    }
}
