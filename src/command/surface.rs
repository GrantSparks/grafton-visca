//! Closed static camera-surface metadata for built-in requests.
//!
//! [`crate::command::semantics::BuiltinCommand`] is the only command-row
//! inventory.  This module adds no second list and makes no surface decision
//! of its own: [`StaticNoun`], the exhaustive [`surface_entry`] match and the
//! [`BuiltinInquiryGate`] of every built-in inquiry are generated from the
//! `@noun` headers and rows of [`crate::noun_table`], which give each command
//! its noun, method spelling and capability gate and each inquiry its gate.  A
//! command without a noun-table row (or protocol exception) fails to build, and
//! so does a typed built-in inquiry without one.  Each match arm's class is a
//! named constant checked against the ledger by `registry_class`, so a row
//! whose kind disagrees with its command fails const evaluation under
//! `cargo check`.

use crate::{capabilities::TypedSupportSurface, noun_table::noun_table};

use super::semantics::{BuiltinCommand, BuiltinRequestClass, BuiltinRequestKind};

/// The capability gate of one noun-table row: the row's own `where` marker,
/// or else its noun header's base gate.
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
    /// A typed-support marker is required (`gate: [typed <Marker>]`, or a
    /// row `where <Marker>`).
    Typed(TypedSupportSurface),
}

/// The base-domain marker of a noun header's `[domain <Marker>]` gate.
///
/// There is one variant per domain that some noun-table row inherits.  The
/// `Image` noun is gated on `HasImageProcessing`, but every one of its rows
/// narrows that with its own `where` marker, so it has no variant here.
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
    /// `HasExposure`.
    Exposure,
    /// `HasWhiteBalance`.
    WhiteBalance,
    /// `HasPresets`.
    Presets,
    /// `HasMenuControl`.
    MenuControl,
}

