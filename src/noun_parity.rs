//! Cross-surface parity gate for the three noun facades.
//!
//! [`crate::async_nouns`], [`crate::blocking::nouns`], and
//! [`crate::dynapi::nouns`] used to be three independent transcriptions of the
//! same closed ledger in [`crate::command::surface`].  Nothing in the type
//! system related them, and the published API snapshots diff each facade only
//! against its own baseline, so a method dropped from one surface,
//! reclassified, given a different argument list, or given a different
//! capability bound would otherwise have passed every gate.
//!
//! Issue #617 moved the rows themselves into [`crate::noun_table`], so the
//! three facades are now *generated* from one table and cannot disagree about a
//! generated row by construction.  That does not retire this gate; it moves
//! what it has to read:
//!
//! * The table is still an independent statement from the ledger, so its rows
//!   are compared against [`surface_entry`] by noun, method spelling, semantic
//!   return class, and required capability bound.
//! * A capability bound in the table is a bare marker name, so each static
//!   facade is still required to resolve that name to a real
//!   `crate::capabilities` trait — a shadow trait borrowing the name is still a
//!   failure.
//! * Each facade must actually consume the table for every ledger noun; a
//!   facade that quietly stopped generating a noun would otherwise lose its
//!   whole method set silently.
//! * Anything a facade still writes by hand inside a noun accessor or noun
//!   trait is a *residual*, and residuals are compared across the three
//!   surfaces exactly as every method used to be.  The declared exemption list
//!   is [`EXEMPT_HAND_WRITTEN`]; a residual that is not on it fails.
//!
//! Nothing here is derived from a hand-maintained `EXPECTED_` table: the noun
//! set, the method set, the classes, and the markers all come from
//! [`surface_entry`].  Rustdoc prose is out of scope here because the table
//! makes it a per-row attribute the three surfaces share.
//!
//! # Parsing contract
//!
//! The reader below is a small brace/paren-depth scanner over comment- and
//! literal-stripped source, not a set of column-exact needles.  Attributes
//! inside `accessor!` invocations, one-liner invocations, multi-line
//! signatures, and re-indentation are all legal input.  Anything the scanner
//! does *not* recognise is a hard panic naming the file and line: this gate
//! must never silently skip a method it cannot read, because a skipped method
//! is an ungated method.  The one macro invocation it will accept inside a noun
//! accessor or noun trait is `noun_table!`, whose rows it reads from the table
//! itself.
//!
//! # What the erased facade cannot be compared on
//!
//! The dynamic facade is object safe, so it carries no compile-time capability
//! bounds at all: `DynZoom::set_position` is a plain trait method whether or
//! not the profile behind it has [`TypedSupportSurface::DirectZoom`].  Its
//! capability enforcement is the run-time `validate_for_profile` check inside
//! [`crate::prepared::prepare_command`], which every
//! [`crate::dynapi::DynSessionCamera`] call reaches through the same
//! `AsyncCameraCore` the typed owner uses.
//!
//! Capability parity for the dynamic surface is therefore *not* verifiable from
//! the source text, and this gate deliberately does not pretend otherwise:
//! [`DynMethodShape`] has no bounds field to leave conveniently empty, and no
//! assertion here compares dynamic bounds against anything.  What is compared
//! for the dynamic surface is the method set, the receiver, the argument types,
//! and the semantic return class.  Any future gate on dynamic capability
//! enforcement has to be a behavioural test against a profile that lacks the
//! capability, not a source scan.

#![allow(clippy::panic)]

use std::collections::{BTreeMap, BTreeSet};

use crate::{
    capabilities::TypedSupportSurface,
    command::{
        semantics::{BuiltinCommand, BuiltinRequestClass},
        surface::{surface_entry, StaticMarkerRequirement, StaticNoun, StaticSurfaceDisposition},
    },
};

const ASYNC_SOURCE: &str = include_str!("async_nouns.rs");
const BLOCKING_SOURCE: &str = include_str!("blocking_nouns.rs");
const DYN_SOURCE: &str = include_str!("dynapi/nouns.rs");
const TABLE_SOURCE: &str = include_str!("noun_table.rs");
const TABLE_LABEL: &str = "src/noun_table.rs";

/// Motion is a safety/observation view rather than a ledger noun, so it has no
/// [`StaticNoun`] row; it is still one of the surfaces that must stay in step.
const MOTION_FACADE: NounFacade = NounFacade {
    accessor: "MotionAccessor",
    dyn_trait: "DynMotion",
    table_key: None,
};

/// The noun methods that are still written by hand on all three facades.
///
/// Every other noun method is generated from [`crate::noun_table`].  These four
/// are the motion safety and observation view: they reach `stop_all_motion`,
/// `is_moving` and `wait_until_idle` on the owner core rather than sending a
/// ledger request, so they share no shape with a table row.  They are exempt
/// from *generation*, not from parity — the gate below compares them across the
/// three surfaces exactly as it compares a generated row against the ledger,
/// and a residual that is not named here is a hard failure.
const EXEMPT_HAND_WRITTEN: &[(&str, &[&str])] = &[(
    "MotionAccessor",
    &[
        "is_moving",
        "is_moving_axes",
        "stop_all_motion",
        "wait_until_idle",
    ],
)];

/// Traits that may legally be implemented *for* an accessor type.
///
/// An `impl Trait for XxxAccessor` adds public methods to an accessor without
/// appearing in any inherent impl, which is exactly how an extension-trait
/// method escapes a parity gate that only reads inherent impls.  Rather than
/// try to inventory such methods across three facades, this gate bans them:
/// only the formatting impls below are allowed, and anything else is a hard
/// failure telling the author to put the method on the inherent impl of all
/// three surfaces instead.
const ALLOWED_ACCESSOR_TRAIT_IMPLS: &[&str] = &["std::fmt::Debug", "fmt::Debug"];

/// Module paths that really do hold the capability marker traits.
///
/// Bounds are compared by resolved full path, never by terminal segment, so a
/// same-named trait from anywhere else cannot silently stand in for a
/// capability gate.  A bound that resolves under one of these prefixes is the
/// real marker and is reduced to its bare name; a bound that resolves anywhere
/// else keeps its path and fails outright.
const CAPABILITY_MODULE_PATHS: &[&str] = &["crate::capabilities::"];

/// The profile trait every facade generic carries; never a capability gate.
///
/// The facades reach it through different re-exports, so both resolved paths
/// are listed.
const PROFILE_TRAIT_PATHS: &[&str] = &[
    "crate::profile::CompileTimeProfile",
    "crate::CompileTimeProfile",
];

/// The semantic return class a facade method advertises in its signature.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SurfaceClass {
    /// Returns no operation handle.
    Plain,
    /// Returns an `AppliedOnly` operation handle.
    AppliedOnly,
    /// Returns a `Targeted` operation handle.
    Targeted,
    /// Returns a decoded inquiry response; not a ledger command row.
    Inquiry,
}

/// The call shape every facade must reproduce for one method.
#[derive(Debug, Clone, PartialEq, Eq)]
struct MethodShape {
    class: SurfaceClass,
    receiver: String,
    arguments: Vec<String>,
}

/// One static facade method: its call shape and the bounds `P` carries.
#[derive(Debug, Clone, PartialEq, Eq)]
struct MethodFacts {
    shape: MethodShape,
    bounds: BTreeSet<String>,
}

/// One dynamic facade method.
///
/// There is deliberately no bounds field: see the module documentation on what
/// the erased facade cannot be compared on.
type DynMethodShape = MethodShape;

/// One parsed dynamic facade: hand-written method shapes per noun trait, and
/// the [`crate::noun_table`] nouns each trait declaration generates.
type DynFacade = (
    BTreeMap<String, BTreeMap<String, DynMethodShape>>,
    BTreeMap<String, BTreeSet<String>>,
);

/// One parsed static facade.
#[derive(Debug, Default)]
struct StaticFacade {
    /// Accessor type name to hand-written method name to facts.
    accessors: BTreeMap<String, BTreeMap<String, MethodFacts>>,
    /// Accessor type name to the capability gate the type itself imposes.
    gates: BTreeMap<String, BTreeSet<String>>,
    /// Accessor type name to the [`noun_table`] nouns its impl generates.
    ///
    /// [`noun_table`]: crate::noun_table
    generated: BTreeMap<String, BTreeSet<String>>,
}

/// One parsed row of [`crate::noun_table`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct TableRow {
    /// The semantic return class the row declares.
    class: SurfaceClass,
    /// Normalized argument types, in declaration order.
    arguments: Vec<String>,
    /// Bare capability marker names the row puts on the static facades.
    gates: BTreeSet<String>,
}

/// The name each facade gives one noun.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct NounFacade {
    accessor: &'static str,
    dyn_trait: &'static str,
    /// The `noun_table!` arm the facades generate this noun from, if any.
    table_key: Option<&'static str>,
}

/// One ledger row projected onto the facades.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct LedgerFacts {
    class: SurfaceClass,
    marker: Option<&'static str>,
}

