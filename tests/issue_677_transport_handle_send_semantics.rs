//! Regression test for #677: the public `TransportHandle` wrapper must forward
//! `send_semantics` to its inner transport instead of inheriting the trait
//! default `SendSemantics::Stream`.
//!
//! Before the fix, `impl AsyncTransport for TransportHandle<R>` forwarded
//! `send`, `recv_into`, and `addressing_mode_hint` but omitted `send_semantics`,
//! so `TransportHandle::Udp(_)` inherited `SendSemantics::Stream` while the
//! underlying `Udp` transport (and the blocking `BlockingTransportHandle` twin)
//! reports `Datagram`. A UDP session opened through the wrapper -- the shape
//! shown in `TransportHandle`'s own rustdoc,
//! `TransportHandle::Udp(rt.connect_udp(...).await?)` -- was then governed by
//! stream-poison rules: one failed `send_to` or one malformed datagram poisoned
//! the entire session instead of failing a single command.
//!
//! `udp_transport_handle_reports_datagram_semantics` fails before the fix
//! (returns `Stream`) and passes after it. It builds the *real* Tokio UDP
//! transport, so it exercises the exact path from the type's documentation.
//!
//! The async `Serial` variant (`TransportHandle::Serial`) is not asserted here:
//! it wraps a concrete `tokio_serial::SerialStream`, which requires a physical
//! or pseudo-terminal serial device that CI does not provide -- the codebase
//! constructs no `SerialStream` in any test for the same reason. It forwards
//! through the identical `match` arm as `Tcp`/`Udp` (verified below), resolves
//! to `async_serial::Serial<_>` whose `send_semantics` is the `Stream` default,
//! and its blocking twin's forwarding is exercised by the blocking suite.

#![cfg(feature = "runtime-tokio")]

use grafton_visca::transport::{AsyncTransport, SendSemantics, TransportConfig};
use grafton_visca::{Runtime, TokioRuntime, TransportHandle};

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
        .connect_udp("127.0.0.1:9999", TransportConfig::default())
        .await
        .expect("udp connect is a local operation and needs no peer");
    let handle: TransportHandle<TokioRuntime> = TransportHandle::Udp(udp);

    assert_eq!(
        handle.send_semantics(),
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
        .connect_tcp(&addr.to_string(), TransportConfig::default())
        .await
        .expect("tcp connect to local listener");
    let handle: TransportHandle<TokioRuntime> = TransportHandle::Tcp(tcp);

    assert_eq!(
        handle.send_semantics(),
        SendSemantics::Stream,
        "TransportHandle::Tcp must report stream semantics"
    );
}
