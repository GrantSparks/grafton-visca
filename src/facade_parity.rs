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
//! The native camera cores share one macro definition. Its two invocations are
//! expanded by the scanner so their private and crate-visible methods, complete
//! bounds and forwarding bodies are compared too. Mutation fixtures ensure an
//! extra helper or a different forwarded argument is observable.
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
        let test_only = trimmed == "#[cfg(test)]" || trimmed.starts_with("#[cfg(all(test,");
        if test_only || (macros && trimmed.starts_with("macro_rules!")) {
            // An out-of-line test module ends with a semicolon, without a
            // brace-delimited body in this file.
            let first_item = lines
                .iter()
                .enumerate()
                .skip(index)
                .find(|(_, line)| line.contains('{') || line.trim_end().ends_with(';'));
            let end = match first_item {
                Some((end, line)) if !line.contains('{') => end + 1,
                _ => block_end(lines, index, label),
            };
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
    scanned_surface(sources, ty, false)
}

/// Includes private methods and complete bodies for the shared camera cores.
fn core_contract(sources: &[(&str, &str)], ty: &str) -> Surface {
    scanned_surface(sources, ty, true)
}

fn scanned_surface(sources: &[(&str, &str)], ty: &str, core: bool) -> Surface {
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
                collect_items(&lines[open + 1..end - 1], label, core, &mut surface);
            }
            index = end;
        }
    }
    surface
}

