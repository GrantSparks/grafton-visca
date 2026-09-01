#!/usr/bin/env python3
"""Validate and execute the 1.x behavioral-parity corpus.

This gate is deliberately a provenance check plus a runner for already-owned
v2 tests.  It does not model the protocol or reproduce scheduler logic.  A
row is valid only when its validator-pinned oracle source, current source and
symbol, exact libtest path, command, and configuration evidence all agree.
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
EXPECTED_ORACLE_COMMIT = "6c7a9d3783861189745536372c4d21de24d4252d"
COMMIT_RE = re.compile(r"^[0-9a-f]{40}$")
ID_RE = re.compile(r"^[a-z][a-z0-9._-]+$")
FUNCTION_RE_TEMPLATE = (
    r"(?<![A-Za-z0-9_])(?:(?:pub(?:\([^)]*\))?)\s+)?(?:async\s+)?fn\s+{name}"
    r"(?:<[^>\n]*>)?\s*\("
)
PENDING_WORDS = ("pending", "missing", "todo", "tbd")

# The complete set of behavior families the corpus is required to cover, pinned
# here and not only in the manifest. A family can now be dropped from coverage
# only by editing this validator as well, so a single quiet manifest edit that
# deletes a family (and its row) no longer passes the gate silently.
EXPECTED_REQUIRED_FAMILIES = frozenset(
    {
        "timeout-category-defaults-selection",
        "retry-counts-exhaustion-and-ceiling",
        "deterministic-equal-jitter",
        "lower16-exact-unique-collision-recovery",
        "stale-sequenced-responses",
        "raw-evidence-based-routing",
        "transport-failure-isolation",
        "command-wire-bytes",
        "inquiry-decode-golden",
        "blocking-out-of-order-receipt-retention",
        "cancellation-detach-observer-late-delivery",
        "malformed-frame-tolerance",
    }
)

# Every mapped v2 test records the transport envelope, camera profile, and
# terminal receipt class it actually exercises. Recording and asserting these
# makes an evidence-laundering swap — e.g. re-pointing a "raw" behavior row at a
# Sony-only test under an unchanged name — a visible manifest diff that the gate
# then checks against the test body, instead of an invisible re-baseline.
VALID_ENVELOPES = frozenset({"sony", "raw", "neutral"})
VALID_RECEIPT_CLASSES = frozenset(
    {
        "applied",
        "targeted",
        "sequenced",
        "inquiry",
        "routing",
        "wire",
        "decode",
        "cancel",
        "detach",
        "poison",
        "transport",
        "retry-budget",
        "timeout",
        "trace",
    }
)
PROFILE_TYPE_RE = re.compile(r"[A-Z][A-Za-z0-9]+")
CONFIG_TOKEN_RE = re.compile(r"[a-z][a-z0-9-]*")
RAW_STRING_START_RE = re.compile(r'(?:br|cr|r)(?P<hashes>#+)?"')
LIFETIME_START_RE = re.compile(r"'[A-Za-z_][A-Za-z0-9_]*")

# The manifest is an index, not its own approval authority. These exact current
# targets are pinned in executable validator code so redirecting a row requires
# review of both the data and the gate. `test_path` is the canonical name
# printed by libtest, which is intentionally distinct from the source helper
# for generated runtime-matrix cases.
PINNED_V2_TARGETS: dict[str, tuple[str, str, str, str]] = {
    # id: (source file, source symbol, canonical libtest path, receipt class)
    "v2-prepared-timeout-selection": (
        "src/prepared.rs",
        "tests::command_completion_uses_category_overrides_then_profile_values",
        "prepared::tests::command_completion_uses_category_overrides_then_profile_values",
        "timeout",
    ),
    "v2-profile-default-deadlines": (
        "src/camera/profile_registry.rs",
        "registry_tests::builtin_profile_default_deadlines_match_the_1x_ack_and_interim_inquiry",
        "camera::profiles::registry_tests::builtin_profile_default_deadlines_match_the_1x_ack_and_interim_inquiry",
        "timeout",
    ),
    "v2-timeout-provenance-partition": (
        "src/command/semantics.rs",
        "tests::timeout_category_partition_preserves_the_1x_provenance_boundary",
        "command::semantics::tests::timeout_category_partition_preserves_the_1x_provenance_boundary",
        "timeout",
    ),
    "v2-prepared-retry-table": (
        "src/prepared.rs",
        "tests::retry_budgets_follow_the_1x_per_category_table",
        "prepared::tests::retry_budgets_follow_the_1x_per_category_table",
        "retry-budget",
    ),
    "v2-engine-deterministic-jitter": (
        "src/runtime/engine/tests.rs",
        "retry_backoff_follows_the_pinned_jitter_sequence",
        "runtime::engine::tests::retry_backoff_follows_the_pinned_jitter_sequence",
        "retry-budget",
    ),
    "v2-engine-lower16": (
        "src/runtime/engine/tests.rs",
        "sony_exact_and_unique_lower16_are_target_safe_and_owner_deduplicated",
        "runtime::engine::tests::sony_exact_and_unique_lower16_are_target_safe_and_owner_deduplicated",
        "sequenced",
    ),
    "v2-engine-stale-retry": (
        "src/runtime/engine/tests.rs",
        "sony_retry_reuses_first_successful_sequence_and_ignores_stale_result",
        "runtime::engine::tests::sony_retry_reuses_first_successful_sequence_and_ignores_stale_result",
        "sequenced",
    ),
    "v2-engine-stale-command-error": (
        "src/runtime/engine/tests.rs",
        "sony_stale_command_errors_do_not_spend_retry_during_backoff_or_ready",
        "runtime::engine::tests::sony_stale_command_errors_do_not_spend_retry_during_backoff_or_ready",
        "sequenced",
    ),
    "v2-engine-stale-inquiry-error": (
        "src/runtime/engine/tests.rs",
        "sony_stale_inquiry_errors_do_not_spend_retry_during_backoff_or_ready",
        "runtime::engine::tests::sony_stale_inquiry_errors_do_not_spend_retry_during_backoff_or_ready",
        "inquiry",
    ),
    "v2-engine-raw-routing": (
        "src/runtime/engine/tests.rs",
        "raw_inquiries_route_by_unique_content_then_per_target_fifo",
        "runtime::engine::tests::raw_inquiries_route_by_unique_content_then_per_target_fifo",
        "routing",
    ),
    "v2-engine-raw-error-routing": (
        "src/runtime/engine/tests.rs",
        "raw_error_policy_requires_unique_socketless_evidence",
        "runtime::engine::tests::raw_error_policy_requires_unique_socketless_evidence",
        "routing",
    ),
    "v2-owner-transport-receive": (
        "src/runtime/owner/tests.rs",
        "tests::blocking::sony_transient_blocking_read_fault_retries_and_keeps_the_session",
        "runtime::owner::tests::blocking::sony_transient_blocking_read_fault_retries_and_keeps_the_session",
        "transport",
    ),
    "v2-owner-out-of-order": (
        "src/runtime/owner/tests.rs",
        "tests::blocking::sony_typed_blocking_wait_pumps_and_retains_out_of_order_peer_results",
        "runtime::owner::tests::blocking::sony_typed_blocking_wait_pumps_and_retains_out_of_order_peer_results",
        "targeted",
    ),
    "v2-owner-lifecycle-trace": (
        "src/runtime/owner/tests.rs",
        "tests::blocking::blocking_owner_matches_canonical_lifecycle_trace",
        "runtime::owner::tests::blocking::blocking_owner_matches_canonical_lifecycle_trace",
        "trace",
    ),
    "v2-owner-observer-late": (
        "src/runtime/owner/tests.rs",
        "tests::blocking::observer_timeout_detaches_without_cancel_and_late_applied_still_caches",
        "runtime::owner::tests::blocking::observer_timeout_detaches_without_cancel_and_late_applied_still_caches",
        "detach",
    ),
    "v2-owner-detached-late": (
        "src/runtime/owner/tests.rs",
        "tests::blocking::detached_late_completion_still_updates_cache_and_subscription",
        "runtime::owner::tests::blocking::detached_late_completion_still_updates_cache_and_subscription",
        "detach",
    ),
    "v2-faults-blocking": (
        "tests/issue_565_transport_faults_blocking.rs",
        "raw_receive_fault_fails_one_command_and_keeps_the_session",
        "raw_receive_fault_fails_one_command_and_keeps_the_session",
        "transport",
    ),
    "v2-faults-async": (
        "tests/issue_565_transport_faults_async.rs",
        "raw_receive_fault_fails_one_command_and_keeps_the_session",
        "raw_receive_fault_fails_one_command_and_keeps_the_session::tokio",
        "transport",
    ),
    "v2-retries-blocking": (
        "tests/issue_566_retry_recovery_blocking.rs",
        "a_sony_movement_command_survives_a_lost_ack",
        "a_sony_movement_command_survives_a_lost_ack",
        "applied",
    ),
    "v2-retries-async": (
        "tests/issue_566_retry_recovery_async.rs",
        "a_silent_sony_camera_is_retried_before_and_after_its_ack",
        "a_silent_sony_camera_is_retried_before_and_after_its_ack::tokio",
        "applied",
    ),
    "v2-retries-scripted": (
        "tests/issue_566_scripted_error_recovery.rs",
        "a_busy_camera_is_replayed_once_and_then_succeeds",
        "a_busy_camera_is_replayed_once_and_then_succeeds",
        "retry-budget",
    ),
    "v2-raw-lifecycle-trace": (
        "tests/issue_542_trace_fixture_contract.rs",
        "protocol_correlation_trace_stays_a_well_formed_normative_record",
        "protocol_correlation_trace_stays_a_well_formed_normative_record",
        "trace",
    ),
    "v2-wire-golden": (
        "tests/issue_633_golden_wire_bytes.rs",
        "cardinal_pan_tilt_directions_match_golden_frames",
        "cardinal_pan_tilt_directions_match_golden_frames",
        "wire",
    ),
    "v2-inquiry-golden": (
        "tests/inquiry_golden_tests_simple.rs",
        "test_power_inquiry_on",
        "test_power_inquiry_on",
        "decode",
    ),
    "v2-inquiry-decoding": (
        "tests/inquiry_response_parsing_tests.rs",
        "test_parse_power_inquiry_responses",
        "test_parse_power_inquiry_responses",
        "decode",
    ),
    "v2-cancel-blocking": (
        "tests/issue_612_cancel_recovery_blocking.rs",
        "blocking_refused_cancellation_returns_the_handle_and_leaves_stop_available",
        "blocking_refused_cancellation_returns_the_handle_and_leaves_stop_available",
        "cancel",
    ),
    "v2-cancel-async": (
        "tests/issue_612_cancel_recovery.rs",
        "tokio_refused_cancellation_is_recoverable",
        "tokio_refused_cancellation_is_recoverable",
        "cancel",
    ),
    "v2-detach-facade": (
        "tests/issue_542_async_facade.rs",
        "dropping_or_detaching_operation_is_observation_only",
        "dropping_or_detaching_operation_is_observation_only",
        "detach",
    ),
    "v2-wire-encoder-unit": (
        "src/command/flip.rs",
        "tests::test_byte_sequence_correctness",
        "command::flip::tests::test_byte_sequence_correctness",
        "wire",
    ),
    "v2-stream-malformed-tolerance": (
        "tests/issue_672_674_681_decode_consequence.rs",
        "stream_quirky_frames_are_discarded_and_keep_the_session",
        "stream_quirky_frames_are_discarded_and_keep_the_session",
        "decode",
    ),
}

PINNED_COMMANDS: dict[str, tuple[str, ...]] = {
    "lib-prepared": ("cargo", "test", "--lib", "prepared::tests"),
    "lib-profile-defaults": (
        "cargo",
        "test",
        "--lib",
        "builtin_profile_default_deadlines_match_the_1x_ack_and_interim_inquiry",
    ),
    "lib-timeout-partition": (
        "cargo",
        "test",
        "--lib",
        "timeout_category_partition_preserves_the_1x_provenance_boundary",
    ),
    "lib-engine": ("cargo", "test", "--lib", "runtime::engine::tests"),
    "lib-owner": ("cargo", "test", "--lib", "runtime::owner::tests"),
    "lib-flip": ("cargo", "test", "--lib", "command::flip::tests"),
    "tokio-542-facade": (
        "cargo", "test", "--no-default-features", "--features", "runtime-tokio",
        "--test", "issue_542_async_facade",
    ),
    "tokio-565-faults": (
        "cargo", "test", "--no-default-features", "--features", "runtime-tokio",
        "--test", "issue_565_transport_faults_async",
    ),
    "tokio-566-retries": (
        "cargo", "test", "--no-default-features", "--features", "runtime-tokio",
        "--test", "issue_566_retry_recovery_async",
    ),
    "tokio-612-cancel": (
        "cargo", "test", "--no-default-features", "--features", "runtime-tokio",
        "--test", "issue_612_cancel_recovery",
    ),
    "tokio-scripted": (
        "cargo", "test", "--no-default-features", "--features",
        "test-utils,blocking,runtime-tokio", "--test", "issue_566_scripted_error_recovery",
    ),
    "test-trace": ("cargo", "test", "--test", "issue_542_trace_fixture_contract"),
    "test-faults-blocking": (
        "cargo", "test", "--test", "issue_565_transport_faults_blocking",
    ),
    "test-retries-blocking": (
        "cargo", "test", "--test", "issue_566_retry_recovery_blocking",
    ),
    "test-cancel-blocking": (
        "cargo", "test", "--test", "issue_612_cancel_recovery_blocking",
    ),
    "test-wire": ("cargo", "test", "--test", "issue_633_golden_wire_bytes"),
    "test-inquiry-golden": (
        "cargo", "test", "--test", "inquiry_golden_tests_simple",
    ),
    "test-inquiry-decode": (
        "cargo", "test", "--test", "inquiry_response_parsing_tests",
    ),
    "test-malformed": (
        "cargo", "test", "--test", "issue_672_674_681_decode_consequence",
    ),
}

# Envelope, profile, and exact command are pinned outside the manifest so a row
# cannot authorize its own reclassification or run a different binary that
# happens to print the same leaf test name.
PINNED_V2_CONTEXT: dict[str, tuple[str, str, str]] = {
    "v2-prepared-timeout-selection": ("neutral", "PtzOpticsG2", "lib-prepared"),
    "v2-profile-default-deadlines": ("neutral", "n/a", "lib-profile-defaults"),
    "v2-timeout-provenance-partition": ("neutral", "n/a", "lib-timeout-partition"),
    "v2-prepared-retry-table": ("neutral", "n/a", "lib-prepared"),
    "v2-engine-deterministic-jitter": ("neutral", "n/a", "lib-engine"),
    "v2-engine-lower16": ("sony", "n/a", "lib-engine"),
    "v2-engine-stale-retry": ("sony", "n/a", "lib-engine"),
    "v2-engine-stale-command-error": ("sony", "n/a", "lib-engine"),
    "v2-engine-stale-inquiry-error": ("sony", "n/a", "lib-engine"),
    "v2-engine-raw-routing": ("raw", "n/a", "lib-engine"),
    "v2-engine-raw-error-routing": ("raw", "n/a", "lib-engine"),
    "v2-owner-transport-receive": ("sony", "SonyBRC300", "lib-owner"),
    "v2-owner-out-of-order": ("sony", "SonyBRC300", "lib-owner"),
    "v2-owner-lifecycle-trace": ("raw", "n/a", "lib-owner"),
    "v2-owner-observer-late": ("raw", "GenericVisca", "lib-owner"),
    "v2-owner-detached-late": ("raw", "n/a", "lib-owner"),
    "v2-faults-blocking": (
        "raw", "NonDefaultCompileTimeProfile", "test-faults-blocking",
    ),
    "v2-faults-async": ("raw", "NonDefaultCompileTimeProfile", "tokio-565-faults"),
    "v2-retries-blocking": ("sony", "SonyFR7", "test-retries-blocking"),
    "v2-retries-async": ("sony", "SonyFR7", "tokio-566-retries"),
    "v2-retries-scripted": (
        "raw", "NonDefaultCompileTimeProfile", "tokio-scripted",
    ),
    "v2-raw-lifecycle-trace": ("neutral", "n/a", "test-trace"),
    "v2-wire-golden": ("neutral", "n/a", "test-wire"),
    "v2-inquiry-golden": ("neutral", "n/a", "test-inquiry-golden"),
    "v2-inquiry-decoding": ("neutral", "n/a", "test-inquiry-decode"),
    "v2-cancel-blocking": ("raw", "PtzOpticsG2", "test-cancel-blocking"),
    "v2-cancel-async": ("raw", "n/a", "tokio-612-cancel"),
    "v2-detach-facade": ("raw", "PtzOpticsG2", "tokio-542-facade"),
    "v2-wire-encoder-unit": ("neutral", "n/a", "lib-flip"),
    "v2-stream-malformed-tolerance": ("raw", "GenericVisca", "test-malformed"),
}

# These patterns are intentionally validator-owned and operate on lexically
# sanitized code only. They prove that a mapped function contains executable
# code identifiers associated with its pinned receipt class; they do not claim
# to be a Rust semantic analyzer or a protocol model.
RECEIPT_EVIDENCE_PATTERNS = {
    "applied": (r"\bAppliedOnly\b", r"\.applied(?:_with_timeout)?\s*\("),
    "targeted": (r"\.wait\s*\(", r"\bTargetReached\b", r"\.settled"),
    "sequenced": (
        r"\bSequenceWidth\b",
        r"\b(?:current|requested|first)_sequence\b",
        r"\bregister_sequence\b",
    ),
    "inquiry": (r"\bInquiryReply\b", r"\bAwaitingReply\b", r"\binquiry_timeout\b"),
    "routing": (r"\bterminal_id\b", r"\bUnmatchedFrame\b", r"\bInquiryReply\b"),
    "wire": (r"\bassert_golden\b", r"\btest_wire_bytes\b", r"\bto_bytes\b"),
    "decode": (r"\bparse_with_(?:type|profile)\b", r"\bcommand_settles_over_stream\b"),
    "cancel": (r"\.cancel\s*\(", r"\b[a-zA-Z0-9_]*cancel[a-zA-Z0-9_]*\s*\("),
    "detach": (
        r"\.detach\s*\(",
        r"\bdrop\s*\(",
        r"\bsubscribe_applied\b",
        r"\bwait_with_timeout\b",
    ),
    "poison": (r"\bStreamPoisoned\b", r"\bSessionState::Poisoned\b"),
    "transport": (
        r"\bError::Io\b",
        r"\bConnectionClosed\b",
        r"\bUnsequencedCommandUnconfirmed\b",
    ),
    "retry-budget": (
        r"\bRetryPolicy\b",
        r"\bRetryClass\b",
        r"\bmax_retries\b",
        r"\bRetryScheduled\b",
        r"\b(?:sent|writes)\s*\(\s*\)\s*\.\s*len\s*\(",
    ),
    "timeout": (
        r"\bTimeoutClass\b",
        r"\b(?:ack|inquiry|completion|quick|movement|preset|long_running)_timeout\b",
        r"\.timeout\.completion\b",
    ),
    "trace": (r"\bcanonical_owner_trace\b", r"\bCANONICAL_OWNER_TRACE\b", r"\bparse_trace\b"),
}

SONY_CODE_RE = re.compile(r"\b(?:EnvelopeKind::Sony|Sony[A-Z][A-Za-z0-9]*|sony_[A-Za-z0-9_]+)\b")
RAW_CODE_RE = re.compile(
    r"\b(?:EnvelopeKind::Raw|GenericVisca|NonDefaultCompileTimeProfile|PtzOptics[A-Za-z0-9]+|"
    r"raw_[A-Za-z0-9_]+|generic_profile|g2_config|policy|rejected_cancel_returns_the_handle)\b|\braw::"
)

# A libtest "running N tests" banner and per-test result line. A name filter
# that matches nothing still exits 0 after printing "running 0 tests", and an
# #[ignore] test prints "... ignored" while the binary exits 0, so exit status
# alone certifies nothing about whether a specific mapped test executed.
RUNNING_RE = re.compile(r"(?m)^running (?P<count>[0-9]+) tests?$")
TEST_LINE_RE = re.compile(r"(?m)^test (?P<path>\S+) \.\.\. (?P<status>ok|FAILED|ignored)")


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


def _mask_range(chars: list[str], start: int, end: int) -> None:
    """Blank a lexical token without moving offsets or deleting newlines."""

    for index in range(start, end):
        if chars[index] not in "\r\n":
            chars[index] = " "


def sanitize_rust(source: str) -> str:
    """Mask Rust comments and literals while preserving code offsets.

    This is deliberately a lexer-level safety boundary, not a Rust parser. It
    understands nested block comments, line comments, ordinary/byte/C strings,
    raw strings with arbitrary hash counts, and ordinary/byte character
    literals. Keeping offsets and newlines stable lets the brace matcher below
    isolate exactly one function body without allowing prose or literal payload
    to satisfy code-evidence checks.
    """

    chars = list(source)
    length = len(source)
    index = 0
    while index < length:
        if source.startswith("//", index):
            end = source.find("\n", index + 2)
            end = length if end < 0 else end
            _mask_range(chars, index, end)
            index = end
            continue

        if source.startswith("/*", index):
            depth = 1
            end = index + 2
            while end < length and depth:
                if source.startswith("/*", end):
                    depth += 1
                    end += 2
                elif source.startswith("*/", end):
                    depth -= 1
                    end += 2
                else:
                    end += 1
            _mask_range(chars, index, end)
            index = end
            continue

        raw = RAW_STRING_START_RE.match(source, index)
        if raw is not None:
            hashes = raw.group("hashes") or ""
            terminator = '"' + hashes
            content_start = raw.end()
            close = source.find(terminator, content_start)
            end = length if close < 0 else close + len(terminator)
            _mask_range(chars, index, end)
            index = end
            continue

        string_prefix = 0
        if source.startswith(('b"', 'c"'), index):
            string_prefix = 1
        if source[index + string_prefix : index + string_prefix + 1] == '"':
            end = index + string_prefix + 1
            escaped = False
            while end < length:
                char = source[end]
                end += 1
                if escaped:
                    escaped = False
                elif char == "\\":
                    escaped = True
                elif char == '"':
                    break
            _mask_range(chars, index, end)
            index = end
            continue

        quote = index + 1 if source.startswith("b'", index) else index
        if quote < length and source[quote] == "'":
            # A bare `'name` without a closing quote is a lifetime or label,
            # not a character literal; leave it visible as Rust code.
            lifetime = LIFETIME_START_RE.match(source, quote)
            if lifetime is not None:
                after = lifetime.end()
                if after >= length or source[after] != "'":
                    index = after
                    continue
            end = quote + 1
            escaped = False
            while end < length and source[end] not in "\r\n":
                char = source[end]
                end += 1
                if escaped:
                    escaped = False
                elif char == "\\":
                    escaped = True
                elif char == "'":
                    break
            _mask_range(chars, index, end)
            index = end
            continue

        index += 1
    return "".join(chars)


def extract_symbol_codes(source: str, symbol: str) -> list[str]:
    """Return sanitized bodies for every declaration with the symbol's leaf."""

    sanitized = sanitize_rust(source)
    name = symbol.rsplit("::", 1)[-1]
    pattern = re.compile(FUNCTION_RE_TEMPLATE.format(name=re.escape(name)))
    bodies: list[str] = []
    for match in pattern.finditer(sanitized):
        opening = sanitized.find("{", match.end())
        semicolon = sanitized.find(";", match.end())
        if opening < 0 or (semicolon >= 0 and semicolon < opening):
            continue
        depth = 1
        index = opening + 1
        while index < len(sanitized) and depth:
            if sanitized[index] == "{":
                depth += 1
            elif sanitized[index] == "}":
                depth -= 1
            index += 1
        if depth:
            continue
        bodies.append(sanitized[opening + 1 : index - 1])
    return bodies


