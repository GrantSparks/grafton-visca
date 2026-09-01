#[path = "common/compile_fail.rs"]
mod compile_fail;

#[test]
fn final_request_classification_contracts() {
    let features = compile_fail::active_grafton_visca_features();
    compile_fail::assert_compile_pass_fixture_paths(
        &["tests/api_contract/pass/final_typed_requests.rs"],
        &features,
    );
    #[allow(unused_mut)]
    let mut fixtures = vec![
        "tests/api_contract/fail/operation_without_affected_axes_is_rejected.rs",
        "tests/api_contract/fail/plain_request_is_not_an_operation.rs",
        "tests/api_contract/fail/inquiry_request_is_not_plain.rs",
        "tests/api_contract/fail/builtin_inquiry_retry_is_private.rs",
        "tests/api_contract/fail/downstream_builtin_inquiry_retry_authority_is_private.rs",
        "tests/api_contract/fail/submission_class_cannot_be_urgent.rs",
        "tests/api_contract/fail/system_protocol_controls_are_internal.rs",
    ];
    // `ts-rs` brings in `unicode-width`, whose blanket private `Sealed` impl
    // adds a feature-dependent rustc help note to these two diagnostics. The
    // direct sealing contract is exercised in the no-default contract gate;
    // the remaining class-bound cases are feature-stable and still run here.
    #[cfg(not(feature = "ts-rs"))]
    {
        // Rust's diagnostic qualification differs across the pure request,
        // blocking-only, and canonical facade surfaces. Keep the source
        // identical while routing each topology to its matching declaration.
        #[cfg(any(feature = "default", feature = "async"))]
        fixtures.push("tests/api_contract/fail/external_request_class_is_sealed.rs");
        #[cfg(all(not(feature = "default"), not(feature = "async"), feature = "blocking"))]
        fixtures.push("tests/api_contract/fail_blocking_only/external_request_class_is_sealed.rs");
        #[cfg(all(
            not(feature = "default"),
            not(feature = "async"),
            not(feature = "blocking")
        ))]
        fixtures.push("tests/api_contract/fail/external_request_class_is_sealed.rs");
        fixtures.push("tests/api_contract/fail/external_completion_kind_is_sealed.rs");
    }
    compile_fail::assert_compile_fail_fixture_paths(&fixtures, &features);
}
