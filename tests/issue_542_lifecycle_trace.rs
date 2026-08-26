//! Issue #542 / Phase 1 lifecycle and runtime-boundary characterization.
//!
//! These fixtures describe the normative 2.0 owner/engine boundary, including
//! behavior that intentionally differs from the temporary 1.x runtime.  The
//! small model below is test-only: it makes the trace executable and replayable
//! without introducing compatibility behavior into production code.

use std::collections::{BTreeMap, HashMap};

const BLOCKING_OUT_OF_ORDER: &str =
    include_str!("fixtures/issue_542/lifecycle/blocking_out_of_order.trace");
const OBSERVER_LATE_DELIVERY: &str =
    include_str!("fixtures/issue_542/lifecycle/observer_late_delivery.trace");
const DEADLINE_CLASSES: &str = include_str!("fixtures/issue_542/lifecycle/deadline_classes.trace");
const CAPACITY_AND_FAILURES: &str =
    include_str!("fixtures/issue_542/lifecycle/capacity_and_failures.trace");
const PTZOPTICS_CANCEL: &str = include_str!("fixtures/issue_542/lifecycle/ptzoptics_cancel.trace");

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Transport {
    Datagram,
    Stream,
}

impl Transport {
    const fn as_str(self) -> &'static str {
        match self {
            Self::Datagram => "datagram",
            Self::Stream => "stream",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum SessionState {
    Running,
    Poisoned,
    Shutdown,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Phase {
    Ready,
    AwaitingAck,
    Executing { socket: u8 },
}

impl Phase {
    fn trace_name(self) -> String {
        match self {
            Self::Ready => "ready".into(),
            Self::AwaitingAck => "awaiting-ack".into(),
            Self::Executing { socket } => format!("executing(socket={socket})"),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
enum Observer {
    Blocking,
    Async,
    Detached,
}

impl Observer {
    fn parse(value: &str) -> Self {
        match value {
            "blocking" => Self::Blocking,
            "async" => Self::Async,
            other => panic!("unknown observer kind: {other}"),
        }
    }

    const fn as_str(self) -> &'static str {
        match self {
            Self::Blocking => "blocking",
            Self::Async => "async",
            Self::Detached => "detached",
        }
    }
}

#[derive(Clone, Debug)]
struct Entry {
    id: u64,
    target: u8,
    wire: String,
    phase: Phase,
    observer: Observer,
    scheduler_deadline: Option<u64>,
}

#[derive(Clone, Debug)]
struct Terminal {
    value: String,
    source: &'static str,
}

#[derive(Debug)]
struct LifecycleModel {
    session: SessionState,
    transport: Transport,
    capacity: usize,
    cancel_supported: bool,
    next_id: u64,
    next_tx: u64,
    labels: HashMap<String, u64>,
    entries: BTreeMap<u64, Entry>,
    retained: HashMap<u64, Terminal>,
    applied_subscribers: Vec<(String, u8)>,
    cancel_transmissions: usize,
}

impl Default for LifecycleModel {
    fn default() -> Self {
        Self {
            session: SessionState::Shutdown,
            transport: Transport::Datagram,
            capacity: 0,
            cancel_supported: false,
            next_id: 1,
            next_tx: 1,
            labels: HashMap::new(),
            entries: BTreeMap::new(),
            retained: HashMap::new(),
            applied_subscribers: Vec::new(),
            cancel_transmissions: 0,
        }
    }
}

#[derive(Debug)]
struct TraceLine<'a> {
    at: u64,
    kind: &'a str,
    action: &'a str,
    positional: Vec<&'a str>,
    fields: HashMap<&'a str, &'a str>,
}

impl<'a> TraceLine<'a> {
    fn parse(line: &'a str) -> Self {
        let mut parts = line.split_ascii_whitespace();
        let at = parts
            .next()
            .expect("trace line timestamp")
            .parse()
            .expect("integer trace timestamp");
        let kind = parts.next().expect("trace record kind");
        let action = parts.next().expect("trace action");
        let mut positional = Vec::new();
        let mut fields = HashMap::new();
        for part in parts {
            if let Some((key, value)) = part.split_once('=') {
                assert!(fields.insert(key, value).is_none(), "duplicate field {key}");
            } else {
                positional.push(part);
            }
        }
        Self {
            at,
            kind,
            action,
            positional,
            fields,
        }
    }

    fn field(&self, name: &str) -> &'a str {
        self.fields
            .get(name)
            .copied()
            .unwrap_or_else(|| panic!("missing {name} on {}", self.action))
    }

    fn optional(&self, name: &str) -> Option<&'a str> {
        self.fields.get(name).copied()
    }

    fn position(&self, index: usize) -> &'a str {
        self.positional
            .get(index)
            .copied()
            .unwrap_or_else(|| panic!("missing positional argument {index} on {}", self.action))
    }
}

impl LifecycleModel {
    fn handle(&mut self, input: &TraceLine<'_>) -> Vec<String> {
        assert_eq!(input.kind, "input", "only input records drive the model");
        match input.action {
            "session" => self.start_session(input),
            "admit" => self.admit(input),
            "blocking-submit" => self.blocking_submit(input),
            "dispatch" => self.dispatch(input),
            "frame" => self.frame(input),
            "blocking-wait" => self.blocking_wait(input),
            "observer-timeout" => self.observer_timeout(input),
            "detach" => self.detach(input),
            "subscribe-applied" => self.subscribe_applied(input),
            "wake" => self.wake(input.at),
            "cancel" => self.cancel(input),
            "shutdown" => self.shutdown(input.at),
            "inspect-transmissions" => self.inspect_transmissions(input),
            action => panic!("unknown lifecycle trace input: {action}"),
        }
    }

