//! A receive must remove only the bytes requested by the caller from TCP.
//!
//! Looking at the Linux receive queue distinguishes a direct read from a
//! buffered read that returns one byte while hiding the rest in user space.

use std::{
    io::{Read, Write},
    net::{SocketAddr, TcpListener, TcpStream},
    time::{Duration, Instant},
};

use crate::transport::{ReceiveOutcome, TransportConfig};

const PAYLOAD: [u8; 4_000] = [0xa5; 4_000];
const BOUND: Duration = Duration::from_secs(2);

struct QueuedPeer {
    stream: TcpStream,
    client: String,
    server: String,
}

impl QueuedPeer {
    fn new(listener: &TcpListener) -> Self {
        fn proc_address(address: SocketAddr) -> String {
            let SocketAddr::V4(address) = address else {
                unreachable!("the test binds an IPv4 listener");
            };
            format!(
                "{:08X}:{:04X}",
                u32::from_ne_bytes(address.ip().octets()),
                address.port()
            )
        }

        let (mut stream, client) = listener.accept().expect("accept connected client");
        let server = stream.local_addr().expect("server address");
        stream
            .set_read_timeout(Some(BOUND))
            .expect("peer read bound");
        stream
            .set_write_timeout(Some(BOUND))
            .expect("peer write bound");
        stream.write_all(&PAYLOAD).expect("queue test payload");
        let peer = Self {
            stream,
            client: proc_address(client),
            server: proc_address(server),
        };
        peer.assert_queued(PAYLOAD.len());
        peer
    }

    fn queued(&self) -> usize {
        let table = std::fs::read_to_string("/proc/net/tcp").expect("Linux TCP socket table");
        let queue = table
            .lines()
            .skip(1)
            .find_map(|line| {
                let mut columns = line.split_whitespace();
                columns.next()?; // table index
                let local = columns.next()?;
                let remote = columns.next()?;
                columns.next()?; // connection state
                let queues = columns.next()?;
                (local == self.client && remote == self.server).then_some(queues)
            })
            .expect("connected client appears in the kernel socket table");
        let (_, receive) = queue
            .split_once(':')
            .expect("transmit:receive queue counts");
        usize::from_str_radix(receive, 16).expect("hexadecimal receive queue size")
    }

    fn assert_queued(&self, expected: usize) {
        let deadline = Instant::now() + BOUND;
        loop {
            let queued = self.queued();
            if queued == expected {
                return;
            }
            assert!(
                Instant::now() < deadline,
                "expected {expected} bytes in the kernel receive queue, found {queued}"
            );
            std::thread::sleep(Duration::from_millis(1));
        }
    }

    fn assert_outbound(&mut self) {
        let mut request = [0; b"outbound".len()];
        self.stream
            .read_exact(&mut request)
            .expect("outbound bytes");
        assert_eq!(&request, b"outbound");
    }
}

fn listener() -> TcpListener {
    TcpListener::bind("127.0.0.1:0").expect("loopback listener")
}

#[cfg(feature = "blocking")]
#[test]
fn blocking_reads_leave_unrequested_bytes_in_kernel() {
    use crate::{command::CommandKind, transport::BlockingTransport};

    let listener = listener();
    let mut transport = crate::transport::blocking::Tcp::connect_with_config(
        &listener.local_addr().expect("listener address").to_string(),
        TransportConfig::for_tcp(),
    )
    .expect("connect blocking transport");
    let mut peer = QueuedPeer::new(&listener);

    let mut first = [0];
    assert_eq!(
        transport
            .recv_into_with_timeout(&mut first, BOUND)
            .expect("one-byte receive"),
        ReceiveOutcome::complete(1)
    );
    assert_eq!(first, PAYLOAD[..1]);
    peer.assert_queued(PAYLOAD.len() - first.len());

    transport
        .send_with_timeout(b"outbound", CommandKind::Command, BOUND)
        .expect("write through the same stream after a read");
    peer.assert_outbound();
    let mut next = [0; 256];
    assert_eq!(
        transport
            .recv_into_with_timeout(&mut next, BOUND)
            .expect("next receive"),
        ReceiveOutcome::complete(next.len())
    );
    assert_eq!(next, PAYLOAD[1..257]);
    peer.assert_queued(PAYLOAD.len() - first.len() - next.len());
}

#[cfg(any(feature = "runtime-tokio", feature = "runtime-smol"))]
async fn assert_async_reads<S: crate::transport::async_tcp::NetStream>(
    mut transport: crate::transport::async_tcp::Tcp<S>,
    listener: TcpListener,
) {
    use crate::transport::AsyncTransport;

    let mut peer = QueuedPeer::new(&listener);
    let mut first = [0];
    assert_eq!(
        transport
            .recv_into(&mut first)
            .await
            .expect("one-byte receive"),
        ReceiveOutcome::complete(1)
    );
    assert_eq!(first, PAYLOAD[..1]);
    peer.assert_queued(PAYLOAD.len() - first.len());

    let (mut reader, mut writer) = transport.split();
    writer.send(b"outbound").await.expect("split writer");
    peer.assert_outbound();
    let mut next = [0; 256];
    assert_eq!(
        reader.recv_into(&mut next).await.expect("split reader"),
        ReceiveOutcome::complete(next.len())
    );
    assert_eq!(next, PAYLOAD[1..257]);
    peer.assert_queued(PAYLOAD.len() - first.len() - next.len());
}

#[cfg(feature = "runtime-tokio")]
#[tokio::test]
async fn tokio_reads_leave_unrequested_bytes_in_kernel() {
    let listener = listener();
    let transport = crate::transport::tokio::tcp::Tcp::connect_with_config(
        &listener.local_addr().expect("listener address").to_string(),
        TransportConfig::for_tcp(),
    )
    .await
    .expect("connect Tokio transport");
    assert_async_reads(transport, listener).await;
}

#[cfg(feature = "runtime-smol")]
#[test]
fn smol_reads_leave_unrequested_bytes_in_kernel() {
    smol::block_on(async {
        let listener = listener();
        let transport = crate::transport::smol::tcp::Tcp::connect_with_config(
            &listener.local_addr().expect("listener address").to_string(),
            TransportConfig::for_tcp(),
        )
        .await
        .expect("connect smol transport");
        assert_async_reads(transport, listener).await;
    });
}