def extract_symbol_code(source: str, symbol: str) -> str | None:
    """Return one unique sanitized function body, rejecting leaf-name decoys."""

    bodies = extract_symbol_codes(source, symbol)
    return bodies[0] if len(bodies) == 1 else None


def function_exists(source: str, symbol: str) -> bool:
    """Find a Rust function declaration without accepting prose or literals."""

    return bool(extract_symbol_codes(source, symbol))


def validate_symbol_test_path(symbol: str, test_path: str, label: str) -> None:
    """Require the source symbol to agree with the canonical libtest path."""

    path_pattern = re.compile(r"[A-Za-z_][A-Za-z0-9_]*(?:::[A-Za-z_][A-Za-z0-9_]*)*")
    if not path_pattern.fullmatch(test_path):
        raise ValidationError(
            f"{label}.test_path must be an exact canonical Rust test path, got {test_path!r}"
        )
    symbol_parts = symbol.split("::")
    path_parts = test_path.split("::")
    if path_parts[-len(symbol_parts) :] == symbol_parts:
        return
    # The runtime-matrix source helper is executed by a generated per-runtime
    # leaf (`helper::tokio` or `helper::smol`). The exact leaf remains pinned;
    # this rule only proves the source helper name is the generated module.
    if (
        len(path_parts) >= 2
        and path_parts[-1] in {"tokio", "smol"}
        and path_parts[-2] == symbol_parts[-1]
    ):
        return
    raise ValidationError(
        f"{label}: source symbol {symbol!r} does not agree with canonical test path "
        f"{test_path!r}"
    )


