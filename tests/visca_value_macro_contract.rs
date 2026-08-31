#[path = "common/compile_fail.rs"]
mod compile_fail;

#[test]
fn visca_value_attributes_are_checked_downstream() {
    let features = compile_fail::active_grafton_visca_features();
    compile_fail::assert_compile_pass_fixture_paths(
        &["tests/api_contract/pass/visca_value_valid_range.rs"],
        &features,
    );
    compile_fail::assert_compile_fail_fixture_paths(
        &[
            "tests/api_contract/fail/visca_value_unknown_bare_attribute.rs",
            "tests/api_contract/fail/visca_value_unknown_value_attribute.rs",
            "tests/api_contract/fail/visca_value_duplicate_attribute.rs",
            "tests/api_contract/fail/visca_value_missing_max.rs",
            "tests/api_contract/fail/visca_value_missing_min.rs",
            "tests/api_contract/fail/visca_value_malformed_min.rs",
            "tests/api_contract/fail/visca_value_malformed_max.rs",
        ],
        &features,
    );
}
