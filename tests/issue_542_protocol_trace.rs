//! Executable Phase 1 characterization traces for issue #542 protocol routing.
//!
//! The fixture deliberately models the normative 2.0 boundary contract instead
//! of the current 1.x scheduler representation. It is a replayable specification:
//! ordered transmissions and decoded frames go in, and stable state effects or
//! observer-visible outcomes come out. Later protocol-engine phases can replay the
//! same fixture through the production engine without changing the trace format.

#![allow(clippy::expect_used, clippy::panic, clippy::unwrap_used)]

use std::collections::{BTreeMap, BTreeSet, VecDeque};

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

#[derive(Debug)]
struct TraceFile {
    format: String,
    scenarios: Vec<Scenario>,
}

#[derive(Debug)]
struct Scenario {
    name: String,
    records: Vec<Record>,
}

#[derive(Debug)]
struct Record {
    at_micros: u64,
    kind: RecordKind,
}

#[derive(Debug)]
enum RecordKind {
    Transmit(Fields),
    Frame(Fields),
    Expect {
        observation: Observation,
        normative: String,
    },
}

type Fields = BTreeMap<String, String>;

#[derive(Debug, Clone, PartialEq, Eq)]
struct Observation {
    observer: String,
    outcome: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Envelope {
    Raw,
    Sony,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RequestKind {
    Command,
    Inquiry,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CommandPhase {
    AwaitingAck,
    Executing { socket: u8 },
}

#[derive(Debug)]
struct Entry {
    target: u8,
    kind: RequestKind,
    envelope: Envelope,
    sequence: Option<u32>,
    route: Option<String>,
    transmission_order: u64,
    phase: Option<CommandPhase>,
}

#[derive(Debug, Default)]
struct CorrelationModel {
    active: BTreeMap<String, Entry>,
    next_transmission_order: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum SequenceMiss {
    TargetIncompatible,
    AmbiguousLower16,
    Unmatched,
}

impl CorrelationModel {
    fn transmit(&mut self, fields: &Fields) {
        let id = required(fields, "id").to_owned();
        let target = parse_u8(required(fields, "target"));
        let kind = match required(fields, "kind") {
            "command" => RequestKind::Command,
            "inquiry" => RequestKind::Inquiry,
            other => panic!("unknown request kind {other:?}"),
        };
        let envelope = parse_envelope(required(fields, "envelope"));
        let sequence = fields.get("sequence").map(|value| parse_u32(value));
        let route = fields.get("route").cloned();

        assert_eq!(
            envelope == Envelope::Sony,
            sequence.is_some(),
            "Sony transmissions carry a sequence and raw transmissions do not"
        );
        assert!(
            self.active
                .insert(
                    id.clone(),
                    Entry {
                        target,
                        kind,
                        envelope,
                        sequence,
                        route,
                        transmission_order: self.next_transmission_order,
                        phase: (kind == RequestKind::Command).then_some(CommandPhase::AwaitingAck),
                    },
                )
                .is_none(),
            "fixture reused active request id {id:?}"
        );
        self.next_transmission_order += 1;
    }

    fn frame(&mut self, fields: &Fields) -> Observation {
        let target = parse_u8(required(fields, "target"));
        let envelope = parse_envelope(required(fields, "envelope"));
        let response = required(fields, "response");
        let socket = fields.get("socket").map(|value| parse_u8(value));

        let resolved = match envelope {
            Envelope::Sony => {
                let sequence = parse_u32(required(fields, "sequence"));
                match self.resolve_sony(target, sequence) {
                    Ok(id) => Some(id),
                    Err(miss) => return ignored_sequence(miss),
                }
            }
            Envelope::Raw => self.resolve_raw(target, response, socket, fields),
        };

        let Some(id) = resolved else {
            return Observation {
                observer: "diagnostics".to_owned(),
                outcome: if response == "completion" && socket.is_some() {
                    "ignored:unmatched-target-socket".to_owned()
                } else {
                    "ignored:unmatched-raw-frame".to_owned()
                },
            };
        };

        match response {
            "ack" => {
                let socket = socket.expect("ACK fixture requires a socket");
                self.assign_socket(&id, target, socket);
                Observation {
                    observer: "engine".to_owned(),
                    outcome: format!("ack:{id}:target={target}:socket={socket}"),
                }
            }
            "completion" => {
                let entry = self
                    .active
                    .remove(&id)
                    .expect("resolved completion must have an active owner");
                assert_eq!(entry.kind, RequestKind::Command);
                if let Some(frame_socket) = socket {
                    assert_eq!(
                        entry.phase,
                        Some(CommandPhase::Executing {
                            socket: frame_socket
                        }),
                        "completion socket must agree with exact command ownership"
                    );
                }
                Observation {
                    observer: id,
                    outcome: "applied".to_owned(),
                }
            }
            "reply" => {
                let entry = self
                    .active
                    .remove(&id)
                    .expect("resolved reply must have an active owner");
                assert_eq!(entry.kind, RequestKind::Inquiry);
                Observation {
                    observer: id,
                    outcome: format!(
                        "reply:{}:{}",
                        required(fields, "route"),
                        required(fields, "payload")
                    ),
                }
            }
            "error" => {
                self.active
                    .remove(&id)
                    .expect("resolved error must have an active owner");
                Observation {
                    observer: id,
                    outcome: format!("error:{}", required(fields, "code")),
                }
            }
            other => panic!("unknown response kind {other:?}"),
        }
    }

    fn resolve_sony(&self, target: u8, sequence: u32) -> Result<String, SequenceMiss> {
        let exact: Vec<_> = self
            .active
            .iter()
            .filter(|(_, entry)| {
                entry.envelope == Envelope::Sony && entry.sequence == Some(sequence)
            })
            .collect();

        if !exact.is_empty() {
            let target_compatible: Vec<_> = exact
                .into_iter()
                .filter(|(_, entry)| entry.target == target)
                .collect();
            return match target_compatible.as_slice() {
                [(id, _)] => Ok((*id).clone()),
                [] => Err(SequenceMiss::TargetIncompatible),
                _ => Err(SequenceMiss::Unmatched),
            };
        }

        let lower16 = sequence as u16;
        let owners: Vec<_> = self
            .active
            .iter()
            .filter(|(_, entry)| {
                entry.envelope == Envelope::Sony
                    && entry.target == target
                    && entry.sequence.is_some_and(|owned| owned as u16 == lower16)
            })
            .map(|(id, _)| id.clone())
            .collect();

        match owners.as_slice() {
            [id] => Ok(id.clone()),
            [] => Err(SequenceMiss::Unmatched),
            _ => Err(SequenceMiss::AmbiguousLower16),
        }
    }

    fn resolve_raw(
        &self,
        target: u8,
        response: &str,
        socket: Option<u8>,
        fields: &Fields,
    ) -> Option<String> {
        match response {
            "ack" => self.oldest_matching(|entry| {
                entry.envelope == Envelope::Raw
                    && entry.target == target
                    && entry.kind == RequestKind::Command
                    && entry.phase == Some(CommandPhase::AwaitingAck)
            }),
            "completion" => {
                let socket = socket?;
                self.oldest_matching(|entry| {
                    entry.envelope == Envelope::Raw
                        && entry.target == target
                        && entry.phase == Some(CommandPhase::Executing { socket })
                })
            }
            "reply" => {
                let route = required(fields, "route");
                let route_matches: Vec<_> = self
                    .active
                    .iter()
                    .filter(|(_, entry)| {
                        entry.envelope == Envelope::Raw
                            && entry.target == target
                            && entry.kind == RequestKind::Inquiry
                            && route != "unknown"
                            && entry.route.as_deref() == Some(route)
                    })
                    .map(|(id, _)| id.clone())
                    .collect();

                if let [id] = route_matches.as_slice() {
                    Some(id.clone())
                } else {
                    self.oldest_matching(|entry| {
                        entry.envelope == Envelope::Raw
                            && entry.target == target
                            && entry.kind == RequestKind::Inquiry
                    })
                }
            }
            "error" => socket
                .and_then(|socket| {
                    self.oldest_matching(|entry| {
                        entry.envelope == Envelope::Raw
                            && entry.target == target
                            && entry.phase == Some(CommandPhase::Executing { socket })
                    })
                })
                .or_else(|| {
                    self.oldest_matching(|entry| {
                        entry.envelope == Envelope::Raw
                            && entry.target == target
                            && entry.kind == RequestKind::Inquiry
                    })
                })
                .or_else(|| {
                    self.newest_matching(|entry| {
                        entry.envelope == Envelope::Raw
                            && entry.target == target
                            && entry.kind == RequestKind::Command
                            && entry.phase == Some(CommandPhase::AwaitingAck)
                    })
                }),
            other => panic!("unknown raw response kind {other:?}"),
        }
    }

    fn assign_socket(&mut self, id: &str, target: u8, socket: u8) {
        assert!(
            !self.active.iter().any(|(other_id, entry)| {
                other_id != id
                    && entry.target == target
                    && entry.phase == Some(CommandPhase::Executing { socket })
            }),
            "target {target} socket {socket} already has an owner"
        );

        let entry = self
            .active
            .get_mut(id)
            .expect("ACK resolution must identify an active request");
        assert_eq!(entry.target, target);
        assert_eq!(entry.kind, RequestKind::Command);
        assert_eq!(entry.phase, Some(CommandPhase::AwaitingAck));
        entry.phase = Some(CommandPhase::Executing { socket });
    }

    fn oldest_matching(&self, predicate: impl Fn(&Entry) -> bool) -> Option<String> {
        self.active
            .iter()
            .filter(|(_, entry)| predicate(entry))
            .min_by_key(|(_, entry)| entry.transmission_order)
            .map(|(id, _)| id.clone())
    }

    fn newest_matching(&self, predicate: impl Fn(&Entry) -> bool) -> Option<String> {
        self.active
            .iter()
            .filter(|(_, entry)| predicate(entry))
            .max_by_key(|(_, entry)| entry.transmission_order)
            .map(|(id, _)| id.clone())
    }
}

fn ignored_sequence(miss: SequenceMiss) -> Observation {
    let outcome = match miss {
        SequenceMiss::TargetIncompatible => "ignored:target-incompatible-sequence",
        SequenceMiss::AmbiguousLower16 => "ignored:ambiguous-lower16-sequence",
        SequenceMiss::Unmatched => "ignored:unmatched-sequenced-reply",
    };
    Observation {
        observer: "diagnostics".to_owned(),
        outcome: outcome.to_owned(),
    }
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
                RecordKind::Expect {
                    observation: Observation {
                        observer: required(&fields, "observer").to_owned(),
                        outcome: required(&fields, "outcome").to_owned(),
                    },
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

fn parse_envelope(value: &str) -> Envelope {
    match value {
        "raw" => Envelope::Raw,
        "sony" => Envelope::Sony,
        other => panic!("unknown envelope {other:?}"),
    }
}

fn parse_u8(value: &str) -> u8 {
    parse_u32(value)
        .try_into()
        .unwrap_or_else(|_| panic!("value {value:?} does not fit in u8"))
}

fn parse_u32(value: &str) -> u32 {
    if let Some(hex) = value.strip_prefix("0x") {
        u32::from_str_radix(hex, 16)
            .unwrap_or_else(|_| panic!("invalid hexadecimal value {value:?}"))
    } else {
        value
            .parse()
            .unwrap_or_else(|_| panic!("invalid integer value {value:?}"))
    }
}

#[test]
fn protocol_correlation_trace_replays_normative_issue_542_outcomes() {
    let trace = parse_trace(TRACE);
    assert_eq!(
        trace.format, FORMAT,
        "trace format must be explicitly versioned"
    );

    let expected_scenarios = BTreeSet::from([
        "raw-command-target-and-socket",
        "raw-error-socket-fifo-and-temporal-fallback",
        "raw-inquiry-content-then-per-target-fifo",
        "sony-exact-out-of-order-and-target-compatible",
        "sony-stale-duplicate-and-unmatched-never-fallback",
        "sony-unique-lower16-collision-and-truncation",
    ]);
    let actual_scenarios: BTreeSet<_> = trace
        .scenarios
        .iter()
        .map(|scenario| scenario.name.as_str())
        .collect();
    assert_eq!(
        actual_scenarios, expected_scenarios,
        "all scoped correlation cases remain named and replayable"
    );

    let mut envelopes = BTreeSet::new();
    let mut truncated_sequence_frames = 0;

    for scenario in trace.scenarios {
        let mut model = CorrelationModel::default();
        let mut pending = VecDeque::new();
        let mut previous_time = 0;
        let mut transmissions = 0;
        let mut frames = 0;
        let mut expectations = 0;

        for record in scenario.records {
            assert!(
                record.at_micros >= previous_time,
                "scenario {:?} is not ordered at {}us",
                scenario.name,
                record.at_micros
            );
            previous_time = record.at_micros;

            match record.kind {
                RecordKind::Transmit(fields) => {
                    assert!(
                        pending.is_empty(),
                        "scenario {:?} must drain effects before its next input",
                        scenario.name
                    );
                    envelopes.insert(required(&fields, "envelope").to_owned());
                    transmissions += 1;
                    model.transmit(&fields);
                }
                RecordKind::Frame(fields) => {
                    assert!(
                        pending.is_empty(),
                        "scenario {:?} must drain effects before its next input",
                        scenario.name
                    );
                    envelopes.insert(required(&fields, "envelope").to_owned());
                    if fields.get("sequence-width").map(String::as_str) == Some("lower16") {
                        assert_eq!(required(&fields, "envelope"), "sony");
                        assert!(parse_u32(required(&fields, "sequence")) <= u32::from(u16::MAX));
                        truncated_sequence_frames += 1;
                    }
                    frames += 1;
                    pending.push_back((record.at_micros, model.frame(&fields)));
                }
                RecordKind::Expect {
                    observation,
                    normative,
                } => {
                    expectations += 1;
                    assert!(
                        NORMATIVE_CLAUSES.contains(&normative.as_str()),
                        "scenario {:?} expectation does not directly state a recognized normative #542 behavior: {:?}",
                        scenario.name,
                        normative
                    );
                    let (effect_time, actual) = pending.pop_front().unwrap_or_else(|| {
                        panic!(
                            "scenario {:?} expected {observation:?} without a preceding effect",
                            scenario.name
                        )
                    });
                    assert_eq!(
                        record.at_micros, effect_time,
                        "scenario {:?} effect timestamp differs from its triggering input",
                        scenario.name
                    );
                    assert_eq!(actual, observation, "scenario {:?} observer-visible outcome mismatch at {}us; normative behavior: {}", scenario.name, record.at_micros, normative);
                }
            }
        }

        assert!(
            pending.is_empty(),
            "scenario {:?} left observer-visible effects unchecked",
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