def command_display(command: list[str]) -> str:
    return " ".join(shlex.quote(part) for part in command)


def run_command(root: Path, command: list[str], test_paths: dict[str, str]) -> None:
    """Run one grouped cargo command and prove every exact mapped path executed.

    A green exit code is not enough: a filter that matches nothing, a test that
    is `#[ignore]`d, and a module rename that empties the filter all exit 0. So
    the captured libtest output must show a non-empty run, and every mapped
    symbol must appear as an executed (`ok`) test — not filtered out, not
    ignored.
    """

    test_ids = list(test_paths)
    print(
        f"[behavioral-parity] running {', '.join(test_ids)}: {command_display(command)}",
        flush=True,
    )
    result = subprocess.run(
        command,
        cwd=root,
        check=False,
        stdout=subprocess.PIPE,
        stderr=subprocess.STDOUT,
        text=True,
    )
    output = result.stdout or ""
    # Surface the captured output so CI logs still show the underlying run.
    print(output, end="", flush=True)
    if result.returncode:
        raise ValidationError(
            f"mapped v2 target failed ({', '.join(test_ids)}): exit {result.returncode}"
        )

    if not any(int(match.group("count")) > 0 for match in RUNNING_RE.finditer(output)):
        raise ValidationError(
            f"mapped v2 target ran zero tests: {command_display(command)}; the name filter "
            "selected nothing (a renamed or moved test/module is the usual cause)"
        )

    executed: dict[str, list[str]] = {}
    for match in TEST_LINE_RE.finditer(output):
        executed.setdefault(match.group("path"), []).append(match.group("status"))

    for test_id, test_path in test_paths.items():
        statuses = executed.get(test_path, [])
        if "ok" in statuses:
            continue
        if "ignored" in statuses:
            raise ValidationError(
                f"mapped test path {test_path!r} ({test_id}) was ignored, not executed; an "
                "#[ignore] on a mapped test makes the gate certify a test that never runs"
            )
        raise ValidationError(
            f"mapped canonical test path {test_path!r} ({test_id}) did not report exact "
            f"status 'ok' under {command_display(command)}; suffix/module collisions, "
            "renames, moves, and filtered-out targets are not accepted"
        )


