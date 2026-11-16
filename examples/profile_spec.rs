//! Export and validate complete camera-profile specifications without hardware.
//!
//! ```sh
//! cargo run --example profile_spec --no-default-features --features serde -- export generic_visca > camera.json
//! cargo run --example profile_spec --no-default-features --features serde -- check camera.json
//! ```
//!
//! An exported built-in specification retains its identity and must continue to
//! match that registry entry exactly. To describe an unlisted camera, set
//! `capabilities.profile_id` to `null`, edit its documented facts, and run
//! `check`. The validated object is also accepted by `ProfileSelection` in a
//! runtime camera configuration. Checking a file performs no camera I/O.

use std::{
    ffi::OsString,
    fs::File,
    io::{self, Write},
    path::{Path, PathBuf},
    process::ExitCode,
};

use grafton_visca::profiles::{ProfileId, ProfileSpec};

const USAGE: &str =
    "Usage: profile_spec export <id>\n       profile_spec check <file>\n       profile_spec --help";

enum Command {
    Export(ProfileId),
    Check(PathBuf),
    Help,
}

fn available_profiles() -> String {
    ProfileId::all()
        .iter()
        .map(ProfileId::as_str)
        .collect::<Vec<_>>()
        .join(", ")
}

fn parse_args(arguments: &[OsString]) -> Result<Command, String> {
    match arguments {
        [command] if command == "--help" || command == "-h" => Ok(Command::Help),
        [command, id] if command == "export" => {
            let id = id.to_str().ok_or("profile id must be valid UTF-8")?;
            id.parse::<ProfileId>()
                .map(Command::Export)
                .map_err(|error| error.to_string())
        }
        [command, path] if command == "check" => Ok(Command::Check(path.into())),
        _ => Err(format!(
            "expected exactly `export <id>` or `check <file>`\n{USAGE}"
        )),
    }
}

fn export(id: ProfileId, output: &mut impl Write) -> Result<(), String> {
    let spec = id
        .spec()
        .map_err(|error| format!("cannot resolve profile {id:?}: {error}"))?;
    serde_json::to_writer_pretty(&mut *output, &spec)
        .map_err(|error| format!("cannot write profile JSON: {error}"))?;
    writeln!(output).map_err(|error| format!("cannot finish profile JSON: {error}"))
}

fn read_spec(path: &Path) -> Result<ProfileSpec, String> {
    let file = File::open(path).map_err(|error| {
        format!(
            "cannot open profile specification {}: {error}",
            path.display()
        )
    })?;
    // ProfileSpec's canonical Deserialize implementation validates the complete
    // facts, grant/encoding agreement and any claimed built-in identity.
    serde_json::from_reader(file)
        .map_err(|error| format!("invalid profile specification {}: {error}", path.display()))
}

fn summary(spec: &ProfileSpec, output: &mut impl Write) -> Result<(), String> {
    let render = |value| {
        serde_json::to_string(value)
            .map_err(|error| format!("cannot render profile inventory: {error}"))
    };
    let surfaces = spec.capabilities().typed_support.iter().collect::<Vec<_>>();
    let encodings = serde_json::to_value(spec.encodings())
        .map_err(|error| format!("cannot render profile encodings: {error}"))?;
    let encodings = encodings
        .as_object()
        .ok_or("profile encodings must serialize as an object")?;
    writeln!(output, "Validated profile: {}", spec.name())
        .map_err(|error| format!("cannot write profile summary: {error}"))?;
    match spec.capabilities().profile_id {
        Some(id) => writeln!(output, "Identity: {}", id.as_str()),
        None => writeln!(output, "Identity: custom"),
    }
    .map_err(|error| format!("cannot write profile identity: {error}"))?;
    writeln!(output, "Granted typed surfaces ({}):", surfaces.len())
        .map_err(|error| format!("cannot write profile surfaces: {error}"))?;
    for surface in &surfaces {
        writeln!(output, "  {}", render(surface)?)
            .map_err(|error| format!("cannot write profile surface: {error}"))?;
    }
    writeln!(output, "Selected encodings ({}):", encodings.len())
        .map_err(|error| format!("cannot write profile encodings: {error}"))?;
    for (surface, encoding) in encodings {
        writeln!(output, "  {surface}: {encoding}")
            .map_err(|error| format!("cannot write profile encoding: {error}"))?;
    }
    if let Some(power) = spec.capabilities().power {
        let query = serde_json::to_string(&power.status_query)
            .map_err(|error| format!("cannot render power query: {error}"))?;
        writeln!(output, "Power status query: {query}")
            .map_err(|error| format!("cannot write power query: {error}"))?;
    }
    writeln!(output, "Allowed transports:")
        .map_err(|error| format!("cannot write profile transports: {error}"))?;
    let transports = spec.transports();
    if let Some(port) = transports.tcp_port() {
        writeln!(output, "  tcp (default port {port})")
            .map_err(|error| format!("cannot write TCP transport: {error}"))?;
    }
    if let Some(port) = transports.udp_port() {
        writeln!(output, "  udp (default port {port})")
            .map_err(|error| format!("cannot write UDP transport: {error}"))?;
    }
    if transports.supports_serial() {
        writeln!(output, "  serial")
            .map_err(|error| format!("cannot write serial transport: {error}"))?;
    }
    Ok(())
}

