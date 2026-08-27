//! Cross-surface parity gate for the three hand-written noun facades.
//!
//! [`crate::async_nouns`], [`crate::blocking::nouns`], and
//! [`crate::dynapi::nouns`] are three independent transcriptions of the same
//! closed ledger in [`crate::command::surface`].  Nothing in the type system
//! relates them, and the published API snapshots diff each facade only against
//! its own baseline, so a method dropped from one surface, reclassified, or
//! given a different capability bound would otherwise pass every gate.
//!
//! This module reads the three sources and compares them against the ledger
//! and against each other by method name, semantic return class, and required
//! capability bound.  Nothing here is derived from a hand-maintained
//! `EXPECTED_` table: the noun set, the method set, the classes, and the
//! markers all come from [`surface_entry`].  Rustdoc prose is deliberately out
//! of scope; only semantic identity is gated.

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

/// Motion is a safety/observation view rather than a ledger noun, so it has no
/// [`StaticNoun`] row; it is still one of the surfaces that must stay in step.
const MOTION_FACADE: NounFacade = NounFacade {
    accessor: "MotionAccessor",
    dyn_trait: "DynMotion",
};

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

/// One facade method reduced to the facts the three surfaces must share.
#[derive(Debug, Clone, PartialEq, Eq)]
struct MethodFacts {
    class: SurfaceClass,
    bounds: BTreeSet<String>,
}

/// The name each facade gives one noun.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct NounFacade {
    accessor: &'static str,
    dyn_trait: &'static str,
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
    }
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

/// Returns the leading identifier of `text`.
fn leading_identifier(text: &str) -> &str {
    let end = text
        .find(|ch: char| !ch.is_alphanumeric() && ch != '_')
        .unwrap_or(text.len());
    &text[..end]
}

/// Splits `text`, which must start with `<`, into the bracket body and rest.
fn split_generics(text: &str) -> Option<(&str, &str)> {
    let mut depth = 0_usize;
    for (index, ch) in text.char_indices() {
        match ch {
            '<' => depth += 1,
            '>' => {
                depth -= 1;
                if depth == 0 {
                    return Some((&text[1..index], &text[index + 1..]));
                }
            }
            _ => {}
        }
    }
    None
}

/// Normalizes a `A + path::B` bound list into marker trait names.
fn marker_bounds(list: &str) -> BTreeSet<String> {
    list.split('+')
        .map(|bound| {
            bound
                .trim()
                .trim_end_matches(',')
                .rsplit("::")
                .next()
                .unwrap_or_default()
                .trim()
                .to_owned()
        })
        .filter(|bound| !bound.is_empty() && bound != "CompileTimeProfile")
        .collect()
}

/// Extracts the bounds placed on the profile parameter `P` in a generic list.
fn profile_bounds(generics: &str) -> BTreeSet<String> {
    for parameter in generics.split(',') {
        if let Some((name, bounds)) = parameter.split_once(':') {
            if name.trim() == "P" {
                return marker_bounds(bounds);
            }
        }
    }
    BTreeSet::new()
}

/// Extracts the bounds placed on `P` by a `where` clause in a signature.
fn where_bounds(signature: &str) -> BTreeSet<String> {
    let Some((_, clause)) = signature.split_once(" where ") else {
        return BTreeSet::new();
    };
    profile_bounds(clause)
}

/// Joins source lines from `start` until `terminator`, returning the text
/// before it and the index of the line after it.
fn join_until(lines: &[&str], start: usize, terminator: char) -> (String, usize) {
    let mut text = String::new();
    let mut index = start;
    while index < lines.len() {
        let line = lines[index];
        let (fragment, done) = match line.find(terminator) {
            Some(position) => (&line[..position], true),
            None => (line, false),
        };
        let fragment = fragment.trim();
        if !fragment.is_empty() {
            if !text.is_empty() {
                text.push(' ');
            }
            text.push_str(fragment);
        }
        index += 1;
        if done {
            break;
        }
    }
    (text, index)
}

/// Classifies a static facade method from its return type.
fn static_class(signature: &str) -> SurfaceClass {
    let Some((_, returns)) = signature.split_once("->") else {
        return SurfaceClass::Plain;
    };
    let returns = returns.split(" where ").next().unwrap_or(returns);
    if returns.contains("Operation<") {
        if returns.contains("AppliedOnly") {
            return SurfaceClass::AppliedOnly;
        }
        if returns.contains("Targeted") {
            return SurfaceClass::Targeted;
        }
    }
    if returns.replace(' ', "").contains("Result<()>") {
        return SurfaceClass::Plain;
    }
    SurfaceClass::Inquiry
}

