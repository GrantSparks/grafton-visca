#!/usr/bin/env bash
set -euo pipefail

usage() {
    echo "usage: $0 RELEASE_TAG EXPECTED_ANNOTATED_TAG_OBJECT [REMOTE]" >&2
    exit 2
}

if (( $# < 2 || $# > 3 )); then
    usage
fi

release_tag="$1"
expected_tag_object="$2"
remote="${3:-origin}"

# Keep this predicate identical to the release workflow's input gate. The
# helper is called again at publication boundaries, so it must not acquire a
# broader ref grammar merely because the first check already ran.
if [[ ! "${release_tag}" =~ ^v(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)\.(0|[1-9][0-9]*)(-[0-9A-Za-z-]+(\.[0-9A-Za-z-]+)*)?$ ]]; then
    echo "release tag must match vX.Y.Z with an optional prerelease suffix" >&2
    exit 1
fi

tag_ref="refs/tags/${release_tag}"
# The strict grammar above admits only the short tag name; this is the one
# fully qualified ref passed to Git, never user-supplied ref syntax.

# GitHub's object format for this repository is SHA-1. Rejecting anything
# else avoids comparing a truncated or otherwise ambiguous object name.
if [[ ! "${expected_tag_object}" =~ ^[0-9a-f]{40}$ ]]; then
    echo "expected annotated tag object must be a 40-character lowercase object id" >&2
    exit 1
fi
if [[ -z "${remote}" || "${remote}" == *$'\n'* || "${remote}" == *$'\r'* ]]; then
    echo "remote must be a non-empty Git remote name or URL" >&2
    exit 1
fi

# The provenance step has already checked this object is annotated. Repeating
# that local check here makes every later boundary fail closed if the checkout
# or the carried expectation is not what the first step established.
expected_type="$(git cat-file -t "${expected_tag_object}" 2>/dev/null)" || {
    echo "expected annotated tag object ${expected_tag_object} is not present locally" >&2
    exit 1
}
if [[ "${expected_type}" != "tag" ]]; then
    echo "expected object ${expected_tag_object} is ${expected_type}, not an annotated tag" >&2
    exit 1
fi

remote_output="$(git ls-remote --refs "${remote}" "${tag_ref}")" || {
    echo "could not query ${remote} for ${tag_ref}" >&2
    exit 1
}
if [[ -z "${remote_output}" ]]; then
    echo "remote ${tag_ref} is missing" >&2
    exit 1
fi

# `ls-remote` should return one tab-separated row for an exact ref. Preserve
# every row (including a malformed blank row) and reject anything other than
# exactly one result instead of silently choosing the first line.
mapfile -t remote_rows <<< "${remote_output}"
if (( ${#remote_rows[@]} != 1 )); then
    echo "remote query for ${tag_ref} returned ${#remote_rows[@]} results; expected exactly one" >&2
    exit 1
fi

remote_line="${remote_rows[0]}"
if [[ "${remote_line}" != *$'\t'* ]]; then
    echo "remote query for ${tag_ref} returned an unparsable result" >&2
    exit 1
fi
remote_object="${remote_line%%$'\t'*}"
remote_ref="${remote_line#*$'\t'}"
if [[ ! "${remote_object}" =~ ^[0-9a-f]{40}$ || "${remote_ref}" != "${tag_ref}" ]]; then
    echo "remote query for ${tag_ref} returned an unexpected ref/object row" >&2
    exit 1
fi
if [[ "${remote_object}" != "${expected_tag_object}" ]]; then
    echo "remote ${tag_ref} object ${remote_object} does not match expected annotated object ${expected_tag_object}" >&2
    exit 1
fi

echo "verified remote ${tag_ref} annotated object ${remote_object} matches the expected local object"
