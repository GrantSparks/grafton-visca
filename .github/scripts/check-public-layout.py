#!/usr/bin/env python3
"""Audit supported export paths in pinned-nightly rustdoc JSON (format 61).

Generate with nightly-2026-08-26, --document-private-items and
--document-hidden-items. Module membership and use edges determine reachable
paths; rustdoc.paths is only definition metadata, never the export inventory.
Type aliases keep their own definition IDs, including runtime specializations.
"""

from __future__ import annotations

import argparse
import collections
import json
import pathlib
import re
import sys


FORMAT_VERSION = 61
ROOT_MODULES = frozenset("blocking async types command request raw wire config profiles capabilities operation error transport runtime observability state_cache testing prelude".split())
DOCUMENTATION_MODULES = frozenset(("architecture",))
ROOT_MACROS = {"value": "proc_attribute", "value_enum": "proc_attribute", "ViscaInquiry": "proc_derive"}
OLD_MODULES = frozenset("camera units profile session dynapi dyn_api runtime_adapters inquiry_conversions completion".split())
FAMILY_ENUMS = frozenset("Zoom Focus PanTilt Gain Shutter Brightness ColorTemperature RedGain BlueGain ExposureCompensation Sharpness ZoomDrive FocusDrive FocusTrigger FocusModeCommand PushAF PushAf".split())
VALUE_NAMES = frozenset("AffectedAxes AffectedAxis AffectedAxisIter CameraId IndividualCameraId CommandSocketCount ViscaSocket ExposureMode FocusMode WhiteBalanceMode AutoFocusSensitivity AutoWhiteBalanceSensitivity FocusZone AntiFlickerMode ImageFlipMode SharpnessMode NoiseReduction2DMode NoiseReduction2D NoiseReduction3D PictureEffectMode NdFilterMode PowerStatus PanTiltDirection PanTiltLimitCorner MenuDirection DirectMenuTarget MenuButton VersionInfo PresetNumber MotionQuery IdleWait MovementTolerance PanTiltPosition PanTiltUnits ZoomDomain".split())
NAMED_HOMES = {
    **dict.fromkeys(VALUE_NAMES, "types"),
    **dict.fromkeys("Request PlainCommand Inquiry OperationCommand ResponseDecoder PayloadShape ControlClass SubmissionClass CommandRetryClass InquiryRetryClass CommandPolicy OperationPolicy InquiryPolicy Class PlainClass InquiryClass OperationClass".split(), "request"),
    **dict.fromkeys("ProfileSpec ProfileSpecBuilder ProfileTiming ProfileTimingBuilder ProfileEnvelope TransportCompatibility PanTiltCoordinateConversion CompileTimeProfile CameraProfile AnyProfile ProfileSelection ProfileId ProfileGroup CommandTimeouts".split(), "profiles"),
    **dict.fromkeys("CameraConfig SessionConfig OperationalTuning TransportOptions TransportKind TargetRegistration MountPosition DEFAULT_ADMISSION_CAPACITY".split(), "config"),
    **dict.fromkeys("Targeted AppliedOnly Kind CancellationOutcome Settlement HaltOutcome HaltReport OperationId".split(), "operation"),
    **dict.fromkeys("Certainty ErrorKind FailureContext FailureStage Recovery".split(), "error"),
    **dict.fromkeys("Response Payload Nibbles BoolConvention RawInquiryPayload ResponseParser parse_inquiry_payload VISCA_TERMINATOR CommandKind WireValue".split(), "wire"),
    **dict.fromkeys("StateCache StateKey KnownState StateEntry PanTiltLimitUpdate".split(), "state_cache"),
    **dict.fromkeys("MetricsSnapshot SessionStatus".split(), "observability"),
    **dict.fromkeys("Executor ResettableTimer ExecError TokioExecutor TokioTimer SmolExecutor SmolTimer Runtime TokioRuntime SmolRuntime TransportHandle".split(), "runtime"),
    **dict.fromkeys("TransportConfig TcpKeepaliveConfig BufferConfig SerialConfig SerialStartup".split(), "transport"),
    "CommandTimeoutClass": "profiles",
}
DATA_KINDS = frozenset(("struct", "enum", "union", "type_alias"))


class LayoutError(ValueError):
    """Input cannot support a complete export-graph audit."""