/// Maps a ledger noun onto the type name each facade uses for it.
const fn noun_facade(noun: StaticNoun) -> NounFacade {
    let (accessor, dyn_trait) = match noun {
        StaticNoun::Power => ("PowerAccessor", "DynPower"),
        StaticNoun::Zoom => ("ZoomAccessor", "DynZoom"),
        StaticNoun::System => ("SystemAccessor", "DynSystem"),
        StaticNoun::PanTilt => ("PanTiltAccessor", "DynPanTilt"),
        StaticNoun::Focus => ("FocusAccessor", "DynFocus"),
        StaticNoun::Exposure => ("ExposureAccessor", "DynExposure"),
        StaticNoun::WhiteBalance => ("WhiteBalanceAccessor", "DynWhiteBalance"),
        StaticNoun::Image => ("ImageAccessor", "DynImage"),
        StaticNoun::Presets => ("PresetsAccessor", "DynPresets"),
        StaticNoun::Tally => ("TallyAccessor", "DynTally"),
        StaticNoun::NdFilter => ("NdFilterAccessor", "DynNdFilter"),
        StaticNoun::MotionSync => ("MotionSyncAccessor", "DynMotionSync"),
        StaticNoun::Menu => ("MenuAccessor", "DynMenu"),
        StaticNoun::Advanced => ("AdvancedAccessor", "DynAdvanced"),
    };
    NounFacade {
        accessor,
        dyn_trait,
        table_key: Some(noun_table_key(noun)),
    }
}

/// Maps a ledger noun onto the arm of [`crate::noun_table`] that carries it.
///
/// The match is exhaustive on purpose: a new noun cannot be added without
/// deciding which table arm the facades generate it from.
#[must_use]
pub(crate) const fn noun_table_key(noun: StaticNoun) -> &'static str {
    match noun {
        StaticNoun::Power => "Power",
        StaticNoun::Zoom => "Zoom",
        StaticNoun::System => "System",
        StaticNoun::PanTilt => "PanTilt",
        StaticNoun::Focus => "Focus",
        StaticNoun::Exposure => "Exposure",
        StaticNoun::WhiteBalance => "WhiteBalance",
        StaticNoun::Image => "Image",
        StaticNoun::Presets => "Presets",
        StaticNoun::Tally => "Tally",
        StaticNoun::NdFilter => "NdFilter",
        StaticNoun::MotionSync => "MotionSync",
        StaticNoun::Menu => "Menu",
        StaticNoun::Advanced => "Advanced",
    }
}

/// Maps an object-safe noun trait name onto its [`crate::noun_table`] arm.
///
/// Only the erased facade's own inventory gate needs this direction.
#[cfg(feature = "dyn-api")]
#[must_use]
pub(crate) fn dyn_trait_noun_key(dyn_trait: &str) -> &'static str {
    for command in BuiltinCommand::ALL {
        if let StaticSurfaceDisposition::Noun { noun, .. } = surface_entry(*command).disposition {
            if noun_facade(noun).dyn_trait == dyn_trait {
                return noun_table_key(noun);
            }
        }
    }
    panic!("{dyn_trait} is not a ledger noun trait")
}

/// Reads [`crate::noun_table`] into noun/method rows.
///
/// The table is the one place a noun method's name, arguments, capability bound
/// and return class are written down, so this reader is what lets every other
/// gate keep checking those facts without reading three generated facades.
#[must_use]
pub(crate) fn table_surface() -> BTreeMap<String, BTreeMap<String, TableRow>> {
    let cleaned = clean_source(TABLE_SOURCE, TABLE_LABEL);
    let lines: Vec<&str> = cleaned.lines().collect();
    let mut table: BTreeMap<String, BTreeMap<String, TableRow>> = BTreeMap::new();

    let mut index = 0;
    while index < lines.len() {
        let trimmed = lines[index].trim();
        let Some(noun) = table_arm_noun(trimmed) else {
            index += 1;
            continue;
        };
        // The rows sit inside the `$consumer! { .. }` call this arm expands to.
        let arm_end = block_end(&lines, index, TABLE_LABEL);
        let mut cursor = index + 1;
        while cursor < arm_end && !lines[cursor].trim().starts_with("$consumer!") {
            cursor += 1;
        }
        assert!(
            cursor < arm_end,
            "{TABLE_LABEL}:{}: the {noun} arm hands nothing to its consumer",
            index + 1,
        );
        let rows_end = block_end(&lines, cursor, TABLE_LABEL);
        let mut rows = BTreeMap::new();
        let mut row_line = cursor + 1;
        let last = rows_end.saturating_sub(1);
        while row_line < last {
            if lines[row_line].trim().is_empty() {
                row_line += 1;
                continue;
            }
            let (text, _, next) = join_until(&lines, row_line, &[';'], TABLE_LABEL);
            let (method, row) = parse_table_row(&text, row_line + 1);
            assert!(
                rows.insert(method.clone(), row).is_none(),
                "{TABLE_LABEL}:{}: duplicate {noun} row {method}",
                row_line + 1,
            );
            row_line = next;
        }
        assert!(
            table.insert(noun.clone(), rows).is_none(),
            "{TABLE_LABEL}:{}: duplicate table arm for {noun}",
            index + 1,
        );
        index = arm_end;
    }

    assert!(!table.is_empty(), "{TABLE_LABEL}: no noun arms were read");
    table
}

/// Returns the noun a `(<Noun> => $consumer:ident) => {` arm header declares.
fn table_arm_noun(trimmed: &str) -> Option<String> {
    let rest = trimmed.strip_prefix('(')?;
    let noun = leading_identifier(rest);
    if noun.is_empty() {
        return None;
    }
    let rest = rest[noun.len()..].trim_start();
    let rest = rest.strip_prefix("=> $consumer:ident)")?;
    rest.trim_start().starts_with("=>").then(|| noun.to_owned())
}

/// Parses one table row into its method name and facts.
fn parse_table_row(text: &str, line: usize) -> (String, TableRow) {
    let origin = format!("{TABLE_LABEL}:{line}");
    let text = text.trim();
    let kind = leading_identifier(text);
    let class = match kind {
        "inquiry" => SurfaceClass::Inquiry,
        "plain" => SurfaceClass::Plain,
        "applied" => SurfaceClass::AppliedOnly,
        "targeted" => SurfaceClass::Targeted,
        other => panic!("{origin}: unknown row kind {other:?} in {text:?}"),
    };

    let rest = text[kind.len()..].trim_start();
    let method = leading_identifier(rest).to_owned();
    assert!(!method.is_empty(), "{origin}: unnamed row in {text:?}");
    let rest = rest[method.len()..].trim_start();
    assert!(
        rest.starts_with('('),
        "{origin}: no argument list in row {text:?}",
    );
    let (argument_text, rest) = split_parens(rest, &origin);
    let arguments: Vec<String> = split_top_level(argument_text, ',')
        .into_iter()
        .filter(|argument| !argument.trim().is_empty())
        .map(|argument| {
            let Some((_, kind)) = argument.split_once(':') else {
                panic!("{origin}: unrecognized argument {argument:?} in {text:?}")
            };
            normalize_type(kind)
        })
        .collect();

    // What is left is `[-> Response] [where A + B] = <request>`.  The request
    // is not compared against anything, so it is only skipped past.
    let head = match split_at_row_assignment(rest) {
        Some((head, _)) => head,
        None => panic!("{origin}: row {text:?} has no `= <request>`"),
    };
    let (head, gates) = match find_keyword(head, "where") {
        Some(offset) => (
            &head[..offset],
            split_top_level(&head[offset + "where".len()..], '+')
                .into_iter()
                .map(|gate| gate.trim().to_owned())
                .filter(|gate| !gate.is_empty())
                .collect(),
        ),
        None => (head, BTreeSet::new()),
    };
    let head = head.trim();
    if class == SurfaceClass::Inquiry {
        assert!(
            head.starts_with("->"),
            "{origin}: inquiry row {text:?} declares no response type",
        );
        assert!(
            arguments.is_empty(),
            "{origin}: inquiry row {text:?} takes arguments",
        );
    } else {
        assert!(
            head.is_empty(),
            "{origin}: unrecognized row tail {head:?} in {text:?}",
        );
    }

    (
        method,
        TableRow {
            class,
            arguments,
            gates,
        },
    )
}

/// Splits a row tail at the `=` that introduces its request expression.
fn split_at_row_assignment(text: &str) -> Option<(&str, &str)> {
    let mut depth = 0_i32;
    let mut previous = ' ';
    for (index, ch) in text.char_indices() {
        match ch {
            '(' | '[' | '{' | '<' => depth += 1,
            ')' | ']' | '}' => depth -= 1,
            '>' if previous != '-' => depth -= 1,
            '=' if depth == 0 && previous != '-' => {
                return Some((&text[..index], &text[index + 1..]))
            }
            _ => {}
        }
        previous = ch;
    }
    None
}

/// Returns the [`crate::noun_table`] nouns `source` generates with `consumer`.
#[must_use]
pub(crate) fn consumed_nouns(
    source: &str,
    label: &'static str,
    consumer: &str,
) -> BTreeSet<String> {
    let cleaned = clean_source(source, label);
    let lines: Vec<&str> = cleaned.lines().collect();
    let skip = skipped_lines(&lines, label);
    let mut nouns = BTreeSet::new();
    for (index, line) in lines.iter().enumerate() {
        if skip[index] {
            continue;
        }
        let trimmed = line.trim();
        let Some(rest) = trimmed.strip_prefix("noun_table!") else {
            continue;
        };
        let inner = split_parens(rest.trim_start(), label).0;
        let Some((noun, named)) = inner.split_once("=>") else {
            panic!("{label}:{}: unrecognized noun_table! invocation", index + 1)
        };
        if named.trim() == consumer {
            assert!(
                nouns.insert(noun.trim().to_owned()),
                "{label}:{}: {consumer} generates {} twice",
                index + 1,
                noun.trim(),
            );
        }
    }
    nouns
}

