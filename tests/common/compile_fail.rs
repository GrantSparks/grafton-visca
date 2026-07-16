use std::{
    env,
    ffi::OsString,
    fs,
    path::{Path, PathBuf},
    process::Command,
};

const EXPECTED_INACCESSIBLE_ERROR_CODES: &[&str] = &[
    "E0277", "E0308", "E0432", "E0433", "E0437", "E0599", "E0603",
];

pub fn active_grafton_visca_features() -> Vec<&'static str> {
    let mut features = Vec::new();

    if cfg!(feature = "default") {
        features.push("default");
    }
    if cfg!(feature = "mode-async") {
        features.push("mode-async");
    }
    if cfg!(feature = "runtime-tokio") {
        features.push("runtime-tokio");
    }
    if cfg!(feature = "runtime-smol") {
        features.push("runtime-smol");
    }
    if cfg!(feature = "transport-serial") {
        features.push("transport-serial");
    }
    if cfg!(feature = "transport-serial-tokio") {
        features.push("transport-serial-tokio");
    }
    if cfg!(feature = "serde") {
        features.push("serde");
    }
    if cfg!(feature = "schemars") {
        features.push("schemars");
    }
    if cfg!(feature = "ts-rs") {
        features.push("ts-rs");
    }
    if cfg!(feature = "dyn-api") {
        features.push("dyn-api");
    }
    if cfg!(feature = "test-utils") {
        features.push("test-utils");
    }
    if cfg!(feature = "DISABLED") {
        features.push("DISABLED");
    }

    features
}

pub fn assert_compile_fail_fixtures(fixture_dirs: &[&str], crate_features: &[&str]) {
    let crate_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    let fixtures = collect_fixtures(&crate_root, fixture_dirs);
    assert!(
        !fixtures.is_empty(),
        "no compile-fail fixtures found in {:?}",
        fixture_dirs
    );

    let work_root = crate_root
        .join("target")
        .join("contract-compile-fail")
        .join(format!(
            "{}-{}",
            std::process::id(),
            sanitize(&fixture_dirs.join("-"))
        ));
    if work_root.exists() {
        fs::remove_dir_all(&work_root).unwrap_or_else(|error| {
            panic!(
                "failed to remove old compile-fail work dir {}: {error}",
                work_root.display()
            )
        });
    }
    fs::create_dir_all(&work_root).unwrap_or_else(|error| {
        panic!(
            "failed to create compile-fail work dir {}: {error}",
            work_root.display()
        )
    });

    prefetch_contract_dependencies(&crate_root, &work_root, crate_features);

    for (index, fixture) in fixtures.iter().enumerate() {
        assert_fixture_does_not_compile(&crate_root, &work_root, index, fixture, crate_features);
    }
}

fn collect_fixtures(crate_root: &Path, fixture_dirs: &[&str]) -> Vec<PathBuf> {
    let mut fixtures = Vec::new();

    for fixture_dir in fixture_dirs {
        let absolute_dir = crate_root.join(fixture_dir);
        let entries = fs::read_dir(&absolute_dir).unwrap_or_else(|error| {
            panic!(
                "failed to read compile-fail fixture dir {}: {error}",
                absolute_dir.display()
            )
        });

        for entry in entries {
            let entry = entry.unwrap_or_else(|error| {
                panic!(
                    "failed to read an entry in fixture dir {}: {error}",
                    absolute_dir.display()
                )
            });
            let path = entry.path();
            if path.extension().is_some_and(|extension| extension == "rs") {
                fixtures.push(path);
            }
        }
    }

    fixtures.sort();
    fixtures
}

fn prefetch_contract_dependencies(crate_root: &Path, work_root: &Path, crate_features: &[&str]) {
    let prefetch_dir = work_root.join("prefetch-dependencies");
    let prefetch_src_dir = prefetch_dir.join("src");
    fs::create_dir_all(&prefetch_src_dir).unwrap_or_else(|error| {
        panic!(
            "failed to create compile-fail dependency prefetch dir {}: {error}",
            prefetch_src_dir.display()
        )
    });
    fs::write(
        prefetch_dir.join("Cargo.toml"),
        case_manifest(crate_root, 0, "prefetch-dependencies", crate_features),
    )
    .unwrap_or_else(|error| {
        panic!(
            "failed to write compile-fail dependency prefetch manifest {}: {error}",
            prefetch_dir.display()
        )
    });
    fs::write(prefetch_src_dir.join("lib.rs"), "").unwrap_or_else(|error| {
        panic!(
            "failed to write compile-fail dependency prefetch source {}: {error}",
            prefetch_src_dir.display()
        )
    });

    let mut command = Command::new(cargo_executable());
    command
        .arg("fetch")
        .arg("--quiet")
        .arg("--manifest-path")
        .arg(prefetch_dir.join("Cargo.toml"))
        .env("CARGO_HTTP_MULTIPLEXING", "false")
        .env("CARGO_NET_RETRY", "10");

    let output = command.output().unwrap_or_else(|error| {
        panic!("failed to prefetch dependencies for compile-fail fixtures: {error}")
    });

    if !output.status.success() {
        let stdout = String::from_utf8_lossy(&output.stdout);
        let stderr = String::from_utf8_lossy(&output.stderr);
        panic!(
            "failed to prefetch dependencies for compile-fail fixtures\nstdout:\n{stdout}\nstderr:\n{stderr}"
        );
    }
}

