//! Release-gate test citations must resolve to real, uniquely named functions.

use std::{
    collections::{BTreeMap, BTreeSet},
    path::{Path, PathBuf},
};

use grafton_visca_test_support::source_scan;

type Functions = BTreeMap<String, Vec<String>>;

#[derive(Default)]
struct SourceIndex {
    functions: Functions,
    registered: BTreeSet<(String, String)>,
    files: BTreeMap<String, usize>,
}

impl SourceIndex {
    fn add(&mut self, path: &str, source: &str) {
        let tests = source_scan::test_function_names(source);
        self.files.insert(path.into(), tests.len());
        for name in source_scan::function_names(source) {
            self.functions.entry(name).or_default().push(path.into());
        }
        self.registered
            .extend(tests.into_iter().map(|name| (name, path.into())));
    }
}

fn source_functions() -> SourceIndex {
    fn scan(root: &Path, path: &Path, functions: &mut SourceIndex) {
        let mut entries = std::fs::read_dir(path)
            .expect("source directory exists")
            .map(|entry| entry.expect("read source entry").path())
            .collect::<Vec<_>>();
        entries.sort();
        for path in entries {
            if path.is_dir() {
                if path
                    .file_name()
                    .is_some_and(|name| name == "target" || name == ".git")
                {
                    continue;
                }
                scan(root, &path, functions);
            } else if path.extension().is_some_and(|extension| extension == "rs") {
                let source = std::fs::read_to_string(&path).expect("read Rust source");
                let relative = path.strip_prefix(root).expect("source is in repository");
                functions.add(&relative.to_string_lossy().replace('\\', "/"), &source);
            }
        }
    }
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let mut functions = SourceIndex::default();
    for directory in [
        "src",
        "tests",
        "test-support/src",
        "grafton-visca-macros/src",
        "examples",
        "fuzz",
    ] {
        scan(&root, &root.join(directory), &mut functions);
    }
    functions
}

fn test_cells(document: &str, inventory: bool) -> Vec<&str> {
    let mut cells = Vec::new();
    let mut in_baselines = false;
    for line in document.lines() {
        if inventory {
            if line == "## Allocation baselines" {
                in_baselines = true;
                continue;
            }
            if in_baselines && line.starts_with("## ") {
                break;
            }
            if in_baselines {
                cells.push(line);
            }
        } else if line.starts_with('|') {
            let columns: Vec<_> = line.split('|').collect();
            match columns.len() {
                4 => cells.push(columns[2]),
                5 => cells.push(columns[3]),
                _ => {}
            }
        }
    }
    cells
}

fn validate(document: &str, inventory: bool, functions: &SourceIndex) -> Result<usize, String> {
    let mut checked = 0;
    let mut errors = Vec::new();
    // A module/file scope applies to subsequent names in the same table cell.
    // In the prose baseline list it also carries over a wrapped line.
    let mut scope: Option<String> = None;
    for cell in test_cells(document, inventory) {
        if !inventory || cell.starts_with("- ") {
            scope = None;
        }
        for code in cell.split('`').skip(1).step_by(2) {
            if code
                .split("::")
                .any(|part| part.starts_with(char::is_uppercase))
            {
                continue;
            }
            if code.starts_with("runtime::") && code.split("::").all(|part| !part.contains('_')) {
                scope = Some(code.to_owned());
                continue;
            }
            let (prefix, names) = code.split_once("::{").map_or_else(
                || {
                    code.rsplit_once("::")
                        .map_or((None, code), |(prefix, name)| (Some(prefix), name))
                },
                |(prefix, names)| (Some(prefix), names.trim_end_matches('}')),
            );
            if code.ends_with(".rs") || (code.starts_with("tests/") && !code.contains("::")) {
                let path = if code.ends_with(".rs") {
                    code.to_owned()
                } else {
                    format!("{code}.rs")
                };
                match functions.files.get(&path) {
                    None => errors.push(format!("cited suite `{code}` has no source file")),
                    Some(0) if !path.starts_with("tests/api_contract/") => {
                        errors.push(format!("cited suite `{code}` has no registered tests"))
                    }
                    Some(_) => {}
                }
                scope = Some(path);
                continue;
            }
            if let Some(prefix) = prefix {
                scope = Some(prefix.to_owned());
            }
            for name in names.split(',').map(str::trim) {
                if !name.contains('_')
                    || !name
                        .chars()
                        .all(|ch| ch.is_ascii_lowercase() || ch.is_ascii_digit() || ch == '_')
                {
                    continue;
                }
                let matches = functions
                    .functions
                    .get(name)
                    .into_iter()
                    .flatten()
                    .filter(|path| {
                        scope.as_ref().is_none_or(|scope| {
                            if scope.ends_with(".rs") {
                                path.as_str() == scope
                            } else if scope.contains("::") {
                                let module = format!("src/{}", scope.replace("::", "/"));
                                path.as_str() == format!("{module}.rs")
                                    || path.starts_with(&format!("{module}/"))
                            } else {
                                Path::new(path)
                                    .file_stem()
                                    .is_some_and(|stem| stem == scope.as_str())
                            }
                        })
                    })
                    .collect::<Vec<_>>();
                if let [path] = matches.as_slice() {
                    if functions
                        .registered
                        .contains(&(name.into(), (*path).clone()))
                    {
                        checked += 1;
                    } else {
                        errors.push(format!("test citation `{name}` in {path} names a function without test registration"));
                    }
                } else {
                    errors.push(format!("test citation `{name}` in {scope:?} must name exactly one fn; found {}: {matches:?}", matches.len()));
                }
            }
        }
    }
    if errors.is_empty() {
        Ok(checked)
    } else {
        Err(errors.join("\n"))
    }
}

