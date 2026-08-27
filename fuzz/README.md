# Grafton VISCA fuzz targets

The `response_parser` target feeds arbitrary bytes to the public raw-frame
validator and VISCA response/inquiry parsers. It uses no transport and does
not expose private implementation details solely for fuzzing.

With `cargo-fuzz` 0.12.0 installed, run:

```text
cargo fuzz check response_parser
cargo fuzz run response_parser corpus/response_parser -- -runs=1000
```

The CI smoke test uses the finite `-runs=1000` budget so parser coverage is
checked without an unbounded fuzzing job. Increase the run count locally when
doing a longer investigation.

## Corpus

`corpus/response_parser/` is committed, and it is the same path `cargo-fuzz`
uses by default, so both CI and a local run start warm. A 1000-execution
budget from an empty corpus is largely spent rediscovering that a VISCA frame
begins with an address byte and ends with `0xFF`; the seeds spend it on the
payload decoders instead.

The seeds are grouped by filename prefix:

| Prefix | What it covers |
| --- | --- |
| `reply_` | Well-formed ACK, completion, and error replies, including every error code the scheduler classifies (`0x02`, `0x03`, `0x04`, `0x05`, `0x41`). |
| `data_` | Well-formed inquiry data replies for the three `InquiryKind`s the target parses: power, zoom position, pan/tilt position. |
| `command_` / `inquiry_` | Well-formed controller-to-camera frames, which is what `raw::Plain` validates. |
| `malformed_` | Inputs every parser must reject without panicking: empty, truncated, missing terminator, bad address byte, out-of-range nibbles, a trailing terminator, and a frame past `raw::MAX_BYTES`. |

Seeds are plain bytes with no header or wrapper; add one by writing the frame
to a new file in that directory.

**Commit every crash as a seed.** When a run finds one, `cargo-fuzz` writes the
reproducing input to `fuzz/artifacts/response_parser/`; the CI job uploads that
directory as the `fuzz-artifacts` artifact on failure. Reproduce it with
`cargo fuzz run response_parser fuzz/artifacts/response_parser/<file>`, then
commit the file into `corpus/response_parser/` under a `malformed_`-prefixed
descriptive name alongside the fix. A crash found once and not pinned is a
crash the next cold run has to find again.
