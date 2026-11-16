//! Release-gate test citations must resolve to real, uniquely named functions.

use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
};

use grafton_visca_test_support::source_scan;

type Functions = BTreeMap<String, Vec<String>>;

fn source_functions() -> Functions {
    fn scan(root: &Path, path: &Path, functions: &mut Functions) {
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
                for name in source_scan::function_names(&source) {
                    functions
                        .entry(name)
                        .or_default()
                        .push(relative.to_string_lossy().replace('\\', "/"));
                }
            }
        }
    }
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let mut functions = Functions::new();
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

fn validate(document: &str, inventory: bool, functions: &Functions) -> Result<usize, String> {
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
                scope = Some(code.to_owned());
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
                if matches.len() == 1 {
                    checked += 1;
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
    let source = "#[cfg(test)] mod tests { fn original_gate_test() {} // fn renamed_gate_test() {}\n const FAKE: &str = \"fn renamed_gate_test() {}\"; type Fake = fn(renamed_gate_test); }";
    let mut functions = Functions::new();
    for name in source_scan::function_names(source) {
        functions
            .entry(name)
            .or_default()
            .push("src/tests.rs".into());
    }
    let original = "| Contract | Test |\n| Gate | `src/tests.rs::original_gate_test` |";
    assert_eq!(validate(original, false, &functions), Ok(1));
    let renamed = original.replace("original_gate_test", "renamed_gate_test");
    let error = validate(&renamed, false, &functions).expect_err("a stale citation must fail");
    assert!(error.contains("renamed_gate_test"), "{error}");
    assert!(error.contains("found 0"), "{error}");
}

#[test]
fn duplicate_and_mis_scoped_test_citations_are_refused() {
    let functions = Functions::from([(
        "repeated_gate_test".into(),
        vec!["src/first.rs".into(), "src/second.rs".into()],
    )]);
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