fn run(arguments: &[OsString], output: &mut impl Write) -> Result<(), String> {
    match parse_args(arguments)? {
        Command::Export(id) => export(id, output),
        Command::Check(path) => summary(&read_spec(&path)?, output),
        Command::Help => writeln!(output, "{USAGE}\n\nAvailable ids: {}", available_profiles())
            .map_err(|error| format!("cannot write usage: {error}")),
    }
}

fn main() -> ExitCode {
    let arguments: Vec<_> = std::env::args_os().skip(1).collect();
    match run(&arguments, &mut io::stdout().lock()) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("{error}");
            ExitCode::FAILURE
        }
    }
}

#[cfg(test)]
#[allow(clippy::expect_used)]
mod tests {
    use super::*;
    use grafton_visca::profiles::ProfileSelection;
    use std::{
        ffi::OsStr,
        fs,
        sync::atomic::{AtomicU64, Ordering},
    };

    struct Scratch(PathBuf);

    impl Scratch {
        fn new() -> Self {
            static NEXT: AtomicU64 = AtomicU64::new(0);
            let path = std::env::temp_dir().join(format!(
                "grafton-visca-profile-spec-{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed),
            ));
            fs::create_dir(&path).expect("temporary profile directory");
            Self(path)
        }

        fn path(&self) -> PathBuf {
            self.0.join("camera.json")
        }
    }

    impl Drop for Scratch {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    fn args(command: &str, argument: impl AsRef<OsStr>) -> [OsString; 2] {
        [command.into(), argument.as_ref().into()]
    }

    #[test]
    fn every_registry_profile_exports_and_checks_through_the_actual_cli_paths() {
        let scratch = Scratch::new();
        for id in ProfileId::all() {
            let tag = id.as_str();
            let mut exported = Vec::new();
            run(&args("export", tag), &mut exported).expect("export built-in");
            assert!(exported.ends_with(b"\n"));
            fs::write(scratch.path(), &exported).expect("exported JSON file");
            let loaded = read_spec(&scratch.path()).expect("strict spec validation");
            assert_eq!(loaded, id.spec().expect("registry spec"));
            let selected: ProfileSelection =
                serde_json::from_slice(&exported).expect("runtime object selection");
            assert_eq!(selected.spec().expect("runtime profile"), loaded);
            let mut output = Vec::new();
            run(&args("check", scratch.path()), &mut output).expect("check exported profile");
            let output = String::from_utf8(output).expect("UTF-8 output");
            assert!(output.contains(&format!("Identity: {tag}")));
            for surface in loaded.capabilities().typed_support.iter() {
                assert!(
                    output.contains(&serde_json::to_string(&surface).expect("surface spelling"))
                );
            }
            let encodings = serde_json::to_value(loaded.encodings()).expect("encoding object");
            for (surface, value) in encodings.as_object().expect("map") {
                assert!(output.contains(&format!("{surface}: {value}")));
            }
            if let Some(power) = loaded.capabilities().power {
                let query = serde_json::to_string(&power.status_query).expect("query spelling");
                assert!(output.contains(&format!("Power status query: {query}")));
            }
            assert_eq!(
                output.contains("  tcp ("),
                loaded.transports().tcp_port().is_some()
            );
            assert_eq!(
                output.contains("  udp ("),
                loaded.transports().udp_port().is_some()
            );
            assert_eq!(
                output.contains("  serial\n"),
                loaded.transports().supports_serial()
            );
        }
    }

    #[test]
    fn a_custom_json_object_uses_the_same_validator_and_runtime_selection() {
        let scratch = Scratch::new();
        let mut document =
            serde_json::to_value(ProfileId::GenericVisca.spec().expect("generic template"))
                .expect("JSON template");
        document["capabilities"]["profile_id"] = serde_json::Value::Null;
        document["capabilities"]["model_name"] = "Acme Cam".into();
        fs::write(
            scratch.path(),
            serde_json::to_vec_pretty(&document).expect("custom JSON"),
        )
        .expect("custom file");
        let spec = read_spec(&scratch.path()).expect("validated custom facts");
        assert_eq!(spec.capabilities().profile_id, None);
        let selection: ProfileSelection =
            serde_json::from_value(document).expect("runtime custom selection");
        assert_eq!(selection.spec().expect("resolved custom selection"), spec);
        let mut output = Vec::new();
        run(&args("check", scratch.path()), &mut output).expect("custom check");
        let output = String::from_utf8(output).expect("summary");
        assert!(output.contains("Validated profile: Acme Cam"));
        assert!(output.contains("Identity: custom"));
    }