def hidden_attribute(attributes: list) -> bool:
    """Recognize the doc(hidden) meta flag, not text quoted inside an attribute."""
    for attribute in attributes:
        if isinstance(attribute, dict):
            if attribute.get("doc_hidden") is True:
                return True
            attribute = attribute.get("other", "")
        if not isinstance(attribute, str):
            continue
        match = re.fullmatch(r"\s*#\s*\[\s*doc\s*\((.*)\)\s*\]\s*", attribute, re.S)
        if not match:
            continue
        body = match.group(1)
        # Mask ordinary/raw string literals before splitting top-level meta.
        body = re.sub(r'r(#+)?".*?"\1|"(?:\\.|[^"\\])*"', '""', body, flags=re.S)
        depth, start = 0, 0
        for index, char in enumerate(body + ","):
            if char in "([{":
                depth += 1
            elif char in ")]}":
                depth -= 1
            elif char == "," and depth == 0:
                if body[start:index].strip() == "hidden":
                    return True
                start = index + 1
    return False


def spelling(name: str) -> str:
    return name.removeprefix("r#")


class Audit:
    """Resolve each supported public route to a definition ID."""

    def __init__(self, document: dict):
        if document.get("format_version") != FORMAT_VERSION:
            raise LayoutError(f"expected rustdoc JSON format {FORMAT_VERSION} from nightly-2026-08-26")
        if document.get("includes_private") is not True:
            raise LayoutError("regenerate with --document-private-items and --document-hidden-items")
        self.index = {str(key): item for key, item in document["index"].items()}
        self.summaries = {str(key): value for key, value in document.get("paths", {}).items()}
        self.root = str(document["root"])
        self.crate = self.index[self.root]["name"]
        self.local_crate = self.index[self.root]["crate_id"]
        self.origins = {}
        self.variant_parent = {}
        self.paths = collections.defaultdict(set)
        self.excluded = {"prelude": set(), "doc_hidden": set()}
        self.cycles = set()
        self.visited = set()
        self.violations = []
        self.canonical_values = set()
        self._declarations(self.root, (), frozenset())
        self.wire_values = set()
        wire_traits = {key for key, item in self.index.items() if item.get("name") == "WireValue" and "trait" in item["inner"] and item["crate_id"] == self.local_crate}
        for key, item in self.index.items():
            detail = next(iter(item["inner"].values()))
            if item["crate_id"] == self.local_crate and isinstance(detail, dict):
                for impl_id in detail.get("impls", []):
                    implementation = self.index.get(str(impl_id), {}).get("inner", {}).get("impl", {})
                    trait = implementation.get("trait") or {}
                    if str(trait.get("id")) in wire_traits:
                        self.wire_values.add(key)

    def describe(self, key: str) -> dict:
        key = str(key)
        if key in self.index:
            item = self.index[key]
            return {"id": key, "name": item.get("name"), "kind": next(iter(item["inner"])), "crate_id": item["crate_id"], "item": item}
        if key in self.summaries:
            summary = self.summaries[key]
            return {"id": key, "name": summary["path"][-1], "kind": summary["kind"], "crate_id": summary["crate_id"], "item": None}
        raise LayoutError(f"reachable definition {key} has neither an index entry nor a path summary")

    def render(self, path: tuple) -> str:
        return "::".join((self.crate, *("r#async" if part == "async" else part for part in path)))

    def _declarations(self, key: str, path: tuple, ancestors: frozenset) -> None:
        if key in ancestors or key not in self.index:
            return
        item = self.index[key]
        if item["crate_id"] != self.local_crate or "use" in item["inner"]:
            return
        self.origins.setdefault(key, path)
        inner = item["inner"]
        if "module" in inner:
            for child in inner["module"]["items"]:
                child = str(child)
                entry = self.index.get(child)
                if entry and entry.get("name"):
                    self._declarations(child, path + (spelling(entry["name"]),), ancestors | {key})
        elif "enum" in inner:
            for variant in inner["enum"]["variants"]:
                variant = str(variant)
                self.variant_parent[variant] = key
                self.origins[variant] = path + (self.index[variant]["name"],)

    def _resolve(self, key: str) -> str:
        seen = set()
        while key in self.index and "use" in self.index[key]["inner"]:
            if key in seen:
                raise LayoutError(f"reexport definition cycle at {key}")
            seen.add(key)
            use = self.index[key]["inner"]["use"]
            if use["is_glob"] or use["id"] is None:
                raise LayoutError(f"cannot resolve reexport target {key}")
            key = str(use["id"])
        return key

    def _binding(self, item: dict) -> set:
        """Name/namespace slots used to apply Rust's explicit-over-glob rule."""
        use = item["inner"].get("use")
        name = use["name"] if use else item.get("name")
        if not name or use and use["is_glob"]:
            return set()
        if use:
            target = self.describe(self._resolve(str(use["id"]))) if use["id"] is not None else None
            kind = target["kind"] if target else None
            definition = target["item"] if target else None
        else:
            kind = next(iter(item["inner"]))
            definition = item
        if kind in ("macro", "proc_attribute", "proc_derive"):
            spaces = ("macro",)
        elif kind in ("function", "constant", "static", "variant"):
            spaces = ("value",)
        elif kind == "struct":
            shape = definition["inner"]["struct"]["kind"] if definition else None
            # Braced structs have no value-namespace constructor binding.
            spaces = ("type",) if isinstance(shape, dict) and "plain" in shape else ("type", "value")
        elif kind is None:
            spaces = ("type", "value", "macro")
        else:
            spaces = ("type",)
        return {(spelling(name), space) for space in spaces}

    def _export(self, key: str, prefix: tuple, ancestors: frozenset, blocked: frozenset = frozenset()) -> None:
        item = self.index.get(key)
        if item is None:
            raise LayoutError(f"module lists missing item {key}")
        if item["visibility"] != "public":
            return
        use = item["inner"].get("use")
        name = use["name"] if use else item.get("name")
        path = prefix if use and use["is_glob"] else prefix + (spelling(name),)
        if hidden_attribute(item.get("attrs", [])):
            self.excluded["doc_hidden"].add((key, path))
            return
        if not (use and use["is_glob"]) and self._binding(item) <= blocked:
            return
        if use:
            if use["id"] is None:
                raise LayoutError(f"public reexport {self.render(path)} has no definition ID")
            target = self._resolve(str(use["id"]))
            if use["is_glob"]:
                self._contents(target, prefix, ancestors, "glob", blocked)
            else:
                self._definition(target, path, ancestors)
        else:
            self._definition(key, path, ancestors)

    def _contents(self, key: str, prefix: tuple, ancestors: frozenset, edge: str, blocked: frozenset = frozenset()) -> None:
        description = self.describe(key)
        item = description["item"]
        if item is None:
            raise LayoutError(f"cannot inspect external {edge} module/enum {self.render(prefix)} ({key})")
        # A public glob exports the children, not its source module. A hidden
        # source module must not hide independently reexported public children.
        if key in ancestors:
            self.cycles.add((key, prefix, edge))
            return
        ancestors = ancestors | {key}
        if "module" in item["inner"]:
            direct = frozenset(binding for child in item["inner"]["module"]["items"] for binding in self._binding(self.index[str(child)]))
            for child in item["inner"]["module"]["items"]:
                entry = self.index[str(child)]
                glob = entry["inner"].get("use", {}).get("is_glob", False)
                self._export(str(child), prefix, ancestors, blocked | direct if glob else blocked)
        elif "enum" in item["inner"]:
            for child in item["inner"]["enum"]["variants"]:
                child = str(child)
                if not self._binding(self.index[child]) <= blocked:
                    self._definition(child, prefix + (self.index[child]["name"],), ancestors)
        else:
            raise LayoutError(f"glob target {key} is not a module or enum")

    def _definition(self, key: str, path: tuple, ancestors: frozenset) -> None:
        description = self.describe(key)
        item = description["item"]
        if item and hidden_attribute(item.get("attrs", [])):
            self.excluded["doc_hidden"].add((key, path))
            return
        if (key, path) in self.visited:
            return
        self.visited.add((key, path))
        if path[:1] == ("prelude",):
            self.excluded["prelude"].add((key, path))
        else:
            self.paths[key].add(path)
        # Prelude routes still receive removal checks, but not home/duplicate checks.
        self._removed(description, path)
        if item and description["kind"] in ("module", "enum"):
            self._contents(key, path, ancestors, "module" if description["kind"] == "module" else "enum")

    def _removed(self, description: dict, path: tuple) -> None:
        name, kind = description["name"] or "", description["kind"]
        origin = self.origins.get(description["id"], ())
        legacy = (kind == "module" and (path[:1] and path[0] in OLD_MODULES or path[-2:] == ("request", "builtin") or "dyn_api" in path))
        legacy |= bool(re.match(r"^(?:Fr7[A-Z_]|Dyn[A-Z]|BlockingDynSessionCamera$)", name))
        legacy |= kind == "enum" and (name in FAMILY_ENUMS or path[:1] == ("command",))
        legacy |= origin[:2] == ("request", "builtin")
        legacy |= path[:1] == ("command",) and name.endswith("Command")
        legacy |= name in ("InquiryData", "InquiryKind", "Connect", "NetTransportBuilder", "BlockingTransportHandle", "RetryClass", "TimeoutClass")
        if legacy:
            self.violations.append({"code": "removed_public_item", "id": description["id"], "path": self.render(path), "name": name})

    def home(self, key: str) -> str | None:
        if key in self.variant_parent:
            return self.home(self.variant_parent[key])
        description = self.describe(key)
        name = description["name"] or ""
        origin = self.origins.get(key, ())
        # Gate marker definitions intentionally share short names with values
        # (e.g. capabilities::gate::FocusZone and types::FocusZone).
        if origin[:1] == ("capabilities",):
            return "capabilities"
        if name in ROOT_MACROS and description["kind"] == ROOT_MACROS[name]:
            return ""
        if name in NAMED_HOMES:
            return NAMED_HOMES[name]
        if key in self.canonical_values or key in self.wire_values or origin[:1] in (("types",), ("units",), ("camera_id",), ("visca_socket",), ("inquiry_conversions",)):
            return "types"
        if origin[:2] == ("camera", "profiles") or origin[:1] == ("profile_selection",):
            return "profiles"
        if origin[:2] == ("camera", "config") or origin[:1] in (("session_config",), ("tuning",), ("target_registration",)):
            return "config"
        if origin[:2] == ("camera", "movement"):
            return "types"
        if origin[:1] in (("completion",), ("outcome",), ("operation_id",)):
            return "operation"
        if origin[:1] in (("async_session",), ("async_operation",), ("async_nouns",)):
            return "async"
        if origin[:1] == ("executor",):
            return "runtime"
        if origin and origin[0] in ROOT_MODULES:
            return origin[0]
        return None

    def _error_roots(self) -> dict:
        roots = {}
        for key in self.paths:
            description = self.describe(key)
            name, kind = description["name"], description["kind"]
            if (name, kind) not in (("Error", "enum"), ("Result", "type_alias")):
                continue
            origin = self.origins.get(key, ())
            if origin[:1] == ("error",) and ("error", name) in self.paths[key]:
                roots[key] = {("error", name), (name,)}
        return roots

    def report(self, surface: str = "unspecified") -> dict:
        self._definition(self.root, (), frozenset())
        self.canonical_values = {key for key, paths in self.paths.items() if self.describe(key)["kind"] in DATA_KINDS and any(path[:1] == ("types",) for path in paths)}
        error_roots = self._error_roots()
        for name, kind in (("Error", "enum"), ("Result", "type_alias")):
            candidates = [key for key in self.paths if self.describe(key)["name"] == name and self.describe(key)["kind"] == kind and self.origins.get(key, ())[:1] == ("error",)]
            for path in ((name,), ("error", name)):
                if not any(path in self.paths[key] for key in candidates):
                    self.violations.append({"code": "missing_mandated_home", "path": self.render(path), "name": name})
        exceptions = dict(error_roots)
        for child, parent in self.variant_parent.items():
            if parent in error_roots and self.describe(parent)["name"] == "Error":
                name = self.describe(child)["name"]
                exceptions[child] = {(name_prefix + (name,)) for name_prefix in error_roots[parent]}
        duplicates, accepted, definitions = [], [], {}
        for key, paths in sorted(self.paths.items()):
            description = self.describe(key)
            allowed = exceptions.get(key, set())
            rendered = sorted(self.render(path) for path in paths)
            definitions[key] = {"name": description["name"], "kind": description["kind"], "paths": rendered, "declaration_path": list(self.origins.get(key, ())), "expected_home": self.home(key)}
            if len(paths) > 1:
                if paths <= allowed:
                    accepted.append({"id": key, "name": description["name"], "paths": rendered})
                else:
                    duplicates.append({"id": key, "name": description["name"], "kind": description["kind"], "paths": rendered})
            for path in sorted(paths):
                if not path:
                    continue
                root_allowed = path[0] in ROOT_MODULES | DOCUMENTATION_MODULES
                if len(path) == 1 and description["kind"] == "module":
                    root_allowed = path[0] in ROOT_MODULES | DOCUMENTATION_MODULES
                elif len(path) == 1:
                    summary = self.summaries.get(key, {})
                    root_allowed = key in error_roots and path in allowed or (path[0] in ROOT_MACROS and description["name"] == path[0] and description["kind"] == ROOT_MACROS[path[0]] and summary.get("path", [None])[0] == "grafton_visca_macros")
                elif path in allowed:
                    root_allowed = True
                if not root_allowed:
                    self.violations.append({"code": "wrong_root", "id": key, "path": self.render(path)})
                home = self.home(key)
                if home is not None and path[0] != home and path not in allowed and not (home == "" and len(path) == 1):
                    self.violations.append({"code": "wrong_home", "id": key, "path": self.render(path), "expected_home": "(crate root)" if home == "" else home})
        return {
            "crate": self.crate, "surface": surface, "format_version": FORMAT_VERSION,
            "ok": not duplicates and not self.violations,
            "counts": {"supported_definitions": len(self.paths), "supported_paths": sum(map(len, self.paths.values())), "repeated_definitions_before_mandated_exceptions": sum(len(paths) > 1 for paths in self.paths.values()), "duplicate_definitions": len(duplicates), "layout_violations": len(self.violations), "excluded_prelude_paths": len(self.excluded["prelude"]), "excluded_doc_hidden_edges": len(self.excluded["doc_hidden"]), "mandated_dual_home_roots": sum(entry["id"] in error_roots for entry in accepted), "mandated_dual_home_descendants": sum(entry["id"] not in error_roots for entry in accepted)},
            "policy_exceptions": ["prelude::* routes are excluded from duplicate/home checks", "only declared #[doc(hidden)] definitions/export edges are hidden", "root Error/Result plus error::* are required dual homes; Error variants inherit them", "architecture is an allowed documentation module; its items still receive duplicate checks", "type alias definition IDs remain distinct; inherent impl members are not module export edges"],
            "duplicates": duplicates, "violations": self.violations,
            "accepted_mandated_dual_homes": accepted,
            "cycle_truncations": [{"id": key, "path": self.render(path), "edge": edge} for key, path, edge in sorted(self.cycles)],
            "definitions": definitions,
        }


