//! Comparison and explicit regeneration of checked-in generated text.
//!
//! The same helper serves whole-file fixtures and marked sections of authored
//! documents. Regeneration always fails the invoking test, so its diff must be
//! reviewed before a normal test run can pass.

use std::{ffi::OsStr, fmt::Write as _, fs, io, ops::Range, path::Path};

const BLESS_VARIABLE: &str = "GRAFTON_VISCA_BLESS";

/// Compares a generated file or document block with its checked-in contents.
///
/// `None` selects the whole file. `Some(name)` selects the contents between
/// exactly one `<!-- BEGIN name -->` and `<!-- END name -->`, each on its own
/// line. The markers and all text outside them remain byte-for-byte intact.
/// Block output receives a final newline when needed to keep the end marker
/// on its own line. Generated newlines use the file's existing newline format;
/// blocks use the format of their begin-marker line. New files use LF.
///
/// Only `GRAFTON_VISCA_BLESS=1` permits a write. The helper then intentionally
/// panics, even when the generated contents were already current. Any presence
/// of `CI` refuses regeneration, including `CI=false`. Ordinary comparison
/// remains available in CI. A missing whole-file fixture may be created when
/// blessing; a document's block markers must already exist.
///
/// # Panics
///
/// Panics on a mismatch, malformed block markers, I/O failure, regeneration
/// under CI, or after successfully regenerating a file or block.
#[track_caller]
pub fn bless_or_assert(path: impl AsRef<Path>, block: Option<&str>, generated: &str) {
    let bless = std::env::var_os(BLESS_VARIABLE);
    let ci = std::env::var_os("CI");
    let policy = Policy::from_values(bless.as_deref(), ci.as_deref());
    if let Err(message) = compare_or_write(path.as_ref(), block, generated, policy) {
        panic!("{message}");
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct Policy {
    bless: bool,
    ci: bool,
}

impl Policy {
    fn from_values(bless: Option<&OsStr>, ci: Option<&OsStr>) -> Self {
        Self {
            bless: bless == Some(OsStr::new("1")),
            ci: ci.is_some(),
        }
    }
}

fn compare_or_write(
    path: &Path,
    block: Option<&str>,
    generated: &str,
    policy: Policy,
) -> Result<(), String> {
    if policy.bless && policy.ci {
        return Err(format!(
            "{BLESS_VARIABLE}=1 regeneration is refused under CI: {}",
            path.display(),
        ));
    }
    let original = match fs::read_to_string(path) {
        Ok(text) => Some(text),
        Err(error) if error.kind() == io::ErrorKind::NotFound && block.is_none() => None,
        Err(error) => return Err(format!("cannot read {}: {error}", path.display())),
    };
    let text = original.as_deref().unwrap_or("");
    let (range, newline) = match block {
        Some(name) => block_range(text, name)
            .map_err(|error| format!("{} (block {name:?}): {error}", path.display()))?,
        None => (0..text.len(), newline_format(text)),
    };
    let expected = generated_text(generated, newline, block.is_some());
    if let Some(name) = block {
        for marker in [
            format!("<!-- BEGIN {name} -->"),
            format!("<!-- END {name} -->"),
        ] {
            if expected.contains(&marker) {
                return Err(format!(
                    "{} (block {name:?}): generated text must not contain its own marker {marker:?}",
                    path.display(),
                ));
            }
        }
    }
    let actual = &text[range.clone()];
    if !policy.bless {
        if original.is_some() && actual == expected {
            return Ok(());
        }
        let location = block.map_or_else(
            || path.display().to_string(),
            |name| format!("{} (block {name:?})", path.display()),
        );
        let missing = if original.is_none() {
            " (file is missing)"
        } else {
            ""
        };
        return Err(format!(
            "generated text differs in {location}{missing}\n{}\n\
             Regenerate locally with {BLESS_VARIABLE}=1, review the diff, then rerun without it.\n\
             Regeneration intentionally fails and is refused under CI.",
            mismatch_diff(actual, &expected),
        ));
    }

    let mut replacement = String::with_capacity(text.len() - range.len() + expected.len());
    replacement.push_str(&text[..range.start]);
    replacement.push_str(&expected);
    replacement.push_str(&text[range.end..]);
    fs::write(path, replacement)
        .map_err(|error| format!("cannot regenerate {}: {error}", path.display()))?;
    let location = block.map_or_else(
        || path.display().to_string(),
        |name| format!("{} (block {name:?})", path.display()),
    );
    Err(format!(
        "regenerated {location}; intentionally failing after {BLESS_VARIABLE}=1. \
         Review the diff and rerun without {BLESS_VARIABLE}.",
    ))
}

fn newline_format(text: &str) -> &'static str {
    match text.find('\n') {
        Some(index) if text[..index].ends_with('\r') => "\r\n",
        _ => "\n",
    }
}

fn generated_text(generated: &str, newline: &str, block: bool) -> String {
    let mut output = generated.replace("\r\n", "\n");
    if block && !output.is_empty() && !output.ends_with('\n') {
        output.push('\n');
    }
    if newline == "\r\n" {
        output = output.replace('\n', "\r\n");
    }
    output
}

fn block_range(text: &str, name: &str) -> Result<(Range<usize>, &'static str), String> {
    if name.is_empty()
        || name.trim() != name
        || name.contains(['<', '>'])
        || name.contains("--")
        || name.chars().any(char::is_control)
    {
        return Err("block name must be nonempty, trimmed text without HTML delimiters or control characters".into());
    }
    let begin = format!("<!-- BEGIN {name} -->");
    let end = format!("<!-- END {name} -->");
    let begin_count = text.matches(&begin).count();
    let end_count = text.matches(&end).count();
    if begin_count != 1 || end_count != 1 {
        return Err(format!(
            "expected exactly one begin marker and one end marker; found {begin_count} begin and {end_count} end markers",
        ));
    }
    // The counts above prove both find operations succeed.
    let begin_start = text.find(&begin).ok_or("missing begin marker")?;
    let end_start = text.find(&end).ok_or("missing end marker")?;
    if begin_start >= end_start {
        return Err("end marker must follow the begin marker".into());
    }
    let begin_end = begin_start + begin.len();
    let begin_line_start = text[..begin_start].rfind('\n').map_or(0, |index| index + 1);
    let begin_line_end = text[begin_end..]
        .find('\n')
        .map(|index| begin_end + index)
        .ok_or("begin marker must end with a newline")?;
    let end_line_start = text[..end_start].rfind('\n').map_or(0, |index| index + 1);
    let end_end = end_start + end.len();
    let end_line_end = text[end_end..]
        .find('\n')
        .map_or(text.len(), |index| end_end + index);
    if !text[begin_line_start..begin_start].trim().is_empty()
        || !text[begin_end..begin_line_end].trim().is_empty()
        || !text[end_line_start..end_start].trim().is_empty()
        || !text[end_end..end_line_end].trim().is_empty()
    {
        return Err("begin and end markers must each occupy their own line".into());
    }
    let content_start = begin_line_end + 1;
    if end_line_start < content_start {
        return Err("end marker must be on a line after the begin marker".into());
    }
    let newline = if text[..begin_line_end].ends_with('\r') {
        "\r\n"
    } else {
        "\n"
    };
    Ok((content_start..end_line_start, newline))
}

fn mismatch_diff(actual: &str, expected: &str) -> String {
    let actual_lines: Vec<_> = actual.split_inclusive('\n').collect();
    let expected_lines: Vec<_> = expected.split_inclusive('\n').collect();
    let mut prefix = 0;
    while prefix < actual_lines.len()
        && prefix < expected_lines.len()
        && actual_lines[prefix] == expected_lines[prefix]
    {
        prefix += 1;
    }
    let mut suffix = 0;
    while suffix < actual_lines.len() - prefix
        && suffix < expected_lines.len() - prefix
        && actual_lines[actual_lines.len() - 1 - suffix]
            == expected_lines[expected_lines.len() - 1 - suffix]
    {
        suffix += 1;
    }
    let actual_end = actual_lines.len() - suffix;
    let expected_end = expected_lines.len() - suffix;
    let context_before = prefix.min(2);
    let context_after = suffix.min(2);
    let start = prefix - context_before;
    let actual_count = context_before + actual_end - prefix + context_after;
    let expected_count = context_before + expected_end - prefix + context_after;
    let mut output = format!(
        "first differing line {}: checked-in {:?}, generated {:?}\n\
         --- checked-in\n+++ generated\n@@ -{},{} +{},{} @@\n",
        prefix + 1,
        actual_lines.get(prefix).copied(),
        expected_lines.get(prefix).copied(),
        start + usize::from(actual_count > 0),
        actual_count,
        start + usize::from(expected_count > 0),
        expected_count,
    );
    diff_lines(&mut output, ' ', &actual_lines[start..prefix]);
    diff_lines(&mut output, '-', &actual_lines[prefix..actual_end]);
    diff_lines(&mut output, '+', &expected_lines[prefix..expected_end]);
    diff_lines(
        &mut output,
        ' ',
        &actual_lines[actual_end..actual_end + context_after],
    );
    output
}

fn diff_lines(output: &mut String, prefix: char, lines: &[&str]) {
    // A malformed generator should not flood a test log with an entire fixture.
    const LIMIT: usize = 40;
    for line in lines.iter().take(LIMIT) {
        output.push(prefix);
        output.push_str(line);
        if !line.ends_with('\n') {
            output.push_str("\n\\ No newline at end of text\n");
        }
    }
    if lines.len() > LIMIT {
        let _ = writeln!(
            output,
            "... {} more {prefix} lines omitted",
            lines.len() - LIMIT
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::{
        path::PathBuf,
        sync::atomic::{AtomicU64, Ordering},
    };

    const COMPARE: Policy = Policy {
        bless: false,
        ci: false,
    };
    const BLESS: Policy = Policy {
        bless: true,
        ci: false,
    };

    struct Scratch(PathBuf);

    impl Scratch {
        fn new() -> Self {
            static NEXT: AtomicU64 = AtomicU64::new(0);
            let root = std::env::temp_dir().join(format!(
                "grafton-visca-generated-{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed),
            ));
            fs::create_dir(&root).expect("unique test directory");
            Self(root)
        }

        fn file(&self) -> PathBuf {
            self.0.join("generated.md")
        }
    }

    impl Drop for Scratch {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    #[test]
    fn environment_policy_requires_exact_opt_in_and_any_ci_presence_refuses() {
        for value in [None, Some(""), Some("0"), Some("true"), Some("01")] {
            assert!(!Policy::from_values(value.map(OsStr::new), None).bless);
        }
        assert!(Policy::from_values(Some(OsStr::new("1")), None).bless);
        for value in ["", "0", "false", "true", "1"] {
            assert!(Policy::from_values(None, Some(OsStr::new(value))).ci);
        }
    }

    #[test]
    fn ordinary_comparison_never_writes_and_reports_the_changed_line() {
        let scratch = Scratch::new();
        let path = scratch.file();
        fs::write(&path, "first\nold\nlast\n").expect("fixture");
        assert!(compare_or_write(&path, None, "first\nold\nlast\n", COMPARE).is_ok());
        let error =
            compare_or_write(&path, None, "first\nnew\nlast\n", COMPARE).expect_err("mismatch");
        assert!(error.contains("first differing line 2"));
        assert!(error.contains("-old\n+new\n"));
        assert!(error.contains("GRAFTON_VISCA_BLESS=1"));
        assert_eq!(
            fs::read_to_string(path).expect("unchanged"),
            "first\nold\nlast\n"
        );
    }

    #[test]
    fn bless_rewrites_the_whole_file_then_intentionally_fails() {
        let scratch = Scratch::new();
        let path = scratch.file();
        fs::write(&path, "old\n").expect("fixture");
        let error = compare_or_write(&path, None, "new\n", BLESS).expect_err("review required");
        assert!(error.contains("intentionally failing"));
        assert_eq!(fs::read_to_string(&path).expect("updated"), "new\n");
        assert!(compare_or_write(&path, None, "new\n", COMPARE).is_ok());
        assert!(compare_or_write(&path, None, "new\n", BLESS).is_err());
    }

    #[test]
    fn missing_whole_file_is_created_only_when_blessing() {
        let scratch = Scratch::new();
        let path = scratch.file();
        let error = compare_or_write(&path, None, "new\n", COMPARE).expect_err("missing fixture");
        assert!(error.contains("file is missing"));
        assert!(!path.exists());
        assert!(compare_or_write(&path, None, "new\n", BLESS).is_err());
        assert_eq!(fs::read_to_string(path).expect("new fixture"), "new\n");
    }

    #[test]
    fn ci_refuses_blessing_before_any_write_but_allows_comparison() {
        let scratch = Scratch::new();
        let path = scratch.file();
        fs::write(&path, "old\n").expect("fixture");
        let error = compare_or_write(
            &path,
            None,
            "new\n",
            Policy {
                bless: true,
                ci: true,
            },
        )
        .expect_err("CI refusal");
        assert!(error.contains("refused under CI"));
        assert!(compare_or_write(
            &path,
            None,
            "old\n",
            Policy {
                bless: true,
                ci: true
            }
        )
        .is_err());
        assert_eq!(fs::read_to_string(&path).expect("untouched"), "old\n");
        assert!(compare_or_write(
            &path,
            None,
            "old\n",
            Policy {
                bless: false,
                ci: true
            }
        )
        .is_ok());
        fs::remove_file(&path).expect("remove fixture");
        assert!(compare_or_write(
            &path,
            None,
            "new\n",
            Policy {
                bless: true,
                ci: true
            }
        )
        .is_err());
        assert!(!path.exists());
    }

    #[test]
    fn block_bless_preserves_authored_prose_other_blocks_and_crlf() {
        let scratch = Scratch::new();
        let path = scratch.file();
        let before = "Authored α prose.\r\n\r\n  <!-- BEGIN TABLE -->  \r\nold\r\n  <!-- END TABLE -->\r\n\r\n<!-- BEGIN OTHER -->\nkept\n<!-- END OTHER -->\nAuthored ending.";
        fs::write(&path, before).expect("document");
        assert!(compare_or_write(&path, Some("TABLE"), "new\nrows", BLESS).is_err());
        let after = fs::read_to_string(&path).expect("updated document");
        assert_eq!(after, before.replace("old\r\n", "new\r\nrows\r\n"));
        assert!(compare_or_write(&path, Some("TABLE"), "new\nrows", COMPARE).is_ok());
    }

    #[test]
    fn block_comparison_accepts_empty_bodies_and_normalizes_generated_newlines() {
        let scratch = Scratch::new();
        let path = scratch.file();
        fs::write(&path, "<!-- BEGIN EMPTY -->\n<!-- END EMPTY -->").expect("empty block");
        assert!(compare_or_write(&path, Some("EMPTY"), "", COMPARE).is_ok());
        assert!(compare_or_write(&path, Some("EMPTY"), "one\r\ntwo\r\n", BLESS).is_err());
        assert_eq!(
            fs::read_to_string(&path).expect("LF document"),
            "<!-- BEGIN EMPTY -->\none\ntwo\n<!-- END EMPTY -->"
        );
    }

    #[test]
    fn malformed_or_ambiguous_markers_never_write() {
        let scratch = Scratch::new();
        let path = scratch.file();
        for document in [
            "authored prose\n",
            "<!-- BEGIN A -->\nno end\n",
            "<!-- END A -->\n<!-- BEGIN A -->\n",
            "<!-- BEGIN A -->\n<!-- BEGIN A -->\n<!-- END A -->\n",
            "<!-- BEGIN A -->\n<!-- END A -->\n<!-- END A -->\n",
            "prose <!-- BEGIN A -->\nold\n<!-- END A -->\n",
            "<!-- BEGIN A -->\nold\n<!-- END A --> prose\n",
            "<!-- BEGIN A --><!-- END A -->\n",
        ] {
            fs::write(&path, document).expect("malformed document");
            assert!(compare_or_write(&path, Some("A"), "new", BLESS).is_err());
            assert_eq!(
                fs::read_to_string(&path).expect("authored text preserved"),
                document
            );
        }
        for name in ["", " A", "A ", "A\nB", "A-->B", "A<B"] {
            assert!(block_range("authored prose", name).is_err());
        }
    }

    #[test]
    fn generated_text_cannot_inject_its_own_block_markers() {
        let scratch = Scratch::new();
        let path = scratch.file();
        let original = "<!-- BEGIN A -->\nold\n<!-- END A -->\n";
        fs::write(&path, original).expect("document");
        for generated in ["<!-- BEGIN A -->\n", "<!-- END A -->\n"] {
            let error =
                compare_or_write(&path, Some("A"), generated, BLESS).expect_err("marker injection");
            assert!(error.contains("must not contain its own marker"));
            assert_eq!(fs::read_to_string(&path).expect("unchanged"), original);
        }
    }

    #[test]
    fn whole_file_newline_format_and_final_newline_are_explicit() {
        let scratch = Scratch::new();
        let path = scratch.file();
        fs::write(&path, "one\r\ntwo\r\n").expect("CRLF fixture");
        assert!(compare_or_write(&path, None, "one\ntwo\n", COMPARE).is_ok());
        assert!(compare_or_write(&path, None, "one\nchanged\n", BLESS).is_err());
        assert_eq!(
            fs::read_to_string(&path).expect("still CRLF"),
            "one\r\nchanged\r\n"
        );
        let error = compare_or_write(&path, None, "one\nchanged", COMPARE)
            .expect_err("missing final newline");
        assert!(error.contains("No newline at end of text"));
    }

    #[test]
    fn diagnostics_bound_large_diffs_and_show_insertions() {
        let inserted = mismatch_diff("one\nthree\n", "one\ntwo\nthree\n");
        assert!(inserted.contains("@@ -1,2 +1,3 @@"));
        assert!(inserted.contains("+two\n"));
        let large = mismatch_diff("", &"new\n".repeat(100));
        assert!(large.contains("60 more + lines omitted"));
        assert!(large.lines().count() < 50);
    }
}
