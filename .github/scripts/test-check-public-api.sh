#!/usr/bin/env bash
# Exercise source comparison and failure handling without compiling rustdoc.
set -euo pipefail

script_dir="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
checker="${script_dir}/check-public-api.sh"
fixture="$(mktemp -d)"
trap 'rm -rf -- "${fixture:?}"' EXIT
mkdir -p "${fixture}/src" "${fixture}/bin" "${fixture}/cargo-args"
git -C "${fixture}" init -q
git -C "${fixture}" config user.name "API Fixture"
git -C "${fixture}" config user.email "api@example.invalid"
echo 'pub struct Original;' >"${fixture}/src/lib.rs"
echo '# Changes' >"${fixture}/CHANGELOG.md"
cat >"${fixture}/Cargo.toml" <<'TOML'
[package]
name = "public-api-fixture"
version = "0.0.0"
edition = "2021"

[features]
blocking = []
runtime-tokio = []
dyn-api = []
TOML
git -C "${fixture}" add src CHANGELOG.md Cargo.toml
git -C "${fixture}" commit -qm 'test: establish API base'
base="$(git -C "${fixture}" rev-parse HEAD)"

cat >"${fixture}/bin/cargo" <<'SH'
#!/usr/bin/env bash
set -euo pipefail
[[ "$*" == '+nightly-2026-08-26 public-api --color never --simplified '* ]]
side=head
if [[ "$PWD" == */base-source.* ]]; then
    side=base
fi
printf '%s\n' "$@" >"${API_FIXTURE_ARGS_DIR}/${side}.args"
printf '%s\n' "${CARGO_TARGET_DIR:?}" >"${API_FIXTURE_ARGS_DIR}/${side}.target"
# Simulate a path-dependency artifact left by the first source revision. The
# archived base must never see an artifact produced by the working tree.
mkdir -p "${CARGO_TARGET_DIR}"
if [[ -f "${CARGO_TARGET_DIR}/fixture-owner" ]]; then
    read -r owner <"${CARGO_TARGET_DIR}/fixture-owner"
    if [[ "${owner}" != "${side}" ]]; then
        echo 'head and base reused the same compiler artifact directory' >&2
        exit 43
    fi
fi
printf '%s\n' "${side}" >"${CARGO_TARGET_DIR}/fixture-owner"
if [[ -n "${API_FIXTURE_FAIL_BASE:-}" && "$PWD" == */base-source.* ]]; then
    exit 42
fi
if [[ -z "${API_FIXTURE_EMPTY:-}" ]]; then
    cat src/lib.rs
fi
SH
chmod +x "${fixture}/bin/cargo"
export PATH="${fixture}/bin:${PATH}"
export API_FIXTURE_ARGS_DIR="${fixture}/cargo-args"
cd "${fixture}"

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

assert_cargo_args() {
    local side="$1"
    shift
    printf '%s\n' +nightly-2026-08-26 public-api --color never --simplified \
        "$@" >"${fixture}/expected.args"
    diff -u "${fixture}/expected.args" "${API_FIXTURE_ARGS_DIR}/${side}.args"
}

assert_cargo_build_dirs() {
    local base_commit="$1" surface="$2"
    printf '%s\n' "${fixture}/target/public-api/${surface}/build/head" \
        >"${fixture}/expected.target"
    diff -u "${fixture}/expected.target" "${API_FIXTURE_ARGS_DIR}/head.target"
    printf '%s\n' "${fixture}/target/public-api/${surface}/build/base-${base_commit}" \
        >"${fixture}/expected.target"
    diff -u "${fixture}/expected.target" "${API_FIXTURE_ARGS_DIR}/base.target"
}

# Each surface retains its complete argument vector, including the legacy
# async surface when both source revisions still declare dyn-api.
for surface in blocking async all-features; do
    rm -f "${API_FIXTURE_ARGS_DIR}/"*.args
    bash "${checker}" "${base}" "${surface}"
    test ! -s "target/public-api/${surface}/changes.diff"
    case "${surface}" in
        blocking) args=(--no-default-features --features blocking) ;;
        async) args=(--no-default-features --features runtime-tokio,dyn-api) ;;
        all-features) args=(--all-features) ;;
    esac
    assert_cargo_args head "${args[@]}"
    assert_cargo_args base "${args[@]}"
    assert_cargo_build_dirs "${base}" "${surface}"
done

# HEAD has removed the feature, while its archived base still needs it to
# expose the old API. Similar-looking comments and metadata are not features.
cat >Cargo.toml <<'TOML'
[package]
name = "public-api-fixture"
version = "0.0.0"
edition = "2021"

# dyn-api was removed from the public feature vocabulary.
[package.metadata.compatibility]
dyn-api = "retired"

[features]
blocking = []
runtime-tokio = []
TOML
rm -f "${API_FIXTURE_ARGS_DIR}/"*.args
bash "${checker}" "${base}" async
assert_cargo_args head --no-default-features --features runtime-tokio
assert_cargo_args base --no-default-features --features runtime-tokio,dyn-api
assert_cargo_build_dirs "${base}" async
git add Cargo.toml
git commit -qm 'test: remove obsolete API feature'
modern_base="$(git rev-parse HEAD)"

# The same script works after the base has also removed the feature.
rm -f "${API_FIXTURE_ARGS_DIR}/"*.args
bash "${checker}" "${modern_base}" async
assert_cargo_args head --no-default-features --features runtime-tokio
assert_cargo_args base --no-default-features --features runtime-tokio
assert_cargo_build_dirs "${modern_base}" async

# Reverse the feature declarations to prove neither side borrows the other's
# selection. This also covers comparisons of older working trees.
git show "${base}:Cargo.toml" >Cargo.toml
rm -f "${API_FIXTURE_ARGS_DIR}/"*.args
bash "${checker}" "${modern_base}" async
assert_cargo_args head --no-default-features --features runtime-tokio,dyn-api
assert_cargo_args base --no-default-features --features runtime-tokio
assert_cargo_build_dirs "${modern_base}" async
git show "${modern_base}:Cargo.toml" >Cargo.toml

# A malformed source manifest must fail rather than silently dropping the
# legacy feature. No cargo invocation may run after that parse failure.
printf '[features\n' >Cargo.toml
rm -f "${API_FIXTURE_ARGS_DIR}/"*.args
expect_failure 1 bash "${checker}" "${base}" async
test ! -e "${API_FIXTURE_ARGS_DIR}/head.args"
test ! -e "${API_FIXTURE_ARGS_DIR}/base.args"
git show "${modern_base}:Cargo.toml" >Cargo.toml

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
expect_failure 2 bash "${checker}" "${base}" tokio-dyn
if compgen -G 'target/public-api/*/base-source.*' >/dev/null; then
    echo 'temporary source archive was not removed' >&2
    exit 1
fi
echo 'public API comparison regression checks passed'
