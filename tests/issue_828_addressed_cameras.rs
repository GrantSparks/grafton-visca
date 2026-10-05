//! #828: `Session::open` checks the registered cameras against the cameras a
//! serial bus startup addressed, before any protocol I/O, on every facade and
//! for caller-built transports.

#![cfg(any(feature = "blocking", feature = "runtime-tokio"))]

use std::sync::{
    atomic::{AtomicUsize, Ordering},
    Arc,
};

use grafton_visca::{
    profile::ProfileSpec,
    profiles::GenericVisca,
    transport::{AddressedBus, AddressingMode, HasTransportConfig, TransportConfig},
    CameraId, Error, SessionConfig,
};

/// A caller-built serial transport whose startup addressed `addressed`
/// cameras. It counts writes so a rejected open can be shown to write nothing.
#[derive(Debug)]
struct BusTransport {
    config: TransportConfig,
    addressed: Option<AddressedBus>,
    writes: Arc<AtomicUsize>,
}

const PORT: &str = "/dev/ttyVISCA0";

impl BusTransport {
    fn new(addressed: Option<u8>) -> (Self, Arc<AtomicUsize>) {
        let writes = Arc::new(AtomicUsize::new(0));
        (
            Self {
                config: TransportConfig::for_serial(),
                addressed: addressed.map(|cameras| AddressedBus::new(PORT, cameras)),
                writes: Arc::clone(&writes),
            },
            writes,
        )
    }
}

impl HasTransportConfig for BusTransport {
    fn transport_config(&self) -> &TransportConfig {
        &self.config
    }

    fn addressed_bus(&self) -> Option<&AddressedBus> {
        self.addressed.as_ref()
    }
}

fn targets(ids: &[u8]) -> SessionConfig {
    let profile = ProfileSpec::from_compile_time::<GenericVisca>().expect("profile");
    let mut config =
        SessionConfig::for_target(CameraId::new(ids[0]).expect("camera id"), profile.clone())
            .expect("first target");
    for id in &ids[1..] {
        config = config
            .with_target(CameraId::new(*id).expect("camera id"), profile.clone())
            .expect("further target");
    }
    config
}

/// (addressed cameras, registered ids, opens)
const CASES: [(Option<u8>, &[u8], bool); 5] = [
    (Some(2), &[1, 2], true),
    // Cameras beyond the registered ones are allowed.
    (Some(3), &[1], true),
    // A registered camera the chain did not address fails the open.
    (Some(1), &[1, 2], false),
    (Some(2), &[3], false),
    // No Address Set ran: nothing to check.
    (None, &[1, 2], true),
];

fn assert_outcome<T: std::fmt::Debug>(
    case: (Option<u8>, &[u8], bool),
    result: Result<T, Error>,
    writes: &AtomicUsize,
) -> Option<T> {
    let (addressed, ids, opens) = case;
    match result {
        Ok(session) => {
            assert!(opens, "{addressed:?} / {ids:?} must not open");
            Some(session)
        }
        Err(error) => {
            assert!(!opens, "{addressed:?} / {ids:?} must open: {error:?}");
            let missing = ids
                .iter()
                .find(|id| Some(**id) > addressed)
                .expect("a missing camera");
            assert!(
                matches!(
                    &error,
                    Error::ConnectionFailed { addr, source, .. }
                        if addr == PORT
                            && source.kind() == std::io::ErrorKind::NotFound
                            && source.to_string() == format!(
                                "camera {missing} was not addressed by Address Set (chain reported {})",
                                addressed.expect("Address Set ran")
                            )
                ),
                "unexpected error {error:?}"
            );
            assert_eq!(writes.load(Ordering::SeqCst), 0, "rejected before any I/O");
            None
        }
    }
}

#[cfg(feature = "blocking")]
mod blocking {
    use super::*;
    use grafton_visca::{
        blocking::Session,
        command::CommandKind,
        transport::{BlockingTransport, ReceiveOutcome, SendSemantics},
    };

    impl BlockingTransport for BusTransport {
        fn send_with_timeout(
            &mut self,
            _bytes: &[u8],
            _kind: CommandKind,
            _timeout: std::time::Duration,
        ) -> Result<(), Error> {
            self.writes.fetch_add(1, Ordering::SeqCst);
            Ok(())
        }

        fn recv_into_with_timeout(
            &mut self,
            _dst: &mut [u8],
            timeout: std::time::Duration,
        ) -> Result<ReceiveOutcome, Error> {
            std::thread::sleep(timeout.min(std::time::Duration::from_millis(5)));
            Err(Error::io_timeout())
        }

        fn addressing_mode_hint(&self) -> Option<AddressingMode> {
            Some(AddressingMode::Serial)
        }

        fn send_semantics(&self) -> SendSemantics {
            SendSemantics::Stream
        }
    }

    #[test]
    fn blocking_session_open_checks_addressed_cameras() {
        for case in CASES {
            let (transport, writes) = BusTransport::new(case.0);
            let result = Session::open(transport, targets(case.1));
            if let Some(session) = assert_outcome(case, result, &writes) {
                session.close().expect("clean close");
            }
        }
    }
}

#[cfg(feature = "runtime-tokio")]
mod tokio_facade {
    use super::*;
    use grafton_visca::{
        runtime::TokioRuntime,
        transport::{AsyncTransport, ReceiveOutcome, SendSemantics},
        Session,
    };

    impl AsyncTransport for BusTransport {
        async fn send(&mut self, _bytes: &[u8]) -> Result<(), Error> {
            self.writes.fetch_add(1, Ordering::SeqCst);
            Ok(())
        }

        async fn recv_into(&mut self, _dst: &mut [u8]) -> Result<ReceiveOutcome, Error> {
            std::future::pending().await
        }

        fn addressing_mode_hint(&self) -> Option<AddressingMode> {
            Some(AddressingMode::Serial)
        }

        fn send_semantics(&self) -> SendSemantics {
            SendSemantics::Stream
        }
    }

    #[tokio::test]
    async fn async_session_open_checks_addressed_cameras() {
        for case in CASES {
            let (transport, writes) = BusTransport::new(case.0);
            let runtime = TokioRuntime::from_current().expect("Tokio runtime");
            let result = Session::open(transport, targets(case.1), runtime).await;
            if let Some(session) = assert_outcome(case, result, &writes) {
                session.close().await.expect("clean close");
            }
        }
    }
}
