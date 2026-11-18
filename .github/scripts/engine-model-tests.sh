# shellcheck shell=bash
#
# Feature-independent engine and owner regressions, by full libtest path.
# Sourced by the feature matrix and standalone CI jobs. The engine and the
# owner turn coordinator are gated on the same condition (`any(async, blocking)`).
# The coordinator and its tests, and the inquiry-shapes tests, have no feature
# cfgs of their own. The engine's two production test-utils accessors only read
# the command spacing and envelope kind. Its other internal feature cfgs are test-only,
# and none apply to any of the listed tests or the helpers they use.
#
# So the feature matrix runs them in exactly one leg (`ENGINE_MODEL_TEST_LEG`
# in `test-all-features.sh`) and skips them, by exact name, in every other;
# that script fails a leg in which any of them ran where it should not, or did
# not run where it should, so a rename turns the matrix red instead of
# silently dropping the test.
readonly ENGINE_MODEL_TESTS=(
    "runtime::engine::tests::arbitrary_stale_and_reordered_inputs_preserve_invariants"
    "runtime::engine::tests::generated_invariant_properties::arbitrary_ordered_and_stale_inputs_preserve_invariants_property"
    "runtime::engine::tests::raw_stream_model::randomized_stream_sessions_bind_every_answer_to_its_originator"
    "runtime::engine::tests::raw_evidence::ten_thousand_halts_against_a_silent_camera_keep_the_ledger_bounded"
    "runtime::engine::tests::inquiry_shapes::randomized_stream_mixed_shapes_never_cross_bind"
    "runtime::engine::tests::inquiry_shapes::randomized_datagram_mixed_shapes_never_cross_bind"
    "runtime::engine::tests::inquiry_shapes::alternating_inquiry_and_stop_debts_bound_the_ack_separator_charge"
    "runtime::owner::turn::coordinator::tests::model::exhaustive_short_sequences_preserve_the_invariants"
    "runtime::owner::turn::coordinator::tests::model::generated_sequences_preserve_the_invariants"
)

# Test paths contain no whitespace, so callers can read the arguments into
# an array with Bash's built-in read (including macOS's Bash 3).
engine_model_skip_args() {
    printf '%s' --exact
    printf ' --skip %s' "${ENGINE_MODEL_TESTS[@]}"
    printf '\n'
}