/// Maps a typed support gate onto the marker trait the facades bound `P` with.
///
/// The match is exhaustive on purpose: a new typed surface cannot be added
/// without deciding which marker trait gates it on the noun facades.
const fn typed_marker_trait(surface: TypedSupportSurface) -> &'static str {
    match surface {
        TypedSupportSurface::DirectZoom => "HasDirectZoom",
        TypedSupportSurface::DigitalZoomToggle => "HasDigitalZoomToggle",
        TypedSupportSurface::DigitalZoomRange => "HasDigitalZoomRange",
        TypedSupportSurface::IrisControl => "HasIrisControl",
        TypedSupportSurface::OnePushFocus => "HasOnePushFocus",
        TypedSupportSurface::PtzOpticsSnapFocus => "HasPtzOpticsSnapFocus",
        TypedSupportSurface::FocusLock => "HasFocusLock",
        TypedSupportSurface::PushAutoFocus => "HasPushAutoFocus",
        TypedSupportSurface::FocusZone => "HasFocusZone",
        TypedSupportSurface::AutoFocusSensitivity => "HasAutoFocusSensitivity",
        TypedSupportSurface::FocusNearLimitInquiry => "HasFocusNearLimitInquiry",
        TypedSupportSurface::BacklightCompensation => "HasBacklightCompensation",
        TypedSupportSurface::WideDynamicRange => "HasWideDynamicRange",
        TypedSupportSurface::ExposureCompensation => "HasExposureCompensation",
        TypedSupportSurface::BrightnessControl => "HasBrightnessControl",
        TypedSupportSurface::OnePushWhiteBalance => "HasOnePushWhiteBalance",
        TypedSupportSurface::AutoTrackingWhiteBalance => "HasAutoTrackingWhiteBalance",
        TypedSupportSurface::AutoWhiteBalanceSensitivity => "HasAutoWhiteBalanceSensitivity",
        TypedSupportSurface::ColorTemperature => "HasColorTemperature",
        TypedSupportSurface::RgbGain => "HasRgbGain",
        TypedSupportSurface::RgbTuning => "HasRgbTuning",
        TypedSupportSurface::ImageFlip => "HasImageFlip",
        TypedSupportSurface::ImageMirror => "HasImageMirror",
        TypedSupportSurface::CombinedImageFlip => "HasCombinedImageFlip",
        TypedSupportSurface::ContrastControl => "HasContrastControl",
        TypedSupportSurface::SharpnessControl => "HasSharpnessControl",
        TypedSupportSurface::SaturationControl => "HasSaturationControl",
        TypedSupportSurface::HueControl => "HasHueControl",
        TypedSupportSurface::LuminanceControl => "HasLuminanceControl",
        TypedSupportSurface::GammaControl => "HasGammaControl",
        TypedSupportSurface::NoiseReduction => "HasNoiseReduction",
        TypedSupportSurface::NoiseReduction2D => "HasNoiseReduction2D",
        TypedSupportSurface::NoiseReduction3D => "HasNoiseReduction3D",
        TypedSupportSurface::PictureEffect => "HasPictureEffect",
        TypedSupportSurface::Tally => "HasTally",
        TypedSupportSurface::DirectMenu => "HasDirectMenuControl",
        TypedSupportSurface::NdFilter => "HasNdFilter",
        TypedSupportSurface::VariableSpeed => "HasVariableSpeed",
        TypedSupportSurface::MotionSync => "HasMotionSync",
    }
}

/// Projects one ledger marker requirement onto a marker trait name.
const fn marker_trait(marker: StaticMarkerRequirement) -> Option<&'static str> {
    match marker {
        StaticMarkerRequirement::None => None,
        StaticMarkerRequirement::Profile(name) => Some(name),
        StaticMarkerRequirement::Typed(surface) => Some(typed_marker_trait(surface)),
    }
}

/// Projects one semantic classification onto the facade return class.
const fn ledger_class(class: BuiltinRequestClass) -> SurfaceClass {
    match class {
        BuiltinRequestClass::Plain { .. } => SurfaceClass::Plain,
        BuiltinRequestClass::AppliedOnly { .. } => SurfaceClass::AppliedOnly,
        BuiltinRequestClass::Targeted { .. } => SurfaceClass::Targeted,
    }
}

/// Every capability marker trait the closed ledger can require.
///
/// A bound observed on a facade that is not in this set is either a typo, a
/// trait that is not a capability gate, or a shadow trait standing in for one;
/// all three are failures rather than something to normalise away.
fn known_markers() -> BTreeSet<&'static str> {
    let mut markers: BTreeSet<&'static str> = TypedSupportSurface::ALL
        .iter()
        .copied()
        .map(typed_marker_trait)
        .collect();
    for command in BuiltinCommand::ALL {
        if let StaticSurfaceDisposition::Noun { marker, .. } = surface_entry(*command).disposition {
            markers.extend(marker_trait(marker));
        }
    }
    markers
}

/// Returns the target-facing ledger rows grouped by noun, in ledger order.
fn ledger_surface() -> Vec<(NounFacade, BTreeMap<&'static str, LedgerFacts>)> {
    let mut order = Vec::new();
    let mut rows: BTreeMap<&'static str, BTreeMap<&'static str, LedgerFacts>> = BTreeMap::new();

    for command in BuiltinCommand::ALL {
        let entry = surface_entry(*command);
        let StaticSurfaceDisposition::Noun {
            noun,
            method,
            marker,
        } = entry.disposition
        else {
            continue;
        };
        let facade = noun_facade(noun);
        let facts = LedgerFacts {
            class: ledger_class(entry.class),
            marker: marker_trait(marker),
        };
        let noun_rows = rows.entry(facade.accessor).or_insert_with(|| {
            order.push(facade);
            BTreeMap::new()
        });
        if let Some(previous) = noun_rows.insert(method, facts) {
            assert_eq!(
                previous, facts,
                "ledger rows collapsed onto {}::{method} disagree",
                facade.accessor,
            );
        }
    }

    order
        .into_iter()
        .map(|facade| {
            let noun_rows = rows
                .remove(facade.accessor)
                .unwrap_or_else(|| panic!("missing ledger rows for {}", facade.accessor));
            (facade, noun_rows)
        })
        .collect()
}

// ---------------------------------------------------------------------------
// Source scanning primitives
// ---------------------------------------------------------------------------

/// Blanks comments and literal contents while preserving the line structure.
///
/// Every subsequent scan runs over the result, so a brace, paren, semicolon,
/// or keyword inside a comment or a string can never steer the parser.
fn clean_source(source: &str, label: &str) -> String {
    assert!(
        !source.contains("r#\""),
        "{label}: raw strings are not supported by the parity scanner",
    );

    let chars: Vec<char> = source.chars().collect();
    let mut out: Vec<char> = Vec::with_capacity(chars.len());
    let mut index = 0;

    fn blank(ch: char) -> char {
        if ch == '\n' {
            '\n'
        } else {
            ' '
        }
    }

    while index < chars.len() {
        let current = chars[index];
        let next = chars.get(index + 1).copied();
        match (current, next) {
            ('/', Some('/')) => {
                while index < chars.len() && chars[index] != '\n' {
                    out.push(' ');
                    index += 1;
                }
            }
            ('/', Some('*')) => {
                let mut depth = 1_usize;
                out.push(' ');
                out.push(' ');
                index += 2;
                while index < chars.len() && depth > 0 {
                    if chars[index] == '*' && chars.get(index + 1) == Some(&'/') {
                        depth -= 1;
                        out.push(' ');
                        out.push(' ');
                        index += 2;
                    } else if chars[index] == '/' && chars.get(index + 1) == Some(&'*') {
                        depth += 1;
                        out.push(' ');
                        out.push(' ');
                        index += 2;
                    } else {
                        out.push(blank(chars[index]));
                        index += 1;
                    }
                }
                assert_eq!(depth, 0, "{label}: unterminated block comment");
            }
            ('"', _) => {
                out.push('"');
                index += 1;
                let mut closed = false;
                while index < chars.len() {
                    let ch = chars[index];
                    if ch == '\\' {
                        out.push(' ');
                        index += 1;
                        if index < chars.len() {
                            out.push(blank(chars[index]));
                            index += 1;
                        }
                        continue;
                    }
                    if ch == '"' {
                        out.push('"');
                        index += 1;
                        closed = true;
                        break;
                    }
                    out.push(blank(ch));
                    index += 1;
                }
                assert!(closed, "{label}: unterminated string literal");
            }
            ('\'', _) => {
                // `'a` is a lifetime, `'x'` and `'\n'` are character literals.
                let literal = match next {
                    Some('\\') => true,
                    Some(_) => chars.get(index + 2) == Some(&'\''),
                    None => false,
                };
                if literal {
                    out.push(' ');
                    index += 1;
                    while index < chars.len() {
                        let ch = chars[index];
                        if ch == '\\' {
                            out.push(' ');
                            index += 1;
                            if index < chars.len() {
                                out.push(blank(chars[index]));
                                index += 1;
                            }
                            continue;
                        }
                        out.push(' ');
                        index += 1;
                        if ch == '\'' {
                            break;
                        }
                    }
                } else {
                    out.push('\'');
                    index += 1;
                }
            }
            _ => {
                out.push(current);
                index += 1;
            }
        }
    }

    out.into_iter().collect()
}