impl DomainGate {
    /// Whether a runtime profile has the domain behind this marker.
    ///
    /// The base-domain markers are blanket-implemented from the domain data
    /// traits (`capabilities::profile_metadata`: `impl<T: Power> HasPower`,
    /// ...), which the `Capabilities::has_*` flags mirror at runtime.  A
    /// pan/tilt position can only be decoded through the profile's coordinate
    /// conversion, so the pan/tilt domain also requires one.  A runtime
    /// `ProfileSpec` cannot express "no menu", so basic OSD menu control stays
    /// reachable on every profile.
    pub(crate) fn permits(self, profile: &crate::ProfileSpec) -> bool {
        let capabilities = profile.capabilities();
        match self {
            Self::Power => capabilities.has_power,
            Self::Zoom => capabilities.has_zoom,
            Self::PanTilt => capabilities.has_pan_tilt && profile.pan_tilt_coordinates().is_some(),
            Self::Focus => capabilities.has_focus,
            Self::Exposure => capabilities.has_exposure,
            Self::WhiteBalance => capabilities.has_white_balance,
            Self::Presets => capabilities.has_presets,
            Self::MenuControl => true,
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

/// Maps a base-domain marker to its [`DomainGate`].
macro_rules! domain_gate {
    (HasPower) => {
        DomainGate::Power
    };
    (HasZoom) => {
        DomainGate::Zoom
    };
    (HasPanTilt) => {
        DomainGate::PanTilt
    };
    (HasFocus) => {
        DomainGate::Focus
    };
    (HasExposure) => {
        DomainGate::Exposure
    };
    (HasWhiteBalance) => {
        DomainGate::WhiteBalance
    };
    (HasPresets) => {
        DomainGate::Presets
    };
    (HasMenuControl) => {
        DomainGate::MenuControl
    };
    ($marker:ident) => {
        compile_error!(concat!(
            "a noun-table row inherits `",
            stringify!($marker),
            "`, which has no `DomainGate`: add a variant and its runtime check in `DomainGate::permits`",
        ))
    };
}

/// The [`RowGate`] of one row: its bracketed `where` marker, or else its
/// noun's base gate.
macro_rules! row_gate {
    ([], [always]) => {
        RowGate::Always
    };
    ([], [domain $marker:ident]) => {
        RowGate::Domain(domain_gate!($marker))
    };
    ([], [typed $marker:ident]) => {
        RowGate::Typed($crate::capabilities::typed_surface!($marker))
    };
    ([$gate:ident], $base:tt) => {
        RowGate::Typed($crate::capabilities::typed_surface!($gate))
    };
}

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

/// Implements [`BuiltinInquiryGate`] for the request type of an `inquiry`
/// row; every other row kind owns no inquiry.
macro_rules! inquiry_gate {
    (inquiry [$($request:tt)*] $gate:tt $base:tt) => {
        impl BuiltinInquiryGate for $($request)* {
            const GATE: RowGate = row_gate!($gate, $base);
        }
    };
    ($kind:ident $request:tt $gate:tt $base:tt) => {};
}

/// Project the noun headers, every noun row and the protocol exceptions into
/// [`StaticNoun`], the exhaustive [`surface_entry`] match and the inquiry
/// gates.
///
/// The first arm normalizes each noun to its name, doc, base gate and rows
/// (kind, optional command, method, bracketed row gate and bracketed request);
/// the second emits the items.  Inquiry and convenience rows (`[]`) emit no
/// match arm, and only inquiry rows emit a [`BuiltinInquiryGate`].  The match
/// therefore has no wildcard and no second hand-written inventory: a missing
/// row is a non-exhaustive match and a duplicate one an unreachable pattern.
macro_rules! surface_registry {
    (@items
        $( $noun:ident $doc:literal $base:tt [
            $( $kind:ident [$($command:ident)?] $method:ident $gate:tt $request:tt; )*
        ] )*
        @exceptions; $( $exkind:ident [$excommand:ident] $exmethod:ident; )*
    ) => {
        /// Static noun containing one target-facing built-in request.
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
        pub(crate) enum StaticNoun {
            $( #[doc = $doc] $noun, )*
        }

        /// Derive the one surface row for a semantic command.
        ///
        /// This match is generated from the noun table, is exhaustive and
        /// contains no default arm, so adding a command requires adding its
        /// explicit owner row there.  `BuiltinCommand` remains the semantic
        /// authority for classification.
        #[must_use]
        #[deny(unreachable_patterns)]
        pub(crate) const fn surface_entry(command: BuiltinCommand) -> StaticSurfaceEntry {
            match command {
                $( $( $(
                    BuiltinCommand::$command => {
                        const CLASS: BuiltinRequestClass =
                            registry_class(BuiltinCommand::$command, registry_class!($kind));
                        StaticSurfaceEntry {
                            command: BuiltinCommand::$command,
                            disposition: StaticSurfaceDisposition::Noun {
                                noun: StaticNoun::$noun,
                                method: stringify!($method),
                                gate: row_gate!($gate, $base),
                            },
                            class: CLASS,
                        }
                    }
                )? )* )*
                $(
                    BuiltinCommand::$excommand => {
                        const CLASS: BuiltinRequestClass =
                            registry_class(BuiltinCommand::$excommand, BuiltinRequestKind::Plain);
                        StaticSurfaceEntry {
                            command: BuiltinCommand::$excommand,
                            disposition: exception_disposition!($exkind, stringify!($exmethod)),
                            class: CLASS,
                        }
                    }
                )*
            }
        }

        const _: () = {
            use crate::command;

            $( $( inquiry_gate!($kind $request $gate $base); )* )*
        };
    };

    (
        $(
            @noun $noun:ident {
                accessor: $accessor:ident,
                getter: $getter:ident,
                dyn_trait: $dyn_trait:ident,
                gate: $base:tt,
                doc: $doc:literal $(,)?
            };
            $(
                $(#[$row_doc:meta])*
                $kind:ident [$($command:ident)?] $method:ident($($arg:ident: $ty:ty),*) -> $ret:ty
                    $(where $gate:ident $(+ $extra:ident)*)? = [$($request:tt)*];
            )*
        )*
        @exceptions; $( $exkind:ident [$excommand:ident] $exmethod:ident -> $exty:ty; )*
    ) => {
        surface_registry!(@items
            $( $noun $doc $base [ $( $kind [$($command)?] $method [$($gate)?] [$($request)*]; )* ] )*
            @exceptions; $( $exkind [$excommand] $exmethod; )*);
    };
}

noun_table!(surface_registry);

/// Returns the runtime typed-support surface carried by one static command row.
///
/// Request validation uses this projection for vendor commands whose typed
/// permission is entirely represented by an optional marker. That makes the
/// dynamic admission gate follow the same noun-table `where` bound that
/// governs static method resolution.
#[must_use]
pub(crate) const fn typed_surface_for_command(
    command: BuiltinCommand,
) -> Option<TypedSupportSurface> {
    match surface_entry(command).disposition {
        StaticSurfaceDisposition::Noun {
            gate: RowGate::Typed(surface),
            ..
        } => Some(surface),
        StaticSurfaceDisposition::Noun { .. }
        | StaticSurfaceDisposition::BroadcastHandshake { .. }
        | StaticSurfaceDisposition::InternalCancellation { .. } => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn surface_ledger_is_exhaustive_unique_and_class_balanced() {
        let mut seen = std::collections::HashSet::new();
        let mut plain = 0;
        let mut applied_only = 0;
        let mut targeted = 0;
        for command in BuiltinCommand::ALL {
            let entry = surface_entry(*command);
            assert!(seen.insert(*command), "duplicate surface ledger row");
            assert_eq!(entry.command, *command);
            assert_eq!(entry.class, command.classification());
            match entry.class {
                BuiltinRequestClass::Plain { .. } => plain += 1,
                BuiltinRequestClass::AppliedOnly { .. } => applied_only += 1,
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
            (plain, applied_only, targeted),
            (118, 16, 15),
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
