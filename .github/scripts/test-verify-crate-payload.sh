#!/usr/bin/env bash
set -euo pipefail

script_directory="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
helper="${VERIFY_CRATE_PAYLOAD_HELPER:-${script_directory}/verify-crate-payload.sh}"
source "${script_directory}/expect-failure.sh"
fixture_directory="$(mktemp -d)"
trap 'rm -rf "${fixture_directory}"' EXIT

crate_name="grafton-visca"
crate_version="2.0.0-rc.1"
local_payload="${fixture_directory}/${crate_name}-${crate_version}.crate"
registry_directory="${fixture_directory}/registry/${crate_name}/${crate_version}"
crate_directory="${fixture_directory}/package"
crate_root="${crate_name}-${crate_version}"
mkdir -p "${crate_directory}/${crate_root}/src" "${registry_directory}"
printf 'pub struct Example;\n' > "${crate_directory}/${crate_root}/src/lib.rs"
printf '[package]\nname = "%s"\nversion = "%s"\n' "${crate_name}" "${crate_version}" \
    > "${crate_directory}/${crate_root}/Cargo.toml"
printf '{"git":{"sha1":"00000000"},"path_in_vcs":""}\n' \
    > "${crate_directory}/${crate_root}/.cargo_vcs_info.json"
printf 'version = 4\n' > "${crate_directory}/${crate_root}/Cargo.lock"

pack_fixture() {
    local destination="$1"
    shift
    tar -C "${crate_directory}" --create --gzip --file "${destination}" "${crate_root}" "$@"
}

verify_fixture() {
    "${helper}" "${crate_name}" "${crate_version}" "${local_payload}" \
        "file://${fixture_directory}/registry"
}

pack_fixture "${local_payload}"
pack_fixture "${registry_directory}/download"
verify_fixture

expected_user_agent="grafton-visca-release-verifier/2.0.0 (+https://github.com/GrantSparks/grafton-visca)"
fake_curl_directory="${fixture_directory}/fake-curl"
user_agent_record="${fixture_directory}/user-agent"
mkdir -p "${fake_curl_directory}"
cat > "${fake_curl_directory}/curl" <<'EOF'
#!/usr/bin/env bash
set -euo pipefail

user_agent=""
output=""
while (( $# > 0 )); do
    case "$1" in
        --user-agent|--output|--retry)
            if (( $# < 2 )); then
                exit 2
            fi
            if [[ "$1" == "--user-agent" ]]; then
                user_agent="$2"
            elif [[ "$1" == "--output" ]]; then
                output="$2"
            fi
            shift 2
            ;;
        --fail|--location|--silent|--show-error)
            shift
            ;;
        *)
            shift
            ;;
    esac
done

[[ "${user_agent}" == "${TEST_EXPECTED_USER_AGENT}" ]]
[[ -n "${output}" ]]
printf '%s\n' "${user_agent}" > "${TEST_USER_AGENT_RECORD}"
cp -- "${TEST_DOWNLOAD_PAYLOAD}" "${output}"
EOF
chmod +x "${fake_curl_directory}/curl"
TEST_DOWNLOAD_PAYLOAD="${registry_directory}/download" \
TEST_EXPECTED_USER_AGENT="${expected_user_agent}" \
TEST_USER_AGENT_RECORD="${user_agent_record}" \
PATH="${fake_curl_directory}:${PATH}" \
"${helper}" \
    "${crate_name}" \
    "${crate_version}" \
    "${local_payload}" \
    "file://${fixture_directory}/registry"
if [[ "$(<"${user_agent_record}")" != "${expected_user_agent}" ]]; then
    echo "expected the payload verifier to send its descriptive User-Agent" >&2
    exit 1
fi

# A fresh library lockfile changes the archive SHA-256 without changing its
# source and metadata.
printf 'version = 4\n# Re-resolved dependency versions.\n' > "${crate_directory}/${crate_root}/Cargo.lock"
pack_fixture "${registry_directory}/download"
verify_fixture

for path in src/lib.rs Cargo.toml .cargo_vcs_info.json; do
    file="${crate_directory}/${crate_root}/${path}"
    cp -- "${file}" "${fixture_directory}/original"
    printf 'changed\n' >> "${file}"
    pack_fixture "${registry_directory}/download"
    expect_failure "registry file differs: ${path}" verify_fixture
    mv -- "${fixture_directory}/original" "${file}"
done

touch "${crate_directory}/${crate_root}/src/extra"
pack_fixture "${registry_directory}/download"
expect_failure "unexpected registry file: src/extra" verify_fixture
rm -- "${crate_directory}/${crate_root}/src/extra"
mv -- "${crate_directory}/${crate_root}/src/lib.rs" "${fixture_directory}/original"
pack_fixture "${registry_directory}/download"
expect_failure "missing registry file: src/lib.rs" verify_fixture
mv -- "${fixture_directory}/original" "${crate_directory}/${crate_root}/src/lib.rs"

touch "${crate_directory}/outside.txt"
pack_fixture "${registry_directory}/download" outside.txt
expect_failure "entry 'outside.txt' is outside package directory" verify_fixture

ln -s ../Cargo.toml "${crate_directory}/${crate_root}/src/link"
pack_fixture "${registry_directory}/download"
expect_failure "unsupported package entry: '${crate_root}/src/link'" verify_fixture

# Prove an unrelated command failure cannot stand in for a payload mismatch.
expect_failure "failure did not contain: expected-payload-mismatch" \
    expect_failure expected-payload-mismatch \
    bash -c 'echo unrelated-download-failure >&2; exit 1'

echo "verify-crate-payload self-test passed"
