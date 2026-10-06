//! Narrow parity guards for the table-driven noun facades.
//!
//! The noun registry in [`crate::noun_table`] is the source of the method
//! names, arguments, request builders, rustdoc, gates and return classes
//! consumed by the async, blocking, and dynamic facades. The static facades
//! are generated whole, accessors and getters included, by one consumer, so
//! nothing in their files can be compared against the registry. The compiled
//! registry and the exhaustive [`crate::command::surface`] projection already
//! make that disagreement impossible.
//!
//! This module therefore keeps only checks that the type system and macro
//! expansion cannot express for the dynamic facade: it must invoke every
//! noun arm.
//!
//! The source reader below is intentionally not a Rust parser. It strips
//! comments/literals and masks test and macro bodies; there is no
//! table/request parsing and no generated-signature cross-comparison.

#![allow(clippy::panic)]

use std::collections::{BTreeMap, BTreeSet};

use crate::{
    capabilities::TypedSupportSurface,
    command::{
        inquiry_structs::{BuiltinInquiryProfileGate, BUILTIN_INQUIRY_ACCESSORS},
        semantics::BuiltinCommand,
        surface::{
            surface_entry, typed_surface_for_command, StaticMarkerRequirement, StaticNoun,
            StaticSurfaceDisposition, BUILTIN_COMMAND_COUNT, NON_NOUN_COMMAND_COUNT,
            TARGET_FACING_COMMAND_COUNT,
        },
    },
    noun_table::noun_table,
};

/// The 14 registry noun arms, in the same order as the public facades.
#[cfg(all(feature = "dyn-api", feature = "async"))]
const NOUN_KEYS: &[&str] = &[
    "Power",
    "Zoom",
    "System",
    "PanTilt",
    "Focus",
    "Presets",
    "Exposure",
    "WhiteBalance",
    "Image",
    "Tally",
    "NdFilter",
    "MotionSync",
    "Menu",
    "Advanced",
];

/// The name each facade gives one registry noun.
#[cfg(all(feature = "dyn-api", feature = "async"))]
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct NounFacade {
    key: &'static str,
    dyn_trait: &'static str,
}

#[cfg(all(feature = "dyn-api", feature = "async"))]
const NOUN_FACADES: &[NounFacade] = &[
    NounFacade {
        key: "Power",
        dyn_trait: "DynPower",
    },
    NounFacade {
        key: "Zoom",
        dyn_trait: "DynZoom",
    },
    NounFacade {
        key: "System",
        dyn_trait: "DynSystem",
    },
    NounFacade {
        key: "PanTilt",
        dyn_trait: "DynPanTilt",
    },
    NounFacade {
        key: "Focus",
        dyn_trait: "DynFocus",
    },
    NounFacade {
        key: "Presets",
        dyn_trait: "DynPresets",
    },
    NounFacade {
        key: "Exposure",
        dyn_trait: "DynExposure",
    },
    NounFacade {
        key: "WhiteBalance",
        dyn_trait: "DynWhiteBalance",
    },
    NounFacade {
        key: "Image",
        dyn_trait: "DynImage",
    },
    NounFacade {
        key: "Tally",
        dyn_trait: "DynTally",
    },
    NounFacade {
        key: "NdFilter",
        dyn_trait: "DynNdFilter",
    },
    NounFacade {
        key: "MotionSync",
        dyn_trait: "DynMotionSync",
    },
    NounFacade {
        key: "Menu",
        dyn_trait: "DynMenu",
    },
    NounFacade {
        key: "Advanced",
        dyn_trait: "DynAdvanced",
    },
];

/// A row category used by the compiled-registry parity checks.
///
/// It is populated by the registry macro below, never by source parsing. The
/// variants keep the compiled inventory readable: command rows own one or
/// more IDs, inquiry rows own one inquiry accessor, and helper rows own no ID.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum TableRow {
    Command,
    Inquiry,
    Helper,
}