/// Classifies a dynamic facade method from its erased return type.
fn dyn_class(signature: &str) -> SurfaceClass {
    if signature.contains("DynAppliedOperation") {
        return SurfaceClass::AppliedOnly;
    }
    if signature.contains("DynTargetedOperation") {
        return SurfaceClass::Targeted;
    }
    if signature.replace(' ', "").contains("Result<(),Error>") {
        return SurfaceClass::Plain;
    }
    SurfaceClass::Inquiry
}

/// Reads the capability gate each accessor type itself imposes on `P`.
///
/// Macro-generated accessors carry the gate on the `Camera` entry point (the
/// third `accessor!` argument); hand-written accessors carry it in the struct
/// generics.
fn accessor_gates(source: &str) -> BTreeMap<String, BTreeSet<String>> {
    let lines: Vec<&str> = source.lines().collect();
    let mut gates = BTreeMap::new();
    let mut index = 0;

    while index < lines.len() {
        let line = lines[index];
        if line.starts_with("accessor!(") {
            let mut arguments = Vec::new();
            index += 1;
            while index < lines.len() && !lines[index].starts_with(");") {
                let text = lines[index].trim();
                if !text.starts_with("//") {
                    arguments.push(text.trim_end_matches(',').to_owned());
                }
                index += 1;
            }
            index += 1;
            if let Some(name) = arguments.first() {
                let bounds = arguments
                    .get(2)
                    .map_or_else(BTreeSet::new, |bound| marker_bounds(bound));
                gates.insert(name.clone(), bounds);
            }
            continue;
        }
        if let Some(rest) = line.strip_prefix("pub struct ") {
            let name = leading_identifier(rest);
            if name.ends_with("Accessor") {
                let generics = split_generics(&rest[name.len()..])
                    .map(|(inner, _)| inner)
                    .unwrap_or_default();
                gates.insert(name.to_owned(), profile_bounds(generics));
            }
        }
        index += 1;
    }

    gates
}

/// Reads one static (async or blocking) facade into noun/method facts.
fn static_surface(
    source: &str,
    method_keyword: &str,
) -> BTreeMap<String, BTreeMap<String, MethodFacts>> {
    let gates = accessor_gates(source);
    let lines: Vec<&str> = source.lines().collect();
    let mut surface: BTreeMap<String, BTreeMap<String, MethodFacts>> = BTreeMap::new();
    let mut current: Option<(String, BTreeSet<String>)> = None;
    let mut index = 0;

    while index < lines.len() {
        let line = lines[index];

        if line.starts_with("impl") {
            let (header, next) = join_until(&lines, index, '{');
            index = next;
            current = accessor_impl(&header, &gates);
            continue;
        }
        if line == "}" {
            current = None;
            index += 1;
            continue;
        }
        if let Some((accessor, bounds)) = current.clone() {
            if let Some(rest) = line.trim_start().strip_prefix(method_keyword) {
                let method = leading_identifier(rest).to_owned();
                let (signature, next) = join_until(&lines, index, '{');
                index = next;
                let mut method_bounds = bounds;
                method_bounds.extend(where_bounds(&signature));
                surface.entry(accessor).or_default().insert(
                    method,
                    MethodFacts {
                        class: static_class(&signature),
                        bounds: method_bounds,
                    },
                );
                continue;
            }
        }
        index += 1;
    }

    surface
}

/// Returns the accessor name and effective `P` bounds of an inherent impl.
fn accessor_impl(
    header: &str,
    gates: &BTreeMap<String, BTreeSet<String>>,
) -> Option<(String, BTreeSet<String>)> {
    if header.contains(" for ") {
        return None;
    }
    let rest = header.strip_prefix("impl")?.trim_start();
    let (generics, rest) = if rest.starts_with('<') {
        split_generics(rest)?
    } else {
        ("", rest)
    };
    let name = leading_identifier(rest.trim_start());
    if !name.ends_with("Accessor") {
        return None;
    }
    let mut bounds = profile_bounds(generics);
    bounds.extend(gates.get(name).into_iter().flatten().cloned());
    Some((name.to_owned(), bounds))
}

