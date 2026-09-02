//! Issue #721: a raw camera's named socket reuse supersedes stale local ownership.

#![allow(clippy::expect_used)]

#[cfg(any(
    feature = "blocking",
    all(feature = "async", feature = "runtime-tokio")
))]
#[path = "common/profile_fixtures.rs"]
mod profile_fixtures;

#[cfg(any(
    feature = "blocking",
    all(feature = "async", feature = "runtime-tokio")
))]
const ACK_SOCKET_ONE: &[u8] = &[0x90, 0x41, 0xff];
#[cfg(any(
    feature = "blocking",
    all(feature = "async", feature = "runtime-tokio")
))]
const COMPLETE_SOCKET_ONE: &[u8] = &[0x90, 0x51, 0xff];
#[cfg(any(
    feature = "blocking",
    all(feature = "async", feature = "runtime-tokio")
))]
const CANCEL_SOCKET_ONE: &[u8] = &[0x81, 0x21, 0xff];

#[cfg(feature = "blocking")]
mod blocking {
    use std::{
        collections::VecDeque,
        sync::{Arc, Mutex},
        time::Duration,
    };

    use grafton_visca::{
        blocking::{Session, SessionConfig},
        command::CommandKind,
        completion::AppliedOnly,
        profile::ProfileSpec,
        request::builtin::ZoomDrive,
        transport::{BlockingTransport, HasTransportConfig, SendSemantics, TransportConfig},
        CancellationOutcome, Error,
    };

    use super::{
        profile_fixtures::NonDefaultCompileTimeProfile, ACK_SOCKET_ONE, CANCEL_SOCKET_ONE,
        COMPLETE_SOCKET_ONE,
    };

    #[derive(Debug)]
    struct ReusedSocketTransport {
        config: TransportConfig,
        replies: VecDeque<Vec<u8>>,
        writes: Arc<Mutex<Vec<Vec<u8>>>>,
    }

    impl ReusedSocketTransport {
        fn new() -> (Self, Arc<Mutex<Vec<Vec<u8>>>>) {
            let writes = Arc::new(Mutex::new(Vec::new()));
            (
                Self {
                    config: TransportConfig::default(),
                    replies: VecDeque::new(),
                    writes: Arc::clone(&writes),
                },
                writes,
            )
        }
    }

    impl HasTransportConfig for ReusedSocketTransport {
        fn transport_config(&self) -> &TransportConfig {
            &self.config
        }
    }

    impl BlockingTransport for ReusedSocketTransport {
        fn send_with_kind(&mut self, bytes: &[u8], _kind: CommandKind) -> Result<(), Error> {
            let write_number = {
                let mut writes = self.writes.lock().expect("writes lock");
                writes.push(bytes.to_vec());
                writes.len()
            };
            match write_number {
                // A earns S1, but its completion is lost. The camera then
                // releases and reuses S1 for B.
                1 | 2 => self.replies.push_back(ACK_SOCKET_ONE.to_vec()),
                // The cancellation wire itself is asserted below. A normal B
                // completion then proves S1 resolves to B, not displaced A.
                _ if bytes == CANCEL_SOCKET_ONE => {
                    self.replies.push_back(COMPLETE_SOCKET_ONE.to_vec());
                }
                _ => {}
            }
            Ok(())
        }

        fn recv_into(&mut self, destination: &mut [u8]) -> Result<usize, Error> {
            self.recv_into_with_timeout(destination, Duration::from_millis(1))
        }

        fn recv_into_with_timeout(
            &mut self,
            destination: &mut [u8],
            _timeout: Duration,
        ) -> Result<usize, Error> {
            let reply = self.replies.pop_front().ok_or(Error::Timeout)?;
            destination[..reply.len()].copy_from_slice(&reply);
            Ok(reply.len())
        }

        fn send_semantics(&self) -> SendSemantics {
            SendSemantics::Datagram
        }
    }

    #[test]
    fn blocking_raw_socket_reuse_drives_cancel_and_completion_on_the_named_socket() {
        let (transport, writes) = ReusedSocketTransport::new();
        let session = Session::open(
            transport,
            SessionConfig::new(
                ProfileSpec::from_compile_time::<NonDefaultCompileTimeProfile>()
                    .expect("two-socket raw profile"),
            ),
        )
        .expect("session");
        let camera = session
            .camera::<NonDefaultCompileTimeProfile>()
            .expect("camera");

        let displaced = camera
            .submit::<AppliedOnly, _>(&ZoomDrive::Tele)
            .expect("first command writes");
        let successor = camera
            .submit::<AppliedOnly, _>(&ZoomDrive::Wide)
            .expect("second command writes after A's ACK");

        assert!(matches!(
            displaced.applied(),
            Err(Error::UnsequencedCommandUnconfirmed)
        ));
        assert_eq!(
            writes.lock().expect("writes lock").len(),
            2,
            "the displaced wait must not emit another command"
        );
        let cancellation = successor.cancel().expect("successor cancellation recorded");
        {
            let writes = writes.lock().expect("writes lock");
            assert_eq!(writes.len(), 3, "cancellation must reach the wire");
            assert_eq!(writes[2], CANCEL_SOCKET_ONE);
        }
        assert_eq!(
            cancellation
                .outcome(Duration::from_secs(1))
                .expect("S1 completion settles the successor"),
            CancellationOutcome::Completed
        );

        let writes = writes.lock().expect("writes lock");
        assert_eq!(writes.len(), 3, "two commands and one socket cancellation");
        assert_eq!(writes[2], CANCEL_SOCKET_ONE);
        drop(writes);
        session.shutdown().expect("shutdown");
    }
}