fn assert_fixture_does_not_compile(
    crate_root: &Path,
    work_root: &Path,
    index: usize,
    fixture: &Path,
    crate_features: &[&str],
) {
    let fixture_name = fixture
        .file_stem()
        .and_then(|stem| stem.to_str())
        .unwrap_or("fixture");
    let case_dir = work_root.join(format!("{index:02}-{}", sanitize(fixture_name)));
    let src_dir = case_dir.join("src");
    fs::create_dir_all(&src_dir).unwrap_or_else(|error| {
        panic!(
            "failed to create compile-fail case dir {}: {error}",
            src_dir.display()
        )
    });

    let source = fs::read_to_string(fixture)
        .unwrap_or_else(|error| panic!("failed to read fixture {}: {error}", fixture.display()));
    fs::write(src_dir.join("main.rs"), source).unwrap_or_else(|error| {
        panic!(
            "failed to write compile-fail case source for {}: {error}",
            fixture.display()
        )
    });
    fs::write(
        case_dir.join("Cargo.toml"),
        case_manifest(crate_root, index, fixture_name, crate_features),
    )
    .unwrap_or_else(|error| {
        panic!(
            "failed to write compile-fail case manifest for {}: {error}",
            fixture.display()
        )
    });

    let output = Command::new(cargo_executable())
        .arg("check")
        .arg("--offline")
        .arg("--quiet")
        .arg("--manifest-path")
        .arg(case_dir.join("Cargo.toml"))
        .env(
            "CARGO_TARGET_DIR",
            crate_root
                .join("target")
                .join("contract-compile-fail-target"),
        )
        .output()
        .unwrap_or_else(|error| {
            panic!(
                "failed to run cargo check for {}: {error}",
                fixture.display()
            )
        });

    let fixture_label = fixture
        .strip_prefix(crate_root)
        .unwrap_or(fixture)
        .display()
        .to_string();
    if output.status.success() {
        panic!("{fixture_label} unexpectedly compiled; an internal API may have become public");
    }

    let stderr = String::from_utf8_lossy(&output.stderr);
    if !EXPECTED_INACCESSIBLE_ERROR_CODES
        .iter()
        .any(|code| stderr.contains(code))
    {
        let stdout = String::from_utf8_lossy(&output.stdout);
        panic!(
            "{fixture_label} failed without an expected privacy/removal error code \
             ({EXPECTED_INACCESSIBLE_ERROR_CODES:?})\nstdout:\n{stdout}\nstderr:\n{stderr}"
        );
    }
}

fn case_manifest(
    crate_root: &Path,
    index: usize,
    fixture_name: &str,
    crate_features: &[&str],
) -> String {
    format!(
        r#"[package]
name = "grafton-visca-contract-{index:02}-{fixture_name}"
version = "0.0.0"
edition = "2021"
publish = false

[workspace]

[dependencies]
grafton-visca = {{ path = {}, default-features = false, features = {} }}
"#,
        toml_string(&crate_root.to_string_lossy()),
        toml_array(crate_features),
        fixture_name = sanitize(fixture_name),
    )
}

fn cargo_executable() -> OsString {
    env::var_os("CARGO").unwrap_or_else(|| OsString::from("cargo"))
}

fn toml_array(values: &[&str]) -> String {
    let values = values
        .iter()
        .map(|value| toml_string(value))
        .collect::<Vec<_>>();
    format!("[{}]", values.join(", "))
}

fn toml_string(value: &str) -> String {
    let mut output = String::from("\"");
    for character in value.chars() {
        match character {
            '\\' => output.push_str("\\\\"),
            '"' => output.push_str("\\\""),
            '\n' => output.push_str("\\n"),
            '\r' => output.push_str("\\r"),
            '\t' => output.push_str("\\t"),
            character => output.push(character),
        }
    }
    output.push('"');
    output
}

fn sanitize(value: &str) -> String {
    value
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() {
                character.to_ascii_lowercase()
            } else {
                '-'
            }
        })
        .collect()
}
