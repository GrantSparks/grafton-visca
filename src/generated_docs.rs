//! Deterministic documentation projections of the live registries.
//!
//! Registry projections use live values; test citations use the shared
//! delimiter-aware source scanner. No test needs a camera connection.
//! Authored evidence remains separate from the generated grant-key index.

#![allow(clippy::expect_used, clippy::panic)]

use std::{collections::BTreeSet, fmt::Write as _, path::PathBuf};

use grafton_visca_test_support::bless_or_assert;

#[path = "release_gate_citations.rs"]
mod release_gate_citations;

use crate::{
    camera::profiles::BUILTIN_PROFILE_FACTS,
    capabilities::TypedSupportSurface,
    command::semantics::{
        AppliedStateEffectRequirement, BuiltinAxisSelection, BuiltinCommand, BuiltinRequestClass,
    },
    noun_metadata::{self, NounRow},
    profiles::{ProfileId, ProfileSpec},
    source_evidence::Source,
    state_cache::StateKey,
};

#[derive(Clone)]
struct Profile {
    id: ProfileId,
    name: &'static str,
    spec: ProfileSpec,
}

#[derive(Clone)]
struct Surface {
    surface: TypedSupportSurface,
    area: &'static str,
    marker: &'static str,
    methods: String,
}

#[derive(Clone)]
struct Grant {
    profile: &'static str,
    surface: TypedSupportSurface,
    source: &'static str,
    key: &'static str,
}

#[derive(Clone)]
struct Documentation {
    profiles: Vec<Profile>,
    surfaces: Vec<Surface>,
    methods: Vec<NounRow>,
    grants: Vec<Grant>,
}

impl Documentation {
    fn current() -> Self {
        let profiles = ProfileId::all()
            .iter()
            .map(|&id| Profile {
                id,
                name: id.compile_time_type_name(),
                spec: id.spec().expect("registered profile validates"),
            })
            .collect();
        let surfaces = TypedSupportSurface::ALL
            .into_iter()
            .map(|surface| Surface {
                surface,
                area: surface.documentation_area(),
                marker: surface.marker_trait_name(),
                methods: surface.documentation_api(),
            })
            .collect();
        let mut grants = Vec::new();
        for profile in BUILTIN_PROFILE_FACTS {
            for &(surface, source) in profile.typed_sources {
                let Source::References(references) = source else {
                    panic!("built-in grant requires positive source references");
                };
                for reference in references
                    .iter()
                    .filter(|reference| reference.models.contains(&profile.id))
                {
                    grants.push(Grant {
                        profile: profile.type_name,
                        surface,
                        source: reference.id.as_str(),
                        key: reference.row,
                    });
                }
            }
        }
        grants.sort_by_key(|grant| (grant.profile, grant.surface as u8, grant.source, grant.key));
        Self {
            profiles,
            surfaces,
            methods: noun_metadata::rows().to_vec(),
            grants,
        }
    }

    fn profile_names(&self, permits: impl Fn(&ProfileSpec) -> bool) -> String {
        let names: Vec<_> = self
            .profiles
            .iter()
            .filter(|profile| permits(&profile.spec))
            .map(|profile| format!("`{}`", profile.name))
            .collect();
        if names.is_empty() {
            "No built-in profile".into()
        } else {
            names.join(", ")
        }
    }

    fn readme_surfaces(&self) -> String {
        let mut output = table("Typed surface | Marker | Built-in profiles");
        for surface in &self.surfaces {
            let _ = writeln!(
                output,
                "| `{:?}` | `{}` | {} |",
                surface.surface,
                surface.marker,
                self.profile_names(|profile| profile.capabilities().supports(surface.surface)),
            );
        }
        output
    }

    fn readme_nouns(&self) -> String {
        let mut output = table("Noun | Built-in method availability");
        let mut seen = BTreeSet::new();
        for row in &self.methods {
            if !seen.insert(row.noun) {
                continue;
            }
            let methods: Vec<_> = self
                .methods
                .iter()
                .filter(|method| method.noun == row.noun)
                .collect();
            let counts = self
                .profiles
                .iter()
                .map(|profile| {
                    let count = methods
                        .iter()
                        .filter(|method| {
                            method.gate.validate(&profile.spec, "documentation").is_ok()
                        })
                        .count();
                    format!("`{}`: {count}/{}", profile.name, methods.len())
                })
                .collect::<Vec<_>>()
                .join("; ");
            let _ = writeln!(output, "| `{}()` | {counts} |", row.noun);
        }
        output
    }