/// Materialize the registry's method names for the facade parity checks that
/// verify each spelling belongs to the expected noun.
///
/// Request expressions and argument types are deliberately captured as token
/// trees and discarded. This is a macro projection of the compiled registry,
/// not a second parser or a second source of truth.
#[must_use]
pub(crate) fn table_surface() -> BTreeMap<String, BTreeMap<String, TableRow>> {
    let mut table: BTreeMap<String, BTreeMap<String, TableRow>> = BTreeMap::new();

    let mut insert = |noun: &str, method: &str, row: TableRow| {
        let rows = table.entry(noun.to_owned()).or_default();
        assert!(
            rows.insert(method.to_owned(), row).is_none(),
            "duplicate noun-table row {noun}::{method}",
        );
    };

    macro_rules! collect {
        (@row $noun:ident, $method:ident, inquiry []) => {
            insert(stringify!($noun), stringify!($method), TableRow::Inquiry);
        };
        (@row $noun:ident, $method:ident, $kind:ident []) => {
            insert(stringify!($noun), stringify!($method), TableRow::Helper);
        };
        (@row $noun:ident, $method:ident, $kind:ident [$command:ident]) => {
            insert(stringify!($noun), stringify!($method), TableRow::Command);
        };

        (
            $(
                @noun $noun:ident { $($header:tt)* };
                $(
                    $(#[$doc:meta])*
                    $kind:ident [$($command:ident)?] $method:ident($($arg:ident: $ty:ty),*) -> $ret:ty
                        $(where $gate:ident $(+ $extra:ident)*)? = [$($request:tt)*];
                )*
            )*
            @exceptions; $($exceptions:tt)*
        ) => {
            $( $( collect!(@row $noun, $method, $kind [$($command)?]); )* )*
        };
    }

    noun_table!(All => collect);
    table
}

/// Count the row categories emitted by the compiled noun-table projection.
fn table_row_counts(table: &BTreeMap<String, BTreeMap<String, TableRow>>) -> (usize, usize, usize) {
    let mut commands = 0;
    let mut inquiries = 0;
    let mut helpers = 0;
    for rows in table.values() {
        for row in rows.values() {
            match row {
                TableRow::Command => commands += 1,
                TableRow::Inquiry => inquiries += 1,
                TableRow::Helper => helpers += 1,
            }
        }
    }
    (commands, inquiries, helpers)
}

/// Reduces a marker path or bare marker to its bare trait name.
///
/// The noun-table `where` clauses and header gates name a bare marker
/// (`HasPower`); the erased accessor metadata stringifies a full path
/// (`crate :: capabilities :: HasPower`). Both collapse to `HasPower` here so the
/// two gate sets can be compared directly.
fn bare_marker(marker: &str) -> String {
    let compact: String = marker.chars().filter(|c| !c.is_whitespace()).collect();
    compact.rsplit("::").next().unwrap_or(&compact).to_owned()
}

/// Projects the static gate the noun surface puts on each built-in inquiry.
///
/// An inquiry row's static gate is its own `where` marker when it has one, and
/// otherwise its noun header's base gate marker. This is exactly
/// the compile-time bound a static `<noun>().<inquiry>()` call resolves, so
/// comparing it against the erased accessor's runtime gate proves the two
/// surfaces cannot drift (#684). Command and helper rows are skipped.
#[must_use]
fn inquiry_static_gates() -> BTreeMap<String, Option<String>> {
    fn last_segment(path: &str) -> String {
        let compact: String = path.chars().filter(|c| !c.is_whitespace()).collect();
        compact.rsplit("::").next().unwrap_or(&compact).to_owned()
    }

    let mut map: BTreeMap<String, Option<String>> = BTreeMap::new();

    macro_rules! collect {
        // Inquiry with a typed `where` marker: the marker is the static gate.
        (@row $base:tt, inquiry [$gate:ident], [$($request:tt)*]) => {
            map.insert(
                last_segment(stringify!($($request)*)),
                Some(bare_marker(stringify!($gate))),
            );
        };
        // Inquiry with no `where`: the static gate is the noun header's base
        // gate.
        (@row [always], inquiry [], [$($request:tt)*]) => {
            map.insert(last_segment(stringify!($($request)*)), None);
        };
        (@row [$base:ident $marker:ident], inquiry [], [$($request:tt)*]) => {
            map.insert(
                last_segment(stringify!($($request)*)),
                Some(bare_marker(stringify!($marker))),
            );
        };
        // Command and helper rows are not inquiries.
        (@row $base:tt, $kind:ident $gate:tt, $request:tt) => {};

        (
            $(
                @noun $noun:ident {
                    accessor: $accessor:ident,
                    getter: $getter:ident,
                    dyn_trait: $dyn_trait:ident,
                    gate: $base:tt,
                    doc: $noun_doc:literal $(,)?
                };
                $(
                    $(#[$doc:meta])*
                    $kind:ident [$($command:ident)?] $method:ident($($arg:ident: $ty:ty),*) -> $ret:ty
                        $(where $gate:ident $(+ $extra:ident)*)? = [$($request:tt)*];
                )*
            )*
            @exceptions; $($exceptions:tt)*
        ) => {
            $( $( collect!(@row $base, $kind [$($gate)?], [$($request)*]); )* )*
        };
    }

    noun_table!(All => collect);
    map
}

/// Maps a static noun to its registry arm name.
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

/// Maps a dynamic noun trait to its registry arm name.
#[must_use]
#[cfg(all(feature = "dyn-api", feature = "async"))]
pub(crate) fn dyn_trait_noun_key(dyn_trait: &str) -> &'static str {
    NOUN_FACADES
        .iter()
        .find(|facade| facade.dyn_trait == dyn_trait)
        .map_or_else(
            || panic!("{dyn_trait} is not a registry noun trait"),
            |facade| facade.key,
        )
}

/// Blanks comments and literal contents while retaining line structure.
fn clean_source(source: &str, label: &str) -> String {
    let chars: Vec<char> = source.chars().collect();
    let mut output = Vec::with_capacity(chars.len());
    let mut index = 0;

    fn blank(character: char) -> char {
        if character == '\n' {
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
                    output.push(' ');
                    index += 1;
                }
            }
            ('/', Some('*')) => {
                let mut depth = 1_usize;
                output.push(' ');
                output.push(' ');
                index += 2;
                while index < chars.len() && depth != 0 {
                    if chars[index] == '/' && chars.get(index + 1) == Some(&'*') {
                        depth += 1;
                        output.push(' ');
                        output.push(' ');
                        index += 2;
                    } else if chars[index] == '*' && chars.get(index + 1) == Some(&'/') {
                        depth -= 1;
                        output.push(' ');
                        output.push(' ');
                        index += 2;
                    } else {
                        output.push(blank(chars[index]));
                        index += 1;
                    }
                }
                assert_eq!(depth, 0, "{label}: unterminated block comment");
            }
            ('"', _) => {
                output.push('"');
                index += 1;
                let mut closed = false;
                while index < chars.len() {
                    let character = chars[index];
                    if character == '\\' {
                        output.push(' ');
                        index += 1;
                        if index < chars.len() {
                            output.push(blank(chars[index]));
                            index += 1;
                        }
                    } else {
                        output.push(if character == '"' {
                            '"'
                        } else {
                            blank(character)
                        });
                        index += 1;
                        if character == '"' {
                            closed = true;
                            break;
                        }
                    }
                }
                assert!(closed, "{label}: unterminated string literal");
            }
            ('\'', _) => {
                // A lifetime starts with `'name`; a character literal closes
                // with another quote and must be blanked like a string.
                let literal = match next {
                    Some('\\') => true,
                    Some(_) => chars.get(index + 2) == Some(&'\''),
                    None => false,
                };
                if literal {
                    output.push(' ');
                    index += 1;
                    while index < chars.len() {
                        let character = chars[index];
                        output.push(blank(character));
                        index += 1;
                        if character == '\'' {
                            break;
                        }
                    }
                } else {
                    output.push('\'');
                    index += 1;
                }
            }
            _ => {
                output.push(current);
                index += 1;
            }
        }
    }

    output.into_iter().collect()
}

