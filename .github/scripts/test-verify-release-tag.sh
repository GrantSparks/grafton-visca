#!/usr/bin/env bash
set -euo pipefail

script_directory="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
helper="${script_directory}/verify-release-tag.sh"
fixture_directory="$(mktemp -d)"
trap 'rm -rf -- "${fixture_directory}"' EXIT

remote_repository="${fixture_directory}/remote.git"
source_repository="${fixture_directory}/source"
release_tag="v2.0.0-rc.1"

git init --bare --quiet "${remote_repository}"
git init --quiet "${source_repository}"
git -C "${source_repository}" config user.email release-test@example.invalid
git -C "${source_repository}" config user.name "Release tag helper test"
printf 'first release commit\n' > "${source_repository}/payload"
git -C "${source_repository}" add payload
git -C "${source_repository}" commit --quiet -m initial
git -C "${source_repository}" remote add origin "${remote_repository}"
git -C "${source_repository}" tag -a "${release_tag}" -m "annotated release tag"
annotated_tag_object="$(git -C "${source_repository}" rev-parse "refs/tags/${release_tag}")"
git -C "${source_repository}" push --quiet origin HEAD:refs/heads/main "refs/tags/${release_tag}"

run_helper() {
    (cd "${source_repository}" && "${helper}" "${release_tag}" "${annotated_tag_object}" "${remote_repository}")
}

expect_failure() {
    local description="$1"
    shift
    if "$@" >/dev/null 2>&1; then
        echo "expected ${description} to fail" >&2
        exit 1
    fi
}

# Matching annotated object.
run_helper

# A force-moved annotated tag must not pass against the original object.
printf 'second release commit\n' >> "${source_repository}/payload"
git -C "${source_repository}" add payload
git -C "${source_repository}" commit --quiet -m moved
git -C "${source_repository}" tag --force -a "${release_tag}" -m "moved annotated release tag"
git -C "${source_repository}" push --quiet --force origin "refs/tags/${release_tag}"
expect_failure "a moved annotated tag" run_helper

# A lightweight tag resolving to the commit must not be confused with the
# original annotated-tag object.
git -C "${source_repository}" tag --force "${release_tag}"
git -C "${source_repository}" push --quiet --force origin "refs/tags/${release_tag}"
expect_failure "a lightweight tag" run_helper

# A missing remote ref is also a hard failure.
git -C "${source_repository}" push --quiet origin ":refs/tags/${release_tag}"
expect_failure "a missing tag" run_helper

# The helper owns the strict input grammar too; a fully qualified ref is not a
# valid release-tag argument.
expect_failure "a fully qualified ref passed as the tag" \
    bash -c 'cd "$1" && "$2" "refs/tags/v2.0.0-rc.1" "$3" "$4"' \
    bash "${source_repository}" "${helper}" "${annotated_tag_object}" "${remote_repository}"

echo "verify-release-tag self-test passed"
