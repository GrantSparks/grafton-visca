//! Canonical noun metadata projected onto the closed protocol ledger.
//!
//! The noun table names public requests. Their exact `LEDGER_ROWS` describe
//! the protocol identities they can encode; query wrappers supply their raw
//! kind and source. Const evaluation proves exhaustive protocol coverage and
//! compatible shared rows, then builds an indexed lookup for preparation.

use crate::{capabilities::TypedSupportSet, noun_table::noun_table, source_evidence::Source};

pub(crate) use crate::capabilities::Domain as DomainGate;

use super::semantics::{
    BuiltinAxisSelection, BuiltinCommand, BuiltinRequestClass, BuiltinRequestKind,
};

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

    /// Enforces the capability and settlement contract of a command row.
    pub(crate) fn validate_command(
        self,
        command: BuiltinCommand,
        profile: &crate::ProfileSpec,
        feature: &'static str,
    ) -> crate::Result<()> {
        self.validate(profile, feature)?;
        if !permits_settlement(
            command,
            profile.settlement_axes(),
            profile.preset_recall_axes(),
        ) {
            return Err(crate::Error::FeatureNotSupported { feature });
        }
        Ok(())
    }

    /// Whether the row consumes a typed grant, beyond presence of a domain.
    pub(crate) const fn requires_typed_support(self) -> bool {
        matches!(self, Self::Typed(_) | Self::DomainAndTyped(_, _))
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

/// Tests the ledger's exact settlement requirement against a profile's proof.
pub(crate) fn permits_settlement(
    command: BuiltinCommand,
    available: Option<crate::AffectedAxes>,
    recall: Option<crate::AffectedAxes>,
) -> bool {
    let required = match command.classification() {
        BuiltinRequestClass::Targeted {
            axes: BuiltinAxisSelection::Exact(axes),
        } => Some(axes),
        BuiltinRequestClass::Targeted {
            axes: BuiltinAxisSelection::ProfilePresetRecall,
        } => recall,
        _ => return true,
    };
    required.is_some_and(|axes| available.is_some_and(|available| available.contains(axes)))
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
    // The settlement refinements are proved from the ledger's targeted axes
    // in validate_command. They do not consume a typed capability grant.
    ([HasPanTiltSettlement], [domain HasPanTilt]) => { $crate::command::surface::RowGate::Domain($crate::capabilities::Domain::PanTilt) };
    ([HasFocusSettlement], [domain HasFocus]) => { $crate::command::surface::RowGate::Domain($crate::capabilities::Domain::Focus) };
    ([HasPresetRecallSettlement], [domain HasPresets]) => { $crate::command::surface::RowGate::Domain($crate::capabilities::Domain::Presets) };
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
    /// A session owner emits the wire primitive directly.
    OwnerOnly,
}

/// One derived row in the closed static noun ledger.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) struct StaticSurfaceEntry {
    /// Exactly one source semantic row.
    pub(crate) command: BuiltinCommand,
    /// Noun mapping or owner-only wire primitive.
    pub(crate) disposition: StaticSurfaceDisposition,
    /// Semantic return class copied from the authoritative row.
    pub(crate) class: BuiltinRequestClass,
}

