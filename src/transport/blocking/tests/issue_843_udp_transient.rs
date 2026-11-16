//! Real UDP ICMP port-unreachable faults recover on every built-in runtime.
//!
//! The observing wrapper forwards the OS error unchanged, then starts the
//! camera at the previously closed port. Recovery therefore requires the
//! production UDP seam to classify the fault, and the owner to retry the
//! same Sony request on the still-running session.

#![cfg(any(
    feature = "blocking",
    feature = "runtime-tokio",
    feature = "runtime-smol"
))]
#![allow(clippy::expect_used, clippy::unwrap_used)]

use std::{
    io::ErrorKind,
    net::{SocketAddr, UdpSocket},
    sync::{Arc, Mutex},
    thread::{self, JoinHandle},
    time::Duration,
};

use crate::config::SessionConfig;
use crate::testing::frames;
use crate::{
    observability::{MetricsSnapshot, SessionStatus},
    operation::AppliedOnly,
    profiles::SonyFr7,
    transport::{HasTransportConfig, ReceiveOutcome, TransportConfig},
    Error,
};

fn sony_session_config() -> SessionConfig {
    SessionConfig::from_compile_time::<SonyFr7>().expect("Sony FR7 profile")
}

struct Recovery {
    endpoint: SocketAddr,
    first_fault: Mutex<Option<ErrorKind>>,
    writes: Mutex<Vec<Vec<u8>>>,
    camera: Mutex<Option<JoinHandle<()>>>,
}

impl Recovery {
    fn new() -> Arc<Self> {
        let reserved = UdpSocket::bind("127.0.0.1:0").expect("reserve camera endpoint");
        let endpoint = reserved.local_addr().unwrap();
        drop(reserved);
        Arc::new(Self {
            endpoint,
            first_fault: Mutex::new(None),
            writes: Mutex::new(Vec::new()),
            camera: Mutex::new(None),
        })
    }

    fn observe_receive(&self, result: &Result<ReceiveOutcome, Error>) {
        let Err(Error::Io { source: error }) = result else {
            return;
        };
        let mut first_fault = self.first_fault.lock().unwrap();
        if first_fault.is_some() {
            return;
        }
        *first_fault = Some(error.kind());

        // Reopen only after the real receive fault has reached the wrapper,
        // so no successful response can stand in for ICMP evidence.
        let camera = UdpSocket::bind(self.endpoint).expect("camera comes back on the same port");
        camera
            .set_read_timeout(Some(Duration::from_secs(3)))
            .unwrap();
        *self.camera.lock().unwrap() = Some(thread::spawn(move || {
            for _ in 0..2 {
                let mut command = [0; 128];
                let (length, source) = camera.recv_from(&mut command).expect("retried command");
                let sequence = frames::sony_sequence(&command[..length]).expect("Sony sequence");
                for response in [frames::ack(1), frames::complete(1)] {
                    camera
                        .send_to(&frames::sony_reply(sequence, &response), source)
                        .expect("camera response");
                }
            }
        }));
    }

    fn assert_recovered(&self, metrics: MetricsSnapshot) {
        assert_eq!(
            *self.first_fault.lock().unwrap(),
            Some(ErrorKind::ConnectionRefused),
            "the real port-unreachable must reach the owner as a transient UDP fault"
        );
        assert_eq!(metrics.session, SessionStatus::Running);
        assert_eq!(metrics.retries_scheduled, 1);
        let writes = self.writes.lock().unwrap();
        assert_eq!(
            writes.len(),
            3,
            "one lost command, its retry, and later work"
        );
        assert_eq!(
            frames::sony_sequence(&writes[0]),
            frames::sony_sequence(&writes[1]),
            "the receive fault retries the existing logical sequence"
        );
        assert_ne!(
            frames::sony_sequence(&writes[1]),
            frames::sony_sequence(&writes[2]),
            "later work has its own sequence"
        );
        self.camera.lock().unwrap().take().unwrap().join().unwrap();
    }
}

struct Observed<T> {
    inner: T,
    recovery: Arc<Recovery>,
}

impl<T: HasTransportConfig> HasTransportConfig for Observed<T> {
    fn identity(&self) -> crate::transport::TransportIdentity<'_> {
        self.inner.identity()
    }

    fn transport_config(&self) -> &TransportConfig {
        self.inner.transport_config()
    }
}

