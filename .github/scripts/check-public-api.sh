#!/usr/bin/env bash
# Generate both sides from source; keep listings and the review diff out of git.
set -euo pipefail

base_ref="${1:?usage: check-public-api.sh BASE_REF SURFACE}"
surface="${2:?usage: check-public-api.sh BASE_REF SURFACE}"
case "${surface}" in
    blocking) cargo_args=(--no-default-features --features blocking) ;;
    tokio-dyn) cargo_args=(--no-default-features --features runtime-tokio,dyn-api) ;;
    all-features) cargo_args=(--all-features) ;;
    *) echo "unknown public API surface: ${surface}" >&2; exit 2 ;;
esac

repo_root="$(git rev-parse --show-toplevel)"
base_commit="$(git rev-parse --verify "${base_ref}^{commit}")"
output="${repo_root}/target/public-api/${surface}"
mkdir -p "${output}"
rm -f -- "${output}/base.txt" "${output}/head.txt" "${output}/changes.diff"
base_source="$(mktemp -d "${output}/base-source.XXXXXX")"
trap 'rm -rf -- "${base_source:?}"' EXIT
git -C "${repo_root}" archive "${base_commit}" | tar -x -C "${base_source}"

generate() {
    local source="$1"
    local destination="$2"
    (
        cd "${source}"
        CARGO_TARGET_DIR="${output}/build" \
            cargo +nightly-2026-08-26 public-api --color never --simplified \
            "${cargo_args[@]}" >"${destination}"
    )
    test -s "${destination}"
}

generate "${repo_root}" "${output}/head.txt"
# Resolve the same dependency versions where the base manifest permits them.
if [[ -f "${repo_root}/Cargo.lock" ]]; then
    cp "${repo_root}/Cargo.lock" "${base_source}/Cargo.lock"
fi
generate "${base_source}" "${output}/base.txt"

if diff -u --label "${base_commit}/${surface}" --label "working-tree/${surface}" \
    "${output}/base.txt" "${output}/head.txt" >"${output}/changes.diff"; then
    echo "public API ${surface}: unchanged"
else
    result=$?
    [[ "${result}" == 1 ]] || exit "${result}"
    cat "${output}/changes.diff"
    if git -C "${repo_root}" diff --quiet "${base_commit}" -- CHANGELOG.md; then
        echo "public API changes require a CHANGELOG.md hunk in the same change" >&2
        exit 1
    else
        result=$?
        [[ "${result}" == 1 ]] || exit "${result}"
    fi
    echo "public API ${surface}: changes recorded; review target/public-api/${surface}/changes.diff"
fi
