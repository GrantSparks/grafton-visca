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
//! Operation axes are [`crate::types::AffectedAxes`]. Iris and ND filter are
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
        /// A reviewed STOP, authorized to use the urgent admission lane.
        stop: bool,
    },
}

impl BuiltinRequestClass {
    /// Whether this ledger row is a reviewed typed STOP.
    pub(crate) const fn is_stop(self) -> bool {
        matches!(self, Self::AppliedOnly { stop: true, .. })
    }

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
            Self::Targeted { axes } | Self::AppliedOnly { axes, .. } => Some(axes),
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
            (Self::Targeted { axes: left }, Self::Targeted { axes: right }) => {
                left.same_selection(right)
            }
            (
                Self::AppliedOnly {
                    axes: left,
                    stop: left_stop,
                },
                Self::AppliedOnly {
                    axes: right,
                    stop: right_stop,
                },
            ) => left.same_selection(right) && left_stop == right_stop,
            _ => false,
        }
    }

    /// Returns the closed write-only state requirement, if any.
    #[must_use]
    #[cfg(test)]
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
    /// `request::PlainClass`.
    Plain,
    /// `request::OperationClass<operation::AppliedOnly>`.
    AppliedOnly,
    /// `request::OperationClass<operation::Targeted>`.
    Targeted,
}

/// The ledger kind of a closed request class marker.
pub(crate) trait BuiltinClassMarker {
    /// Kind a built-in request with this class must serve.
    const KIND: BuiltinRequestKind;
}

impl BuiltinClassMarker for crate::request::PlainClass {
    const KIND: BuiltinRequestKind = BuiltinRequestKind::Plain;
}

impl BuiltinClassMarker for crate::request::OperationClass<crate::completion::Targeted> {
    const KIND: BuiltinRequestKind = BuiltinRequestKind::Targeted;
}

impl BuiltinClassMarker for crate::request::OperationClass<crate::completion::AppliedOnly> {
    const KIND: BuiltinRequestKind = BuiltinRequestKind::AppliedOnly;
}

/// A built-in request type's tie to the ledger.
///
/// Everything else in the type's contract (the class check, fixed operation
/// axes, the write-only state effect and the typed capability gate) is read
/// from [`Self::LEDGER_ROW`].
pub(crate) trait BuiltinRequestContract: crate::Request<Class: BuiltinClassMarker> {
    /// A representative ledger row this type serves. Selector arms preserve
    /// its class, axes and STOP authority; each arm retains its own state effect.
    /// The typed-request inventory checks these contracts at compile time.
    const LEDGER_ROW: BuiltinCommand;

    /// Exact value-selected row, exposed only to the literal wire inventory.
    #[cfg(test)]
    fn selected_ledger_row(&self) -> BuiltinCommand {
        Self::LEDGER_ROW
    }

    /// Exact selected protocol row when target context chooses an encoding.
    #[cfg(test)]
    fn selected_ledger_row_for_target(
        &self,
        _target: &crate::TargetRegistration,
    ) -> crate::Result<BuiltinCommand> {
        Ok(self.selected_ledger_row())
    }

    /// Every declared selector arm, generated from the actual rows match.
    const LEDGER_ROWS: &'static [BuiltinCommand] = &[Self::LEDGER_ROW];
    /// Generated from the row's Set/Clear/Invalidate key, or its lack of effect.
    type State;
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
            ..
        } => axes,
        _ => panic!("ledger row has no fixed operation axes"),
    }
}

/// Type-level projection contract emitted beside each ledger row.
pub(crate) struct LedgerRow<const ROW: usize>;

pub(crate) trait LedgerState {
    type State;
}

