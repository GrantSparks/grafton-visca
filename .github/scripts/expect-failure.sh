#!/usr/bin/env bash
# Sourced by regression scripts: failure is evidence only for the named reason.
expect_failure() {
    local expected_text="$1"
    shift
    local log_file status=0
    log_file="$(mktemp)"
    "$@" >"${log_file}" 2>&1 || status=$?
    if (( status == 0 )); then
        cat "${log_file}" >&2
        rm -f -- "${log_file}"
        echo "expected failure containing: ${expected_text}" >&2
        return 1
    fi
    if ! grep -Fq -- "${expected_text}" "${log_file}"; then
        cat "${log_file}" >&2
        rm -f -- "${log_file}"
        echo "failure did not contain: ${expected_text}" >&2
        return 1
    fi
    rm -f -- "${log_file}"
}