/// Returns the index just past a brace-delimited item.
pub(crate) fn block_end(lines: &[&str], start: usize, label: &str) -> usize {
    let mut depth = 0_i32;
    let mut opened = false;
    for (line_index, line) in lines.iter().enumerate().skip(start) {
        for character in line.chars() {
            match character {
                '{' => {
                    depth += 1;
                    opened = true;
                }
                '}' => depth -= 1,
                _ => {}
            }
        }
        if opened && depth <= 0 {
            return line_index + 1;
        }
    }
    panic!("{label}:{}: unterminated block", start + 1);
}

/// Marks test and macro bodies in a cleaned source file.
fn masked_lines(lines: &[&str], label: &str, macros: bool) -> Vec<bool> {
    let mut masked = vec![false; lines.len()];
    let mut index = 0;
    while index < lines.len() {
        let trimmed = lines[index].trim();
        if trimmed == "#[cfg(test)]" || (macros && trimmed.starts_with("macro_rules!")) {
            let end = block_end(lines, index, label);
            for item in masked.iter_mut().take(end).skip(index) {
                *item = true;
            }
            index = end;
        } else {
            index += 1;
        }
    }
    masked
}

/// Returns the source with `#[cfg(test)]` items replaced by blank lines.
pub(crate) fn without_test_modules(source: &str, label: &str) -> String {
    let cleaned = clean_source(source, label);
    let lines: Vec<&str> = cleaned.lines().collect();
    let masked = masked_lines(&lines, label, false);
    source
        .lines()
        .zip(masked)
        .map(|(line, is_masked)| if is_masked { "" } else { line })
        .collect::<Vec<_>>()
        .join("\n")
}

