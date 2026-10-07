//! Method-set parity between the blocking and async facades (#805).
//!
//! The two facades are written per execution mode because their `open`,
//! `close` and waiting styles legitimately differ, but they must expose the
//! same public surface: every public method on `Session`, `CameraSession`,
//! `Camera` and `Operation` exists on both, with the same generics,
//! parameters and return type, and both expand the same shared method
//! macros. Only the `async` keyword may differ. The noun accessors and their
//! `Camera` getters are not compared here: one consumer
//! (`crate::noun_facade::static_noun_facade!`) generates both facades' copies.
//!
//! The reader below is a declaration scanner, intentionally not a Rust parser:
//! comments, literals, test modules and macro bodies are blanked before
//! anything is matched.

#![allow(clippy::panic)]

use std::collections::BTreeMap;

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

const BLOCKING: &[(&str, &str)] = &[("src/blocking.rs", include_str!("blocking.rs"))];
const ASYNC: &[(&str, &str)] = &[
    ("src/async_session.rs", include_str!("async_session.rs")),
    ("src/operation.rs", include_str!("operation.rs")),
];

/// The public facade types both modes must expose identically.
const FACADE_TYPES: &[&str] = &["Session", "CameraSession", "Camera", "Operation"];

/// One public method as both facades must agree on it: its receiver and its
/// parameter names. Shared macros are recorded by name with an empty
/// signature.
type Surface = BTreeMap<String, String>;

/// The header of an `impl` item that starts at `start`, up to its `{`.
fn impl_header(lines: &[String], start: usize) -> String {
    let mut header = String::new();
    for line in &lines[start..] {
        match line.find('{') {
            Some(brace) => {
                header.push_str(&line[..brace]);
                return header;
            }
            None => {
                header.push_str(line);
                header.push(' ');
            }
        }
    }
    panic!("unterminated impl header at line {}", start + 1);
}

/// The self type an inherent `impl` header names, or `None` for a trait impl.
fn inherent_self_type(header: &str) -> Option<String> {
    let header = header.trim().strip_prefix("impl")?;
    if header.contains(" for ") {
        return None;
    }
    // Skip the impl's own generic parameter list.
    let header = header.trim_start();
    let header = if header.starts_with('<') {
        let mut depth = 0_i32;
        let mut end = 0;
        for (index, character) in header.char_indices() {
            match character {
                '<' => depth += 1,
                '>' => {
                    depth -= 1;
                    if depth == 0 {
                        end = index + 1;
                        break;
                    }
                }
                _ => {}
            }
        }
        &header[end..]
    } else {
        header
    };
    let name: String = header
        .trim_start()
        .chars()
        .take_while(|character| character.is_alphanumeric() || *character == '_')
        .collect();
    (!name.is_empty()).then_some(name)
}

/// The facade-neutral shape of one method signature: its generics, every
/// parameter (name and type) and its return type, up to its `where` clause
/// or body. Only what legitimately differs between the facades is
/// normalized away: whitespace, the `async` keyword (whose futures resolve to
/// the same value), `crate::`/facade module paths, the `Result` alias
/// spelling, and each facade's own name for its runtime-profile camera.
fn signature_shape(signature: &str) -> String {
    let start = signature
        .find("fn ")
        .unwrap_or_else(|| panic!("not a function signature: {signature}"));
    let mut shape = String::new();
    let mut depth = 0_i32;
    let rest = &signature[start + 3..];
    for (index, character) in rest.char_indices() {
        match character {
            '(' | '<' | '[' => depth += 1,
            ')' | ']' => depth -= 1,
            '>' if !rest[..index].ends_with('-') => depth -= 1,
            '{' if depth == 0 => break,
            'w' if depth == 0 && rest[index..].starts_with("where") => break,
            _ => {}
        }
        shape.push(character);
    }
    let mut shape: String = shape.split_whitespace().collect::<Vec<_>>().join(" ");
    for path in ["crate::", "blocking::", "async_session::", "operation::"] {
        shape = shape.replace(path, "");
    }
    // Each facade names its own runtime-profile camera.
    shape = shape.replace("BlockingDynSessionCamera", "DynSessionCamera");
    shape
        .replace("( ", "(")
        .replace(", )", ")")
        .replace(", Error>", ">")
}

