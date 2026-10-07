#[path = "common/compile_fail.rs"]
mod compile_fail;

#[test]
fn visca_value_attributes_are_checked_downstream() {
    let features = compile_fail::active_grafton_visca_features();
    compile_fail::assert_compile_pass_fixture_paths(
        &[
            "tests/api_contract/pass/visca_range_type_downstream.rs",
            "tests/api_contract/pass/visca_value_valid_range.rs",
        ],
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
            "tests/api_contract/fail/visca_value_empty_valid_values.rs",
            "tests/api_contract/fail/visca_value_mixed_valid_values_bounds.rs",
            "tests/api_contract/fail/visca_enum_all_skipped.rs",
            "tests/api_contract/fail/visca_enum_skipped_discriminant_collision.rs",
            "tests/api_contract/fail/visca_value_missing_domain.rs",
            "tests/api_contract/fail/visca_range_type_wide_inner.rs",
            "tests/api_contract/fail/visca_range_inverted_bounds.rs",
            "tests/api_contract/fail/visca_enum_discriminant_above_u8.rs",
            "tests/api_contract/fail/visca_enum_discriminant_negative.rs",
            "tests/api_contract/fail/visca_inquiry_suffixed_literal.rs",
            "tests/api_contract/fail/visca_inquiry_duplicate_attribute.rs",
            "tests/api_contract/fail/visca_inquiry_literal_out_of_range.rs",
            "tests/api_contract/fail/visca_enum_duplicate_attribute.rs",
            "tests/api_contract/fail/visca_enum_exhaustive_removed.rs",
            "tests/api_contract/fail/visca_enum_suffixed_discriminant.rs",
        ],
        &features,
    );
}
