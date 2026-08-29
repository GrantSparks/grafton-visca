# 1.x behavioral oracle corpus

`manifest.json` is the executable 1.x provenance gate for the 2.0 release.
It pins the exact 1.x Git object (`6c7a9d3783861189745536372c4d21de24d4252d`),
records the old source tests that define each preserved behavior, and maps each
row to a current v2 production-path test or normative fixture.

Run the gate from any directory in a non-shallow checkout with:

```text
bash .github/scripts/validate-behavioral-parity.sh
```

The validator checks the pinned object with `git show`, verifies every old
source symbol and every v2 symbol, then groups and runs the mapped `cargo test`
commands. `--skip-tests` is available for a quick provenance-only check while
editing the manifest; CI never uses it.

The corpus is behavior-only. It does not require 1.x public names or source
compatibility, and it does not replace the v2 protocol/lifecycle fixtures. A
row may be marked `intentional-change` only when its `approved_change` appears
in the manifest's explicit `approved_intentional_changes` list; otherwise the
validator rejects it.
