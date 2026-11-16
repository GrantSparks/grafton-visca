#!/usr/bin/env bash
# Exercise source comparison and failure handling without compiling rustdoc.
set -euo pipefail

script_dir="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
checker="${script_dir}/check-public-api.sh"
fixture="$(mktemp -d)"
trap 'rm -rf -- "${fixture:?}"' EXIT
mkdir -p "${fixture}/src" "${fixture}/bin"
git -C "${fixture}" init -q
git -C "${fixture}" config user.name "API Fixture"
git -C "${fixture}" config user.email "api@example.invalid"
echo 'pub struct Original;' >"${fixture}/src/lib.rs"
echo '# Changes' >"${fixture}/CHANGELOG.md"
git -C "${fixture}" add src CHANGELOG.md
git -C "${fixture}" commit -qm 'test: establish API base'
base="$(git -C "${fixture}" rev-parse HEAD)"

cat >"${fixture}/bin/cargo" <<'SH'
#!/usr/bin/env bash
set -euo pipefail
[[ "$*" == '+nightly-2026-08-26 public-api --color never --simplified '* ]]
if [[ -n "${API_FIXTURE_FAIL_BASE:-}" && "$PWD" == */base-source.* ]]; then
    exit 42
fi
if [[ -z "${API_FIXTURE_EMPTY:-}" ]]; then
    cat src/lib.rs
fi
SH
chmod +x "${fixture}/bin/cargo"
export PATH="${fixture}/bin:${PATH}"
cd "${fixture}"

for surface in blocking tokio-dyn all-features; do
    bash "${checker}" "${base}" "${surface}"
    test ! -s "target/public-api/${surface}/changes.diff"
done

expect_failure() {
    local expected="$1"
    shift
    local result=0
    "$@" >"${fixture}/failure.log" 2>&1 || result=$?
    if [[ "${result}" != "${expected}" ]]; then
        cat "${fixture}/failure.log" >&2
        echo "expected status ${expected}, got ${result}" >&2
        exit 1
    fi
}

# The baseline comes from its commit, not from the modified working tree.
echo 'pub struct Changed;' >src/lib.rs
expect_failure 1 bash "${checker}" "${base}" blocking
grep -Eq 'public API changes require a CHANGELOG.md hunk' failure.log
grep -Eq '^\+pub struct Changed;' target/public-api/blocking/changes.diff
grep -Eq '^pub struct Original;' target/public-api/blocking/base.txt
echo 'Document the intended change.' >>CHANGELOG.md
bash "${checker}" "${base}" blocking

# Tool failures and empty output must fail even with a changelog entry.
export API_FIXTURE_FAIL_BASE=1
expect_failure 42 bash "${checker}" "${base}" blocking
unset API_FIXTURE_FAIL_BASE
test ! -e target/public-api/blocking/changes.diff
export API_FIXTURE_EMPTY=1
expect_failure 1 bash "${checker}" "${base}" blocking
unset API_FIXTURE_EMPTY
expect_failure 2 bash "${checker}" "${base}" unknown
if compgen -G 'target/public-api/*/base-source.*' >/dev/null; then
    echo 'temporary source archive was not removed' >&2
    exit 1
fi
echo 'public API comparison regression checks passed'
