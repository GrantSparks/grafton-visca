//! Source-text scanner for the closed-inventory integration gates.
//!
//! These gates read crate sources as text, which is only sound if the reading
//! is delimiter-aware.  A surface file can name its own items as string
//! literals inside its in-file tests, so `source.contains("surface_entry")`
//! could be satisfied by test data after the item itself is gone.
//! [`declarations`] blanks every `#[cfg(test)]` item so a positive gate reads
//! the declaration region only.
//!
//! This is the integration-test twin of the scanner in `src/facade_parity.rs`;
//! the crate's own scanner cannot be reached from an integration test binary.

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
