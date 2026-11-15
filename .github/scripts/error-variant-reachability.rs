//! Standalone release gate; reuse the integration gates' scanner without Cargo.
#[path = "../../test-support/src/source_scan.rs"]
mod source_scan;

use std::{collections::BTreeSet, env, fs, path::Path, process::ExitCode};

fn main() -> ExitCode {
    let root = env::args_os().nth(1).expect("repository root argument");
    let root = Path::new(&root);
    let source = fs::read_to_string(root.join("src/error.rs")).expect("read public Error source");
    let production = source_scan::production_code(&source);
    // Keep the human-readable enum declaration inventory independent from the
    // reviewed constructor inventory. Attribute literals and test code are gone.
    let body = production
        .split_once("pub enum Error {")
        .expect("public Error declaration")
        .1;
    let body = body.split_once("\n}").expect("end of public Error").0;
    let declared: BTreeSet<_> = body
        .lines()
        .filter_map(|line| {
            let rest = line.strip_prefix("    ")?;
            if !rest.starts_with(char::is_uppercase) {
                return None;
            }
            Some(
                rest.split(|ch: char| !ch.is_alphanumeric() && ch != '_')
                    .next()
                    .unwrap()
                    .to_owned(),
            )
        })
        .collect();
    assert!(!declared.is_empty(), "no public Error variants found");
    let inventory = include_str!("error-variant-constructors.tsv");
    let mut reviewed = BTreeSet::new();
    let mut failed = false;
    for line in inventory
        .lines()
        .filter(|line| !line.is_empty() && !line.starts_with('#'))
    {
        let mut fields = line.splitn(3, '\t');
        let variant = fields.next().unwrap();
        let path = fields.next().expect("inventory path");
        let expression = fields.next().expect("inventory expression");
        if !reviewed.insert(variant.to_owned()) {
            eprintln!("Duplicate Error::{variant} construction inventory entry");
            failed = true;
            continue;
        }
        let source = fs::read_to_string(root.join(path)).unwrap_or_default();
        let source_tokens = source_scan::production_tokens(&source);
        let expression_tokens = source_scan::production_tokens(expression);
        assert!(
            !expression_tokens.is_empty(),
            "empty expression for {variant}"
        );
        let count = source_tokens
            .windows(expression_tokens.len())
            .filter(|window| *window == expression_tokens)
            .count();
        if count != 1 {
            eprintln!("Error::{variant} production constructor occurs {count} times; expected exactly one:");
            eprintln!("  {path}: {expression}");
            failed = true;
        }
    }
    for variant in declared.difference(&reviewed) {
        eprintln!("Error::{variant} has no inventoried production constructor");
        failed = true;
    }
    for variant in reviewed.difference(&declared) {
        eprintln!("Error::{variant} construction inventory entry is not a public variant");
        failed = true;
    }
    if failed {
        ExitCode::FAILURE
    } else {
        println!(
            "Verified unique production constructor expressions for {} public Error variants.",
            declared.len()
        );
        ExitCode::SUCCESS
    }
}
