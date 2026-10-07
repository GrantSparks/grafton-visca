# shellcheck shell=bash
#
# The protocol engine's randomized, generated and exhaustive model tests, by
# full libtest path. Sourced by `test-all-features.sh` and `miri-tests.sh`.
#
# They are the expensive part of the library unit tests (about 90 CPU-seconds
# of the ~92 a no-default `--lib` run costs) and they cannot differ by feature
# configuration: `src/runtime/engine/` has no `cfg(feature = ...)` outside its
# test module, and the crate items it uses (`protocol::framer`, `raw`, the
# error and ID types) are gated at most on `any(async, blocking, test)`, which
# `cfg(test)` makes uniform. None of these tests is among the engine tests
# gated on a facade.
#
# So the feature matrix runs them in exactly one leg (`ENGINE_MODEL_TEST_LEG`
# in `test-all-features.sh`) and skips them, by exact name, in every other;
# that script fails a leg in which any of them ran where it should not, or did
# not run where it should, so a rename turns the matrix red instead of
# silently dropping the test. Miri skips them because an interpreted run of
# them would not finish.
readonly ENGINE_MODEL_TESTS=(
    "runtime::engine::tests::arbitrary_stale_and_reordered_inputs_preserve_invariants"
    "runtime::engine::tests::generated_invariant_properties::arbitrary_ordered_and_stale_inputs_preserve_invariants_property"
    "runtime::engine::tests::raw_stream_model::randomized_stream_sessions_bind_every_answer_to_its_originator"
    "runtime::engine::tests::raw_evidence::ten_thousand_halts_against_a_silent_camera_keep_the_ledger_bounded"
)
