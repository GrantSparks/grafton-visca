//! The blocking owner's time source.
//!
//! The worker and every blocking handle read time, sleep and wait for
//! boundary events through one [`Clock`]. Production uses [`Clock::System`],
//! a direct call to `Instant::now`, `thread::sleep` or the `flume` wait. A
//! test may run the owner on a [`ManualClock`] instead, so its deadlines are
//! virtual. Without `test` or `test-utils` the manual variant does not exist,
//! and every operation below compiles to the system call alone.

use std::{
    thread,
    time::{Duration, Instant},
};

#[cfg(any(test, feature = "test-utils"))]
use crate::testing::manual_clock::{Bell, ManualClock, PendingParticipant, Source, Woken};

/// Where the blocking owner reads time.
#[derive(Debug, Clone, Default)]
pub(crate) enum Clock {
    /// The monotonic system clock.
    #[default]
    System,
    /// A test's virtual clock.
    #[cfg(any(test, feature = "test-utils"))]
    Manual(ManualClock),
}

impl Clock {
    /// The current instant.
    #[inline]
    pub(crate) fn now(&self) -> Instant {
        match self {
            Self::System => Instant::now(),
            #[cfg(any(test, feature = "test-utils"))]
            Self::Manual(clock) => clock.now(),
        }
    }

    /// Block the calling thread for `duration`.
    pub(crate) fn sleep(&self, duration: Duration) {
        match self {
            Self::System => thread::sleep(duration),
            #[cfg(any(test, feature = "test-utils"))]
            Self::Manual(clock) => clock.sleep(duration),
        }
    }

    /// Send `message` on a bounded `lane`, or give up once `deadline` passes
    /// while it is full. `None` is the deadline; the message is dropped.
    pub(crate) fn send_until<T>(
        &self,
        lane: &flume::Sender<T>,
        message: T,
        deadline: Instant,
    ) -> Option<Result<(), flume::SendError<T>>> {
        match self {
            Self::System => match lane.send_deadline(message, deadline) {
                Ok(()) => Some(Ok(())),
                Err(flume::SendTimeoutError::Timeout(_)) => None,
                Err(flume::SendTimeoutError::Disconnected(message)) => {
                    Some(Err(flume::SendError(message)))
                }
            },
            #[cfg(any(test, feature = "test-utils"))]
            Self::Manual(clock) => {
                let mut message = Some(message);
                clock.wait_until(deadline, || match lane.try_send(message.take()?) {
                    Ok(()) => Some(Ok(())),
                    Err(flume::TrySendError::Full(full)) => {
                        message = Some(full);
                        None
                    }
                    Err(flume::TrySendError::Disconnected(lost)) => {
                        Some(Err(flume::SendError(lost)))
                    }
                })
            }
        }
    }

    /// Block until every sender of `lane` is gone, discarding anything sent.
    pub(crate) fn await_disconnect<T>(&self, lane: &flume::Receiver<T>) {
        match self {
            Self::System => while lane.recv().is_ok() {},
            #[cfg(any(test, feature = "test-utils"))]
            Self::Manual(_) => while self.select().recv(lane, |sent| sent.is_ok()).wait() {},
        }
    }

    /// A wait for the first of several channel receives.
    pub(crate) fn select<'a, T>(&'a self) -> Select<'a, T> {
        Select(match self {
            Self::System => Selection::System(flume::Selector::new()),
            #[cfg(any(test, feature = "test-utils"))]
            Self::Manual(clock) => Selection::Manual {
                clock,
                arms: Vec::new(),
            },
        })
    }

    /// A participant registration for a thread about to be spawned, so a
    /// virtual clock counts the thread as running from before it starts.
    pub(crate) fn register_thread(&self) -> ThreadRegistration {
        ThreadRegistration {
            #[cfg(any(test, feature = "test-utils"))]
            pending: match self {
                Self::System => None,
                Self::Manual(clock) => Some(clock.register()),
            },
        }
    }
}

/// A thread's pending registration with the owner's clock.
#[derive(Debug)]
pub(crate) struct ThreadRegistration {
    #[cfg(any(test, feature = "test-utils"))]
    pending: Option<PendingParticipant>,
}

impl ThreadRegistration {
    /// Bind the registration to the calling thread for the lifetime of the
    /// returned guard.
    pub(crate) fn enter(self) -> ThreadParticipation {
        ThreadParticipation {
            #[cfg(any(test, feature = "test-utils"))]
            _participant: self.pending.map(PendingParticipant::enter),
        }
    }
}

