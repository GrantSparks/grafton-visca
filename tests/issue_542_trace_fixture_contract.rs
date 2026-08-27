//! Integrity of the normative issue-542 trace fixtures themselves.
//!
//! Every fixture is replayed through the production engine and owner:
//! `runtime::engine::tests::phase_one_protocol_fixture_replays_through_production_engine`
//! replays the protocol correlation trace, and
//! `runtime::owner::tests::lifecycle_trace` replays each lifecycle trace
//! record for record. Those replays prove the *library* still behaves the way
//! the fixtures say it does.
//!
//! What a production replay cannot check is whether the normative record is
//! still a well-formed normative record: that the trace format stays
//! explicitly versioned, that every scoped correlation case is still present,
//! that every expectation still cites a recognized #542 clause rather than an
//! invented one, that every frame is answered by exactly one expectation, and
//! that both envelopes and all three lower-16 sequence cases stay covered.
//! Those are properties of the fixture, so they are checked here — and only
//! those. The hand-written correlation simulator this file used to carry was
//! deleted with issue #634: it could not fail on any library regression.

#![allow(clippy::expect_used, clippy::panic, clippy::unwrap_used)]

use std::collections::{BTreeMap, BTreeSet};

const TRACE: &str = include_str!("fixtures/issue_542/protocol/correlation-routing.trace");
const FORMAT: &str = "issue-542-protocol-correlation/v1";

const NORMATIVE_CLAUSES: &[&str] = &[
    "#542 exact Sony sequence matches are authoritative and target-compatible.",
    "#542 decoded frames carry explicit target identity and the engine never recovers it from request bytes.",
    "#542 unique lower-16-bit fallback is collision-safe.",
    "#542 unique lower-16-bit fallback becomes valid only when exactly one target-compatible owner remains active.",
    "#542 raw command ACK and completion attribution is keyed by target and socket together.",
    "#542 every target and socket pair has at most one active owner.",
    "#542 malformed, unmatched, duplicate, reordered, or unsolicited input cannot mutate unrelated state.",
    "#542 completion releases all target and socket ownership and emits Applied for the exact request.",
    "#542 raw inquiry replies use route and content matching before FIFO fallback.",
    "#542 raw inquiry FIFO fallback is established independently for each target.",
    "#542 ambiguous raw inquiry content matching falls back to the established per-target FIFO.",
    "#542 absent raw inquiry content matching falls back to the established per-target FIFO.",
    "#542 raw errors without socket or inquiry ownership use the most-recent target-local pending command.",
    "#542 raw errors retain the established target-local temporal attribution policy.",
    "#542 a raw error without socket uses per-target inquiry FIFO before temporal command fallback.",
    "#542 a raw error with an owned target and socket is attributed before FIFO or temporal fallback.",
    "#542 exact Sony sequence matches are authoritative and acquire the target and socket pair.",
    "#542 terminal completion removes all sequence and target-socket ownership for the request.",
    "#542 an unmatched sequenced reply is stale or duplicate traffic and never falls back to socket, FIFO, or temporal heuristics.",
];

const SCENARIOS: &[&str] = &[
    "raw-command-target-and-socket",
    "raw-error-socket-fifo-and-temporal-fallback",
    "raw-inquiry-content-then-per-target-fifo",
    "sony-exact-out-of-order-and-target-compatible",
    "sony-stale-duplicate-and-unmatched-never-fallback",
    "sony-unique-lower16-collision-and-truncation",
];

type Fields = BTreeMap<String, String>;

#[derive(Debug)]
enum RecordKind {
    Transmit(Fields),
    Frame(Fields),
    Expect { normative: String },
}

#[derive(Debug)]
struct Record {
    at_micros: u64,
    kind: RecordKind,
}

#[derive(Debug)]
struct Scenario {
    name: String,
    records: Vec<Record>,
}

#[derive(Debug)]
struct TraceFile {
    format: String,
    scenarios: Vec<Scenario>,
}

fn parse_fields(source: &str, line_number: usize) -> Fields {
    let mut fields = Fields::new();
    for token in source.split_ascii_whitespace() {
        let (key, value) = token
            .split_once('=')
            .unwrap_or_else(|| panic!("line {line_number}: malformed field {token:?}"));
        assert!(
            fields.insert(key.to_owned(), value.to_owned()).is_none(),
            "line {line_number}: duplicate field {key:?}"
        );
    }
    fields
}

fn required<'a>(fields: &'a Fields, key: &str) -> &'a str {
    fields
        .get(key)
        .unwrap_or_else(|| panic!("missing required field {key:?} in {fields:?}"))
}

fn parse_u32(value: &str) -> u32 {
    value.strip_prefix("0x").map_or_else(
        || {
            value
                .parse()
                .unwrap_or_else(|_| panic!("invalid integer value {value:?}"))
        },
        |hex| {
            u32::from_str_radix(hex, 16)
                .unwrap_or_else(|_| panic!("invalid hexadecimal value {value:?}"))
        },
    )
}

