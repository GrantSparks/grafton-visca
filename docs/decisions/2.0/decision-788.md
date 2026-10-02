# Decision record: Extensible public types are `#[non_exhaustive]` before 2.0.0 (#788)

Status: Accepted — 2026-10-02

Decision number: D27 (`docs/issue_542_design_review.md`). Raised during the
#784 review; not one of its proposals.

## Context

The `api/2.0.0-rc.1/all-features.txt` snapshot lists 664 public types, of
which 61 are `#[non_exhaustive]`. After 2.0.0, adding a field or variant to
any other public struct or enum is a 3.0-class change. The shapes most likely
to grow include:

- configuration and options: `TransportConfig`, `BufferConfig`,
  `TcpKeepaliveConfig`, the serial `Config`, the Tokio `SerialConfig`, and the
  `TransportOptions` variants;
- queries and waits: `MotionQuery`, `IdleWait`, `MovementTolerance`;
- reports: `MetricsSnapshot` and the `DiagnosticEvent` variants;
- the payloads of `Error` struct variants;
- `ReceiveOutcome` and `FrameMeta`.

## Decision

1. Types whose fields or variants may grow (configuration, options, queries,
   reports, metrics, diagnostic events, error payloads, outcomes) become
   `#[non_exhaustive]`. Anything a user must construct gets a constructor or
   builder.
2. Pure value types fixed by the VISCA protocol or by arithmetic (positions,
   command payload structs, `Fraction`, units) stay exhaustive.
3. Custom transports construct `Error` values, so affected `Error` variants
   get constructor functions in the same change that makes their payloads
   non-exhaustive.
4. New types introduced by D19-D26 are `#[non_exhaustive]` from the start
   wherever rule 1 applies.
5. The implementation PR includes an inventory table classifying every public
   struct and struct-like variant, with the API snapshot diff, a CHANGELOG
   `**BREAKING**` entry, and a migration section for struct-literal and
   exhaustive-match call sites.

## Breaking-change impact

Breaking for code that builds these types with struct literals or matches
them exhaustively. It must land before 2.0.0 final, and lands before rc.3.

## Effort

M. Mostly mechanical, with careful review of the classification. Touches
docs, examples and tests that use struct literals.
