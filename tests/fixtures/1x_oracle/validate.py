#!/usr/bin/env python3
"""Validate and execute the 1.x behavioral-parity corpus.

This gate is deliberately a provenance check plus a runner for already-owned
v2 tests.  It does not model the protocol or reproduce scheduler logic.  A
row is valid only when its 1.x source blob and symbol, its current v2 source
file and symbol, and the command that runs that v2 target all still exist.
"""

from __future__ import annotations

import argparse
import json
import re
import shlex
import subprocess
import sys
from collections import OrderedDict
from pathlib import Path
from typing import Any


FORMAT = "grafton-visca-1x-behavioral-oracle/v1"
COMMIT_RE = re.compile(r"^[0-9a-f]{40}$")
ID_RE = re.compile(r"^[a-z][a-z0-9._-]+$")
FUNCTION_RE_TEMPLATE = (
    r"(?m)^\s*(?:(?:pub(?:\([^)]*\))?)\s+)?(?:async\s+)?fn\s+{name}"
    r"(?:<[^>\n]*>)?\s*\("
)
PENDING_WORDS = ("pending", "missing", "todo", "tbd")


class ValidationError(Exception):
    """A corpus validation failure that should be reported as one gate error."""


def git(root: Path, *args: str, check: bool = True) -> str:
    result = subprocess.run(
        ["git", *args],
        cwd=root,
        check=False,
        stdout=subprocess.PIPE,
        stderr=subprocess.PIPE,
        text=True,
    )
    if check and result.returncode:
        detail = result.stderr.strip() or result.stdout.strip()
        raise ValidationError(f"git {' '.join(args)} failed: {detail}")
    return result.stdout.strip()


def safe_relative_path(root: Path, value: Any, label: str) -> Path:
    if not isinstance(value, str) or not value:
        raise ValidationError(f"{label}: path must be a non-empty repository-relative string")
    path = Path(value)
    if path.is_absolute() or ".." in path.parts:
        raise ValidationError(f"{label}: path must stay inside the repository: {value!r}")
    resolved = (root / path).resolve()
    try:
        resolved.relative_to(root.resolve())
    except ValueError as exc:
        raise ValidationError(f"{label}: path escapes the repository: {value!r}") from exc
    return path


def require_dict(value: Any, label: str) -> dict[str, Any]:
    if not isinstance(value, dict):
        raise ValidationError(f"{label}: expected an object")
    return value


def require_list(value: Any, label: str) -> list[Any]:
    if not isinstance(value, list) or not value:
        raise ValidationError(f"{label}: expected a non-empty list")
    return value


def require_string(value: Any, label: str) -> str:
    if not isinstance(value, str) or not value.strip():
        raise ValidationError(f"{label}: expected a non-empty string")
    return value


def validate_id(value: Any, label: str) -> str:
    identifier = require_string(value, label)
    if not ID_RE.fullmatch(identifier):
        raise ValidationError(
            f"{label}: {identifier!r} is not a stable lowercase corpus ID"
        )
    lowered = identifier.lower()
    if any(word in lowered for word in PENDING_WORDS):
        raise ValidationError(f"{label}: pending/missing placeholder IDs are not allowed")
    return identifier


def function_exists(source: str, symbol: str) -> bool:
    """Find a Rust function declaration without treating comments as symbols."""

    name = symbol.rsplit("::", 1)[-1]
    pattern = re.compile(FUNCTION_RE_TEMPLATE.format(name=re.escape(name)))
    return pattern.search(source) is not None


def command_display(command: list[str]) -> str:
    return " ".join(shlex.quote(part) for part in command)


def run_command(root: Path, command: list[str], test_ids: list[str]) -> None:
    print(
        f"[behavioral-parity] running {', '.join(test_ids)}: {command_display(command)}",
        flush=True,
    )
    result = subprocess.run(command, cwd=root, check=False)
    if result.returncode:
        raise ValidationError(
            f"mapped v2 target failed ({', '.join(test_ids)}): exit {result.returncode}"
        )


