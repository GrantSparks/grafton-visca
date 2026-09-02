//! Narrow parity guards for the table-driven noun facades.
//!
//! The noun registry in [`crate::noun_table`] is the source of the method
//! names, arguments, request builders, rustdoc, and return classes consumed by
//! the async, blocking, and dynamic facades. Re-reading those facts from
//! generated source would only compare a file with its own expansion. The
//! compiled registry and the exhaustive [`crate::command::surface`] projection
//! already make that disagreement impossible.
//!
//! This module therefore keeps only checks that the type system and macro
//! expansion cannot express:
//!
//! * every facade must invoke every noun arm;
//! * the four hand-written Motion methods are the only residual noun methods;
//! * an accessor extension-trait implementation cannot smuggle in a method;
//! * static facade capability names must be direct imports of the real
//!   `crate::capabilities` traits.
//!
//! The source reader below is intentionally not a Rust parser. It strips
//! comments/literals, masks test and macro bodies, and then looks only for
//! those four structural declarations. There is no table/request parsing and
//! no generated-signature cross-comparison: both are redundant now that one
//! registry expands all three facades.

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

const ASYNC_SOURCE: &str = include_str!("async_nouns.rs");
const BLOCKING_SOURCE: &str = include_str!("blocking_nouns.rs");
const DYN_SOURCE: &str = include_str!("dynapi/nouns.rs");

/// Motion is a safety/observation view rather than a ledger noun.
const MOTION_ACCESSOR: &str = "MotionAccessor";
const MOTION_TRAIT: &str = "DynMotion";

/// The only noun methods that are intentionally still hand-written.
const MOTION_METHODS: &[&str] = &[
    "is_moving",
    "is_moving_axes",
    "stop_all_motion",
    "wait_until_idle",
];

/// Traits that may legally be implemented for an accessor type.
const ALLOWED_ACCESSOR_TRAIT_IMPLS: &[&str] = &["std::fmt::Debug", "fmt::Debug"];

/// The 14 registry noun arms, in the same order as the public facades.
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
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct NounFacade {
    key: &'static str,
    accessor: &'static str,
    dyn_trait: &'static str,
}