/// Returns cleaned declaration lines, optionally excluding macro definitions.
#[cfg(all(feature = "async", feature = "blocking"))]
pub(crate) fn declaration_lines(source: &str, label: &str, macros: bool) -> Vec<String> {
    let cleaned = clean_source(source, label);
    let lines: Vec<&str> = cleaned.lines().collect();
    let masked = masked_lines(&lines, label, macros);
    lines
        .into_iter()
        .zip(masked)
        .map(|(line, is_masked)| {
            if is_masked {
                String::new()
            } else {
                line.to_owned()
            }
        })
        .collect()
}

/// Extracts the parenthesized body of one macro invocation.
#[cfg(all(feature = "dyn-api", feature = "async"))]
fn invocation_body<'a>(text: &'a str, label: &str) -> (&'a str, usize) {
    assert!(
        text.starts_with('('),
        "{label}: macro invocation has no `(`"
    );
    let mut depth = 0_i32;
    for (index, character) in text.char_indices() {
        match character {
            '(' => depth += 1,
            ')' => {
                depth -= 1;
                if depth == 0 {
                    return (&text[1..index], index + 1);
                }
            }
            _ => {}
        }
    }
    panic!("{label}: unterminated macro invocation");
}

/// Returns the registry noun arms consumed by one facade macro.
///
/// This is the only source check that follows `noun_table!`; it reads the
/// invocation's two identifiers and does not inspect any generated row.
#[must_use]
#[cfg(all(feature = "dyn-api", feature = "async"))]
pub(crate) fn consumed_nouns(
    source: &str,
    label: &'static str,
    consumer: &str,
) -> BTreeSet<String> {
    let declarations = without_test_modules(source, label);
    let cleaned = clean_source(&declarations, label);
    let mut consumed = BTreeSet::new();
    let mut offset = 0;

    while let Some(relative) = cleaned[offset..].find("noun_table!") {
        let start = offset + relative + "noun_table!".len();
        let rest = &cleaned[start..];
        if !rest.starts_with('(') {
            offset = start;
            continue;
        }
        let (body, consumed_length) = invocation_body(rest, label);
        let Some((noun, named_consumer)) = body.split_once("=>") else {
            panic!("{label}: noun_table! invocation has no consumer");
        };
        if named_consumer.trim() == consumer {
            let noun = noun.trim();
            assert!(
                NOUN_KEYS.contains(&noun),
                "{label}: {consumer} consumes unknown noun arm {noun:?}",
            );
            assert!(
                consumed.insert(noun.to_owned()),
                "{label}: {consumer} consumes {noun} more than once",
            );
        }
        offset = start + consumed_length;
    }

    consumed
}