def validate_manifest(root: Path, manifest_path: Path, run_tests: bool) -> tuple[int, int]:
    try:
        data = json.loads(manifest_path.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError) as exc:
        raise ValidationError(f"cannot read manifest {manifest_path}: {exc}") from exc

    corpus = require_dict(data, "manifest")
    if corpus.get("format") != FORMAT:
        raise ValidationError(
            f"manifest format must be {FORMAT!r}, got {corpus.get('format')!r}"
        )
    issue = require_string(corpus.get("issue"), "manifest.issue")
    if issue != "#542":
        raise ValidationError(f"manifest.issue must be '#542', got {issue!r}")

    oracle = require_dict(corpus.get("oracle"), "manifest.oracle")
    commit = require_string(oracle.get("commit"), "manifest.oracle.commit")
    if not COMMIT_RE.fullmatch(commit):
        raise ValidationError("manifest.oracle.commit must be a full 40-character SHA-1")

    # Do not silently accept a moving branch or an abbreviated object.  The
    # exact object must be present in the checkout used by this gate.
    object_type = git(root, "cat-file", "-t", commit, check=False)
    if object_type != "commit":
        shallow = git(root, "rev-parse", "--is-shallow-repository", check=False)
        if shallow == "true":
            raise ValidationError(
                "pinned 1.x oracle commit is unavailable in this shallow checkout; "
                "the behavioral-parity CI job must use actions/checkout with fetch-depth: 0"
            )
        raise ValidationError(
            f"pinned 1.x oracle commit {commit} is not present (git cat-file returned {object_type!r})"
        )
    resolved_commit = git(root, "rev-parse", f"{commit}^{{commit}}")
    if resolved_commit != commit:
        raise ValidationError(
            f"oracle object resolved to {resolved_commit}, expected exact {commit}"
        )
    shallow = git(root, "rev-parse", "--is-shallow-repository", check=False)
    if shallow == "true":
        raise ValidationError(
            "behavioral-parity validation requires non-shallow Git history so the pinned "
            "oracle remains reproducible; fetch with depth 0"
        )

    required_families = require_list(
        corpus.get("required_families"), "manifest.required_families"
    )
    required_family_names = [
        validate_id(family, "manifest.required_families entry") for family in required_families
    ]
    if len(set(required_family_names)) != len(required_family_names):
        raise ValidationError("manifest.required_families contains duplicate IDs")

    approved_changes_value = corpus.get("approved_intentional_changes", [])
    if not isinstance(approved_changes_value, list):
        raise ValidationError("manifest.approved_intentional_changes must be a list")
    approved_changes = {
        validate_id(change, "manifest.approved_intentional_changes entry")
        for change in approved_changes_value
    }

    v2_tests_raw = require_list(corpus.get("v2_tests"), "manifest.v2_tests")
    v2_tests: dict[str, dict[str, Any]] = {}
    all_ids: set[str] = set()
    for index, raw_test in enumerate(v2_tests_raw):
        label = f"manifest.v2_tests[{index}]"
        test = require_dict(raw_test, label)
        test_id = validate_id(test.get("id"), f"{label}.id")
        if test_id in all_ids:
            raise ValidationError(f"duplicate corpus ID: {test_id}")
        all_ids.add(test_id)
        path = safe_relative_path(root, test.get("file"), f"{label}.file")
        source_path = root / path
        if not source_path.is_file():
            raise ValidationError(f"{label}: v2 source file does not exist: {path}")
        symbol = require_string(test.get("symbol"), f"{label}.symbol")
        source = source_path.read_text(encoding="utf-8")
        if not function_exists(source, symbol):
            raise ValidationError(
                f"{label}: v2 symbol {symbol!r} is missing from {path}; update the mapping"
            )
        command_value = require_list(test.get("command"), f"{label}.command")
        if not all(isinstance(part, str) and part for part in command_value):
            raise ValidationError(f"{label}.command must contain non-empty strings")
        command = [str(part) for part in command_value]
        if command[0] != "cargo" or "test" not in command[1:]:
            raise ValidationError(
                f"{label}.command must directly invoke cargo test, got {command_display(command)}"
            )
        v2_tests[test_id] = test

    behaviors = require_list(corpus.get("behaviors"), "manifest.behaviors")
    behavior_ids: set[str] = set()
    families_seen: set[str] = set()
    command_groups: OrderedDict[tuple[str, ...], list[str]] = OrderedDict()

    for index, raw_behavior in enumerate(behaviors):
        label = f"manifest.behaviors[{index}]"
        behavior = require_dict(raw_behavior, label)
        behavior_id = validate_id(behavior.get("id"), f"{label}.id")
        if behavior_id in all_ids or behavior_id in behavior_ids:
            raise ValidationError(f"duplicate corpus ID: {behavior_id}")
        behavior_ids.add(behavior_id)
        if behavior_id not in required_family_names:
            raise ValidationError(
                f"{label}: behavior ID {behavior_id!r} is not in required_families"
            )
        families_seen.add(behavior_id)
        if behavior.get("state") != "verified":
            raise ValidationError(
                f"{label}: state must be 'verified'; pending or unreviewed rows are rejected"
            )
        classification = require_string(
            behavior.get("classification"), f"{label}.classification"
        )
        if classification == "preserved":
            if "approved_change" in behavior:
                raise ValidationError(
                    f"{label}: preserved rows cannot carry an approved behavior change"
                )
        elif classification == "intentional-change":
            approved_change = validate_id(
                behavior.get("approved_change"), f"{label}.approved_change"
            )
            if approved_change not in approved_changes:
                raise ValidationError(
                    f"{label}: intentional behavior change {approved_change!r} is not approved"
                )
        else:
            raise ValidationError(
                f"{label}: classification must be 'preserved' or 'intentional-change'"
            )
        clause = require_string(behavior.get("clause"), f"{label}.clause")
        if not clause.startswith("#542"):
            raise ValidationError(f"{label}.clause must cite #542")
        require_string(behavior.get("rationale"), f"{label}.rationale")

        sources = require_list(behavior.get("oracle_sources"), f"{label}.oracle_sources")
        for source_index, raw_source in enumerate(sources):
            source_label = f"{label}.oracle_sources[{source_index}]"
            source_ref = require_dict(raw_source, source_label)
            source_path = require_string(source_ref.get("path"), f"{source_label}.path")
            # This path is deliberately validated against the pinned commit,
            # not the current checkout: a 2.0 rewrite may remove the old file.
            safe_relative_path(root, source_path, f"{source_label}.path")
            blob = git(root, "show", f"{commit}:{source_path}", check=False)
            if not blob:
                # Empty source files are not valid provenance anchors either;
                # git show's status is checked below for a precise error.
                status = subprocess.run(
                    ["git", "cat-file", "-e", f"{commit}:{source_path}"],
                    cwd=root,
                    check=False,
                    stdout=subprocess.DEVNULL,
                    stderr=subprocess.DEVNULL,
                ).returncode
                if status:
                    raise ValidationError(
                        f"{source_label}: source file {source_path!r} is absent at oracle {commit}"
                    )
            symbols = require_list(source_ref.get("symbols"), f"{source_label}.symbols")
            seen_symbols: set[str] = set()
            for symbol_index, raw_symbol in enumerate(symbols):
                symbol = require_string(
                    raw_symbol, f"{source_label}.symbols[{symbol_index}]"
                )
                if symbol in seen_symbols:
                    raise ValidationError(
                        f"{source_label}: duplicate source symbol {symbol!r}"
                    )
                seen_symbols.add(symbol)
                if not function_exists(blob, symbol):
                    raise ValidationError(
                        f"{source_label}: symbol {symbol!r} is missing at {commit}:{source_path}"
                    )

        mapped_tests = require_list(behavior.get("v2_tests"), f"{label}.v2_tests")
        seen_mapped: set[str] = set()
        for test_index, raw_test_id in enumerate(mapped_tests):
            test_id = validate_id(raw_test_id, f"{label}.v2_tests[{test_index}]")
            if test_id in seen_mapped:
                raise ValidationError(f"{label}: duplicate v2 test reference {test_id!r}")
            seen_mapped.add(test_id)
            if test_id not in v2_tests:
                raise ValidationError(
                    f"{label}: v2 test reference {test_id!r} has no definition"
                )
            command = tuple(v2_tests[test_id]["command"])
            command_groups.setdefault(command, []).append(test_id)

    missing_families = set(required_family_names) - families_seen
    extra_families = families_seen - set(required_family_names)
    if missing_families:
        raise ValidationError(
            "manifest is missing required behavior families: "
            + ", ".join(sorted(missing_families))
        )
    if extra_families:
        raise ValidationError(
            "manifest contains behavior families not declared as required: "
            + ", ".join(sorted(extra_families))
        )
    if len(behavior_ids) != len(required_family_names):
        raise ValidationError(
            "each required behavior family must have exactly one verified row"
        )

    # Every test definition is expected to be reachable from a row.  A dead
    # definition is as dangerous as a stale row because it can give a false
    # impression of coverage while never being executed.
    referenced_tests = {test_id for ids in command_groups.values() for test_id in ids}
    unreferenced_tests = set(v2_tests) - referenced_tests
    if unreferenced_tests:
        raise ValidationError(
            "v2 test definitions are not mapped by any behavior row: "
            + ", ".join(sorted(unreferenced_tests))
        )

    if run_tests:
        for command, test_ids in command_groups.items():
            run_command(root, list(command), test_ids)
    else:
        print("[behavioral-parity] structural validation complete (--skip-tests; no cargo targets run)")

    return len(behaviors), len(v2_tests)


