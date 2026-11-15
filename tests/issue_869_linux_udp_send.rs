//! The real connected UDP send seam retains Linux's queued ICMP cause.

#![cfg(all(target_os = "linux", feature = "blocking"))]
#![allow(clippy::expect_used, clippy::unwrap_used)]

use std::{
    io,
    net::{SocketAddr, UdpSocket},
    sync::{Arc, Mutex},
    thread::{self, JoinHandle},
    time::Duration,
};

use grafton_visca::{
    command::CommandKind,
    completion::AppliedOnly,
    request::builtin::ZoomStop,
    transport::{
        AddressingMode, BlockingTransport, BlockingTransportHandle, HasTransportConfig,
        ReceiveOutcome, SendSemantics, Transport, TransportConfig,
    },
    Error, ErrorKind, SessionStatus,
};
use grafton_visca_test_support::{
    fake_camera::frames,
    profile_fixtures::{session_config, NonDefaultCompileTimeProfile},
};

struct RestartingCamera {
    endpoint: SocketAddr,
    worker: Mutex<Option<JoinHandle<()>>>,
}

impl RestartingCamera {
    fn after_send_failure(&self) {
        let socket = UdpSocket::bind(self.endpoint).expect("camera restarts on the same port");
        socket
            .set_read_timeout(Some(Duration::from_secs(3)))
            .unwrap();
        *self.worker.lock().unwrap() = Some(thread::spawn(move || {
            let mut command = [0; 128];
            let (length, peer) = socket.recv_from(&mut command).expect("later command");
            assert_eq!(&command[..length], &[0x81, 0x01, 0x04, 0x07, 0x00, 0xff]);
            for reply in [frames::ack(1), frames::complete(1)] {
                socket.send_to(&reply, peer).expect("camera response");
            }
        }));
    }
}

/// Keep the owner out of receive while Linux queues the error: the first
/// native send targets the closed port, and the second native send observes
/// its ICMP port-unreachable. No synthetic transport error enters the test.
struct PrimeSend {
    inner: BlockingTransportHandle,
    first: bool,
    camera: Arc<RestartingCamera>,
}

impl HasTransportConfig for PrimeSend {
    fn transport_config(&self) -> &TransportConfig {
        self.inner.transport_config()
    }

    fn standard_transport_kind(&self) -> Option<grafton_visca::camera::TransportKind> {
        self.inner.standard_transport_kind()
    }
}

impl BlockingTransport for PrimeSend {
    fn send_with_timeout(
        &mut self,
        bytes: &[u8],
        kind: CommandKind,
        timeout: Duration,
    ) -> Result<(), Error> {
        if !std::mem::take(&mut self.first) {
            return self.inner.send_with_timeout(bytes, kind, timeout);
        }
        self.inner.send_with_timeout(bytes, kind, timeout)?;
        let cause = self
            .inner
            .send_with_timeout(bytes, kind, timeout)
            .expect_err("the second send observes loopback ICMP port-unreachable");
        assert!(
            matches!(&cause, Error::Io(error) if error.kind() == io::ErrorKind::ConnectionRefused)
        );
        self.camera.after_send_failure();
        Err(cause)
    }

    fn recv_into_with_timeout(
        &mut self,
        bytes: &mut [u8],
        timeout: Duration,
    ) -> Result<ReceiveOutcome, Error> {
        self.inner.recv_into_with_timeout(bytes, timeout)
    }

    fn send_semantics(&self) -> SendSemantics {
        SendSemantics::Datagram
    }

    fn addressing_mode_hint(&self) -> Option<AddressingMode> {
        self.inner.addressing_mode_hint()
    }
}

#[test]
fn a_real_second_udp_send_preserves_econnrefused_and_later_work_applies() {
    let reservation = UdpSocket::bind("127.0.0.1:0").unwrap();
    let endpoint = reservation.local_addr().unwrap();
    drop(reservation);
    let camera = Arc::new(RestartingCamera {
        endpoint,
        worker: Mutex::new(None),
    });
    let inner = Transport::udp()
        .address(endpoint.to_string())
        .build_blocking()
        .expect("native connected UDP transport");
    let session = grafton_visca::blocking::Session::open(
        PrimeSend {
            inner,
            first: true,
            camera: Arc::clone(&camera),
        },
        session_config(),
    )
    .unwrap();
    let target = session.camera::<NonDefaultCompileTimeProfile>().unwrap();
    let mut first = target.submit::<AppliedOnly, _>(&ZoomStop).unwrap();
    let failure = first.applied().expect_err("the actual send failed");
    assert_eq!(failure.kind(), ErrorKind::Transport);
    assert!(!failure.requires_new_session());
    let mut source: Option<&(dyn std::error::Error + 'static)> = Some(&failure);
    let mut os_kind = None;
    while let Some(error) = source {
        if let Some(error) = error.downcast_ref::<io::Error>() {
            os_kind = Some(error.kind());
            break;
        }
        if let Some(error) = error.downcast_ref::<Arc<io::Error>>() {
            os_kind = Some(error.kind());
            break;
        }
        source = error.source();
    }
    assert_eq!(os_kind, Some(io::ErrorKind::ConnectionRefused));
    let mut next = target.submit::<AppliedOnly, _>(&ZoomStop).unwrap();
    next.applied().expect("the same UDP session remains usable");
    let metrics = session.metrics().unwrap();
    assert_eq!(metrics.session, SessionStatus::Running);
    assert_eq!(metrics.writes, 2);
    assert_eq!(metrics.write_failures, 1);
    session.close().unwrap();
    camera
        .worker
        .lock()
        .unwrap()
        .take()
        .unwrap()
        .join()
        .unwrap();
}
