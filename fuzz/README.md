# Grafton VISCA fuzz targets

The fuzz package exercises five bounded protocol layers through doc-hidden
`test-utils` wrappers; production protocol modules remain crate-private:

| Target | Coverage |
| --- | --- |
| `response_parser` | Raw-frame admission, basic response parsing, and every generated `InquiryKind` decoder. |
| `protocol_framer` | Raw and Sony framing with whole and split reads, plus a structurally valid Sony frame derived from each input. |
| `sony_envelope` | Sony request framing, public VISCA extraction, owner control-reply extraction, and sequence RESET framing. |
| `response_target` | Strict serial and single-/multi-target IP source routing. |
| `serial_startup` | Address Set and I/F Clear phases with arbitrary chunks, read/write failures and timing ticks, bounded progress and retained input. |

With `cargo-fuzz` 0.12.0 installed, run:

```text
python3 fuzz/seed_corpus.py
for target in response_parser protocol_framer sony_envelope response_target serial_startup; do
  cargo fuzz check "$target"
  cargo fuzz run "$target" "fuzz/corpus/$target" -- -runs=1000
done
```

The CI smoke test uses the finite `-runs=1000` budget so parser coverage is
checked without an unbounded fuzzing job. Increase the run count locally when
doing a longer investigation.

## Corpus

Run `python3 fuzz/seed_corpus.py` to materialize each target's ignored
`corpus/<target>/` directory, the path `cargo-fuzz` uses by default. The
generator preserves additional inputs discovered by local fuzz runs. The
structured harnesses derive valid framing from the mutated input, so the finite CI budget reaches past protocol headers.
Every `response_parser` seed is run through the complete generated inquiry
inventory, including parameterized queries. Together the existing seed patterns
form the requested kind × pattern matrix, and a newly added inquiry kind joins
it automatically.

The seeds are grouped by filename prefix:

| Prefix | What it covers |
| --- | --- |
| `reply_` | Well-formed ACK, completion, and error replies, including every error code the scheduler classifies (`0x02`, `0x03`, `0x04`, `0x05`, `0x41`). |
| `data_` | Well-formed inquiry data replies; every seed is offered to all generated inquiry kinds. |
| `command_` / `inquiry_` | Well-formed controller-to-camera frames, which is what `raw::Plain` validates. |
| `malformed_` | Inputs every parser must reject without panicking: empty, truncated, missing terminator, bad address byte, out-of-range nibbles, a trailing terminator, and a frame past `raw::MAX_BYTES`. |

Seeds are plain bytes with no header or wrapper. Add reproducible cases to
`SEEDS` in `seed_corpus.py`; repeated noise and other synthetic inputs are
constructed there rather than stored as binary files.

When a run finds a crash, `cargo-fuzz` writes the reproducing input to
`fuzz/artifacts/<target>/`; CI uploads that directory as the `fuzz-artifacts`
artifact on failure. Attach the reproducer to the issue and reproduce it with
`cargo fuzz run <target> fuzz/artifacts/<target>/<file>`. Preserve the fix with
a focused source regression or a deterministic seed recipe, rather than a
committed run-output directory.

The `serial_startup` recipes cover an Address Set reply, Network Change,
oversized noise and an RS-485 broadcast echo followed by a reply. Stable unit
tests construct the same four cases directly and exercise the production
startup driver without device I/O.
