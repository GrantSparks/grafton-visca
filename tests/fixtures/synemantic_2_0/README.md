# Synemantic 2.0 downstream contract fixture

This is a local, reproducible downstream compile fixture for the
`2.0.0-rc.1` surface. It checks the final Tokio/dyn-api camera types together
with serde, schemars, and ts-rs integration without requiring a camera or a
private sibling checkout.

The current upstream Synemantic `main` still advertises the 1.1 grafton-visca
dependency and removed compatibility feature names, and also uses private
path dependencies. CI therefore validates this fixture and reports that an
external-head build is unavailable; it makes no claim about that repository.
