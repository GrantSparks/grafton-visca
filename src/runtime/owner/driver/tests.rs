#![allow(clippy::expect_used, clippy::panic, clippy::unnecessary_wraps)]

use std::{
    cell::RefCell,
    collections::VecDeque,
    future::Future,
    pin::pin,
    rc::Rc,
    sync::Arc,
    task::{Context, Poll, Wake, Waker},
    time::{Duration, Instant},
};

type Trace = Rc<RefCell<Vec<&'static str>>>;
type OwnerSnapshot = Trace;
type OwnerInputTurn = ();
type Effect = ();

#[derive(Clone, Copy)]
enum OwnerEvent {
    Admission,
    Control(()),
    Ack,
    Completion,
    Shutdown,
}

struct ClassifiedEvent {
    event: OwnerEvent,
    selected: OwnerEvent,
    suppress_due: bool,
}

enum TurnPlan {
    Redeliver(OwnerEvent),
    Select(OwnerEvent),
}

#[derive(Clone, Copy)]
enum TurnOutcome {
    Continue,
    Stop,
}

struct Resume(TurnOutcome);

impl Resume {
    fn input_turn(&self) -> Option<&OwnerInputTurn> {
        None
    }
}

enum TurnStep {
    Drive {
        effects: VecDeque<Effect>,
        then: Resume,
    },
    Pace {
        pause: (),
        outcome: TurnOutcome,
    },
    Done(TurnOutcome),
}

enum WriteOutcome {
    NotSent(crate::Error),
    Sent,
}

struct Policy {
    write_timeout: Duration,
}

struct State {
    trace: Trace,
    policy: Policy,
    refuse_write: bool,
}

impl State {
    fn policy(&self) -> &Policy {
        &self.policy
    }

    fn prepare_write(&mut self, _staged: &()) -> Result<(), crate::Error> {
        self.trace.borrow_mut().push("prepare_write");
        if self.refuse_write {
            Err(crate::Error::InvalidRequest {
                reason: "test admission refusal".into(),
            })
        } else {
            Ok(())
        }
    }
}

struct Core {
    state: State,
    script: VecDeque<OwnerEvent>,
    running: bool,
}

impl Core {
    fn is_running(&self) -> bool {
        self.state.trace.borrow_mut().push("is_running");
        self.running
    }

    fn begin_turn(&mut self) -> bool {
        self.state.trace.borrow_mut().push("begin_turn");
        true
    }

    fn plan<D>(&mut self, _now: Instant, _driver: &mut D) -> TurnPlan {
        self.state.trace.borrow_mut().push("plan");
        let event = self.script.pop_front().expect("bounded test script");
        if matches!(event, OwnerEvent::Completion) {
            TurnPlan::Redeliver(event)
        } else {
            TurnPlan::Select(event)
        }
    }

    fn redeliver(&mut self, event: OwnerEvent) -> OwnerEvent {
        self.state.trace.borrow_mut().push("redeliver");
        event
    }

    fn classify(&mut self, event: OwnerEvent, _at: Instant) -> Option<ClassifiedEvent> {
        self.state.trace.borrow_mut().push("classify");
        Some(ClassifiedEvent {
            event,
            selected: event,
            suppress_due: false,
        })
    }

    fn control_consumed_wake(&self, _at: Instant) -> Option<()> {
        self.state.trace.borrow_mut().push("control_consumed_wake");
        None
    }

    fn handle<D>(
        &mut self,
        event: OwnerEvent,
        _driver: &mut D,
        _at: Instant,
        suppress_due: bool,
    ) -> TurnStep {
        assert!(!suppress_due);
        self.state.trace.borrow_mut().push("handle");
        if matches!(event, OwnerEvent::Control(_)) {
            return TurnStep::Pace {
                pause: (),
                outcome: TurnOutcome::Continue,
            };
        }
        let effects = if matches!(event, OwnerEvent::Admission) {
            VecDeque::from([()])
        } else {
            VecDeque::new()
        };
        TurnStep::Drive {
            effects,
            then: Resume(if matches!(event, OwnerEvent::Shutdown) {
                TurnOutcome::Stop
            } else {
                TurnOutcome::Continue
            }),
        }
    }

    fn next_transmit(&mut self, effects: &mut VecDeque<Effect>, _at: Instant) -> Option<()> {
        self.state.trace.borrow_mut().push("next_transmit");
        effects.pop_front()
    }

    fn finish_transmit(
        &mut self,
        _turn: Option<&OwnerInputTurn>,
        _staged: &(),
        result: WriteOutcome,
        _at: Instant,
        _effects: &mut VecDeque<Effect>,
    ) {
        let label = match result {
            WriteOutcome::Sent => "finish_transmit:sent",
            WriteOutcome::NotSent(cause) => {
                assert_eq!(cause.kind(), crate::ErrorKind::InvalidParameter);
                "finish_transmit:not_sent"
            }
        };
        self.state.trace.borrow_mut().push(label);
    }

    fn resume<D>(&mut self, then: Resume, _driver: &mut D) -> TurnStep {
        self.state.trace.borrow_mut().push("resume");
        TurnStep::Done(then.0)
    }

