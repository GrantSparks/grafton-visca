//! Regression coverage for #677 and #899: handle enums forward the complete
//! identity of their inner transport, preserving stream/datagram failure policy.

#![cfg(feature = "runtime-tokio")]

use grafton_visca::runtime::{Runtime, TokioRuntime, TransportHandle};
use grafton_visca::transport::{
    HasTransportConfig, IoBounds, NetworkEndpoint, SendSemantics, TcpConnect, TransportIdentity,
    UdpConnect,
};

/// A UDP session built through the wrapper must be governed by datagram rules.
/// This is the defect in #677: the omitted `send_semantics` forward demoted
/// every `TransportHandle::Udp` session to the stream-poison policy.
#[tokio::test]
async fn udp_transport_handle_reports_datagram_semantics() {
    let rt = TokioRuntime::from_current().expect("current tokio runtime");

    // UDP `connect` only sets the socket's default peer; it is a local
    // operation that needs no listener. This is the exact shape from
    // `TransportHandle`'s rustdoc.
    let udp = rt
        .connect_udp(
            NetworkEndpoint::parse("127.0.0.1:9999", None).expect("endpoint"),
            UdpConnect::default(),
            IoBounds::default(),
        )
        .await
        .expect("udp connect is a local operation and needs no peer");
    let handle: TransportHandle<TokioRuntime> = TransportHandle::Udp(udp);

    assert_eq!(handle.identity(), TransportIdentity::Udp);

    assert_eq!(
        handle.identity().send_semantics(),
        SendSemantics::Datagram,
        "TransportHandle::Udp must forward datagram semantics; the inherited \
         Stream default poisons the whole session on one bad datagram (#677)"
    );
}

/// The `Tcp` variant must keep reporting stream semantics. This value happened
/// to be correct before the fix (the default was also `Stream`), so this guards
/// against a future forward that hard-codes the wrong value per variant.
#[tokio::test]
async fn tcp_transport_handle_reports_stream_semantics() {
    let rt = TokioRuntime::from_current().expect("current tokio runtime");

    // TCP connect performs a handshake, so it needs a listener. The kernel
    // completes the handshake into the accept backlog; we never call `accept`.
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("bind loopback listener");
    let addr = listener.local_addr().expect("listener local addr");

    let tcp = rt
        .connect_tcp(
            NetworkEndpoint::parse(&addr.to_string(), None).expect("endpoint"),
            TcpConnect::default(),
            IoBounds::default(),
        )
        .await
        .expect("tcp connect to local listener");
    let handle: TransportHandle<TokioRuntime> = TransportHandle::Tcp(tcp);

    assert_eq!(handle.identity(), TransportIdentity::Tcp);

    assert_eq!(
        handle.identity().send_semantics(),
        SendSemantics::Stream,
        "TransportHandle::Tcp must report stream semantics"
    );
}
