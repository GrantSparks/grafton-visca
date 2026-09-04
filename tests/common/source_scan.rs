//! Source-text scanners shared by the closed-inventory integration gates.
//!
//! These gates read crate sources as text, which is only sound if the reading
//! is anchored and delimiter-aware.  Two failure modes motivated this module:
//!
//! * *Self-satisfying gates.*  A surface file names its own accessors and
//!   method spellings as string literals inside its in-file inventory tests, so
//!   `source.contains("PowerAccessor")` can be satisfied by the test data after
//!   the accessor itself is gone.  [`declarations`] blanks every
//!   `#[cfg(test)]` item so a positive gate reads the declaration region only.
//! * *Ambiguous needles.*  `src/command/semantics.rs` declares two
//!   `pub const ALL: &[Self]` slices; picking one with `rsplit_once` silently
//!   depends on which is written last.  [`builtin_command_rows`] anchors on the
//!   `impl BuiltinCommand` block and reads the slice by bracket depth, so it
//!   tolerates re-indentation and fails loudly rather than counting the wrong
//!   list.
//!
//! This is the integration-test twin of the scanner in `src/noun_parity.rs`;
//! the crate's own gate cannot be reached from an integration test binary.

#![allow(dead_code)]

/// Blanks comments and literal contents while preserving the line structure.
///
/// Every scan below runs over the result, so a brace inside a string such as
/// `format!("pub fn {method}(")` can never be mistaken for a real delimiter.
fn clean(source: &str) -> String {
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
            }
            ('"', _) => {
                out.push('"');
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
                    out.push(if ch == '"' { '"' } else { blank(ch) });
                    index += 1;
                    if ch == '"' {
                        break;
                    }
                }
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

/// Returns the index just past the line closing the brace block at `start`.
fn block_end(lines: &[&str], start: usize) -> usize {
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
    panic!("unterminated block starting at line {}", start + 1)
}

/// Returns `source` with every `#[cfg(test)]` item blanked out.
///
/// Line numbering is preserved so a failure still points at the right line of
/// the real file.
pub fn declarations(source: &str) -> String {
    let cleaned = clean(source);
    let cleaned_lines: Vec<&str> = cleaned.lines().collect();
    let mut skip = vec![false; cleaned_lines.len()];
    let mut index = 0;
    while index < cleaned_lines.len() {
        if cleaned_lines[index].trim() == "#[cfg(test)]" {
            let end = block_end(&cleaned_lines, index);
            for entry in skip.iter_mut().take(end).skip(index) {
                *entry = true;
            }
            index = end;
            continue;
        }
        index += 1;
    }

    source
        .lines()
        .zip(skip)
        .map(|(line, skip)| if skip { "" } else { line })
        .collect::<Vec<_>>()
        .join("\n")
}

/// Returns the body of the single item whose trimmed header line is `header`.
fn item_body(source: &str, header: &str) -> String {
    let cleaned = clean(source);
    let lines: Vec<&str> = cleaned.lines().collect();
    let starts: Vec<usize> = lines
        .iter()
        .enumerate()
        .filter(|(_, line)| line.trim() == header)
        .map(|(index, _)| index)
        .collect();
    assert_eq!(
        starts.len(),
        1,
        "expected exactly one {header:?} header, found {}",
        starts.len(),
    );
    let start = starts[0];
    let end = block_end(&lines, start);
    lines[start..end].join("\n")
}

/// Returns the `[...]` body that follows `needle`, read by bracket depth.
fn bracket_body(text: &str, needle: &str) -> String {
    let matches = text.matches(needle).count();
    assert_eq!(
        matches, 1,
        "expected exactly one {needle:?} anchor, found {matches}",
    );
    let start = text.find(needle).unwrap_or_else(|| unreachable!());
    let rest = &text[start + needle.len()..];
    let mut depth = 1_i32;
    for (index, ch) in rest.char_indices() {
        match ch {
            '[' => depth += 1,
            ']' => {
                depth -= 1;
                if depth == 0 {
                    return rest[..index].to_owned();
                }
            }
            _ => {}
        }
    }
    panic!("unterminated slice after {needle:?}")
}

/// Rows listed in the closed `BuiltinCommand::ALL` slice.
///
/// Anchored on the `impl BuiltinCommand` block: `BuiltinCommandDomain` declares
/// an `ALL` slice of its own, so an unanchored needle counts whichever list
/// happens to be written last.
pub fn builtin_command_rows(semantics: &str) -> usize {
    let body = item_body(semantics, "impl BuiltinCommand {");
    bracket_body(&body, "pub const ALL: &[Self] = &[")
        .matches("Self::")
        .count()
}

/// Entries in the generated camera-facing inquiry-accessor table.
///
/// This is the table behind `BUILTIN_INQUIRY_ACCESSORS`, and therefore an
/// independent source for the dynamic projection's inquiry-method count.  The
/// `accessors { .. }` group of the macro *definition* is skipped by requiring
/// the block to be free of macro metavariables.
pub fn inquiry_accessor_rows(inquiry_structs: &str) -> usize {
    let cleaned = clean(inquiry_structs);
    let lines: Vec<&str> = cleaned.lines().collect();
    let mut bodies = Vec::new();
    for (index, line) in lines.iter().enumerate() {
        if line.trim() != "accessors {" {
            continue;
        }
        let end = block_end(&lines, index);
        let body = lines[index..end].join("\n");
        if !body.contains('$') {
            bodies.push(body);
        }
    }
    assert_eq!(
        bodies.len(),
        1,
        "expected exactly one generated `accessors` table, found {}",
        bodies.len(),
    );
    bodies[0]
        .lines()
        .filter(|line| line.contains("=>") && line.trim_end().ends_with(';'))
        .count()
}