def audit(document: dict, surface: str = "unspecified") -> dict:
    return Audit(document).report(surface)


def main(argv: list[str] | None = None) -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("rustdoc_json", type=pathlib.Path)
    parser.add_argument("--surface", default="unspecified")
    parser.add_argument("--report", type=pathlib.Path, help="write full definition-ID/path audit JSON")
    args = parser.parse_args(argv)
    try:
        with args.rustdoc_json.open(encoding="utf-8") as source:
            report = audit(json.load(source), args.surface)
    except (OSError, ValueError, KeyError, TypeError) as error:
        print(f"public layout: cannot audit: {error}", file=sys.stderr)
        return 2
    if args.report:
        args.report.parent.mkdir(parents=True, exist_ok=True)
        args.report.write_text(json.dumps(report, indent=2, sort_keys=True) + "\n", encoding="utf-8")
    counts = report["counts"]
    print(f"public layout {args.surface}: {'PASS' if report['ok'] else 'FAIL'}; {counts['duplicate_definitions']} unexpected duplicate definitions ({counts['repeated_definitions_before_mandated_exceptions']} repeated before mandated exceptions), {counts['layout_violations']} layout violations")
    print(f"  Exclusions: {counts['excluded_prelude_paths']} prelude routes, {counts['excluded_doc_hidden_edges']} declared hidden edges; {counts['mandated_dual_home_roots']} mandated Error/Result roots (and inherited Error variants).")
    for duplicate in report["duplicates"]:
        print(f"  duplicate {duplicate['name']} [{duplicate['id']}]: {', '.join(duplicate['paths'])}")
    for violation in report["violations"]:
        print(f"  {violation['code']}: {violation['path']}" + (f" -> {violation['expected_home']}" if "expected_home" in violation else ""))
    if args.report:
        print(f"  Full export-graph audit: {args.report}")
    return 0 if report["ok"] else 1


if __name__ == "__main__":
    sys.exit(main())
