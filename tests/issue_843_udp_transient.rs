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

use grafton_visca::{
    completion::AppliedOnly,
    profiles::SonyFR7,
    request::builtin::{FocusStop, ZoomStop},
    transport::{
        AddressingMode, HasTransportConfig, ReceiveOutcome, SendSemantics, TransportConfig,
    },
    Error, MetricsSnapshot, SessionStatus,
};
use grafton_visca_test_support::{fake_camera::frames, profile_fixtures::sony_session_config};

struct StreamCamera(grafton_visca_test_support::fake_camera::FakeCamera);

impl StreamCamera {
    #[cfg(feature = "blocking")]
    fn blocking_wire(&self) -> grafton_visca_test_support::fake_camera::BlockingWire {
        self.0.blocking_wire().with_semantics(SendSemantics::Stream)
    }

    #[cfg(any(feature = "runtime-tokio", feature = "runtime-smol"))]
    fn async_wire(&self) -> grafton_visca_test_support::fake_camera::AsyncWire {
        self.0.async_wire().with_semantics(SendSemantics::Stream)
    }
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
        let Err(Error::Io(error)) = result else {
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
    fn transport_config(&self) -> &TransportConfig {
        self.inner.transport_config()
    }

    fn standard_transport_kind(&self) -> Option<grafton_visca::camera::TransportKind> {
        self.inner.standard_transport_kind()
    }
}

#[cfg(feature = "blocking")]
impl<T: grafton_visca::transport::BlockingTransport> grafton_visca::transport::BlockingTransport
    for Observed<T>
{
    fn send_with_timeout(
        &mut self,
        bytes: &[u8],
        kind: grafton_visca::command::CommandKind,
        timeout: Duration,
    ) -> Result<(), Error> {
        self.inner.send_with_timeout(bytes, kind, timeout)?;
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

    fn send_semantics(&self) -> SendSemantics {
        self.inner.send_semantics()
    }

    fn addressing_mode_hint(&self) -> Option<AddressingMode> {
        self.inner.addressing_mode_hint()
    }
}

#[cfg(any(feature = "runtime-tokio", feature = "runtime-smol"))]
impl<T: grafton_visca::transport::AsyncTransport> grafton_visca::transport::AsyncTransport
    for Observed<T>
{
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

    fn send_semantics(&self) -> SendSemantics {
        self.inner.send_semantics()
    }

    fn addressing_mode_hint(&self) -> Option<AddressingMode> {
        self.inner.addressing_mode_hint()
    }
}

#[cfg(feature = "blocking")]
#[test]
fn blocking_udp_port_unreachable_retries_and_keeps_session() {
    let recovery = Recovery::new();
    let inner = grafton_visca::transport::Transport::udp()
        .address(recovery.endpoint.to_string())
        .build_blocking()
        .expect("UDP transport");
    let session = grafton_visca::blocking::Session::open(
        Observed {
            inner,
            recovery: recovery.clone(),
        },
        sony_session_config(),
    )
    .expect("session");
    let camera = session.camera::<SonyFR7>().unwrap();
    camera
        .submit::<AppliedOnly, _>(&ZoomStop)
        .unwrap()
        .applied_with_timeout(Duration::from_secs(3))
        .expect("the ICMP fault retries instead of ending the owner");
    camera
        .submit::<AppliedOnly, _>(&FocusStop)
        .unwrap()
        .applied_with_timeout(Duration::from_secs(3))
        .expect("later work still applies");
    recovery.assert_recovered(session.metrics().unwrap());
    session.shutdown().unwrap();
}

#[cfg(any(feature = "runtime-tokio", feature = "runtime-smol"))]
async fn async_udp_port_unreachable_retries_and_keeps_session(
    runtime: impl grafton_visca::Runtime,
) {
    let recovery = Recovery::new();
    let inner = runtime
        .connect_udp(&recovery.endpoint.to_string(), TransportConfig::for_udp())
        .await
        .expect("UDP transport");
    let session = grafton_visca::Session::open(
        Observed {
            inner,
            recovery: recovery.clone(),
        },
        sony_session_config(),
        runtime,
    )
    .await
    .expect("session");
    let camera = session.camera::<SonyFR7>().unwrap();
    camera
        .submit::<AppliedOnly, _>(&ZoomStop)
        .await
        .unwrap()
        .applied_with_timeout(Duration::from_secs(3))
        .await
        .expect("the ICMP fault retries instead of ending the owner");
    camera
        .submit::<AppliedOnly, _>(&FocusStop)
        .await
        .unwrap()
        .applied_with_timeout(Duration::from_secs(3))
        .await
        .expect("later work still applies");
    recovery.assert_recovered(session.metrics().await.unwrap());
    session.shutdown().unwrap();
}

#[cfg(any(feature = "runtime-tokio", feature = "runtime-smol"))]
grafton_visca_test_support::runtime_matrix!(async_udp_port_unreachable_retries_and_keeps_session);

grafton_visca_test_support::facade_matrix! {
    fn a_stream_connection_reset_still_ends_the_session() {
        use grafton_visca_test_support::{
            fake_camera::FakeCamera,
            profile_fixtures::{session_config, NonDefaultCompileTimeProfile},
        };

        let reset = StreamCamera(FakeCamera::new(|_, answer| {
            answer.fault(Error::Io(Arc::new(ErrorKind::ConnectionReset.into())));
        }));
        let session = open!(reset, session_config()).unwrap();
        let camera = session.camera::<NonDefaultCompileTimeProfile>().unwrap();
        let error = wait!(wait!(camera.submit::<AppliedOnly, _>(&ZoomStop)).unwrap().applied())
            .expect_err("a connection reset remains session death outside the Windows UDP seam");
        assert!(error.requires_new_session(), "{error:?}");
    }
}
