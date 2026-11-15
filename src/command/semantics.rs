//! Authoritative semantic classification for built-in command requests.
//!
//! This module is deliberately a small, data-oriented ledger.  It is the
//! source that the typed-request conversion consumes; command bytes do not
//! carry semantic metadata.  The `builtin_command_ledger!` list below is
//! the one place a built-in command is named: [`BuiltinCommand`], its
//! test-build `BuiltinCommand::ALL` inventory and its
//! [`BuiltinCommand::classification`] are all expanded from that list, so an
//! unreviewed variant cannot exist and no second row list can drift.
//!
//! The classification rule from issue #542 is:
//!
//! * a request is an operation only when it starts, stops, or retargets
//!   physical actuation and exact VISCA cancellation has useful lifecycle
//!   meaning;
//! * [`BuiltinRequestClass::Targeted`] is reserved for a meaningful physical
//!   end state;
//! * [`BuiltinRequestClass::AppliedOnly`] is used when actuation has no
//!   meaningful settled target; and
//! * configuration, mode selection, and stored-state edits are plain.
//!
//! Operation axes are [`crate::AffectedAxes`]. Iris and ND filter are
//! included because their commands physically reposition a lens/filter and
//! both have exact position inquiries in the built-in inquiry inventory. They
//! are intentionally not silently mapped to an all-axis movement query.
//!
//! The production typed-request lowering consumes this ledger through
//! [`BuiltinRequestContract`]: every built-in request type names one ledger
//! row, its declared class is checked against that row at compile time, and
//! its fixed operation axes and write-only state effect are read from it.
//! The ledger is therefore load-bearing in ordinary builds, not only in the
//! independent semantic tests.

use crate::{AffectedAxes, StateKey};

/// Exact axis selection for an operation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum BuiltinAxisSelection {
    /// The command always affects this exact non-empty set.
    Exact(AffectedAxes),
    /// Preset recall selects the exact set from validated profile facts.
    ///
    /// This is not an all-axis fallback: a profile must provide a concrete,
    /// non-empty set before a preset request can be prepared.
    ProfilePresetRecall,
}

impl BuiltinAxisSelection {
    /// Returns whether two selections are the same exact axes or both
    /// profile-selected.
    const fn same_selection(self, other: Self) -> bool {
        match (self, other) {
            (Self::Exact(left), Self::Exact(right)) => left.bits() == right.bits(),
            (Self::ProfilePresetRecall, Self::ProfilePresetRecall) => true,
            (Self::Exact(_), Self::ProfilePresetRecall)
            | (Self::ProfilePresetRecall, Self::Exact(_)) => false,
        }
    }
}

/// Required cache projection for one write-only state command.
///
/// The value itself is carried by the typed request. This closed requirement
/// tells preparation whether that value can be written,
/// removed, or must be forgotten after exact application.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum AppliedStateEffectRequirement {
    /// The request supplies a deterministic replacement value.
    Set(StateKey),
    /// The request removes one deterministic state entry (for example a
    /// cleared pan/tilt limit corner). A boolean `Off` command is `Set` with
    /// a false value, because false is still known after exact application.
    Clear(StateKey),
    /// The request changes state without exposing a deterministic value.
    Invalidate(StateKey),
}

impl AppliedStateEffectRequirement {
    /// Returns whether two effects have the same verb and state key.
    const fn same_effect(self, other: Self) -> bool {
        match (self, other) {
            (Self::Set(left), Self::Set(right))
            | (Self::Clear(left), Self::Clear(right))
            | (Self::Invalidate(left), Self::Invalidate(right)) => left as u8 == right as u8,
            _ => false,
        }
    }

    /// Returns the affected write-only state key.
    #[cfg(test)]
    #[must_use]
    pub(crate) const fn state(self) -> StateKey {
        match self {
            Self::Set(state) | Self::Clear(state) | Self::Invalidate(state) => state,
        }
    }
}

/// Semantic class of one built-in request.
///
/// Operation variants carry their exact non-empty axis selection directly in
/// the enum.  It is therefore impossible for a ledger row to represent an
/// operation without affected axes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum BuiltinRequestClass {
    /// Configuration, mode, persistence, and stored-state edit.
    Plain {
        /// Closed cache requirement, when this is a write-only state edit.
        state_effect: Option<AppliedStateEffectRequirement>,
    },
    /// Physical actuation with a meaningful end state.
    Targeted {
        /// Exact fixed or profile-selected affected axes.
        axes: BuiltinAxisSelection,
    },
    /// Physical actuation whose terminal protocol application is the only
    /// meaningful lifecycle point.
    AppliedOnly {
        /// Exact fixed affected axes.
        axes: BuiltinAxisSelection,
    },
}

impl BuiltinRequestClass {
    /// Returns the completion class, if this is an operation.
    #[cfg(test)]
    #[must_use]
    pub(crate) const fn completion(self) -> Option<BuiltinCompletionClass> {
        match self {
            Self::Plain { .. } => None,
            Self::Targeted { .. } => Some(BuiltinCompletionClass::Targeted),
            Self::AppliedOnly { .. } => Some(BuiltinCompletionClass::AppliedOnly),
        }
    }

    /// Returns the operation's exact axis selection, if any.
    #[cfg(test)]
    #[must_use]
    pub(crate) const fn axes(self) -> Option<BuiltinAxisSelection> {
        match self {
            Self::Plain { .. } => None,
            Self::Targeted { axes } | Self::AppliedOnly { axes } => Some(axes),
        }
    }

    /// Returns the request kind this class requires.
    #[must_use]
    pub(crate) const fn kind(self) -> BuiltinRequestKind {
        match self {
            Self::Plain { .. } => BuiltinRequestKind::Plain,
            Self::Targeted { .. } => BuiltinRequestKind::Targeted,
            Self::AppliedOnly { .. } => BuiltinRequestKind::AppliedOnly,
        }
    }

    /// Returns whether two classes impose the same request contract: the
    /// same class, the same axis selection and the same state effect.
    #[must_use]
    pub(crate) const fn same_contract(self, other: Self) -> bool {
        match (self, other) {
            (
                Self::Plain { state_effect: left },
                Self::Plain {
                    state_effect: right,
                },
            ) => match (left, right) {
                (None, None) => true,
                (Some(left), Some(right)) => left.same_effect(right),
                (None, Some(_)) | (Some(_), None) => false,
            },
            (Self::Targeted { axes: left }, Self::Targeted { axes: right })
            | (Self::AppliedOnly { axes: left }, Self::AppliedOnly { axes: right }) => {
                left.same_selection(right)
            }
            _ => false,
        }
    }

    /// Returns the closed write-only state requirement, if any.
    #[must_use]
    #[cfg(any(feature = "async", feature = "blocking", test))]
    pub(crate) const fn state_effect(self) -> Option<AppliedStateEffectRequirement> {
        match self {
            Self::Plain { state_effect } => state_effect,
            Self::Targeted { .. } | Self::AppliedOnly { .. } => None,
        }
    }
}

/// Completion class selected by a built-in operation.
#[cfg(test)]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum BuiltinCompletionClass {
    /// Physical motion has a meaningful terminal state.
    Targeted,
    /// Physical actuation is observed only through command application.
    AppliedOnly,
}

/// The request kind of one ledger row: the closed `Request::Class` a typed
/// request serving it must declare, and the kind a noun-table row spells as
/// `plain`, `applied` or `targeted`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum BuiltinRequestKind {
    /// `request::Plain`.
    Plain,
    /// `request::Operation<completion::AppliedOnly>`.
    AppliedOnly,
    /// `request::Operation<completion::Targeted>`.
    Targeted,
}

/// The ledger kind of a closed request class marker.
pub(crate) trait BuiltinClassMarker {
    /// Kind a built-in request with this class must serve.
    const KIND: BuiltinRequestKind;
}

impl BuiltinClassMarker for crate::request::Plain {
    const KIND: BuiltinRequestKind = BuiltinRequestKind::Plain;
}

impl BuiltinClassMarker for crate::request::Operation<crate::completion::Targeted> {
    const KIND: BuiltinRequestKind = BuiltinRequestKind::Targeted;
}

impl BuiltinClassMarker for crate::request::Operation<crate::completion::AppliedOnly> {
    const KIND: BuiltinRequestKind = BuiltinRequestKind::AppliedOnly;
}

/// A built-in request type's tie to the ledger.
///
/// Everything else in the type's contract (the class check, fixed operation
/// axes, the write-only state effect and the typed capability gate) is read
/// from [`Self::LEDGER_ROW`].
pub(crate) trait BuiltinRequestContract: crate::Request<Class: BuiltinClassMarker> {
    /// A ledger row this type serves. Every ledger row served by the type
    /// must classify identically; the typed-request inventory checks this at
    /// compile time.
    const LEDGER_ROW: BuiltinCommand;
    /// Proof that the declared closed `Request::Class` serves the kind of
    /// [`Self::LEDGER_ROW`]: evaluating it fails when they disagree.
    /// `builtin_request!` forces it for every built-in type with a
    /// module-level `const`, so a mismatch fails `cargo check`. Implementors
    /// never override it.
    const CLASS_MATCHES_ROW: () = assert!(
        Self::LEDGER_ROW.classification().kind() as u8
            == <Self::Class as BuiltinClassMarker>::KIND as u8,
        "request class disagrees with its ledger row",
    );
}

/// The exact fixed axes of `T`'s ledger row.
///
/// # Panics
///
/// Fails const evaluation when the row is plain or selects its axes from the
/// profile.
#[allow(clippy::panic)]
pub(crate) const fn fixed_axes<T: BuiltinRequestContract>() -> AffectedAxes {
    match T::LEDGER_ROW.classification() {
        BuiltinRequestClass::Targeted {
            axes: BuiltinAxisSelection::Exact(axes),
        }
        | BuiltinRequestClass::AppliedOnly {
            axes: BuiltinAxisSelection::Exact(axes),
        } => axes,
        _ => panic!("ledger row has no fixed operation axes"),
    }
}

/// The write-only state effect of `T`'s ledger row.
///
/// # Panics
///
/// Fails const evaluation when the row has no state effect.
#[allow(clippy::panic)]
#[cfg(any(feature = "blocking", feature = "async"))]
pub(crate) const fn state_effect<T: BuiltinRequestContract>() -> AppliedStateEffectRequirement {
    match T::LEDGER_ROW.classification().state_effect() {
        Some(effect) => effect,
        None => panic!("ledger row has no write-only state effect"),
    }
}