def reject_placeholder(value: str, label: str) -> None:
    if any(word in value.lower() for word in PENDING_WORDS):
        raise ValidationError(f"{label}: placeholder value {value!r} is not allowed")


def validate_configuration(
    source: str, symbol: str, test: dict[str, Any], label: str
) -> None:
    """Check recorded configuration against sanitized executable code evidence.

    This is audited traceability, not a semantic model of Rust or VISCA. It
    deliberately rejects comments and literals as evidence, requires concrete
    code identifiers for profile/envelope claims, and requires a validator-owned
    code marker associated with the row's separately pinned receipt class.

    A maintainer must still inspect what those identifiers do. The gate's job is
    to keep that reviewed source/test/path/configuration chain from silently
    drifting or being satisfied by explanatory prose.
    """

    envelope = require_string(test.get("envelope"), f"{label}.envelope")
    reject_placeholder(envelope, f"{label}.envelope")
    if envelope not in VALID_ENVELOPES:
        raise ValidationError(
            f"{label}.envelope must be one of {sorted(VALID_ENVELOPES)}, got {envelope!r}"
        )

    receipt_class = require_string(test.get("receipt_class"), f"{label}.receipt_class")
    reject_placeholder(receipt_class, f"{label}.receipt_class")
    if not CONFIG_TOKEN_RE.fullmatch(receipt_class):
        raise ValidationError(
            f"{label}.receipt_class must be a lowercase token, got {receipt_class!r}"
        )
    if receipt_class not in VALID_RECEIPT_CLASSES:
        raise ValidationError(
            f"{label}.receipt_class {receipt_class!r} is not a known receipt class; add it to "
            "VALID_RECEIPT_CLASSES if it is a deliberate new class"
        )

    profile = require_string(test.get("profile"), f"{label}.profile")
    reject_placeholder(profile, f"{label}.profile")

    code = extract_symbol_code(source, symbol)
    if code is None:
        raise ValidationError(
            f"{label}: body of {symbol!r} is missing or its leaf function name is not unique; "
            "a same-named decoy cannot supply configuration evidence"
        )
    has_sony_code = SONY_CODE_RE.search(code) is not None
    has_raw_code = RAW_CODE_RE.search(code) is not None

    evidence_patterns = RECEIPT_EVIDENCE_PATTERNS.get(receipt_class, ())
    if not evidence_patterns or not any(
        re.search(pattern, code) is not None for pattern in evidence_patterns
    ):
        raise ValidationError(
            f"{label}: body of {symbol!r} has no sanitized code evidence for pinned "
            f"receipt class {receipt_class!r}; comments, strings, raw strings, and char "
            "literals do not count"
        )

    if profile == "n/a":
        # Engine-level rows name their envelope constructor directly. Neutral
        # encode/decode/trace tests intentionally make no envelope claim.
        if envelope == "sony" and not has_sony_code:
            raise ValidationError(
                f"{label}: envelope 'sony' but the body of {symbol!r} carries no Sony code evidence"
            )
        if envelope == "raw" and not has_raw_code:
            raise ValidationError(
                f"{label}: envelope 'raw' but the body of {symbol!r} carries no raw code evidence"
            )
        if envelope == "raw" and has_sony_code:
            raise ValidationError(
                f"{label}: envelope 'raw' but the body of {symbol!r} uses Sony code; "
                "a raw row must not run a Sony-only test"
            )
        return

    if not PROFILE_TYPE_RE.fullmatch(profile):
        raise ValidationError(
            f"{label}.profile must be a concrete profile type (e.g. SonyFR7) or 'n/a', "
            f"got {profile!r}"
        )
    is_sony_profile = profile.startswith("Sony")
    if is_sony_profile != (envelope == "sony"):
        raise ValidationError(
            f"{label}: envelope/profile disagree (envelope={envelope!r}, profile={profile!r}); "
            "a Sony profile requires envelope 'sony' and vice versa"
        )
    if re.search(r"\b" + re.escape(profile) + r"\b", code) is None:
        raise ValidationError(
            f"{label}: profile {profile!r} does not appear as code in the body of "
            f"{symbol!r}; comments and literals are not configuration evidence"
        )
    if envelope == "sony" and not has_sony_code:
        raise ValidationError(
            f"{label}: envelope 'sony' but the body of {symbol!r} carries no Sony code evidence"
        )
    if envelope == "raw" and not has_raw_code:
        raise ValidationError(
            f"{label}: envelope 'raw' but the body of {symbol!r} carries no raw code evidence"
        )
    if envelope == "raw" and has_sony_code:
        raise ValidationError(
            f"{label}: envelope 'raw' but the body of {symbol!r} uses Sony code; "
            "a raw row must not run a Sony-only test"
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
    if commit != EXPECTED_ORACLE_COMMIT:
        raise ValidationError(
            "manifest.oracle.commit must match validator-pinned oracle "
            f"{EXPECTED_ORACLE_COMMIT}, got {commit}; changing the historical base "
            "requires validator review, not a manifest-only rebaseline"
        )

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

    # The required family set is pinned in this validator, so shrinking coverage
    # takes a validator edit, not a lone manifest edit.
    declared_family_set = set(required_family_names)
    dropped_families = EXPECTED_REQUIRED_FAMILIES - declared_family_set
    if dropped_families:
        raise ValidationError(
            "manifest.required_families is missing families pinned in the validator: "
            + ", ".join(sorted(dropped_families))
            + " (removing a required family must also edit EXPECTED_REQUIRED_FAMILIES)"
        )
    added_families = declared_family_set - EXPECTED_REQUIRED_FAMILIES
    if added_families:
        raise ValidationError(
            "manifest.required_families adds families not pinned in the validator: "
            + ", ".join(sorted(added_families))
            + " (add them to EXPECTED_REQUIRED_FAMILIES so the set stays reviewed)"
        )

    approved_changes_value = corpus.get("approved_intentional_changes", [])
    if not isinstance(approved_changes_value, list):
        raise ValidationError("manifest.approved_intentional_changes must be a list")
    approved_changes = {
        validate_id(change, "manifest.approved_intentional_changes entry")
        for change in approved_changes_value
    }

    # A waiver cannot approve itself inside this file alone. Every approved
    # intentional-change id must also appear verbatim in the changelog, so a new
    # waiver forces a reviewed, user-visible CHANGELOG entry naming what it
    # supersedes rather than a self-blessed manifest line.
    changelog_path = root / "CHANGELOG.md"
    try:
        changelog_text = changelog_path.read_text(encoding="utf-8")
    except OSError as exc:
        raise ValidationError(f"cannot read CHANGELOG.md to verify waivers: {exc}") from exc
    for change_id in sorted(approved_changes):
        if change_id not in changelog_text:
            raise ValidationError(
                f"approved intentional change {change_id!r} is not documented in CHANGELOG.md; "
                "an approved waiver must appear verbatim in the changelog so it is reviewed there"
            )

    if set(PINNED_V2_TARGETS) != set(PINNED_V2_CONTEXT):
        raise ValidationError(
            "validator bug: PINNED_V2_TARGETS and PINNED_V2_CONTEXT must pin the same IDs"
        )

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
        pinned = PINNED_V2_TARGETS.get(test_id)
        if pinned is None:
            raise ValidationError(
                f"{label}: v2 target {test_id!r} is not pinned in PINNED_V2_TARGETS; "
                "new mappings require validator review, not a manifest-only edit"
            )
        pinned_file, pinned_symbol, pinned_test_path, pinned_receipt_class = pinned
        pinned_envelope, pinned_profile, pinned_command_key = PINNED_V2_CONTEXT[test_id]
        pinned_command = PINNED_COMMANDS[pinned_command_key]
        path = safe_relative_path(root, test.get("file"), f"{label}.file")
        if path.as_posix() != pinned_file:
            raise ValidationError(
                f"{label}.file must match validator-pinned source {pinned_file!r}, "
                f"got {path.as_posix()!r}"
            )
        source_path = root / path
        if not source_path.is_file():
            raise ValidationError(f"{label}: v2 source file does not exist: {path}")
        symbol = require_string(test.get("symbol"), f"{label}.symbol")
        if symbol != pinned_symbol:
            raise ValidationError(
                f"{label}.symbol must match validator-pinned symbol {pinned_symbol!r}, "
                f"got {symbol!r}"
            )
        test_path = require_string(test.get("test_path"), f"{label}.test_path")
        if test_path != pinned_test_path:
            raise ValidationError(
                f"{label}.test_path must match validator-pinned canonical path "
                f"{pinned_test_path!r}, got {test_path!r}"
            )
        validate_symbol_test_path(symbol, test_path, label)
        if test.get("receipt_class") != pinned_receipt_class:
            raise ValidationError(
                f"{label}.receipt_class must match validator-pinned class "
                f"{pinned_receipt_class!r}, got {test.get('receipt_class')!r}"
            )
        if test.get("envelope") != pinned_envelope:
            raise ValidationError(
                f"{label}.envelope must match validator-pinned envelope "
                f"{pinned_envelope!r}, got {test.get('envelope')!r}"
            )
        if test.get("profile") != pinned_profile:
            raise ValidationError(
                f"{label}.profile must match validator-pinned profile "
                f"{pinned_profile!r}, got {test.get('profile')!r}"
            )
        source = source_path.read_text(encoding="utf-8")
        if not function_exists(source, symbol):
            raise ValidationError(
                f"{label}: v2 symbol {symbol!r} is missing from {path}; update the mapping"
            )
        validate_configuration(source, symbol, test, label)
        command_value = require_list(test.get("command"), f"{label}.command")
        if not all(isinstance(part, str) and part for part in command_value):
            raise ValidationError(f"{label}.command must contain non-empty strings")
        command = [str(part) for part in command_value]
        if tuple(command) != pinned_command:
            raise ValidationError(
                f"{label}.command must match validator-pinned command "
                f"{command_display(list(pinned_command))}, got {command_display(command)}"
            )
        if command[0] != "cargo" or "test" not in command[1:]:
            raise ValidationError(
                f"{label}.command must directly invoke cargo test, got {command_display(command)}"
            )
        v2_tests[test_id] = test

    missing_pinned_targets = set(PINNED_V2_TARGETS) - set(v2_tests)
    if missing_pinned_targets:
        raise ValidationError(
            "manifest is missing validator-pinned v2 targets: "
            + ", ".join(sorted(missing_pinned_targets))
        )

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
            test_paths = {
                test_id: str(v2_tests[test_id]["test_path"]) for test_id in test_ids
            }
            run_command(root, list(command), test_paths)
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
        f"{EXPECTED_ORACLE_COMMIT}"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
