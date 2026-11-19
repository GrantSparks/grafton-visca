# Downstream 2.0 contract fixture

This fixture checks the `2.0.0` API with Tokio, the async and Tokio serial
facades, serde, schemars, and ts-rs. It exercises the owner-backed Tokio
constructor's `Send` contract, noun access, profile lowering, schema generation,
and serialization.

The BRC-300 assertion checks the base image noun used by profile-specific
adapters. The picture-effect test checks that model-specific bytes without
an assigned variant are represented as `Unknown`.

See [the release guide](../../../RELEASING.md#paired-downstream-candidate-validation)
for validating an exact downstream candidate against unpublished source and
then against the published packages.
