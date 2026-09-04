#!/usr/bin/env bash
set -euo pipefail

script_directory="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
helper="${script_directory}/verify-crate-payload.sh"
fixture_directory="$(mktemp -d)"
trap 'rm -rf "${fixture_directory}"' EXIT

crate_name="grafton-visca"
crate_version="2.0.0-rc.1"
local_payload="${fixture_directory}/${crate_name}-${crate_version}.crate"
registry_directory="${fixture_directory}/registry/${crate_name}/${crate_version}"
mkdir -p "${registry_directory}"
printf 'fixture crate payload\n' > "${local_payload}"
cp -- "${local_payload}" "${registry_directory}/download"

"${helper}" \
    "${crate_name}" \
    "${crate_version}" \
    "${local_payload}" \
    "file://${fixture_directory}/registry"

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

printf 'different fixture crate payload\n' > "${registry_directory}/download"
if "${helper}" \
    "${crate_name}" \
    "${crate_version}" \
    "${local_payload}" \
    "file://${fixture_directory}/registry"; then
    echo "expected a mismatched registry payload to fail" >&2
    exit 1
fi

echo "verify-crate-payload self-test passed"