/// The closed semantic ledger: one row per built-in command, in protocol
/// order.
///
/// `plain`, `plain(Set(<key>))`, `plain(Clear(<key>))`,
/// `plain(Invalidate(<key>))`, `targeted(<axes>)`,
/// `targeted(profile_preset_recall)` and `applied(<axes>)` are the only row
/// forms; `<key>` names a [`StateKey`] and `<axes>` an
/// [`AffectedAxes`] constant. [`BuiltinCommand`], the test-build
/// `BuiltinCommand::ALL` and [`BuiltinCommand::classification`] are all
/// expanded from this one list, so a row cannot be declared without a
/// classification, listed twice, or left out of the inventory.
macro_rules! builtin_command_ledger {
    ($( $(#[$meta:meta])* $command:ident => { class: $class:ident $(($($effect:tt)*))?, source: $source:expr } ),+ $(,)?) => {
        /// Every built-in command request that must be audited before typed
        /// conversion; generated from `builtin_command_ledger!`.
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
        pub(crate) enum BuiltinCommand {
            $( $(#[$meta])* $command, )+
        }

        impl BuiltinCommand {
            /// Every ledger row, in protocol order. Production code reaches
            /// rows only through exhaustive matches, so the inventory exists
            /// for tests alone.
            #[cfg(test)]
            pub(crate) const ALL: &'static [Self] = &[ $( Self::$command, )+ ];

            /// Source authority for this exact built-in row.
            pub(crate) const fn source(self) -> crate::source_evidence::Source {
                match self { $( Self::$command => $source, )+ }
            }

            /// Returns the ledger's classification of this row.
            #[must_use]
            pub(crate) const fn classification(self) -> BuiltinRequestClass {
                match self {
                    $( Self::$command => ledger_class!($class $(($($effect)*))?), )+
                }
            }
        }
    };
}

/// Expands one ledger row form into its [`BuiltinRequestClass`].
macro_rules! ledger_class {
    (plain) => {
        BuiltinRequestClass::Plain { state_effect: None }
    };
    (plain(Set($key:ident))) => {
        BuiltinRequestClass::Plain {
            state_effect: Some(AppliedStateEffectRequirement::Set(StateKey::$key)),
        }
    };
    (plain(Clear($key:ident))) => {
        BuiltinRequestClass::Plain {
            state_effect: Some(AppliedStateEffectRequirement::Clear(StateKey::$key)),
        }
    };
    (plain(Invalidate($key:ident))) => {
        BuiltinRequestClass::Plain {
            state_effect: Some(AppliedStateEffectRequirement::Invalidate(
                StateKey::$key,
            )),
        }
    };
    // Must precede the generic `targeted($axes)` arm.
    (targeted(profile_preset_recall)) => {
        BuiltinRequestClass::Targeted {
            axes: BuiltinAxisSelection::ProfilePresetRecall,
        }
    };
    (targeted($axes:ident)) => {
        BuiltinRequestClass::Targeted {
            axes: BuiltinAxisSelection::Exact(AffectedAxes::$axes),
        }
    };
    (applied($axes:ident)) => {
        BuiltinRequestClass::AppliedOnly {
            axes: BuiltinAxisSelection::Exact(AffectedAxes::$axes),
        }
    };
    ($($row:tt)*) => {
        compile_error!(concat!(
            "unknown ledger class `",
            stringify!($($row)*),
            "`; expected plain, plain(Set(<StateKey>)), plain(Clear(<StateKey>)), ",
            "plain(Invalidate(<StateKey>)), targeted(<AXES>), ",
            "targeted(profile_preset_recall) or applied(<AXES>)"
        ))
    };
}

builtin_command_ledger! {
    // Pan/tilt.
    PanTiltHome => { class: targeted(PAN_TILT), source: crate::source_evidence::source!([(R1, [PtzOpticsG2, PtzOptics30X], "r1-pantilthome-command", "01 06 04"), (R14, [PtzOpticsG3], "r14-pantilthome-command", "01 06 04"), (R7, [SonyFr7], "r7-pantilthome-command", "01 06 04"), (R11, [SonyBrcH900], "r11-pantilthome-command", "01 06 04"), (R8, [SonyEviH100, GenericVisca], "r8-pantilthome-command", "01 06 04"), (R12, [SonyBrc300, GenericVisca], "r12-pantilthome-command", "01 06 04"), (R21, [NearusBrc300, GenericVisca], "r21-pantilthome-command", "01 06 04")]) },
    PanTiltReset => { class: targeted(PAN_TILT), source: crate::source_evidence::source!([(R1, [PtzOpticsG2, PtzOptics30X], "r1-pantiltreset-command", "01 06 05"), (R14, [PtzOpticsG3], "r14-pantiltreset-command", "01 06 05"), (R7, [SonyFr7], "r7-pantiltreset-command", "01 06 05"), (R11, [SonyBrcH900], "r11-pantiltreset-command", "01 06 05"), (R8, [SonyEviH100, GenericVisca], "r8-pantiltreset-command", "01 06 05"), (R12, [SonyBrc300, GenericVisca], "r12-pantiltreset-command", "01 06 05"), (R21, [NearusBrc300, GenericVisca], "r21-pantiltreset-command", "01 06 05")]) },
    PanTiltDrive => { class: applied(PAN_TILT), source: crate::source_evidence::source!([(R1, [PtzOpticsG2, PtzOptics30X], "r1-pantiltdrive-command", "01 06 01"), (R14, [PtzOpticsG3], "r14-pantiltdrive-command", "01 06 01"), (R7, [SonyFr7], "r7-pantiltdrive-command", "01 06 01"), (R11, [SonyBrcH900], "r11-pantiltdrive-command", "01 06 01"), (R8, [SonyEviH100, GenericVisca], "r8-pantiltdrive-command", "01 06 01"), (R12, [SonyBrc300, GenericVisca], "r12-pantiltdrive-command", "01 06 01"), (R21, [NearusBrc300, GenericVisca], "r21-pantiltdrive-command", "01 06 01")]) },
    PanTiltStop => { class: applied(PAN_TILT), source: crate::source_evidence::source!([(R1, [PtzOpticsG2, PtzOptics30X], "r1-pantiltstop-command", "01 06 01"), (R14, [PtzOpticsG3], "r14-pantiltstop-command", "01 06 01"), (R7, [SonyFr7], "r7-pantiltstop-command", "01 06 01"), (R11, [SonyBrcH900], "r11-pantiltstop-command", "01 06 01"), (R8, [SonyEviH100, GenericVisca], "r8-pantiltstop-command", "01 06 01"), (R12, [SonyBrc300, GenericVisca], "r12-pantiltstop-command", "01 06 01"), (R21, [NearusBrc300, GenericVisca], "r21-pantiltstop-command", "01 06 01")]) },
    PanTiltAbsolute => { class: targeted(PAN_TILT), source: crate::source_evidence::source!([(R1, [PtzOpticsG2, PtzOptics30X], "r1-pantiltabsolute-command", "01 06 02"), (R14, [PtzOpticsG3], "r14-pantiltabsolute-command", "01 06 02"), (R7, [SonyFr7], "r7-pantiltabsolute-command", "01 06 02"), (R11, [SonyBrcH900], "r11-pantiltabsolute-command", "01 06 02"), (R8, [SonyEviH100, GenericVisca], "r8-pantiltabsolute-command", "01 06 02"), (R12, [SonyBrc300, GenericVisca], "r12-pantiltabsolute-command", "01 06 02"), (R21, [NearusBrc300, GenericVisca], "r21-pantiltabsolute-command", "01 06 02")]) },
    PanTiltRelative => { class: targeted(PAN_TILT), source: crate::source_evidence::source!([(R1, [PtzOpticsG2, PtzOptics30X], "r1-pantiltrelative-command", "01 06 03"), (R14, [PtzOpticsG3], "r14-pantiltrelative-command", "01 06 03"), (R7, [SonyFr7], "r7-pantiltrelative-command", "01 06 03"), (R11, [SonyBrcH900], "r11-pantiltrelative-command", "01 06 03"), (R8, [SonyEviH100, GenericVisca], "r8-pantiltrelative-command", "01 06 03"), (R12, [SonyBrc300, GenericVisca], "r12-pantiltrelative-command", "01 06 03"), (R21, [NearusBrc300, GenericVisca], "r21-pantiltrelative-command", "01 06 03")]) },
    PanTiltLimitSet => { class: plain(Set(PanTiltLimits)), source: crate::source_evidence::source!([(R1, [PtzOpticsG2, PtzOptics30X], "r1-pantiltlimitset-command", "01 06 07 00"), (R14, [PtzOpticsG3], "r14-pantiltlimitset-command", "01 06 07 00"), (R7, [SonyFr7], "r7-pantiltlimitset-command", "01 06 07 00"), (R11, [SonyBrcH900], "r11-pantiltlimitset-command", "01 06 07 00"), (R8, [SonyEviH100, GenericVisca], "r8-pantiltlimitset-command", "01 06 07 00"), (R12, [SonyBrc300, GenericVisca], "r12-pantiltlimitset-command", "01 06 07 00"), (R21, [NearusBrc300, GenericVisca], "r21-pantiltlimitset-command", "01 06 07 00")]) },
    PanTiltLimitClear => { class: plain(Clear(PanTiltLimits)), source: crate::source_evidence::source!([(R1, [PtzOpticsG2, PtzOptics30X], "r1-pantiltlimitclear-command", "01 06 07 01"), (R14, [PtzOpticsG3], "r14-pantiltlimitclear-command", "01 06 07 01"), (R7, [SonyFr7], "r7-pantiltlimitclear-command", "01 06 07 01"), (R11, [SonyBrcH900], "r11-pantiltlimitclear-command", "01 06 07 01"), (R8, [SonyEviH100, GenericVisca], "r8-pantiltlimitclear-command", "01 06 07 01"), (R12, [SonyBrc300, GenericVisca], "r12-pantiltlimitclear-command", "01 06 07 01"), (R21, [NearusBrc300, GenericVisca], "r21-pantiltlimitclear-command", "01 06 07 01")]) },
    // Zoom.
    ZoomStop => { class: applied(ZOOM), source: crate::source_evidence::source!([(R1, [PtzOpticsG2, PtzOptics30X], "r1-zoomstop-command", "01 04 07 00"), (R14, [PtzOpticsG3], "r14-zoomstop-command", "01 04 07 00"), (R7, [SonyFr7], "r7-zoomstop-command", "01 04 07 00"), (R11, [SonyBrcH900], "r11-zoomstop-command", "01 04 07 00"), (R8, [SonyEviH100, GenericVisca], "r8-zoomstop-command", "01 04 07 00"), (R12, [SonyBrc300, GenericVisca], "r12-zoomstop-command", "01 04 07 00"), (R21, [NearusBrc300, GenericVisca], "r21-zoomstop-command", "01 04 07 00")]) },
    ZoomTele => { class: applied(ZOOM), source: crate::source_evidence::source!([(R1, [PtzOpticsG2, PtzOptics30X], "r1-zoomtele-command", "01 04 07 02"), (R14, [PtzOpticsG3], "r14-zoomtele-command", "01 04 07 02"), (R7, [SonyFr7], "r7-zoomtele-command", "01 04 07 02"), (R11, [SonyBrcH900], "r11-zoomtele-command", "01 04 07 02"), (R8, [SonyEviH100, GenericVisca], "r8-zoomtele-command", "01 04 07 02"), (R12, [SonyBrc300, GenericVisca], "r12-zoomtele-command", "01 04 07 02"), (R21, [NearusBrc300, GenericVisca], "r21-zoomtele-command", "01 04 07 02")]) },
    ZoomWide => { class: applied(ZOOM), source: crate::source_evidence::source!([(R1, [PtzOpticsG2, PtzOptics30X], "r1-zoomwide-command", "01 04 07 03"), (R14, [PtzOpticsG3], "r14-zoomwide-command", "01 04 07 03"), (R7, [SonyFr7], "r7-zoomwide-command", "01 04 07 03"), (R11, [SonyBrcH900], "r11-zoomwide-command", "01 04 07 03"), (R8, [SonyEviH100, GenericVisca], "r8-zoomwide-command", "01 04 07 03"), (R12, [SonyBrc300, GenericVisca], "r12-zoomwide-command", "01 04 07 03"), (R21, [NearusBrc300, GenericVisca], "r21-zoomwide-command", "01 04 07 03")]) },
    ZoomTeleVariable => { class: applied(ZOOM), source: crate::source_evidence::source!([(R1, [PtzOpticsG2, PtzOptics30X], "r1-zoomtelevariable-command", "01 04 07"), (R14, [PtzOpticsG3], "r14-zoomtelevariable-command", "01 04 07"), (R7, [SonyFr7], "r7-zoomtelevariable-command", "01 04 07"), (R11, [SonyBrcH900], "r11-zoomtelevariable-command", "01 04 07"), (R8, [SonyEviH100, GenericVisca], "r8-zoomtelevariable-command", "01 04 07"), (R12, [SonyBrc300, GenericVisca], "r12-zoomtelevariable-command", "01 04 07"), (R21, [NearusBrc300, GenericVisca], "r21-zoomtelevariable-command", "01 04 07")]) },
    ZoomWideVariable => { class: applied(ZOOM), source: crate::source_evidence::source!([(R1, [PtzOpticsG2, PtzOptics30X], "r1-zoomwidevariable-command", "01 04 07"), (R14, [PtzOpticsG3], "r14-zoomwidevariable-command", "01 04 07"), (R7, [SonyFr7], "r7-zoomwidevariable-command", "01 04 07"), (R11, [SonyBrcH900], "r11-zoomwidevariable-command", "01 04 07"), (R8, [SonyEviH100, GenericVisca], "r8-zoomwidevariable-command", "01 04 07"), (R12, [SonyBrc300, GenericVisca], "r12-zoomwidevariable-command", "01 04 07"), (R21, [NearusBrc300, GenericVisca], "r21-zoomwidevariable-command", "01 04 07")]) },
    ZoomPosition => { class: targeted(ZOOM), source: crate::source_evidence::source!([(R1, [PtzOpticsG2, PtzOptics30X], "r1-zoomposition-command", "01 04 47"), (R14, [PtzOpticsG3], "r14-zoomposition-command", "01 04 47"), (R7, [SonyFr7], "r7-zoomposition-command", "01 04 47"), (R11, [SonyBrcH900], "r11-zoomposition-command", "01 04 47"), (R8, [SonyEviH100, GenericVisca], "r8-zoomposition-command", "01 04 47"), (R12, [SonyBrc300, GenericVisca], "r12-zoomposition-command", "01 04 47"), (R21, [NearusBrc300, GenericVisca], "r21-zoomposition-command", "01 04 47")]) },
    DigitalZoom => { class: plain(Set(DigitalZoomMode)), source: crate::source_evidence::source!([(R8, [SonyEviH100, GenericVisca], "r8-digitalzoom-command", "01 04 06"), (R12, [SonyBrc300, GenericVisca], "r12-digitalzoom-command", "01 04 06"), (R21, [NearusBrc300, GenericVisca], "r21-digitalzoom-command", "01 04 06")]) },
    // Focus.
    FocusStop => { class: applied(FOCUS), source: crate::source_evidence::source!([(R1, [PtzOpticsG2, PtzOptics30X], "r1-focusstop-command", "01 04 08 00"), (R14, [PtzOpticsG3], "r14-focusstop-command", "01 04 08 00"), (R7, [SonyFr7], "r7-focusstop-command", "01 04 08 00"), (R11, [SonyBrcH900], "r11-focusstop-command", "01 04 08 00"), (R8, [SonyEviH100, GenericVisca], "r8-focusstop-command", "01 04 08 00"), (R12, [SonyBrc300, GenericVisca], "r12-focusstop-command", "01 04 08 00"), (R21, [NearusBrc300, GenericVisca], "r21-focusstop-command", "01 04 08 00")]) },
    FocusFar => { class: applied(FOCUS), source: crate::source_evidence::source!([(R1, [PtzOpticsG2, PtzOptics30X], "r1-focusfar-command", "01 04 08 02"), (R14, [PtzOpticsG3], "r14-focusfar-command", "01 04 08 02"), (R7, [SonyFr7], "r7-focusfar-command", "01 04 08 02"), (R11, [SonyBrcH900], "r11-focusfar-command", "01 04 08 02"), (R8, [SonyEviH100, GenericVisca], "r8-focusfar-command", "01 04 08 02"), (R12, [SonyBrc300, GenericVisca], "r12-focusfar-command", "01 04 08 02"), (R21, [NearusBrc300, GenericVisca], "r21-focusfar-command", "01 04 08 02")]) },
    FocusNear => { class: applied(FOCUS), source: crate::source_evidence::source!([(R1, [PtzOpticsG2, PtzOptics30X], "r1-focusnear-command", "01 04 08 03"), (R14, [PtzOpticsG3], "r14-focusnear-command", "01 04 08 03"), (R7, [SonyFr7], "r7-focusnear-command", "01 04 08 03"), (R11, [SonyBrcH900], "r11-focusnear-command", "01 04 08 03"), (R8, [SonyEviH100, GenericVisca], "r8-focusnear-command", "01 04 08 03"), (R12, [SonyBrc300, GenericVisca], "r12-focusnear-command", "01 04 08 03"), (R21, [NearusBrc300, GenericVisca], "r21-focusnear-command", "01 04 08 03")]) },
    FocusFarVariable => { class: applied(FOCUS), source: crate::source_evidence::source!([(R1, [PtzOpticsG2, PtzOptics30X], "r1-focusfarvariable-command", "01 04 08"), (R14, [PtzOpticsG3], "r14-focusfarvariable-command", "01 04 08"), (R7, [SonyFr7], "r7-focusfarvariable-command", "01 04 08"), (R11, [SonyBrcH900], "r11-focusfarvariable-command", "01 04 08"), (R8, [SonyEviH100, GenericVisca], "r8-focusfarvariable-command", "01 04 08"), (R12, [SonyBrc300, GenericVisca], "r12-focusfarvariable-command", "01 04 08"), (R21, [NearusBrc300, GenericVisca], "r21-focusfarvariable-command", "01 04 08")]) },
    FocusNearVariable => { class: applied(FOCUS), source: crate::source_evidence::source!([(R1, [PtzOpticsG2, PtzOptics30X], "r1-focusnearvariable-command", "01 04 08"), (R14, [PtzOpticsG3], "r14-focusnearvariable-command", "01 04 08"), (R7, [SonyFr7], "r7-focusnearvariable-command", "01 04 08"), (R11, [SonyBrcH900], "r11-focusnearvariable-command", "01 04 08"), (R8, [SonyEviH100, GenericVisca], "r8-focusnearvariable-command", "01 04 08"), (R12, [SonyBrc300, GenericVisca], "r12-focusnearvariable-command", "01 04 08"), (R21, [NearusBrc300, GenericVisca], "r21-focusnearvariable-command", "01 04 08")]) },
    FocusPosition => { class: targeted(FOCUS), source: crate::source_evidence::source!([(R1, [PtzOpticsG2, PtzOptics30X], "r1-focusposition-command", "01 04 48"), (R14, [PtzOpticsG3], "r14-focusposition-command", "01 04 48"), (R7, [SonyFr7], "r7-focusposition-command", "01 04 48"), (R11, [SonyBrcH900], "r11-focusposition-command", "01 04 48"), (R8, [SonyEviH100, GenericVisca], "r8-focusposition-command", "01 04 48"), (R12, [SonyBrc300, GenericVisca], "r12-focusposition-command", "01 04 48"), (R21, [NearusBrc300, GenericVisca], "r21-focusposition-command", "01 04 48")]) },
    FocusAuto => { class: plain, source: crate::source_evidence::source!([(R1, [PtzOpticsG2, PtzOptics30X], "r1-focusauto-command", "01 04 38 02"), (R14, [PtzOpticsG3], "r14-focusauto-command", "01 04 38 02"), (R7, [SonyFr7], "r7-focusauto-command", "01 04 38 02"), (R11, [SonyBrcH900], "r11-focusauto-command", "01 04 38 02"), (R8, [SonyEviH100, GenericVisca], "r8-focusauto-command", "01 04 38 02"), (R12, [SonyBrc300, GenericVisca], "r12-focusauto-command", "01 04 38 02"), (R21, [NearusBrc300, GenericVisca], "r21-focusauto-command", "01 04 38 02")]) },
    FocusManual => { class: plain, source: crate::source_evidence::source!([(R1, [PtzOpticsG2, PtzOptics30X], "r1-focusmanual-command", "01 04 38 03"), (R14, [PtzOpticsG3], "r14-focusmanual-command", "01 04 38 03"), (R7, [SonyFr7], "r7-focusmanual-command", "01 04 38 03"), (R11, [SonyBrcH900], "r11-focusmanual-command", "01 04 38 03"), (R8, [SonyEviH100, GenericVisca], "r8-focusmanual-command", "01 04 38 03"), (R12, [SonyBrc300, GenericVisca], "r12-focusmanual-command", "01 04 38 03"), (R21, [NearusBrc300, GenericVisca], "r21-focusmanual-command", "01 04 38 03")]) },
    FocusOnePush => { class: applied(FOCUS), source: crate::source_evidence::source!([(R8, [SonyEviH100, GenericVisca], "r8-focusonepush-command", "01 04 18 01"), (R12, [SonyBrc300, GenericVisca], "r12-focusonepush-command", "01 04 18 01"), (R21, [NearusBrc300, GenericVisca], "r21-focusonepush-command", "01 04 18 01"), (R11, [SonyBrcH900], "r11-focusonepush-command", "01 04 18 01")]) },
    FocusInfinity => { class: targeted(FOCUS), source: crate::source_evidence::source!([(R8, [SonyEviH100, GenericVisca], "r8-focusinfinity-command", "01 04 18 02"), (R12, [SonyBrc300, GenericVisca], "r12-focusinfinity-command", "01 04 18 02"), (R21, [NearusBrc300, GenericVisca], "r21-focusinfinity-command", "01 04 18 02"), (R11, [SonyBrcH900], "r11-focusinfinity-command", "01 04 18 02")]) },
    FocusToggle => { class: plain, source: crate::source_evidence::source!([(R1, [PtzOpticsG2, PtzOptics30X], "r1-focustoggle-command", "01 04 38 10"), (R14, [PtzOpticsG3], "r14-focustoggle-command", "01 04 38 10"), (R7, [SonyFr7], "r7-focustoggle-command", "01 04 38 10"), (R11, [SonyBrcH900], "r11-focustoggle-command", "01 04 38 10"), (R8, [SonyEviH100, GenericVisca], "r8-focustoggle-command", "01 04 38 10"), (R12, [SonyBrc300, GenericVisca], "r12-focustoggle-command", "01 04 38 10"), (R21, [NearusBrc300, GenericVisca], "r21-focustoggle-command", "01 04 38 10")]) },
    FocusSnap => { class: applied(FOCUS), source: crate::source_evidence::Source::Unsourced },
    FocusZone => { class: plain, source: crate::source_evidence::source!([(R20, [PtzOpticsG2, PtzOpticsG3, PtzOptics30X], "r20-focuszone-command", "01 04 AA")]) },
    FocusAutoSensitivity => { class: plain, source: crate::source_evidence::source!([(R1, [PtzOpticsG2, PtzOptics30X], "r1-focusautosensitivity-command", "01 04 58"), (R14, [PtzOpticsG3], "r14-focusautosensitivity-command", "01 04 58"), (R8, [SonyEviH100], "r8-focusautosensitivity-command", "01 04 58")]) },
    FocusNearLimit => { class: plain, source: crate::source_evidence::source!([(R8, [SonyEviH100], "r8-focusnearlimit-command", "01 04 28")]) },
    FocusLock => { class: plain(Set(FocusLockMode)), source: crate::source_evidence::source!([(R1, [PtzOpticsG2, PtzOptics30X], "r1-focuslock-command", "0A 04 68"), (R14, [PtzOpticsG3], "r14-focuslock-command", "0A 04 68")]) },
    PushAfPress => { class: applied(FOCUS), source: crate::source_evidence::source!([(R7, [SonyFr7], "r7-pushafpress-command", "01 7E 04 58")]) },
    PushAfRelease => { class: applied(FOCUS), source: crate::source_evidence::source!([(R7, [SonyFr7], "r7-pushafrelease-command", "01 7E 04 58")]) },
    // Presets.
    PresetRecall => { class: targeted(profile_preset_recall), source: crate::source_evidence::source!([(R1, [PtzOpticsG2, PtzOptics30X], "r1-presetrecall-command", "01 04 3F 02"), (R14, [PtzOpticsG3], "r14-presetrecall-command", "01 04 3F 02"), (R7, [SonyFr7], "r7-presetrecall-command", "01 04 3F 02"), (R11, [SonyBrcH900], "r11-presetrecall-command", "01 04 3F 02"), (R8, [SonyEviH100, GenericVisca], "r8-presetrecall-command", "01 04 3F 02"), (R12, [SonyBrc300, GenericVisca], "r12-presetrecall-command", "01 04 3F 02"), (R21, [NearusBrc300, GenericVisca], "r21-presetrecall-command", "01 04 3F 02")]) },
    PresetRecallSpeed => { class: plain(Set(PresetRecallSpeed)), source: crate::source_evidence::source!([(R1, [PtzOpticsG2, PtzOptics30X], "r1-presetrecallspeed-command", "01 06 01"), (R10, [PtzOpticsG3], "r10-presetrecallspeed-command", "01 06 01")]) },
    PresetSet => { class: plain, source: crate::source_evidence::source!([(R1, [PtzOpticsG2, PtzOptics30X], "r1-presetset-command", "01 04 3F 01"), (R14, [PtzOpticsG3], "r14-presetset-command", "01 04 3F 01"), (R7, [SonyFr7], "r7-presetset-command", "01 04 3F 01"), (R8, [SonyEviH100, GenericVisca], "r8-presetset-command", "01 04 3F 01"), (R12, [SonyBrc300, GenericVisca], "r12-presetset-command", "01 04 3F 01"), (R21, [NearusBrc300, GenericVisca], "r21-presetset-command", "01 04 3F 01")]) },
    PresetReset => { class: plain, source: crate::source_evidence::source!([(R1, [PtzOpticsG2, PtzOptics30X], "r1-presetreset-command", "01 04 3F 00"), (R14, [PtzOpticsG3], "r14-presetreset-command", "01 04 3F 00"), (R7, [SonyFr7], "r7-presetreset-command", "01 04 3F 00"), (R8, [SonyEviH100, GenericVisca], "r8-presetreset-command", "01 04 3F 00"), (R12, [SonyBrc300, GenericVisca], "r12-presetreset-command", "01 04 3F 00"), (R21, [NearusBrc300, GenericVisca], "r21-presetreset-command", "01 04 3F 00")]) },
    // Power.
    PowerOn => { class: plain, source: crate::source_evidence::source!([(R1, [PtzOpticsG2, PtzOptics30X], "r1-poweron-command", "01 04 00 02"), (R14, [PtzOpticsG3], "r14-poweron-command", "01 04 00 02"), (R7, [SonyFr7], "r7-poweron-command", "01 04 00 02"), (R11, [SonyBrcH900], "r11-poweron-command", "01 04 00 02"), (R8, [SonyEviH100, GenericVisca], "r8-poweron-command", "01 04 00 02"), (R12, [SonyBrc300, GenericVisca], "r12-poweron-command", "01 04 00 02"), (R21, [NearusBrc300, GenericVisca], "r21-poweron-command", "01 04 00 02")]) },
    PowerStandby => { class: plain, source: crate::source_evidence::source!([(R1, [PtzOpticsG2, PtzOptics30X], "r1-powerstandby-command", "01 04 00 03"), (R14, [PtzOpticsG3], "r14-powerstandby-command", "01 04 00 03"), (R7, [SonyFr7], "r7-powerstandby-command", "01 04 00 03"), (R11, [SonyBrcH900], "r11-powerstandby-command", "01 04 00 03"), (R8, [SonyEviH100, GenericVisca], "r8-powerstandby-command", "01 04 00 03"), (R12, [SonyBrc300, GenericVisca], "r12-powerstandby-command", "01 04 00 03"), (R21, [NearusBrc300, GenericVisca], "r21-powerstandby-command", "01 04 00 03")]) },
    // Exposure and iris.
    ExposureMode => { class: plain, source: crate::source_evidence::source!([(R1, [PtzOpticsG2, PtzOptics30X], "r1-exposuremode-command", "01 04 39"), (R14, [PtzOpticsG3], "r14-exposuremode-command", "01 04 39"), (R11, [SonyBrcH900], "r11-exposuremode-command", "01 04 39"), (R8, [SonyEviH100, GenericVisca], "r8-exposuremode-command", "01 04 39"), (R12, [SonyBrc300, GenericVisca], "r12-exposuremode-command", "01 04 39"), (R21, [NearusBrc300, GenericVisca], "r21-exposuremode-command", "01 04 39")]) },
    ExposureCompensationOn => { class: plain, source: crate::source_evidence::source!([(R1, [PtzOpticsG2, PtzOptics30X], "r1-exposurecompensationon-command", "01 04 3E 02"), (R14, [PtzOpticsG3], "r14-exposurecompensationon-command", "01 04 3E 02"), (R8, [SonyEviH100, GenericVisca], "r8-exposurecompensationon-command", "01 04 3E 02"), (R12, [SonyBrc300, GenericVisca], "r12-exposurecompensationon-command", "01 04 3E 02"), (R21, [NearusBrc300, GenericVisca], "r21-exposurecompensationon-command", "01 04 3E 02")]) },
    ExposureCompensationOff => { class: plain, source: crate::source_evidence::source!([(R1, [PtzOpticsG2, PtzOptics30X], "r1-exposurecompensationoff-command", "01 04 3E 03"), (R14, [PtzOpticsG3], "r14-exposurecompensationoff-command", "01 04 3E 03"), (R8, [SonyEviH100, GenericVisca], "r8-exposurecompensationoff-command", "01 04 3E 03"), (R12, [SonyBrc300, GenericVisca], "r12-exposurecompensationoff-command", "01 04 3E 03"), (R21, [NearusBrc300, GenericVisca], "r21-exposurecompensationoff-command", "01 04 3E 03")]) },
    ExposureCompensationReset => { class: plain, source: crate::source_evidence::source!([(R1, [PtzOpticsG2, PtzOptics30X], "r1-exposurecompensationreset-command", "01 04 0E 00"), (R14, [PtzOpticsG3], "r14-exposurecompensationreset-command", "01 04 0E 00"), (R8, [SonyEviH100, GenericVisca], "r8-exposurecompensationreset-command", "01 04 0E 00"), (R12, [SonyBrc300, GenericVisca], "r12-exposurecompensationreset-command", "01 04 0E 00"), (R21, [NearusBrc300, GenericVisca], "r21-exposurecompensationreset-command", "01 04 0E 00")]) },
    ExposureCompensationUp => { class: plain, source: crate::source_evidence::source!([(R1, [PtzOpticsG2, PtzOptics30X], "r1-exposurecompensationup-command", "01 04 0E 02"), (R14, [PtzOpticsG3], "r14-exposurecompensationup-command", "01 04 0E 02"), (R8, [SonyEviH100, GenericVisca], "r8-exposurecompensationup-command", "01 04 0E 02"), (R12, [SonyBrc300, GenericVisca], "r12-exposurecompensationup-command", "01 04 0E 02"), (R21, [NearusBrc300, GenericVisca], "r21-exposurecompensationup-command", "01 04 0E 02")]) },
    ExposureCompensationDown => { class: plain, source: crate::source_evidence::source!([(R1, [PtzOpticsG2, PtzOptics30X], "r1-exposurecompensationdown-command", "01 04 0E 03"), (R14, [PtzOpticsG3], "r14-exposurecompensationdown-command", "01 04 0E 03"), (R8, [SonyEviH100, GenericVisca], "r8-exposurecompensationdown-command", "01 04 0E 03"), (R12, [SonyBrc300, GenericVisca], "r12-exposurecompensationdown-command", "01 04 0E 03"), (R21, [NearusBrc300, GenericVisca], "r21-exposurecompensationdown-command", "01 04 0E 03")]) },
    ExposureCompensationDirect => { class: plain, source: crate::source_evidence::source!([(R1, [PtzOpticsG2, PtzOptics30X], "r1-exposurecompensationdirect-command", "01 04 4E"), (R14, [PtzOpticsG3], "r14-exposurecompensationdirect-command", "01 04 4E"), (R8, [SonyEviH100, GenericVisca], "r8-exposurecompensationdirect-command", "01 04 4E"), (R12, [SonyBrc300, GenericVisca], "r12-exposurecompensationdirect-command", "01 04 4E"), (R21, [NearusBrc300, GenericVisca], "r21-exposurecompensationdirect-command", "01 04 4E")]) },
    DynamicRange => { class: plain, source: crate::source_evidence::source!([(R1, [PtzOpticsG2, PtzOptics30X], "r1-dynamicrange-command", "01 04 25"), (R14, [PtzOpticsG3], "r14-dynamicrange-command", "01 04 25")]) },
    // Iris commands are finite physical aperture movements.  They have a
    // meaningful end state and an exact Iris inquiry, so every
    // reset/step/direct form is targeted rather than applied-only.
    IrisReset => { class: targeted(IRIS), source: crate::source_evidence::source!([(R1, [PtzOpticsG2, PtzOptics30X], "r1-irisreset-command", "01 04 0B 00"), (R14, [PtzOpticsG3], "r14-irisreset-command", "01 04 0B 00"), (R11, [SonyBrcH900], "r11-irisreset-command", "01 04 0B 00"), (R8, [SonyEviH100, GenericVisca], "r8-irisreset-command", "01 04 0B 00"), (R12, [SonyBrc300, GenericVisca], "r12-irisreset-command", "01 04 0B 00"), (R21, [NearusBrc300, GenericVisca], "r21-irisreset-command", "01 04 0B 00")]) },
    IrisUp => { class: targeted(IRIS), source: crate::source_evidence::source!([(R1, [PtzOpticsG2, PtzOptics30X], "r1-irisup-command", "01 04 0B 02"), (R14, [PtzOpticsG3], "r14-irisup-command", "01 04 0B 02"), (R11, [SonyBrcH900], "r11-irisup-command", "01 04 0B 02"), (R8, [SonyEviH100, GenericVisca], "r8-irisup-command", "01 04 0B 02"), (R12, [SonyBrc300, GenericVisca], "r12-irisup-command", "01 04 0B 02"), (R21, [NearusBrc300, GenericVisca], "r21-irisup-command", "01 04 0B 02")]) },
    IrisDown => { class: targeted(IRIS), source: crate::source_evidence::source!([(R1, [PtzOpticsG2, PtzOptics30X], "r1-irisdown-command", "01 04 0B 03"), (R14, [PtzOpticsG3], "r14-irisdown-command", "01 04 0B 03"), (R11, [SonyBrcH900], "r11-irisdown-command", "01 04 0B 03"), (R8, [SonyEviH100, GenericVisca], "r8-irisdown-command", "01 04 0B 03"), (R12, [SonyBrc300, GenericVisca], "r12-irisdown-command", "01 04 0B 03"), (R21, [NearusBrc300, GenericVisca], "r21-irisdown-command", "01 04 0B 03")]) },
    IrisDirect => { class: targeted(IRIS), source: crate::source_evidence::source!([(R1, [PtzOpticsG2, PtzOptics30X], "r1-irisdirect-command", "01 04 4B"), (R14, [PtzOpticsG3], "r14-irisdirect-command", "01 04 4B"), (R11, [SonyBrcH900], "r11-irisdirect-command", "01 04 4B"), (R8, [SonyEviH100, GenericVisca], "r8-irisdirect-command", "01 04 4B"), (R12, [SonyBrc300, GenericVisca], "r12-irisdirect-command", "01 04 4B"), (R21, [NearusBrc300, GenericVisca], "r21-irisdirect-command", "01 04 4B")]) },
    ShutterReset => { class: plain, source: crate::source_evidence::source!([(R1, [PtzOpticsG2, PtzOptics30X], "r1-shutterreset-command", "01 04 0A 00"), (R14, [PtzOpticsG3], "r14-shutterreset-command", "01 04 0A 00"), (R11, [SonyBrcH900], "r11-shutterreset-command", "01 04 0A 00"), (R8, [SonyEviH100, GenericVisca], "r8-shutterreset-command", "01 04 0A 00"), (R12, [SonyBrc300, GenericVisca], "r12-shutterreset-command", "01 04 0A 00"), (R21, [NearusBrc300, GenericVisca], "r21-shutterreset-command", "01 04 0A 00")]) },
    ShutterUp => { class: plain, source: crate::source_evidence::source!([(R1, [PtzOpticsG2, PtzOptics30X], "r1-shutterup-command", "01 04 0A 02"), (R14, [PtzOpticsG3], "r14-shutterup-command", "01 04 0A 02"), (R11, [SonyBrcH900], "r11-shutterup-command", "01 04 0A 02"), (R8, [SonyEviH100, GenericVisca], "r8-shutterup-command", "01 04 0A 02"), (R12, [SonyBrc300, GenericVisca], "r12-shutterup-command", "01 04 0A 02"), (R21, [NearusBrc300, GenericVisca], "r21-shutterup-command", "01 04 0A 02")]) },
    ShutterDown => { class: plain, source: crate::source_evidence::source!([(R1, [PtzOpticsG2, PtzOptics30X], "r1-shutterdown-command", "01 04 0A 03"), (R14, [PtzOpticsG3], "r14-shutterdown-command", "01 04 0A 03"), (R11, [SonyBrcH900], "r11-shutterdown-command", "01 04 0A 03"), (R8, [SonyEviH100, GenericVisca], "r8-shutterdown-command", "01 04 0A 03"), (R12, [SonyBrc300, GenericVisca], "r12-shutterdown-command", "01 04 0A 03"), (R21, [NearusBrc300, GenericVisca], "r21-shutterdown-command", "01 04 0A 03")]) },
    ShutterDirect => { class: plain, source: crate::source_evidence::source!([(R1, [PtzOpticsG2, PtzOptics30X], "r1-shutterdirect-command", "01 04 4A"), (R14, [PtzOpticsG3], "r14-shutterdirect-command", "01 04 4A"), (R11, [SonyBrcH900], "r11-shutterdirect-command", "01 04 4A"), (R8, [SonyEviH100, GenericVisca], "r8-shutterdirect-command", "01 04 4A"), (R12, [SonyBrc300, GenericVisca], "r12-shutterdirect-command", "01 04 4A"), (R21, [NearusBrc300, GenericVisca], "r21-shutterdirect-command", "01 04 4A")]) },
    BrightnessReset => { class: plain, source: crate::source_evidence::source!([(R1, [PtzOpticsG2, PtzOptics30X], "r1-brightnessreset-command", "01 04 0D 00"), (R14, [PtzOpticsG3], "r14-brightnessreset-command", "01 04 0D 00"), (R8, [SonyEviH100, GenericVisca], "r8-brightnessreset-command", "01 04 0D 00"), (R12, [SonyBrc300, GenericVisca], "r12-brightnessreset-command", "01 04 0D 00"), (R21, [NearusBrc300, GenericVisca], "r21-brightnessreset-command", "01 04 0D 00")]) },
    BrightnessUp => { class: plain, source: crate::source_evidence::source!([(R1, [PtzOpticsG2, PtzOptics30X], "r1-brightnessup-command", "01 04 0D 02"), (R14, [PtzOpticsG3], "r14-brightnessup-command", "01 04 0D 02"), (R8, [SonyEviH100, GenericVisca], "r8-brightnessup-command", "01 04 0D 02"), (R12, [SonyBrc300, GenericVisca], "r12-brightnessup-command", "01 04 0D 02"), (R21, [NearusBrc300, GenericVisca], "r21-brightnessup-command", "01 04 0D 02")]) },
    BrightnessDown => { class: plain, source: crate::source_evidence::source!([(R1, [PtzOpticsG2, PtzOptics30X], "r1-brightnessdown-command", "01 04 0D 03"), (R14, [PtzOpticsG3], "r14-brightnessdown-command", "01 04 0D 03"), (R8, [SonyEviH100, GenericVisca], "r8-brightnessdown-command", "01 04 0D 03"), (R12, [SonyBrc300, GenericVisca], "r12-brightnessdown-command", "01 04 0D 03"), (R21, [NearusBrc300, GenericVisca], "r21-brightnessdown-command", "01 04 0D 03")]) },
    BrightnessSet => { class: plain, source: crate::source_evidence::source!([(R1, [PtzOpticsG2, PtzOptics30X], "r1-brightnessset-command", "01 04 4D"), (R14, [PtzOpticsG3], "r14-brightnessset-command", "01 04 4D"), (R8, [SonyEviH100, GenericVisca], "r8-brightnessset-command", "01 04 4D"), (R12, [SonyBrc300, GenericVisca], "r12-brightnessset-command", "01 04 4D"), (R21, [NearusBrc300, GenericVisca], "r21-brightnessset-command", "01 04 4D")]) },
    AntiFlicker => { class: plain, source: crate::source_evidence::source!([(R1, [PtzOpticsG2, PtzOptics30X], "r1-antiflicker-command", "01 04 23"), (R10, [PtzOpticsG3], "r10-antiflicker-command", "01 04 23")]) },
    SpotlightOn => { class: plain(Set(Spotlight)), source: crate::source_evidence::source!([(R7, [SonyFr7], "r7-spotlighton-command", "01 04 3A 02"), (R11, [SonyBrcH900], "r11-spotlighton-command", "01 04 3A 02")]) },
    SpotlightOff => { class: plain(Set(Spotlight)), source: crate::source_evidence::source!([(R7, [SonyFr7], "r7-spotlightoff-command", "01 04 3A 03")]) },
    AutoSlowShutterOn => { class: plain(Set(AutoSlowShutter)), source: crate::source_evidence::source!([(R8, [SonyEviH100, GenericVisca], "r8-autoslowshutteron-command", "01 04 5A 02"), (R12, [SonyBrc300, GenericVisca], "r12-autoslowshutteron-command", "01 04 5A 02"), (R21, [NearusBrc300, GenericVisca], "r21-autoslowshutteron-command", "01 04 5A 02")]) },
    AutoSlowShutterOff => { class: plain(Set(AutoSlowShutter)), source: crate::source_evidence::source!([(R8, [SonyEviH100, GenericVisca], "r8-autoslowshutteroff-command", "01 04 5A 03"), (R12, [SonyBrc300, GenericVisca], "r12-autoslowshutteroff-command", "01 04 5A 03"), (R21, [NearusBrc300, GenericVisca], "r21-autoslowshutteroff-command", "01 04 5A 03")]) },
    // Gain.
    GainReset => { class: plain, source: crate::source_evidence::source!([(R1, [PtzOpticsG2, PtzOptics30X], "r1-gainreset-command", "01 04 0C 00"), (R14, [PtzOpticsG3], "r14-gainreset-command", "01 04 0C 00"), (R11, [SonyBrcH900], "r11-gainreset-command", "01 04 0C 00"), (R8, [SonyEviH100, GenericVisca], "r8-gainreset-command", "01 04 0C 00"), (R12, [SonyBrc300, GenericVisca], "r12-gainreset-command", "01 04 0C 00"), (R21, [NearusBrc300, GenericVisca], "r21-gainreset-command", "01 04 0C 00")]) },
    GainUp => { class: plain, source: crate::source_evidence::source!([(R1, [PtzOpticsG2, PtzOptics30X], "r1-gainup-command", "01 04 0C 02"), (R14, [PtzOpticsG3], "r14-gainup-command", "01 04 0C 02"), (R11, [SonyBrcH900], "r11-gainup-command", "01 04 0C 02"), (R8, [SonyEviH100, GenericVisca], "r8-gainup-command", "01 04 0C 02"), (R12, [SonyBrc300, GenericVisca], "r12-gainup-command", "01 04 0C 02"), (R21, [NearusBrc300, GenericVisca], "r21-gainup-command", "01 04 0C 02")]) },
    GainDown => { class: plain, source: crate::source_evidence::source!([(R1, [PtzOpticsG2, PtzOptics30X], "r1-gaindown-command", "01 04 0C 03"), (R14, [PtzOpticsG3], "r14-gaindown-command", "01 04 0C 03"), (R11, [SonyBrcH900], "r11-gaindown-command", "01 04 0C 03"), (R8, [SonyEviH100, GenericVisca], "r8-gaindown-command", "01 04 0C 03"), (R12, [SonyBrc300, GenericVisca], "r12-gaindown-command", "01 04 0C 03"), (R21, [NearusBrc300, GenericVisca], "r21-gaindown-command", "01 04 0C 03")]) },
    GainDirect => { class: plain, source: crate::source_evidence::source!([(R1, [PtzOpticsG2, PtzOptics30X], "r1-gaindirect-command", "01 04 4C"), (R14, [PtzOpticsG3], "r14-gaindirect-command", "01 04 4C"), (R11, [SonyBrcH900], "r11-gaindirect-command", "01 04 4C"), (R8, [SonyEviH100, GenericVisca], "r8-gaindirect-command", "01 04 4C"), (R12, [SonyBrc300, GenericVisca], "r12-gaindirect-command", "01 04 4C"), (R21, [NearusBrc300, GenericVisca], "r21-gaindirect-command", "01 04 4C")]) },
    GainLimit => { class: plain, source: crate::source_evidence::source!([(R1, [PtzOpticsG2, PtzOptics30X], "r1-gainlimit-command", "01 04 2C"), (R14, [PtzOpticsG3], "r14-gainlimit-command", "01 04 2C")]) },
    // White balance.
    WhiteBalanceAuto => { class: plain, source: crate::source_evidence::source!([(R1, [PtzOpticsG2, PtzOptics30X], "r1-whitebalanceauto-command", "01 04 35 00"), (R14, [PtzOpticsG3], "r14-whitebalanceauto-command", "01 04 35 00"), (R11, [SonyBrcH900], "r11-whitebalanceauto-command", "01 04 35 00"), (R8, [SonyEviH100, GenericVisca], "r8-whitebalanceauto-command", "01 04 35 00"), (R12, [SonyBrc300, GenericVisca], "r12-whitebalanceauto-command", "01 04 35 00"), (R21, [NearusBrc300, GenericVisca], "r21-whitebalanceauto-command", "01 04 35 00")]) },
    WhiteBalanceIndoor => { class: plain, source: crate::source_evidence::source!([(R1, [PtzOpticsG2, PtzOptics30X], "r1-whitebalanceindoor-command", "01 04 35 01"), (R14, [PtzOpticsG3], "r14-whitebalanceindoor-command", "01 04 35 01"), (R11, [SonyBrcH900], "r11-whitebalanceindoor-command", "01 04 35 01"), (R8, [SonyEviH100, GenericVisca], "r8-whitebalanceindoor-command", "01 04 35 01"), (R12, [SonyBrc300, GenericVisca], "r12-whitebalanceindoor-command", "01 04 35 01"), (R21, [NearusBrc300, GenericVisca], "r21-whitebalanceindoor-command", "01 04 35 01")]) },
    WhiteBalanceOutdoor => { class: plain, source: crate::source_evidence::source!([(R1, [PtzOpticsG2, PtzOptics30X], "r1-whitebalanceoutdoor-command", "01 04 35 02"), (R14, [PtzOpticsG3], "r14-whitebalanceoutdoor-command", "01 04 35 02"), (R11, [SonyBrcH900], "r11-whitebalanceoutdoor-command", "01 04 35 02"), (R8, [SonyEviH100, GenericVisca], "r8-whitebalanceoutdoor-command", "01 04 35 02"), (R12, [SonyBrc300, GenericVisca], "r12-whitebalanceoutdoor-command", "01 04 35 02"), (R21, [NearusBrc300, GenericVisca], "r21-whitebalanceoutdoor-command", "01 04 35 02")]) },
    WhiteBalanceOnePush => { class: plain, source: crate::source_evidence::source!([(R1, [PtzOpticsG2, PtzOptics30X], "r1-whitebalanceonepush-command", "01 04 35 03"), (R14, [PtzOpticsG3], "r14-whitebalanceonepush-command", "01 04 35 03"), (R11, [SonyBrcH900], "r11-whitebalanceonepush-command", "01 04 35 03"), (R8, [SonyEviH100, GenericVisca], "r8-whitebalanceonepush-command", "01 04 35 03"), (R12, [SonyBrc300, GenericVisca], "r12-whitebalanceonepush-command", "01 04 35 03"), (R21, [NearusBrc300, GenericVisca], "r21-whitebalanceonepush-command", "01 04 35 03")]) },
    WhiteBalanceAutoTracking => { class: plain, source: crate::source_evidence::source!([(R7, [SonyFr7], "r7-whitebalanceautotracking-command", "01 04 35 04")]) },
    WhiteBalanceManual => { class: plain, source: crate::source_evidence::source!([(R1, [PtzOpticsG2, PtzOptics30X], "r1-whitebalancemanual-command", "01 04 35 05"), (R14, [PtzOpticsG3], "r14-whitebalancemanual-command", "01 04 35 05"), (R11, [SonyBrcH900], "r11-whitebalancemanual-command", "01 04 35 05"), (R8, [SonyEviH100, GenericVisca], "r8-whitebalancemanual-command", "01 04 35 05"), (R12, [SonyBrc300, GenericVisca], "r12-whitebalancemanual-command", "01 04 35 05"), (R21, [NearusBrc300, GenericVisca], "r21-whitebalancemanual-command", "01 04 35 05")]) },
    WhiteBalanceColorTemperature => { class: plain, source: crate::source_evidence::source!([(R1, [PtzOpticsG2, PtzOptics30X], "r1-whitebalancecolortemperature-command", "01 04 35 20"), (R14, [PtzOpticsG3], "r14-whitebalancecolortemperature-command", "01 04 35 20")]) },
    AutoWhiteBalanceSensitivity => { class: plain, source: crate::source_evidence::source!([(R1, [PtzOpticsG2, PtzOptics30X], "r1-autowhitebalancesensitivity-command", "01 04 A9"), (R14, [PtzOpticsG3], "r14-autowhitebalancesensitivity-command", "01 04 A9")]) },
    OnePushWhiteBalanceTrigger => { class: plain, source: crate::source_evidence::source!([(R1, [PtzOpticsG2, PtzOptics30X], "r1-onepushwhitebalancetrigger-command", "01 04 10 05"), (R14, [PtzOpticsG3], "r14-onepushwhitebalancetrigger-command", "01 04 10 05"), (R8, [SonyEviH100, GenericVisca], "r8-onepushwhitebalancetrigger-command", "01 04 10 05"), (R12, [SonyBrc300, GenericVisca], "r12-onepushwhitebalancetrigger-command", "01 04 10 05"), (R21, [NearusBrc300, GenericVisca], "r21-onepushwhitebalancetrigger-command", "01 04 10 05")]) },
    // Color.
    RedTuning => { class: plain, source: crate::source_evidence::source!([(R1, [PtzOpticsG2, PtzOptics30X], "r1-redtuning-command", "01 04 43"), (R14, [PtzOpticsG3], "r14-redtuning-command", "01 04 43")]) },
    BlueTuning => { class: plain, source: crate::source_evidence::source!([(R1, [PtzOpticsG2, PtzOptics30X], "r1-bluetuning-command", "01 04 44"), (R14, [PtzOpticsG3], "r14-bluetuning-command", "01 04 44")]) },
    Saturation => { class: plain, source: crate::source_evidence::source!([(R1, [PtzOpticsG2, PtzOptics30X], "r1-saturation-command", "01 04 49"), (R14, [PtzOpticsG3], "r14-saturation-command", "01 04 49"), (R8, [SonyEviH100], "r8-saturation-command", "01 04 49")]) },
    Hue => { class: plain, source: crate::source_evidence::source!([(R1, [PtzOpticsG2, PtzOptics30X], "r1-hue-command", "01 04 4F"), (R14, [PtzOpticsG3], "r14-hue-command", "01 04 4F"), (R8, [SonyEviH100], "r8-hue-command", "01 04 4F")]) },
    ColorTemperatureReset => { class: plain, source: crate::source_evidence::source!([(R1, [PtzOpticsG2, PtzOptics30X], "r1-colortemperaturereset-command", "01 04 20"), (R14, [PtzOpticsG3], "r14-colortemperaturereset-command", "01 04 20")]) },
    ColorTemperatureUp => { class: plain, source: crate::source_evidence::source!([(R1, [PtzOpticsG2, PtzOptics30X], "r1-colortemperatureup-command", "01 04 20"), (R14, [PtzOpticsG3], "r14-colortemperatureup-command", "01 04 20")]) },
    ColorTemperatureDown => { class: plain, source: crate::source_evidence::source!([(R1, [PtzOpticsG2, PtzOptics30X], "r1-colortemperaturedown-command", "01 04 20"), (R14, [PtzOpticsG3], "r14-colortemperaturedown-command", "01 04 20")]) },
    ColorTemperatureDirect => { class: plain, source: crate::source_evidence::source!([(R1, [PtzOpticsG2, PtzOptics30X], "r1-colortemperaturedirect-command", "01 04 20"), (R14, [PtzOpticsG3], "r14-colortemperaturedirect-command", "01 04 20")]) },
    RedGainReset => { class: plain, source: crate::source_evidence::source!([(R1, [PtzOpticsG2, PtzOptics30X], "r1-redgainreset-command", "01 04 03 00"), (R14, [PtzOpticsG3], "r14-redgainreset-command", "01 04 03 00"), (R8, [SonyEviH100, GenericVisca], "r8-redgainreset-command", "01 04 03 00"), (R12, [SonyBrc300, GenericVisca], "r12-redgainreset-command", "01 04 03 00"), (R21, [NearusBrc300, GenericVisca], "r21-redgainreset-command", "01 04 03 00")]) },
    RedGainUp => { class: plain, source: crate::source_evidence::source!([(R1, [PtzOpticsG2, PtzOptics30X], "r1-redgainup-command", "01 04 03 02"), (R14, [PtzOpticsG3], "r14-redgainup-command", "01 04 03 02"), (R8, [SonyEviH100, GenericVisca], "r8-redgainup-command", "01 04 03 02"), (R12, [SonyBrc300, GenericVisca], "r12-redgainup-command", "01 04 03 02"), (R21, [NearusBrc300, GenericVisca], "r21-redgainup-command", "01 04 03 02")]) },
    RedGainDown => { class: plain, source: crate::source_evidence::source!([(R1, [PtzOpticsG2, PtzOptics30X], "r1-redgaindown-command", "01 04 03 03"), (R14, [PtzOpticsG3], "r14-redgaindown-command", "01 04 03 03"), (R8, [SonyEviH100, GenericVisca], "r8-redgaindown-command", "01 04 03 03"), (R12, [SonyBrc300, GenericVisca], "r12-redgaindown-command", "01 04 03 03"), (R21, [NearusBrc300, GenericVisca], "r21-redgaindown-command", "01 04 03 03")]) },
    RedGainDirect => { class: plain, source: crate::source_evidence::source!([(R1, [PtzOpticsG2, PtzOptics30X], "r1-redgaindirect-command", "01 04 43"), (R14, [PtzOpticsG3], "r14-redgaindirect-command", "01 04 43"), (R8, [SonyEviH100, GenericVisca], "r8-redgaindirect-command", "01 04 43"), (R12, [SonyBrc300, GenericVisca], "r12-redgaindirect-command", "01 04 43"), (R21, [NearusBrc300, GenericVisca], "r21-redgaindirect-command", "01 04 43")]) },
    BlueGainReset => { class: plain, source: crate::source_evidence::source!([(R1, [PtzOpticsG2, PtzOptics30X], "r1-bluegainreset-command", "01 04 04 00"), (R14, [PtzOpticsG3], "r14-bluegainreset-command", "01 04 04 00"), (R8, [SonyEviH100, GenericVisca], "r8-bluegainreset-command", "01 04 04 00"), (R12, [SonyBrc300, GenericVisca], "r12-bluegainreset-command", "01 04 04 00"), (R21, [NearusBrc300, GenericVisca], "r21-bluegainreset-command", "01 04 04 00")]) },
    BlueGainUp => { class: plain, source: crate::source_evidence::source!([(R1, [PtzOpticsG2, PtzOptics30X], "r1-bluegainup-command", "01 04 04 02"), (R14, [PtzOpticsG3], "r14-bluegainup-command", "01 04 04 02"), (R8, [SonyEviH100, GenericVisca], "r8-bluegainup-command", "01 04 04 02"), (R12, [SonyBrc300, GenericVisca], "r12-bluegainup-command", "01 04 04 02"), (R21, [NearusBrc300, GenericVisca], "r21-bluegainup-command", "01 04 04 02")]) },
    BlueGainDown => { class: plain, source: crate::source_evidence::source!([(R1, [PtzOpticsG2, PtzOptics30X], "r1-bluegaindown-command", "01 04 04 03"), (R14, [PtzOpticsG3], "r14-bluegaindown-command", "01 04 04 03"), (R8, [SonyEviH100, GenericVisca], "r8-bluegaindown-command", "01 04 04 03"), (R12, [SonyBrc300, GenericVisca], "r12-bluegaindown-command", "01 04 04 03"), (R21, [NearusBrc300, GenericVisca], "r21-bluegaindown-command", "01 04 04 03")]) },
    BlueGainDirect => { class: plain, source: crate::source_evidence::source!([(R1, [PtzOpticsG2, PtzOptics30X], "r1-bluegaindirect-command", "01 04 44"), (R14, [PtzOpticsG3], "r14-bluegaindirect-command", "01 04 44"), (R8, [SonyEviH100, GenericVisca], "r8-bluegaindirect-command", "01 04 44"), (R12, [SonyBrc300, GenericVisca], "r12-bluegaindirect-command", "01 04 44"), (R21, [NearusBrc300, GenericVisca], "r21-bluegaindirect-command", "01 04 44")]) },
    // Image processing and orientation.
    SharpnessMode => { class: plain, source: crate::source_evidence::source!([(R1, [PtzOpticsG2, PtzOptics30X], "r1-sharpnessmode-command", "01 04 05"), (R14, [PtzOpticsG3], "r14-sharpnessmode-command", "01 04 05")]) },
    SharpnessReset => { class: plain, source: crate::source_evidence::source!([(R1, [PtzOpticsG2, PtzOptics30X], "r1-sharpnessreset-command", "01 04 02 00"), (R14, [PtzOpticsG3], "r14-sharpnessreset-command", "01 04 02 00"), (R7, [SonyFr7], "r7-sharpnessreset-command", "01 04 02 00"), (R8, [SonyEviH100], "r8-sharpnessreset-command", "01 04 02 00")]) },
    SharpnessUp => { class: plain, source: crate::source_evidence::source!([(R1, [PtzOpticsG2, PtzOptics30X], "r1-sharpnessup-command", "01 04 02 02"), (R14, [PtzOpticsG3], "r14-sharpnessup-command", "01 04 02 02"), (R7, [SonyFr7], "r7-sharpnessup-command", "01 04 02 02"), (R8, [SonyEviH100], "r8-sharpnessup-command", "01 04 02 02")]) },
    SharpnessDown => { class: plain, source: crate::source_evidence::source!([(R1, [PtzOpticsG2, PtzOptics30X], "r1-sharpnessdown-command", "01 04 02 03"), (R14, [PtzOpticsG3], "r14-sharpnessdown-command", "01 04 02 03"), (R7, [SonyFr7], "r7-sharpnessdown-command", "01 04 02 03"), (R8, [SonyEviH100], "r8-sharpnessdown-command", "01 04 02 03")]) },
    SharpnessDirect => { class: plain, source: crate::source_evidence::source!([(R1, [PtzOpticsG2, PtzOptics30X], "r1-sharpnessdirect-command", "01 04 42"), (R14, [PtzOpticsG3], "r14-sharpnessdirect-command", "01 04 42"), (R7, [SonyFr7], "r7-sharpnessdirect-command", "01 04 42"), (R8, [SonyEviH100], "r8-sharpnessdirect-command", "01 04 42")]) },
    Luminance => { class: plain, source: crate::source_evidence::source!([(R1, [PtzOpticsG2, PtzOptics30X], "r1-luminance-command", "01 04 A1"), (R14, [PtzOpticsG3], "r14-luminance-command", "01 04 A1")]) },
    Contrast => { class: plain, source: crate::source_evidence::source!([(R1, [PtzOpticsG2, PtzOptics30X], "r1-contrast-command", "01 04 A2"), (R14, [PtzOpticsG3], "r14-contrast-command", "01 04 A2")]) },
    Gamma => { class: plain, source: crate::source_evidence::source!([(R1, [PtzOpticsG2, PtzOptics30X], "r1-gamma-command", "01 04 5B"), (R14, [PtzOpticsG3], "r14-gamma-command", "01 04 5B"), (R8, [SonyEviH100], "r8-gamma-command", "01 04 5B")]) },
    Backlight => { class: plain, source: crate::source_evidence::source!([(R1, [PtzOpticsG2, PtzOptics30X], "r1-backlight-command", "01 04 33"), (R14, [PtzOpticsG3], "r14-backlight-command", "01 04 33"), (R7, [SonyFr7], "r7-backlight-command", "01 04 33"), (R8, [SonyEviH100, GenericVisca], "r8-backlight-command", "01 04 33"), (R12, [SonyBrc300, GenericVisca], "r12-backlight-command", "01 04 33"), (R21, [NearusBrc300, GenericVisca], "r21-backlight-command", "01 04 33")]) },
    NoiseReduction2dMode => { class: plain, source: crate::source_evidence::source!([(R14, [PtzOpticsG2, PtzOpticsG3, PtzOptics30X], "r14-noisereduction2dmode-command", "01 04 50")]) },
    NoiseReduction2d => { class: plain, source: crate::source_evidence::source!([(R14, [PtzOpticsG2, PtzOpticsG3, PtzOptics30X], "r14-noisereduction2d-command", "01 04 53"), (R8, [SonyEviH100], "r8-noisereduction2d-command", "01 04 53")]) },
    NoiseReduction2dOff => { class: plain, source: crate::source_evidence::source!([(R14, [PtzOpticsG2, PtzOpticsG3, PtzOptics30X], "r14-noisereduction2doff-command", "01 04 53"), (R8, [SonyEviH100], "r8-noisereduction2doff-command", "01 04 53")]) },
    NoiseReduction3d => { class: plain, source: crate::source_evidence::source!([(R14, [PtzOpticsG2, PtzOpticsG3, PtzOptics30X], "r14-noisereduction3d-command", "01 04 54")]) },
    NoiseReduction3dOff => { class: plain, source: crate::source_evidence::source!([(R14, [PtzOpticsG2, PtzOpticsG3, PtzOptics30X], "r14-noisereduction3doff-command", "01 04 54")]) },
    // The single-axis flip and mirror opcodes move one axis and say nothing
    // about the other, so a complete pair stops being known.
    ImageFlipOff => { class: plain(Invalidate(Flip)), source: crate::source_evidence::source!([(R1, [PtzOpticsG2, PtzOptics30X], "r1-imageflipoff-command", "01 04 66"), (R14, [PtzOpticsG3], "r14-imageflipoff-command", "01 04 66"), (R12, [SonyBrc300], "r12-imageflipoff-command", "01 04 66"), (R21, [NearusBrc300], "r21-imageflipoff-command", "01 04 66")]) },
    ImageFlipHorizontal => { class: plain(Invalidate(Flip)), source: crate::source_evidence::source!([(R1, [PtzOpticsG2, PtzOptics30X], "r1-imagefliphorizontal-command", "01 04 61"), (R14, [PtzOpticsG3], "r14-imagefliphorizontal-command", "01 04 61")]) },
    ImageFlipHorizontalOff => { class: plain(Invalidate(Flip)), source: crate::source_evidence::source!([(R1, [PtzOpticsG2, PtzOptics30X], "r1-imagefliphorizontaloff-command", "01 04 61"), (R14, [PtzOpticsG3], "r14-imagefliphorizontaloff-command", "01 04 61")]) },
    ImageFlipVertical => { class: plain(Invalidate(Flip)), source: crate::source_evidence::source!([(R1, [PtzOpticsG2, PtzOptics30X], "r1-imageflipvertical-command", "01 04 66"), (R14, [PtzOpticsG3], "r14-imageflipvertical-command", "01 04 66"), (R12, [SonyBrc300], "r12-imageflipvertical-command", "01 04 66"), (R21, [NearusBrc300], "r21-imageflipvertical-command", "01 04 66")]) },
    // The combined-flip opcode carries both axes in one parameter, so it
    // establishes a complete horizontal/vertical pair.
    ImageFlipBoth => { class: plain(Set(Flip)), source: crate::source_evidence::source!([(R1, [PtzOpticsG2, PtzOptics30X], "r1-imageflipboth-command", "01 04 A4"), (R14, [PtzOpticsG3], "r14-imageflipboth-command", "01 04 A4")]) },
    ImageFlipCombined => { class: plain(Set(Flip)), source: crate::source_evidence::source!([(R1, [PtzOpticsG2, PtzOptics30X], "r1-imageflipcombined-command", "01 04 A4"), (R14, [PtzOpticsG3], "r14-imageflipcombined-command", "01 04 A4")]) },
    // The `Off` request supplies `false`: a known value, not removal of the
    // cache entry.
    ImageFreezeOn => { class: plain(Set(ImageFreeze)), source: crate::source_evidence::Source::Unsourced },
    ImageFreezeOff => { class: plain(Set(ImageFreeze)), source: crate::source_evidence::Source::Unsourced },
    PictureEffect => { class: plain, source: crate::source_evidence::source!([(R1, [PtzOpticsG2, PtzOptics30X], "r1-pictureeffect-command", "01 04 63"), (R14, [PtzOpticsG3], "r14-pictureeffect-command", "01 04 63")]) },
    // Neutral-density filter.
    NdFilterMode => { class: plain(Set(NdFilterMode)), source: crate::source_evidence::source!([(R7, [SonyFr7], "r7-ndfiltermode-command", "01 7E 04 52")]) },
    // ND direct and step commands physically reposition the filter. The
    // direct target and each finite step have meaningful end states and are
    // therefore targeted on the ND axis.
    NdFilterDirect => { class: targeted(ND_FILTER), source: crate::source_evidence::source!([(R7, [SonyFr7], "r7-ndfilterdirect-command", "01 7E 04 42")]) },
    NdFilterStepUp => { class: targeted(ND_FILTER), source: crate::source_evidence::source!([(R7, [SonyFr7], "r7-ndfilterstepup-command", "01 7E 04 12 02")]) },
    NdFilterStepDown => { class: targeted(ND_FILTER), source: crate::source_evidence::source!([(R7, [SonyFr7], "r7-ndfilterstepdown-command", "01 7E 04 12 03")]) },
    NdFilterAutoOn => { class: plain(Set(AutoNdFilter)), source: crate::source_evidence::source!([(R7, [SonyFr7], "r7-ndfilterautoon-command", "01 7E 04 53 02")]) },
    NdFilterAutoOff => { class: plain(Set(AutoNdFilter)), source: crate::source_evidence::source!([(R7, [SonyFr7], "r7-ndfilterautooff-command", "01 7E 04 53 03")]) },
    // Tally. Red/green tally status has profile-gated inquiries. Brightness
    // and vendor output mode do not, so their applied effects are either
    // deterministic or explicitly invalidated.
    TallyRedOn => { class: plain, source: crate::source_evidence::source!([(R7, [SonyFr7], "r7-tallyredon-command", "01 7E 01 0A 00 02")]) },
    TallyRedOff => { class: plain, source: crate::source_evidence::source!([(R7, [SonyFr7], "r7-tallyredoff-command", "01 7E 01 0A 00 03")]) },
    TallyBrightLow => { class: plain(Set(TallyBrightness)), source: crate::source_evidence::Source::Unsourced },
    TallyBrightHigh => { class: plain(Set(TallyBrightness)), source: crate::source_evidence::Source::Unsourced },
    TallyGreenOn => { class: plain, source: crate::source_evidence::source!([(R7, [SonyFr7], "r7-tallygreenon-command", "01 7E 04 1A 00 02")]) },
    TallyGreenOff => { class: plain, source: crate::source_evidence::source!([(R7, [SonyFr7], "r7-tallygreenoff-command", "01 7E 04 1A 00 03")]) },
    TallyFlash => { class: plain(Invalidate(TallyMode)), source: crate::source_evidence::Source::Unsourced },
    TallyOn => { class: plain(Set(TallyMode)), source: crate::source_evidence::Source::Unsourced },
    TallyOff => { class: plain(Set(TallyMode)), source: crate::source_evidence::Source::Unsourced },
    // Menus.
    MenuDisplay => { class: plain, source: crate::source_evidence::source!([(R7, [SonyFr7], "r7-menudisplay-command", "01 06 06")]) },
    MenuNavigate => { class: applied(PAN_TILT), source: crate::source_evidence::source!([(R1, [PtzOpticsG2, PtzOptics30X], "r1-menunavigate-command", "01 06 01"), (R14, [PtzOpticsG3], "r14-menunavigate-command", "01 06 01"), (R7, [SonyFr7], "r7-menunavigate-command", "01 06 01"), (R11, [SonyBrcH900], "r11-menunavigate-command", "01 06 01"), (R8, [SonyEviH100, GenericVisca], "r8-menunavigate-command", "01 06 01"), (R12, [SonyBrc300, GenericVisca], "r12-menunavigate-command", "01 06 01"), (R21, [NearusBrc300, GenericVisca], "r21-menunavigate-command", "01 06 01")]) },
    MenuSelect => { class: plain, source: crate::source_evidence::source!([(R1, [PtzOpticsG2, PtzOptics30X], "r1-menuselect-command", "01 06 06"), (R14, [PtzOpticsG3], "r14-menuselect-command", "01 06 06")]) },
    MenuCancel => { class: plain, source: crate::source_evidence::source!([(R1, [PtzOpticsG2, PtzOptics30X], "r1-menucancel-command", "01 06 06"), (R14, [PtzOpticsG3], "r14-menucancel-command", "01 06 06")]) },
    DirectMenu => { class: plain, source: crate::source_evidence::source!([(R7, [SonyFr7], "r7-directmenu-command", "01 7E 04 72")]) },
    // Streaming. The multicast `Off` request supplies `false`: a known value,
    // not removal of the cache entry.
    MulticastStreamingOn => { class: plain(Set(MulticastStreaming)), source: crate::source_evidence::source!([(R1, [PtzOpticsG2, PtzOptics30X], "r1-multicaststreamingon-command", "0B 01 23"), (R10, [PtzOpticsG3], "r10-multicaststreamingon-command", "0B 01 23")]) },
    MulticastStreamingOff => { class: plain(Set(MulticastStreaming)), source: crate::source_evidence::source!([(R1, [PtzOpticsG2, PtzOptics30X], "r1-multicaststreamingoff-command", "0B 01 23"), (R10, [PtzOpticsG3], "r10-multicaststreamingoff-command", "0B 01 23")]) },
    NdiQuality => { class: plain(Set(NdiQuality)), source: crate::source_evidence::source!([(R1, [PtzOpticsG2, PtzOptics30X], "r1-ndiquality-command", "0B 01 01"), (R10, [PtzOpticsG3], "r10-ndiquality-command", "0B 01 01")]) },
    UsbAudioOn => { class: plain, source: crate::source_evidence::source!([(R1, [PtzOpticsG2, PtzOptics30X], "r1-usbaudioon-command", "2A 02 A0 04")]) },
    UsbAudioOff => { class: plain, source: crate::source_evidence::source!([(R1, [PtzOpticsG2, PtzOptics30X], "r1-usbaudiooff-command", "2A 02 A0 04")]) },
    // System.
    AddressSet => { class: plain, source: crate::source_evidence::source!([(R8, [SonyEviH100, GenericVisca], "r8-addressset-command", "30 01"), (R12, [SonyBrc300, GenericVisca], "r12-addressset-command", "30 01"), (R21, [NearusBrc300, GenericVisca], "r21-addressset-command", "30 01"), (R11, [SonyBrcH900], "r11-addressset-command", "30 01")]) },
    InterfaceClear => { class: plain, source: crate::source_evidence::source!([(R1, [PtzOpticsG2, PtzOptics30X], "r1-interfaceclear-command", "01 00 01"), (R14, [PtzOpticsG3], "r14-interfaceclear-command", "01 00 01"), (R7, [SonyFr7], "r7-interfaceclear-command", "01 00 01"), (R11, [SonyBrcH900], "r11-interfaceclear-command", "01 00 01"), (R8, [SonyEviH100, GenericVisca], "r8-interfaceclear-command", "01 00 01"), (R12, [SonyBrc300, GenericVisca], "r12-interfaceclear-command", "01 00 01"), (R21, [NearusBrc300, GenericVisca], "r21-interfaceclear-command", "01 00 01")]) },
    CommandCancel => { class: plain, source: crate::source_evidence::source!([(R1, [PtzOpticsG2, PtzOptics30X], "r1-commandcancel-command", "2p"), (R14, [PtzOpticsG3], "r14-commandcancel-command", "2p"), (R7, [SonyFr7], "r7-commandcancel-command", "2p"), (R11, [SonyBrcH900], "r11-commandcancel-command", "2p"), (R8, [SonyEviH100, GenericVisca], "r8-commandcancel-command", "2p"), (R12, [SonyBrc300, GenericVisca], "r12-commandcancel-command", "2p"), (R21, [NearusBrc300, GenericVisca], "r21-commandcancel-command", "2p")]) },
    SettingsSave => { class: plain, source: crate::source_evidence::source!([(R1, [PtzOpticsG2, PtzOptics30X], "r1-settingssave-command", "01 04 A5 10"), (R10, [PtzOpticsG3], "r10-settingssave-command", "01 04 A5 10")]) },
    // Motion-related configuration.
    MotionSyncMode => { class: plain, source: crate::source_evidence::Source::Unsourced },
    MotionSyncPreset => { class: plain, source: crate::source_evidence::Source::Unsourced },
    VariableSpeedMode => { class: plain(Set(VariableSpeedMode)), source: crate::source_evidence::source!([(R7, [SonyFr7], "r7-variablespeedmode-command", "01 06 45")]) },
}

#[cfg(test)]
impl BuiltinCommand {
    /// Returns whether this command is a plain request.
    #[must_use]
    pub(crate) const fn is_plain(self) -> bool {
        matches!(self.classification(), BuiltinRequestClass::Plain { .. })
    }

    /// Returns the operation completion class, if this command is an
    /// operation.
    #[must_use]
    pub(crate) const fn completion(self) -> Option<BuiltinCompletionClass> {
        self.classification().completion()
    }
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::*;
    use std::collections::HashSet;
    #[test]
    fn every_admitted_command_resolves_its_exact_positive_source() {
        let index = crate::source_evidence::checked::Index::parse(include_str!(
            "../../docs/visca_reference.md"
        ))
        .expect("checked positive source table");
        for command in BuiltinCommand::ALL {
            let source = command.source();
            let subject = format!("command.{command:?}");
            for facts in crate::camera::profiles::BUILTIN_PROFILE_FACTS {
                if source.covers_model(Some(facts.id)) {
                    let model = format!("{:?}", facts.id);
                    let result = index.validate(source, &subject, Some(&model), None, None);
                    assert!(result.is_ok(), "{model}: {result:?}");
                }
            }
        }
    }

    #[test]
    fn ledger_is_closed_and_every_row_has_a_classification() {
        let mut seen = HashSet::new();
        for command in BuiltinCommand::ALL {
            assert!(seen.insert(*command), "duplicate ledger row: {command:?}");
            let class = command.classification();
            if command.is_plain() {
                assert!(class.axes().is_none());
            } else {
                assert!(
                    command.completion().is_some(),
                    "operation row has no completion class: {command:?}"
                );
                let axes = class.axes().expect("operation must declare axes");
                if let BuiltinAxisSelection::Exact(axes) = axes {
                    assert_ne!(axes.bits(), 0);
                }
            }
        }
        assert_eq!(seen.len(), BuiltinCommand::ALL.len());
    }

    #[test]
    fn operation_rows_use_only_exact_axes_or_profile_selected_preset_axes() {
        for command in BuiltinCommand::ALL {
            match command.classification() {
                BuiltinRequestClass::Targeted { axes }
                | BuiltinRequestClass::AppliedOnly { axes } => {
                    if let BuiltinAxisSelection::Exact(axes) = axes {
                        assert!(axes.is_single() || axes.axis_count() > 1);
                    }
                }
                BuiltinRequestClass::Plain { .. } => {}
            }
        }
        assert_eq!(
            BuiltinCommand::PresetRecall.classification().axes(),
            Some(BuiltinAxisSelection::ProfilePresetRecall)
        );
    }

    #[test]
    fn physical_actuation_audit_decisions_are_explicit() {
        assert_eq!(
            BuiltinCommand::MenuNavigate.classification(),
            BuiltinRequestClass::AppliedOnly {
                axes: BuiltinAxisSelection::Exact(AffectedAxes::PAN_TILT)
            },
        );
        for command in [
            BuiltinCommand::IrisReset,
            BuiltinCommand::IrisUp,
            BuiltinCommand::IrisDown,
            BuiltinCommand::IrisDirect,
        ] {
            assert_eq!(command.completion(), Some(BuiltinCompletionClass::Targeted));
            assert_eq!(
                command.classification().axes(),
                Some(BuiltinAxisSelection::Exact(AffectedAxes::IRIS))
            );
        }
        for command in [
            BuiltinCommand::NdFilterDirect,
            BuiltinCommand::NdFilterStepUp,
            BuiltinCommand::NdFilterStepDown,
        ] {
            assert_eq!(command.completion(), Some(BuiltinCompletionClass::Targeted));
            assert_eq!(
                command.classification().axes(),
                Some(BuiltinAxisSelection::Exact(AffectedAxes::ND_FILTER))
            );
        }
        for command in [BuiltinCommand::PushAfPress, BuiltinCommand::PushAfRelease] {
            assert_eq!(
                command.completion(),
                Some(BuiltinCompletionClass::AppliedOnly)
            );
            assert_eq!(
                command.classification().axes(),
                Some(BuiltinAxisSelection::Exact(AffectedAxes::FOCUS))
            );
        }
    }

    #[test]
    fn write_only_state_effects_are_closed() {
        assert_eq!(
            BuiltinCommand::PanTiltLimitSet
                .classification()
                .state_effect(),
            Some(AppliedStateEffectRequirement::Set(StateKey::PanTiltLimits))
        );
        assert_eq!(
            BuiltinCommand::PanTiltLimitClear
                .classification()
                .state_effect(),
            Some(AppliedStateEffectRequirement::Clear(
                StateKey::PanTiltLimits
            ))
        );
        assert_eq!(
            BuiltinCommand::ImageFreezeOn
                .classification()
                .state_effect(),
            Some(AppliedStateEffectRequirement::Set(StateKey::ImageFreeze))
        );
        assert_eq!(
            BuiltinCommand::ImageFreezeOff
                .classification()
                .state_effect(),
            Some(AppliedStateEffectRequirement::Set(StateKey::ImageFreeze))
        );
        assert_eq!(
            BuiltinCommand::TallyFlash.classification().state_effect(),
            Some(AppliedStateEffectRequirement::Invalidate(
                StateKey::TallyMode
            ))
        );
        assert_eq!(
            BuiltinCommand::DigitalZoom.classification().state_effect(),
            Some(AppliedStateEffectRequirement::Set(
                StateKey::DigitalZoomMode
            ))
        );

        let represented: HashSet<_> = BuiltinCommand::ALL
            .iter()
            .filter_map(|command| {
                command
                    .classification()
                    .state_effect()
                    .map(|effect| effect.state())
            })
            .collect();
        let listed: HashSet<_> = StateKey::ALL.iter().copied().collect();
        assert_eq!(listed.len(), StateKey::ALL.len());
        for state in StateKey::ALL {
            assert!(
                represented.contains(state),
                "write-only state has no ledger effect: {state:?}"
            );
        }
        assert!(BuiltinCommand::ALL.iter().any(|command| {
            command
                .classification()
                .state_effect()
                .is_some_and(|effect| matches!(effect, AppliedStateEffectRequirement::Set(_)))
        }));
        assert!(BuiltinCommand::ALL.iter().any(|command| {
            command
                .classification()
                .state_effect()
                .is_some_and(|effect| matches!(effect, AppliedStateEffectRequirement::Clear(_)))
        }));
        assert!(BuiltinCommand::ALL.iter().any(|command| {
            command
                .classification()
                .state_effect()
                .is_some_and(|effect| {
                    matches!(effect, AppliedStateEffectRequirement::Invalidate(_))
                })
        }));
    }

    /// Pins the request policy of every built-in row whose timeout class or
    /// retry class is a deliberate decision rather than the generic default for
    /// its domain, so that changing one is a visible, reviewed edit here.
    ///
    /// - Urgent stops and focus triggers complete on the quick deadline but keep
    ///   the movement retry class, because an unacknowledged stop must still be
    ///   retried on movement error evidence.
    /// - Pan/tilt limit edits and focus-mode settings change configuration, not
    ///   position, so they are plain quick/standard writes.
    /// - Targeted iris and ND-filter operations settle through profile-selected
    ///   protocol inquiries, so they use the movement deadline.
    /// - Image-tuning writes (sharpness, gamma, noise reduction, flip) are quick
    ///   configuration writes; the 2D noise-reduction mode control shares that
    ///   policy.
    /// - `CommandCancel` is quick and never retried; preset set/reset/recall use
    ///   the preset deadline and retry class.
    #[test]
    fn deliberate_request_timeout_and_retry_policies_are_pinned() {
        use crate::request::builtin as req;
        use crate::{RetryClass as Retry, TimeoutClass as Timeout};

        fn pin<R: crate::Request>(
            command: BuiltinCommand,
            timeout: Timeout,
            retry: Retry,
            pinned: &mut HashSet<BuiltinCommand>,
        ) {
            assert_eq!(R::TIMEOUT_CLASS, timeout, "{command:?} timeout class");
            assert_eq!(R::RETRY_CLASS, retry, "{command:?} retry class");
            assert!(
                BuiltinCommand::ALL.contains(&command),
                "{command:?} is not a BuiltinCommand row"
            );
            assert!(pinned.insert(command), "{command:?} pinned twice");
        }

        let mut pinned = HashSet::new();
        let p = &mut pinned;

        // Urgent stops and applied-only focus triggers: quick deadline,
        // movement retry class.
        pin::<req::PanTiltStop>(
            BuiltinCommand::PanTiltStop,
            Timeout::Quick,
            Retry::Movement,
            p,
        );
        pin::<req::ZoomStop>(BuiltinCommand::ZoomStop, Timeout::Quick, Retry::Movement, p);
        pin::<req::FocusStop>(
            BuiltinCommand::FocusStop,
            Timeout::Quick,
            Retry::Movement,
            p,
        );
        pin::<req::FocusTrigger>(
            BuiltinCommand::FocusOnePush,
            Timeout::Quick,
            Retry::Movement,
            p,
        );
        pin::<req::FocusTrigger>(
            BuiltinCommand::FocusSnap,
            Timeout::Quick,
            Retry::Movement,
            p,
        );
        pin::<req::PushAfPress>(
            BuiltinCommand::PushAfPress,
            Timeout::Quick,
            Retry::Movement,
            p,
        );
        pin::<req::PushAfRelease>(
            BuiltinCommand::PushAfRelease,
            Timeout::Quick,
            Retry::Movement,
            p,
        );

        // Plain configuration edits: quick deadline, standard retry class.
        pin::<req::PanTiltLimitSet>(
            BuiltinCommand::PanTiltLimitSet,
            Timeout::Quick,
            Retry::Standard,
            p,
        );
        pin::<req::PanTiltLimitClear>(
            BuiltinCommand::PanTiltLimitClear,
            Timeout::Quick,
            Retry::Standard,
            p,
        );
        for command in [
            BuiltinCommand::FocusAuto,
            BuiltinCommand::FocusManual,
            BuiltinCommand::FocusToggle,
        ] {
            pin::<req::FocusModeCommand>(command, Timeout::Quick, Retry::Standard, p);
        }
        for command in [
            BuiltinCommand::SharpnessMode,
            BuiltinCommand::SharpnessReset,
            BuiltinCommand::SharpnessUp,
            BuiltinCommand::SharpnessDown,
            BuiltinCommand::SharpnessDirect,
        ] {
            pin::<crate::command::Sharpness>(command, Timeout::Quick, Retry::Standard, p);
        }
        pin::<crate::command::GammaCommand>(
            BuiltinCommand::Gamma,
            Timeout::Quick,
            Retry::Standard,
            p,
        );
        for command in [
            BuiltinCommand::NoiseReduction2d,
            BuiltinCommand::NoiseReduction2dOff,
        ] {
            pin::<crate::command::NoiseReduction2D>(command, Timeout::Quick, Retry::Standard, p);
        }
        for command in [
            BuiltinCommand::NoiseReduction3d,
            BuiltinCommand::NoiseReduction3dOff,
        ] {
            pin::<crate::command::NoiseReduction3D>(command, Timeout::Quick, Retry::Standard, p);
        }
        pin::<crate::command::NoiseReduction2DModeCommand>(
            BuiltinCommand::NoiseReduction2dMode,
            Timeout::Quick,
            Retry::Standard,
            p,
        );
        for command in [
            BuiltinCommand::ImageFlipBoth,
            BuiltinCommand::ImageFlipCombined,
        ] {
            pin::<crate::command::ImageFlipCombinedCommand>(
                command,
                Timeout::Quick,
                Retry::Standard,
                p,
            );
        }

        // Targeted iris and ND-filter operations: movement deadline and retry.
        pin::<req::IrisReset>(
            BuiltinCommand::IrisReset,
            Timeout::Movement,
            Retry::Movement,
            p,
        );
        pin::<req::IrisUp>(
            BuiltinCommand::IrisUp,
            Timeout::Movement,
            Retry::Movement,
            p,
        );
        pin::<req::IrisDown>(
            BuiltinCommand::IrisDown,
            Timeout::Movement,
            Retry::Movement,
            p,
        );
        pin::<req::IrisDirect>(
            BuiltinCommand::IrisDirect,
            Timeout::Movement,
            Retry::Movement,
            p,
        );
        pin::<req::NdFilterDirect>(
            BuiltinCommand::NdFilterDirect,
            Timeout::Movement,
            Retry::Movement,
            p,
        );
        pin::<req::NdFilterStepUp>(
            BuiltinCommand::NdFilterStepUp,
            Timeout::Movement,
            Retry::Movement,
            p,
        );
        pin::<req::NdFilterStepDown>(
            BuiltinCommand::NdFilterStepDown,
            Timeout::Movement,
            Retry::Movement,
            p,
        );

        // Cancellation is never replayed; presets have their own class.
        pin::<crate::command::system::CommandCancelCommand>(
            BuiltinCommand::CommandCancel,
            Timeout::Quick,
            Retry::Never,
            p,
        );
        pin::<req::PresetSet>(BuiltinCommand::PresetSet, Timeout::Preset, Retry::Preset, p);
        pin::<req::PresetReset>(
            BuiltinCommand::PresetReset,
            Timeout::Preset,
            Retry::Preset,
            p,
        );
        pin::<req::PresetRecall>(
            BuiltinCommand::PresetRecall,
            Timeout::Preset,
            Retry::Preset,
            p,
        );
    }
}
