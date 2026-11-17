#!/usr/bin/env bash
# Generate both sides from source; keep listings and the review diff out of git.
set -euo pipefail

base_ref="${1:?usage: check-public-api.sh BASE_REF SURFACE}"
surface="${2:?usage: check-public-api.sh BASE_REF SURFACE}"
case "${surface}" in
    blocking | async | all-features) ;;
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
    local build_name="$3"
    local -a cargo_args
    case "${surface}" in
        blocking) cargo_args=(--no-default-features --features blocking) ;;
        async)
            # Generate each revision's complete async facade. Older revisions
            # expose runtime-profile views through dyn-api; the normal camera
            # surface includes those views after that feature is removed.
            # Read the exact source manifest rather than inferring the base's
            # features from HEAD or silently omitting the old dynamic API.
            local async_features
            async_features="$(python3 - "${source}/Cargo.toml" <<'PYFEATURES'
import sys
import tomllib

with open(sys.argv[1], "rb") as manifest:
    features = tomllib.load(manifest).get("features", {})
if not isinstance(features, dict):
    raise TypeError("Cargo manifest features must be a table")
print("runtime-tokio,dyn-api" if "dyn-api" in features else "runtime-tokio")
PYFEATURES
)"
            cargo_args=(--no-default-features --features "${async_features}")
            ;;
        all-features) cargo_args=(--all-features) ;;
    esac
    (
        cd "${source}"
        # Archived source timestamps may predate cached path-dependency
        # artifacts. Isolate the working tree and each immutable baseline so
        # Cargo cannot reuse a macro built from a different source revision.
        CARGO_TARGET_DIR="${output}/build/${build_name}" \
            cargo +nightly-2026-08-26 public-api --color never --simplified \
            "${cargo_args[@]}" >"${destination}"
    )
    test -s "${destination}"
}

generate "${repo_root}" "${output}/head.txt" head
# Resolve the same dependency versions where the base manifest permits them.
if [[ -f "${repo_root}/Cargo.lock" ]]; then
    cp "${repo_root}/Cargo.lock" "${base_source}/Cargo.lock"
fi
generate "${base_source}" "${output}/base.txt" "base-${base_commit}"

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