fn parse_trace(source: &str) -> TraceFile {
    let mut format = None;
    let mut scenarios = Vec::new();
    let mut current: Option<Scenario> = None;

    for (line_index, source_line) in source.lines().enumerate() {
        let line_number = line_index + 1;
        let line = source_line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }

        if let Some(value) = line.strip_prefix("trace ") {
            assert!(
                format.replace(value.to_owned()).is_none(),
                "line {line_number}: duplicate trace header"
            );
            continue;
        }
        if let Some(name) = line.strip_prefix("scenario ") {
            assert!(current.is_none(), "line {line_number}: nested scenario");
            current = Some(Scenario {
                name: name.to_owned(),
                records: Vec::new(),
            });
            continue;
        }
        if line == "end" {
            scenarios.push(
                current
                    .take()
                    .unwrap_or_else(|| panic!("line {line_number}: end outside scenario")),
            );
            continue;
        }

        let scenario = current
            .as_mut()
            .unwrap_or_else(|| panic!("line {line_number}: record outside scenario"));
        let columns: Vec<_> = line.split('|').map(str::trim).collect();
        assert!(
            columns.len() == 3 || columns.len() == 4,
            "line {line_number}: expected three or four pipe-delimited columns"
        );
        let at_micros = columns[0]
            .parse::<u64>()
            .unwrap_or_else(|_| panic!("line {line_number}: invalid timestamp {:?}", columns[0]));
        let fields = parse_fields(columns[2], line_number);
        let kind = match columns[1] {
            "transmit" => {
                assert_eq!(
                    columns.len(),
                    3,
                    "line {line_number}: transmit has no normative column"
                );
                RecordKind::Transmit(fields)
            }
            "frame" => {
                assert_eq!(
                    columns.len(),
                    3,
                    "line {line_number}: frame has no normative column"
                );
                RecordKind::Frame(fields)
            }
            "expect" => {
                assert_eq!(
                    columns.len(),
                    4,
                    "line {line_number}: expectation requires its #542 normative clause"
                );
                // The observation columns are the production replay's
                // expectations; only their presence is a fixture property.
                required(&fields, "observer");
                required(&fields, "outcome");
                RecordKind::Expect {
                    normative: columns[3].to_owned(),
                }
            }
            other => panic!("line {line_number}: unknown record kind {other:?}"),
        };
        scenario.records.push(Record { at_micros, kind });
    }

    assert!(current.is_none(), "unterminated scenario");
    TraceFile {
        format: format.expect("trace header is required"),
        scenarios,
    }
}

#[test]
fn protocol_correlation_trace_stays_a_well_formed_normative_record() {
    let trace = parse_trace(TRACE);
    assert_eq!(
        trace.format, FORMAT,
        "trace format must be explicitly versioned"
    );

    let actual_scenarios: BTreeSet<_> = trace
        .scenarios
        .iter()
        .map(|scenario| scenario.name.as_str())
        .collect();
    assert_eq!(
        actual_scenarios,
        SCENARIOS.iter().copied().collect::<BTreeSet<_>>(),
        "all scoped correlation cases remain named and replayable"
    );

    let mut envelopes = BTreeSet::new();
    let mut truncated_sequence_frames = 0_usize;

    for scenario in &trace.scenarios {
        let mut previous_time = 0;
        let mut transmissions = 0_usize;
        let mut frames = 0_usize;
        let mut expectations = 0_usize;
        let mut answered = true;

        for record in &scenario.records {
            assert!(
                record.at_micros >= previous_time,
                "scenario {:?} is not ordered at {}us",
                scenario.name,
                record.at_micros
            );
            previous_time = record.at_micros;

            match &record.kind {
                RecordKind::Transmit(fields) => {
                    assert!(
                        answered,
                        "scenario {:?} leaves a frame unanswered before its next input",
                        scenario.name
                    );
                    let envelope = required(fields, "envelope");
                    assert_eq!(
                        envelope == "sony",
                        fields.contains_key("sequence"),
                        "scenario {:?}: Sony transmissions carry a sequence and raw transmissions do not",
                        scenario.name
                    );
                    envelopes.insert(envelope.to_owned());
                    transmissions += 1;
                }
                RecordKind::Frame(fields) => {
                    assert!(
                        answered,
                        "scenario {:?} leaves a frame unanswered before its next input",
                        scenario.name
                    );
                    envelopes.insert(required(fields, "envelope").to_owned());
                    if fields.get("sequence-width").map(String::as_str) == Some("lower16") {
                        assert_eq!(required(fields, "envelope"), "sony");
                        assert!(parse_u32(required(fields, "sequence")) <= u32::from(u16::MAX));
                        truncated_sequence_frames += 1;
                    }
                    frames += 1;
                    answered = false;
                }
                RecordKind::Expect { normative } => {
                    assert!(
                        NORMATIVE_CLAUSES.contains(&normative.as_str()),
                        "scenario {:?} expectation does not directly state a recognized normative #542 behavior: {normative:?}",
                        scenario.name
                    );
                    assert!(
                        !answered,
                        "scenario {:?} states an expectation without a preceding frame",
                        scenario.name
                    );
                    answered = true;
                    expectations += 1;
                }
            }
        }

        assert!(
            answered,
            "scenario {:?} left a frame unanswered",
            scenario.name
        );
        assert!(
            transmissions > 0,
            "scenario {:?} records no transmissions",
            scenario.name
        );
        assert!(frames > 0, "scenario {:?} records no frames", scenario.name);
        assert_eq!(
            frames, expectations,
            "scenario {:?} must name one normative expected outcome per frame",
            scenario.name
        );
    }

    assert_eq!(
        envelopes,
        BTreeSet::from(["raw".to_owned(), "sony".to_owned()])
    );
    assert!(
        truncated_sequence_frames >= 3,
        "unique, ambiguous, and recovered lower-16 cases must remain explicit"
    );
}
