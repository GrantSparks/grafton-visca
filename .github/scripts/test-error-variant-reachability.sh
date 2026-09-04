#!/usr/bin/env bash
set -euo pipefail

repository_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/../.." && pwd)"
checker="${repository_root}/.github/scripts/check-error-variant-reachability.sh"

bash "${checker}"

fixture="$(mktemp -d)"
trap 'rm -rf -- "${fixture}"' EXIT
cp -R "${repository_root}/src" "${fixture}/src"

# Leave the engine's production `matches!(..., Error::SyntaxError)` reference
# intact while deleting the actual protocol constructor. The old textual gate
# accepted this fixture; the explicit inventory must reject it (#736).
sed -i 's/0x02 => Self::SyntaxError,/0x02 => Self::Unknown(code),/' \
  "${fixture}/src/error.rs"

if ERROR_REACHABILITY_REPOSITORY_ROOT="${fixture}" bash "${checker}" >/dev/null 2>&1; then
  echo "Error reachability gate accepted a pattern match without a constructor" >&2
  exit 1
fi

echo "Error reachability mutation fixture passed."
