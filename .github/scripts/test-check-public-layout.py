#!/usr/bin/env python3
"""Mutation contracts for the actual rustdoc module/use export graph."""

from __future__ import annotations

import copy
import importlib.util
import json
import pathlib
import subprocess
import sys
import tempfile
import unittest


sys.dont_write_bytecode = True
SCRIPT = pathlib.Path(__file__).with_name("check-public-layout.py")
SPEC = importlib.util.spec_from_file_location("public_layout", SCRIPT)
LAYOUT = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(LAYOUT)


class Crate:
    def __init__(self):
        self.document = {"format_version": 61, "includes_private": True, "root": 0, "index": {}, "paths": {}}
        self.next_id = 0
        self.root = self.module("visca", parent=None)

    def item(self, name, inner, parent=None, visibility="public", attrs=()):
        key = self.next_id
        self.next_id += 1
        self.document["index"][str(key)] = {"id": key, "crate_id": 0, "name": name, "visibility": visibility, "attrs": list(attrs), "inner": inner}
        if parent is not None:
            self.document["index"][str(parent)]["inner"]["module"]["items"].append(key)
        return key

    def module(self, name, parent=0, **kwargs):
        return self.item(name, {"module": {"is_crate": parent is None, "items": [], "is_stripped": False}}, parent, **kwargs)

    def struct(self, name, parent, **kwargs):
        return self.item(name, {"struct": {"kind": "unit", "impls": []}}, parent, **kwargs)

    def use(self, name, target, parent, glob=False, **kwargs):
        return self.item(None, {"use": {"source": f"source::{name}", "name": name, "id": target, "is_glob": glob}}, parent, **kwargs)

    def enum(self, name, parent, variants=()):
        key = self.item(name, {"enum": {"variants": [], "impls": []}}, parent)
        for variant in variants:
            child = self.item(variant, {"variant": {"kind": "plain", "discriminant": None}}, visibility="default")
            self.document["index"][str(key)]["inner"]["enum"]["variants"].append(child)
        return key

    def check(self):
        return LAYOUT.audit(self.document, "mutation")


def valid_crate():
    crate = Crate()
    crate.types = crate.module("types")
    crate.units = crate.module("units", visibility="default")
    crate.level = crate.struct("Level", crate.units)
    crate.use("Level", crate.level, crate.types)
    # This single canonical summary intentionally omits every alias path.
    crate.document["paths"][str(crate.level)] = {"crate_id": 0, "path": ["visca", "units", "Level"], "kind": "struct"}
    crate.error_module = crate.module("error")
    crate.error = crate.enum("Error", crate.error_module, ("SyntaxError", "Unknown"))
    crate.result = crate.item("Result", {"type_alias": {"type": {"primitive": "u8"}}}, crate.error_module)
    crate.root_error_use = crate.use("Error", crate.error, crate.root)
    crate.use("Result", crate.result, crate.root)
    crate.prelude = crate.module("prelude")
    crate.use("Level", crate.level, crate.prelude)
    crate.hidden = crate.module("__macro_support", attrs=({"other": "#[doc(hidden)]"},))
    crate.use("Level", crate.level, crate.hidden)
    return crate


