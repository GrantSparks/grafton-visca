//! Public sessions reject transport callbacks without losing teardown.

#![cfg(feature = "blocking")]
#![allow(clippy::expect_used)]

use std::{
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc,
    },
    time::Duration,
};

use grafton_visca::{
    blocking::Session,
    config::SessionConfig,
    operation::AppliedOnly,
    profiles::GenericVisca,
    transport::{BlockingTransport, HasTransportConfig, ReceiveOutcome, TransportConfig},
    Error,
};

type Callback = Box<dyn FnOnce() + Send>;

// Local fake: invoking user code inside native transport I/O is the behavior
// under test; scripted peer replies cannot exercise worker-thread re-entry.
struct ReenteringTransport {
    config: TransportConfig,
    callbacks: flume::Receiver<Callback>,
    during_write: bool,
    dropped: Arc<AtomicBool>,
}

impl Drop for ReenteringTransport {
    fn drop(&mut self) {
        self.dropped.store(true, Ordering::Release);
    }
}

impl HasTransportConfig for ReenteringTransport {
    fn identity(&self) -> grafton_visca::transport::TransportIdentity<'_> {
        grafton_visca::transport::TransportIdentity::Custom {
            semantics: grafton_visca::transport::SendSemantics::Stream,
            addressing: grafton_visca::transport::AddressingMode::Ip,
        }
    }

    fn transport_config(&self) -> &TransportConfig {
        &self.config
    }
}

impl BlockingTransport for ReenteringTransport {
    fn send_with_timeout(&mut self, _bytes: &[u8], _timeout: Duration) -> Result<(), Error> {
        if self.during_write {
            if let Ok(callback) = self.callbacks.try_recv() {
                callback();
            }
        }
        Ok(())
    }

    fn recv_into_with_timeout(
        &mut self,
        _dst: &mut [u8],
        timeout: Duration,
    ) -> Result<ReceiveOutcome, Error> {
        if self.during_write {
            std::thread::sleep(timeout);
        } else if let Ok(callback) = self.callbacks.recv_timeout(timeout) {
            callback();
        }
        Err(Error::io_timeout())
    }
}

fn callback_metrics(during_write: bool) {
    let (callbacks, receiver) = flume::bounded(1);
    let dropped = Arc::new(AtomicBool::new(false));
    let session = Session::open(
        ReenteringTransport {
            config: TransportConfig::default(),
            callbacks: receiver,
            during_write,
            dropped: Arc::clone(&dropped),
        },
        &SessionConfig::from_compile_time::<GenericVisca>().expect("raw profile"),
    )
    .expect("blocking session");
    let callback_session = session.clone();
    let (answered, result) = flume::bounded(1);
    callbacks
        .send(Box::new(move || {
            answered
                .send(callback_session.metrics())
                .expect("test observes callback");
        }) as Callback)
        .expect("install callback");
    if during_write {
        let camera = session
            .camera::<GenericVisca>(grafton_visca::types::IndividualCameraId::CAMERA_1)
            .expect("typed camera");
        drop(
            camera
                .submit::<AppliedOnly, _>(&grafton_visca::command::zoom::Stop)
                .expect("admit STOP"),
        );
    }
    assert!(matches!(
        result
            .recv_timeout(Duration::from_secs(2))
            .expect("worker callback must return promptly"),
        Err(Error::InvalidState { .. })
    ));
    session
        .metrics()
        .expect("worker still serves external calls");
    session.close().expect("external close joins the worker");
    assert!(dropped.load(Ordering::Acquire));
}

#[test]
fn public_read_callback_cannot_deadlock_its_session() {
    callback_metrics(false);
}

#[test]
fn public_write_callback_cannot_deadlock_its_session() {
    callback_metrics(true);
}
