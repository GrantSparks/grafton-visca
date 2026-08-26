# Grafton VISCA fuzz targets

The `response_parser` target feeds arbitrary bytes to the public raw-frame
validator and VISCA response/inquiry parsers. It uses no transport and does
not expose private implementation details solely for fuzzing.

With `cargo-fuzz` 0.12.0 installed, run:

```text
cargo fuzz check response_parser
cargo fuzz run response_parser -- -runs=1000
```

The CI smoke test uses the finite `-runs=1000` budget so parser coverage is
checked without an unbounded fuzzing job. Increase the run count locally when
doing a longer investigation.
