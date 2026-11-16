#!/usr/bin/env python3
"""Capture and compare executable test inventories across integration layouts.

Run `capture OUTPUT` on each checkout, then `compare BEFORE AFTER` on their
inventory.json files. Capture uses every leg of test-all-features.sh list-json,
keeps its complete Cargo/libtest log, and normalizes only the suite prefix
introduced when formerly independent test crates become modules. It includes
library, example, fixture and macro tests, so changes in feature gating fail.

`measure OUTPUT --target-dir EMPTY_DIRECTORY` records a cold default build and
an incremental build after a temporary codegen edit to one populated suite, including executable
bytes and Cargo's aggregate integration compile/link time. Use the same host,
toolchain and CARGO_BUILD_JOBS setting for both checkouts.
"""

import argparse
import json
import os
from pathlib import Path
import re
import shlex
import shutil
import subprocess
import time


SELECTED_TARGETS = {"api_stability_test", "no_default_smoke"}
ALLOCATION_SUITES = {
    "dyn_api_integration_test",
    "issue_517_inquiry_encoding",
    "issue_548_allocation_baselines",
}


def inventory(output: str) -> list[str]:
    """Normalize Cargo binary identity and libtest suite names, never test names."""
    records = []
    target = None
    prefix = None
    for line in output.splitlines():
        match = re.search(r"Running (.*?) \((.*?)\)", line)
        if match:
            source, executable = match.groups()
            source = source.replace("\\", "/")
            executable = executable.replace("\\", "/")
            target = re.sub(r"-[a-f0-9]{8,}$", "", Path(executable).name.removesuffix(".exe"))
            prefix = None
            if source.startswith("tests/"):
                path = Path(source)
                suite = path.parent.name if path.stem == "main" else path.stem
                if suite in SELECTED_TARGETS:
                    target = suite
                elif suite in {"allocation", "integration"}:
                    target = suite
                else:
                    target = "allocation" if suite in ALLOCATION_SUITES else "integration"
                    prefix = suite
        case = re.fullmatch(r"(.+): (test|benchmark)", line)
        if case and target:
            name, kind = case.groups()
            if prefix:
                name = f"{prefix}::{name}"
            records.append(f"{target}::{name}: {kind}")
    if len(records) != len(set(records)):
        raise ValueError("test inventory contains duplicate qualified test names")
    return sorted(records)


def run(repository: Path, args: list[str], log: Path, env: dict[str, str]) -> float:
    started = time.monotonic()
    with log.open("w") as stream:
        subprocess.run(args, cwd=repository, env=env, stdout=stream,
                       stderr=subprocess.STDOUT, check=True)
    return time.monotonic() - started


def capture(repository: Path, output: Path, resume: bool) -> None:
    output.mkdir(parents=True, exist_ok=True)
    legs = json.loads(subprocess.check_output(
        ["bash", ".github/scripts/test-all-features.sh", "list-json"], cwd=repository))
    partial = output / "inventory.partial.json"
    result = json.loads(partial.read_text()) if resume and partial.exists() else {}
    for index, leg in enumerate(legs):
        if leg["name"] in result:
            continue
        log = output / f"{index:02d}.log"
        args = ["cargo", "+1.98.0", "test", *shlex.split(leg["command"]),
                "--", "--list", "--format", "terse"]
        elapsed = run(repository, args, log, dict(os.environ))
        result[leg["name"]] = inventory(log.read_text())
        partial.write_text(json.dumps(result, indent=2) + "\n")
        print(f"{leg['name']}: {len(result[leg['name']])} tests, {elapsed:.2f}s", flush=True)
    partial.replace(output / "inventory.json")


