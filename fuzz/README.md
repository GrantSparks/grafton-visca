# Grafton VISCA fuzz targets

The fuzz package exercises four bounded protocol layers through doc-hidden
`test-utils` wrappers; production protocol modules remain crate-private:

| Target | Coverage |
| --- | --- |
| `response_parser` | Raw-frame admission, basic response parsing, and every generated `InquiryKind` decoder (currently 73 distinct kinds). |
| `protocol_framer` | Raw and Sony framing with whole and split reads, plus a structurally valid Sony frame derived from each input. |
| `sony_envelope` | Sony request framing, public VISCA extraction, owner control-reply extraction, and sequence RESET framing. |
| `response_target` | Strict serial and single-/multi-target IP source routing. |

With `cargo-fuzz` 0.12.0 installed, run:

```text
for target in response_parser protocol_framer sony_envelope response_target; do
  cargo fuzz check "$target"
  cargo fuzz run "$target" "corpus/$target" -- -runs=1000
done
```

The CI smoke test uses the finite `-runs=1000` budget so parser coverage is
checked without an unbounded fuzzing job. Increase the run count locally when
doing a longer investigation.

## Corpus

Each target has a committed `corpus/<target>/` directory, which is the path
`cargo-fuzz` uses by default. The structured harnesses derive valid framing
from the mutated input, so the finite CI budget reaches past protocol headers.
Every `response_parser` seed is run through the complete generated inquiry
inventory: 63 queryable rows collapse onto 62 distinct response kinds, then 11
decode-only kinds bring the current total to 73. Together the existing seed
patterns form the requested kind × pattern matrix, and a newly added inquiry
kind joins it automatically.

The seeds are grouped by filename prefix:

| Prefix | What it covers |
| --- | --- |
| `reply_` | Well-formed ACK, completion, and error replies, including every error code the scheduler classifies (`0x02`, `0x03`, `0x04`, `0x05`, `0x41`). |
| `data_` | Well-formed inquiry data replies; every seed is offered to all generated inquiry kinds. |
| `command_` / `inquiry_` | Well-formed controller-to-camera frames, which is what `raw::Plain` validates. |
| `malformed_` | Inputs every parser must reject without panicking: empty, truncated, missing terminator, bad address byte, out-of-range nibbles, a trailing terminator, and a frame past `raw::MAX_BYTES`. |

Seeds are plain bytes with no header or wrapper; add one by writing the frame
to a new file in that directory.

**Commit every crash as a seed.** When a run finds one, `cargo-fuzz` writes the
reproducing input to `fuzz/artifacts/<target>/`; the CI job uploads that
directory as the `fuzz-artifacts` artifact on failure. Reproduce it with
`cargo fuzz run <target> fuzz/artifacts/<target>/<file>`, then commit the file
into that target's corpus under a descriptive name alongside the fix. A crash
found once and not pinned is a crash the next cold run has to find again.