/// The public surface of `ty` across a facade's source files.
fn surface(sources: &[(&str, &str)], ty: &str) -> Surface {
    let mut surface = Surface::new();
    for (label, source) in sources {
        let lines = declaration_lines(source, label, true);
        let line_refs: Vec<&str> = lines.iter().map(String::as_str).collect();
        let mut index = 0;
        while index < lines.len() {
            let trimmed = lines[index].trim_start();
            if !trimmed.starts_with("impl") {
                index += 1;
                continue;
            }
            let end = block_end(&line_refs, index, label);
            if inherent_self_type(&impl_header(&lines, index)).as_deref() == Some(ty) {
                let open = (index..end)
                    .find(|line| lines[*line].contains('{'))
                    .unwrap_or_else(|| panic!("{label}:{}: impl without a body", index + 1));
                collect_items(&lines[open + 1..end - 1], label, &mut surface);
            }
            index = end;
        }
    }
    surface
}

/// Records the public methods and shared macro expansions directly inside
/// one `impl` body (nested items are skipped).
fn collect_items(body: &[String], label: &str, surface: &mut Surface) {
    let mut depth = 0_i32;
    let mut index = 0;
    while index < body.len() {
        let line = &body[index];
        let trimmed = line.trim_start();
        if depth == 0 {
            let method = ["pub fn ", "pub async fn ", "pub const fn "]
                .iter()
                .find_map(|prefix| trimmed.strip_prefix(prefix));
            if let Some(rest) = method {
                let name: String = rest
                    .chars()
                    .take_while(|character| character.is_alphanumeric() || *character == '_')
                    .collect();
                let mut signature = String::new();
                for line in &body[index..] {
                    signature.push_str(line);
                    signature.push(' ');
                    if line.contains('{') || line.trim_start().starts_with("where") {
                        break;
                    }
                }
                // Opening legitimately differs: the async facade also takes
                // the executor its owner actor runs on.
                let parameters = if name == "open" {
                    String::from("(facade-specific)")
                } else {
                    signature_shape(&signature)
                };
                let previous = surface.insert(name.clone(), parameters);
                assert!(previous.is_none(), "{label}: `{name}` declared twice");
            } else if let Some(bang) = trimmed.find("!(") {
                let path = &trimmed[..bang];
                if !path.contains(' ') && !path.is_empty() {
                    let name = path.rsplit("::").next().unwrap_or(path);
                    surface.insert(format!("{name}!"), String::new());
                }
            }
        }
        for character in line.chars() {
            match character {
                '{' => depth += 1,
                '}' => depth -= 1,
                _ => {}
            }
        }
        index += 1;
    }
}

#[test]
fn both_facades_expose_the_same_method_set() {
    for ty in FACADE_TYPES {
        let blocking = surface(BLOCKING, ty);
        let asynchronous = surface(ASYNC, ty);
        assert!(
            !blocking.is_empty(),
            "the scanner found no blocking `{ty}` methods"
        );
        assert!(
            !asynchronous.is_empty(),
            "the scanner found no async `{ty}` methods"
        );
        assert_eq!(
            blocking, asynchronous,
            "`{ty}` differs between the blocking and async facades"
        );
    }
}

#[test]
fn the_scanner_reads_signatures_and_skips_trait_impls() {
    let source = "\
impl<K> Operation<K>
where
    K: Kind,
{
    /// `pub fn ignored_in_docs(&self)`.
    pub fn id(&self) -> OperationId { inner() }
    pub async fn wait(
        &mut self,
        timeout: Duration,
    ) -> Result<()> {
        {}
    }
    fn private(&self) {}
    crate::shared::shared_methods!();
}

impl<K> fmt::Debug for Operation<K> {
    pub fn not_inherent(&self) {}
}
";
    let surface = surface(&[("fixture", source)], "Operation");
    assert_eq!(
        surface,
        Surface::from([
            ("id".to_owned(), "id(&self) -> OperationId".to_owned()),
            ("shared_methods!".to_owned(), String::new()),
            (
                "wait".to_owned(),
                "wait(&mut self, timeout: Duration) -> Result<()>".to_owned()
            ),
        ])
    );
}

/// Self-tests of the declaration scanner.
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
            "    struct GhostAccessor;\n",
            "}\n",
            "pub struct LaterAccessor;\n",
        );
        let declarations = declaration_lines(source, "fixture", false);
        assert_eq!(declarations.len(), source.lines().count());
        let declarations = declarations.join("\n");
        assert!(declarations.contains("RealAccessor"));
        assert!(declarations.contains("LaterAccessor"));
        assert!(!declarations.contains("GhostAccessor"));
    }
}
