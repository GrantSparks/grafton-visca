//! Allocation probe for the production actor's parked receive/select loop.

use std::{
    future::Future,
    pin::Pin,
    sync::{Arc, Mutex},
    task::{Context, Poll, Waker},
};

use super::{
    adapter::AdapterFraming, boundary::OwnerSnapshot, AsyncOwnerActor, AsyncOwnerDriver,
    AsyncOwnerHandle, OwnerBuffers, OwnerReceive, RetainedStreamInput, WireWrite, WriteOutcome,
};
use crate::{
    executor::Executor,
    runtime::engine::{DecodedFrame, DecodedResponse},
    target::TargetSlot,
    transport::{
        AddressingMode, HasTransportConfig, SendSemantics, TransportIdentity,
        ValidatedTransportConfig,
    },
    Error, OperationalTuning, ProfileSpec, ViscaSocket,
};

#[derive(Default)]
struct ReceiveSignal {
    pending: bool,
    parked: bool,
    received: usize,
    waker: Option<Waker>,
}

// Local fake: replace device readiness only; the production actor selects its
// real boundary lanes and timers and drives/recycles a real inert frame turn.
struct ProbeDriver {
    config: ValidatedTransportConfig,
    signal: Arc<Mutex<ReceiveSignal>>,
}

impl HasTransportConfig for ProbeDriver {
    fn identity(&self) -> TransportIdentity<'_> {
        TransportIdentity::Custom {
            semantics: SendSemantics::Datagram,
            addressing: AddressingMode::Ip,
        }
    }

    fn transport_config(&self) -> &ValidatedTransportConfig {
        &self.config
    }
}

impl RetainedStreamInput for ProbeDriver {}

impl AsyncOwnerDriver for ProbeDriver {
    async fn write(&mut self, _write: WireWrite<'_>) -> WriteOutcome {
        WriteOutcome::NotSent(Error::InvalidState {
            reason: "receive-only select probe cannot write".into(),
        })
    }

    async fn receive(&mut self, buffers: &mut OwnerBuffers) -> OwnerReceive {
        std::future::poll_fn(|context| {
            let mut signal = self
                .signal
                .lock()
                .unwrap_or_else(|error| error.into_inner());
            if signal.pending {
                signal.pending = false;
                signal.received += 1;
                Poll::Ready(())
            } else {
                if signal
                    .waker
                    .as_ref()
                    .is_none_or(|waker| !waker.will_wake(context.waker()))
                {
                    signal.waker = Some(context.waker().clone());
                }
                signal.parked = true;
                Poll::Pending
            }
        })
        .await;
        let mut frames = buffers.take_frames();
        assert!(
            frames
                .try_push(DecodedFrame {
                    target: TargetSlot::ALL[0],
                    sequence: None,
                    response: DecodedResponse::Completion {
                        socket: Some(ViscaSocket::S1),
                    },
                })
                .is_ok(),
            "one probe frame fits the receive bound"
        );
        OwnerReceive::Frames(frames)
    }
}

pub(crate) struct SelectProbe {
    run: Pin<Box<dyn Future<Output = OwnerSnapshot> + Send>>,
    signal: Arc<Mutex<ReceiveSignal>>,
    // All boundary senders stay connected while the actor parks.
    _handle: AsyncOwnerHandle,
}

impl std::fmt::Debug for SelectProbe {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("SelectProbe")
            .finish_non_exhaustive()
    }
}

impl SelectProbe {
    pub(crate) fn new(runtime: impl Executor) -> Result<Self, Error> {
        let profile = ProfileSpec::from_compile_time::<crate::profiles::GenericVisca>()?;
        let signal = Arc::new(Mutex::new(ReceiveSignal::default()));
        let driver = ProbeDriver {
            config: ValidatedTransportConfig::default(),
            signal: Arc::clone(&signal),
        };
        let (_, policy) = AdapterFraming::for_targets(
            &driver,
            &[(TargetSlot::ALL[0], &profile)],
            crate::session_config::ValidatedTuning::validate(OperationalTuning::new(), [&profile])?,
            crate::DEFAULT_ADMISSION_CAPACITY,
            false,
        )?;
        let (handle, actor) = AsyncOwnerActor::new(policy, runtime)?;
        Ok(Self {
            run: Box::pin(actor.run(driver)),
            signal,
            _handle: handle,
        })
    }

    pub(crate) fn receive_frame(&mut self) {
        let mut signal = self
            .signal
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        signal.pending = true;
        signal.parked = false;
        if let Some(waker) = &signal.waker {
            waker.wake_by_ref();
        }
    }

    pub(crate) fn poll_parked(&mut self, context: &mut Context<'_>) -> Result<usize, Error> {
        for _ in 0..4 {
            if self.run.as_mut().poll(context).is_ready() {
                return Err(Error::InvalidState {
                    reason: "select probe actor unexpectedly terminated".into(),
                });
            }
            let signal = self
                .signal
                .lock()
                .unwrap_or_else(|error| error.into_inner());
            if signal.parked {
                return Ok(signal.received);
            }
        }
        Err(Error::InvalidState {
            reason: "select probe actor did not park after bounded handoffs".into(),
        })
    }
}