/// Marks the lines of `#[cfg(test)]` items and `macro_rules!` bodies.
///
/// Both regions describe the surface rather than declare it: the in-file
/// inventory tests name every accessor as a string literal and the macro
/// bodies contain `$name`-shaped method templates.  Reading either as surface
/// is how a gate ends up asserting that a file contains its own test data.
fn skipped_lines(lines: &[&str], label: &str) -> Vec<bool> {
    masked_lines(lines, label, true)
}

/// Marks the lines belonging to `#[cfg(test)]` items, and optionally to
/// `macro_rules!` bodies.
fn masked_lines(lines: &[&str], label: &str, macros: bool) -> Vec<bool> {
    let mut skip = vec![false; lines.len()];
    let mut index = 0;
    while index < lines.len() {
        let trimmed = lines[index].trim();
        if trimmed == "#[cfg(test)]" || (macros && trimmed.starts_with("macro_rules!")) {
            let end = block_end(lines, index, label);
            for entry in skip.iter_mut().take(end).skip(index) {
                *entry = true;
            }
            index = end;
            continue;
        }
        index += 1;
    }
    skip
}

/// Returns `source` with every `#[cfg(test)]` item blanked out.
///
/// A surface file names its own accessors and method spellings as string
/// literals inside its in-file inventory tests, so a `contains` check over the
/// whole file can be satisfied by the test data instead of by the surface it is
/// supposed to be checking.  Every positive source gate must therefore read the
/// declaration region only.  Line numbering is preserved so that a failure
/// still points at the right place in the real file.
pub(crate) fn without_test_modules(source: &str, label: &str) -> String {
    let cleaned = clean_source(source, label);
    let cleaned_lines: Vec<&str> = cleaned.lines().collect();
    let skip = masked_lines(&cleaned_lines, label, false);
    source
        .lines()
        .zip(skip)
        .map(|(line, skip)| if skip { "" } else { line })
        .collect::<Vec<_>>()
        .join("\n")
}

/// Returns the index just past the line closing the brace block at `start`.
fn block_end(lines: &[&str], start: usize, label: &str) -> usize {
    let mut depth = 0_i32;
    let mut opened = false;
    for (offset, line) in lines.iter().enumerate().skip(start) {
        for ch in line.chars() {
            match ch {
                '{' => {
                    depth += 1;
                    opened = true;
                }
                '}' => depth -= 1,
                _ => {}
            }
        }
        if opened && depth <= 0 {
            return offset + 1;
        }
    }
    panic!("{label}:{}: unterminated block", start + 1)
}

/// Joins lines from `start` until the first terminator at delimiter depth zero.
///
/// Returns the joined text with the terminator excluded, the terminator that
/// matched, and the index of the line to continue from.  Depth tracking is
/// what keeps `[u8; 4]` from ending a trait-method signature and what lets a
/// signature span as many lines as `rustfmt` wants.
fn join_until(
    lines: &[&str],
    start: usize,
    terminators: &[char],
    label: &str,
) -> (String, char, usize) {
    let mut text = String::new();
    let mut depth = 0_i32;
    let mut index = start;

    while index < lines.len() {
        let line = lines[index];
        let mut cut = None;
        for (offset, ch) in line.char_indices() {
            if depth == 0 && terminators.contains(&ch) {
                cut = Some((offset, ch));
                break;
            }
            match ch {
                '(' | '[' | '{' => depth += 1,
                ')' | ']' | '}' => depth -= 1,
                _ => {}
            }
        }
        let (fragment, terminator) = match cut {
            Some((offset, ch)) => (&line[..offset], Some(ch)),
            None => (line, None),
        };
        let fragment = fragment.trim();
        if !fragment.is_empty() {
            if !text.is_empty() {
                text.push(' ');
            }
            text.push_str(fragment);
        }
        index += 1;
        if let Some(terminator) = terminator {
            return (collapse_whitespace(&text), terminator, index);
        }
    }

    panic!(
        "{label}:{}: no {terminators:?} terminator before end of file",
        start + 1
    )
}

/// Returns the index just past the item starting at `start`.
fn item_end(lines: &[&str], start: usize, label: &str) -> usize {
    let (_, terminator, next) = join_until(lines, start, &['{', ';'], label);
    if terminator == ';' {
        next
    } else {
        block_end(lines, start, label)
    }
}

/// Collapses runs of whitespace into single spaces.
fn collapse_whitespace(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for ch in text.chars() {
        if ch.is_whitespace() {
            if !out.ends_with(' ') {
                out.push(' ');
            }
        } else {
            out.push(ch);
        }
    }
    out.trim().to_owned()
}

/// Returns the leading identifier (or path) of `text`.
fn leading_identifier(text: &str) -> &str {
    let end = text
        .find(|ch: char| !ch.is_alphanumeric() && ch != '_')
        .unwrap_or(text.len());
    &text[..end]
}

/// Splits `text`, which must start with `(`, into the paren body and rest.
fn split_parens<'a>(text: &'a str, label: &str) -> (&'a str, &'a str) {
    let mut depth = 0_usize;
    for (index, ch) in text.char_indices() {
        match ch {
            '(' => depth += 1,
            ')' => {
                depth -= 1;
                if depth == 0 {
                    return (&text[1..index], &text[index + 1..]);
                }
            }
            _ => {}
        }
    }
    panic!("{label}: unbalanced parentheses in {text:?}")
}

/// Splits `text`, which must start with `{`, into the brace body and rest.
fn split_braces<'a>(text: &'a str, label: &str) -> (&'a str, &'a str) {
    let mut depth = 0_usize;
    for (index, ch) in text.char_indices() {
        match ch {
            '{' => depth += 1,
            '}' => {
                depth -= 1;
                if depth == 0 {
                    return (&text[1..index], &text[index + 1..]);
                }
            }
            _ => {}
        }
    }
    panic!("{label}: unbalanced braces in {text:?}")
}

/// Splits `text`, which must start with `<`, into the bracket body and rest.
fn split_generics<'a>(text: &'a str, label: &str) -> (&'a str, &'a str) {
    let mut depth = 0_usize;
    let mut previous = ' ';
    for (index, ch) in text.char_indices() {
        match ch {
            '<' => depth += 1,
            '>' if previous != '-' => {
                depth -= 1;
                if depth == 0 {
                    return (&text[1..index], &text[index + 1..]);
                }
            }
            _ => {}
        }
        previous = ch;
    }
    panic!("{label}: unbalanced generics in {text:?}")
}

/// Splits `text` on `separator` occurrences that sit at delimiter depth zero.
fn split_top_level(text: &str, separator: char) -> Vec<String> {
    let mut parts = Vec::new();
    let mut current = String::new();
    let mut depth = 0_i32;
    let mut previous = ' ';
    for ch in text.chars() {
        match ch {
            '(' | '[' | '{' | '<' => depth += 1,
            ')' | ']' | '}' => depth -= 1,
            '>' if previous != '-' => depth -= 1,
            _ => {}
        }
        if ch == separator && depth == 0 {
            parts.push(current.trim().to_owned());
            current = String::new();
        } else {
            current.push(ch);
        }
        previous = ch;
    }
    parts.push(current.trim().to_owned());
    parts
}

/// Returns the byte offset of `keyword` used as a word at depth zero.
fn find_keyword(text: &str, keyword: &str) -> Option<usize> {
    let bytes = text.as_bytes();
    let mut depth = 0_i32;
    let mut previous = ' ';
    for (index, ch) in text.char_indices() {
        match ch {
            '(' | '[' | '{' | '<' => depth += 1,
            ')' | ']' | '}' => depth -= 1,
            '>' if previous != '-' => depth -= 1,
            _ => {}
        }
        previous = ch;
        if depth != 0 || !text[index..].starts_with(keyword) {
            continue;
        }
        let before_ok = index == 0 || !is_word_byte(bytes[index - 1]);
        let after = index + keyword.len();
        let after_ok = after >= bytes.len() || !is_word_byte(bytes[after]);
        if before_ok && after_ok {
            return Some(index);
        }
    }
    None
}

/// Returns whether `byte` can appear inside a Rust identifier.
const fn is_word_byte(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || byte == b'_'
}

/// Strips any leading `#[...]` attributes from `text`.
fn strip_attributes<'a>(text: &'a str, label: &str) -> &'a str {
    let mut rest = text.trim_start();
    while let Some(after) = rest.strip_prefix("#[") {
        let mut depth = 1_i32;
        let mut end = None;
        for (index, ch) in after.char_indices() {
            match ch {
                '[' => depth += 1,
                ']' => {
                    depth -= 1;
                    if depth == 0 {
                        end = Some(index);
                        break;
                    }
                }
                _ => {}
            }
        }
        let Some(end) = end else {
            panic!("{label}: unbalanced attribute in {text:?}")
        };
        rest = after[end + 1..].trim_start();
    }
    rest
}

