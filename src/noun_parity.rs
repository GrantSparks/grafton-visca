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
//! All three facades are now generated whole from that registry, so no
//! facade file is compared against it here. This module keeps the compiled
//! registry's readable totals and the static/erased gate cross-checks, plus
//! the declaration scanner that `crate::facade_parity` reads the hand-written
//! session and camera facades with.
//!
//! The source reader below is intentionally not a Rust parser. It strips
//! comments/literals and masks test and macro bodies.

#![allow(clippy::panic)]

use std::collections::{BTreeMap, BTreeSet};

use crate::{
    command::{
        inquiry_structs::{BuiltinInquiryProfileGate, BUILTIN_INQUIRY_ACCESSORS},
        semantics::BuiltinCommand,
        surface::{
            surface_entry, StaticSurfaceDisposition, BUILTIN_COMMAND_COUNT, NON_NOUN_COMMAND_COUNT,
            TARGET_FACING_COMMAND_COUNT,
        },
    },
    noun_table::noun_table,
};

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

/// Materialize the registry's rows per noun, keyed by the `Dyn*` trait name
/// its header declares.
///
/// Request expressions and argument types are deliberately captured as token
/// trees and discarded. This is a macro projection of the compiled registry,
/// not a second parser or a second source of truth.
#[must_use]
pub(crate) fn table_surface() -> BTreeMap<String, BTreeMap<String, TableRow>> {
    let mut table: BTreeMap<String, BTreeMap<String, TableRow>> = BTreeMap::new();

    let mut insert = |dyn_trait: &str, method: &str, row: TableRow| {
        let rows = table.entry(dyn_trait.to_owned()).or_default();
        assert!(
            rows.insert(method.to_owned(), row).is_none(),
            "duplicate noun-table row {dyn_trait}::{method}",
        );
    };

    macro_rules! collect {
        (@row $dyn_trait:ident, $method:ident, inquiry []) => {
            insert(stringify!($dyn_trait), stringify!($method), TableRow::Inquiry);
        };
        (@row $dyn_trait:ident, $method:ident, $kind:ident []) => {
            insert(stringify!($dyn_trait), stringify!($method), TableRow::Helper);
        };
        (@row $dyn_trait:ident, $method:ident, $kind:ident [$command:ident]) => {
            insert(stringify!($dyn_trait), stringify!($method), TableRow::Command);
        };

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
            $( $( collect!(@row $dyn_trait, $method, $kind [$($command)?]); )* )*
        };
    }

    noun_table!(collect);
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

    noun_table!(collect);
    map
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

#[test]
fn compiled_registry_inventory_counts_remain_readable() {
    let mut target = 0;
    let mut exceptions = 0;
    let mut nouns = std::collections::HashSet::new();
    for command in BuiltinCommand::ALL {
        let entry = surface_entry(*command);
        assert_eq!(entry.class, command.classification());
        match entry.disposition {
            StaticSurfaceDisposition::Noun { noun, .. } => {
                target += 1;
                nouns.insert(noun);
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

        let registry_helpers: BTreeSet<(String, String)> = table
            .iter()
            .flat_map(|(dyn_trait, rows)| {
                rows.iter()
                    .filter(|(_, row)| matches!(row, TableRow::Helper))
                    .map(move |(method, _)| (dyn_trait.clone(), method.clone()))
            })
            .collect();
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
}