def compare(before: Path, after: Path) -> None:
    left = json.loads(before.read_text())
    right = json.loads(after.read_text())
    if not left or not right:
        raise SystemExit("cannot compare an empty feature-leg inventory")
    failures = []
    for leg in sorted(left.keys() | right.keys()):
        if leg not in left or leg not in right:
            failures.append(f"feature leg missing from one inventory: {leg}")
            continue
        removed = set(left[leg]) - set(right[leg])
        added = set(right[leg]) - set(left[leg])
        if removed or added:
            failures.append(f"{leg}:\n" + "\n".join(
                [f"  removed {name}" for name in sorted(removed)] +
                [f"  added {name}" for name in sorted(added)]))
    if failures:
        raise SystemExit("\n".join(failures))
    print(f"identical executable test inventories in all {len(left)} feature legs")


def measurement(repository: Path, output: Path, target: Path) -> None:
    if target.exists() and any(target.iterdir()):
        raise SystemExit(f"cold measurement needs an empty target directory: {target}")
    output.mkdir(parents=True, exist_ok=True)
    env = dict(os.environ, CARGO_TARGET_DIR=str(target))
    for name in ("cold", "incremental"):
        original = None
        if name == "incremental":
            suite = repository / "tests/issue_633_golden_wire_bytes.rs"
            if not suite.exists():
                suite = repository / "tests/integration/issue_633_golden_wire_bytes.rs"
            # A bare mtime change lets rustc reuse every codegen result and
            # skip linking. Force a real one-suite rebuild without changing
            # its tests, then restore the checkout even when Cargo fails.
            original = suite.read_text()
            suite.write_text(original + "\n#[used]\nstatic INCREMENTAL_MEASUREMENT_EDIT: u8 = 1;\n")
        log = output / f"{name}.log"
        try:
            elapsed = run(repository, ["cargo", "+1.98.0", "test", "--workspace", "--all-targets",
                                      "--no-run", "--timings", "--message-format=json"], log, env)
        finally:
            if original is not None:
                suite.write_text(original)
        executables = {}
        rebuilt = []
        for line in log.read_text().splitlines():
            try:
                row = json.loads(line)
            except ValueError:
                continue
            if row.get("reason") == "compiler-artifact" and row.get("executable") and "test" in row["target"]["kind"]:
                executables[row["target"]["name"]] = Path(row["executable"]).stat().st_size
                if not row["fresh"]:
                    rebuilt.append(row["target"]["name"])
        timing = max((target / "cargo-timings").glob("cargo-timing-*.html"), key=lambda p: p.stat().st_mtime)
        match = re.search(r"const UNIT_DATA = (\[.*?\]);", timing.read_text(), re.S)
        if match is None:
            raise SystemExit(f"Cargo timing unit data missing: {timing}")
        units = json.loads(match.group(1))
        duration = sum(unit["duration"] for unit in units
                       if re.fullmatch(r' [A-Za-z0-9_-]+ "test" \(test\)', unit.get("target", "")))
        result = {"wall_seconds": elapsed, "integration_executables": executables,
                  "integration_executable_bytes": sum(executables.values()),
                  "integration_compile_link_seconds": duration,
                  "rebuilt_integration_targets": rebuilt}
        shutil.copyfile(timing, output / f"{name}-timing.html")
        (output / f"{name}.json").write_text(json.dumps(result, indent=2) + "\n")
        print(f"{name}: {json.dumps(result)}", flush=True)


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("mode", choices=("capture", "compare", "measure"))
    parser.add_argument("output", type=Path)
    parser.add_argument("after", type=Path, nargs="?")
    parser.add_argument("--repository", type=Path, default=Path(__file__).resolve().parents[2])
    parser.add_argument("--target-dir", type=Path)
    parser.add_argument("--resume", action="store_true", help="resume a capture's completed feature legs")
    args = parser.parse_args()
    if args.mode == "capture":
        capture(args.repository.resolve(), args.output.resolve(), args.resume)
    elif args.mode == "compare":
        if args.after is None:
            parser.error("compare requires BEFORE and AFTER inventory.json paths")
        compare(args.output, args.after)
    else:
        if args.target_dir is None:
            parser.error("measure requires --target-dir EMPTY_DIRECTORY")
        measurement(args.repository.resolve(), args.output.resolve(), args.target_dir.resolve())


if __name__ == "__main__":
    main()
