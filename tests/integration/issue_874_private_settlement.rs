#![cfg(any(feature = "blocking", feature = "async"))]

use grafton_visca_test_support::{active_grafton_visca_features, compile_fail};

#[test]
fn settlement_lowering_proofs_are_not_public_api() {
    compile_fail::assert_compile_fail_fixture_paths(
        &["tests/api_contract/fail_facade/completion_settlement_is_private.rs"],
        &active_grafton_visca_features!(),
    );
}
