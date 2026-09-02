#!/usr/bin/env bash
set -euo pipefail

repository_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
cd "${repository_root}"

mapfile -t declared_variants < <(
  sed -n '/^pub enum Error {$/,/^}$/p' src/error.rs |
    sed -nE 's/^    ([A-Z][A-Za-z0-9_]*)([ ({,].*)?$/\1/p'
)

if ((${#declared_variants[@]} == 0)); then
  echo "No public Error variants were found in src/error.rs" >&2
  exit 1
fi

declare -A reachable=()

production_error_references() {
  if command -v rg >/dev/null 2>&1; then
    rg --no-filename --only-matching 'Error::[A-Z][A-Za-z0-9_]*' src \
      --glob '!src/error.rs' \
      --glob '!src/lib.rs' \
      --glob '!src/testing/**' \
      --glob '!**/tests.rs' \
      --glob '!**/tests/**'
    return
  fi

  # GitHub's runner images do not guarantee ripgrep. Keep the reachability
  # gate usable there with the same production-tree exclusions.
  find src -type f -name '*.rs' \
    ! -path 'src/error.rs' \
    ! -path 'src/lib.rs' \
    ! -path 'src/testing/*' \
    ! -name 'tests.rs' \
    ! -path '*/tests/*' \
    -exec grep -hoE 'Error::[A-Z][A-Za-z0-9_]*' {} + || true
}

# Count only the production tree. The enum implementation, crate-level docs,
# shipped test toolkit, and unit-test modules can all mention or manufacture a
# variant without proving that library behavior can return it (#722).
while IFS= read -r reference; do
  reachable["${reference#Error::}"]="production source"
done < <(
  production_error_references | sort -u
)

# These factories live beside the enum, so the broad search above excludes
# them along with the enum's pattern matches and doctests. Pin each actual
# construction expression explicitly instead of treating any self-reference
# in error.rs as reachability evidence.
record_internal_factory() {
  local variant="$1"
  local expression="$2"
  if ! grep -Fq -- "${expression}" src/error.rs; then
    echo "Missing Error::${variant} production factory: ${expression}" >&2
    exit 1
  fi
  reachable["${variant}"]="src/error.rs factory"
}

record_internal_factory MessageLengthError '0x01 => Self::MessageLengthError'
record_internal_factory Unknown '_ => Self::Unknown(code)'
record_internal_factory Io 'Self::Io(Arc::new(err))'
record_internal_factory ParseError 'Self::ParseError(Cow::Owned(err.to_string()))'
record_internal_factory InvalidResponseLength 'Self::InvalidResponseLength {'
record_internal_factory WithContext 'Self::WithContext {'

missing=()
for variant in "${declared_variants[@]}"; do
  if [[ -z "${reachable[${variant}]+present}" ]]; then
    missing+=("${variant}")
  fi
done

if ((${#missing[@]} != 0)); then
  echo "Public Error variants without a production construction site:" >&2
  printf '  Error::%s\n' "${missing[@]}" >&2
  echo "Delete each dead variant or add its real production path before the RC API locks." >&2
  exit 1
fi

printf 'Verified production reachability for %d public Error variants.\n' \
  "${#declared_variants[@]}"
