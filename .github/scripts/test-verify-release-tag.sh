#!/usr/bin/env bash
set -euo pipefail

script_directory="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
helper="${VERIFY_RELEASE_TAG_HELPER:-${script_directory}/verify-release-tag.sh}"
source "${script_directory}/expect-failure.sh"
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
release_commit="$(git -C "${source_repository}" rev-parse HEAD)"
annotated_tag_object="$(git -C "${source_repository}" rev-parse "refs/tags/${release_tag}")"
git -C "${source_repository}" push --quiet origin HEAD:refs/heads/main "refs/tags/${release_tag}"

run_helper() {
    (cd "${source_repository}" && "${helper}" "${release_tag}" "${annotated_tag_object}" "${remote_repository}")
}

# Matching annotated object.
run_helper

# A force-moved annotated tag must not pass against the original object.
printf 'second release commit\n' >> "${source_repository}/payload"
git -C "${source_repository}" add payload
git -C "${source_repository}" commit --quiet -m moved
git -C "${source_repository}" tag --force -a "${release_tag}" -m "moved annotated release tag"
git -C "${source_repository}" push --quiet --force origin "refs/tags/${release_tag}"
expect_failure "does not match expected annotated object" run_helper

# A lightweight tag resolving to the commit must not be confused with the
# original annotated-tag object.
git -C "${source_repository}" tag --force "${release_tag}" "${release_commit}"
git -C "${source_repository}" push --quiet --force origin "refs/tags/${release_tag}"
expect_failure "does not match expected annotated object" run_helper || {
    echo "lightweight tag at the release commit was not rejected for its object identity" >&2
    exit 1
}

# A missing remote ref is also a hard failure.
git -C "${source_repository}" push --quiet origin ":refs/tags/${release_tag}"
expect_failure "remote refs/tags/${release_tag} is missing" run_helper

# The helper owns the strict input grammar too; a fully qualified ref is not a
# valid release-tag argument.
expect_failure "release tag must match vX.Y.Z" \
    bash -c 'cd "$1" && "$2" "refs/tags/v2.0.0-rc.1" "$3" "$4"' \
    bash "${source_repository}" "${helper}" "${annotated_tag_object}" "${remote_repository}"

# A non-v tag is deliberately not a release identity. It must remain outside
# the release CI tag pattern and the publication workflow's accepted grammar.
expect_failure "release tag must match vX.Y.Z" \
    bash -c 'cd "$1" && "$2" "qualification-v2.0.0-rc.1" "$3" "$4"' \
    bash "${source_repository}" "${helper}" "${annotated_tag_object}" "${remote_repository}"

echo "verify-release-tag self-test passed"
