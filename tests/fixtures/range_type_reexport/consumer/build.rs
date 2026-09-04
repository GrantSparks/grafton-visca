fn main() {
    let manifest = include_str!("Cargo.toml");
    for forbidden_dependency in ["grafton-visca", "serde", "schemars", "ts-rs"] {
        assert!(
            !manifest.contains(forbidden_dependency),
            "the consumer must not acquire a direct `{forbidden_dependency}` dependency"
        );
    }
}