    fn start_session(&mut self, input: &TraceLine<'_>) -> Vec<String> {
        let transport = match input.field("transport") {
            "datagram" => Transport::Datagram,
            "stream" => Transport::Stream,
            other => panic!("unknown transport: {other}"),
        };
        let capacity = input
            .field("capacity")
            .parse()
            .expect("integer admission capacity");
        let cancel_supported = match input.field("cancel") {
            "supported" => true,
            "unsupported" => false,
            other => panic!("unknown cancellation policy: {other}"),
        };

        *self = Self {
            session: SessionState::Running,
            transport,
            capacity,
            cancel_supported,
            ..Self::default()
        };
        vec![format!(
            "{} effect session state=running transport={} capacity={} cancel={}",
            input.at,
            transport.as_str(),
            capacity,
            input.field("cancel")
        )]
    }

    fn admit(&mut self, input: &TraceLine<'_>) -> Vec<String> {
        let ticket = input.position(0);
        if self.session != SessionState::Running {
            let error = match self.session {
                SessionState::Poisoned => "StreamPoisoned",
                SessionState::Shutdown => "RuntimeShutdown",
                SessionState::Running => unreachable!(),
            };
            return vec![format!(
                "{} outcome admission ticket={ticket} error={error} engine-entry=none observer=none transmission=none",
                input.at
            )];
        }
        if self.entries.len() >= self.capacity {
            return vec![format!(
                "{} outcome admission ticket={ticket} error=Capacity capacity={} engine-entry=none observer=none transmission=none",
                input.at, self.capacity
            )];
        }

        let id = self.next_id;
        self.next_id += 1;
        let target = input
            .field("target")
            .parse()
            .expect("integer camera target");
        let observer = Observer::parse(input.field("observer"));
        let scheduler_deadline = input
            .optional("scheduler")
            .map(|value| value.parse().expect("integer scheduler deadline"));
        let entry = Entry {
            id,
            target,
            wire: input.field("wire").into(),
            phase: Phase::Ready,
            observer,
            scheduler_deadline,
        };
        assert!(self.labels.insert(ticket.into(), id).is_none());
        assert!(self.entries.insert(id, entry).is_none());
        vec![
            format!(
                "{} effect admitted ticket={ticket} id={id} target={target} permit=acquired state=ready observer={}",
                input.at,
                observer.as_str()
            ),
            format!("{} outcome admission ticket={ticket} id={id}", input.at),
        ]
    }