    fn vocabulary(&self) -> String {
        let mut output =
            String::from("| Area | Marker | Typed surface |\n| ---- | ------ | ------------- |\n");
        for surface in &self.surfaces {
            let _ = writeln!(
                output,
                "| {} | `{}` | {} |",
                surface.area, surface.marker, surface.methods,
            );
        }
        output
    }

    fn method_support(&self) -> String {
        let mut output = table("Method | Required markers | Result payload | Built-in profiles");
        for row in &self.methods {
            let markers = if row.gate_markers.is_empty() {
                "none".into()
            } else {
                inline_names(row.gate_markers.iter().copied())
            };
            let _ = writeln!(
                output,
                "| `{}().{}` | {markers} | `{}` | {} |",
                row.noun,
                row.method,
                escape_cell(row.return_type),
                self.profile_names(|profile| row.gate.validate(profile, "documentation").is_ok()),
            );
        }
        output
    }

    fn transports(&self) -> String {
        let mut output = table("Profile | TCP default | UDP default | Serial | Envelope");
        for profile in &self.profiles {
            let transports = profile.spec.transports();
            let _ = writeln!(
                output,
                "| `{}` | {} | {} | {} | `{:?}` |",
                profile.name,
                port(transports.tcp_port()),
                port(transports.udp_port()),
                if transports.supports_serial() {
                    "yes"
                } else {
                    "no"
                },
                profile.spec.envelope(),
            );
        }
        output
    }

    fn profile_markers(&self) -> String {
        let mut output = table("Profile | Typed support markers");
        for profile in &self.profiles {
            let _ = writeln!(
                output,
                "| `{}` | {} |",
                profile.name,
                inline_names(
                    profile
                        .spec
                        .capabilities()
                        .typed_support
                        .iter()
                        .map(|surface| surface.marker_trait_name())
                ),
            );
        }
        output
    }

    fn focus_zones(&self) -> String {
        let mut output = String::from("| Profile | `focus_zones` |\n| ------- | ------------- |\n");
        for profile in &self.profiles {
            let zones = profile
                .spec
                .capabilities()
                .focus
                .as_ref()
                .map(|focus| focus.zones.as_slice())
                .unwrap_or_default();
            let zones = if zones.is_empty() {
                "none".into()
            } else {
                zones
                    .iter()
                    .map(|zone| format!("`{zone:?}`"))
                    .collect::<Vec<_>>()
                    .join(", ")
            };
            let _ = writeln!(output, "| `{}` | {zones} |", profile.name);
        }
        output
    }

    fn typed_gates(&self) -> String {
        inline_names(
            self.surfaces
                .iter()
                .map(|surface| format!("{:?}", surface.surface)),
        ) + ".\n"
    }

    fn nouns(&self) -> String {
        let mut seen = BTreeSet::new();
        inline_names(
            self.methods
                .iter()
                .filter(|row| seen.insert(row.noun))
                .map(|row| format!("{}Accessor", row.noun_name)),
        ) + ".\n"
    }

    fn noun_entry_points(&self) -> String {
        let mut output = table("Both facades | Accessor | Methods");
        let mut seen = BTreeSet::new();
        for row in &self.methods {
            if seen.insert(row.noun) {
                let count = self
                    .methods
                    .iter()
                    .filter(|method| method.noun == row.noun)
                    .count();
                let _ = writeln!(
                    output,
                    "| `{}()` | `{}Accessor` | {count} |",
                    row.noun, row.noun_name
                );
            }
        }
        output
    }

    fn timings(&self) -> String {
        let mut output = table("Profile | ACK (ms) | Inquiry (ms) | Command spacing (ms) | Inquiry spacing (ms) | Quick / movement / preset / long-running / network (ms)");
        for profile in &self.profiles {
            let timing = profile.spec.timing();
            let commands = timing.command_timeouts();
            let _ = writeln!(
                output,
                "| `{}` | {} | {} | {} | {} | {} / {} / {} / {} / {} |",
                profile.name,
                timing.ack_timeout().as_millis(),
                timing.inquiry_timeout().as_millis(),
                timing.command_spacing().as_millis(),
                timing.inquiry_spacing().as_millis(),
                commands.quick_timeout().as_millis(),
                commands.movement_timeout().as_millis(),
                commands.preset_timeout().as_millis(),
                commands.long_running_timeout().as_millis(),
                commands.network_timeout().as_millis(),
            );
        }
        output
    }