#[test]
fn every_release_gate_test_citation_names_one_real_function() {
    let functions = source_functions();
    for (path, inventory, minimum) in [
        ("docs/allocation_and_validation.md", false, 30),
        ("docs/architecture_inventory.md", true, 3),
    ] {
        let document = std::fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join(path))
            .expect("release document exists");
        let checked = validate(&document, inventory, &functions)
            .unwrap_or_else(|error| panic!("{path}:\n{error}"));
        assert!(
            checked >= minimum,
            "{path}: citation extraction checked only {checked} tests"
        );
    }
}

#[test]
fn renamed_test_citation_reports_the_missing_function() {
    let source = "#[cfg(test)] mod tests { #[test] fn original_gate_test() {} // fn renamed_gate_test() {}\n const FAKE: &str = \"fn renamed_gate_test() {}\"; type Fake = fn(renamed_gate_test); }";
    let mut functions = SourceIndex::default();
    functions.add("src/tests.rs", source);
    let original = "| Contract | Test |\n| Gate | `src/tests.rs::original_gate_test` |";
    assert_eq!(validate(original, false, &functions), Ok(1));
    let renamed = original.replace("original_gate_test", "renamed_gate_test");
    let error = validate(&renamed, false, &functions).expect_err("a stale citation must fail");
    assert!(error.contains("renamed_gate_test"), "{error}");
    assert!(error.contains("found 0"), "{error}");
}

#[test]
fn duplicate_and_mis_scoped_test_citations_are_refused() {
    let mut functions = SourceIndex::default();
    for path in ["src/first.rs", "src/second.rs"] {
        functions.add(path, "#[test] fn repeated_gate_test() {}");
    }
    let citation = "| Gate | `repeated_gate_test` |";
    assert!(validate(citation, false, &functions)
        .expect_err("an ambiguous name must fail")
        .contains("found 2"));
    let qualified = citation.replace("repeated_gate_test", "src/first.rs::repeated_gate_test");
    assert_eq!(validate(&qualified, false, &functions), Ok(1));
    let wrong_path = qualified.replace("first.rs", "missing.rs");
    assert!(validate(&wrong_path, false, &functions)
        .expect_err("a declaration in a different file cannot satisfy the citation")
        .contains("found 0"));
}

#[test]
fn removed_test_attributes_and_matrix_registrations_are_rejected() {
    for source in [
        "#[test] fn cited_gate_test() {}",
        "async fn cited_gate_test() {} runtime_matrix!(cited_gate_test);",
        "facade_matrix! { fn cited_gate_test() {} }",
    ] {
        let mut functions = SourceIndex::default();
        functions.add("tests/gate.rs", source);
        let citation = "| Gate | `tests/gate.rs::cited_gate_test` |";
        assert_eq!(validate(citation, false, &functions), Ok(1));
        let mut removed = SourceIndex::default();
        removed.add("tests/gate.rs", "fn cited_gate_test() {}");
        let error = validate(citation, false, &removed)
            .expect_err("a declaration alone is not an executable test");
        assert!(
            error.contains("cited_gate_test") && error.contains("without test registration"),
            "{error}"
        );
    }
}

#[test]
fn missing_or_unregistered_cited_suites_are_rejected() {
    let mut functions = SourceIndex::default();
    functions.add("tests/valid_suite.rs", "#[test] fn a_real_test() {}");
    functions.add("tests/helpers.rs", "fn helper_only() {}");
    let valid = "| Gate | `tests/valid_suite.rs` |";
    assert_eq!(validate(valid, false, &functions), Ok(0));
    let error = validate(
        &valid.replace("valid_suite", "nonexistent_gate_suite"),
        false,
        &functions,
    )
    .expect_err("missing file must fail");
    assert!(
        error.contains("nonexistent_gate_suite.rs") && error.contains("no source file"),
        "{error}"
    );
    let error = validate(&valid.replace("valid_suite", "helpers"), false, &functions)
        .expect_err("a helper-only file is not a test suite");
    assert!(
        error.contains("helpers.rs") && error.contains("no registered tests"),
        "{error}"
    );
}