/// Reads the dynamic facade traits into noun/method facts.
fn dyn_surface(source: &str) -> BTreeMap<String, BTreeMap<String, MethodFacts>> {
    let lines: Vec<&str> = source.lines().collect();
    let mut surface: BTreeMap<String, BTreeMap<String, MethodFacts>> = BTreeMap::new();
    let mut current: Option<String> = None;
    let mut index = 0;

    while index < lines.len() {
        let line = lines[index];

        if let Some(rest) = line.strip_prefix("pub trait ") {
            current = Some(leading_identifier(rest).to_owned());
            index += 1;
            continue;
        }
        if line == "}" {
            current = None;
            index += 1;
            continue;
        }
        if let Some(name) = current.clone() {
            if let Some(rest) = line.trim_start().strip_prefix("fn ") {
                let method = leading_identifier(rest).to_owned();
                let (signature, next) = join_until(&lines, index, ';');
                index = next;
                surface.entry(name).or_default().insert(
                    method,
                    MethodFacts {
                        class: dyn_class(&signature),
                        // The erased facade gates capabilities at run time, so
                        // it carries no compile-time marker bound to compare.
                        bounds: BTreeSet::new(),
                    },
                );
                continue;
            }
        }
        index += 1;
    }

    surface
}

/// Returns the sorted method names of one parsed noun.
fn method_names(noun: &BTreeMap<String, MethodFacts>) -> Vec<&str> {
    noun.keys().map(String::as_str).collect()
}

#[test]
fn noun_surfaces_agree_on_method_name_class_and_capability_bound() {
    let ledger = ledger_surface();
    let async_surface = static_surface(ASYNC_SOURCE, "pub async fn ");
    let blocking_surface = static_surface(BLOCKING_SOURCE, "pub fn ");
    let dynamic_surface = dyn_surface(DYN_SOURCE);

    let facades: Vec<NounFacade> = ledger
        .iter()
        .map(|(facade, _)| *facade)
        .chain(std::iter::once(MOTION_FACADE))
        .collect();

    for facade in &facades {
        let asynchronous = async_surface
            .get(facade.accessor)
            .unwrap_or_else(|| panic!("async facade is missing {}", facade.accessor));
        let blocking = blocking_surface
            .get(facade.accessor)
            .unwrap_or_else(|| panic!("blocking facade is missing {}", facade.accessor));
        let dynamic = dynamic_surface
            .get(facade.dyn_trait)
            .unwrap_or_else(|| panic!("dynamic facade is missing {}", facade.dyn_trait));

        assert_eq!(
            method_names(asynchronous),
            method_names(blocking),
            "async and blocking {} expose different methods",
            facade.accessor,
        );
        assert_eq!(
            method_names(asynchronous),
            method_names(dynamic),
            "async {} and {} expose different methods",
            facade.accessor,
            facade.dyn_trait,
        );

        for (method, asynchronous_facts) in asynchronous {
            let blocking_facts = &blocking[method];
            let dynamic_facts = &dynamic[method];
            assert_eq!(
                asynchronous_facts.class, blocking_facts.class,
                "async and blocking {}::{method} disagree on return class",
                facade.accessor,
            );
            assert_eq!(
                asynchronous_facts.class, dynamic_facts.class,
                "async {}::{method} and {}::{method} disagree on return class",
                facade.accessor, facade.dyn_trait,
            );
            assert_eq!(
                asynchronous_facts.bounds, blocking_facts.bounds,
                "async and blocking {}::{method} disagree on capability bounds",
                facade.accessor,
            );
        }
    }

    let gates = accessor_gates(ASYNC_SOURCE);
    for (facade, rows) in &ledger {
        let asynchronous = &async_surface[facade.accessor];
        let gate = gates.get(facade.accessor).cloned().unwrap_or_default();

        for (method, facts) in rows {
            let observed = asynchronous.get(*method).unwrap_or_else(|| {
                panic!(
                    "ledger row {}::{method} has no facade method",
                    facade.accessor
                )
            });
            assert_eq!(
                observed.class, facts.class,
                "{}::{method} does not carry its ledger return class",
                facade.accessor,
            );

            let mut expected = gate.clone();
            expected.extend(facts.marker.map(str::to_owned));
            assert_eq!(
                observed.bounds, expected,
                "{}::{method} does not carry its ledger capability bound",
                facade.accessor,
            );
        }
    }
}