    fn pace_receive(&mut self, _pause: (), _at: Instant) {
        self.state.trace.borrow_mut().push("pace_receive");
    }

    fn finish(
        &mut self,
        _selected: OwnerEvent,
        _control_consumed_wake: Option<()>,
        outcome: TurnOutcome,
    ) -> bool {
        self.state.trace.borrow_mut().push("finish");
        self.running = !matches!(outcome, TurnOutcome::Stop);
        self.running
    }

    fn conclude(&mut self) -> OwnerSnapshot {
        self.state.trace.borrow_mut().push("conclude");
        Rc::clone(&self.state.trace)
    }
}

impl Drop for Core {
    fn drop(&mut self) {
        self.state.trace.borrow_mut().push("core:drop");
    }
}

trait Driver {}

struct ScriptedDriver(Trace);

impl Driver for ScriptedDriver {}

impl Drop for ScriptedDriver {
    fn drop(&mut self) {
        self.0.borrow_mut().push("driver:drop");
    }
}

macro_rules! test_shell {
    ($shell:ident, $io:ident, [$($async:tt)*], [$($await:tt)*]) => {
        struct $shell {
            core: Core,
            io: $io,
        }

        impl $shell {
            owner_turn_driver! {
                async: [$($async)*],
                await: [$($await)*],
                driver: Driver,
            }
        }

        struct $io {
            trace: Trace,
            now: Instant,
        }

        impl $io {
            fn now(&self) -> Instant {
                self.now
            }

            $($async)* fn cooperative_handoff(&mut self) {
                self.trace.borrow_mut().push("cooperative_handoff");
            }

            $($async)* fn select_event<D: Driver>(
                &mut self,
                _core: &mut Core,
                selected: OwnerEvent,
                _driver: &mut D,
            ) -> OwnerEvent {
                self.trace.borrow_mut().push("select_event");
                selected
            }

            $($async)* fn write<D: Driver>(
                &mut self,
                _driver: &mut D,
                _wire: (),
                timeout: Duration,
            ) -> WriteOutcome {
                assert_eq!(timeout, Duration::from_secs(1));
                self.trace.borrow_mut().push("write");
                WriteOutcome::Sent
            }
        }
    };
}

test_shell!(BlockingShell, BlockingIo, [], []);
test_shell!(AsyncShell, AsyncIo, [async], [.await]);

fn core(trace: Trace, refuse_write: bool) -> Core {
    Core {
        state: State {
            trace,
            policy: Policy {
                write_timeout: Duration::from_secs(1),
            },
            refuse_write,
        },
        script: VecDeque::from([
            OwnerEvent::Admission,
            OwnerEvent::Control(()),
            OwnerEvent::Ack,
            OwnerEvent::Completion,
            OwnerEvent::Shutdown,
        ]),
        running: true,
    }
}

struct NeverWoken;

impl Wake for NeverWoken {
    fn wake(self: Arc<Self>) {
        unreachable!("the scripted async hooks complete without parking");
    }
}

fn complete(future: impl Future<Output = Trace>) -> Trace {
    let mut future = pin!(future);
    let waker = Waker::from(Arc::new(NeverWoken));
    match future.as_mut().poll(&mut Context::from_waker(&waker)) {
        Poll::Ready(trace) => trace,
        Poll::Pending => unreachable!("the scripted async hooks are ready"),
    }
}

#[test]
fn expanded_shells_share_the_entire_core_call_order_and_teardown_barrier() {
    for refuse_write in [false, true] {
        let blocking_trace = Trace::default();
        let blocking = BlockingShell {
            core: core(Rc::clone(&blocking_trace), refuse_write),
            io: BlockingIo {
                trace: Rc::clone(&blocking_trace),
                now: Instant::now(),
            },
        }
        .run(ScriptedDriver(Rc::clone(&blocking_trace)));

        let async_trace = Trace::default();
        let asynchronous = complete(
            AsyncShell {
                core: core(Rc::clone(&async_trace), refuse_write),
                io: AsyncIo {
                    trace: Rc::clone(&async_trace),
                    now: Instant::now(),
                },
            }
            .run(ScriptedDriver(Rc::clone(&async_trace))),
        );
        assert_eq!(*blocking.borrow(), *asynchronous.borrow());
        let calls = blocking.borrow();
        let prepared = calls
            .iter()
            .position(|call| *call == "prepare_write")
            .expect("write preparation");
        let finish = calls
            .iter()
            .position(|call| call.starts_with("finish_transmit:"))
            .expect("write feedback");
        assert_eq!(finish - prepared, if refuse_write { 1 } else { 2 });
        assert_eq!(calls[finish + 1], "next_transmit");
        assert_eq!(calls[finish + 2], "resume");
        assert!(calls.contains(&"control_consumed_wake"));
        assert!(calls.contains(&"pace_receive"));
        assert!(calls.contains(&"redeliver"));
        assert_eq!(
            &calls[calls.len() - 3..],
            &["conclude", "driver:drop", "core:drop"]
        );
    }
}