class ExportGraphTests(unittest.TestCase):
    def test_private_origins_preludes_and_explicit_mandated_dual_homes(self):
        crate = valid_crate()
        report = crate.check()
        self.assertTrue(report["ok"], report)
        self.assertEqual(report["definitions"][str(crate.level)]["paths"], ["visca::types::Level"])
        self.assertEqual(report["counts"]["mandated_dual_home_roots"], 2)
        self.assertEqual(report["counts"]["mandated_dual_home_descendants"], 2)
        self.assertEqual(report["counts"]["repeated_definitions_before_mandated_exceptions"], 4)
        self.assertGreater(report["counts"]["excluded_prelude_paths"], 0)
        self.assertGreater(report["counts"]["excluded_doc_hidden_edges"], 0)

    def test_root_alias_is_detected_even_when_paths_has_one_origin(self):
        crate = valid_crate()
        crate.use("RootAlias", crate.level, crate.root)
        report = crate.check()
        duplicate = next(entry for entry in report["duplicates"] if entry["id"] == str(crate.level))
        self.assertEqual(duplicate["paths"], ["visca::RootAlias", "visca::types::Level"])
        self.assertTrue(any(entry["code"] == "wrong_root" for entry in report["violations"]))
        self.assertEqual(len(crate.document["paths"]), 1)

    def test_glob_exports_private_module_contents_at_the_public_route(self):
        crate = valid_crate()
        config = crate.module("config")
        crate.use("*", crate.units, config, glob=True)
        report = crate.check()
        self.assertEqual(next(entry for entry in report["duplicates"] if entry["id"] == str(crate.level))["paths"], ["visca::config::Level", "visca::types::Level"])
        self.assertTrue(any(entry["code"] == "wrong_home" and entry["expected_home"] == "types" for entry in report["violations"]))

    def test_single_wrong_home_is_detected_without_any_duplicate(self):
        crate = valid_crate()
        crate.document["index"][str(crate.types)]["inner"]["module"]["items"].clear()
        config = crate.module("config")
        crate.use("Level", crate.level, config)
        report = crate.check()
        self.assertFalse(report["duplicates"])
        self.assertTrue(any(entry["code"] == "wrong_home" and entry["id"] == str(crate.level) for entry in report["violations"]))

    def test_explicit_and_private_names_shadow_glob_exports(self):
        for visibility in ("public", "default"):
            crate = valid_crate()
            config = crate.module("config")
            crate.use("*", crate.units, config, glob=True)
            crate.struct("Level", config, visibility=visibility)
            report = crate.check()
            self.assertTrue(report["ok"], report)
            self.assertEqual(report["definitions"][str(crate.level)]["paths"], ["visca::types::Level"])

    def test_shadowing_respects_type_and_value_namespaces(self):
        crate = valid_crate()
        config = crate.module("config")
        crate.use("*", crate.units, config, glob=True)
        crate.item("Level", {"function": {}}, config)
        self.assertTrue(any(entry["id"] == str(crate.level) for entry in crate.check()["duplicates"]))

    def test_a_braced_struct_has_no_independent_constructor_namespace(self):
        crate = valid_crate()
        crate.document["index"][str(crate.level)]["inner"]["struct"]["kind"] = {"plain": {"fields": [], "has_stripped_fields": False}}
        config = crate.module("config")
        crate.use("*", crate.units, config, glob=True)
        crate.item("Level", {"type_alias": {"type": {"primitive": "u8"}}}, config)
        self.assertTrue(crate.check()["ok"])

    def test_module_alias_walks_definitions_below_a_nonpublic_parent(self):
        crate = Crate()
        private = crate.module("implementation", visibility="default")
        module = crate.module("public_inner", private)
        value = crate.struct("Level", module)
        crate.use("types", module, crate.root)
        crate.use("config", module, crate.root)
        report = crate.check()
        duplicates = {entry["id"]: entry["paths"] for entry in report["duplicates"]}
        self.assertEqual(duplicates[str(value)], ["visca::config::Level", "visca::types::Level"])
        self.assertEqual(duplicates[str(module)], ["visca::config", "visca::types"])
        self.assertFalse(any("implementation" in path for entry in report["definitions"].values() for path in entry["paths"]))

    def test_module_alias_cycle_is_finite_and_does_not_hide_duplicates(self):
        crate = valid_crate()
        crate.use("again", crate.types, crate.types)
        report = crate.check()
        self.assertTrue(report["cycle_truncations"])
        self.assertTrue(any(entry["id"] == str(crate.types) for entry in report["duplicates"]))
        self.assertLess(report["counts"]["supported_paths"], 100)

    def test_glob_cycles_terminate_and_keep_reachable_names(self):
        crate = valid_crate()
        config = crate.module("config")
        crate.use("*", config, crate.types, glob=True)
        crate.use("*", crate.types, config, glob=True)
        report = crate.check()
        self.assertTrue(report["cycle_truncations"])
        self.assertEqual(len(next(entry for entry in report["duplicates"] if entry["id"] == str(crate.level))["paths"]), 2)

    def test_hidden_module_name_is_not_itself_an_exception(self):
        crate = valid_crate()
        crate.document["index"][str(crate.hidden)]["attrs"] = []
        report = crate.check()
        self.assertFalse(report["ok"])
        self.assertTrue(any(entry["path"].startswith("visca::__macro_support") for entry in report["violations"]))

    def test_quoted_hidden_text_is_not_a_hidden_attribute(self):
        for attribute in ['#[doc = "#[doc(hidden)]"]', '#[doc(alias = "a, hidden, b")]', '#[doc(cfg(any(hidden)))]', '#[doc(hidden = "false")]']:
            self.assertFalse(LAYOUT.hidden_attribute([{"other": attribute}]), attribute)
        self.assertTrue(LAYOUT.hidden_attribute([{"other": '#[doc(alias = "hidden", hidden)]'}]))

    def test_glob_does_not_inherit_hidden_from_its_source_module(self):
        crate = valid_crate()
        config = crate.module("config")
        crate.use("*", crate.hidden, config, glob=True)
        self.assertTrue(any(entry["id"] == str(crate.level) for entry in crate.check()["duplicates"]))

    def test_fake_error_alias_and_third_real_error_home_are_not_waived(self):
        crate = valid_crate()
        crate.document["index"][str(crate.root)]["inner"]["module"]["items"].remove(crate.root_error_use)
        crate.use("Error", crate.level, crate.root)
        report = crate.check()
        self.assertTrue(any(entry["code"] == "wrong_root" and entry["id"] == str(crate.level) for entry in report["violations"]))
        crate = valid_crate()
        crate.use("ErrorAgain", crate.error, crate.error_module)
        report = crate.check()
        self.assertTrue(any(entry["id"] == str(crate.error) and len(entry["paths"]) == 3 for entry in report["duplicates"]))
        self.assertGreaterEqual(report["counts"]["mandated_dual_home_descendants"], 0)

    def test_required_root_proc_macros_resolve_external_definition_ids(self):
        crate = valid_crate()
        for index, (name, kind) in enumerate(LAYOUT.ROOT_MACROS.items(), 1000):
            crate.document["paths"][str(index)] = {"crate_id": 1, "path": ["grafton_visca_macros", name], "kind": kind}
            crate.use(name, index, crate.root)
        self.assertTrue(crate.check()["ok"])
        crate.document["paths"]["1000"]["path"][0] = "unrelated_macro_package"
        self.assertFalse(crate.check()["ok"])

    def test_distinct_runtime_type_alias_ids_are_not_collapsed(self):
        crate = valid_crate()
        transport = crate.module("transport")
        for name in ("tokio", "smol"):
            module = crate.module(name, transport)
            crate.item("TcpTransport", {"type_alias": {"type": {"resolved_path": {"id": 999, "path": "Tcp", "args": {"runtime": name}}}}}, module)
        self.assertTrue(crate.check()["ok"])

    def test_capability_markers_with_value_names_have_distinct_homes(self):
        crate = valid_crate()
        capabilities = crate.module("capabilities")
        gate = crate.module("gate", capabilities)
        marker = crate.struct("FocusZone", gate)
        value = crate.struct("FocusZone", crate.units)
        crate.use("FocusZone", value, crate.types)
        report = crate.check()
        self.assertTrue(report["ok"], report)
        self.assertEqual(report["definitions"][str(marker)]["expected_home"], "capabilities")
        self.assertEqual(report["definitions"][str(value)]["expected_home"], "types")

    def test_required_error_root_cannot_disappear_under_an_exception(self):
        crate = valid_crate()
        crate.document["index"][str(crate.root)]["inner"]["module"]["items"].remove(crate.root_error_use)
        report = crate.check()
        self.assertTrue(any(entry["code"] == "missing_mandated_home" and entry["path"] == "visca::Error" for entry in report["violations"]))

    def test_timeout_metadata_and_architecture_documentation_have_valid_homes(self):
        crate = valid_crate()
        profiles = crate.module("profiles")
        crate.enum("CommandTimeoutClass", profiles, ("Quick", "Movement"))
        crate.module("architecture")
        self.assertTrue(crate.check()["ok"])

    def test_class_policies_have_one_canonical_request_home(self):
        crate = valid_crate()
        request = crate.module("request")
        crate.enum("CommandRetryClass", request, ("Never", "Standard", "Movement", "Preset"))
        crate.enum("InquiryRetryClass", request, ("Never", "Inquiry"))
        policies = [crate.struct(name, request) for name in ("CommandPolicy", "OperationPolicy", "InquiryPolicy")]
        self.assertTrue(crate.check()["ok"])

        raw = crate.module("raw")
        for policy in policies:
            name = crate.document["index"][str(policy)]["name"]
            crate.use(name, policy, raw)
        report = crate.check()
        misplaced = {entry["path"] for entry in report["violations"] if entry["code"] == "wrong_home"}
        self.assertTrue({f"visca::raw::{name}" for name in ("CommandPolicy", "OperationPolicy", "InquiryPolicy")} <= misplaced)

    def test_flat_policy_enums_stay_private_even_through_renamed_exports(self):
        crate = valid_crate()
        internal = crate.module("requests", visibility="default")
        retry = crate.enum("RetryClass", internal)
        timeout = crate.enum("TimeoutClass", internal)
        self.assertTrue(crate.check()["ok"], "private lowering enums are permitted")

        request = crate.module("request")
        crate.use("LegacyRetry", retry, request)
        crate.use("LegacyTimeout", timeout, crate.prelude)
        report = crate.check()
        removed = {entry["path"] for entry in report["violations"] if entry["code"] == "removed_public_item"}
        self.assertTrue({"visca::request::LegacyRetry", "visca::prelude::LegacyTimeout"} <= removed)

    def test_removed_modules_family_enums_and_fr7_requests_fail(self):
        crate = valid_crate()
        crate.module("camera")
        command = crate.module("command")
        exposure = crate.module("exposure", command)
        crate.enum("Gain", exposure)
        crate.struct("Fr7SetGain", exposure)
        request = crate.module("request")
        crate.module("builtin", request)
        report = crate.check()
        removed = {entry["path"] for entry in report["violations"] if entry["code"] == "removed_public_item"}
        self.assertTrue({"visca::camera", "visca::command::exposure::Gain", "visca::command::exposure::Fr7SetGain", "visca::request::builtin"} <= removed)

    def test_uninspectable_globs_and_wrong_schema_fail_closed(self):
        crate = valid_crate()
        crate.document["paths"]["1000"] = {"crate_id": 1, "path": ["external", "module"], "kind": "module"}
        crate.use("*", 1000, crate.types, glob=True)
        with self.assertRaises(LAYOUT.LayoutError):
            crate.check()
        document = copy.deepcopy(valid_crate().document)
        document["includes_private"] = False
        with self.assertRaises(LAYOUT.LayoutError):
            LAYOUT.audit(document)

    def test_cli_writes_full_graph_and_reports_exceptions_on_failure(self):
        crate = valid_crate()
        crate.use("RootAlias", crate.level, crate.root)
        with tempfile.TemporaryDirectory() as directory:
            source, report = pathlib.Path(directory) / "input.json", pathlib.Path(directory) / "report.json"
            source.write_text(json.dumps(crate.document), encoding="utf-8")
            result = subprocess.run([sys.executable, str(SCRIPT), str(source), "--surface", "fixture", "--report", str(report)], text=True, capture_output=True, check=False)
            self.assertEqual(result.returncode, 1, result.stderr)
            self.assertIn("1 unexpected duplicate", result.stdout)
            self.assertIn("mandated Error/Result", result.stdout)
            self.assertEqual(json.loads(report.read_text())["definitions"][str(crate.level)]["paths"], ["visca::RootAlias", "visca::types::Level"])


if __name__ == "__main__":
    unittest.main()
