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
            ('r', Some('"' | '#')) => {
                // Raw strings may contain quotes, escapes and fake Rust items.
                let mut quote = index + 1;
                while chars.get(quote) == Some(&'#') {
                    quote += 1;
                }
                if chars.get(quote) != Some(&'"') {
                    out.push(current);
                    index += 1;
                    continue;
                }
                let hashes = quote - index - 1;
                out.extend(chars[index..=quote].iter().map(|&ch| blank(ch)));
                index = quote + 1;
                while index < chars.len() {
                    let closes = chars[index] == '"'
                        && (0..hashes).all(|offset| chars.get(index + 1 + offset) == Some(&'#'));
                    let end = if closes {
                        index + hashes + 1
                    } else {
                        index + 1
                    };
                    out.extend(chars[index..end].iter().map(|&ch| blank(ch)));
                    index = end;
                    if closes {
                        break;
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
                ';' if !opened => return offset + 1,
                _ => {}
            }
        }
        if opened && depth <= 0 {
            return offset + 1;
        }
    }
    panic!("unterminated block starting at line {}", start + 1)
}

/// Returns `source` with every test-only item blanked out.
///
/// Line numbering is preserved so a failure still points at the right line of
/// the real file.
pub fn declarations(source: &str) -> String {
    strip_test_items(source, &clean(source))
}

/// Production tokens: comments, literal contents and test-only items are blank.
///
/// Unlike [`declarations`], this is suitable for constructor expression checks:
/// a comment or string containing Rust syntax cannot satisfy a positive gate.
/// Line numbers are preserved for diagnostics.
pub fn production_code(source: &str) -> String {
    clean(&declarations(source))
}

fn strip_test_items(source: &str, cleaned: &str) -> String {
    let cleaned_lines: Vec<&str> = cleaned.lines().collect();
    let mut skip = vec![false; cleaned_lines.len()];
    let mut index = 0;
    while index < cleaned_lines.len() {
        if test_only_attribute(cleaned_lines[index].trim()) {
            let end = block_end(&cleaned_lines, index);
            for entry in skip.iter_mut().take(end).skip(index) {
                *entry = true;
            }
            index = end;
            continue;
        }
        index += 1;
    }

    let mut result = source
        .lines()
        .zip(skip)
        .map(|(line, skip)| if skip { "" } else { line })
        .collect::<Vec<_>>()
        .join("\n");
    if source.ends_with('\n') {
        result.push('\n');
    }
    result
}

// Evaluate only the `test` fact. All other cfg predicates are unknown; an item
// is omitted only when it definitely cannot exist outside a test build.
fn test_only_attribute(line: &str) -> bool {
    fn evaluate(input: &str) -> Option<bool> {
        let input = input.trim();
        if input == "test" {
            return Some(false);
        }
        for (name, conjunction) in [("all(", true), ("any(", false)] {
            if let Some(arguments) = input.strip_prefix(name).and_then(|s| s.strip_suffix(')')) {
                let mut depth = 0;
                let mut start = 0;
                let mut values = Vec::new();
                for (index, ch) in arguments.char_indices() {
                    match ch {
                        '(' => depth += 1,
                        ')' => depth -= 1,
                        ',' if depth == 0 => {
                            values.push(evaluate(&arguments[start..index]));
                            start = index + 1;
                        }
                        _ => {}
                    }
                }
                if !arguments[start..].trim().is_empty() {
                    values.push(evaluate(&arguments[start..]));
                }
                if values.contains(&Some(!conjunction)) {
                    return Some(!conjunction);
                }
                return values.iter().all(Option::is_some).then_some(conjunction);
            }
        }
        input
            .strip_prefix("not(")
            .and_then(|s| s.strip_suffix(')'))
            .and_then(evaluate)
            .map(|value| !value)
    }
    line.strip_prefix("#[cfg(")
        .and_then(|s| s.strip_suffix(")]"))
        .is_some_and(|predicate| evaluate(predicate) == Some(false))
}

#[cfg(test)]
mod tests {
    use super::{declarations, production_code};

    #[test]
    fn production_tokens_ignore_literals_comments_and_test_only_items() {
        let source = r####"pub fn live() { Error::Live { reason: reason.into() } }
// Error::Fake { reason: reason.into() }
/* nested /* comment */ Error::Fake */
const TEXT: &str = "Error::Fake { reason: reason.into() }";
const RAW: &str = r###"a " quote; Error::Fake { reason: reason.into() }"###;
#[cfg(all(feature = "blocking", test))]
mod tests { fn fake() { Error::Fake { reason: reason.into() } } }
#[cfg(any(test, feature = "blocking"))]
pub fn feature_live() { Error::FeatureLive }
#[cfg(test)]
mod external_tests;
"####;
        let production = production_code(source);
        assert!(production.contains("Error::Live"));
        assert!(production.contains("Error::FeatureLive"));
        assert!(!production.contains("Error::Fake"));
        assert_eq!(production.lines().count(), source.lines().count());
        let declarations = declarations(source);
        assert!(declarations.contains("// Error::Fake"));
        assert!(!declarations.contains("mod tests"));
    }
}