#[test]
fn compiled_registry_inventory_counts_remain_readable() {
    let mut target = 0;
    let mut exceptions = 0;
    let mut nouns = BTreeSet::new();
    for command in BuiltinCommand::ALL {
        let entry = surface_entry(*command);
        assert_eq!(entry.class, command.classification());
        match entry.disposition {
            StaticSurfaceDisposition::Noun { noun, .. } => {
                target += 1;
                nouns.insert(noun_table_key(noun));
            }
            StaticSurfaceDisposition::BroadcastHandshake { .. }
            | StaticSurfaceDisposition::InternalCancellation { .. } => exceptions += 1,
        }
    }
    assert_eq!(BuiltinCommand::ALL.len(), BUILTIN_COMMAND_COUNT); // 149 commands
    assert_eq!(target, TARGET_FACING_COMMAND_COUNT); // 146 target-facing
    assert_eq!(exceptions, NON_NOUN_COMMAND_COUNT); // 3 protocol exceptions
    assert_eq!(nouns.len(), 14); // 14 nouns
    assert_eq!(BUILTIN_INQUIRY_ACCESSORS.len(), 62); // 62 typed inquiries

    // The compiled noun-table projection has one row for each target-facing
    // command ID, one for each typed inquiry, and one for each empty-ID
    // convenience helper. Keep all four totals visible so a helper cannot be
    // mistaken for a command row or silently disappear from a facade.
    let table = table_surface();
    let (command_rows, inquiry_rows, helper_rows) = table_row_counts(&table);
    assert_eq!(command_rows, 146); // 146 command-method rows
    assert_eq!(inquiry_rows, 62); // 62 inquiry rows
    assert_eq!(helper_rows, 8); // 8 empty-ID helper rows
    assert_eq!(command_rows + inquiry_rows + helper_rows, 216); // 216 total rows

    #[cfg(all(feature = "dyn-api", feature = "async"))]
    {
        use crate::dynapi::DYN_NOUN_CONVENIENCE_METHODS;

        let mut registry_helpers = BTreeSet::new();
        for facade in NOUN_FACADES {
            let rows = table.get(facade.key);
            assert!(
                rows.is_some(),
                "every registry noun has a projected row map"
            );
            if let Some(rows) = rows {
                for (method, row) in rows {
                    if matches!(row, TableRow::Helper) {
                        registry_helpers.insert((facade.dyn_trait.to_owned(), method.to_owned()));
                    }
                }
            }
        }
        let declared_helpers: BTreeSet<(String, String)> = DYN_NOUN_CONVENIENCE_METHODS
            .iter()
            .map(|(noun, method)| ((*noun).to_owned(), (*method).to_owned()))
            .collect();
        assert_eq!(declared_helpers, registry_helpers);
    }
}

