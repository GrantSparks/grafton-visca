#[test]
fn final_request_classification_contracts() {
    let cases = trybuild::TestCases::new();
    cases.pass("tests/api_contract/pass/final_typed_requests.rs");
    // `ts-rs` brings in `unicode-width`, whose blanket private `Sealed` impl
    // adds a feature-dependent rustc help note to these two diagnostics. The
    // direct sealing contract is exercised in the no-default contract gate;
    // the remaining class-bound cases are feature-stable and still run here.
    #[cfg(not(feature = "ts-rs"))]
    {
        // Rust's diagnostic qualification differs across the pure request,
        // blocking-only, and canonical facade surfaces. Keep the source
        // identical while routing each topology to its own stable snapshot.
        #[cfg(any(feature = "default", feature = "async"))]
        cases.compile_fail("tests/api_contract/fail/external_request_class_is_sealed.rs");
        #[cfg(all(not(feature = "default"), not(feature = "async"), feature = "blocking"))]
        cases.compile_fail(
            "tests/api_contract/fail_blocking_only/external_request_class_is_sealed.rs",
        );
        #[cfg(all(
            not(feature = "default"),
            not(feature = "async"),
            not(feature = "blocking")
        ))]
        cases.compile_fail(
            "tests/api_contract/fail_no_canonical/external_request_class_is_sealed.rs",
        );
        cases.compile_fail("tests/api_contract/fail/external_completion_kind_is_sealed.rs");
    }
    cases.compile_fail("tests/api_contract/fail/operation_without_affected_axes_is_rejected.rs");
    cases.compile_fail("tests/api_contract/fail/plain_request_is_not_an_operation.rs");
    cases.compile_fail("tests/api_contract/fail/inquiry_request_is_not_plain.rs");
    cases.compile_fail("tests/api_contract/fail/builtin_inquiry_retry_is_private.rs");
}