/// Normalizes a type, receiver, or bound for cross-surface comparison.
///
/// Lifetimes are dropped because the blocking facade threads a session
/// lifetime the async facade does not have; everything else is preserved so
/// that `f32` and `f64`, or `&self` and `self`, cannot compare equal.
fn normalize_type(text: &str) -> String {
    let chars: Vec<char> = text.chars().collect();
    let mut spaced = String::with_capacity(text.len());
    let mut index = 0;
    while index < chars.len() {
        let ch = chars[index];
        if ch == '\'' {
            index += 1;
            while index < chars.len() && (chars[index].is_alphanumeric() || chars[index] == '_') {
                index += 1;
            }
            continue;
        }
        if ch.is_whitespace() {
            if !spaced.ends_with(' ') {
                spaced.push(' ');
            }
            index += 1;
            continue;
        }
        spaced.push(ch);
        index += 1;
    }

    const TIGHT: &[char] = &['<', '>', '(', ')', '[', ']', ',', ':', '&', ';'];
    let spaced: Vec<char> = spaced.trim().chars().collect();
    let mut tight = String::with_capacity(spaced.len());
    for (index, ch) in spaced.iter().enumerate() {
        if *ch == ' ' {
            let previous = tight.chars().last();
            let next = spaced.get(index + 1).copied();
            if previous.is_some_and(|ch| TIGHT.contains(&ch))
                || next.is_some_and(|ch| TIGHT.contains(&ch))
            {
                continue;
            }
        }
        tight.push(*ch);
    }

    // Dropping a lifetime can leave an empty generic slot behind.
    let mut result = tight;
    loop {
        let collapsed = result
            .replace("<,", "<")
            .replace(",,", ",")
            .replace(",>", ">")
            .replace("(,", "(")
            .replace(",)", ")")
            .replace("<>", "");
        if collapsed == result {
            return collapsed;
        }
        result = collapsed;
    }
}

// ---------------------------------------------------------------------------
// Capability bounds
// ---------------------------------------------------------------------------

/// Maps every name a source imports onto the path it was imported from.
///
/// Path resolution is what lets the three facades be compared by full path
/// rather than by terminal segment.  `types::ZoomSpeed` and
/// `crate::types::ZoomSpeed` are the same type and must compare equal;
/// `shadow::HasDirectZoom` resolves to nothing and stays qualified, so it can
/// never collapse onto the capability marker whose name it borrowed.
fn use_map(lines: &[&str], skip: &[bool], label: &str) -> BTreeMap<String, String> {
    let mut imports = BTreeMap::new();
    let mut index = 0;
    while index < lines.len() {
        if skip[index] || !lines[index].trim_start().starts_with("use ") {
            index += 1;
            continue;
        }
        let (text, _, next) = join_until(lines, index, &[';'], label);
        let tree = text.trim_start().trim_start_matches("use ");
        expand_use(tree, "", label, &mut imports);
        index = next;
    }
    imports
}

/// Expands one `use` tree into `name -> path` leaves.
fn expand_use(text: &str, prefix: &str, label: &str, imports: &mut BTreeMap<String, String>) {
    let text = text.trim();
    if text.is_empty() {
        return;
    }
    if let Some(open) = text.find('{') {
        let head = &text[..open];
        let (inner, rest) = split_braces(&text[open..], label);
        assert!(
            rest.trim().is_empty(),
            "{label}: unrecognized use tree {text:?}",
        );
        let prefix = format!("{prefix}{head}");
        for part in split_top_level(inner, ',') {
            expand_use(&part, &prefix, label, imports);
        }
        return;
    }

    assert!(
        !text.contains('*'),
        "{label}: glob import {text:?} defeats path resolution",
    );
    let (path, name) = match text.split_once(" as ") {
        Some((path, alias)) => (format!("{prefix}{}", path.trim()), alias.trim().to_owned()),
        None if text == "self" => {
            let path = prefix.trim_end_matches("::").to_owned();
            let name = path.rsplit("::").next().unwrap_or(&path).to_owned();
            (path, name)
        }
        None => (
            format!("{prefix}{text}"),
            text.rsplit("::").next().unwrap_or(text).to_owned(),
        ),
    };
    imports.insert(name, path);
}

/// Rewrites every path in `text` through the source's own imports.
fn resolve_paths(text: &str, imports: &BTreeMap<String, String>) -> String {
    let chars: Vec<char> = text.chars().collect();
    let mut out = String::with_capacity(text.len());
    let mut index = 0;
    while index < chars.len() {
        let ch = chars[index];
        if !ch.is_alphabetic() && ch != '_' {
            out.push(ch);
            index += 1;
            continue;
        }
        let start = index;
        while index < chars.len() {
            let ch = chars[index];
            if ch.is_alphanumeric() || ch == '_' {
                index += 1;
            } else if ch == ':' && chars.get(index + 1) == Some(&':') {
                index += 2;
            } else {
                break;
            }
        }
        let path: String = chars[start..index].iter().collect();
        let (root, rest) = path
            .split_once("::")
            .map_or((path.as_str(), ""), |(root, rest)| (root, rest));
        match imports.get(root) {
            Some(resolved) if rest.is_empty() => out.push_str(resolved),
            Some(resolved) => {
                out.push_str(resolved);
                out.push_str("::");
                out.push_str(rest);
            }
            None => out.push_str(&path),
        }
    }
    out
}

/// Normalizes an `A + path::B` bound list into capability marker names.
///
/// Bounds are compared by full path: only the documented capability module
/// paths are stripped, so `shadow::HasDirectZoom` stays `shadow::HasDirectZoom`
/// and fails the known-marker check instead of collapsing onto the real gate.
fn marker_bounds(list: &str, imports: &BTreeMap<String, String>, label: &str) -> BTreeSet<String> {
    let mut bounds = BTreeSet::new();
    for bound in split_top_level(list, '+') {
        let bound = resolve_paths(&normalize_type(bound.trim_end_matches(',')), imports);
        if bound.is_empty() || PROFILE_TRAIT_PATHS.contains(&bound.as_str()) {
            continue;
        }
        let name = CAPABILITY_MODULE_PATHS
            .iter()
            .find_map(|prefix| bound.strip_prefix(prefix))
            .unwrap_or_else(|| {
                panic!(
                    "{label}: capability bound {bound:?} does not resolve into a documented \
                     capability module path",
                )
            });
        bounds.insert(name.to_owned());
    }
    bounds
}

/// Unions the bounds placed on the profile parameter `P` by every predicate.
///
/// Every `P:` predicate is read, not just the first: `where P: A, P: B` places
/// both `A` and `B` on `P` and dropping either one hides a capability change.
fn profile_bounds(
    predicates: &str,
    imports: &BTreeMap<String, String>,
    label: &str,
) -> BTreeSet<String> {
    let mut bounds = BTreeSet::new();
    for predicate in split_top_level(predicates, ',') {
        let Some((name, list)) = predicate.split_once(':') else {
            continue;
        };
        if name.trim() == "P" {
            bounds.extend(marker_bounds(list, imports, label));
        }
    }
    bounds
}

// ---------------------------------------------------------------------------
// Signatures
// ---------------------------------------------------------------------------

/// One parsed function signature.
#[derive(Debug, Clone)]
struct Signature {
    name: String,
    receiver: String,
    arguments: Vec<String>,
    returns: String,
    where_clause: String,
}

/// Parses a joined `fn` signature, with the body brace or `;` already removed.
fn parse_signature(
    text: &str,
    imports: &BTreeMap<String, String>,
    label: &str,
    line: usize,
) -> Signature {
    let origin = format!("{label}:{line}");
    let Some(offset) = find_keyword(text, "fn") else {
        panic!("{origin}: no `fn` keyword in signature {text:?}")
    };
    let rest = text[offset + 2..].trim_start();
    let name = leading_identifier(rest).to_owned();
    assert!(!name.is_empty(), "{origin}: unnamed function in {text:?}");

    let rest = rest[name.len()..].trim_start();
    let rest = if rest.starts_with('<') {
        split_generics(rest, &origin).1.trim_start()
    } else {
        rest
    };

    assert!(
        rest.starts_with('('),
        "{origin}: no argument list in signature {text:?}",
    );
    let (arguments_text, rest) = split_parens(rest, &origin);
    let rest = rest.trim_start();

    let where_offset = find_keyword(rest, "where");
    let (tail, where_clause) = match where_offset {
        Some(offset) => (&rest[..offset], rest[offset + "where".len()..].trim()),
        None => (rest, ""),
    };
    let returns = match tail.trim().strip_prefix("->") {
        Some(returns) => normalize_type(returns),
        None => {
            assert!(
                tail.trim().is_empty(),
                "{origin}: unrecognized signature tail {tail:?}",
            );
            String::new()
        }
    };

    let mut receiver = String::new();
    let mut arguments = Vec::new();
    for (index, argument) in split_top_level(arguments_text, ',').into_iter().enumerate() {
        let argument = strip_attributes(&argument, &origin).trim().to_owned();
        if argument.is_empty() {
            continue;
        }
        let normalized = normalize_type(&argument);
        if index == 0
            && (normalized == "self" || normalized == "&self" || normalized == "&mut self")
        {
            receiver = normalized;
            continue;
        }
        let Some((_, kind)) = argument.split_once(':') else {
            panic!("{origin}: unrecognized argument {argument:?} in {text:?}")
        };
        arguments.push(resolve_paths(&normalize_type(kind), imports));
    }

    Signature {
        name,
        receiver,
        arguments,
        returns,
        where_clause: where_clause.to_owned(),
    }
}

/// Classifies a static facade method from its normalized return type.
fn static_class(returns: &str) -> SurfaceClass {
    if returns.is_empty() {
        return SurfaceClass::Plain;
    }
    if returns.contains("Operation<") {
        if returns.contains("AppliedOnly") {
            return SurfaceClass::AppliedOnly;
        }
        if returns.contains("Targeted") {
            return SurfaceClass::Targeted;
        }
    }
    if returns.contains("Result<()>") {
        return SurfaceClass::Plain;
    }
    SurfaceClass::Inquiry
}