impl StaticSurfaceEntry {
    /// Canonical method spelling for a refused row's diagnostic.
    pub(crate) const fn feature(self) -> &'static str {
        match self.disposition {
            StaticSurfaceDisposition::Noun { method, .. } => method,
            StaticSurfaceDisposition::OwnerOnly => "owner-only VISCA primitive",
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

/// Extracts a canonical query type while preserving one request-expression source.
/// Constructors and physical reply mapping do not change its request identity.
macro_rules! query_type {
    ($consumer:ident, [$($arguments:tt)*], [with_decoder [$($request:tt)*] => $decoder:path]) => {
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
    name: &'static str,
    rows: &'static [BuiltinCommand],
    kind: BuiltinRequestKind,
    disposition: StaticSurfaceDisposition,
}

#[derive(Clone, Copy)]
struct QuerySurface {
    name: &'static str,
    source: Source,
    kind: crate::command::inquiry_structs::InquiryKind,
    gate: RowGate,
}

macro_rules! command_surface {
    (inquiry, $noun:ident, $key:ident, $method:ident, $request:ty, $gate:tt, $base:tt) => {
        None
    };
    ($kind:ident, $noun:ident, $key:ident, $method:ident, $request:ty, $gate:tt, $base:tt) => {
        Some(CommandSurface {
            name: concat!(stringify!($noun), "::", stringify!($key)),
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
    ([$gate:tt $base:tt $key:ident], $request:ty) => {
        Some(QuerySurface {
            name: stringify!($key),
            source: <$request as crate::command::nouns::NounInquiryContract>::SOURCE,
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
        query_type!(query_surface_entry, [$gate $base $key], $request)
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
                title: $title:literal,
                gate: $base:tt,
                doc: $doc:literal $(,)?
            };
            $(
                $(#[$row_doc:meta])*
                $kind:ident [$key:ident] $method:ident($($(#[$coercion:ident])? $arg:ident: $ty:ty),*) -> $ret:ty
                    $(where $gate:ident $(+ $extra:ident)*)? = [$($request:tt)*];
            )*
        )*
    ) => {
        /// The noun accessor owning a canonical method.
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
        pub(crate) enum StaticNoun {
            $( #[doc = $doc] $noun, )*
        }

        #[cfg(test)]
        impl StaticNoun {
            pub(crate) const fn title(self) -> &'static str {
                match self { $(Self::$noun => $title,)* }
            }

            pub(crate) const fn accessor(self) -> &'static str {
                match self { $(Self::$noun => stringify!($getter),)* }
            }
        }

        const COMMAND_SURFACES: &[Option<CommandSurface>] = {
            use crate::command;
            &[
                $( $( command_surface!($kind, $noun, $key, $method, $ret, [$($gate $(, $extra)*)?], $base), )* )*
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

#[derive(Clone, Copy)]
struct FixedSource {
    gate: RowGate,
    row: &'static str,
    source: Source,
}

/// Collect the fixed-domain and unconditional source promises once. Value
/// families prove their exact selected ledger row independently.
const fn fixed_sources() -> [Option<FixedSource>; COMMAND_SURFACES.len() + QUERY_SURFACES.len()] {
    let mut sources = [None; COMMAND_SURFACES.len() + QUERY_SURFACES.len()];
    let mut count = 0;
    let mut index = 0;
    while index < COMMAND_SURFACES.len() {
        if let Some(CommandSurface {
            rows: [row],
            name,
            disposition:
                StaticSurfaceDisposition::Noun {
                    gate: gate @ (RowGate::Always | RowGate::Domain(_)),
                    ..
                },
            ..
        }) = COMMAND_SURFACES[index]
        {
            sources[count] = Some(FixedSource {
                gate,
                row: name,
                source: row.source(),
            });
            count += 1;
        }
        index += 1;
    }
    let mut query_index = 0;
    while query_index < QUERY_SURFACES.len() {
        if let Some(QuerySurface {
            gate: gate @ (RowGate::Always | RowGate::Domain(_)),
            name,
            source,
            ..
        }) = QUERY_SURFACES[query_index]
        {
            sources[count] = Some(FixedSource {
                gate,
                row: name,
                source,
            });
            count += 1;
        }
        query_index += 1;
    }
    sources
}

const fn first_uncovered_source(
    profile: crate::profiles::ProfileId,
    gate: RowGate,
    sources: &[Option<FixedSource>],
) -> Option<&'static str> {
    let mut index = 0;
    while index < sources.len() {
        let Some(FixedSource {
            gate: required,
            row,
            source,
        }) = sources[index]
        else {
            break;
        };
        if required.same_as(gate) && !source.covers_model(Some(profile)) {
            return Some(row);
        }
        index += 1;
    }
    None
}

/// Check the promises of one gate for one profile implementing that gate.
pub(crate) const fn uncovered_fixed_source(
    profile: crate::profiles::ProfileId,
    gate: RowGate,
) -> Option<&'static str> {
    const SOURCES: &[Option<FixedSource>] = &fixed_sources();
    first_uncovered_source(profile, gate, SOURCES)
}

/// Compiles the protocol projection once; request preparation performs one
/// indexed lookup rather than searching canonical methods at runtime.
#[allow(clippy::panic)]
const fn build_surface_index() -> [StaticSurfaceEntry; BuiltinCommand::ALL.len()] {
    let first = BuiltinCommand::ALL[0];
    let mut index = [StaticSurfaceEntry {
        command: first,
        disposition: StaticSurfaceDisposition::OwnerOnly,
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
                        _ => panic!("an owner-only protocol row appeared in the noun table"),
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
        if !covered[row] {
            let command = BuiltinCommand::ALL[row];
            let class = command.classification();
            assert!(
                matches!(class, BuiltinRequestClass::OwnerOnly),
                "a request-bearing protocol row has no canonical noun method"
            );
            index[row] = StaticSurfaceEntry {
                command,
                disposition: StaticSurfaceDisposition::OwnerOnly,
                class,
            };
        }
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
        StaticSurfaceDisposition::OwnerOnly => RowGate::Always,
    }
}

/// The parent domains required by every row that consumes one typed surface.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum SurfaceDomain {
    /// Every consuming row is domain free.
    None,
    /// Every consuming row requires this one domain.
    One(DomainGate),
    /// Domains disagree, a domain-free row is mixed in, or no row consumes it.
    /// Such a surface needs an explicit metadata rule.
    Mixed,
}

impl SurfaceDomain {
    /// The fallback's capability check. Const validation rejects `Mixed` for
    /// every surface without an explicit discovery rule.
    pub(crate) fn permits(self, capabilities: &crate::capabilities::Capabilities) -> bool {
        match self {
            Self::None => true,
            Self::One(domain) => domain.permits_capabilities(capabilities),
            Self::Mixed => false,
        }
    }
}

const fn fold_surface_domain(
    current: Option<SurfaceDomain>,
    gate: RowGate,
    surface: crate::capabilities::TypedSupportSurface,
) -> Option<SurfaceDomain> {
    let next = match gate {
        RowGate::Typed(surfaces) if surfaces.contains(surface) => SurfaceDomain::None,
        RowGate::DomainAndTyped(domain, surfaces) if surfaces.contains(surface) => {
            SurfaceDomain::One(domain)
        }
        RowGate::Always
        | RowGate::Domain(_)
        | RowGate::Typed(_)
        | RowGate::DomainAndTyped(_, _) => {
            return current;
        }
    };
    Some(match (current, next) {
        (None, next) => next,
        (Some(SurfaceDomain::None), SurfaceDomain::None) => SurfaceDomain::None,
        (Some(SurfaceDomain::One(left)), SurfaceDomain::One(right))
            if left as u8 == right as u8 =>
        {
            SurfaceDomain::One(left)
        }
        _ => SurfaceDomain::Mixed,
    })
}

/// Compile-time fold of command and inquiry gates generated from noun rows.
/// Discovery performs one indexed lookup, with no runtime ledger traversal.
pub(crate) const fn surface_domain(
    surface: crate::capabilities::TypedSupportSurface,
) -> SurfaceDomain {
    use crate::capabilities::TypedSupportSurface;
    const DOMAINS: [SurfaceDomain; TypedSupportSurface::ALL.len()] = {
        let mut domains = [SurfaceDomain::Mixed; TypedSupportSurface::ALL.len()];
        let mut surface_index = 0;
        while surface_index < TypedSupportSurface::ALL.len() {
            let surface = TypedSupportSurface::ALL[surface_index];
            let mut domain = None;
            let mut index = 0;
            while index < BuiltinCommand::ALL.len() {
                domain = fold_surface_domain(domain, row_gate(BuiltinCommand::ALL[index]), surface);
                index += 1;
            }
            index = 0;
            while index < QUERY_SURFACES.len() {
                if let Some(query) = QUERY_SURFACES[index] {
                    domain = fold_surface_domain(domain, query.gate, surface);
                }
                index += 1;
            }
            domains[surface_index] = match domain {
                Some(domain) => domain,
                None => SurfaceDomain::Mixed,
            };
            surface_index += 1;
        }
        domains
    };
    DOMAINS[surface as usize]
}

/// Whether a typed grant exclusively enables inquiries. Both sides come from
/// canonical surfaces: a qualifier absent from command row gates is not itself
/// an inquiry permission (for example, the shared digital-zoom range).
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
    const INQUIRY_SURFACES: TypedSupportSet = {
        let mut surfaces = TypedSupportSet::EMPTY;
        let mut index = 0;
        while index < QUERY_SURFACES.len() {
            if let Some(query) = QUERY_SURFACES[index] {
                match query.gate {
                    RowGate::Typed(required) | RowGate::DomainAndTyped(_, required) => {
                        surfaces = surfaces.union(required);
                    }
                    RowGate::Always | RowGate::Domain(_) => {}
                }
            }
            index += 1;
        }
        surfaces
    };
    INQUIRY_SURFACES.contains(surface) && !COMMAND_SURFACES.contains(surface)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fixed_source_checks_name_the_first_uncovered_row() {
        let sources = [
            Some(FixedSource {
                gate: RowGate::Domain(DomainGate::Focus),
                row: "Focus::Stop",
                source: Source::Unsourced,
            }),
            Some(FixedSource {
                gate: RowGate::Always,
                row: "Always::Missing",
                source: Source::Unsourced,
            }),
        ];
        for profile in crate::profiles::ProfileId::all() {
            assert_eq!(
                first_uncovered_source(*profile, RowGate::Always, &sources),
                Some("Always::Missing")
            );
            assert_eq!(
                first_uncovered_source(*profile, RowGate::Domain(DomainGate::Focus), &sources),
                Some("Focus::Stop")
            );
            assert_eq!(
                first_uncovered_source(*profile, RowGate::Domain(DomainGate::Zoom), &sources),
                None
            );
        }
    }

    #[test]
    fn surface_domains_are_derived_from_every_consuming_row() {
        use crate::capabilities::TypedSupportSurface as S;
        assert_eq!(
            surface_domain(S::GainLimit),
            SurfaceDomain::One(DomainGate::Exposure)
        );
        assert_eq!(surface_domain(S::Tally), SurfaceDomain::None);
        let mixed = S::ALL
            .into_iter()
            .filter(|surface| surface_domain(*surface) == SurfaceDomain::Mixed)
            .collect::<Vec<_>>();
        // Its dedicated digital-range fact handles both domain-bearing zoom
        // rows and the domain-free high-resolution zoom noun.
        assert_eq!(mixed, [S::DigitalZoomRange]);
    }

    #[test]
    fn surface_domain_fold_rejects_mixed_or_absent_parent_rules() {
        use crate::capabilities::TypedSupportSurface as S;
        let required = TypedSupportSet::from_surface(S::GainLimit);
        let exposure = RowGate::DomainAndTyped(DomainGate::Exposure, required);
        let zoom = RowGate::DomainAndTyped(DomainGate::Zoom, required);
        let standalone = RowGate::Typed(required);
        let first = fold_surface_domain(None, exposure, S::GainLimit);
        assert_eq!(first, Some(SurfaceDomain::One(DomainGate::Exposure)));
        assert_eq!(fold_surface_domain(first, exposure, S::GainLimit), first);
        assert_eq!(
            fold_surface_domain(first, zoom, S::GainLimit),
            Some(SurfaceDomain::Mixed)
        );
        assert_eq!(
            fold_surface_domain(first, standalone, S::GainLimit),
            Some(SurfaceDomain::Mixed)
        );
        assert_eq!(
            fold_surface_domain(Some(SurfaceDomain::None), exposure, S::GainLimit),
            Some(SurfaceDomain::Mixed)
        );
        assert_eq!(
            fold_surface_domain(Some(SurfaceDomain::Mixed), exposure, S::GainLimit),
            Some(SurfaceDomain::Mixed)
        );
        assert_eq!(fold_surface_domain(None, exposure, S::Tally), None);
    }

    #[test]
    #[allow(clippy::expect_used)]
    fn moving_a_noun_header_changes_admission_and_discovery_together() {
        use crate::capabilities::TypedSupportSurface as S;
        // The only changed input is the same noun header consumed by the
        // production surface table. No discovery-side domain list is edited.
        let before = noun_row_gate!([HasGainLimit], [domain HasExposure]);
        let after = noun_row_gate!([HasGainLimit], [domain HasZoom]);
        let mut caps =
            crate::capabilities::Capabilities::runtime_baseline("moved noun", 1).expect("caps");
        caps.typed_support = TypedSupportSet::from_surface(S::GainLimit);
        caps.zoom = crate::profiles::ProfileId::PtzOpticsG2
            .spec()
            .expect("profile")
            .capabilities()
            .zoom;
        for (gate, expected) in [(before, false), (after, true)] {
            let discovered =
                fold_surface_domain(None, gate, S::GainLimit).expect("row owns surface");
            assert_eq!(gate.permits(&caps), expected);
            assert_eq!(discovered.permits(&caps), expected);
        }
    }

    #[test]
    fn inquiry_only_permissions_exclude_command_and_shared_range_grants() {
        use crate::capabilities::TypedSupportSurface as S;
        for inquiry in [
            S::VersionInquiry,
            S::MenuDisplayInquiry,
            S::ColorTemperatureInquiry,
            S::FocusZoneInquiry,
            S::PresetRecallSpeedInquiry,
        ] {
            assert!(is_inquiry_only_surface(inquiry), "{inquiry:?}");
        }
        for command in [S::DigitalZoomRange, S::DirectZoom, S::ExposureMode] {
            assert!(!is_inquiry_only_surface(command), "{command:?}");
        }
    }

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
    fn query_type_projection_unwraps_fallible_construction_and_custom_decoder() {
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
                    with_decoder [with_target |_target|
                        command::zoom::NormalizedInquiry::new(domain)]
                        => custom_decoder
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
        let mut owner_only = 0;
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
                BuiltinRequestClass::OwnerOnly => owner_only += 1,
                BuiltinRequestClass::AppliedOnly { stop, .. } => {
                    applied_only += 1;
                    stops += usize::from(stop);
                }
                BuiltinRequestClass::Targeted { .. } => targeted += 1,
            }
            match entry.disposition {
                StaticSurfaceDisposition::Noun { method, .. } => assert!(!method.is_empty()),
                StaticSurfaceDisposition::OwnerOnly => {
                    assert_eq!(entry.class, BuiltinRequestClass::OwnerOnly);
                }
            }
        }

        // Pin counts per class to detect reclassification. Update these counts
        // in the commit that changes the ledger and name the class in its message.
        assert_eq!(
            (plain, applied_only, stops, targeted, owner_only),
            (161, 22, 4, 15, 3),
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
                StaticSurfaceDisposition::OwnerOnly => ("owner-only VISCA primitive", None),
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
    fn owner_only_rows_never_have_noun_dispositions() {
        let owner_only: Vec<BuiltinCommand> = BuiltinCommand::ALL
            .iter()
            .copied()
            .filter(|command| !surface_entry(*command).is_target_facing())
            .collect();
        assert_eq!(
            owner_only,
            [
                BuiltinCommand::AddressSet,
                BuiltinCommand::InterfaceClear,
                BuiltinCommand::CommandCancel,
            ]
        );
        let reserved = ["address_set", "interface_clear", "cancel_command"];
        for command in BuiltinCommand::ALL {
            if let StaticSurfaceDisposition::Noun { noun, method, .. } =
                surface_entry(*command).disposition
            {
                assert!(
                    !reserved.contains(&method),
                    "noun {noun:?} reuses the reserved owner-only spelling {method}",
                );
            }
        }
    }
}
