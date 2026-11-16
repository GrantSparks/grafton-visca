## Problem and resulting behavior

Describe the concrete problem and what callers observe after this change.

## Validation

List the checks run, their results, and any remaining failure with its exact
command and test name. Include movement or lifecycle considerations when relevant.

## Checklist

- [ ] The declared feature matrix passes: `bash .github/scripts/test-all-features.sh`.
- [ ] Supported feature shapes have no Clippy warnings: `bash .github/scripts/test-all-features.sh clippy`.
- [ ] Rustdoc builds without warnings across supported shapes: `bash .github/scripts/test-all-features.sh doc`.
- [ ] The pinned formatter passes: `cargo +nightly-2026-08-26 fmt --all -- --check`.
- [ ] Relevant blocking, async and all-feature doctests and compile contracts pass.
- [ ] Documentation and examples describe the resulting API and behavior.
- [ ] Unreleased `CHANGELOG.md` entries describe the net caller-visible change and reference the issue; released sections are unchanged.
- [ ] Movement commands document physical safety and distinguish applied completion from settlement evidence.
- [ ] Any public API diff is intentional and reviewed.

## Camera profile contributions

Complete this section for a profile contribution; otherwise mark it not applicable.
See the [contribution guide](https://github.com/GrantSparks/grafton-visca/blob/main/CONTRIBUTING.md#camera-profile-support) and
[worked promotion path](https://github.com/GrantSparks/grafton-visca/blob/main/docs/camera_profile_support.md#worked-example-acme_camjson-to-a-registry-row).

- [ ] The contribution invariant holds: one profile row and evidence, plus an encoding variant with golden bytes only when an existing concept needs a different dialect. New public concepts are reviewed separately under vendor-neutral names.
- [ ] The complete `ProfileSpec` validates with `profile_spec check`; its fact keys and values match the registry row.
- [ ] Source or probe evidence identifies the exact model, firmware, transport, packets and limits for every grant. Decoder candidates alone do not authorize controls.
- [ ] Each grant has its checked `source!` reference and authored evidence entry; withheld or uncertain support stays conservative.
- [ ] Generated support docs and the source-key index were blessed with `GRAFTON_VISCA_BLESS=1`, their diff reviewed, and checks rerun without the variable.
- [ ] Any new encoding variant has reviewed independent literal golden bytes and selection/validation coverage.
- [ ] The change adds no unsubstantiated hardware-verification claim.