/// Classifies a dynamic facade method from its erased return type.
fn dyn_class(returns: &str) -> SurfaceClass {
    if returns.contains("DynAppliedOperation") {
        return SurfaceClass::AppliedOnly;
    }
    if returns.contains("DynTargetedOperation") {
        return SurfaceClass::Targeted;
    }
    if returns.contains("Result<(),Error>") {
        return SurfaceClass::Plain;
    }
    SurfaceClass::Inquiry
}

// ---------------------------------------------------------------------------
// Static facades
// ---------------------------------------------------------------------------

/// Reads one static (async or blocking) facade into noun/method facts.
fn static_facade(source: &str, label: &'static str) -> StaticFacade {
    let cleaned = clean_source(source, label);
    let lines: Vec<&str> = cleaned.lines().collect();
    let skip = skipped_lines(&lines, label);
    let imports = use_map(&lines, &skip, label);
    let mut facade = StaticFacade::default();

    // First pass: the capability gate each accessor type itself imposes.
    let mut index = 0;
    while index < lines.len() {
        if skip[index] {
            index += 1;
            continue;
        }
        let trimmed = lines[index].trim();
        if trimmed.starts_with("accessor!") {
            let (name, bound, next) = accessor_invocation(&lines, index, label);
            let bounds = bound.map_or_else(BTreeSet::new, |bound| {
                marker_bounds(&bound, &imports, label)
            });
            facade.gates.insert(name, bounds);
            index = next;
            continue;
        }
        if let Some(rest) = trimmed.strip_prefix("pub struct ") {
            let name = leading_identifier(rest);
            if name.ends_with("Accessor") {
                let (header, _, _) = join_until(&lines, index, &['{', ';'], label);
                let after = header
                    .split_once(name)
                    .map_or("", |(_, after)| after)
                    .trim_start();
                let generics = if after.starts_with('<') {
                    split_generics(after, label).0
                } else {
                    ""
                };
                facade
                    .gates
                    .insert(name.to_owned(), profile_bounds(generics, &imports, label));
            }
            index = item_end(&lines, index, label);
            continue;
        }
        index += 1;
    }

    // Second pass: the inherent impls that carry the accessor methods.
    let mut index = 0;
    while index < lines.len() {
        if skip[index] {
            index += 1;
            continue;
        }
        let trimmed = lines[index].trim();
        if !is_item_start(trimmed) {
            index += 1;
            continue;
        }
        if !trimmed.starts_with("impl") {
            index = item_end(&lines, index, label);
            continue;
        }

        let (header, _, body_start) = join_until(&lines, index, &['{'], label);
        let end = block_end(&lines, index, label);
        let target = impl_target(&header, label, index + 1);
        match target {
            ImplTarget::Inherent { name, generics } if name.ends_with("Accessor") => {
                let mut bounds = profile_bounds(&generics, &imports, label);
                bounds.extend(facade.gates.get(&name).into_iter().flatten().cloned());
                let (methods, generated) =
                    accessor_methods(&lines, body_start, end, label, &imports, &bounds);
                facade
                    .accessors
                    .entry(name.clone())
                    .or_default()
                    .extend(methods);
                facade.generated.entry(name).or_default().extend(generated);
            }
            ImplTarget::Trait { path, name } if name.ends_with("Accessor") => {
                assert!(
                    ALLOWED_ACCESSOR_TRAIT_IMPLS.contains(&path.as_str()),
                    "{label}:{}: `impl {path} for {name}` adds methods to an accessor outside \
                     the inherent impl the parity gate reads; put the method on all three \
                     facades instead, or add the trait to ALLOWED_ACCESSOR_TRAIT_IMPLS",
                    index + 1,
                );
            }
            _ => {}
        }
        index = end;
    }

    facade
}

/// What an `impl` header targets.
#[derive(Debug)]
enum ImplTarget {
    /// An inherent impl on `name` with the given generic parameter list.
    Inherent { name: String, generics: String },
    /// A `impl path for name` trait implementation.
    Trait { path: String, name: String },
}

/// Splits an `impl` header into its generics, optional trait, and self type.
fn impl_target(header: &str, label: &str, line: usize) -> ImplTarget {
    let origin = format!("{label}:{line}");
    let rest = header
        .trim()
        .strip_prefix("impl")
        .unwrap_or_else(|| panic!("{origin}: not an impl header: {header:?}"))
        .trim_start();
    let (generics, rest) = if rest.starts_with('<') {
        let (generics, rest) = split_generics(rest, &origin);
        (generics.to_owned(), rest.trim_start())
    } else {
        (String::new(), rest)
    };

    match find_keyword(rest, "for") {
        Some(offset) => {
            let path = normalize_type(&rest[..offset]);
            let name = leading_identifier(rest[offset + "for".len()..].trim_start()).to_owned();
            ImplTarget::Trait { path, name }
        }
        None => ImplTarget::Inherent {
            name: leading_identifier(rest).to_owned(),
            generics,
        },
    }
}

/// Strips a leading visibility qualifier from a trimmed declaration.
fn strip_visibility(trimmed: &str) -> &str {
    if let Some(rest) = trimmed.strip_prefix("pub ") {
        return rest.trim_start();
    }
    if trimmed.starts_with("pub(") {
        if let Some((_, rest)) = trimmed.split_once(')') {
            return rest.trim_start();
        }
    }
    trimmed
}

/// Returns whether a declaration is an associated const or type rather than a
/// method.  Neither can add a callable method, so neither affects parity.
fn is_associated_data(declaration: &str) -> bool {
    (declaration.starts_with("const ") && !declaration.starts_with("const fn "))
        || declaration.starts_with("type ")
}

/// Returns whether a trimmed line starts a top-level item with a body.
fn is_item_start(trimmed: &str) -> bool {
    const STARTS: &[&str] = &[
        "impl",
        "pub trait ",
        "trait ",
        "pub struct ",
        "struct ",
        "pub enum ",
        "enum ",
        "pub mod ",
        "mod ",
        "pub fn ",
        "fn ",
        "pub async fn ",
        "async fn ",
        "macro_rules!",
    ];
    STARTS.iter().any(|start| trimmed.starts_with(start))
}

/// Reads one `accessor!` invocation, returning the type name and its bound.
fn accessor_invocation(
    lines: &[&str],
    start: usize,
    label: &str,
) -> (String, Option<String>, usize) {
    let origin = format!("{label}:{}", start + 1);
    let (text, _, next) = join_until(lines, start, &[';'], label);
    let open = text
        .find('(')
        .unwrap_or_else(|| panic!("{origin}: no argument list in {text:?}"));
    let inner = split_parens(&text[open..], &origin).0;
    let arguments: Vec<String> = split_top_level(inner, ',')
        .into_iter()
        .map(|argument| strip_attributes(&argument, &origin).trim().to_owned())
        .filter(|argument| !argument.is_empty())
        .collect();
    assert!(
        (2..=3).contains(&arguments.len()),
        "{origin}: unrecognized accessor! invocation {text:?}",
    );
    let name = leading_identifier(&arguments[0]).to_owned();
    assert!(
        name.ends_with("Accessor"),
        "{origin}: accessor! declared non-accessor type {name:?}",
    );
    (name, arguments.get(2).cloned(), next)
}

/// Reads one accessor impl body.
///
/// Returns the hand-written public methods and the [`crate::noun_table`] nouns
/// the impl generates.
fn accessor_methods(
    lines: &[&str],
    body_start: usize,
    end: usize,
    label: &'static str,
    imports: &BTreeMap<String, String>,
    impl_bounds: &BTreeSet<String>,
) -> (BTreeMap<String, MethodFacts>, BTreeSet<String>) {
    let mut methods = BTreeMap::new();
    let mut generated = BTreeSet::new();
    let mut index = body_start;
    let last = end.saturating_sub(1);

    while index < last {
        let trimmed = lines[index].trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            index += 1;
            continue;
        }

        // The one macro invocation this gate accepts: its rows are read from
        // the table itself, so nothing is skipped unread.
        if let Some(rest) = trimmed.strip_prefix("noun_table!") {
            let inner = split_parens(rest.trim_start(), label).0;
            let Some((noun, _)) = inner.split_once("=>") else {
                panic!("{label}:{}: unrecognized noun_table! invocation", index + 1)
            };
            assert!(
                generated.insert(noun.trim().to_owned()),
                "{label}:{}: accessor impl generates {} twice",
                index + 1,
                noun.trim(),
            );
            index = item_end(lines, index, label);
            continue;
        }

        // Only bare `pub` reaches a consumer; `pub(crate)`/`pub(super)` items
        // are internal helpers and are read but not inventoried.
        let public = trimmed.starts_with("pub ");
        let declaration = strip_visibility(trimmed);
        if is_associated_data(declaration) {
            index = item_end(lines, index, label);
            continue;
        }

        let mut rest = declaration;
        for keyword in ["default ", "const ", "async ", "unsafe ", "extern "] {
            rest = rest.strip_prefix(keyword).unwrap_or(rest);
        }
        assert!(
            rest.starts_with("fn "),
            "{label}:{}: unrecognized item {trimmed:?} in an accessor impl; a macro invocation \
             other than `noun_table!` here can declare methods the parity gate would never see, \
             so the gate refuses to skip what it cannot read",
            index + 1,
        );

        let (text, _, _) = join_until(lines, index, &['{'], label);
        let signature = parse_signature(&text, imports, label, index + 1);
        if public {
            let mut bounds = impl_bounds.clone();
            bounds.extend(profile_bounds(&signature.where_clause, imports, label));
            let facts = MethodFacts {
                shape: MethodShape {
                    class: static_class(&signature.returns),
                    receiver: signature.receiver,
                    arguments: signature.arguments,
                },
                bounds,
            };
            assert!(
                methods.insert(signature.name.clone(), facts).is_none(),
                "{label}:{}: duplicate accessor method {}",
                index + 1,
                signature.name,
            );
        }
        index = block_end(lines, index, label);
    }

    (methods, generated)
}