#[cfg(feature = "blocking")]
impl<T: crate::transport::BlockingTransport> crate::transport::BlockingTransport for Observed<T> {
    fn send_with_timeout(&mut self, bytes: &[u8], timeout: Duration) -> Result<(), Error> {
        self.inner.send_with_timeout(bytes, timeout)?;
        self.recovery.writes.lock().unwrap().push(bytes.to_vec());
        Ok(())
    }

    fn recv_into_with_timeout(
        &mut self,
        buffer: &mut [u8],
        timeout: Duration,
    ) -> Result<ReceiveOutcome, Error> {
        let received = self.inner.recv_into_with_timeout(buffer, timeout);
        self.recovery.observe_receive(&received);
        received
    }
}

#[cfg(any(feature = "runtime-tokio", feature = "runtime-smol"))]
impl<T: crate::transport::AsyncTransport> crate::transport::AsyncTransport for Observed<T> {
    async fn send(&mut self, bytes: &[u8]) -> Result<(), Error> {
        self.inner.send(bytes).await?;
        self.recovery.writes.lock().unwrap().push(bytes.to_vec());
        Ok(())
    }

    async fn recv_into(&mut self, buffer: &mut [u8]) -> Result<ReceiveOutcome, Error> {
        let received = self.inner.recv_into(buffer).await;
        self.recovery.observe_receive(&received);
        received
    }
}

#[cfg(feature = "blocking")]
#[test]
fn blocking_udp_port_unreachable_retries_and_keeps_session() {
    let recovery = Recovery::new();
    let inner = crate::transport::blocking::Udp::connect_with_config(
        &recovery.endpoint.to_string(),
        TransportConfig::for_udp(),
    )
    .expect("UDP transport");
    let session = crate::blocking::Session::open(
        Observed {
            inner,
            recovery: recovery.clone(),
        },
        &sony_session_config(),
    )
    .expect("session");
    let camera = session
        .camera::<SonyFr7>(crate::IndividualCameraId::CAMERA_1)
        .unwrap();
    camera
        .submit::<AppliedOnly, _>(&crate::command::zoom::Stop)
        .unwrap()
        .applied_with_timeout(Duration::from_secs(3))
        .expect("the ICMP fault retries instead of ending the owner");
    camera
        .submit::<AppliedOnly, _>(&crate::command::focus::Stop)
        .unwrap()
        .applied_with_timeout(Duration::from_secs(3))
        .expect("later work still applies");
    recovery.assert_recovered(session.metrics().unwrap());
    session.shutdown().unwrap();
}

#[cfg(any(feature = "runtime-tokio", feature = "runtime-smol"))]
async fn async_udp_port_unreachable_retries_and_keeps_session(
    runtime: impl crate::runtime::Runtime,
) {
    let recovery = Recovery::new();
    let inner = runtime
        .connect_udp(&recovery.endpoint.to_string(), TransportConfig::for_udp())
        .await
        .expect("UDP transport");
    let session = crate::r#async::Session::open(
        Observed {
            inner,
            recovery: recovery.clone(),
        },
        &sony_session_config(),
        runtime,
    )
    .await
    .expect("session");
    let camera = session
        .camera::<SonyFr7>(crate::IndividualCameraId::CAMERA_1)
        .unwrap();
    camera
        .submit::<AppliedOnly, _>(&crate::command::zoom::Stop)
        .await
        .unwrap()
        .applied_with_timeout(Duration::from_secs(3))
        .await
        .expect("the ICMP fault retries instead of ending the owner");
    camera
        .submit::<AppliedOnly, _>(&crate::command::focus::Stop)
        .await
        .unwrap()
        .applied_with_timeout(Duration::from_secs(3))
        .await
        .expect("later work still applies");
    recovery.assert_recovered(session.metrics().await.unwrap());
    session.shutdown().unwrap();
}

#[cfg(feature = "runtime-tokio")]
#[tokio::test]
async fn tokio_udp_port_unreachable_retries_and_keeps_session() {
    async_udp_port_unreachable_retries_and_keeps_session(
        crate::runtime::TokioRuntime::from_current().expect("Tokio runtime"),
    )
    .await;
}

#[cfg(feature = "runtime-smol")]
#[test]
fn smol_udp_port_unreachable_retries_and_keeps_session() {
    smol::block_on(async_udp_port_unreachable_retries_and_keeps_session(
        crate::runtime::SmolRuntime::new(),
    ));
}