/// Holds a thread's participation in the owner's clock.
#[derive(Debug)]
pub(crate) struct ThreadParticipation {
    #[cfg(any(test, feature = "test-utils"))]
    _participant: Option<crate::testing::manual_clock::ClockParticipant>,
}

/// A [`flume::Selector`] measured against the owner's [`Clock`].
pub(crate) struct Select<'a, T>(Selection<'a, T>);

enum Selection<'a, T> {
    System(flume::Selector<'a, T>),
    #[cfg(any(test, feature = "test-utils"))]
    Manual {
        clock: &'a ManualClock,
        arms: Vec<Box<dyn Arm<'a, T> + 'a>>,
    },
}

impl<'a, T: 'a> Select<'a, T> {
    /// Add a receive from `receiver`, mapped to the wait's value by `map`.
    pub(crate) fn recv<U, F>(self, receiver: &'a flume::Receiver<U>, map: F) -> Self
    where
        U: 'a,
        F: FnMut(Result<U, flume::RecvError>) -> T + 'a,
    {
        Self(match self.0 {
            Selection::System(selector) => Selection::System(selector.recv(receiver, map)),
            #[cfg(any(test, feature = "test-utils"))]
            Selection::Manual { clock, mut arms } => {
                arms.push(Box::new(RecvArm { receiver, map }));
                Selection::Manual { clock, arms }
            }
        })
    }

    /// Wait until one receive completes.
    pub(crate) fn wait(self) -> T {
        match self.0 {
            Selection::System(selector) => selector.wait(),
            #[cfg(any(test, feature = "test-utils"))]
            Selection::Manual { clock, arms } => {
                let mut arms = Arms(arms);
                // With no deadline the wait ends only with a value.
                loop {
                    if let Some(value) = clock.wait_on(None, &mut arms) {
                        break value;
                    }
                }
            }
        }
    }

    /// Wait until one receive completes, or `None` once `deadline` passes
    /// first.
    pub(crate) fn wait_until(self, deadline: Instant) -> Option<T> {
        match self.0 {
            Selection::System(selector) => selector.wait_deadline(deadline).ok(),
            #[cfg(any(test, feature = "test-utils"))]
            Selection::Manual { clock, arms } => clock.wait_on(Some(deadline), &mut Arms(arms)),
        }
    }
}

/// One receive of a manual-clock selection.
#[cfg(any(test, feature = "test-utils"))]
trait Arm<'a, T> {
    /// The receive's value if it is ready now.
    fn try_now(&mut self) -> Option<T>;

    /// Add the receive to `selector`.
    fn select<'s>(
        &'s mut self,
        selector: flume::Selector<'s, Woken<T>>,
    ) -> flume::Selector<'s, Woken<T>>
    where
        'a: 's,
        T: 's;
}

#[cfg(any(test, feature = "test-utils"))]
struct RecvArm<'a, U, F> {
    receiver: &'a flume::Receiver<U>,
    map: F,
}

#[cfg(any(test, feature = "test-utils"))]
impl<'a, T, U, F> Arm<'a, T> for RecvArm<'a, U, F>
where
    F: FnMut(Result<U, flume::RecvError>) -> T,
{
    fn try_now(&mut self) -> Option<T> {
        match self.receiver.try_recv() {
            Ok(value) => Some((self.map)(Ok(value))),
            Err(flume::TryRecvError::Disconnected) => {
                Some((self.map)(Err(flume::RecvError::Disconnected)))
            }
            Err(flume::TryRecvError::Empty) => None,
        }
    }

    fn select<'s>(
        &'s mut self,
        selector: flume::Selector<'s, Woken<T>>,
    ) -> flume::Selector<'s, Woken<T>>
    where
        'a: 's,
        T: 's,
    {
        let map = &mut self.map;
        selector.recv(self.receiver, move |received| Woken::Value(map(received)))
    }
}

/// A manual-clock selection's receives, in the order they were added.
#[cfg(any(test, feature = "test-utils"))]
struct Arms<'a, T>(Vec<Box<dyn Arm<'a, T> + 'a>>);

#[cfg(any(test, feature = "test-utils"))]
impl<'a, T: 'a> Source<T> for Arms<'a, T> {
    fn poll(&mut self) -> Option<T> {
        self.0.iter_mut().find_map(|arm| arm.try_now())
    }

    fn block(&mut self, bell: &flume::Receiver<Bell>) -> Woken<T> {
        let mut selector = flume::Selector::new();
        for arm in &mut self.0 {
            selector = arm.select(selector);
        }
        selector
            .recv(bell, |rung| Woken::Bell(rung.unwrap_or(Bell::Expired)))
            .wait()
    }
}
