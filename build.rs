fn main() {
    println!("cargo:rerun-if-env-changed=CARGO_FEATURE_RUNTIME_ASYNC_STD");
    if std::env::var_os("CARGO_FEATURE_RUNTIME_ASYNC_STD").is_some() {
        println!(
            "cargo:warning=feature \"runtime-async-std\" is deprecated, now aliases \"runtime-smol\", and will be removed in 0.13.0; prefer runtime-smol/SmolRuntime for new code"
        );
    }
}
