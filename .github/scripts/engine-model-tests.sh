# shellcheck shell=bash
#
# The protocol engine's randomized, generated and exhaustive model tests, by
# full libtest path. Sourced by `test-all-features.sh`.
#
# They are the expensive part of the library unit tests (about 90 CPU-seconds
# per `--lib` run) and they cannot differ by facade configuration: the engine
# compiles only with a facade (`any(async, blocking)`), its production code in
# `src/runtime/engine/` has no further `cfg(feature = ...)`, and the crate items
# it uses (`protocol::framer`, `raw`, the error and ID types) are gated at most
# on that same facade condition. The feature cfgs that do exist there are
# test-only: four engine tests gated on a facade, and one helper in `types.rs`
# under `cfg(all(test, any(async, blocking)))`. None of the four tests below is
# among those tests or uses that helper.
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
)
