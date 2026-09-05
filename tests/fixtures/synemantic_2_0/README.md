# Synemantic 2.0 downstream contract fixture

This is a local, reproducible downstream contract fixture for the
`2.0.0-rc.2` surface. Its grafton-visca feature set matches the Synemantic
native migration candidate: Tokio plus the async and Tokio-serial facades,
with serde, schemars, and ts-rs; Synemantic keeps its own erased camera facade
and does not enable grafton-visca's `dyn-api` feature. It checks the
owner-backed Tokio constructor's `Send` contract, noun access, profile
lowering, and the schema/serialization surface without requiring a camera or a
private sibling checkout.

The BRC-300 assertion is deliberately concrete: its source-backed base image
noun is required by Synemantic's profile-specific adapter. The picture-effect
test also guards the removed compatibility values, leaving model-specific
bytes explicitly represented as `Unknown`.

This fixture is not a build of an arbitrary external Synemantic branch. The
release's immutable paired-candidate procedure, including source replacement
before the RC is in the registry and registry-only verification afterward, is
specified in `RELEASING.md`.