    fn blocking_submit(&mut self, input: &TraceLine<'_>) -> Vec<String> {
        let mut records = self.admit(input);
        let ticket = input.position(0);
        let id = self.id(ticket);
        records.extend(self.dispatch_id(input.at, id, "ok"));
        records.push(format!(
            "{} outcome blocking-handle ticket={ticket} id={id} initial-write=complete",
            input.at
        ));
        records
    }

    fn dispatch(&mut self, input: &TraceLine<'_>) -> Vec<String> {
        let id = self.id(input.position(0));
        self.dispatch_id(input.at, id, input.field("result"))
    }

    fn dispatch_id(&mut self, at: u64, id: u64, result: &str) -> Vec<String> {
        let entry = self.entries.get_mut(&id).expect("dispatch active entry");
        assert_eq!(entry.phase, Phase::Ready, "only ready work is dispatchable");
        entry.phase = Phase::AwaitingAck;
        let tx = self.next_tx;
        self.next_tx += 1;
        let mut records = vec![
            format!("{at} effect state id={id} from=ready to=sending"),
            format!(
                "{at} effect transmit tx={tx} id={id} kind=request target={} wire={}",
                entry.target, entry.wire
            ),
            format!("{at} driver-input transmission-finished tx={tx} result={result}"),
        ];

        if result == "ok" {
            records.push(format!(
                "{at} effect state id={id} from=sending to=awaiting-ack"
            ));
            return records;
        }

        if self.transport == Transport::Datagram {
            records.extend(self.terminal(
                at,
                id,
                Terminal {
                    value: format!("Error::{result}"),
                    source: "transport",
                },
            ));
        } else {
            self.session = SessionState::Poisoned;
            records.push(format!(
                "{at} effect session from=running to=poisoned cause={result}"
            ));
            let ids: Vec<_> = self.entries.keys().copied().collect();
            for active_id in ids {
                records.extend(self.terminal(
                    at,
                    active_id,
                    Terminal {
                        value: "Error::StreamPoisoned".into(),
                        source: "transport",
                    },
                ));
            }
        }
        records
    }

    fn frame(&mut self, input: &TraceLine<'_>) -> Vec<String> {
        let frame_kind = input.position(0);
        let id = self.id(input.position(1));
        let bytes = input.field("bytes");
        match frame_kind {
            "ack" => {
                let socket = input.field("socket").parse().expect("integer VISCA socket");
                let entry = self.entries.get_mut(&id).expect("ACK active entry");
                assert_eq!(entry.phase, Phase::AwaitingAck, "ACK phase compatibility");
                entry.phase = Phase::Executing { socket };
                vec![
                    format!(
                        "{} effect frame-routed id={id} target={} via=target-socket socket={socket} bytes={bytes}",
                        input.at, entry.target
                    ),
                    format!(
                        "{} effect state id={id} from=awaiting-ack to=executing(socket={socket})",
                        input.at
                    ),
                ]
            }
            "complete" => {
                let entry = self.entries.get(&id).expect("completion active entry");
                let socket = match entry.phase {
                    Phase::Executing { socket } => socket,
                    other => panic!("completion incompatible with {other:?}"),
                };
                let mut records = vec![format!(
                    "{} effect frame-routed id={id} target={} via=target-socket socket={socket} bytes={bytes}",
                    input.at, entry.target
                )];
                records.extend(self.terminal(
                    input.at,
                    id,
                    Terminal {
                        value: "Applied".into(),
                        source: "protocol",
                    },
                ));
                records
            }
            "error" => {
                let entry = self.entries.get(&id).expect("error active entry");
                let socket = input.field("socket");
                let code = input.field("code");
                let target = entry.target;
                let mut records = vec![format!(
                    "{} effect frame-routed id={id} target={target} via=target-socket socket={socket} bytes={bytes}",
                    input.at
                )];
                let value = match code {
                    "02" => "Error::SyntaxError",
                    other => panic!("unsupported fixture camera error: {other}"),
                };
                records.extend(self.terminal(
                    input.at,
                    id,
                    Terminal {
                        value: value.into(),
                        source: "camera",
                    },
                ));
                records
            }
            other => panic!("unknown fixture frame: {other}"),
        }
    }