// ---------------------------------------------------------------------------
// Dynamic facade
// ---------------------------------------------------------------------------

/// Reads the dynamic facade traits into noun/method shapes.
///
/// Capability bounds are absent by construction; see the module documentation.
/// Returns the hand-written method shapes and, per trait, the
/// [`crate::noun_table`] nouns the trait declaration generates.
fn dyn_facade(source: &str, label: &'static str) -> DynFacade {
    let cleaned = clean_source(source, label);
    let lines: Vec<&str> = cleaned.lines().collect();
    let skip = skipped_lines(&lines, label);
    let imports = use_map(&lines, &skip, label);
    let mut surface: BTreeMap<String, BTreeMap<String, DynMethodShape>> = BTreeMap::new();
    let mut generated: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();

    let mut index = 0;
    while index < lines.len() {
        if skip[index] {
            index += 1;
            continue;
        }
        let trimmed = lines[index].trim();
        if !is_item_start(trimmed) {
            index += 1;
            continue;
        }
        let Some(rest) = trimmed.strip_prefix("pub trait ") else {
            index = item_end(&lines, index, label);
            continue;
        };

        let name = leading_identifier(rest).to_owned();
        let (_, _, body_start) = join_until(&lines, index, &['{'], label);
        let end = block_end(&lines, index, label);
        let trait_generated = generated.entry(name.clone()).or_default();
        let methods = surface.entry(name).or_default();

        let mut cursor = body_start;
        let last = end.saturating_sub(1);
        while cursor < last {
            let line = lines[cursor].trim();
            if line.is_empty() || line.starts_with('#') {
                cursor += 1;
                continue;
            }
            if let Some(rest) = line.strip_prefix("noun_table!") {
                let inner = split_parens(rest.trim_start(), label).0;
                let Some((noun, _)) = inner.split_once("=>") else {
                    panic!(
                        "{label}:{}: unrecognized noun_table! invocation",
                        cursor + 1
                    )
                };
                assert!(
                    trait_generated.insert(noun.trim().to_owned()),
                    "{label}:{}: dynamic noun trait generates {} twice",
                    cursor + 1,
                    noun.trim(),
                );
                cursor = item_end(&lines, cursor, label);
                continue;
            }
            if is_associated_data(line) {
                cursor = item_end(&lines, cursor, label);
                continue;
            }
            assert!(
                line.starts_with("fn ") || line.starts_with("async fn "),
                "{label}:{}: unrecognized item {line:?} in a dynamic noun trait; a macro \
                 invocation other than `noun_table!` here can declare methods the parity gate \
                 would never see",
                cursor + 1,
            );
            let (text, terminator, next) = join_until(&lines, cursor, &[';', '{'], label);
            let signature = parse_signature(&text, &imports, label, cursor + 1);
            let shape = MethodShape {
                class: dyn_class(&signature.returns),
                receiver: signature.receiver,
                arguments: signature.arguments,
            };
            assert!(
                methods.insert(signature.name.clone(), shape).is_none(),
                "{label}:{}: duplicate dynamic method {}",
                cursor + 1,
                signature.name,
            );
            cursor = if terminator == ';' {
                next
            } else {
                block_end(&lines, cursor, label)
            };
        }

        index = end;
    }

    (surface, generated)
}

/// Returns the sorted method names of one parsed noun.
fn method_names<T>(noun: &BTreeMap<String, T>) -> Vec<&str> {
    noun.keys().map(String::as_str).collect()
}

/// Projects one noun's table rows onto a static facade's method facts.
///
/// The accessor's own type-level gate is unioned into every row, because the
/// generated method inherits it from the impl block it lands in.
fn static_table_methods(
    table: &BTreeMap<String, BTreeMap<String, TableRow>>,
    nouns: &BTreeSet<String>,
    gate: &BTreeSet<String>,
    label: &str,
    accessor: &str,
) -> BTreeMap<String, MethodFacts> {
    let mut methods = BTreeMap::new();
    for noun in nouns {
        let rows = table
            .get(noun)
            .unwrap_or_else(|| panic!("{label}: {accessor} generates unknown noun {noun}"));
        for (method, row) in rows {
            let mut bounds = gate.clone();
            bounds.extend(row.gates.iter().cloned());
            let facts = MethodFacts {
                shape: MethodShape {
                    class: row.class,
                    receiver: "&self".to_owned(),
                    arguments: row.arguments.clone(),
                },
                bounds,
            };
            assert!(
                methods.insert(method.clone(), facts).is_none(),
                "{label}: {accessor} generates {method} twice",
            );
        }
    }
    methods
}

/// Merges a facade's generated rows with whatever it still writes by hand.
fn merge_methods(
    mut generated: BTreeMap<String, MethodFacts>,
    residual: &BTreeMap<String, MethodFacts>,
    label: &str,
    accessor: &str,
) -> BTreeMap<String, MethodFacts> {
    for (method, facts) in residual {
        assert!(
            generated.insert(method.clone(), facts.clone()).is_none(),
            "{label}: hand-written {accessor}::{method} shadows a generated table row",
        );
    }
    generated
}

/// Returns the hand-written methods `accessor` is allowed to still carry.
fn exempt_methods(accessor: &str) -> BTreeSet<String> {
    EXEMPT_HAND_WRITTEN
        .iter()
        .find(|(name, _)| *name == accessor)
        .map(|(_, methods)| methods.iter().map(|method| (*method).to_owned()).collect())
        .unwrap_or_default()
}