    fn grant_index(&self) -> String {
        let mut output = String::from("## Typed grant key index\n\n");
        output.push_str(&table(
            "Profile | Typed grant | Source | Authored evidence key",
        ));
        for grant in &self.grants {
            let _ = writeln!(
                output,
                "| `{}` | `{:?}` | `{}` | `{}` |",
                grant.profile, grant.surface, grant.source, grant.key
            );
        }
        output
    }
}

fn table(columns: &str) -> String {
    format!(
        "| {columns} |\n| {} |\n",
        columns
            .split('|')
            .map(|_| "---")
            .collect::<Vec<_>>()
            .join(" | ")
    )
}

fn inline_names(names: impl IntoIterator<Item = impl AsRef<str>>) -> String {
    names
        .into_iter()
        .map(|name| format!("`{}`", name.as_ref()))
        .collect::<Vec<_>>()
        .join(", ")
}

fn port(port: Option<u16>) -> String {
    port.map_or_else(|| "n/a".into(), |port| port.to_string())
}

fn escape_cell(cell: &str) -> String {
    cell.replace('|', "\\|").replace('\n', " ")
}

fn test_utilities() -> String {
    let mut output = table("Public module | Utility | Export condition");
    let utilities = crate::testing::PUBLIC_TEST_UTILITIES
        .iter()
        .map(|&(name, condition)| ("testing", name, condition))
        .chain(
            crate::testing::testkit_metadata::PUBLIC_TEST_UTILITIES
                .iter()
                .map(|&(name, condition)| ("testing::testkit", name, condition)),
        );
    for (module, name, condition) in utilities {
        let _ = writeln!(output, "| `{module}` | `{name}` | `{condition}` |");
    }
    output
}

fn ledger_rows() -> Vec<(BuiltinCommand, BuiltinRequestClass)> {
    BuiltinCommand::ALL
        .iter()
        .map(|&row| (row, row.classification()))
        .collect()
}

fn ledger_decisions(rows: &[(BuiltinCommand, BuiltinRequestClass)]) -> String {
    fn axes(selection: BuiltinAxisSelection) -> String {
        match selection {
            BuiltinAxisSelection::Exact(axes) => {
                inline_names(axes.iter().map(|axis| format!("{axis:?}")))
            }
            BuiltinAxisSelection::ProfilePresetRecall => "profile-selected exact axes".into(),
        }
    }
    let mut output = table("Built-in row | Class and exact axes | Applied-state requirement");
    for &(row, class) in rows {
        let kind = match class {
            BuiltinRequestClass::Plain { .. } => "Plain".into(),
            BuiltinRequestClass::Targeted { axes: selection } => {
                format!("Targeted — {}", axes(selection))
            }
            BuiltinRequestClass::AppliedOnly {
                axes: selection, ..
            } => format!(
                "AppliedOnly{} — {}",
                if class.is_stop() { " STOP" } else { "" },
                axes(selection)
            ),
        };
        let effect = match class.state_effect() {
            None => "—".into(),
            Some(AppliedStateEffectRequirement::Set(key)) => format!("Set `{key:?}`"),
            Some(AppliedStateEffectRequirement::Clear(key)) => format!("Clear `{key:?}`"),
            Some(AppliedStateEffectRequirement::Invalidate(key)) => format!("Invalidate `{key:?}`"),
        };
        let _ = writeln!(output, "| `{row:?}` | {kind} | {effect} |");
    }
    output
}

fn write_only_keys(keys: &[StateKey]) -> String {
    inline_names(keys.iter().map(|key| format!("{key:?}"))) + ".\n"
}

