# Re-exported range-macro compile contract

`provider` depends on `grafton-visca` and re-exports only
`visca_range_type!`. `consumer` depends only on `provider` and invokes that
macro, so the final expansion cannot discover `grafton-visca`, `serde`,
`schemars`, or `ts-rs` in the consumer's manifest.

Run the consumer both without helper derives and with
`--features range-helper-derives`. The latter is the regression oracle: it
fails if the procedural adapter tries to resolve `grafton-visca` from the
final caller instead of using the defining declarative macro's hygienic
`$crate` path.

This is a compile contract under `api/2.0.0-rc.1/README.md` lines 65-74. The
hidden adapter is intentionally absent from rustdoc API snapshots, while the
downstream behavior of its expansion remains release-gated.