/// The closed semantic ledger: one row per built-in command, in protocol
/// order.
///
/// `plain`, `plain(Set(<key>))`, `plain(Clear(<key>))`,
/// `plain(Invalidate(<key>))`, `targeted(<axes>)`,
/// `targeted(profile_preset_recall)`, `applied(<axes>)` and
/// `applied_stop(<axes>)` are the only row forms; `<key>` names a [`crate::state_cache::StateKey`] and `<axes>` an
/// [`crate::types::AffectedAxes`] constant. [`BuiltinCommand`], the test-build
/// `BuiltinCommand::ALL` and [`BuiltinCommand::classification`] are all
/// expanded from this one list, so a row cannot be declared without a
/// classification, listed twice, or left out of the inventory.
/// Each sourced row also owns one catalogue-derived `packet` pattern; its
/// `source` references carry provenance keys and model coverage only.
macro_rules! builtin_command_ledger {
    ($( $(#[$meta:meta])* $command:ident => { class: $class:ident $(($($effect:tt)*))?, packet: $packet:expr, source: $source:expr } ),+ $(,)?) => {
        /// Every built-in command request that must be audited before typed
        /// conversion; generated from `builtin_command_ledger!`.
        #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
        pub(crate) enum BuiltinCommand {
            $( $(#[$meta])* $command, )+
        }

        $(
            impl LedgerState for LedgerRow<{ BuiltinCommand::$command as usize }> {
                type State = ledger_state!($class $(($($effect)*))?);
            }
            const _: () = {
                if $source.is_sourced() {
                    let packet: Option<crate::source_evidence::PacketPattern> = $packet;
                    assert!(packet.is_some(), "a sourced command requires its catalogue packet pattern");
                    if let Some(packet) = packet {
                        assert!(!packet.is_empty(), "a sourced command requires a nonempty catalogue packet");
                    }
                }
            };
        )+

        impl BuiltinCommand {
            /// Every ledger row, in protocol order. Production code reaches
            /// rows through the const-validated canonical surface index.
            pub(crate) const ALL: &'static [Self] = &[ $( Self::$command, )+ ];

            /// Source authority for this exact built-in row.
            pub(crate) const fn source(self) -> crate::source_evidence::Source {
                match self { $( Self::$command => $source, )+ }
            }

            /// Catalogue-derived opcode/selector pattern for this row.
            pub(crate) const fn packet(self) -> Option<crate::source_evidence::PacketPattern> {
                match self { $( Self::$command => $packet, )+ }
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

/// Selects a projection type from the same tokens that classify the row.
macro_rules! ledger_state {
    (plain(Set($key:ident))) => {
        crate::request::builtin_contract::Set<crate::state_cache::state_values::$key>
    };
    (plain(Clear($key:ident))) => {
        crate::request::builtin_contract::Clear<crate::state_cache::state_values::$key>
    };
    (plain(Invalidate($key:ident))) => {
        crate::request::builtin_contract::Invalidate<crate::state_cache::state_values::$key>
    };
    ($class:ident $(($($arguments:tt)*))?) => {
        crate::request::builtin_contract::NoState
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
            axes: BuiltinAxisSelection::Exact(AffectedAxes::$axes), stop: false,
        }
    };
    (applied_stop($axes:ident)) => {
        BuiltinRequestClass::AppliedOnly {
            axes: BuiltinAxisSelection::Exact(AffectedAxes::$axes), stop: true,
        }
    };
    ($($row:tt)*) => {
        compile_error!(concat!(
            "unknown ledger class `",
            stringify!($($row)*),
            "`; expected plain, plain(Set(<StateKey>)), plain(Clear(<StateKey>)), ",
            "plain(Invalidate(<StateKey>)), targeted(<AXES>), ",
            "targeted(profile_preset_recall), applied(<AXES>) or applied_stop(<AXES>)"
        ))
    };
}

builtin_command_ledger! {
    // Pan/tilt.
    PanTiltHome => { class: targeted(PAN_TILT), packet: Some(crate::source_evidence::PacketPattern::exact(&crate::command::bytes::constants::pan_tilt::HOME)), source: crate::source_evidence::source!([(R1, [PtzOpticsG2, PtzOptics30x], "r1-pantilthome-command"), (R14, [PtzOpticsG3], "r14-pantilthome-command"), (R7, [SonyFr7], "r7-pantilthome-command"), (R11, [SonyBrcH900], "r11-pantilthome-command"), (R8, [SonyEviH100, GenericVisca], "r8-pantilthome-command"), (R12, [SonyBrc300, GenericVisca], "r12-pantilthome-command"), (R21, [NearusBrc300, GenericVisca], "r21-pantilthome-command")]) },
    PanTiltReset => { class: targeted(PAN_TILT), packet: Some(crate::source_evidence::PacketPattern::exact(&crate::command::bytes::constants::pan_tilt::RESET)), source: crate::source_evidence::source!([(R1, [PtzOpticsG2, PtzOptics30x], "r1-pantiltreset-command"), (R14, [PtzOpticsG3], "r14-pantiltreset-command"), (R7, [SonyFr7], "r7-pantiltreset-command"), (R11, [SonyBrcH900], "r11-pantiltreset-command"), (R8, [SonyEviH100, GenericVisca], "r8-pantiltreset-command"), (R12, [SonyBrc300, GenericVisca], "r12-pantiltreset-command"), (R21, [NearusBrc300, GenericVisca], "r21-pantiltreset-command")]) },
    PanTiltDrive => { class: applied(PAN_TILT), packet: Some(crate::source_evidence::PacketPattern::exact(&crate::command::bytes::constants::pan_tilt::DRIVE).then(&[0, 0], &[0, 0]).select(&[&[3, 1], &[3, 2], &[1, 3], &[2, 3], &[1, 1], &[2, 1], &[1, 2], &[2, 2]])), source: crate::source_evidence::source!([(R1, [PtzOpticsG2, PtzOptics30x], "r1-pantiltdrive-command"), (R14, [PtzOpticsG3], "r14-pantiltdrive-command"), (R7, [SonyFr7], "r7-pantiltdrive-command"), (R11, [SonyBrcH900], "r11-pantiltdrive-command"), (R8, [SonyEviH100, GenericVisca], "r8-pantiltdrive-command"), (R12, [SonyBrc300, GenericVisca], "r12-pantiltdrive-command"), (R21, [NearusBrc300, GenericVisca], "r21-pantiltdrive-command")]) },
    PanTiltStop => { class: applied_stop(PAN_TILT), packet: Some(crate::source_evidence::PacketPattern::exact(&crate::command::bytes::constants::pan_tilt::DRIVE).then(&[0, 0, 0x03, 0x03], &[0, 0, 0xFF, 0xFF])), source: crate::source_evidence::source!([(R1, [PtzOpticsG2, PtzOptics30x], "r1-pantiltstop-command"), (R14, [PtzOpticsG3], "r14-pantiltstop-command"), (R7, [SonyFr7], "r7-pantiltstop-command"), (R11, [SonyBrcH900], "r11-pantiltstop-command"), (R8, [SonyEviH100, GenericVisca], "r8-pantiltstop-command"), (R12, [SonyBrc300, GenericVisca], "r12-pantiltstop-command"), (R21, [NearusBrc300, GenericVisca], "r21-pantiltstop-command")]) },
    PanTiltAbsolute => { class: targeted(PAN_TILT), packet: Some(crate::source_evidence::PacketPattern::exact(&crate::command::bytes::constants::pan_tilt::ABSOLUTE)), source: crate::source_evidence::source!([(R1, [PtzOpticsG2, PtzOptics30x], "r1-pantiltabsolute-command"), (R14, [PtzOpticsG3], "r14-pantiltabsolute-command"), (R7, [SonyFr7], "r7-pantiltabsolute-command"), (R11, [SonyBrcH900], "r11-pantiltabsolute-command"), (R8, [SonyEviH100, GenericVisca], "r8-pantiltabsolute-command"), (R12, [SonyBrc300, GenericVisca], "r12-pantiltabsolute-command"), (R21, [NearusBrc300, GenericVisca], "r21-pantiltabsolute-command")]) },
    PanTiltRelative => { class: targeted(PAN_TILT), packet: Some(crate::source_evidence::PacketPattern::exact(&crate::command::bytes::constants::pan_tilt::RELATIVE)), source: crate::source_evidence::source!([(R1, [PtzOpticsG2, PtzOptics30x], "r1-pantiltrelative-command"), (R14, [PtzOpticsG3], "r14-pantiltrelative-command"), (R7, [SonyFr7], "r7-pantiltrelative-command"), (R11, [SonyBrcH900], "r11-pantiltrelative-command"), (R8, [SonyEviH100, GenericVisca], "r8-pantiltrelative-command"), (R12, [SonyBrc300, GenericVisca], "r12-pantiltrelative-command"), (R21, [NearusBrc300, GenericVisca], "r21-pantiltrelative-command")]) },
    PanTiltLimitSet => { class: plain(Set(PanTiltLimit)), packet: Some(crate::source_evidence::PacketPattern::exact(&crate::command::bytes::constants::pan_tilt::LIMIT_SET)), source: crate::source_evidence::source!([(R1, [PtzOpticsG2, PtzOptics30x], "r1-pantiltlimitset-command"), (R14, [PtzOpticsG3], "r14-pantiltlimitset-command"), (R7, [SonyFr7], "r7-pantiltlimitset-command"), (R11, [SonyBrcH900], "r11-pantiltlimitset-command"), (R8, [SonyEviH100, GenericVisca], "r8-pantiltlimitset-command"), (R12, [SonyBrc300, GenericVisca], "r12-pantiltlimitset-command"), (R21, [NearusBrc300, GenericVisca], "r21-pantiltlimitset-command")]) },
    PanTiltLimitClear => { class: plain(Clear(PanTiltLimit)), packet: Some(crate::source_evidence::PacketPattern::exact(&crate::command::bytes::constants::pan_tilt::LIMIT_CLEAR)), source: crate::source_evidence::source!([(R1, [PtzOpticsG2, PtzOptics30x], "r1-pantiltlimitclear-command"), (R14, [PtzOpticsG3], "r14-pantiltlimitclear-command"), (R7, [SonyFr7], "r7-pantiltlimitclear-command"), (R11, [SonyBrcH900], "r11-pantiltlimitclear-command"), (R8, [SonyEviH100, GenericVisca], "r8-pantiltlimitclear-command"), (R12, [SonyBrc300, GenericVisca], "r12-pantiltlimitclear-command"), (R21, [NearusBrc300, GenericVisca], "r21-pantiltlimitclear-command")]) },
    // Zoom.
    ZoomStop => { class: applied_stop(ZOOM), packet: Some(crate::source_evidence::PacketPattern::exact(&crate::command::bytes::constants::zoom::DRIVE).then(&[0x00], &[0xFF])), source: crate::source_evidence::source!([(R1, [PtzOpticsG2, PtzOptics30x], "r1-zoomstop-command"), (R14, [PtzOpticsG3], "r14-zoomstop-command"), (R7, [SonyFr7], "r7-zoomstop-command"), (R11, [SonyBrcH900], "r11-zoomstop-command"), (R8, [SonyEviH100, GenericVisca], "r8-zoomstop-command"), (R12, [SonyBrc300, GenericVisca], "r12-zoomstop-command"), (R21, [NearusBrc300, GenericVisca], "r21-zoomstop-command")]) },
    ZoomTele => { class: applied(ZOOM), packet: Some(crate::source_evidence::PacketPattern::exact(&crate::command::bytes::constants::zoom::DRIVE).then(&[0x02], &[0xFF])), source: crate::source_evidence::source!([(R1, [PtzOpticsG2, PtzOptics30x], "r1-zoomtele-command"), (R14, [PtzOpticsG3], "r14-zoomtele-command"), (R7, [SonyFr7], "r7-zoomtele-command"), (R11, [SonyBrcH900], "r11-zoomtele-command"), (R8, [SonyEviH100, GenericVisca], "r8-zoomtele-command"), (R12, [SonyBrc300, GenericVisca], "r12-zoomtele-command"), (R21, [NearusBrc300, GenericVisca], "r21-zoomtele-command")]) },
    ZoomWide => { class: applied(ZOOM), packet: Some(crate::source_evidence::PacketPattern::exact(&crate::command::bytes::constants::zoom::DRIVE).then(&[0x03], &[0xFF])), source: crate::source_evidence::source!([(R1, [PtzOpticsG2, PtzOptics30x], "r1-zoomwide-command"), (R14, [PtzOpticsG3], "r14-zoomwide-command"), (R7, [SonyFr7], "r7-zoomwide-command"), (R11, [SonyBrcH900], "r11-zoomwide-command"), (R8, [SonyEviH100, GenericVisca], "r8-zoomwide-command"), (R12, [SonyBrc300, GenericVisca], "r12-zoomwide-command"), (R21, [NearusBrc300, GenericVisca], "r21-zoomwide-command")]) },
    ZoomTeleVariable => { class: applied(ZOOM), packet: Some(crate::source_evidence::PacketPattern::exact(&crate::command::bytes::constants::zoom::DRIVE).then(&[0x20], &[0xF0])), source: crate::source_evidence::source!([(R1, [PtzOpticsG2, PtzOptics30x], "r1-zoomtelevariable-command"), (R14, [PtzOpticsG3], "r14-zoomtelevariable-command"), (R7, [SonyFr7], "r7-zoomtelevariable-command"), (R11, [SonyBrcH900], "r11-zoomtelevariable-command"), (R8, [SonyEviH100, GenericVisca], "r8-zoomtelevariable-command"), (R12, [SonyBrc300, GenericVisca], "r12-zoomtelevariable-command"), (R21, [NearusBrc300, GenericVisca], "r21-zoomtelevariable-command")]) },
    ZoomWideVariable => { class: applied(ZOOM), packet: Some(crate::source_evidence::PacketPattern::exact(&crate::command::bytes::constants::zoom::DRIVE).then(&[0x30], &[0xF0])), source: crate::source_evidence::source!([(R1, [PtzOpticsG2, PtzOptics30x], "r1-zoomwidevariable-command"), (R14, [PtzOpticsG3], "r14-zoomwidevariable-command"), (R7, [SonyFr7], "r7-zoomwidevariable-command"), (R11, [SonyBrcH900], "r11-zoomwidevariable-command"), (R8, [SonyEviH100, GenericVisca], "r8-zoomwidevariable-command"), (R12, [SonyBrc300, GenericVisca], "r12-zoomwidevariable-command"), (R21, [NearusBrc300, GenericVisca], "r21-zoomwidevariable-command")]) },
    ZoomPosition => { class: targeted(ZOOM), packet: Some(crate::source_evidence::PacketPattern::exact(&crate::command::bytes::constants::zoom::DIRECT)), source: crate::source_evidence::source!([(R1, [PtzOpticsG2, PtzOptics30x], "r1-zoomposition-command"), (R14, [PtzOpticsG3], "r14-zoomposition-command"), (R7, [SonyFr7], "r7-zoomposition-command"), (R11, [SonyBrcH900], "r11-zoomposition-command"), (R8, [SonyEviH100, GenericVisca], "r8-zoomposition-command"), (R12, [SonyBrc300, GenericVisca], "r12-zoomposition-command"), (R21, [NearusBrc300, GenericVisca], "r21-zoomposition-command")]) },
    DigitalZoom => { class: plain(Set(ZoomDigitalZoom)), packet: Some(crate::source_evidence::PacketPattern::exact(&crate::command::bytes::constants::zoom::DIGITAL)), source: crate::source_evidence::source!([(R8, [SonyEviH100, GenericVisca], "r8-digitalzoom-command"), (R12, [SonyBrc300, GenericVisca], "r12-digitalzoom-command"), (R21, [NearusBrc300, GenericVisca], "r21-digitalzoom-command")]) },
    // Focus.
    FocusStop => { class: applied_stop(FOCUS), packet: Some(crate::source_evidence::PacketPattern::exact(&crate::command::bytes::constants::focus::DRIVE).then(&[0x00], &[0xFF])), source: crate::source_evidence::source!([(R1, [PtzOpticsG2, PtzOptics30x], "r1-focusstop-command"), (R14, [PtzOpticsG3], "r14-focusstop-command"), (R7, [SonyFr7], "r7-focusstop-command"), (R11, [SonyBrcH900], "r11-focusstop-command"), (R8, [SonyEviH100, GenericVisca], "r8-focusstop-command"), (R12, [SonyBrc300, GenericVisca], "r12-focusstop-command"), (R21, [NearusBrc300, GenericVisca], "r21-focusstop-command")]) },
    FocusFar => { class: applied(FOCUS), packet: Some(crate::source_evidence::PacketPattern::exact(&crate::command::bytes::constants::focus::DRIVE).then(&[0x02], &[0xFF])), source: crate::source_evidence::source!([(R1, [PtzOpticsG2, PtzOptics30x], "r1-focusfar-command"), (R14, [PtzOpticsG3], "r14-focusfar-command"), (R7, [SonyFr7], "r7-focusfar-command"), (R11, [SonyBrcH900], "r11-focusfar-command"), (R8, [SonyEviH100, GenericVisca], "r8-focusfar-command"), (R12, [SonyBrc300, GenericVisca], "r12-focusfar-command"), (R21, [NearusBrc300, GenericVisca], "r21-focusfar-command")]) },
    FocusNear => { class: applied(FOCUS), packet: Some(crate::source_evidence::PacketPattern::exact(&crate::command::bytes::constants::focus::DRIVE).then(&[0x03], &[0xFF])), source: crate::source_evidence::source!([(R1, [PtzOpticsG2, PtzOptics30x], "r1-focusnear-command"), (R14, [PtzOpticsG3], "r14-focusnear-command"), (R7, [SonyFr7], "r7-focusnear-command"), (R11, [SonyBrcH900], "r11-focusnear-command"), (R8, [SonyEviH100, GenericVisca], "r8-focusnear-command"), (R12, [SonyBrc300, GenericVisca], "r12-focusnear-command"), (R21, [NearusBrc300, GenericVisca], "r21-focusnear-command")]) },
    FocusFarVariable => { class: applied(FOCUS), packet: Some(crate::source_evidence::PacketPattern::exact(&crate::command::bytes::constants::focus::DRIVE).then(&[0x20], &[0xF0])), source: crate::source_evidence::source!([(R1, [PtzOpticsG2, PtzOptics30x], "r1-focusfarvariable-command"), (R14, [PtzOpticsG3], "r14-focusfarvariable-command"), (R7, [SonyFr7], "r7-focusfarvariable-command"), (R11, [SonyBrcH900], "r11-focusfarvariable-command"), (R8, [SonyEviH100, GenericVisca], "r8-focusfarvariable-command"), (R12, [SonyBrc300, GenericVisca], "r12-focusfarvariable-command"), (R21, [NearusBrc300, GenericVisca], "r21-focusfarvariable-command")]) },
    FocusNearVariable => { class: applied(FOCUS), packet: Some(crate::source_evidence::PacketPattern::exact(&crate::command::bytes::constants::focus::DRIVE).then(&[0x30], &[0xF0])), source: crate::source_evidence::source!([(R1, [PtzOpticsG2, PtzOptics30x], "r1-focusnearvariable-command"), (R14, [PtzOpticsG3], "r14-focusnearvariable-command"), (R7, [SonyFr7], "r7-focusnearvariable-command"), (R11, [SonyBrcH900], "r11-focusnearvariable-command"), (R8, [SonyEviH100, GenericVisca], "r8-focusnearvariable-command"), (R12, [SonyBrc300, GenericVisca], "r12-focusnearvariable-command"), (R21, [NearusBrc300, GenericVisca], "r21-focusnearvariable-command")]) },
    FocusPosition => { class: targeted(FOCUS), packet: Some(crate::source_evidence::PacketPattern::exact(&crate::command::bytes::constants::focus::DIRECT)), source: crate::source_evidence::source!([(R1, [PtzOpticsG2, PtzOptics30x], "r1-focusposition-command"), (R14, [PtzOpticsG3], "r14-focusposition-command"), (R7, [SonyFr7], "r7-focusposition-command"), (R11, [SonyBrcH900], "r11-focusposition-command"), (R8, [SonyEviH100, GenericVisca], "r8-focusposition-command"), (R12, [SonyBrc300, GenericVisca], "r12-focusposition-command"), (R21, [NearusBrc300, GenericVisca], "r21-focusposition-command")]) },
    FocusAuto => { class: plain, packet: Some(crate::source_evidence::PacketPattern::exact(&crate::command::bytes::constants::focus::MODE).then(&[0x02], &[0xFF])), source: crate::source_evidence::source!([(R1, [PtzOpticsG2, PtzOptics30x], "r1-focusauto-command"), (R14, [PtzOpticsG3], "r14-focusauto-command"), (R7, [SonyFr7], "r7-focusauto-command"), (R11, [SonyBrcH900], "r11-focusauto-command"), (R8, [SonyEviH100, GenericVisca], "r8-focusauto-command"), (R12, [SonyBrc300, GenericVisca], "r12-focusauto-command"), (R21, [NearusBrc300, GenericVisca], "r21-focusauto-command")]) },
    FocusManual => { class: plain, packet: Some(crate::source_evidence::PacketPattern::exact(&crate::command::bytes::constants::focus::MODE).then(&[0x03], &[0xFF])), source: crate::source_evidence::source!([(R1, [PtzOpticsG2, PtzOptics30x], "r1-focusmanual-command"), (R14, [PtzOpticsG3], "r14-focusmanual-command"), (R7, [SonyFr7], "r7-focusmanual-command"), (R11, [SonyBrcH900], "r11-focusmanual-command"), (R8, [SonyEviH100, GenericVisca], "r8-focusmanual-command"), (R12, [SonyBrc300, GenericVisca], "r12-focusmanual-command"), (R21, [NearusBrc300, GenericVisca], "r21-focusmanual-command")]) },
    FocusOnePush => { class: applied(FOCUS), packet: Some(crate::source_evidence::PacketPattern::exact(&crate::command::bytes::constants::focus::ONE_PUSH).then(&[0x01], &[0xFF])), source: crate::source_evidence::source!([(R8, [SonyEviH100, GenericVisca], "r8-focusonepush-command"), (R12, [SonyBrc300, GenericVisca], "r12-focusonepush-command"), (R21, [NearusBrc300, GenericVisca], "r21-focusonepush-command"), (R11, [SonyBrcH900], "r11-focusonepush-command")]) },
    FocusInfinity => { class: targeted(FOCUS), packet: Some(crate::source_evidence::PacketPattern::exact(&crate::command::bytes::constants::focus::ONE_PUSH).then(&[0x02], &[0xFF])), source: crate::source_evidence::source!([(R8, [SonyEviH100, GenericVisca], "r8-focusinfinity-command"), (R12, [SonyBrc300, GenericVisca], "r12-focusinfinity-command"), (R21, [NearusBrc300, GenericVisca], "r21-focusinfinity-command"), (R11, [SonyBrcH900], "r11-focusinfinity-command")]) },
    FocusToggle => { class: plain, packet: Some(crate::source_evidence::PacketPattern::exact(&crate::command::bytes::constants::focus::MODE).then(&[0x10], &[0xFF])), source: crate::source_evidence::source!([(R1, [PtzOpticsG2, PtzOptics30x], "r1-focustoggle-command"), (R14, [PtzOpticsG3], "r14-focustoggle-command"), (R7, [SonyFr7], "r7-focustoggle-command"), (R11, [SonyBrcH900], "r11-focustoggle-command"), (R8, [SonyEviH100, GenericVisca], "r8-focustoggle-command"), (R12, [SonyBrc300, GenericVisca], "r12-focustoggle-command"), (R21, [NearusBrc300, GenericVisca], "r21-focustoggle-command")]) },
    FocusSnap => { class: applied(FOCUS), packet: None, source: crate::source_evidence::Source::Unsourced },
    FocusZone => { class: plain, packet: Some(crate::source_evidence::PacketPattern::exact(&crate::command::bytes::constants::focus::ZONE)), source: crate::source_evidence::source!([(R20, [PtzOpticsG2, PtzOpticsG3, PtzOptics30x], "r20-focuszone-command")]) },
    FocusAutoSensitivity => { class: plain, packet: Some(crate::source_evidence::PacketPattern::exact(&crate::command::bytes::constants::focus::AF_SENSITIVITY)), source: crate::source_evidence::source!([(R1, [PtzOpticsG2, PtzOptics30x], "r1-focusautosensitivity-command"), (R14, [PtzOpticsG3], "r14-focusautosensitivity-command"), (R8, [SonyEviH100], "r8-focusautosensitivity-command")]) },
    FocusNearLimit => { class: plain, packet: Some(crate::source_evidence::PacketPattern::exact(&crate::command::bytes::constants::focus::NEAR_LIMIT)), source: crate::source_evidence::source!([(R8, [SonyEviH100], "r8-focusnearlimit-command")]) },
    FocusLock => { class: plain(Set(FocusLock)), packet: Some(crate::source_evidence::PacketPattern::exact(&crate::command::bytes::constants::focus::LOCK)), source: crate::source_evidence::source!([(R1, [PtzOpticsG2, PtzOptics30x], "r1-focuslock-command"), (R14, [PtzOpticsG3], "r14-focuslock-command")]) },
    PushAfPress => { class: applied(FOCUS), packet: Some(crate::source_evidence::PacketPattern::exact(&crate::command::bytes::constants::focus::PUSH_AF).then(&[0x01], &[0xFF])), source: crate::source_evidence::source!([(R7, [SonyFr7], "r7-pushafpress-command")]) },
    PushAfRelease => { class: applied(FOCUS), packet: Some(crate::source_evidence::PacketPattern::exact(&crate::command::bytes::constants::focus::PUSH_AF).then(&[0x00], &[0xFF])), source: crate::source_evidence::source!([(R7, [SonyFr7], "r7-pushafrelease-command")]) },
    // Presets.
    PresetRecall => { class: targeted(profile_preset_recall), packet: Some(crate::source_evidence::PacketPattern::exact(&crate::command::bytes::constants::preset::MEMORY).then(&[0x02], &[0xFF])), source: crate::source_evidence::source!([(R1, [PtzOpticsG2, PtzOptics30x], "r1-presetrecall-command"), (R14, [PtzOpticsG3], "r14-presetrecall-command"), (R7, [SonyFr7], "r7-presetrecall-command"), (R11, [SonyBrcH900], "r11-presetrecall-command"), (R8, [SonyEviH100, GenericVisca], "r8-presetrecall-command"), (R12, [SonyBrc300, GenericVisca], "r12-presetrecall-command"), (R21, [NearusBrc300, GenericVisca], "r21-presetrecall-command")]) },
    PresetRecallSpeed => { class: plain(Set(PresetsRecallSpeed)), packet: Some(crate::source_evidence::PacketPattern::exact(&crate::command::bytes::constants::preset::RECALL_SPEED).then(&[0, 0xFF], &[0, 0xFF])), source: crate::source_evidence::source!([(R1, [PtzOpticsG2, PtzOptics30x], "r1-presetrecallspeed-command"), (R10, [PtzOpticsG3], "r10-presetrecallspeed-command")]) },
    PresetSet => { class: plain, packet: Some(crate::source_evidence::PacketPattern::exact(&crate::command::bytes::constants::preset::MEMORY).then(&[0x01], &[0xFF])), source: crate::source_evidence::source!([(R1, [PtzOpticsG2, PtzOptics30x], "r1-presetset-command"), (R14, [PtzOpticsG3], "r14-presetset-command"), (R7, [SonyFr7], "r7-presetset-command"), (R8, [SonyEviH100, GenericVisca], "r8-presetset-command"), (R12, [SonyBrc300, GenericVisca], "r12-presetset-command"), (R21, [NearusBrc300, GenericVisca], "r21-presetset-command")]) },
    PresetReset => { class: plain, packet: Some(crate::source_evidence::PacketPattern::exact(&crate::command::bytes::constants::preset::MEMORY).then(&[0x00], &[0xFF])), source: crate::source_evidence::source!([(R1, [PtzOpticsG2, PtzOptics30x], "r1-presetreset-command"), (R14, [PtzOpticsG3], "r14-presetreset-command"), (R7, [SonyFr7], "r7-presetreset-command"), (R8, [SonyEviH100, GenericVisca], "r8-presetreset-command"), (R12, [SonyBrc300, GenericVisca], "r12-presetreset-command"), (R21, [NearusBrc300, GenericVisca], "r21-presetreset-command")]) },
    // Power.
    PowerOn => { class: plain, packet: Some(crate::source_evidence::PacketPattern::exact(&crate::command::bytes::constants::power::ON)), source: crate::source_evidence::source!([(R1, [PtzOpticsG2, PtzOptics30x], "r1-poweron-command"), (R14, [PtzOpticsG3], "r14-poweron-command"), (R7, [SonyFr7], "r7-poweron-command"), (R11, [SonyBrcH900], "r11-poweron-command"), (R8, [SonyEviH100, GenericVisca], "r8-poweron-command"), (R12, [SonyBrc300, GenericVisca], "r12-poweron-command"), (R21, [NearusBrc300, GenericVisca], "r21-poweron-command")]) },
    PowerStandby => { class: plain, packet: Some(crate::source_evidence::PacketPattern::exact(&crate::command::bytes::constants::power::STANDBY)), source: crate::source_evidence::source!([(R1, [PtzOpticsG2, PtzOptics30x], "r1-powerstandby-command"), (R14, [PtzOpticsG3], "r14-powerstandby-command"), (R7, [SonyFr7], "r7-powerstandby-command"), (R11, [SonyBrcH900], "r11-powerstandby-command"), (R8, [SonyEviH100, GenericVisca], "r8-powerstandby-command"), (R12, [SonyBrc300, GenericVisca], "r12-powerstandby-command"), (R21, [NearusBrc300, GenericVisca], "r21-powerstandby-command")]) },
    // Exposure and iris.
    ExposureMode => { class: plain, packet: Some(crate::source_evidence::PacketPattern::exact(&crate::command::bytes::constants::exposure::MODE)), source: crate::source_evidence::source!([(R1, [PtzOpticsG2, PtzOptics30x], "r1-exposuremode-command"), (R14, [PtzOpticsG3], "r14-exposuremode-command"), (R11, [SonyBrcH900], "r11-exposuremode-command"), (R8, [SonyEviH100, GenericVisca], "r8-exposuremode-command"), (R12, [SonyBrc300, GenericVisca], "r12-exposuremode-command"), (R21, [NearusBrc300, GenericVisca], "r21-exposuremode-command")]) },
    ExposureCompensationOn => { class: plain, packet: Some(crate::source_evidence::PacketPattern::exact(&crate::command::bytes::constants::exposure::COMPENSATION_SWITCH).then(&[0x02], &[0xFF])), source: crate::source_evidence::source!([(R1, [PtzOpticsG2, PtzOptics30x], "r1-exposurecompensationon-command"), (R14, [PtzOpticsG3], "r14-exposurecompensationon-command"), (R8, [SonyEviH100, GenericVisca], "r8-exposurecompensationon-command"), (R12, [SonyBrc300, GenericVisca], "r12-exposurecompensationon-command"), (R21, [NearusBrc300, GenericVisca], "r21-exposurecompensationon-command")]) },
    ExposureCompensationOff => { class: plain, packet: Some(crate::source_evidence::PacketPattern::exact(&crate::command::bytes::constants::exposure::COMPENSATION_SWITCH).then(&[0x03], &[0xFF])), source: crate::source_evidence::source!([(R1, [PtzOpticsG2, PtzOptics30x], "r1-exposurecompensationoff-command"), (R14, [PtzOpticsG3], "r14-exposurecompensationoff-command"), (R8, [SonyEviH100, GenericVisca], "r8-exposurecompensationoff-command"), (R12, [SonyBrc300, GenericVisca], "r12-exposurecompensationoff-command"), (R21, [NearusBrc300, GenericVisca], "r21-exposurecompensationoff-command")]) },
    ExposureCompensationReset => { class: plain, packet: Some(crate::source_evidence::PacketPattern::exact(&crate::command::bytes::constants::exposure::COMPENSATION_STEP).then(&[0x00], &[0xFF])), source: crate::source_evidence::source!([(R1, [PtzOpticsG2, PtzOptics30x], "r1-exposurecompensationreset-command"), (R14, [PtzOpticsG3], "r14-exposurecompensationreset-command"), (R8, [SonyEviH100, GenericVisca], "r8-exposurecompensationreset-command"), (R12, [SonyBrc300, GenericVisca], "r12-exposurecompensationreset-command"), (R21, [NearusBrc300, GenericVisca], "r21-exposurecompensationreset-command")]) },
    ExposureCompensationUp => { class: plain, packet: Some(crate::source_evidence::PacketPattern::exact(&crate::command::bytes::constants::exposure::COMPENSATION_STEP).then(&[0x02], &[0xFF])), source: crate::source_evidence::source!([(R1, [PtzOpticsG2, PtzOptics30x], "r1-exposurecompensationup-command"), (R14, [PtzOpticsG3], "r14-exposurecompensationup-command"), (R8, [SonyEviH100, GenericVisca], "r8-exposurecompensationup-command"), (R12, [SonyBrc300, GenericVisca], "r12-exposurecompensationup-command"), (R21, [NearusBrc300, GenericVisca], "r21-exposurecompensationup-command")]) },
    ExposureCompensationDown => { class: plain, packet: Some(crate::source_evidence::PacketPattern::exact(&crate::command::bytes::constants::exposure::COMPENSATION_STEP).then(&[0x03], &[0xFF])), source: crate::source_evidence::source!([(R1, [PtzOpticsG2, PtzOptics30x], "r1-exposurecompensationdown-command"), (R14, [PtzOpticsG3], "r14-exposurecompensationdown-command"), (R8, [SonyEviH100, GenericVisca], "r8-exposurecompensationdown-command"), (R12, [SonyBrc300, GenericVisca], "r12-exposurecompensationdown-command"), (R21, [NearusBrc300, GenericVisca], "r21-exposurecompensationdown-command")]) },
    ExposureCompensationDirect => { class: plain, packet: Some(crate::source_evidence::PacketPattern::exact(&crate::command::bytes::constants::exposure::COMPENSATION_DIRECT).prefix_len(3)), source: crate::source_evidence::source!([(R1, [PtzOpticsG2, PtzOptics30x], "r1-exposurecompensationdirect-command"), (R14, [PtzOpticsG3], "r14-exposurecompensationdirect-command"), (R8, [SonyEviH100, GenericVisca], "r8-exposurecompensationdirect-command"), (R12, [SonyBrc300, GenericVisca], "r12-exposurecompensationdirect-command"), (R21, [NearusBrc300, GenericVisca], "r21-exposurecompensationdirect-command")]) },
    DynamicRange => { class: plain, packet: Some(crate::source_evidence::PacketPattern::exact(&crate::command::bytes::constants::exposure::DYNAMIC_RANGE).prefix_len(3)), source: crate::source_evidence::source!([(R1, [PtzOpticsG2, PtzOptics30x], "r1-dynamicrange-command"), (R14, [PtzOpticsG3], "r14-dynamicrange-command")]) },
    // Iris commands are finite physical aperture movements.  They have a
    // meaningful end state and an exact Iris inquiry, so every
    // reset/step/direct form is targeted rather than applied-only.
    IrisReset => { class: targeted(IRIS), packet: Some(crate::source_evidence::PacketPattern::exact(&crate::command::bytes::constants::exposure::IRIS_STEP).then(&[0x00], &[0xFF])), source: crate::source_evidence::source!([(R1, [PtzOpticsG2, PtzOptics30x], "r1-irisreset-command"), (R14, [PtzOpticsG3], "r14-irisreset-command"), (R11, [SonyBrcH900], "r11-irisreset-command"), (R8, [SonyEviH100, GenericVisca], "r8-irisreset-command"), (R12, [SonyBrc300, GenericVisca], "r12-irisreset-command"), (R21, [NearusBrc300, GenericVisca], "r21-irisreset-command")]) },
    IrisUp => { class: targeted(IRIS), packet: Some(crate::source_evidence::PacketPattern::exact(&crate::command::bytes::constants::exposure::IRIS_STEP).then(&[0x02], &[0xFF])), source: crate::source_evidence::source!([(R1, [PtzOpticsG2, PtzOptics30x], "r1-irisup-command"), (R14, [PtzOpticsG3], "r14-irisup-command"), (R11, [SonyBrcH900], "r11-irisup-command"), (R8, [SonyEviH100, GenericVisca], "r8-irisup-command"), (R12, [SonyBrc300, GenericVisca], "r12-irisup-command"), (R21, [NearusBrc300, GenericVisca], "r21-irisup-command")]) },
    IrisDown => { class: targeted(IRIS), packet: Some(crate::source_evidence::PacketPattern::exact(&crate::command::bytes::constants::exposure::IRIS_STEP).then(&[0x03], &[0xFF])), source: crate::source_evidence::source!([(R1, [PtzOpticsG2, PtzOptics30x], "r1-irisdown-command"), (R14, [PtzOpticsG3], "r14-irisdown-command"), (R11, [SonyBrcH900], "r11-irisdown-command"), (R8, [SonyEviH100, GenericVisca], "r8-irisdown-command"), (R12, [SonyBrc300, GenericVisca], "r12-irisdown-command"), (R21, [NearusBrc300, GenericVisca], "r21-irisdown-command")]) },
    IrisDirect => { class: targeted(IRIS), packet: Some(crate::source_evidence::PacketPattern::exact(&crate::command::bytes::constants::exposure::IRIS_DIRECT).prefix_len(3)), source: crate::source_evidence::source!([(R1, [PtzOpticsG2, PtzOptics30x], "r1-irisdirect-command"), (R14, [PtzOpticsG3], "r14-irisdirect-command"), (R11, [SonyBrcH900], "r11-irisdirect-command"), (R8, [SonyEviH100, GenericVisca], "r8-irisdirect-command"), (R12, [SonyBrc300, GenericVisca], "r12-irisdirect-command"), (R21, [NearusBrc300, GenericVisca], "r21-irisdirect-command")]) },
    ShutterReset => { class: plain, packet: Some(crate::source_evidence::PacketPattern::exact(&crate::command::bytes::constants::exposure::SHUTTER_STEP).then(&[0x00], &[0xFF])), source: crate::source_evidence::source!([(R1, [PtzOpticsG2, PtzOptics30x], "r1-shutterreset-command"), (R14, [PtzOpticsG3], "r14-shutterreset-command"), (R11, [SonyBrcH900], "r11-shutterreset-command"), (R8, [SonyEviH100, GenericVisca], "r8-shutterreset-command"), (R12, [SonyBrc300, GenericVisca], "r12-shutterreset-command"), (R21, [NearusBrc300, GenericVisca], "r21-shutterreset-command")]) },
    ShutterUp => { class: plain, packet: Some(crate::source_evidence::PacketPattern::exact(&crate::command::bytes::constants::exposure::SHUTTER_STEP).then(&[0x02], &[0xFF])), source: crate::source_evidence::source!([(R1, [PtzOpticsG2, PtzOptics30x], "r1-shutterup-command"), (R14, [PtzOpticsG3], "r14-shutterup-command"), (R11, [SonyBrcH900], "r11-shutterup-command"), (R8, [SonyEviH100, GenericVisca], "r8-shutterup-command"), (R12, [SonyBrc300, GenericVisca], "r12-shutterup-command"), (R21, [NearusBrc300, GenericVisca], "r21-shutterup-command")]) },
    ShutterDown => { class: plain, packet: Some(crate::source_evidence::PacketPattern::exact(&crate::command::bytes::constants::exposure::SHUTTER_STEP).then(&[0x03], &[0xFF])), source: crate::source_evidence::source!([(R1, [PtzOpticsG2, PtzOptics30x], "r1-shutterdown-command"), (R14, [PtzOpticsG3], "r14-shutterdown-command"), (R11, [SonyBrcH900], "r11-shutterdown-command"), (R8, [SonyEviH100, GenericVisca], "r8-shutterdown-command"), (R12, [SonyBrc300, GenericVisca], "r12-shutterdown-command"), (R21, [NearusBrc300, GenericVisca], "r21-shutterdown-command")]) },
    ShutterDirect => { class: plain, packet: Some(crate::source_evidence::PacketPattern::exact(&crate::command::bytes::constants::exposure::SHUTTER_DIRECT).prefix_len(3)), source: crate::source_evidence::source!([(R1, [PtzOpticsG2, PtzOptics30x], "r1-shutterdirect-command"), (R14, [PtzOpticsG3], "r14-shutterdirect-command"), (R11, [SonyBrcH900], "r11-shutterdirect-command"), (R8, [SonyEviH100, GenericVisca], "r8-shutterdirect-command"), (R12, [SonyBrc300, GenericVisca], "r12-shutterdirect-command"), (R21, [NearusBrc300, GenericVisca], "r21-shutterdirect-command")]) },
    BrightnessReset => { class: plain, packet: Some(crate::source_evidence::PacketPattern::exact(&crate::command::bytes::constants::exposure::BRIGHT_STEP).then(&[0x00], &[0xFF])), source: crate::source_evidence::source!([(R1, [PtzOpticsG2, PtzOptics30x], "r1-brightnessreset-command"), (R14, [PtzOpticsG3], "r14-brightnessreset-command"), (R8, [SonyEviH100, GenericVisca], "r8-brightnessreset-command"), (R12, [SonyBrc300, GenericVisca], "r12-brightnessreset-command"), (R21, [NearusBrc300, GenericVisca], "r21-brightnessreset-command")]) },
    BrightnessUp => { class: plain, packet: Some(crate::source_evidence::PacketPattern::exact(&crate::command::bytes::constants::exposure::BRIGHT_STEP).then(&[0x02], &[0xFF])), source: crate::source_evidence::source!([(R1, [PtzOpticsG2, PtzOptics30x], "r1-brightnessup-command"), (R14, [PtzOpticsG3], "r14-brightnessup-command"), (R8, [SonyEviH100, GenericVisca], "r8-brightnessup-command"), (R12, [SonyBrc300, GenericVisca], "r12-brightnessup-command"), (R21, [NearusBrc300, GenericVisca], "r21-brightnessup-command")]) },
    BrightnessDown => { class: plain, packet: Some(crate::source_evidence::PacketPattern::exact(&crate::command::bytes::constants::exposure::BRIGHT_STEP).then(&[0x03], &[0xFF])), source: crate::source_evidence::source!([(R1, [PtzOpticsG2, PtzOptics30x], "r1-brightnessdown-command"), (R14, [PtzOpticsG3], "r14-brightnessdown-command"), (R8, [SonyEviH100, GenericVisca], "r8-brightnessdown-command"), (R12, [SonyBrc300, GenericVisca], "r12-brightnessdown-command"), (R21, [NearusBrc300, GenericVisca], "r21-brightnessdown-command")]) },
    BrightnessSet => { class: plain, packet: Some(crate::source_evidence::PacketPattern::exact(&crate::command::bytes::constants::exposure::BRIGHT_DIRECT).prefix_len(3)), source: crate::source_evidence::source!([(R1, [PtzOpticsG2, PtzOptics30x], "r1-brightnessset-command"), (R14, [PtzOpticsG3], "r14-brightnessset-command"), (R8, [SonyEviH100, GenericVisca], "r8-brightnessset-command"), (R12, [SonyBrc300, GenericVisca], "r12-brightnessset-command"), (R21, [NearusBrc300, GenericVisca], "r21-brightnessset-command")]) },
    AntiFlicker => { class: plain, packet: Some(crate::source_evidence::PacketPattern::exact(&crate::command::bytes::constants::exposure::ANTI_FLICKER)), source: crate::source_evidence::source!([(R1, [PtzOpticsG2, PtzOptics30x], "r1-antiflicker-command"), (R10, [PtzOpticsG3], "r10-antiflicker-command")]) },
    SpotlightOn => { class: plain(Set(ExposureSpotlight)), packet: Some(crate::source_evidence::PacketPattern::exact(&crate::command::bytes::constants::exposure::SPOTLIGHT_ON)), source: crate::source_evidence::source!([(R7, [SonyFr7], "r7-spotlighton-command"), (R11, [SonyBrcH900], "r11-spotlighton-command")]) },
    SpotlightOff => { class: plain(Set(ExposureSpotlight)), packet: Some(crate::source_evidence::PacketPattern::exact(&crate::command::bytes::constants::exposure::SPOTLIGHT_OFF)), source: crate::source_evidence::source!([(R7, [SonyFr7], "r7-spotlightoff-command")]) },
    AutoSlowShutterOn => { class: plain(Set(ExposureAutoSlowShutter)), packet: Some(crate::source_evidence::PacketPattern::exact(&crate::command::bytes::constants::exposure::AUTO_SLOW_SHUTTER_ON)), source: crate::source_evidence::source!([(R8, [SonyEviH100, GenericVisca], "r8-autoslowshutteron-command"), (R12, [SonyBrc300, GenericVisca], "r12-autoslowshutteron-command"), (R21, [NearusBrc300, GenericVisca], "r21-autoslowshutteron-command")]) },
    AutoSlowShutterOff => { class: plain(Set(ExposureAutoSlowShutter)), packet: Some(crate::source_evidence::PacketPattern::exact(&crate::command::bytes::constants::exposure::AUTO_SLOW_SHUTTER_OFF)), source: crate::source_evidence::source!([(R8, [SonyEviH100, GenericVisca], "r8-autoslowshutteroff-command"), (R12, [SonyBrc300, GenericVisca], "r12-autoslowshutteroff-command"), (R21, [NearusBrc300, GenericVisca], "r21-autoslowshutteroff-command")]) },
    // Gain.
    GainReset => { class: plain, packet: Some(crate::source_evidence::PacketPattern::exact(&crate::command::bytes::constants::gain::STEP).then(&[0x00], &[0xFF])), source: crate::source_evidence::source!([(R1, [PtzOpticsG2, PtzOptics30x], "r1-gainreset-command"), (R14, [PtzOpticsG3], "r14-gainreset-command"), (R11, [SonyBrcH900], "r11-gainreset-command"), (R8, [SonyEviH100, GenericVisca], "r8-gainreset-command"), (R12, [SonyBrc300, GenericVisca], "r12-gainreset-command"), (R21, [NearusBrc300, GenericVisca], "r21-gainreset-command")]) },
    GainUp => { class: plain, packet: Some(crate::source_evidence::PacketPattern::exact(&crate::command::bytes::constants::gain::STEP).then(&[0x02], &[0xFF])), source: crate::source_evidence::source!([(R1, [PtzOpticsG2, PtzOptics30x], "r1-gainup-command"), (R14, [PtzOpticsG3], "r14-gainup-command"), (R11, [SonyBrcH900], "r11-gainup-command"), (R8, [SonyEviH100, GenericVisca], "r8-gainup-command"), (R12, [SonyBrc300, GenericVisca], "r12-gainup-command"), (R21, [NearusBrc300, GenericVisca], "r21-gainup-command")]) },
    GainDown => { class: plain, packet: Some(crate::source_evidence::PacketPattern::exact(&crate::command::bytes::constants::gain::STEP).then(&[0x03], &[0xFF])), source: crate::source_evidence::source!([(R1, [PtzOpticsG2, PtzOptics30x], "r1-gaindown-command"), (R14, [PtzOpticsG3], "r14-gaindown-command"), (R11, [SonyBrcH900], "r11-gaindown-command"), (R8, [SonyEviH100, GenericVisca], "r8-gaindown-command"), (R12, [SonyBrc300, GenericVisca], "r12-gaindown-command"), (R21, [NearusBrc300, GenericVisca], "r21-gaindown-command")]) },
    GainDirect => { class: plain, packet: Some(crate::source_evidence::PacketPattern::exact(&crate::command::bytes::constants::gain::DIRECT).prefix_len(3)), source: crate::source_evidence::source!([(R1, [PtzOpticsG2, PtzOptics30x], "r1-gaindirect-command"), (R14, [PtzOpticsG3], "r14-gaindirect-command"), (R11, [SonyBrcH900], "r11-gaindirect-command"), (R8, [SonyEviH100, GenericVisca], "r8-gaindirect-command"), (R12, [SonyBrc300, GenericVisca], "r12-gaindirect-command"), (R21, [NearusBrc300, GenericVisca], "r21-gaindirect-command")]) },
    GainLimit => { class: plain, packet: Some(crate::source_evidence::PacketPattern::exact(&crate::command::bytes::constants::gain::LIMIT)), source: crate::source_evidence::source!([(R1, [PtzOpticsG2, PtzOptics30x], "r1-gainlimit-command"), (R14, [PtzOpticsG3], "r14-gainlimit-command")]) },
    // White balance.
    WhiteBalanceAuto => { class: plain, packet: Some(crate::source_evidence::PacketPattern::exact(&crate::command::bytes::constants::white_balance::MODE).then(&[0x00], &[0xFF])), source: crate::source_evidence::source!([(R1, [PtzOpticsG2, PtzOptics30x], "r1-whitebalanceauto-command"), (R14, [PtzOpticsG3], "r14-whitebalanceauto-command"), (R11, [SonyBrcH900], "r11-whitebalanceauto-command"), (R8, [SonyEviH100, GenericVisca], "r8-whitebalanceauto-command"), (R12, [SonyBrc300, GenericVisca], "r12-whitebalanceauto-command"), (R21, [NearusBrc300, GenericVisca], "r21-whitebalanceauto-command")]) },
    WhiteBalanceIndoor => { class: plain, packet: Some(crate::source_evidence::PacketPattern::exact(&crate::command::bytes::constants::white_balance::MODE).then(&[0x01], &[0xFF])), source: crate::source_evidence::source!([(R1, [PtzOpticsG2, PtzOptics30x], "r1-whitebalanceindoor-command"), (R14, [PtzOpticsG3], "r14-whitebalanceindoor-command"), (R11, [SonyBrcH900], "r11-whitebalanceindoor-command"), (R8, [SonyEviH100, GenericVisca], "r8-whitebalanceindoor-command"), (R12, [SonyBrc300, GenericVisca], "r12-whitebalanceindoor-command"), (R21, [NearusBrc300, GenericVisca], "r21-whitebalanceindoor-command")]) },
    WhiteBalanceOutdoor => { class: plain, packet: Some(crate::source_evidence::PacketPattern::exact(&crate::command::bytes::constants::white_balance::MODE).then(&[0x02], &[0xFF])), source: crate::source_evidence::source!([(R1, [PtzOpticsG2, PtzOptics30x], "r1-whitebalanceoutdoor-command"), (R14, [PtzOpticsG3], "r14-whitebalanceoutdoor-command"), (R11, [SonyBrcH900], "r11-whitebalanceoutdoor-command"), (R8, [SonyEviH100, GenericVisca], "r8-whitebalanceoutdoor-command"), (R12, [SonyBrc300, GenericVisca], "r12-whitebalanceoutdoor-command"), (R21, [NearusBrc300, GenericVisca], "r21-whitebalanceoutdoor-command")]) },
    WhiteBalanceOnePush => { class: plain, packet: Some(crate::source_evidence::PacketPattern::exact(&crate::command::bytes::constants::white_balance::MODE).then(&[0x03], &[0xFF])), source: crate::source_evidence::source!([(R1, [PtzOpticsG2, PtzOptics30x], "r1-whitebalanceonepush-command"), (R14, [PtzOpticsG3], "r14-whitebalanceonepush-command"), (R11, [SonyBrcH900], "r11-whitebalanceonepush-command"), (R8, [SonyEviH100, GenericVisca], "r8-whitebalanceonepush-command"), (R12, [SonyBrc300, GenericVisca], "r12-whitebalanceonepush-command"), (R21, [NearusBrc300, GenericVisca], "r21-whitebalanceonepush-command")]) },
    WhiteBalanceAutoTracking => { class: plain, packet: Some(crate::source_evidence::PacketPattern::exact(&crate::command::bytes::constants::white_balance::MODE).then(&[0x04], &[0xFF])), source: crate::source_evidence::source!([(R7, [SonyFr7], "r7-whitebalanceautotracking-command")]) },
    WhiteBalanceManual => { class: plain, packet: Some(crate::source_evidence::PacketPattern::exact(&crate::command::bytes::constants::white_balance::MODE).then(&[0x05], &[0xFF])), source: crate::source_evidence::source!([(R1, [PtzOpticsG2, PtzOptics30x], "r1-whitebalancemanual-command"), (R14, [PtzOpticsG3], "r14-whitebalancemanual-command"), (R11, [SonyBrcH900], "r11-whitebalancemanual-command"), (R8, [SonyEviH100, GenericVisca], "r8-whitebalancemanual-command"), (R12, [SonyBrc300, GenericVisca], "r12-whitebalancemanual-command"), (R21, [NearusBrc300, GenericVisca], "r21-whitebalancemanual-command"), (R7, [SonyFr7], "r7-command-whitebalancemanual")]) },
    WhiteBalanceColorTemperature => { class: plain, packet: Some(crate::source_evidence::PacketPattern::exact(&crate::command::bytes::constants::white_balance::MODE).then(&[0x20], &[0xFF])), source: crate::source_evidence::source!([(R1, [PtzOpticsG2, PtzOptics30x], "r1-whitebalancecolortemperature-command"), (R14, [PtzOpticsG3], "r14-whitebalancecolortemperature-command")]) },
    AutoWhiteBalanceSensitivity => { class: plain, packet: Some(crate::source_evidence::PacketPattern::exact(&crate::command::bytes::constants::white_balance::AWB_SENSITIVITY)), source: crate::source_evidence::source!([(R1, [PtzOpticsG2, PtzOptics30x], "r1-autowhitebalancesensitivity-command"), (R14, [PtzOpticsG3], "r14-autowhitebalancesensitivity-command")]) },
    OnePushWhiteBalanceTrigger => { class: plain, packet: Some(crate::source_evidence::PacketPattern::exact(&crate::command::bytes::constants::white_balance::ONE_PUSH_TRIGGER)), source: crate::source_evidence::source!([(R1, [PtzOpticsG2, PtzOptics30x], "r1-onepushwhitebalancetrigger-command"), (R14, [PtzOpticsG3], "r14-onepushwhitebalancetrigger-command"), (R8, [SonyEviH100, GenericVisca], "r8-onepushwhitebalancetrigger-command"), (R12, [SonyBrc300, GenericVisca], "r12-onepushwhitebalancetrigger-command"), (R21, [NearusBrc300, GenericVisca], "r21-onepushwhitebalancetrigger-command")]) },
    // Color.
    RedTuning => { class: plain, packet: Some(crate::source_evidence::PacketPattern::exact(&crate::command::bytes::constants::color::RED_TUNING_DIRECT).prefix_len(3)), source: crate::source_evidence::source!([(R1, [PtzOpticsG2, PtzOptics30x], "r1-redtuning-command"), (R14, [PtzOpticsG3], "r14-redtuning-command")]) },
    BlueTuning => { class: plain, packet: Some(crate::source_evidence::PacketPattern::exact(&crate::command::bytes::constants::color::BLUE_TUNING_DIRECT).prefix_len(3)), source: crate::source_evidence::source!([(R1, [PtzOpticsG2, PtzOptics30x], "r1-bluetuning-command"), (R14, [PtzOpticsG3], "r14-bluetuning-command")]) },
    Saturation => { class: plain, packet: Some(crate::source_evidence::PacketPattern::exact(&crate::command::bytes::constants::color::SATURATION).prefix_len(3)), source: crate::source_evidence::source!([(R1, [PtzOpticsG2, PtzOptics30x], "r1-saturation-command"), (R14, [PtzOpticsG3], "r14-saturation-command"), (R8, [SonyEviH100], "r8-saturation-command")]) },
    Hue => { class: plain, packet: Some(crate::source_evidence::PacketPattern::exact(&crate::command::bytes::constants::color::HUE).prefix_len(3)), source: crate::source_evidence::source!([(R1, [PtzOpticsG2, PtzOptics30x], "r1-hue-command"), (R14, [PtzOpticsG3], "r14-hue-command"), (R8, [SonyEviH100], "r8-hue-command")]) },
    ColorTemperatureReset => { class: plain, packet: Some(crate::source_evidence::PacketPattern::exact(&crate::command::bytes::constants::color::TEMPERATURE).then(&[0x00, 0xFF], &[0xFF, 0xFF])), source: crate::source_evidence::source!([(R1, [PtzOpticsG2, PtzOptics30x], "r1-colortemperaturereset-command"), (R14, [PtzOpticsG3], "r14-colortemperaturereset-command")]) },
    ColorTemperatureUp => { class: plain, packet: Some(crate::source_evidence::PacketPattern::exact(&crate::command::bytes::constants::color::TEMPERATURE).then(&[0x02, 0xFF], &[0xFF, 0xFF])), source: crate::source_evidence::source!([(R1, [PtzOpticsG2, PtzOptics30x], "r1-colortemperatureup-command"), (R14, [PtzOpticsG3], "r14-colortemperatureup-command")]) },
    ColorTemperatureDown => { class: plain, packet: Some(crate::source_evidence::PacketPattern::exact(&crate::command::bytes::constants::color::TEMPERATURE).then(&[0x03, 0xFF], &[0xFF, 0xFF])), source: crate::source_evidence::source!([(R1, [PtzOpticsG2, PtzOptics30x], "r1-colortemperaturedown-command"), (R14, [PtzOpticsG3], "r14-colortemperaturedown-command")]) },
    ColorTemperatureDirect => { class: plain, packet: Some(crate::source_evidence::PacketPattern::exact(&crate::command::bytes::constants::color::TEMPERATURE).then(&[0, 0, 0xFF], &[0xF0, 0xF0, 0xFF])), source: crate::source_evidence::source!([(R1, [PtzOpticsG2, PtzOptics30x], "r1-colortemperaturedirect-command"), (R14, [PtzOpticsG3], "r14-colortemperaturedirect-command")]) },
    RedGainReset => { class: plain, packet: Some(crate::source_evidence::PacketPattern::exact(&crate::command::bytes::constants::color::RED_GAIN_STEP).then(&[0x00], &[0xFF])), source: crate::source_evidence::source!([(R1, [PtzOpticsG2, PtzOptics30x], "r1-redgainreset-command"), (R14, [PtzOpticsG3], "r14-redgainreset-command"), (R8, [SonyEviH100, GenericVisca], "r8-redgainreset-command"), (R12, [SonyBrc300, GenericVisca], "r12-redgainreset-command"), (R21, [NearusBrc300, GenericVisca], "r21-redgainreset-command")]) },
    RedGainUp => { class: plain, packet: Some(crate::source_evidence::PacketPattern::exact(&crate::command::bytes::constants::color::RED_GAIN_STEP).then(&[0x02], &[0xFF])), source: crate::source_evidence::source!([(R1, [PtzOpticsG2, PtzOptics30x], "r1-redgainup-command"), (R14, [PtzOpticsG3], "r14-redgainup-command"), (R8, [SonyEviH100, GenericVisca], "r8-redgainup-command"), (R12, [SonyBrc300, GenericVisca], "r12-redgainup-command"), (R21, [NearusBrc300, GenericVisca], "r21-redgainup-command")]) },
    RedGainDown => { class: plain, packet: Some(crate::source_evidence::PacketPattern::exact(&crate::command::bytes::constants::color::RED_GAIN_STEP).then(&[0x03], &[0xFF])), source: crate::source_evidence::source!([(R1, [PtzOpticsG2, PtzOptics30x], "r1-redgaindown-command"), (R14, [PtzOpticsG3], "r14-redgaindown-command"), (R8, [SonyEviH100, GenericVisca], "r8-redgaindown-command"), (R12, [SonyBrc300, GenericVisca], "r12-redgaindown-command"), (R21, [NearusBrc300, GenericVisca], "r21-redgaindown-command")]) },
    RedGainDirect => { class: plain, packet: Some(crate::source_evidence::PacketPattern::exact(&crate::command::bytes::constants::color::RED_GAIN_DIRECT).prefix_len(3)), source: crate::source_evidence::source!([(R1, [PtzOpticsG2, PtzOptics30x], "r1-redgaindirect-command"), (R14, [PtzOpticsG3], "r14-redgaindirect-command"), (R8, [SonyEviH100, GenericVisca], "r8-redgaindirect-command"), (R12, [SonyBrc300, GenericVisca], "r12-redgaindirect-command"), (R21, [NearusBrc300, GenericVisca], "r21-redgaindirect-command")]) },
    BlueGainReset => { class: plain, packet: Some(crate::source_evidence::PacketPattern::exact(&crate::command::bytes::constants::color::BLUE_GAIN_STEP).then(&[0x00], &[0xFF])), source: crate::source_evidence::source!([(R1, [PtzOpticsG2, PtzOptics30x], "r1-bluegainreset-command"), (R14, [PtzOpticsG3], "r14-bluegainreset-command"), (R8, [SonyEviH100, GenericVisca], "r8-bluegainreset-command"), (R12, [SonyBrc300, GenericVisca], "r12-bluegainreset-command"), (R21, [NearusBrc300, GenericVisca], "r21-bluegainreset-command")]) },
    BlueGainUp => { class: plain, packet: Some(crate::source_evidence::PacketPattern::exact(&crate::command::bytes::constants::color::BLUE_GAIN_STEP).then(&[0x02], &[0xFF])), source: crate::source_evidence::source!([(R1, [PtzOpticsG2, PtzOptics30x], "r1-bluegainup-command"), (R14, [PtzOpticsG3], "r14-bluegainup-command"), (R8, [SonyEviH100, GenericVisca], "r8-bluegainup-command"), (R12, [SonyBrc300, GenericVisca], "r12-bluegainup-command"), (R21, [NearusBrc300, GenericVisca], "r21-bluegainup-command")]) },
    BlueGainDown => { class: plain, packet: Some(crate::source_evidence::PacketPattern::exact(&crate::command::bytes::constants::color::BLUE_GAIN_STEP).then(&[0x03], &[0xFF])), source: crate::source_evidence::source!([(R1, [PtzOpticsG2, PtzOptics30x], "r1-bluegaindown-command"), (R14, [PtzOpticsG3], "r14-bluegaindown-command"), (R8, [SonyEviH100, GenericVisca], "r8-bluegaindown-command"), (R12, [SonyBrc300, GenericVisca], "r12-bluegaindown-command"), (R21, [NearusBrc300, GenericVisca], "r21-bluegaindown-command")]) },
    BlueGainDirect => { class: plain, packet: Some(crate::source_evidence::PacketPattern::exact(&crate::command::bytes::constants::color::BLUE_GAIN_DIRECT).prefix_len(3)), source: crate::source_evidence::source!([(R1, [PtzOpticsG2, PtzOptics30x], "r1-bluegaindirect-command"), (R14, [PtzOpticsG3], "r14-bluegaindirect-command"), (R8, [SonyEviH100, GenericVisca], "r8-bluegaindirect-command"), (R12, [SonyBrc300, GenericVisca], "r12-bluegaindirect-command"), (R21, [NearusBrc300, GenericVisca], "r21-bluegaindirect-command")]) },
    // Image processing and orientation.
    SharpnessMode => { class: plain, packet: Some(crate::source_evidence::PacketPattern::exact(&crate::command::bytes::constants::image::SHARPNESS_MODE)), source: crate::source_evidence::source!([(R1, [PtzOpticsG2, PtzOptics30x], "r1-sharpnessmode-command"), (R14, [PtzOpticsG3], "r14-sharpnessmode-command")]) },
    SharpnessReset => { class: plain, packet: Some(crate::source_evidence::PacketPattern::exact(&crate::command::bytes::constants::image::SHARPNESS_STEP).then(&[0x00], &[0xFF])), source: crate::source_evidence::source!([(R1, [PtzOpticsG2, PtzOptics30x], "r1-sharpnessreset-command"), (R14, [PtzOpticsG3], "r14-sharpnessreset-command"), (R7, [SonyFr7], "r7-sharpnessreset-command"), (R8, [SonyEviH100], "r8-sharpnessreset-command")]) },
    SharpnessUp => { class: plain, packet: Some(crate::source_evidence::PacketPattern::exact(&crate::command::bytes::constants::image::SHARPNESS_STEP).then(&[0x02], &[0xFF])), source: crate::source_evidence::source!([(R1, [PtzOpticsG2, PtzOptics30x], "r1-sharpnessup-command"), (R14, [PtzOpticsG3], "r14-sharpnessup-command"), (R7, [SonyFr7], "r7-sharpnessup-command"), (R8, [SonyEviH100], "r8-sharpnessup-command")]) },
    SharpnessDown => { class: plain, packet: Some(crate::source_evidence::PacketPattern::exact(&crate::command::bytes::constants::image::SHARPNESS_STEP).then(&[0x03], &[0xFF])), source: crate::source_evidence::source!([(R1, [PtzOpticsG2, PtzOptics30x], "r1-sharpnessdown-command"), (R14, [PtzOpticsG3], "r14-sharpnessdown-command"), (R7, [SonyFr7], "r7-sharpnessdown-command"), (R8, [SonyEviH100], "r8-sharpnessdown-command")]) },
    SharpnessDirect => { class: plain, packet: Some(crate::source_evidence::PacketPattern::exact(&crate::command::bytes::constants::image::SHARPNESS_DIRECT).prefix_len(3)), source: crate::source_evidence::source!([(R1, [PtzOpticsG2, PtzOptics30x], "r1-sharpnessdirect-command"), (R14, [PtzOpticsG3], "r14-sharpnessdirect-command"), (R7, [SonyFr7], "r7-sharpnessdirect-command"), (R8, [SonyEviH100], "r8-sharpnessdirect-command")]) },
    Luminance => { class: plain, packet: Some(crate::source_evidence::PacketPattern::exact(&crate::command::bytes::constants::image::LUMINANCE).prefix_len(3)), source: crate::source_evidence::source!([(R1, [PtzOpticsG2, PtzOptics30x], "r1-luminance-command"), (R14, [PtzOpticsG3], "r14-luminance-command")]) },
    Contrast => { class: plain, packet: Some(crate::source_evidence::PacketPattern::exact(&crate::command::bytes::constants::image::CONTRAST).prefix_len(3)), source: crate::source_evidence::source!([(R1, [PtzOpticsG2, PtzOptics30x], "r1-contrast-command"), (R14, [PtzOpticsG3], "r14-contrast-command")]) },
    Gamma => { class: plain, packet: Some(crate::source_evidence::PacketPattern::exact(&crate::command::bytes::constants::image::GAMMA)), source: crate::source_evidence::source!([(R1, [PtzOpticsG2, PtzOptics30x], "r1-gamma-command"), (R14, [PtzOpticsG3], "r14-gamma-command"), (R8, [SonyEviH100], "r8-gamma-command")]) },
    Backlight => { class: plain, packet: Some(crate::source_evidence::PacketPattern::exact(&crate::command::bytes::constants::image::BACKLIGHT)), source: crate::source_evidence::source!([(R1, [PtzOpticsG2, PtzOptics30x], "r1-backlight-command"), (R14, [PtzOpticsG3], "r14-backlight-command"), (R7, [SonyFr7], "r7-backlight-command"), (R8, [SonyEviH100, GenericVisca], "r8-backlight-command"), (R12, [SonyBrc300, GenericVisca], "r12-backlight-command"), (R21, [NearusBrc300, GenericVisca], "r21-backlight-command")]) },
    NoiseReduction2dMode => { class: plain, packet: Some(crate::source_evidence::PacketPattern::exact(&crate::command::bytes::constants::image::NOISE_REDUCTION_2D_MODE)), source: crate::source_evidence::source!([(R14, [PtzOpticsG2, PtzOpticsG3, PtzOptics30x], "r14-noisereduction2dmode-command")]) },
    NoiseReduction2d => { class: plain, packet: Some(crate::source_evidence::PacketPattern::exact(&crate::command::bytes::constants::image::NOISE_REDUCTION_2D).select(&[&[1], &[2], &[3], &[4], &[5]])), source: crate::source_evidence::source!([(R14, [PtzOpticsG2, PtzOpticsG3, PtzOptics30x], "r14-noisereduction2d-command"), (R8, [SonyEviH100], "r8-noisereduction2d-command")]) },
    NoiseReduction2dOff => { class: plain, packet: Some(crate::source_evidence::PacketPattern::exact(&crate::command::bytes::constants::image::NOISE_REDUCTION_2D).then(&[0], &[0xFF])), source: crate::source_evidence::source!([(R14, [PtzOpticsG2, PtzOpticsG3, PtzOptics30x], "r14-noisereduction2doff-command"), (R8, [SonyEviH100], "r8-noisereduction2doff-command")]) },
    NoiseReduction3d => { class: plain, packet: Some(crate::source_evidence::PacketPattern::exact(&crate::command::bytes::constants::image::NOISE_REDUCTION_3D).select(&[&[1], &[2], &[3], &[4], &[5], &[6], &[7], &[8]])), source: crate::source_evidence::source!([(R14, [PtzOpticsG2, PtzOpticsG3, PtzOptics30x], "r14-noisereduction3d-command")]) },
    NoiseReduction3dOff => { class: plain, packet: Some(crate::source_evidence::PacketPattern::exact(&crate::command::bytes::constants::image::NOISE_REDUCTION_3D).then(&[0], &[0xFF])), source: crate::source_evidence::source!([(R14, [PtzOpticsG2, PtzOpticsG3, PtzOptics30x], "r14-noisereduction3doff-command")]) },
    // The single-axis flip and mirror opcodes move one axis and say nothing
    // about the other, so a complete pair stops being known.
    ImageFlipOff => { class: plain(Invalidate(ImageFlipMode)), packet: Some(crate::source_evidence::PacketPattern::exact(&crate::command::bytes::constants::flip::VERTICAL).then(&[3], &[0xFF])), source: crate::source_evidence::source!([(R1, [PtzOpticsG2, PtzOptics30x], "r1-imageflipoff-command"), (R14, [PtzOpticsG3], "r14-imageflipoff-command"), (R12, [SonyBrc300], "r12-imageflipoff-command"), (R21, [NearusBrc300], "r21-imageflipoff-command")]) },
    ImageFlipHorizontal => { class: plain(Invalidate(ImageFlipMode)), packet: Some(crate::source_evidence::PacketPattern::exact(&crate::command::bytes::constants::flip::HORIZONTAL).then(&[2], &[0xFF])), source: crate::source_evidence::source!([(R1, [PtzOpticsG2, PtzOptics30x], "r1-imagefliphorizontal-command"), (R14, [PtzOpticsG3], "r14-imagefliphorizontal-command")]) },
    ImageFlipHorizontalOff => { class: plain(Invalidate(ImageFlipMode)), packet: Some(crate::source_evidence::PacketPattern::exact(&crate::command::bytes::constants::flip::HORIZONTAL).then(&[3], &[0xFF])), source: crate::source_evidence::source!([(R1, [PtzOpticsG2, PtzOptics30x], "r1-imagefliphorizontaloff-command"), (R14, [PtzOpticsG3], "r14-imagefliphorizontaloff-command")]) },
    ImageFlipVertical => { class: plain(Invalidate(ImageFlipMode)), packet: Some(crate::source_evidence::PacketPattern::exact(&crate::command::bytes::constants::flip::VERTICAL).then(&[2], &[0xFF])), source: crate::source_evidence::source!([(R1, [PtzOpticsG2, PtzOptics30x], "r1-imageflipvertical-command"), (R14, [PtzOpticsG3], "r14-imageflipvertical-command"), (R12, [SonyBrc300], "r12-imageflipvertical-command"), (R21, [NearusBrc300], "r21-imageflipvertical-command")]) },
    // The combined-flip opcode carries both axes in one parameter, so it
    // establishes a complete horizontal/vertical pair.
    ImageFlipBoth => { class: plain(Set(ImageFlipMode)), packet: Some(crate::source_evidence::PacketPattern::exact(&crate::command::bytes::constants::image::FLIP_COMBINED).then(&[3], &[0xFF])), source: crate::source_evidence::source!([(R1, [PtzOpticsG2, PtzOptics30x], "r1-imageflipboth-command"), (R14, [PtzOpticsG3], "r14-imageflipboth-command")]) },
    ImageFlipCombined => { class: plain(Set(ImageFlipMode)), packet: Some(crate::source_evidence::PacketPattern::exact(&crate::command::bytes::constants::image::FLIP_COMBINED)), source: crate::source_evidence::source!([(R1, [PtzOpticsG2, PtzOptics30x], "r1-imageflipcombined-command"), (R14, [PtzOpticsG3], "r14-imageflipcombined-command")]) },
    // The `Off` request supplies `false`: a known value, not removal of the
    // cache entry.
    ImageFreezeOn => { class: plain(Set(ImageFreeze)), packet: None, source: crate::source_evidence::Source::Unsourced },
    ImageFreezeOff => { class: plain(Set(ImageFreeze)), packet: None, source: crate::source_evidence::Source::Unsourced },
    PictureEffect => { class: plain, packet: Some(crate::source_evidence::PacketPattern::exact(&crate::command::bytes::constants::image::PICTURE_EFFECT)), source: crate::source_evidence::source!([(R1, [PtzOpticsG2, PtzOptics30x], "r1-pictureeffect-command"), (R14, [PtzOpticsG3], "r14-pictureeffect-command")]) },
    // Neutral-density filter.
    NdFilterMode => { class: plain(Set(NdFilterMode)), packet: Some(crate::source_evidence::PacketPattern::exact(&crate::command::bytes::constants::nd_filter::MODE)), source: crate::source_evidence::source!([(R7, [SonyFr7], "r7-ndfiltermode-command")]) },
    // ND direct and step commands physically reposition the filter. The
    // direct target and each finite step have meaningful end states and are
    // therefore targeted on the ND axis.
    NdFilterDirect => { class: targeted(ND_FILTER), packet: Some(crate::source_evidence::PacketPattern::exact(&crate::command::bytes::constants::nd_filter::DIRECT).prefix_len(4)), source: crate::source_evidence::source!([(R7, [SonyFr7], "r7-ndfilterdirect-command")]) },
    NdFilterStepUp => { class: targeted(ND_FILTER), packet: Some(crate::source_evidence::PacketPattern::exact(&crate::command::bytes::constants::nd_filter::STEP).then(&[0x02], &[0xFF])), source: crate::source_evidence::source!([(R7, [SonyFr7], "r7-ndfilterstepup-command")]) },
    NdFilterStepDown => { class: targeted(ND_FILTER), packet: Some(crate::source_evidence::PacketPattern::exact(&crate::command::bytes::constants::nd_filter::STEP).then(&[0x03], &[0xFF])), source: crate::source_evidence::source!([(R7, [SonyFr7], "r7-ndfilterstepdown-command")]) },
    NdFilterAutoOn => { class: plain(Set(NdFilterAuto)), packet: Some(crate::source_evidence::PacketPattern::exact(&crate::command::bytes::constants::nd_filter::AUTO).then(&[0x02], &[0xFF])), source: crate::source_evidence::source!([(R7, [SonyFr7], "r7-ndfilterautoon-command")]) },
    NdFilterAutoOff => { class: plain(Set(NdFilterAuto)), packet: Some(crate::source_evidence::PacketPattern::exact(&crate::command::bytes::constants::nd_filter::AUTO).then(&[0x03], &[0xFF])), source: crate::source_evidence::source!([(R7, [SonyFr7], "r7-ndfilterautooff-command")]) },
    // Tally. Red/green tally status has profile-gated inquiries. Brightness
    // and lamp output mode do not, so exact application records their typed
    // values, including the flashing mode.
    TallyRedOn => { class: plain, packet: Some(crate::source_evidence::PacketPattern::exact(&crate::command::bytes::constants::tally::RED_ON)), source: crate::source_evidence::source!([(R7, [SonyFr7], "r7-tallyredon-command")]) },
    TallyRedOff => { class: plain, packet: Some(crate::source_evidence::PacketPattern::exact(&crate::command::bytes::constants::tally::RED_OFF)), source: crate::source_evidence::source!([(R7, [SonyFr7], "r7-tallyredoff-command")]) },
    TallyBrightLow => { class: plain(Set(TallyBrightness)), packet: None, source: crate::source_evidence::Source::Unsourced },
    TallyBrightHigh => { class: plain(Set(TallyBrightness)), packet: None, source: crate::source_evidence::Source::Unsourced },
    TallyGreenOn => { class: plain, packet: Some(crate::source_evidence::PacketPattern::exact(&crate::command::bytes::constants::tally::GREEN_ON)), source: crate::source_evidence::source!([(R7, [SonyFr7], "r7-tallygreenon-command")]) },
    TallyGreenOff => { class: plain, packet: Some(crate::source_evidence::PacketPattern::exact(&crate::command::bytes::constants::tally::GREEN_OFF)), source: crate::source_evidence::source!([(R7, [SonyFr7], "r7-tallygreenoff-command")]) },
    TallyFlash => { class: plain(Set(TallyMode)), packet: None, source: crate::source_evidence::Source::Unsourced },
    TallyOn => { class: plain(Set(TallyMode)), packet: None, source: crate::source_evidence::Source::Unsourced },
    TallyOff => { class: plain(Set(TallyMode)), packet: None, source: crate::source_evidence::Source::Unsourced },
    // Menus.
    MenuDisplay => { class: plain, packet: Some(crate::source_evidence::PacketPattern::exact(&crate::command::bytes::constants::menu::MENU).select(&[&[0x02], &[0x03]])), source: crate::source_evidence::source!([(R7, [SonyFr7], "r7-menudisplay-command")]) },
    MenuNavigate => { class: applied(PAN_TILT), packet: Some(crate::source_evidence::PacketPattern::exact(&crate::command::bytes::constants::menu::NAVIGATE).select(&[&[3, 1], &[3, 2], &[1, 3], &[2, 3]])), source: crate::source_evidence::source!([(R1, [PtzOpticsG2, PtzOptics30x], "r1-menunavigate-command"), (R14, [PtzOpticsG3], "r14-menunavigate-command"), (R7, [SonyFr7], "r7-menunavigate-command"), (R11, [SonyBrcH900], "r11-menunavigate-command"), (R8, [SonyEviH100, GenericVisca], "r8-menunavigate-command"), (R12, [SonyBrc300, GenericVisca], "r12-menunavigate-command"), (R21, [NearusBrc300, GenericVisca], "r21-menunavigate-command")]) },
    MenuSelect => { class: plain, packet: Some(crate::source_evidence::PacketPattern::exact(&crate::command::bytes::constants::menu::MENU).then(&[0x05], &[0xFF])), source: crate::source_evidence::source!([(R1, [PtzOpticsG2, PtzOptics30x], "r1-menuselect-command"), (R14, [PtzOpticsG3], "r14-menuselect-command")]) },
    MenuCancel => { class: plain, packet: Some(crate::source_evidence::PacketPattern::exact(&crate::command::bytes::constants::menu::MENU).then(&[0x04], &[0xFF])), source: crate::source_evidence::source!([(R1, [PtzOpticsG2, PtzOptics30x], "r1-menucancel-command"), (R14, [PtzOpticsG3], "r14-menucancel-command")]) },
    DirectMenu => { class: plain, packet: Some(crate::source_evidence::PacketPattern::exact(&crate::command::bytes::constants::menu::DIRECT)), source: crate::source_evidence::source!([(R7, [SonyFr7], "r7-directmenu-command")]) },
    // Streaming. The multicast `Off` request supplies `false`: a known value,
    // not removal of the cache entry.
    MulticastStreamingOn => { class: plain(Set(StreamingMulticast)), packet: Some(crate::source_evidence::PacketPattern::exact(&crate::command::bytes::constants::streaming::MULTICAST).then(&[1], &[0xFF])), source: crate::source_evidence::source!([(R1, [PtzOpticsG2, PtzOptics30x], "r1-multicaststreamingon-command"), (R10, [PtzOpticsG3], "r10-multicaststreamingon-command")]) },
    MulticastStreamingOff => { class: plain(Set(StreamingMulticast)), packet: Some(crate::source_evidence::PacketPattern::exact(&crate::command::bytes::constants::streaming::MULTICAST).then(&[2], &[0xFF])), source: crate::source_evidence::source!([(R1, [PtzOpticsG2, PtzOptics30x], "r1-multicaststreamingoff-command"), (R10, [PtzOpticsG3], "r10-multicaststreamingoff-command")]) },
    NdiQuality => { class: plain(Set(StreamingNdiQuality)), packet: Some(crate::source_evidence::PacketPattern::exact(&crate::command::bytes::constants::streaming::NDI_QUALITY)), source: crate::source_evidence::source!([(R1, [PtzOpticsG2, PtzOptics30x], "r1-ndiquality-command"), (R10, [PtzOpticsG3], "r10-ndiquality-command")]) },
    UsbAudioOn => { class: plain, packet: Some(crate::source_evidence::PacketPattern::exact(&crate::command::bytes::constants::streaming::USB_AUDIO).then(&[2], &[0xFF])), source: crate::source_evidence::source!([(R1, [PtzOpticsG2, PtzOptics30x], "r1-usbaudioon-command")]) },
    UsbAudioOff => { class: plain, packet: Some(crate::source_evidence::PacketPattern::exact(&crate::command::bytes::constants::streaming::USB_AUDIO).then(&[3], &[0xFF])), source: crate::source_evidence::source!([(R1, [PtzOpticsG2, PtzOptics30x], "r1-usbaudiooff-command")]) },
    // System.
    AddressSet => { class: plain, packet: Some(crate::source_evidence::PacketPattern::exact(&crate::command::bytes::constants::system::ADDRESS_SET)), source: crate::source_evidence::source!([(R8, [SonyEviH100, GenericVisca], "r8-addressset-command"), (R12, [SonyBrc300, GenericVisca], "r12-addressset-command"), (R21, [NearusBrc300, GenericVisca], "r21-addressset-command"), (R11, [SonyBrcH900], "r11-addressset-command")]) },
    InterfaceClear => { class: plain, packet: Some(crate::source_evidence::PacketPattern::exact(&crate::command::bytes::constants::system::INTERFACE_CLEAR)), source: crate::source_evidence::source!([(R1, [PtzOpticsG2, PtzOptics30x], "r1-interfaceclear-command"), (R14, [PtzOpticsG3], "r14-interfaceclear-command"), (R7, [SonyFr7], "r7-interfaceclear-command"), (R11, [SonyBrcH900], "r11-interfaceclear-command"), (R8, [SonyEviH100, GenericVisca], "r8-interfaceclear-command"), (R12, [SonyBrc300, GenericVisca], "r12-interfaceclear-command"), (R21, [NearusBrc300, GenericVisca], "r21-interfaceclear-command")]) },
    CommandCancel => { class: plain, packet: Some(crate::source_evidence::PacketPattern::exact(&[]).then(&[crate::command::bytes::constants::system::COMMAND_CANCEL], &[0xF0])), source: crate::source_evidence::source!([(R1, [PtzOpticsG2, PtzOptics30x], "r1-commandcancel-command"), (R14, [PtzOpticsG3], "r14-commandcancel-command"), (R7, [SonyFr7], "r7-commandcancel-command"), (R11, [SonyBrcH900], "r11-commandcancel-command"), (R8, [SonyEviH100, GenericVisca], "r8-commandcancel-command"), (R12, [SonyBrc300, GenericVisca], "r12-commandcancel-command"), (R21, [NearusBrc300, GenericVisca], "r21-commandcancel-command")]) },
    SettingsSave => { class: plain, packet: Some(crate::source_evidence::PacketPattern::exact(&crate::command::bytes::constants::system::SETTINGS_SAVE)), source: crate::source_evidence::source!([(R1, [PtzOpticsG2, PtzOptics30x], "r1-settingssave-command"), (R10, [PtzOpticsG3], "r10-settingssave-command")]) },
    // Motion-related configuration. The speed command retains its historical
    // protocol row identity; its exact applied value is now cached because no
    // sourced inquiry can read that speed back.
    MotionSyncMode => { class: plain, packet: None, source: crate::source_evidence::Source::Unsourced },
    MotionSyncPreset => { class: plain(Set(MotionSyncSpeed)), packet: None, source: crate::source_evidence::Source::Unsourced },
    VariableSpeedMode => { class: plain(Set(PanTiltSpeedMode)), packet: Some(crate::source_evidence::PacketPattern::exact(&crate::command::bytes::constants::variable_speed::STEP_RANGE)), source: crate::source_evidence::source!([(R7, [SonyFr7], "r7-variablespeedmode-command")]) },
    // Sony FR7 cinema controls, R7 v4.
    Fr7AeLevelUp => { class: plain, packet: Some(crate::source_evidence::PacketPattern::exact(&crate::command::bytes::constants::exposure::COMPENSATION_STEP).then(&[0x02], &[0xFF])), source: crate::source_evidence::source!(R7, [SonyFr7], "r7-command-fr7aelevelup") },
    Fr7AeLevelDown => { class: plain, packet: Some(crate::source_evidence::PacketPattern::exact(&crate::command::bytes::constants::exposure::COMPENSATION_STEP).then(&[0x03], &[0xFF])), source: crate::source_evidence::source!(R7, [SonyFr7], "r7-command-fr7aeleveldown") },
    Fr7AutoGain => { class: plain, packet: Some(crate::source_evidence::PacketPattern::exact(&crate::command::bytes::constants::sony_fr7_exposure::AUTO_GAIN)), source: crate::source_evidence::source!(R7, [SonyFr7], "r7-command-fr7autogain") },
    Fr7AutoShutter => { class: plain, packet: Some(crate::source_evidence::PacketPattern::exact(&crate::command::bytes::constants::sony_fr7_exposure::AUTO_SHUTTER)), source: crate::source_evidence::source!(R7, [SonyFr7], "r7-command-fr7autoshutter") },
    Fr7AutoIris => { class: plain, packet: Some(crate::source_evidence::PacketPattern::exact(&crate::command::bytes::constants::sony_fr7_exposure::AUTO_IRIS)), source: crate::source_evidence::source!(R7, [SonyFr7], "r7-command-fr7autoiris") },
    Fr7IrisUp => { class: applied(IRIS), packet: Some(crate::source_evidence::PacketPattern::exact(&crate::command::bytes::constants::sony_fr7_exposure::IRIS_STEP).then(&[0x02], &[0xFF])), source: crate::source_evidence::source!(R7, [SonyFr7], "r7-command-fr7irisup") },
    Fr7IrisDown => { class: applied(IRIS), packet: Some(crate::source_evidence::PacketPattern::exact(&crate::command::bytes::constants::sony_fr7_exposure::IRIS_STEP).then(&[0x03], &[0xFF])), source: crate::source_evidence::source!(R7, [SonyFr7], "r7-command-fr7irisdown") },
    Fr7WhiteBalanceSet => { class: plain, packet: Some(crate::source_evidence::PacketPattern::exact(&crate::command::bytes::constants::white_balance::ONE_PUSH_TRIGGER)), source: crate::source_evidence::source!(R7, [SonyFr7], "r7-command-fr7whitebalanceset") },
    Fr7PresetWhiteUp => { class: plain, packet: Some(crate::source_evidence::PacketPattern::exact(&crate::command::bytes::constants::sony_fr7_white_balance::PRESET_WHITE_UP)), source: crate::source_evidence::source!(R7, [SonyFr7], "r7-command-fr7presetwhiteup") },
    Fr7PresetWhiteDown => { class: plain, packet: Some(crate::source_evidence::PacketPattern::exact(&crate::command::bytes::constants::sony_fr7_white_balance::PRESET_WHITE_DOWN)), source: crate::source_evidence::source!(R7, [SonyFr7], "r7-command-fr7presetwhitedown") },
    Fr7PresetWhiteDirect => { class: plain, packet: Some(crate::source_evidence::PacketPattern::exact(&crate::command::bytes::constants::sony_fr7_white_balance::PRESET_WHITE_DIRECT)), source: crate::source_evidence::source!(R7, [SonyFr7], "r7-command-fr7presetwhitedirect") },
    Fr7MemoryColorTemperatureUp => { class: plain, packet: Some(crate::source_evidence::PacketPattern::exact(&crate::command::bytes::constants::sony_fr7_white_balance::MEMORY_COLOR_TEMPERATURE_UP)), source: crate::source_evidence::source!(R7, [SonyFr7], "r7-command-fr7memorycolortemperatureup") },
    Fr7MemoryColorTemperatureDown => { class: plain, packet: Some(crate::source_evidence::PacketPattern::exact(&crate::command::bytes::constants::sony_fr7_white_balance::MEMORY_COLOR_TEMPERATURE_DOWN)), source: crate::source_evidence::source!(R7, [SonyFr7], "r7-command-fr7memorycolortemperaturedown") },
    Fr7MemoryColorTemperatureDirect => { class: plain, packet: Some(crate::source_evidence::PacketPattern::exact(&crate::command::bytes::constants::sony_fr7_white_balance::MEMORY_COLOR_TEMPERATURE_DIRECT)), source: crate::source_evidence::source!(R7, [SonyFr7], "r7-command-fr7memorycolortemperaturedirect") },
    Fr7MemoryRedGainUp => { class: plain, packet: Some(crate::source_evidence::PacketPattern::exact(&crate::command::bytes::constants::sony_fr7_white_balance::MEMORY_RED_GAIN_UP)), source: crate::source_evidence::source!(R7, [SonyFr7], "r7-command-fr7memoryredgainup") },
    Fr7MemoryRedGainDown => { class: plain, packet: Some(crate::source_evidence::PacketPattern::exact(&crate::command::bytes::constants::sony_fr7_white_balance::MEMORY_RED_GAIN_DOWN)), source: crate::source_evidence::source!(R7, [SonyFr7], "r7-command-fr7memoryredgaindown") },
    Fr7MemoryRedGainDirect => { class: plain, packet: Some(crate::source_evidence::PacketPattern::exact(&crate::command::bytes::constants::sony_fr7_white_balance::MEMORY_RED_GAIN_DIRECT)), source: crate::source_evidence::source!(R7, [SonyFr7], "r7-command-fr7memoryredgaindirect") },
    Fr7MemoryBlueGainUp => { class: plain, packet: Some(crate::source_evidence::PacketPattern::exact(&crate::command::bytes::constants::sony_fr7_white_balance::MEMORY_BLUE_GAIN_UP)), source: crate::source_evidence::source!(R7, [SonyFr7], "r7-command-fr7memorybluegainup") },
    Fr7MemoryBlueGainDown => { class: plain, packet: Some(crate::source_evidence::PacketPattern::exact(&crate::command::bytes::constants::sony_fr7_white_balance::MEMORY_BLUE_GAIN_DOWN)), source: crate::source_evidence::source!(R7, [SonyFr7], "r7-command-fr7memorybluegaindown") },
    Fr7MemoryBlueGainDirect => { class: plain, packet: Some(crate::source_evidence::PacketPattern::exact(&crate::command::bytes::constants::sony_fr7_white_balance::MEMORY_BLUE_GAIN_DIRECT)), source: crate::source_evidence::source!(R7, [SonyFr7], "r7-command-fr7memorybluegaindirect") },
    Fr7MemoryTintUp => { class: plain, packet: Some(crate::source_evidence::PacketPattern::exact(&crate::command::bytes::constants::sony_fr7_white_balance::MEMORY_TINT_UP)), source: crate::source_evidence::source!(R7, [SonyFr7], "r7-command-fr7memorytintup") },
    Fr7MemoryTintDown => { class: plain, packet: Some(crate::source_evidence::PacketPattern::exact(&crate::command::bytes::constants::sony_fr7_white_balance::MEMORY_TINT_DOWN)), source: crate::source_evidence::source!(R7, [SonyFr7], "r7-command-fr7memorytintdown") },
    Fr7MemoryTintDirect => { class: plain, packet: Some(crate::source_evidence::PacketPattern::exact(&crate::command::bytes::constants::sony_fr7_white_balance::MEMORY_TINT_DIRECT)), source: crate::source_evidence::source!(R7, [SonyFr7], "r7-command-fr7memorytintdirect") },
    Fr7AtwColorTemperatureOffsetUp => { class: plain, packet: Some(crate::source_evidence::PacketPattern::exact(&crate::command::bytes::constants::sony_fr7_white_balance::ATW_COLOR_TEMPERATURE_OFFSET_UP)), source: crate::source_evidence::source!(R7, [SonyFr7], "r7-command-fr7atwcolortemperatureoffsetup") },
    Fr7AtwColorTemperatureOffsetDown => { class: plain, packet: Some(crate::source_evidence::PacketPattern::exact(&crate::command::bytes::constants::sony_fr7_white_balance::ATW_COLOR_TEMPERATURE_OFFSET_DOWN)), source: crate::source_evidence::source!(R7, [SonyFr7], "r7-command-fr7atwcolortemperatureoffsetdown") },
    Fr7AtwColorTemperatureOffsetDirect => { class: plain, packet: Some(crate::source_evidence::PacketPattern::exact(&crate::command::bytes::constants::sony_fr7_white_balance::ATW_COLOR_TEMPERATURE_OFFSET_DIRECT)), source: crate::source_evidence::source!(R7, [SonyFr7], "r7-command-fr7atwcolortemperatureoffsetdirect") },
    Fr7AtwTintOffsetUp => { class: plain, packet: Some(crate::source_evidence::PacketPattern::exact(&crate::command::bytes::constants::sony_fr7_white_balance::ATW_TINT_OFFSET_UP)), source: crate::source_evidence::source!(R7, [SonyFr7], "r7-command-fr7atwtintoffsetup") },
    Fr7AtwTintOffsetDown => { class: plain, packet: Some(crate::source_evidence::PacketPattern::exact(&crate::command::bytes::constants::sony_fr7_white_balance::ATW_TINT_OFFSET_DOWN)), source: crate::source_evidence::source!(R7, [SonyFr7], "r7-command-fr7atwtintoffsetdown") },
    Fr7AtwTintOffsetDirect => { class: plain, packet: Some(crate::source_evidence::PacketPattern::exact(&crate::command::bytes::constants::sony_fr7_white_balance::ATW_TINT_OFFSET_DIRECT)), source: crate::source_evidence::source!(R7, [SonyFr7], "r7-command-fr7atwtintoffsetdirect") },
    Fr7MasterBlackUp => { class: plain, packet: Some(crate::source_evidence::PacketPattern::exact(&crate::command::bytes::constants::sony_fr7_image::MASTER_BLACK_STEP).then(&[0x02], &[0xFF])), source: crate::source_evidence::source!(R7, [SonyFr7], "r7-command-fr7masterblackup") },
    Fr7MasterBlackDown => { class: plain, packet: Some(crate::source_evidence::PacketPattern::exact(&crate::command::bytes::constants::sony_fr7_image::MASTER_BLACK_STEP).then(&[0x03], &[0xFF])), source: crate::source_evidence::source!(R7, [SonyFr7], "r7-command-fr7masterblackdown") },
    Fr7MasterBlackDirect => { class: plain, packet: Some(crate::source_evidence::PacketPattern::exact(&crate::command::bytes::constants::sony_fr7_image::MASTER_BLACK_DIRECT)), source: crate::source_evidence::source!(R7, [SonyFr7], "r7-command-fr7masterblackdirect") },
    Fr7RedBlackUp => { class: plain, packet: Some(crate::source_evidence::PacketPattern::exact(&crate::command::bytes::constants::sony_fr7_image::RED_BLACK_STEP).then(&[0x02], &[0xFF])), source: crate::source_evidence::source!(R7, [SonyFr7], "r7-command-fr7redblackup") },
    Fr7RedBlackDown => { class: plain, packet: Some(crate::source_evidence::PacketPattern::exact(&crate::command::bytes::constants::sony_fr7_image::RED_BLACK_STEP).then(&[0x03], &[0xFF])), source: crate::source_evidence::source!(R7, [SonyFr7], "r7-command-fr7redblackdown") },
    Fr7RedBlackDirect => { class: plain, packet: Some(crate::source_evidence::PacketPattern::exact(&crate::command::bytes::constants::sony_fr7_image::RED_BLACK_DIRECT)), source: crate::source_evidence::source!(R7, [SonyFr7], "r7-command-fr7redblackdirect") },
    Fr7BlueBlackUp => { class: plain, packet: Some(crate::source_evidence::PacketPattern::exact(&crate::command::bytes::constants::sony_fr7_image::BLUE_BLACK_STEP).then(&[0x02], &[0xFF])), source: crate::source_evidence::source!(R7, [SonyFr7], "r7-command-fr7blueblackup") },
    Fr7BlueBlackDown => { class: plain, packet: Some(crate::source_evidence::PacketPattern::exact(&crate::command::bytes::constants::sony_fr7_image::BLUE_BLACK_STEP).then(&[0x03], &[0xFF])), source: crate::source_evidence::source!(R7, [SonyFr7], "r7-command-fr7blueblackdown") },
    Fr7BlueBlackDirect => { class: plain, packet: Some(crate::source_evidence::PacketPattern::exact(&crate::command::bytes::constants::sony_fr7_image::BLUE_BLACK_DIRECT)), source: crate::source_evidence::source!(R7, [SonyFr7], "r7-command-fr7blueblackdirect") },
    Fr7DetailSetting => { class: plain, packet: Some(crate::source_evidence::PacketPattern::exact(&crate::command::bytes::constants::sony_fr7_image::DETAIL_SETTING)), source: crate::source_evidence::source!(R7, [SonyFr7], "r7-command-fr7detailsetting") },
    Fr7KneeSetting => { class: plain, packet: Some(crate::source_evidence::PacketPattern::exact(&crate::command::bytes::constants::sony_fr7_image::KNEE_SETTING)), source: crate::source_evidence::source!(R7, [SonyFr7], "r7-command-fr7kneesetting") },
    Fr7SetKneeMode => { class: plain, packet: Some(crate::source_evidence::PacketPattern::exact(&crate::command::bytes::constants::sony_fr7_image::KNEE_MODE)), source: crate::source_evidence::source!(R7, [SonyFr7], "r7-command-fr7setkneemode") },
    Fr7SetKneeSlope => { class: plain, packet: Some(crate::source_evidence::PacketPattern::exact(&crate::command::bytes::constants::sony_fr7_image::KNEE_SLOPE)), source: crate::source_evidence::source!(R7, [SonyFr7], "r7-command-fr7setkneeslope") },
    Fr7SetKneePoint => { class: plain, packet: Some(crate::source_evidence::PacketPattern::exact(&crate::command::bytes::constants::sony_fr7_image::KNEE_POINT)), source: crate::source_evidence::source!(R7, [SonyFr7], "r7-command-fr7setkneepoint") },
    Fr7ZoomHighResolutionTele => { class: applied(ZOOM), packet: Some(crate::source_evidence::PacketPattern::exact(&crate::command::bytes::constants::sony_fr7_zoom::HIGH_RESOLUTION_DRIVE).then(&[0x02], &[0xFF])), source: crate::source_evidence::source!(R7, [SonyFr7], "r7-command-fr7zoomhighresolutiontele") },
    Fr7ZoomHighResolutionWide => { class: applied(ZOOM), packet: Some(crate::source_evidence::PacketPattern::exact(&crate::command::bytes::constants::sony_fr7_zoom::HIGH_RESOLUTION_DRIVE).then(&[0x03], &[0xFF])), source: crate::source_evidence::source!(R7, [SonyFr7], "r7-command-fr7zoomhighresolutionwide") },
    Fr7ZoomHighResolutionStop => { class: applied_stop(ZOOM), packet: Some(crate::source_evidence::PacketPattern::exact(&crate::command::bytes::constants::sony_fr7_zoom::HIGH_RESOLUTION_DRIVE).then(&[0x00], &[0xFF])), source: crate::source_evidence::source!(R7, [SonyFr7], "r7-command-fr7zoomhighresolutionstop") },
    Fr7SetPresetSpeedSelection => { class: plain, packet: Some(crate::source_evidence::PacketPattern::exact(&crate::command::bytes::constants::sony_fr7_presets::PRESET_SPEED_SELECTION)), source: crate::source_evidence::source!(R7, [SonyFr7], "r7-command-fr7setpresetspeedselection") },
    Fr7SetCommonPresetSpeed => { class: plain, packet: Some(crate::source_evidence::PacketPattern::exact(&crate::command::bytes::constants::sony_fr7_presets::COMMON_PRESET_SPEED)), source: crate::source_evidence::source!(R7, [SonyFr7], "r7-command-fr7setcommonpresetspeed") },
    Fr7SetSeparatePresetMode => { class: plain, packet: Some(crate::source_evidence::PacketPattern::exact(&crate::command::bytes::constants::sony_fr7_presets::SEPARATE_PRESET_MODE)), source: crate::source_evidence::source!(R7, [SonyFr7], "r7-command-fr7setseparatepresetmode") },
    Fr7SetSeparatePresetSpeed => { class: plain, packet: Some(crate::source_evidence::PacketPattern::exact(&crate::command::bytes::constants::sony_fr7_presets::SEPARATE_PRESET_SPEED)), source: crate::source_evidence::source!(R7, [SonyFr7], "r7-command-fr7setseparatepresetspeed") },
    Fr7SetSeparatePresetDuration => { class: plain, packet: Some(crate::source_evidence::PacketPattern::exact(&crate::command::bytes::constants::sony_fr7_presets::SEPARATE_PRESET_DURATION)), source: crate::source_evidence::source!(R7, [SonyFr7], "r7-command-fr7setseparatepresetduration") },
    WhiteBalancePreset => { class: plain, packet: Some(crate::source_evidence::PacketPattern::exact(&crate::command::bytes::constants::white_balance::MODE).then(&[0x0A], &[0xFF])), source: crate::source_evidence::source!(R7, [SonyFr7], "r7-command-whitebalancepreset") },
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
                    let packet = command
                        .packet()
                        .expect("sourced command pattern")
                        .to_string();
                    let result =
                        index.validate(source, &subject, Some(&model), Some(&packet), None);
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
                | BuiltinRequestClass::AppliedOnly { axes, .. } => {
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
                axes: BuiltinAxisSelection::Exact(AffectedAxes::PAN_TILT),
                stop: false
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
            Some(AppliedStateEffectRequirement::Set(StateKey::PanTiltLimit))
        );
        assert_eq!(
            BuiltinCommand::PanTiltLimitClear
                .classification()
                .state_effect(),
            Some(AppliedStateEffectRequirement::Clear(StateKey::PanTiltLimit))
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
            Some(AppliedStateEffectRequirement::Set(StateKey::TallyMode))
        );
        assert_eq!(
            BuiltinCommand::DigitalZoom.classification().state_effect(),
            Some(AppliedStateEffectRequirement::Set(
                StateKey::ZoomDigitalZoom
            ))
        );

        assert_eq!(
            BuiltinCommand::MotionSyncPreset
                .classification()
                .state_effect(),
            Some(AppliedStateEffectRequirement::Set(
                StateKey::MotionSyncSpeed
            ))
        );
        assert_eq!(
            BuiltinCommand::MotionSyncMode
                .classification()
                .state_effect(),
            None
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
            assert_eq!(
                matches!(R::CONTROL_CLASS, crate::ControlClass::Urgent),
                command.classification().is_stop()
                    || matches!(command, BuiltinCommand::CommandCancel),
                "{command:?} urgent admission authority",
            );
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
            pin::<crate::command::image_wire::NoiseReduction2DCommand>(
                command,
                Timeout::Quick,
                Retry::Standard,
                p,
            );
        }
        for command in [
            BuiltinCommand::NoiseReduction3d,
            BuiltinCommand::NoiseReduction3dOff,
        ] {
            pin::<crate::command::image_wire::NoiseReduction3DCommand>(
                command,
                Timeout::Quick,
                Retry::Standard,
                p,
            );
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
        pin::<crate::command::system_wire::CommandCancelCommand>(
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