const NOUN_FACADES: &[NounFacade] = &[
    NounFacade {
        key: "Power",
        accessor: "PowerAccessor",
        dyn_trait: "DynPower",
    },
    NounFacade {
        key: "Zoom",
        accessor: "ZoomAccessor",
        dyn_trait: "DynZoom",
    },
    NounFacade {
        key: "System",
        accessor: "SystemAccessor",
        dyn_trait: "DynSystem",
    },
    NounFacade {
        key: "PanTilt",
        accessor: "PanTiltAccessor",
        dyn_trait: "DynPanTilt",
    },
    NounFacade {
        key: "Focus",
        accessor: "FocusAccessor",
        dyn_trait: "DynFocus",
    },
    NounFacade {
        key: "Presets",
        accessor: "PresetsAccessor",
        dyn_trait: "DynPresets",
    },
    NounFacade {
        key: "Exposure",
        accessor: "ExposureAccessor",
        dyn_trait: "DynExposure",
    },
    NounFacade {
        key: "WhiteBalance",
        accessor: "WhiteBalanceAccessor",
        dyn_trait: "DynWhiteBalance",
    },
    NounFacade {
        key: "Image",
        accessor: "ImageAccessor",
        dyn_trait: "DynImage",
    },
    NounFacade {
        key: "Tally",
        accessor: "TallyAccessor",
        dyn_trait: "DynTally",
    },
    NounFacade {
        key: "NdFilter",
        accessor: "NdFilterAccessor",
        dyn_trait: "DynNdFilter",
    },
    NounFacade {
        key: "MotionSync",
        accessor: "MotionSyncAccessor",
        dyn_trait: "DynMotionSync",
    },
    NounFacade {
        key: "Menu",
        accessor: "MenuAccessor",
        dyn_trait: "DynMenu",
    },
    NounFacade {
        key: "Advanced",
        accessor: "AdvancedAccessor",
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

    macro_rules! collect {
        (@row_kind []) => {
            TableRow::Helper
        };
        (@row_kind [$($command:ident),+]) => {
            TableRow::Command
        };

        (@noun $noun:ident; $($rows:tt)*) => {
            collect!(@rows stringify!($noun); $($rows)*);
        };

        (@rows $noun:expr; @noun $next:ident; $($rest:tt)*) => {
            collect!(@rows stringify!($next); $($rest)*);
        };

        (@rows $noun:expr; @exceptions; $($rest:tt)*) => {};
        (@rows $noun:expr;) => {};

        (@rows $noun:expr; $(#[$doc:meta])*
            inquiry $($inquiry:ident)::+ $method:ident() -> $response:ty
                $(where $gate:tt $(+ $extra:tt)*)? = $request:expr;
            $($rest:tt)*) => {
            let rows = table.entry($noun.to_owned()).or_default();
            assert!(
                rows.insert(stringify!($method).to_owned(), TableRow::Inquiry).is_none(),
                "duplicate noun-table row {}::{}",
                $noun,
                stringify!($method),
            );
            collect!(@rows $noun; $($rest)*);
        };

        (@rows $noun:expr; $(#[$doc:meta])*
            $kind:ident [$($command:ident),*] $method:ident(
                $($arg:ident: $ty:ty),*
            ) $(-> $request_ty:ty)? $(where $gate:tt $(+ $extra:tt)*)?
                = checked $request:expr;
            $($rest:tt)*) => {
            let rows = table.entry($noun.to_owned()).or_default();
            assert!(
                rows.insert(
                    stringify!($method).to_owned(),
                    collect!(@row_kind [$($command),*]),
                )
                .is_none(),
                "duplicate noun-table row {}::{}",
                $noun,
                stringify!($method),
            );
            collect!(@rows $noun; $($rest)*);
        };

        (@rows $noun:expr; $(#[$doc:meta])*
            $kind:ident [$($command:ident),*] $method:ident(
                $($arg:ident: $ty:ty),*
            ) $(-> $request_ty:ty)? $(where $gate:tt $(+ $extra:tt)*)?
                = with_profile |$profile:ident| $request:expr;
            $($rest:tt)*) => {
            let rows = table.entry($noun.to_owned()).or_default();
            assert!(
                rows.insert(
                    stringify!($method).to_owned(),
                    collect!(@row_kind [$($command),*]),
                )
                .is_none(),
                "duplicate noun-table row {}::{}",
                $noun,
                stringify!($method),
            );
            collect!(@rows $noun; $($rest)*);
        };

        (@rows $noun:expr; $(#[$doc:meta])*
            $kind:ident [$($command:ident),*] $method:ident(
                $($arg:ident: $ty:ty),*
            ) $(-> $request_ty:ty)? $(where $gate:tt $(+ $extra:tt)*)?
                = with_core |$core:ident| $request:expr;
            $($rest:tt)*) => {
            let rows = table.entry($noun.to_owned()).or_default();
            assert!(
                rows.insert(
                    stringify!($method).to_owned(),
                    collect!(@row_kind [$($command),*]),
                )
                .is_none(),
                "duplicate noun-table row {}::{}",
                $noun,
                stringify!($method),
            );
            collect!(@rows $noun; $($rest)*);
        };

        (@rows $noun:expr; $(#[$doc:meta])*
            $kind:ident [$($command:ident),*] $method:ident(
                $($arg:ident: $ty:ty),*
            ) $(-> $request_ty:ty)? $(where $gate:tt $(+ $extra:tt)*)?
                = delegate $target:ident($($delegated:expr),*);
            $($rest:tt)*) => {
            let rows = table.entry($noun.to_owned()).or_default();
            assert!(
                rows.insert(
                    stringify!($method).to_owned(),
                    collect!(@row_kind [$($command),*]),
                )
                .is_none(),
                "duplicate noun-table row {}::{}",
                $noun,
                stringify!($method),
            );
            collect!(@rows $noun; $($rest)*);
        };

        (@rows $noun:expr; $(#[$doc:meta])*
            $kind:ident [$($command:ident),*] $method:ident(
                $($arg:ident: $ty:ty),*
            ) $(-> $request_ty:ty)? $(where $gate:tt $(+ $extra:tt)*)?
                = $request:expr;
            $($rest:tt)*) => {
            let rows = table.entry($noun.to_owned()).or_default();
            assert!(
                rows.insert(
                    stringify!($method).to_owned(),
                    collect!(@row_kind [$($command),*]),
                )
                .is_none(),
                "duplicate noun-table row {}::{}",
                $noun,
                stringify!($method),
            );
            collect!(@rows $noun; $($rest)*);
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
/// The noun-table `where` clauses and [`noun_marker`] name a bare marker
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
/// otherwise its noun's base-domain marker ([`noun_marker`]). This is exactly
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
        (@noun $noun:ident; $($rows:tt)*) => {
            collect!(@rows $noun; $($rows)*);
        };
        (@rows $noun:ident; @noun $next:ident; $($rest:tt)*) => {
            collect!(@rows $next; $($rest)*);
        };
        (@rows $noun:ident; @exceptions; $($rest:tt)*) => {};
        (@rows $noun:ident;) => {};

        // Inquiry with a typed `where` marker: the marker is the static gate.
        (@rows $noun:ident; $(#[$doc:meta])*
            inquiry $($inquiry:ident)::+ $method:ident() -> $response:ty
                where $gate:ident $(+ $extra:ident)* = $request:expr;
            $($rest:tt)*) => {
            map.insert(
                last_segment(stringify!($($inquiry)::+)),
                Some(bare_marker(stringify!($gate))),
            );
            collect!(@rows $noun; $($rest)*);
        };
        // Inquiry with no `where`: the static gate is the noun's base marker.
        (@rows $noun:ident; $(#[$doc:meta])*
            inquiry $($inquiry:ident)::+ $method:ident() -> $response:ty = $request:expr;
            $($rest:tt)*) => {
            map.insert(
                last_segment(stringify!($($inquiry)::+)),
                noun_marker(StaticNoun::$noun).map(bare_marker),
            );
            collect!(@rows $noun; $($rest)*);
        };

        // Command rows carry an explicit request form; none are inquiries, so
        // each form simply recurses past the row.
        (@rows $noun:ident; $(#[$doc:meta])*
            $kind:ident [$($command:ident),*] $method:ident($($arg:ident: $ty:ty),*)
                $(-> $request_ty:ty)? $(where $gate:ident $(+ $extra:ident)*)? = checked $request:expr;
            $($rest:tt)*) => {
            collect!(@rows $noun; $($rest)*);
        };
        (@rows $noun:ident; $(#[$doc:meta])*
            $kind:ident [$($command:ident),*] $method:ident($($arg:ident: $ty:ty),*)
                $(-> $request_ty:ty)? $(where $gate:ident $(+ $extra:ident)*)?
                = with_profile |$profile:ident| $request:expr;
            $($rest:tt)*) => {
            collect!(@rows $noun; $($rest)*);
        };
        (@rows $noun:ident; $(#[$doc:meta])*
            $kind:ident [$($command:ident),*] $method:ident($($arg:ident: $ty:ty),*)
                $(-> $request_ty:ty)? $(where $gate:ident $(+ $extra:ident)*)?
                = with_core |$core:ident| $request:expr;
            $($rest:tt)*) => {
            collect!(@rows $noun; $($rest)*);
        };
        (@rows $noun:ident; $(#[$doc:meta])*
            $kind:ident [$($command:ident),*] $method:ident($($arg:ident: $ty:ty),*)
                $(where $gate:ident $(+ $extra:ident)*)? = delegate $target:ident($($delegated:expr),*);
            $($rest:tt)*) => {
            collect!(@rows $noun; $($rest)*);
        };
        (@rows $noun:ident; $(#[$doc:meta])*
            $kind:ident [$($command:ident),*] $method:ident($($arg:ident: $ty:ty),*)
                $(-> $request_ty:ty)? $(where $gate:ident $(+ $extra:ident)*)? = $request:expr;
            $($rest:tt)*) => {
            collect!(@rows $noun; $($rest)*);
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
fn block_end(lines: &[&str], start: usize, label: &str) -> usize {
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
fn declaration_lines(source: &str, label: &str, macros: bool) -> Vec<String> {
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

/// Returns a joined top-level item header and the line containing its `{`.
fn item_header(lines: &[String], start: usize, label: &str) -> (String, usize) {
    let mut text = String::new();
    let mut depth = 0_i32;
    for (line_index, line) in lines.iter().enumerate().skip(start) {
        for character in line.chars() {
            if character == '{' && depth == 0 {
                let prefix = line
                    .find('{')
                    .map_or(line.as_str(), |offset| &line[..offset])
                    .trim();
                let header = if text.is_empty() {
                    prefix.to_owned()
                } else if prefix.is_empty() {
                    text.trim().to_owned()
                } else {
                    format!("{} {prefix}", text.trim())
                };
                return (header, line_index);
            }
            match character {
                '(' | '[' | '<' => depth += 1,
                ')' | ']' => depth -= 1,
                '>' if depth > 0 => depth -= 1,
                _ => {}
            }
        }
        if !line.trim().is_empty() {
            if !text.is_empty() {
                text.push(' ');
            }
            text.push_str(line.trim());
        }
    }
    panic!("{label}:{}: item has no body", start + 1);
}

/// Returns the first identifier in a declaration tail.
fn first_identifier(text: &str) -> &str {
    let end = text
        .find(|character: char| !character.is_ascii_alphanumeric() && character != '_')
        .unwrap_or(text.len());
    &text[..end]
}

/// Returns the public methods written in inherent accessor impls.
fn accessor_methods(source: &str, label: &str) -> BTreeMap<String, BTreeSet<String>> {
    let lines = declaration_lines(source, label, true);
    let refs: Vec<&str> = lines.iter().map(String::as_str).collect();
    let mut methods: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();

    for index in 0..lines.len() {
        let trimmed = lines[index].trim();
        if !trimmed.starts_with("impl") {
            continue;
        }
        let (header, body_line) = item_header(&lines, index, label);
        if header.contains(" for ") {
            continue;
        }
        let Some(accessor) = NOUN_FACADES
            .iter()
            .map(|facade| facade.accessor)
            .chain(std::iter::once(MOTION_ACCESSOR))
            .find(|name| header.contains(name))
        else {
            continue;
        };
        let end = block_end(&refs, index, label);
        for line in lines.iter().take(end).skip(body_line + 1) {
            let trimmed = line.trim();
            if !trimmed.starts_with("pub ") || trimmed.starts_with("pub(") {
                continue;
            }
            let mut declaration = trimmed.strip_prefix("pub ").unwrap_or(trimmed);
            if let Some(rest) = declaration.strip_prefix("async ") {
                declaration = rest;
            }
            if let Some(rest) = declaration.strip_prefix("unsafe ") {
                declaration = rest;
            }
            let Some(rest) = declaration.strip_prefix("fn ") else {
                continue;
            };
            let name = first_identifier(rest);
            if !name.is_empty() {
                methods
                    .entry(accessor.to_owned())
                    .or_default()
                    .insert(name.to_owned());
            }
        }
    }

    methods
}

/// Returns the methods declared directly in dynamic noun traits.
fn dynamic_trait_methods(source: &str, label: &str) -> BTreeMap<String, BTreeSet<String>> {
    let lines = declaration_lines(source, label, true);
    let refs: Vec<&str> = lines.iter().map(String::as_str).collect();
    let mut methods = BTreeMap::new();

    for index in 0..lines.len() {
        let trimmed = lines[index].trim();
        let Some(rest) = trimmed.strip_prefix("pub trait ") else {
            continue;
        };
        let name = first_identifier(rest);
        if !name.starts_with("Dyn") || name == "DynSessionCameraNouns" {
            continue;
        }
        let (_, body_line) = item_header(&lines, index, label);
        let end = block_end(&refs, index, label);
        let mut trait_methods = BTreeSet::new();
        for line in lines.iter().take(end).skip(body_line + 1) {
            let trimmed = line.trim();
            let declaration = trimmed
                .strip_prefix("async fn ")
                .or_else(|| trimmed.strip_prefix("fn "));
            if let Some(rest) = declaration {
                let method = first_identifier(rest);
                if !method.is_empty() {
                    trait_methods.insert(method.to_owned());
                }
            }
        }
        methods.insert(name.to_owned(), trait_methods);
    }

    methods
}

/// Rejects extension-trait impls that add methods to an accessor.
fn assert_accessor_trait_impls(source: &str, label: &str) {
    let lines = declaration_lines(source, label, true);
    let refs: Vec<&str> = lines.iter().map(String::as_str).collect();
    for index in 0..lines.len() {
        if !lines[index].trim().starts_with("impl") {
            continue;
        }
        let (header, _) = item_header(&lines, index, label);
        let Some((trait_path, target)) = header.split_once(" for ") else {
            continue;
        };
        if !NOUN_FACADES
            .iter()
            .map(|facade| facade.accessor)
            .chain(std::iter::once(MOTION_ACCESSOR))
            .any(|accessor| target.contains(accessor))
        {
            continue;
        }
        let trait_path = trait_path
            .trim_start_matches("impl")
            .trim()
            .trim_start_matches('<');
        assert!(
            ALLOWED_ACCESSOR_TRAIT_IMPLS
                .iter()
                .any(|allowed| trait_path.contains(allowed)),
            "{label}: extension trait impl {trait_path:?} for accessor {target:?} is not allowed",
        );
        // Keep the block walk here so malformed/missing braces fail with the
        // same useful source location as the other structural guards.
        let _ = block_end(&refs, index, label);
    }
}

/// Maps a typed support capability to the marker imported by static facades.
const fn typed_marker(surface: TypedSupportSurface) -> &'static str {
    match surface {
        TypedSupportSurface::DirectZoom => "HasDirectZoom",
        TypedSupportSurface::DigitalZoomToggle => "HasDigitalZoomToggle",
        TypedSupportSurface::DigitalZoomRange => "HasDigitalZoomRange",
        TypedSupportSurface::ExposureMode => "HasExposureMode",
        TypedSupportSurface::IrisControl => "HasIrisControl",
        TypedSupportSurface::IrisControlInquiry => "HasIrisControlInquiry",
        TypedSupportSurface::OnePushFocus => "HasOnePushFocus",
        TypedSupportSurface::PtzOpticsSnapFocus => "HasPtzOpticsSnapFocus",
        TypedSupportSurface::PtzOpticsAntiFlicker => "HasPtzOpticsAntiFlicker",
        TypedSupportSurface::PtzOpticsSettingsSave => "HasPtzOpticsSettingsSave",
        TypedSupportSurface::PtzOpticsPresetRecallSpeed => "HasPtzOpticsPresetRecallSpeed",
        TypedSupportSurface::SonySpotlight => "HasSonySpotlight",
        TypedSupportSurface::SonyAutoSlowShutter => "HasSonyAutoSlowShutter",
        TypedSupportSurface::PtzOpticsMulticastStreaming => "HasPtzOpticsMulticastStreaming",
        TypedSupportSurface::PtzOpticsNdiQuality => "HasPtzOpticsNdiQuality",
        TypedSupportSurface::FocusLock => "HasFocusLock",
        TypedSupportSurface::PushAutoFocus => "HasPushAutoFocus",
        TypedSupportSurface::FocusZone => "HasFocusZone",
        TypedSupportSurface::FocusZoneInquiry => "HasFocusZoneInquiry",
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
        TypedSupportSurface::NoiseReduction2D => "HasNoiseReduction2D",
        TypedSupportSurface::NoiseReduction3D => "HasNoiseReduction3D",
        TypedSupportSurface::NoiseReduction2DControl => "HasNoiseReduction2DControl",
        TypedSupportSurface::NoiseReduction3DControl => "HasNoiseReduction3DControl",
        TypedSupportSurface::PictureEffect => "HasPictureEffect",
        TypedSupportSurface::Tally => "HasTally",
        TypedSupportSurface::DirectMenu => "HasDirectMenuControl",
        TypedSupportSurface::NdFilter => "HasNdFilter",
        TypedSupportSurface::VariableSpeed => "HasVariableSpeed",
        TypedSupportSurface::MotionSync => "HasMotionSync",
        TypedSupportSurface::UsbAudio => "HasUsbAudio",
    }
}

/// Returns the marker used by the default gate on one static noun.
const fn noun_marker(noun: StaticNoun) -> Option<&'static str> {
    match noun {
        StaticNoun::Power => Some("HasPower"),
        StaticNoun::Zoom => Some("HasZoom"),
        StaticNoun::System => None,
        StaticNoun::PanTilt => Some("HasPanTilt"),
        StaticNoun::Focus => Some("HasFocus"),
        StaticNoun::Exposure => Some("HasExposure"),
        StaticNoun::WhiteBalance => Some("HasWhiteBalance"),
        StaticNoun::Image => Some("HasImageProcessing"),
        StaticNoun::Presets => Some("HasPresets"),
        StaticNoun::Tally => Some("HasTally"),
        StaticNoun::NdFilter => Some("HasNdFilter"),
        StaticNoun::MotionSync => Some("HasMotionSync"),
        StaticNoun::Menu => Some("HasMenuControl"),
        StaticNoun::Advanced => None,
    }
}

/// Default markers on every static noun accessor.
///
/// These bounds exist even when a noun's particular command row is narrowed
/// by an optional support marker, and inquiries can be the only row that uses
/// one of them.
const STATIC_NOUN_DEFAULTS: &[StaticNoun] = &[
    StaticNoun::Power,
    StaticNoun::Zoom,
    StaticNoun::System,
    StaticNoun::PanTilt,
    StaticNoun::Focus,
    StaticNoun::Presets,
    StaticNoun::Exposure,
    StaticNoun::WhiteBalance,
    StaticNoun::Image,
    StaticNoun::Tally,
    StaticNoun::NdFilter,
    StaticNoun::MotionSync,
    StaticNoun::Menu,
    StaticNoun::Advanced,
];

/// All marker names that a static facade must resolve directly.
///
/// This is projected from actual static noun defaults, command rows, and
/// generated inquiry accessors. It must not start from every possible typed
/// support surface: a runtime-only support bit does not require a static
/// facade import.
fn required_markers() -> BTreeSet<String> {
    let mut markers = BTreeSet::new();

    for noun in STATIC_NOUN_DEFAULTS {
        if let Some(marker) = noun_marker(*noun) {
            markers.insert(marker.to_owned());
        }
    }

    for command in BuiltinCommand::ALL {
        let StaticSurfaceDisposition::Noun { marker, .. } = surface_entry(*command).disposition
        else {
            continue;
        };
        match marker {
            StaticMarkerRequirement::None => {}
            StaticMarkerRequirement::Profile(marker) => {
                markers.insert(marker.to_owned());
            }
            StaticMarkerRequirement::Typed(surface) => {
                markers.insert(typed_marker(surface).to_owned());
            }
        }
    }

    for accessor in BUILTIN_INQUIRY_ACCESSORS {
        if let BuiltinInquiryProfileGate::Capability { marker } = accessor.profile_gate {
            markers.insert(bare_marker(marker));
        }
    }

    markers
}

#[test]
fn direct_import_markers_follow_static_noun_and_inquiry_gates() {
    let markers = required_markers();

    assert!(markers.contains("HasDirectZoom"));
    assert!(
        markers.contains("HasExposureMode"),
        "shared exposure-mode static gates require their direct imports"
    );
    assert!(
        markers.contains("HasFocusZoneInquiry"),
        "inquiry-only static gates still require their direct imports"
    );
    assert!(
        !markers.contains("HasDigitalZoomRange"),
        "DigitalZoomRange is runtime-only after the normalized optical zoom gate was narrowed"
    );
}

/// Splits the body of a capability import, rejecting aliases and globs.
fn imported_capability_names(source: &str, label: &str) -> BTreeSet<String> {
    let lines = declaration_lines(source, label, false);
    let mut imported = BTreeSet::new();
    let mut index = 0;
    while index < lines.len() {
        if !lines[index].trim().starts_with("use ") {
            index += 1;
            continue;
        }
        let (statement, next) = statement_until_semicolon(&lines, index, label);
        assert!(
            !statement.split_whitespace().any(|word| word == "as"),
            "{label}: capability aliases are not allowed: {statement:?}",
        );
        let compact: String = statement
            .chars()
            .filter(|character| !character.is_whitespace())
            .collect();
        let Some(capability) = compact.find("capabilities::") else {
            index = next;
            continue;
        };
        assert!(
            compact.starts_with("usecrate::{") || compact.starts_with("usecrate::capabilities::"),
            "{label}: capability import is not rooted at crate::capabilities: {statement:?}",
        );
        let after = &compact[capability + "capabilities::".len()..];
        if after.starts_with('*') {
            panic!("{label}: capability glob import defeats resolution");
        }
        if !after.starts_with('{') {
            let name = first_identifier(after);
            if !name.is_empty() {
                imported.insert(name.to_owned());
            }
            index = next;
            continue;
        }
        let (body, _) = braced_body(after, label);
        for item in body
            .split(',')
            .map(str::trim)
            .filter(|item| !item.is_empty())
        {
            let name = first_identifier(item);
            if !name.is_empty() {
                imported.insert(name.to_owned());
            }
        }
        index = next;
    }
    imported
}

/// Returns a statement through its top-level semicolon.
fn statement_until_semicolon(lines: &[String], start: usize, label: &str) -> (String, usize) {
    let mut statement = String::new();
    let mut depth = 0_i32;
    for (index, line) in lines.iter().enumerate().skip(start) {
        for character in line.chars() {
            if character == ';' && depth == 0 {
                return (statement, index + 1);
            }
            statement.push(character);
            match character {
                '{' | '[' | '(' => depth += 1,
                '}' | ']' | ')' => depth -= 1,
                _ => {}
            }
        }
        statement.push(' ');
    }
    panic!("{label}:{}: use statement has no semicolon", start + 1);
}

/// Returns a brace body and the byte offset after its closing brace.
fn braced_body<'a>(text: &'a str, label: &str) -> (&'a str, usize) {
    assert!(
        text.starts_with('{'),
        "{label}: expected capability import body"
    );
    let mut depth = 0_i32;
    for (index, character) in text.char_indices() {
        match character {
            '{' => depth += 1,
            '}' => {
                depth -= 1;
                if depth == 0 {
                    return (&text[1..index], index + 1);
                }
            }
            _ => {}
        }
    }
    panic!("{label}: unbalanced capability import braces");
}

/// Compares direct imports against the registry's required marker names.
fn assert_real_capability_imports(source: &str, label: &str) {
    let expected = required_markers();
    let imported = imported_capability_names(source, label);
    for marker in expected {
        assert!(
            imported.contains(&marker),
            "{label}: capability marker {marker} is not imported directly from crate::capabilities",
        );
    }
}

/// Returns whether each source has exactly the expected residual Motion methods.
fn assert_motion_and_no_residuals() {
    for (label, source) in [
        ("src/async_nouns.rs", ASYNC_SOURCE),
        ("src/blocking_nouns.rs", BLOCKING_SOURCE),
    ] {
        assert_accessor_trait_impls(source, label);
        let methods = accessor_methods(source, label);
        let expected: BTreeSet<String> = MOTION_METHODS
            .iter()
            .map(|name| (*name).to_owned())
            .collect();
        assert_eq!(
            methods.get(MOTION_ACCESSOR).cloned().unwrap_or_default(),
            expected,
            "{label}: MotionAccessor handwritten methods changed",
        );
        for (accessor, names) in methods {
            if accessor != MOTION_ACCESSOR {
                assert!(
                    names.is_empty(),
                    "{label}: generated accessor {accessor} contains handwritten methods {names:?}",
                );
            }
        }
        assert_real_capability_imports(source, label);
    }

    assert_accessor_trait_impls(DYN_SOURCE, "src/dynapi/nouns.rs");
    let dynamic = dynamic_trait_methods(DYN_SOURCE, "src/dynapi/nouns.rs");
    let expected: BTreeSet<String> = MOTION_METHODS
        .iter()
        .map(|name| (*name).to_owned())
        .collect();
    assert_eq!(
        dynamic.get(MOTION_TRAIT).cloned().unwrap_or_default(),
        expected,
        "src/dynapi/nouns.rs: DynMotion handwritten methods changed",
    );
    for (trait_name, names) in dynamic {
        if trait_name != MOTION_TRAIT {
            assert!(
                names.is_empty(),
                "src/dynapi/nouns.rs: generated noun trait {trait_name} contains handwritten methods {names:?}",
            );
        }
    }
}

#[test]
fn generated_facades_consume_every_registry_noun() {
    assert_eq!(NOUN_KEYS.len(), NOUN_FACADES.len());
    let expected: BTreeSet<String> = NOUN_KEYS.iter().map(|key| (*key).to_owned()).collect();
    for (label, source, consumer) in [
        ("src/async_nouns.rs", ASYNC_SOURCE, "async_noun_methods"),
        (
            "src/blocking_nouns.rs",
            BLOCKING_SOURCE,
            "blocking_noun_methods",
        ),
        ("src/dynapi/nouns.rs", DYN_SOURCE, "dyn_noun_declarations"),
    ] {
        assert_eq!(
            consumed_nouns(source, label, consumer),
            expected,
            "{label}: {consumer} must consume every registry noun arm",
        );
    }

    // The dynamic implementation projection is a second invocation site,
    // but it must consume the same complete set as the declarations.
    let declarations = consumed_nouns(DYN_SOURCE, "src/dynapi/nouns.rs", "dyn_noun_declarations");
    let implementations = consumed_nouns(DYN_SOURCE, "src/dynapi/nouns.rs", "dyn_noun_impls");
    assert_eq!(declarations, implementations);
    assert_eq!(declarations.len(), NOUN_KEYS.len());
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
    assert_eq!(BuiltinCommand::ALL.len(), BUILTIN_COMMAND_COUNT); // 150 commands
    assert_eq!(target, TARGET_FACING_COMMAND_COUNT); // 147 target-facing
    assert_eq!(exceptions, NON_NOUN_COMMAND_COUNT); // 3 protocol exceptions
    assert_eq!(nouns.len(), 14); // 14 nouns
    assert_eq!(BUILTIN_INQUIRY_ACCESSORS.len(), 62); // 62 typed inquiries

    // The compiled noun-table projection has one row for each target-facing
    // command ID, one for each typed inquiry, and one for each empty-ID
    // convenience helper. Keep all four totals visible so a helper cannot be
    // mistaken for a command row or silently disappear from a facade.
    let table = table_surface();
    let (command_rows, inquiry_rows, helper_rows) = table_row_counts(&table);
    assert_eq!(command_rows, 147); // 147 command-method rows
    assert_eq!(inquiry_rows, 62); // 62 inquiry rows
    assert_eq!(helper_rows, 9); // 9 empty-ID helper rows
    assert_eq!(command_rows + inquiry_rows + helper_rows, 218); // 218 total rows

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

#[test]
fn only_motion_methods_remain_handwritten_and_capabilities_are_real() {
    assert_motion_and_no_residuals();
}

/// Issue #684: the erased inquiry surface gates each built-in inquiry on exactly
/// the marker the static noun surface resolves, so the two gate sets cannot
/// drift. The static gate is the inquiry's own `where` marker, or its noun's
/// base-domain marker when the row carries none; the erased gate is the runtime
/// accessor gate recorded in [`BUILTIN_INQUIRY_ACCESSORS`].
#[test]
fn erased_inquiry_gates_match_the_static_noun_surface() {
    // Two inquiries carry a static base marker that the shared accessor gate
    // deliberately leaves `Always`, for reasons unrelated to drift:
    //   * `PanTiltPositionInquiry` enforces `has_pan_tilt` inside its own
    //     coordinate-decoder `validate_for_profile`, so its behavioral gate
    //     still matches `HasPanTilt` — it is just not expressed as the shared
    //     accessor gate.
    //   * `MenuOpenCloseInquiry` has no runtime menu capability to gate on: a
    //     runtime `ProfileSpec` cannot express "no menu", so basic OSD menu
    //     stays universally reachable, exactly like the menu commands.
    const ACCESSOR_UNGATED_EXCEPTIONS: &[&str] =
        &["PanTiltPositionInquiry", "MenuOpenCloseInquiry"];

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
    fn consumed_nouns_reads_only_macro_invocation_identifiers() {
        let source = "noun_table!(Power => consumer);\nnoun_table!(Zoom => other);\n";
        assert_eq!(
            consumed_nouns(source, "fixture", "consumer"),
            BTreeSet::from(["Power".to_owned()]),
        );
    }

    #[test]
    #[should_panic(expected = "capability aliases are not allowed")]
    fn capability_aliases_are_not_accepted() {
        let _ =
            imported_capability_names("use crate::capabilities::{HasZoom as Shadow};\n", "fixture");
    }
}