fn assert_blocks(blocks: &[(&str, &str, String)]) {
    // Bless every requested block before propagating the helper's intentional
    // failure; otherwise a multi-block page would need one invocation per block.
    let mut first_failure = None;
    for (path, block, generated) in blocks {
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join(path);
        if let Err(failure) = std::panic::catch_unwind(|| {
            bless_or_assert(path, Some(block), generated);
        }) {
            first_failure.get_or_insert(failure);
        }
    }
    if let Some(failure) = first_failure {
        std::panic::resume_unwind(failure);
    }
}

#[test]
fn generated_documents_readme() {
    let docs = Documentation::current();
    assert_blocks(&[
        (
            "README.md",
            "PROFILE SUPPORT SURFACES",
            docs.readme_surfaces(),
        ),
        ("README.md", "PROFILE NOUN COVERAGE", docs.readme_nouns()),
    ]);
}

#[test]
fn generated_documents_profile_guide() {
    let docs = Documentation::current();
    assert_blocks(&[
        (
            "docs/camera_profile_support.md",
            "GENERATED TYPED SUPPORT VOCABULARY",
            docs.vocabulary(),
        ),
        (
            "docs/camera_profile_support.md",
            "GENERATED TYPED METHODS",
            docs.method_support(),
        ),
        (
            "docs/camera_profile_support.md",
            "GENERATED PROFILE TRANSPORTS",
            docs.transports(),
        ),
        (
            "docs/camera_profile_support.md",
            "GENERATED PROFILE MARKERS",
            docs.profile_markers(),
        ),
        (
            "docs/camera_profile_support.md",
            "GENERATED FOCUS ZONES",
            docs.focus_zones(),
        ),
    ]);
}

#[test]
fn generated_documents_architecture() {
    let docs = Documentation::current();
    let keys = inline_names(StateKey::ALL.iter().map(|key| format!("{key:?}"))) + ".\n";
    assert_blocks(&[
        (
            "docs/architecture_inventory.md",
            "BUILTIN PROFILES",
            docs.transports(),
        ),
        (
            "docs/architecture_inventory.md",
            "OPTIONAL TYPED GATES",
            docs.typed_gates(),
        ),
        (
            "docs/architecture_inventory.md",
            "STATIC NOUNS",
            docs.nouns(),
        ),
        (
            "docs/architecture_inventory.md",
            "STABLE TEST UTILITIES",
            test_utilities(),
        ),
        (
            "docs/architecture_2_0.md",
            "NOUN ENTRY POINTS",
            docs.noun_entry_points(),
        ),
        (
            "docs/architecture_2_0.md",
            "PROFILE TIMINGS",
            docs.timings(),
        ),
        ("docs/architecture_2_0.md", "WRITE-ONLY STATE KEYS", keys),
    ]);
}

#[test]
fn generated_documents_grant_key_index() {
    assert_blocks(&[(
        "docs/visca_reference.md",
        "TYPED GRANT KEY INDEX",
        Documentation::current().grant_index(),
    )]);
}

#[test]
fn generated_documents_request_semantics() {
    assert_blocks(&[
        (
            "docs/request_semantics.md",
            "LEDGER DECISIONS",
            ledger_decisions(&ledger_rows()),
        ),
        (
            "docs/request_semantics.md",
            "WRITE-ONLY STATE KEYS",
            write_only_keys(StateKey::ALL),
        ),
    ]);
}

#[test]
fn stale_request_semantics_detect_changed_ledger_effects_and_keys() {
    let original = ledger_rows();
    let mut changed = original.clone();
    let row = changed
        .iter_mut()
        .find(|(row, _)| *row == BuiltinCommand::TallyFlash)
        .expect("tally row exists");
    row.1 = BuiltinRequestClass::Plain {
        state_effect: Some(AppliedStateEffectRequirement::Invalidate(
            StateKey::TallyMode,
        )),
    };
    assert_stale(&ledger_decisions(&original), &ledger_decisions(&changed));
    assert_stale(
        &write_only_keys(StateKey::ALL),
        &write_only_keys(&StateKey::ALL[1..]),
    );
}