def main() -> int:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument(
        "--manifest",
        type=Path,
        default=Path(__file__).with_name("manifest.json"),
        help="repository-relative or absolute path to the corpus manifest",
    )
    parser.add_argument(
        "--skip-tests",
        action="store_true",
        help="validate provenance and mappings without running cargo (CI must omit this)",
    )
    args = parser.parse_args()

    try:
        # Resolve the repository from Git rather than from a fixed workspace
        # path.  This keeps local checkouts, CI, and release validation alike.
        probe_root = Path.cwd()
        root = Path(git(probe_root, "rev-parse", "--show-toplevel"))
        manifest_path = args.manifest
        if not manifest_path.is_absolute():
            manifest_path = (root / manifest_path).resolve()
        behaviors, v2_tests = validate_manifest(root, manifest_path, not args.skip_tests)
    except ValidationError as exc:
        print(f"behavioral parity validation failed: {exc}", file=sys.stderr)
        return 1
    except (OSError, UnicodeError) as exc:
        print(f"behavioral parity validation failed: {exc}", file=sys.stderr)
        return 1

    print(
        f"[behavioral-parity] verified {behaviors} behavior families, "
        f"{v2_tests} mapped v2 test targets, oracle "
        "6c7a9d3783861189745536372c4d21de24d4252d"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