    fn terminal(&mut self, at: u64, id: u64, terminal: Terminal) -> Vec<String> {
        let entry = self.entries.remove(&id).expect("terminal active entry");
        let mut records = vec![format!(
            "{at} effect terminal id={id} outcome={} source={} state=removed permit=released",
            terminal.value, terminal.source
        )];
        match entry.observer {
            Observer::Blocking => {
                self.retained.insert(id, terminal.clone());
                records.push(format!(
                    "{at} effect outcome-retained id={id} observer=blocking"
                ));
            }
            Observer::Async => records.push(format!(
                "{at} outcome operation id={id} {} source={}",
                terminal.value, terminal.source
            )),
            Observer::Detached => records.push(format!(
                "{at} effect delivery-discarded id={id} observer=detached"
            )),
        }

        if terminal.value == "Applied" {
            for (subscriber, target) in &self.applied_subscribers {
                if *target == entry.target {
                    records.push(format!(
                        "{at} outcome applied-state observer={subscriber} target={target} id={id} state=applied"
                    ));
                }
            }
        }
        records
    }

    fn blocking_wait(&mut self, input: &TraceLine<'_>) -> Vec<String> {
        let id = self.id(input.position(0));
        let terminal = self.retained.remove(&id).expect("retained blocking result");
        vec![format!(
            "{} outcome blocking-wait id={id} {} source={}",
            input.at, terminal.value, terminal.source
        )]
    }

    fn observer_timeout(&mut self, input: &TraceLine<'_>) -> Vec<String> {
        let id = self.id(input.position(0));
        let entry = self.entries.get_mut(&id).expect("timeout active observer");
        let phase = entry.phase.trace_name();
        entry.observer = Observer::Detached;
        vec![
            format!(
                "{} outcome operation id={id} Error::Timeout source=observer",
                input.at
            ),
            format!(
                "{} effect observer-detached id={id} protocol-state={phase} routing=retained",
                input.at
            ),
        ]
    }

    fn detach(&mut self, input: &TraceLine<'_>) -> Vec<String> {
        let id = self.id(input.position(0));
        let entry = self.entries.get_mut(&id).expect("detach active observer");
        let phase = entry.phase.trace_name();
        entry.observer = Observer::Detached;
        vec![format!(
            "{} effect observer-detached id={id} protocol-state={phase} routing=retained",
            input.at
        )]
    }

    fn subscribe_applied(&mut self, input: &TraceLine<'_>) -> Vec<String> {
        let observer = input.position(0).to_owned();
        let target = input
            .field("target")
            .parse()
            .expect("integer applied-state target");
        self.applied_subscribers.push((observer.clone(), target));
        vec![format!(
            "{} effect subscriber-added observer={observer} target={target}",
            input.at
        )]
    }

    fn wake(&mut self, at: u64) -> Vec<String> {
        let due: Vec<_> = self
            .entries
            .values()
            .filter_map(|entry| {
                entry
                    .scheduler_deadline
                    .filter(|deadline| *deadline <= at)
                    .map(|deadline| (deadline, entry.id))
            })
            .collect();
        let mut due = due;
        due.sort_unstable();
        let mut records = Vec::new();
        for (_, id) in due {
            records.extend(self.terminal(
                at,
                id,
                Terminal {
                    value: "Error::Timeout".into(),
                    source: "scheduler",
                },
            ));
        }
        records
    }

    fn cancel(&mut self, input: &TraceLine<'_>) -> Vec<String> {
        let id = self.id(input.position(0));
        let entry = self.entries.get(&id).expect("cancel active entry");
        if entry.phase == Phase::Ready {
            let mut records = vec![format!(
                "{} effect cancellation-recorded id={id} disposition=queued-local cancel-frame=none",
                input.at
            )];
            records.extend(self.terminal(
                input.at,
                id,
                Terminal {
                    value: "Cancelled".into(),
                    source: "local",
                },
            ));
            records.push(format!(
                "{} outcome cancellation id={id} Cancelled source=local",
                input.at
            ));
            return records;
        }

        if !self.cancel_supported {
            return vec![
                format!(
                    "{} outcome cancellation id={id} Error::NotSupported source=profile",
                    input.at
                ),
                format!(
                    "{} effect cancellation-ignored id={id} protocol-state={} cancel-frame=none",
                    input.at,
                    entry.phase.trace_name()
                ),
            ];
        }

        panic!("supported sent cancellation is outside this lifecycle fixture")
    }

