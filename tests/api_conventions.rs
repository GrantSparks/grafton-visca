//! Private unit checks of the generated API contract.
//!
//! The library includes this file and invokes `api_convention_tests!`; keeping
//! the test definitions inside that macro grants access to private metadata
//! without introducing a public testing API. Its standalone Cargo integration
//! target contains no executed tests; the checks run in the library unit suite.

#![allow(unused_macros, unused_imports)]

macro_rules! api_convention_tests {
    () => {
        #[cfg(test)]
        mod api_conventions {
            use crate::command::surface::RowGate;
            use crate::noun_metadata::{rows, NounKind, NounRow};
            use std::collections::BTreeSet;

            fn camel_case(name: &str) -> String {
                name.split('_')
                    .map(|part| {
                        let mut characters = part.chars();
                        let first = characters.next().expect("nonempty snake-case segment");
                        first.to_uppercase().chain(characters).collect::<String>()
                    })
                    .collect()
            }

            fn canonical_type(ty: &str) -> String {
                let compact: String = ty.chars().filter(|character| !character.is_whitespace()).collect();
                compact
                    .replace("crate::", "")
                    .replace("types::", "")
                    .replace("std::time::Duration", "Duration")
            }

            fn find(noun: &str, method: &str) -> &'static NounRow {
                rows().iter().find(|row| row.noun == noun && row.method == method)
                    .unwrap_or_else(|| panic!("missing {noun}().{method}()"))
            }

            // These are actions, not open-ended exceptions to the property
            // grammar. Their allowed nouns pin their meaning across domains.
            fn action_nouns(method: &str) -> Option<&'static [&'static str]> {
                Some(match method {
                    "on" | "standby" => &["power"],
                    "tele" | "wide" | "tele_variable" | "wide_variable"
                        | "tele_high_resolution" | "wide_high_resolution" | "stop_high_resolution" => &["zoom"],
                    "stop" => &["zoom", "pan_tilt", "focus"],
                    "save_settings" => &["system"],
                    "home" | "reset" | "drive" | "absolute" | "relative" | "clear_limit" => &["pan_tilt"],
                    "far" | "near" | "far_variable" | "near_variable" | "infinity" | "snap"
                        | "push_auto_focus_press" | "push_auto_focus_release" => &["focus"],
                    "one_push" => &["focus", "white_balance"],
                    "recall" | "store" | "clear" => &["presets"],
                    "navigate" | "enter" | "back" | "direct" => &["menu"],
                    "stop_all" | "is_moving" | "wait_until_idle" => &["motion"],
                    _ => return None,
                })
            }

            fn is_single_step(method: &str) -> bool {
                ["_up", "_down", "_reset"].iter().any(|suffix| method.ends_with(suffix))
            }

            fn is_counted_step(method: &str) -> bool {
                method.ends_with("_up_by") || method.ends_with("_down_by")
            }

            fn property_argument<'a>(setter: &'a NounRow, getter: Option<&NounRow>) -> &'a crate::noun_metadata::NounArgument {
                let values: Vec<_> = setter.arguments.iter().filter(|argument| {
                    !getter.is_some_and(|getter| getter.arguments.iter().any(|key| {
                        key.name == argument.name && canonical_type(key.ty) == canonical_type(argument.ty)
                    }))
                }).collect();
                assert_eq!(values.len(), 1, "{}().{} must take one property value besides its inquiry selectors", setter.noun, setter.method);
                values[0]
            }

            #[test]
            fn closed_surface_has_249_unique_methods_and_one_naming_grammar() {
                let rows = rows();
                assert_eq!(rows.len(), 249, "Appendix A's complete surface changed");
                assert_eq!(rows.iter().filter(|row| row.kind == NounKind::Motion).count(), 3);
                let mut identities = BTreeSet::new();
                for row in rows {
                    assert!(identities.insert((row.noun, row.method)), "duplicate {row:?}");
                    assert!(row.method.split('_').all(|part| !part.is_empty() && part.chars().all(|character| character.is_ascii_lowercase() || character.is_ascii_digit())), "not snake_case: {row:?}");
                    for prefix in ["get_", "enable_", "disable_", "increase_", "decrease_"] {
                        assert!(!row.method.starts_with(prefix), "forbidden naming form: {row:?}");
                    }
                    for suffix in ["_on", "_off", "_direct", "_status", "_set"] {
                        assert!(!row.method.ends_with(suffix), "forbidden naming form: {row:?}");
                    }
                    if row.kind == NounKind::Inquiry {
                        continue;
                    }
                    if let Some(property) = row.method.strip_prefix("set_") {
                        assert!(!property.is_empty());
                        assert!(!row.arguments.is_empty(), "property setter without a value: {row:?}");
                    } else if let Some(property) = row.method.strip_prefix("toggle_") {
                        assert!(!property.is_empty());
                        assert!(row.arguments.is_empty(), "a toggle takes no guessed state: {row:?}");
                    } else if is_counted_step(row.method) {
                        assert_eq!(row.arguments.len(), 1, "counted step: {row:?}");
                        assert_eq!(row.arguments[0].name, "steps", "counted step: {row:?}");
                    } else if is_single_step(row.method) {
                        assert!(row.arguments.is_empty(), "single step: {row:?}");
                    } else {
                        let allowed = action_nouns(row.method).unwrap_or_else(|| panic!("unreviewed action verb: {row:?}"));
                        assert!(allowed.contains(&row.noun), "action meaning drifted across nouns: {row:?}");
                    }
                }
                // The overload-prone names have one contract in the final API.
                assert!(rows.iter().filter(|row| row.method == "preset").all(|row| row.noun == "nd_filter" && row.kind == NounKind::Inquiry));
                assert!(rows.iter().filter(|row| row.method == "one_push").all(|row| matches!(row.kind, NounKind::Plain | NounKind::Applied)));
            }

            #[test]
            fn request_names_follow_method_stems_and_concepts_have_no_vendor_tokens() {
                for row in rows() {
                    let public_tokens = std::iter::once(row.method)
                        .chain(std::iter::once(row.request_type))
                        .chain(row.arguments.iter().map(|argument| argument.ty))
                        .chain(row.gate_markers.iter().copied());
                    for token in public_tokens {
                        let lowered = token.to_ascii_lowercase();
                        for vendor in ["sony", "fr7", "ptzoptics", "ptz_optics", "nearus", "brc", "evih"] {
                            assert!(!lowered.contains(vendor), "vendor token outside profile/encoder names: {row:?} token={token}");
                        }
                    }
                    if row.kind == NounKind::Motion {
                        assert!(row.request_type.is_empty());
                        continue;
                    }
                    let mut expected = camel_case(row.method);
                    if row.kind == NounKind::Inquiry { expected.push_str("Inquiry"); }
                    let actual = row.request_type.replace(' ', "");
                    assert_eq!(actual.rsplit("::").next(), Some(expected.as_str()), "request name does not derive from method: {row:?}");
                    assert_eq!(row.command, Some(expected.as_str()), "table key differs from public request name: {row:?}");
                    assert!(actual.contains(&format!("command::{}::", row.noun)), "request is outside its noun: {row:?}");
                }
            }

            #[test]
            fn readable_properties_and_setters_share_the_same_value_type() {
                let mut checked = 0;
                for setter in rows().iter().filter(|row| row.method.starts_with("set_")) {
                    let property = setter.method.strip_prefix("set_").expect("setter");
                    let getter = rows().iter().find(|row| row.noun == setter.noun && row.method == property && row.kind == NounKind::Inquiry);
                    if let Some(getter) = getter {
                        let value = property_argument(setter, Some(getter));
                        assert_eq!(canonical_type(value.ty), canonical_type(getter.return_type), "read-modify-write type mismatch: {getter:?} / {setter:?}");
                        checked += 1;
                    }
                }
                assert!(checked >= 40, "property matching accidentally stopped checking the readable surface: {checked}");
                let position = find("pan_tilt", "position");
                for method in ["absolute", "relative", "set_limit"] {
                    let movement = find("pan_tilt", method);
                    let argument = movement.arguments.iter().find(|argument| matches!(argument.name, "position" | "offset")).expect("physical pan/tilt position argument");
                    assert_eq!(canonical_type(argument.ty), canonical_type(position.return_type), "degree position round-trip: {movement:?}");
                }
                let normalized = find("zoom", "normalized");
                let setter = find("zoom", "set_normalized");
                assert_eq!(canonical_type(property_argument(setter, Some(normalized)).ty), canonical_type(normalized.return_type));
            }

            #[test]
            fn cache_keys_and_actual_cache_value_types_match_the_noun_setters() {
                for key in crate::state_cache::StateKey::ALL {
                    let noun = key.noun_name();
                    let property = key.property_name();
                    assert_eq!(format!("{key:?}"), format!("{}{}", camel_case(noun), camel_case(property)), "cache key no longer mirrors its noun/property");
                    if noun == "pan_tilt" && property == "limit" {
                        // Set and clear share a corner-local aggregate rather
                        // than a single scalar property value.
                        assert_eq!(canonical_type(key.value_type_name()), "PanTiltLimitUpdate");
                        assert_eq!(find(noun, "set_limit").arguments.len(), 2);
                        assert_eq!(find(noun, "clear_limit").arguments.len(), 1);
                        continue;
                    }
                    let setter_name = format!("set_{property}");
                    let setter = find(noun, &setter_name);
                    assert_eq!(canonical_type(property_argument(setter, None).ty), canonical_type(key.value_type_name()), "cache/setter type mismatch for {key:?}");
                }
            }

            #[test]
            fn runtime_gates_retain_every_static_header_and_refinement_marker() {
                for row in rows() {
                    let mut runtime = BTreeSet::new();
                    match row.gate {
                        RowGate::Always => {},
                        RowGate::Domain(domain) => { runtime.insert(format!("Has{domain:?}")); },
                        RowGate::Typed(surfaces) => { runtime.extend(surfaces.iter().map(|surface| format!("Has{surface:?}"))); },
                        RowGate::DomainAndTyped(domain, surfaces) => {
                            runtime.insert(format!("Has{domain:?}"));
                            runtime.extend(surfaces.iter().map(|surface| format!("Has{surface:?}")));
                        },
                    }
                    let static_markers: BTreeSet<_> = row.gate_markers.iter().map(|marker| (*marker).to_owned()).collect();
                    assert_eq!(runtime, static_markers, "static/runtime gate lost a marker: {row:?}");
                }
            }

            #[test]
            fn authored_docs_are_specific_and_completion_docs_match_the_return_shape() {
                for row in rows() {
                    let authored = row.docs.trim();
                    assert!(!authored.is_empty(), "undocumented method: {row:?}");
                    let mut shallow = authored.trim_end_matches('.').to_ascii_lowercase();
                    if let Some(without_source) = shallow.strip_suffix(" (r7)") { shallow = without_source.to_owned(); }
                    for prefix in ["reads sony fr7 ", "sony fr7 ", "reads ", "returns "] {
                        if let Some(body) = shallow.strip_prefix(prefix) { shallow = body.to_owned(); break; }
                    }
                    assert_ne!(shallow, row.method.replace('_', " "), "humanized-method placeholder documentation: {row:?}");
                    let completion = row.completion_doc;
                    match row.kind {
                        NounKind::Inquiry => assert!(completion.contains("decoded value")),
                        NounKind::Plain => assert!(completion.contains("protocol-completed")),
                        NounKind::Applied => assert!(completion.contains("applied-only operation handle") && completion.contains("does not stop motion")),
                        NounKind::Targeted => assert!(completion.contains("targeted operation handle") && completion.contains("settled()") && completion.contains("does not stop motion")),
                        NounKind::Motion => assert!(completion.contains("owner") && completion.contains("result")),
                    }
                }
            }
        }
    };
}

pub(crate) use api_convention_tests;