/// Records the public methods and shared macro expansions directly inside
/// one `impl` body (nested items are skipped).
fn collect_items(body: &[String], label: &str, core: bool, surface: &mut Surface) {
    let mut depth = 0_i32;
    let mut index = 0;
    while index < body.len() {
        let line = &body[index];
        let trimmed = line.trim_start();
        if depth == 0 {
            if let Some(rest) = method_declaration(trimmed, core) {
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
                let parameters = if core {
                    let lines: Vec<&str> = body.iter().map(String::as_str).collect();
                    let end = block_end(&lines, index, label);
                    let declaration = body[index..end].join(" ");
                    let start = declaration
                        .find("fn ")
                        .unwrap_or_else(|| panic!("{label}: missing fn"));
                    compact_contract(&declaration[start..])
                } else if name == "open" {
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

/// Reads public signatures, or every inherent method when inspecting a core.
fn method_declaration(line: &str, core: bool) -> Option<&str> {
    let declaration = if let Some(rest) = line.strip_prefix("pub ") {
        rest
    } else if core {
        match line.strip_prefix("pub(") {
            Some(rest) => rest.split_once(')')?.1.trim_start(),
            None => line,
        }
    } else {
        return None;
    };
    let declaration = declaration.trim_start();
    let declaration = declaration.strip_prefix("async ").unwrap_or(declaration);
    let declaration = declaration.strip_prefix("const ").unwrap_or(declaration);
    declaration.strip_prefix("fn ")
}

fn compact_contract(source: &str) -> String {
    source
        .split_whitespace()
        .collect::<String>()
        .replace("crate::", "")
        .replace(".await", "")
}

/// Returns the byte following a balanced block in cleaned source.
fn closing_brace(source: &str, open: usize) -> usize {
    let mut depth = 0_usize;
    for (offset, character) in source[open..].char_indices() {
        match character {
            '{' => depth += 1,
            '}' => {
                depth -= 1;
                if depth == 0 {
                    return open + offset + 1;
                }
            }
            _ => {}
        }
    }
    panic!("unterminated camera core block");
}

/// Expands the one shared core definition with the facade's actual arguments.
/// Exact native-mode arguments are checked before comparing expanded methods;
/// a macro marker on its own would miss both visibility and body drift.
fn expand_camera_core(source: &str, core: &str, owner: &str, asynchronous: bool) -> String {
    let source = clean_source(source, core);
    assert_eq!(
        source.matches("camera_core!").count(),
        1,
        "{core}: exactly one shared definition"
    );
    let call = source
        .find("camera_core!")
        .unwrap_or_else(|| panic!("{core}: no macro invocation"));
    let open = call
        + source[call..]
            .find('{')
            .unwrap_or_else(|| panic!("{core}: no invocation body"));
    let end = closing_brace(&source, open);
    let asynchronous_token = if asynchronous { "async" } else { "" };
    let await_token = if asynchronous { ".await" } else { "" };
    let actual = source[open + 1..end - 1]
        .split_whitespace()
        .collect::<String>();
    assert_eq!(
        actual.trim_end_matches(','),
        format!("core:{core},owner:{owner},async:[{asynchronous_token}],await:[{await_token}]"),
        "{core}: native mode arguments"
    );
    let definition = clean_source(include_str!("camera_core.rs"), "camera_core.rs");
    let marker = ") => {";
    let open = definition
        .find(marker)
        .unwrap_or_else(|| panic!("camera core macro has no template"))
        + marker.len()
        - 1;
    let template_end = closing_brace(&definition, open);
    let template = definition[open + 1..template_end - 1]
        .replace("$core", core)
        .replace("$owner", owner)
        .replace("$($async)*", asynchronous_token)
        .replace("$($await)*", await_token);
    let start = source[..call].rfind('\n').map_or(0, |index| index + 1);
    format!("{}{}{}", &source[..start], template, &source[end..])
}

#[test]
fn both_native_camera_cores_have_the_same_methods_and_forwarding_contract() {
    let blocking = expand_camera_core(
        include_str!("blocking.rs"),
        "BlockingCameraCore",
        "BlockingOwnerHandle",
        false,
    );
    let asynchronous = expand_camera_core(
        include_str!("async_session.rs"),
        "AsyncCameraCore",
        "AsyncOwnerHandle",
        true,
    );
    let blocking = core_contract(&[("blocking core", &blocking)], "BlockingCameraCore");
    let asynchronous = core_contract(&[("async core", &asynchronous)], "AsyncCameraCore");
    assert_eq!(
        blocking, asynchronous,
        "native camera core contracts differ"
    );
    assert_eq!(
        blocking.keys().map(String::as_str).collect::<Vec<_>>(),
        [
            "execute",
            "inquire",
            "into_typed",
            "is_moving",
            "profile",
            "registration",
            "set_submission_class",
            "state_cache",
            "stop_all_motion",
            "submission_class",
            "submit",
            "target",
            "tuning",
            "wait_until_idle",
        ],
        "the scanner must inspect every real core method"
    );
}

#[test]
fn core_contract_detects_private_helpers_and_different_forwarding() {
    let original = "impl Core {\n pub(crate) fn execute<C>(&self, request: &C) where C: PlainCommand + ?Sized { prepare(self.class); }\n fn tuning(&self) { self.owner.validated_tuning() }\n}\n";
    let changed_argument =
        original.replace("prepare(self.class)", "prepare(ClassSelection::Request)");
    let changed_bound = original.replace("PlainCommand + ?Sized", "PlainCommand + Send");
    let extra_helper = format!("{original}impl Core {{\n fn extra(&self) {{}}\n}}\n");
    let reference = core_contract(&[("original", original)], "Core");
    assert_eq!(reference.len(), 2);
    assert!(reference.contains_key("tuning"));
    assert_ne!(
        reference,
        core_contract(&[("changed argument", &changed_argument)], "Core")
    );
    assert_ne!(
        reference,
        core_contract(&[("extra helper", &extra_helper)], "Core")
    );
    assert_ne!(
        reference,
        core_contract(&[("changed bound", &changed_bound)], "Core")
    );
}

#[test]
#[should_panic(expected = "native mode arguments")]
fn core_contract_rejects_a_non_native_macro_invocation() {
    expand_camera_core(
        "camera_core! { core: Core, owner: Owner, async: [], await: [.await] }",
        "Core",
        "Owner",
        false,
    );
}

/// Bind the recording-driver tests to the real native entry points: neither
/// shell may bypass the shared driver with a handwritten method.
fn assert_native_driver_binding(source: &str, ty: &str, expected_arguments: &str) {
    let cleaned = clean_source(source, ty);
    assert_eq!(
        cleaned.matches("owner_turn_driver!").count(),
        1,
        "{ty}: exactly one shared driver invocation"
    );
    let call = cleaned
        .find("owner_turn_driver!")
        .unwrap_or_else(|| panic!("{ty}: missing driver invocation"));
    let open = call
        + cleaned[call..]
            .find('{')
            .unwrap_or_else(|| panic!("{ty}: no driver arguments"));
    let end = closing_brace(&cleaned, open);
    let arguments = cleaned[open + 1..end - 1]
        .split_whitespace()
        .collect::<String>();
    assert_eq!(
        arguments.trim_end_matches(','),
        expected_arguments,
        "{ty}: native driver arguments"
    );
    let methods = core_contract(&[(ty, source)], ty);
    for method in ["run", "handle_event", "drive"] {
        assert!(
            !methods.contains_key(method),
            "{ty}::{method} must come from the shared driver"
        );
    }
}

#[test]
fn both_native_owners_bind_the_shared_driver_without_handwritten_turn_methods() {
    assert_native_driver_binding(
        include_str!("runtime/owner/blocking.rs"),
        "BlockingOwnerWorker",
        "async:[],await:[],driver:BlockingOwnerDriver",
    );
    assert_native_driver_binding(
        include_str!("runtime/owner/async_actor.rs"),
        "AsyncOwnerActor",
        "async:[async],await:[.await],driver:AsyncOwnerDriver",
    );
}

#[test]
fn driver_binding_gate_detects_a_handwritten_override() {
    let source = "impl Owner {\n owner_turn_driver! { async: [], await: [], driver: Driver }\n}\n";
    let arguments = "async:[],await:[],driver:Driver";
    assert_native_driver_binding(source, "Owner", arguments);
    for method in ["run", "handle_event", "drive"] {
        let changed = source.replace("\n}\n", &format!("\n fn {method}(&self) {{}}\n}}\n"));
        assert!(
            std::panic::catch_unwind(|| assert_native_driver_binding(&changed, "Owner", arguments))
                .is_err(),
            "the gate must reject handwritten {method}"
        );
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
    pub(crate) fn restricted(&self) {}
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
    fn macro_mask_keeps_its_outer_body_even_with_semicolon_only_rules() {
        let source = "macro_rules! hidden {\n ($value:expr) => ($value);\n}\nimpl Real { pub fn visible(&self) {} }\n";
        let declarations = declaration_lines(source, "macro fixture", true);
        assert!(declarations[..3].iter().all(|line| line.trim().is_empty()));
        assert!(declarations[3].starts_with("impl Real"));
    }

    #[test]
    fn declaration_scan_drops_in_file_test_data() {
        let source = concat!(
            "pub struct RealAccessor;\n",
            "#[cfg(test)]\n",
            "mod tests {\n",
            "    struct GhostAccessor;\n",
            "}\n",
            "#[cfg(all(test, feature = \"test-utils\"))]\n",
            "mod mode_tests { struct ModeGhost; }\n",
            "#[cfg(test)]\n",
            "mod external_tests;\n",
            "pub struct LaterAccessor;\n",
        );
        let declarations = declaration_lines(source, "fixture", false);
        assert_eq!(declarations.len(), source.lines().count());
        let declarations = declarations.join("\n");
        assert!(declarations.contains("RealAccessor"));
        assert!(declarations.contains("LaterAccessor"));
        assert!(!declarations.contains("GhostAccessor"));
        assert!(!declarations.contains("ModeGhost"));
        assert!(!declarations.contains("external_tests"));
    }
}