/// Issue #684: the erased inquiry surface gates each built-in inquiry on exactly
/// the marker the static noun surface resolves, so the two gate sets cannot
/// drift. The static gate is the inquiry's own `where` marker, or its noun's
/// base-domain marker when the row carries none; the erased gate is the runtime
/// accessor gate recorded in [`BUILTIN_INQUIRY_ACCESSORS`].
#[test]
fn erased_inquiry_gates_match_the_static_noun_surface() {
    // One inquiry carries a static base marker that the shared accessor gate
    // deliberately leaves `Always`, for a reason unrelated to drift:
    // `MenuOpenCloseInquiry` has no runtime menu capability to gate on: a
    // runtime `ProfileSpec` cannot express "no menu", so basic OSD menu
    // stays universally reachable, exactly like the menu commands.
    const ACCESSOR_UNGATED_EXCEPTIONS: &[&str] = &["MenuOpenCloseInquiry"];

    let static_gates = inquiry_static_gates();

    // The two projections describe the same closed set of inquiries.
    let accessor_commands: BTreeSet<&str> = BUILTIN_INQUIRY_ACCESSORS
        .iter()
        .map(|accessor| accessor.command.name())
        .collect();
    let table_commands: BTreeSet<&str> = static_gates.keys().map(String::as_str).collect();
    assert_eq!(
        accessor_commands, table_commands,
        "the erased accessor inventory and the noun-table inquiry rows must cover \
         the same commands",
    );

    for accessor in BUILTIN_INQUIRY_ACCESSORS {
        let command = accessor.command.name();
        let erased = match accessor.profile_gate {
            BuiltinInquiryProfileGate::Always => None,
            BuiltinInquiryProfileGate::Capability { marker } => Some(bare_marker(marker)),
        };
        let expected = static_gates
            .get(command)
            .unwrap_or_else(|| panic!("accessor command {command} has no noun-table row"))
            .clone();

        if ACCESSOR_UNGATED_EXCEPTIONS.contains(&command) {
            assert_eq!(
                erased, None,
                "{command} is a documented accessor-ungated exception",
            );
            assert!(
                expected.is_some(),
                "{command} exception must still name a static marker enforced elsewhere",
            );
            continue;
        }

        assert_eq!(
            erased, expected,
            "erased and static inquiry gates disagree for {command}",
        );
    }
}

/// The vendor command validators consult `typed_surface_for_command`, which is
/// projected from the static noun table. Keep the closed command set explicit
/// here so adding a runtime family check cannot silently bypass its static
/// `where Has*` gate.
#[test]
fn dynamic_vendor_command_gates_match_the_static_noun_surface() {
    const GATES: &[(BuiltinCommand, TypedSupportSurface)] = &[
        (
            BuiltinCommand::ExposureMode,
            TypedSupportSurface::ExposureMode,
        ),
        (
            BuiltinCommand::AntiFlicker,
            TypedSupportSurface::PtzOpticsAntiFlicker,
        ),
        (
            BuiltinCommand::SettingsSave,
            TypedSupportSurface::PtzOpticsSettingsSave,
        ),
        (
            BuiltinCommand::PresetRecallSpeed,
            TypedSupportSurface::PtzOpticsPresetRecallSpeed,
        ),
        (
            BuiltinCommand::SpotlightOn,
            TypedSupportSurface::SonySpotlight,
        ),
        (
            BuiltinCommand::SpotlightOff,
            TypedSupportSurface::SonySpotlight,
        ),
        (
            BuiltinCommand::AutoSlowShutterOn,
            TypedSupportSurface::SonyAutoSlowShutter,
        ),
        (
            BuiltinCommand::AutoSlowShutterOff,
            TypedSupportSurface::SonyAutoSlowShutter,
        ),
        (
            BuiltinCommand::MulticastStreamingOn,
            TypedSupportSurface::PtzOpticsMulticastStreaming,
        ),
        (
            BuiltinCommand::MulticastStreamingOff,
            TypedSupportSurface::PtzOpticsMulticastStreaming,
        ),
        (
            BuiltinCommand::NdiQuality,
            TypedSupportSurface::PtzOpticsNdiQuality,
        ),
    ];

    for &(command, surface) in GATES {
        let StaticSurfaceDisposition::Noun { marker, .. } = surface_entry(command).disposition
        else {
            panic!("{command:?} must remain a static noun command");
        };
        assert_eq!(marker, StaticMarkerRequirement::Typed(surface));
        assert_eq!(typed_surface_for_command(command), Some(surface));
    }
}

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
    #[cfg(all(feature = "dyn-api", feature = "async"))]
    fn consumed_nouns_reads_only_macro_invocation_identifiers() {
        let source = "noun_table!(Power => consumer);\nnoun_table!(Zoom => other);\n";
        assert_eq!(
            consumed_nouns(source, "fixture", "consumer"),
            BTreeSet::from(["Power".to_owned()]),
        );
    }
}