    #[test]
    fn checking_custom_facts_never_adds_grants_encodings_or_rewrites_the_file() {
        let scratch = Scratch::new();
        let mut document =
            serde_json::to_value(ProfileId::PtzOpticsG2.spec().expect("profile template"))
                .expect("JSON template");
        document["capabilities"]["profile_id"] = serde_json::Value::Null;
        document["capabilities"]["model_name"] = "Acme Cam".into();
        let grants = document["capabilities"]["typed_support"]
            .as_array_mut()
            .expect("grant array");
        let previous_grants = grants.len();
        grants.retain(|surface| surface != "preset_recall_speed");
        assert_eq!(grants.len(), previous_grants - 1);
        assert!(document["encodings"]
            .as_object_mut()
            .expect("encoding map")
            .remove("preset_recall_speed")
            .is_some());
        let before = serde_json::to_vec_pretty(&document).expect("custom JSON");
        fs::write(scratch.path(), &before).expect("custom file");
        let mut output = Vec::new();
        run(&args("check", scratch.path()), &mut output).expect("check custom facts");
        assert_eq!(fs::read(scratch.path()).expect("checked file"), before);
        let checked = serde_json::to_value(read_spec(&scratch.path()).expect("validated facts"))
            .expect("serialized validated facts");
        assert_eq!(checked, document);
        let output = String::from_utf8(output).expect("summary");
        assert!(!output.contains("preset_recall_speed"));
    }

    #[test]
    fn check_reports_path_json_location_and_identity_validation_errors() {
        let scratch = Scratch::new();
        let path = scratch.path();
        let missing = read_spec(&path).expect_err("missing file");
        assert!(missing.contains(&path.display().to_string()));
        assert!(missing.contains("cannot open"));
        fs::write(&path, "{\n  broken\n").expect("invalid JSON");
        let syntax = read_spec(&path).expect_err("JSON syntax error");
        assert!(syntax.contains(&path.display().to_string()));
        assert!(syntax.contains("line 2"));
        let mut changed = serde_json::to_value(ProfileId::GenericVisca.spec().expect("generic"))
            .expect("template");
        changed["capabilities"]["model_name"] = "Different model".into();
        fs::write(&path, serde_json::to_vec(&changed).expect("changed JSON"))
            .expect("mismatched built-in file");
        let mismatch = read_spec(&path).expect_err("claimed built-in identity must match");
        assert!(mismatch.contains(&path.display().to_string()));
        assert!(mismatch.contains("capabilities.profile_id"));
        assert!(mismatch.contains("built-in identity"));
    }

    #[test]
    fn every_export_retains_all_required_top_level_fact_keys() {
        for id in ProfileId::all() {
            let value = serde_json::to_value(id.spec().expect("profile")).expect("JSON");
            for key in value.as_object().expect("fact object").keys() {
                let mut missing = value.clone();
                missing.as_object_mut().expect("fact object").remove(key);
                assert!(
                    serde_json::from_value::<ProfileSpec>(missing).is_err(),
                    "{id:?}: missing required fact {key}"
                );
            }
            let mut unknown = value;
            unknown
                .as_object_mut()
                .expect("fact object")
                .insert("unknown_fact".into(), true.into());
            assert!(serde_json::from_value::<ProfileSpec>(unknown).is_err());
        }
    }

    #[test]
    fn argument_validation_uses_only_canonical_registry_ids() {
        for arguments in [
            vec![],
            vec!["export".into()],
            vec!["check".into()],
            vec!["unknown".into()],
            vec!["export".into(), "generic_visca".into(), "extra".into()],
        ] {
            assert!(parse_args(&arguments).is_err());
        }
        for invalid in ["generic-visca", "GenericVisca", "unknown_profile"] {
            let error = parse_args(&args("export", invalid))
                .err()
                .expect("unknown canonical id");
            assert!(error.contains(invalid));
            for id in ProfileId::all() {
                assert!(error.contains(id.as_str()));
            }
        }
    }

    #[test]
    fn export_propagates_writer_failures() {
        struct Closed;
        impl Write for Closed {
            fn write(&mut self, _: &[u8]) -> io::Result<usize> {
                Err(io::Error::new(io::ErrorKind::BrokenPipe, "closed output"))
            }
            fn flush(&mut self) -> io::Result<()> {
                Ok(())
            }
        }
        let error = export(ProfileId::GenericVisca, &mut Closed).expect_err("output failure");
        assert!(error.contains("closed output"));
    }
}
