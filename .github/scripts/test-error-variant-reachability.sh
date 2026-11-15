#!/usr/bin/env bash
set -euo pipefail

script_directory="$(cd -- "$(dirname -- "${BASH_SOURCE[0]}")" && pwd)"
repository_root="$(cd "${script_directory}/../.." && pwd)"
checker="${script_directory}/check-error-variant-reachability.sh"
source "${script_directory}/expect-failure.sh"

bash "${checker}"
fixture="$(mktemp -d)"
trap 'rm -rf -- "${fixture}"' EXIT

reset_fixture() {
    rm -rf -- "${fixture}/src"
    cp -R "${repository_root}/src" "${fixture}/src"
}
run_fixture() {
    ERROR_REACHABILITY_REPOSITORY_ROOT="${fixture}" bash "${checker}"
}

reset_fixture
# Leave all pattern matches intact; only remove the production constructor.
sed -i 's/0x02 => Self::SyntaxError,/0x02 => Self::Unknown(code),/' "${fixture}/src/error.rs"
expect_failure 'Error::SyntaxError production constructor occurs 0 times' run_fixture

reset_fixture
python3 - "${fixture}" <<'PY'
from pathlib import Path
import sys
root = Path(sys.argv[1])
p = root / 'src/error.rs'
s = p.read_text()
s = s.replace('Self::WithContext {\n            context: context.into(),\n            source: Box::new(self),\n        }', 'self')
p.write_text(s)
p = root / 'src/transport/address.rs'
s = p.read_text().replace('Error::InvalidAddress {\n        reason: reason.into(),\n    }', 'Error::Unknown(0)')
p.write_text(s)
PY
expect_failure 'Error::WithContext production constructor occurs 0 times' run_fixture
expect_failure 'Error::InvalidAddress production constructor occurs 0 times' run_fixture

reset_fixture
cat >> "${fixture}/src/error.rs" <<'RS'
// Self::WithContext { context: context.into(), source: Box::new(self), }
const INVENTORY_DECOY: &str = r#"Self::WithContext { context: context.into(), source: Box::new(self), }"#;
#[cfg(test)]
mod inventory_decoy {
    impl super::Error {
        fn decoy(self, context: impl Into<std::borrow::Cow<'static, str>>) -> Self {
            Self::WithContext { context: context.into(), source: Box::new(self), }
        }
    }
}
RS
# Test-only copies are irrelevant to a production-only uniqueness check.
run_fixture
cat >> "${fixture}/src/error.rs" <<'RS'
impl Error {
    fn duplicated_constructor(self, context: impl Into<std::borrow::Cow<'static, str>>) -> Self {
        Self::WithContext { context: context.into(), source: Box::new(self), }
    }
}
RS
expect_failure 'Error::WithContext production constructor occurs 2 times' run_fixture

# Exercise the shared scanner's literal and compound-cfg regression directly.
rustc +1.98.0 --edition=2021 --deny warnings --test \
    "${repository_root}/test-support/src/source_scan.rs" -o "${fixture}/scanner-tests"
"${fixture}/scanner-tests"
echo 'Error reachability mutation fixtures passed.'