#[cfg(all(feature = "async", feature = "runtime-tokio"))]
mod asynchronous {
    use std::{
        future::Future,
        sync::{Arc, Mutex},
        time::Duration,
    };

    use grafton_visca::{
        completion::AppliedOnly,
        profile::ProfileSpec,
        request::builtin::ZoomDrive,
        transport::{
            AddressingMode, AsyncTransport, HasTransportConfig, SendSemantics, TransportConfig,
        },
        CancellationOutcome, Error, Session, SessionConfig, TokioRuntime,
    };

    use super::{
        profile_fixtures::NonDefaultCompileTimeProfile, ACK_SOCKET_ONE, CANCEL_SOCKET_ONE,
        COMPLETE_SOCKET_ONE,
    };

    #[derive(Debug)]
    struct ReusedSocketTransport {
        config: TransportConfig,
        replies: flume::Receiver<Vec<u8>>,
        reply_tx: flume::Sender<Vec<u8>>,
        writes: Arc<Mutex<Vec<Vec<u8>>>>,
    }

    impl ReusedSocketTransport {
        fn new() -> (Self, Arc<Mutex<Vec<Vec<u8>>>>) {
            let (reply_tx, replies) = flume::unbounded();
            let writes = Arc::new(Mutex::new(Vec::new()));
            (
                Self {
                    config: TransportConfig::default(),
                    replies,
                    reply_tx,
                    writes: Arc::clone(&writes),
                },
                writes,
            )
        }
    }

    impl HasTransportConfig for ReusedSocketTransport {
        fn transport_config(&self) -> &TransportConfig {
            &self.config
        }
    }

    impl AsyncTransport for ReusedSocketTransport {
        fn send(&mut self, bytes: &[u8]) -> impl Future<Output = Result<(), Error>> + Send {
            let write_number = {
                let mut writes = self.writes.lock().expect("writes lock");
                writes.push(bytes.to_vec());
                writes.len()
            };
            let cancel_socket_one = bytes == CANCEL_SOCKET_ONE;
            let reply_tx = self.reply_tx.clone();
            async move {
                let reply = match (write_number, cancel_socket_one) {
                    (1 | 2, _) => Some(ACK_SOCKET_ONE),
                    (_, true) => Some(COMPLETE_SOCKET_ONE),
                    _ => None,
                };
                if let Some(reply) = reply {
                    reply_tx
                        .send_async(reply.to_vec())
                        .await
                        .map_err(|_| Error::ConnectionClosed { reason: None })?;
                }
                Ok(())
            }
        }

        async fn recv_into(&mut self, destination: &mut [u8]) -> Result<usize, Error> {
            let reply = self
                .replies
                .recv_async()
                .await
                .map_err(|_| Error::ConnectionClosed { reason: None })?;
            destination[..reply.len()].copy_from_slice(&reply);
            Ok(reply.len())
        }

        fn addressing_mode_hint(&self) -> Option<AddressingMode> {
            Some(AddressingMode::Ip)
        }

        fn send_semantics(&self) -> SendSemantics {
            SendSemantics::Datagram
        }
    }

    #[tokio::test]
    async fn async_raw_socket_reuse_drives_cancel_and_completion_on_the_named_socket() {
        let (transport, writes) = ReusedSocketTransport::new();
        let session = Session::open(
            transport,
            SessionConfig::new(
                ProfileSpec::from_compile_time::<NonDefaultCompileTimeProfile>()
                    .expect("two-socket raw profile"),
            ),
            TokioRuntime::from_current().expect("Tokio runtime"),
        )
        .await
        .expect("session");
        let camera = session
            .camera::<NonDefaultCompileTimeProfile>()
            .expect("camera");

        let displaced = camera
            .submit::<AppliedOnly, _>(&ZoomDrive::Tele)
            .await
            .expect("first command writes");
        let successor = camera
            .submit::<AppliedOnly, _>(&ZoomDrive::Wide)
            .await
            .expect("second command writes");

        assert!(matches!(
            displaced.applied().await,
            Err(Error::UnsequencedCommandUnconfirmed)
        ));
        let cancellation = successor
            .cancel()
            .await
            .expect("successor cancellation recorded");
        assert_eq!(
            cancellation
                .outcome(Duration::from_secs(1))
                .await
                .expect("S1 completion settles the successor"),
            CancellationOutcome::Completed
        );

        {
            let writes = writes.lock().expect("writes lock");
            assert_eq!(writes.len(), 3, "two commands and one socket cancellation");
            assert_eq!(writes[2], CANCEL_SOCKET_ONE);
        }
        session.shutdown().await.expect("shutdown");
    }
}