    fn shutdown(&mut self, at: u64) -> Vec<String> {
        assert_eq!(self.session, SessionState::Running);
        self.session = SessionState::Shutdown;
        let mut records = vec![format!(
            "{at} effect session from=running to=shutdown reason=explicit"
        )];
        let ids: Vec<_> = self.entries.keys().copied().collect();
        for id in ids {
            records.extend(self.terminal(
                at,
                id,
                Terminal {
                    value: "Error::RuntimeShutdown".into(),
                    source: "shutdown",
                },
            ));
        }
        records
    }

    fn inspect_transmissions(&self, input: &TraceLine<'_>) -> Vec<String> {
        assert_eq!(input.field("kind"), "cancel");
        vec![format!(
            "{} outcome transmissions kind=cancel count={}",
            input.at, self.cancel_transmissions
        )]
    }

    fn id(&self, label: &str) -> u64 {
        *self
            .labels
            .get(label)
            .unwrap_or_else(|| panic!("unknown request label: {label}"))
    }
}

fn fixture_records(fixture: &str) -> Vec<&str> {
    fixture
        .lines()
        .map(str::trim)
        .filter(|line| !line.is_empty() && !line.starts_with('#'))
        .collect()
}

fn replay(fixture: &str) -> Vec<String> {
    let expected = fixture_records(fixture);
    let mut model = LifecycleModel::default();
    let mut actual = Vec::new();
    for line in &expected {
        let parsed = TraceLine::parse(line);
        if parsed.kind == "input" {
            actual.push((*line).to_owned());
            actual.extend(model.handle(&parsed));
        }
    }
    assert_eq!(actual, expected, "lifecycle trace replay changed");
    actual
}

#[test]
fn blocking_handles_follow_initial_write_and_retain_exact_out_of_order_outcomes() {
    let records = replay(BLOCKING_OUT_OF_ORDER);
    let first_write = records
        .iter()
        .position(|line| line.contains("transmission-finished tx=1 result=ok"))
        .expect("first initial write result");
    let first_handle = records
        .iter()
        .position(|line| line.contains("blocking-handle ticket=a"))
        .expect("first blocking handle");
    assert!(
        first_write < first_handle,
        "a blocking handle is returned only after its initial write completes"
    );
}

#[test]
fn observer_timeout_and_detach_preserve_late_routing_and_target_delivery() {
    let records = replay(OBSERVER_LATE_DELIVERY);
    assert!(records.iter().any(|line| {
        line.contains("observer-detached id=1") && line.contains("routing=retained")
    }));
    assert!(records.iter().any(|line| {
        line.contains("applied-state observer=target-one") && line.contains("target=1 id=1")
    }));
}

#[test]
fn observer_scheduler_and_transport_deadlines_are_distinct() {
    let records = replay(DEADLINE_CLASSES);
    for source in ["observer", "scheduler", "transport"] {
        assert!(
            records
                .iter()
                .any(|line| line.contains(&format!("source={source}"))),
            "fixture must expose the {source} deadline source"
        );
    }
}

#[test]
fn capacity_datagram_stream_poison_and_shutdown_boundaries_are_distinct() {
    replay(CAPACITY_AND_FAILURES);
}

#[test]
fn ptzoptics_g2_queued_cancel_is_local_but_sent_cancel_is_unsupported() {
    let records = replay(PTZOPTICS_CANCEL);
    assert!(records.iter().any(|line| line
        .contains("cancellation-recorded id=1 disposition=queued-local cancel-frame=none")));
    assert!(records
        .iter()
        .any(|line| { line.contains("Error::NotSupported source=profile") }));
    assert!(records
        .iter()
        .any(|line| line.ends_with("transmissions kind=cancel count=0")));
}
