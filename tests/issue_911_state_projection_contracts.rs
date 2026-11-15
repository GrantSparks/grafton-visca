//! The ledger-generated state projection requires exactly its typed value hook.

#![cfg(any(feature = "blocking", feature = "async"))]

use grafton_visca_test_support::{active_grafton_visca_features, compile_fail};

#[test]
fn state_projection_obligations_are_compile_time_contracts() {
    let features = active_grafton_visca_features!();
    compile_fail::assert_compile_pass_fixture_paths(
        &["tests/api_contract/pass_facade/state_projection_value_contract.rs"],
        &features,
    );
    compile_fail::assert_compile_fail_fixture_paths(
        &[
            "tests/api_contract/fail_facade/state_set_requires_value_method.rs",
            "tests/api_contract/fail_facade/state_clear_requires_value_provider.rs",
            "tests/api_contract/fail_facade/state_projection_cannot_override_validation.rs",
        ],
        &features,
    );
}
