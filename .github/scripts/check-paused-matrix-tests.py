#!/usr/bin/env python3
"""Keep audited paused scenarios out of production-only matrix legs.

Each suite below has one facade_matrix! { paused: ... } at its root (or the
`paused` module in runtime_tuning). Their scenario bodies have no feature
selection besides the facade. Transport faults, retries, raw evidence, stream
stall and menu navigation are entirely paused; runtime_tuning and operation
observation also keep separate, ordinary async concurrency modules. The latter
modules must continue to run in production-only Tokio/smol legs.
"""

import argparse
from pathlib import Path
import re
import shlex

SUITES = {
    "issue_554_owner_cancellation": "paused",
    "fr7_pan_tilt": "",
    "issue_553_owner_motion": "paused",
    "issue_565_transport_faults": "",
    "issue_566_retry_recovery": "",
    "issue_631_runtime_tuning": "paused",
    "issue_777_operation_observation": "",
    "issue_778_control_reserve": "paused",
    "issue_795_raw_correlation_evidence": "",
    "issue_795_stream_stall_inquiry": "",
    "issue_839_menu_navigation": "",
    "issue_848_stream_framing": "cancellation",
    "issue_858_late_evidence": "",
    "issue_869_envelope_admission": "",
    # The #872 observer-budget body has no feature-selected behavior either.
    "issue_872_observer_budgets": "settlement_budget",
    "issue_870_relative_offsets": "facades",
    "issue_894_fr7_families": "paused",
    "issue_892_payload_shape": "",
    "issue_975_lazy_settlement": "",
    "issue_979_owner_read_timeout": "",
}


def has_test_utils(command: str) -> bool:
    args = shlex.split(command)
    if "--all-features" in args:
        return True
    for index, argument in enumerate(args):
        if argument == "--features" and index + 1 < len(args):
            features = args[index + 1]
        elif argument.startswith("--features="):
            features = argument.removeprefix("--features=")
        else:
            continue
        if "test-utils" in re.split(r"[,\s]+", features):
            return True
    return False


def violations(command: str, output: str) -> list[str]:
    if has_test_utils(command):
        return []
    found = []
    for line in output.splitlines():
        case = re.match(r"test ([a-zA-Z0-9_]+)::([a-zA-Z0-9_:]+) \.\.\. ", line)
        if case is None:
            continue
        suite, name = case.groups()
        if suite not in SUITES:
            continue
        parts = name.split("::")
        if len(parts) >= 2 and parts[-1] in {"blocking", "smol"} and "::".join(parts[:-2]) == SUITES[suite]:
            found.append(f"{suite}::{name}")
    return found


def self_test() -> None:
    repository = Path(__file__).resolve().parents[2]
    discovered = {
        path.stem for path in (repository / "tests" / "integration").glob("*.rs")
        if re.search(r"facade_matrix!\s*\{\s*paused\s*:", path.read_text())
    }
    assert discovered == set(SUITES), "update the audited paused-suite inventory when adding or removing a suite"
    for suite, prefix in SUITES.items():
        for facade in ("blocking", "smol"):
            name = "::".join(part for part in (prefix, "scenario", facade) if part)
            log = f"Running tests/integration/main.rs (target/debug/deps/integration-hash)\ntest {suite}::{name} ... ok\n"
            expected = [f"{suite}::{name}"]
            assert violations("--features blocking", log) == expected
            assert violations("--features runtime-smol,test-utils", log) == []
            assert violations("--all-features", log) == []
    log = "test issue_631_runtime_tuning::paused::scenario::blocking ... ignored\n"
    assert violations("--features blocking", log)
    # The separate concurrency modules still run without virtual time.
    log = "test issue_777_operation_observation::async_facade::scenario::smol ... ok\n"
    assert violations("--features runtime-smol", log) == []
    log = "test issue_866_owner_timers::local_manual_clock_actor_reuses_four_timer_factories ... ok\n"
    assert violations("--no-default-features --features async,test-utils", log) == []
    assert not has_test_utils("--features test-utils-extra")
    assert has_test_utils("--features=runtime-tokio,test-utils")


def main() -> None:
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--self-test", action="store_true")
    parser.add_argument("--command", default="")
    parser.add_argument("log", nargs="?", type=Path)
    args = parser.parse_args()
    if args.self_test:
        self_test()
        return
    if args.log is None:
        parser.error("log is required unless --self-test is selected")
    found = violations(args.command, args.log.read_text(errors="replace"))
    if found:
        parser.exit(1, "paused scenario compiled without test-utils:\n  " + "\n  ".join(found) + "\n")


if __name__ == "__main__":
    main()