fn edited_spec(
    base: &ProfileSpec,
    mut capabilities: crate::capabilities::Capabilities,
    transports: crate::profiles::TransportCompatibility,
    timing: crate::profiles::ProfileTiming,
) -> ProfileSpec {
    capabilities.profile_id = None;
    let grants = capabilities.typed_support;
    let mut builder = ProfileSpec::builder(capabilities)
        .transports(transports)
        .envelope(base.envelope())
        .timing(timing)
        .maximum_command_sockets(base.maximum_command_sockets())
        .supports_operation_complete(base.supports_operation_complete())
        .supports_command_cancel(base.supports_command_cancel())
        .position_inquiries(base.position_inquiries());
    for selection in base
        .encodings()
        .iter()
        .filter(|selection| grants.contains(selection.surface()))
    {
        builder = builder.encoding(selection);
    }
    builder
        .build()
        .expect("mutated facts remain a valid community profile")
}

fn assert_stale(before: &str, after: &str) {
    use std::sync::atomic::{AtomicU64, Ordering};
    static NEXT: AtomicU64 = AtomicU64::new(0);
    assert_ne!(
        before, after,
        "the changed authority must alter its projection"
    );
    let path = std::env::temp_dir().join(format!(
        "grafton-visca-stale-doc-{}-{}.md",
        std::process::id(),
        NEXT.fetch_add(1, Ordering::Relaxed),
    ));
    let original = format!(
        "Authored before.\n<!-- BEGIN FIXTURE -->\n{before}<!-- END FIXTURE -->\nAuthored after.\n"
    );
    std::fs::write(&path, &original).expect("isolated stale-document fixture");
    let rejected = std::panic::catch_unwind(|| bless_or_assert(&path, Some("FIXTURE"), after));
    let actual = std::fs::read_to_string(&path).expect("fixture remains readable");
    std::fs::remove_file(&path).expect("remove fixture");
    assert!(
        rejected.is_err(),
        "a stale generated block must fail comparison"
    );
    assert_eq!(
        actual, original,
        "ordinary comparison must preserve the entire document"
    );
}

#[test]
fn stale_support_docs_detect_a_changed_profile_grant() {
    let original = Documentation::current();
    let mut changed = original.clone();
    let profile = changed
        .profiles
        .iter_mut()
        .find(|profile| profile.id == ProfileId::PtzOpticsG2)
        .expect("registered G2");
    let mut capabilities = profile.spec.capabilities().clone();
    capabilities.typed_support = capabilities
        .typed_support
        .without(TypedSupportSurface::FocusLock);
    profile.spec = edited_spec(
        &profile.spec,
        capabilities,
        profile.spec.transports(),
        profile.spec.timing(),
    );
    assert_stale(&original.readme_surfaces(), &changed.readme_surfaces());
    assert_stale(&original.method_support(), &changed.method_support());
    assert_eq!(original.profiles.len(), changed.profiles.len());
    assert_eq!(original.methods.len(), changed.methods.len());
}

#[test]
fn stale_architecture_docs_detect_changed_profile_facts() {
    let original = Documentation::current();
    let mut changed = original.clone();
    let profile = changed
        .profiles
        .iter_mut()
        .find(|profile| profile.id == ProfileId::PtzOpticsG2)
        .expect("registered G2");
    let mut timing = profile.spec.timing();
    timing.ack_timeout += std::time::Duration::from_millis(1);
    profile.spec = edited_spec(
        &profile.spec,
        profile.spec.capabilities().clone(),
        profile.spec.transports().with_udp(11259),
        timing,
    );
    assert_stale(&original.transports(), &changed.transports());
    assert_stale(&original.timings(), &changed.timings());
    assert_eq!(original.profiles.len(), changed.profiles.len());
}

#[test]
fn stale_method_docs_detect_changed_noun_metadata() {
    let original = Documentation::current();
    let mut changed = original.clone();
    let row = changed
        .methods
        .iter_mut()
        .find(|row| row.noun == "focus" && row.method == "set_lock")
        .expect("real focus-lock row");
    row.gate = crate::command::surface::RowGate::Always;
    row.gate_markers = &[];
    row.return_type = "ChangedPayload";
    assert_stale(&original.method_support(), &changed.method_support());
    assert_eq!(original.methods.len(), changed.methods.len());
}

#[test]
fn stale_grant_index_detects_changed_evidence_metadata() {
    let original = Documentation::current();
    let mut changed = original.clone();
    changed
        .grants
        .first_mut()
        .expect("sourced registry grant")
        .key = "mutation-only-evidence-key";
    assert_stale(&original.grant_index(), &changed.grant_index());
    assert_eq!(original.grants.len(), changed.grants.len());
}