#[test]
fn noun_surfaces_agree_on_method_name_class_arguments_and_capability_bound() {
    let ledger = ledger_surface();
    let table = table_surface();
    let asynchronous = static_facade(ASYNC_SOURCE, "src/async_nouns.rs");
    let blocking = static_facade(BLOCKING_SOURCE, "src/blocking_nouns.rs");
    let (dynamic, dynamic_generated) = dyn_facade(DYN_SOURCE, "src/dynapi/nouns.rs");

    let markers = known_markers();
    for (label, facade) in [
        ("src/async_nouns.rs", &asynchronous),
        ("src/blocking_nouns.rs", &blocking),
    ] {
        for (accessor, bounds) in &facade.gates {
            for bound in bounds {
                assert!(
                    markers.contains(bound.as_str()),
                    "{label}: {accessor} is gated on {bound:?}, which is not a ledger \
                     capability marker",
                );
            }
        }
        for (accessor, methods) in &facade.accessors {
            for (method, facts) in methods {
                for bound in &facts.bounds {
                    assert!(
                        markers.contains(bound.as_str()),
                        "{label}: {accessor}::{method} is gated on {bound:?}, which is not a \
                         ledger capability marker",
                    );
                }
            }
        }
    }

    // A table gate is a bare marker name, so it means whatever each facade's
    // imports say it means.  Both static facades must resolve it to the real
    // capability trait: a shadow trait borrowing the name would otherwise
    // silently stand in for the gate on one surface.
    let table_gates: BTreeSet<&str> = table
        .values()
        .flat_map(|rows| rows.values())
        .flat_map(|row| row.gates.iter().map(String::as_str))
        .collect();
    for gate in &table_gates {
        assert!(
            markers.contains(gate),
            "{TABLE_LABEL}: {gate:?} is not a ledger capability marker",
        );
    }
    for (label, source) in [
        ("src/async_nouns.rs", ASYNC_SOURCE),
        ("src/blocking_nouns.rs", BLOCKING_SOURCE),
    ] {
        let cleaned = clean_source(source, label);
        let lines: Vec<&str> = cleaned.lines().collect();
        let skip = skipped_lines(&lines, label);
        let imports = use_map(&lines, &skip, label);
        for gate in &table_gates {
            assert_eq!(
                imports.get(*gate).map(String::as_str),
                Some(format!("crate::capabilities::{gate}").as_str()),
                "{label}: the table gate {gate:?} does not resolve to the real capability \
                 marker on this facade",
            );
        }
    }

    let facades: Vec<NounFacade> = ledger
        .iter()
        .map(|(facade, _)| *facade)
        .chain(std::iter::once(MOTION_FACADE))
        .collect();

    for facade in &facades {
        let accessor = facade.accessor;
        let asynchronous_residual = asynchronous
            .accessors
            .get(accessor)
            .unwrap_or_else(|| panic!("async facade is missing {accessor}"));
        let blocking_residual = blocking
            .accessors
            .get(accessor)
            .unwrap_or_else(|| panic!("blocking facade is missing {accessor}"));
        let dynamic_residual = dynamic
            .get(facade.dyn_trait)
            .unwrap_or_else(|| panic!("dynamic facade is missing {}", facade.dyn_trait));

        // Whatever a facade still writes by hand has to be declared, on every
        // surface: an undeclared residual is a method that escaped generation.
        let exempt = exempt_methods(accessor);
        for (label, residual) in [
            ("src/async_nouns.rs", &method_names(asynchronous_residual)),
            ("src/blocking_nouns.rs", &method_names(blocking_residual)),
            ("src/dynapi/nouns.rs", &method_names(dynamic_residual)),
        ] {
            let observed: BTreeSet<String> =
                residual.iter().map(|name| (*name).to_owned()).collect();
            assert_eq!(
                observed, exempt,
                "{label}: {accessor} hand-writes methods that are not the declared \
                 EXEMPT_HAND_WRITTEN set",
            );
        }

        // The generated half must come from the same table arm on all three.
        let asynchronous_nouns = asynchronous
            .generated
            .get(accessor)
            .cloned()
            .unwrap_or_default();
        let blocking_nouns = blocking
            .generated
            .get(accessor)
            .cloned()
            .unwrap_or_default();
        let dynamic_nouns = dynamic_generated
            .get(facade.dyn_trait)
            .cloned()
            .unwrap_or_default();
        let expected_nouns: BTreeSet<String> = facade
            .table_key
            .into_iter()
            .map(str::to_owned)
            .collect::<BTreeSet<_>>();
        assert_eq!(
            asynchronous_nouns, expected_nouns,
            "async {accessor} does not generate exactly its own table arm",
        );
        assert_eq!(
            blocking_nouns, expected_nouns,
            "blocking {accessor} does not generate exactly its own table arm",
        );
        assert_eq!(
            dynamic_nouns, expected_nouns,
            "{} does not generate exactly its own table arm",
            facade.dyn_trait,
        );

        assert_eq!(
            asynchronous.gates.get(accessor),
            blocking.gates.get(accessor),
            "async and blocking {accessor} carry different type-level capability gates",
        );

        let asynchronous_gate = asynchronous
            .gates
            .get(accessor)
            .cloned()
            .unwrap_or_default();
        let blocking_gate = blocking.gates.get(accessor).cloned().unwrap_or_default();
        let asynchronous_noun = merge_methods(
            static_table_methods(
                &table,
                &asynchronous_nouns,
                &asynchronous_gate,
                "src/async_nouns.rs",
                accessor,
            ),
            asynchronous_residual,
            "src/async_nouns.rs",
            accessor,
        );
        let blocking_noun = merge_methods(
            static_table_methods(
                &table,
                &blocking_nouns,
                &blocking_gate,
                "src/blocking_nouns.rs",
                accessor,
            ),
            blocking_residual,
            "src/blocking_nouns.rs",
            accessor,
        );
        let dynamic_noun = merge_methods(
            static_table_methods(
                &table,
                &dynamic_nouns,
                &BTreeSet::new(),
                "src/dynapi/nouns.rs",
                facade.dyn_trait,
            ),
            &dynamic_residual
                .iter()
                .map(|(method, shape)| {
                    (
                        method.clone(),
                        MethodFacts {
                            shape: shape.clone(),
                            bounds: BTreeSet::new(),
                        },
                    )
                })
                .collect(),
            "src/dynapi/nouns.rs",
            facade.dyn_trait,
        );

        assert_eq!(
            method_names(&asynchronous_noun),
            method_names(&blocking_noun),
            "async and blocking {accessor} expose different methods",
        );
        assert_eq!(
            method_names(&asynchronous_noun),
            method_names(&dynamic_noun),
            "async {accessor} and {} expose different methods",
            facade.dyn_trait,
        );

        for (method, asynchronous_facts) in &asynchronous_noun {
            let blocking_facts = &blocking_noun[method];
            let dynamic_shape = &dynamic_noun[method].shape;
            assert_eq!(
                asynchronous_facts.shape, blocking_facts.shape,
                "async and blocking {accessor}::{method} disagree on receiver, argument \
                 types, or return class",
            );
            assert_eq!(
                asynchronous_facts.shape.receiver, dynamic_shape.receiver,
                "async {accessor}::{method} and {}::{method} disagree on receiver",
                facade.dyn_trait,
            );
            assert_eq!(
                asynchronous_facts.shape.arguments, dynamic_shape.arguments,
                "async {accessor}::{method} and {}::{method} disagree on argument types",
                facade.dyn_trait,
            );
            assert_eq!(
                asynchronous_facts.shape.class, dynamic_shape.class,
                "async {accessor}::{method} and {}::{method} disagree on return class",
                facade.dyn_trait,
            );
            assert_eq!(
                asynchronous_facts.bounds, blocking_facts.bounds,
                "async and blocking {accessor}::{method} disagree on capability bounds",
            );
        }
    }

    for (facade, rows) in &ledger {
        let accessor = facade.accessor;
        let noun = facade
            .table_key
            .unwrap_or_else(|| panic!("ledger noun {accessor} has no table arm"));
        let gate = asynchronous
            .gates
            .get(accessor)
            .cloned()
            .unwrap_or_default();
        let table_rows = &table[noun];

        for (method, facts) in rows {
            let observed = table_rows
                .get(*method)
                .unwrap_or_else(|| panic!("ledger row {accessor}::{method} has no table row"));
            assert_eq!(
                observed.class, facts.class,
                "{accessor}::{method} does not carry its ledger return class",
            );

            let mut expected = gate.clone();
            expected.extend(facts.marker.map(str::to_owned));
            let mut bounds = gate.clone();
            bounds.extend(observed.gates.iter().cloned());
            assert_eq!(
                bounds, expected,
                "{accessor}::{method} does not carry its ledger capability bound",
            );
        }
    }
}

/// Unit tests for the scanner primitives.
///
/// The parity gate above is only as good as the reader underneath it, and a
/// reader that quietly mis-parses is worse than no gate at all.  Each test here
/// pins one property the gate depends on.
#[cfg(test)]
mod scanner {
    use super::*;

    #[test]
    fn comments_and_literals_cannot_steer_the_scanner() {
        let cleaned = clean_source(
            "let needle = format!(\"pub fn {method}(\"); // }} not a brace\n",
            "fixture",
        );
        assert!(!cleaned.contains('{'));
        assert!(!cleaned.contains('}'));
        assert_eq!(cleaned.lines().count(), 1);
    }

    #[test]
    fn lifetimes_are_not_character_literals() {
        let cleaned = clean_source("impl<'a, P> Foo<'a, P> {}\n", "fixture");
        assert!(cleaned.contains("<'a, P>"));
    }

    #[test]
    fn join_until_ignores_terminators_inside_delimiters() {
        let lines = [
            "    fn raw(&self) -> Result<[u8; 4], Error>;",
            "    fn next();",
        ];
        let (text, terminator, next) = join_until(&lines, 0, &[';'], "fixture");
        assert_eq!(terminator, ';');
        assert_eq!(next, 1);
        assert_eq!(text, "fn raw(&self) -> Result<[u8; 4], Error>");
    }

    #[test]
    fn join_until_spans_lines_until_the_body_brace() {
        let lines = [
            "    pub async fn set(",
            "        &self,",
            "        value: u8,",
            "    ) -> Result<()>",
            "    where",
            "        P: HasZoom,",
            "    {",
        ];
        let (text, terminator, next) = join_until(&lines, 0, &['{'], "fixture");
        assert_eq!(terminator, '{');
        assert_eq!(next, 7);
        assert_eq!(
            text,
            "pub async fn set( &self, value: u8, ) -> Result<()> where P: HasZoom,"
        );
    }

    #[test]
    fn split_top_level_keeps_generic_arguments_together() {
        assert_eq!(
            split_top_level("a: Result<u8, Error>, b: u8", ','),
            vec!["a: Result<u8, Error>".to_owned(), "b: u8".to_owned()],
        );
    }

    #[test]
    fn normalize_type_drops_lifetimes_but_not_widths() {
        assert_eq!(
            normalize_type("Operation<'session, Targeted>"),
            "Operation<Targeted>"
        );
        assert_ne!(normalize_type("f32"), normalize_type("f64"));
        assert_ne!(normalize_type("&self"), normalize_type("self"));
    }

    #[test]
    fn profile_bounds_read_every_predicate() {
        let imports = BTreeMap::from([
            (
                "HasZoom".to_owned(),
                "crate::capabilities::HasZoom".to_owned(),
            ),
            (
                "HasTally".to_owned(),
                "crate::capabilities::HasTally".to_owned(),
            ),
        ]);
        let bounds = profile_bounds("P: HasZoom, P: HasTally", &imports, "fixture");
        assert_eq!(
            bounds,
            BTreeSet::from(["HasZoom".to_owned(), "HasTally".to_owned()]),
        );
    }

    #[test]
    #[should_panic(expected = "does not resolve into a documented capability module path")]
    fn a_shadow_trait_never_stands_in_for_a_capability() {
        let imports = BTreeMap::new();
        let _ = marker_bounds("shadow::HasDirectZoom", &imports, "fixture");
    }

    #[test]
    fn declaration_scan_drops_in_file_test_data() {
        let source = concat!(
            "pub struct RealAccessor;\n",
            "#[cfg(test)]\n",
            "mod tests {\n",
            "    const NAMES: &[&str] = &[\"GhostAccessor\"];\n",
            "}\n",
            "pub struct LaterAccessor;\n",
        );
        let declarations = without_test_modules(source, "fixture");
        assert!(declarations.contains("RealAccessor"));
        assert!(declarations.contains("LaterAccessor"));
        assert!(!declarations.contains("GhostAccessor"));
        assert_eq!(declarations.lines().count(), source.lines().count());
    }

    #[test]
    fn the_real_sources_keep_their_declarations_out_of_their_tests() {
        let declarations = without_test_modules(BLOCKING_SOURCE, "src/blocking_nouns.rs");
        assert!(declarations.contains("pub struct MotionAccessor"));
        assert!(!declarations.contains("mod inventory_tests"));
    }
}
