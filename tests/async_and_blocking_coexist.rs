//! Canonical facade topology contracts.
//!
//! `blocking` and `async` are independently selectable. Runtime features
//! imply `async`, while the default feature set supplies `blocking`; therefore
//! one build can expose both owner facades without duplicate root names or
//! runtime parameters leaking into operation handles.

#[cfg(all(
    feature = "blocking",
    any(
        not(feature = "async"),
        feature = "runtime-tokio",
        feature = "runtime-smol"
    )
))]
fn assert_blocking_surface() {
    use grafton_visca::{
        blocking::{Camera, Operation, Session, SessionConfig},
        completion::{AppliedOnly, Targeted},
        profiles::PtzOpticsG2,
        Error, ProfileSpec,
    };

    let _: Option<Camera<'static, PtzOpticsG2>> = None;
    let _: Option<Session> = None;
    let _: Option<SessionConfig> = None;
    let _: Option<Operation<'static, Targeted>> = None;
    let _: Option<Operation<'static, AppliedOnly>> = None;
    let _: fn(Operation<'static, Targeted>) -> Result<(), Error> = Operation::applied;
    let _: fn(Operation<'static, AppliedOnly>) -> Result<(), Error> = Operation::applied;
    let _: fn(ProfileSpec) -> SessionConfig = SessionConfig::new;
}

#[cfg(all(
    feature = "async",
    any(feature = "runtime-tokio", feature = "runtime-smol")
))]
fn assert_async_surface() {
    use std::future::Future;

    use grafton_visca::{
        completion::Targeted, profiles::PtzOpticsG2, Camera, Operation, Session, SessionConfig,
    };

    let _: Option<Camera<PtzOpticsG2>> = None;
    let _: Option<Session> = None;
    let _: Option<SessionConfig> = None;
    let _: Option<Operation<Targeted>> = None;
    fn returns_future<K>(operation: Operation<K>) -> impl Future<Output = grafton_visca::Result<()>>
    where
        K: grafton_visca::completion::Kind,
    {
        operation.applied()
    }
    let _ = returns_future::<Targeted>;
}

#[cfg(all(feature = "blocking", not(feature = "async")))]
#[test]
fn blocking_only_surface_compiles() {
    assert_blocking_surface();
}

#[cfg(all(
    feature = "async",
    feature = "runtime-tokio",
    not(feature = "blocking")
))]
#[test]
fn async_only_tokio_surface_compiles() {
    assert_async_surface();
    let _: fn() -> grafton_visca::Result<grafton_visca::TokioRuntime> =
        || grafton_visca::TokioRuntime::from_current();
}

#[cfg(all(feature = "async", feature = "runtime-smol", not(feature = "blocking")))]
#[test]
fn async_only_smol_surface_compiles() {
    assert_async_surface();
    let _: fn() -> grafton_visca::SmolRuntime = grafton_visca::SmolRuntime::new;
}

#[cfg(all(feature = "blocking", feature = "async", feature = "runtime-tokio"))]
#[test]
fn blocking_and_tokio_surfaces_coexist() {
    assert_blocking_surface();
    assert_async_surface();
}

#[cfg(all(feature = "blocking", feature = "async", feature = "runtime-smol"))]
#[test]
fn blocking_and_smol_surfaces_coexist() {
    assert_blocking_surface();
    assert_async_surface();
}

#[cfg(all(
    feature = "blocking",
    feature = "async",
    feature = "runtime-tokio",
    feature = "runtime-smol"
))]
#[test]
fn blocking_and_all_supported_executors_coexist() {
    assert_blocking_surface();
    assert_async_surface();
    let _: fn() -> grafton_visca::Result<grafton_visca::TokioRuntime> =
        || grafton_visca::TokioRuntime::from_current();
    let _: fn() -> grafton_visca::SmolRuntime = grafton_visca::SmolRuntime::new;
}

// Keep one behavioral coexistence probe in this crate.  The type assertions
// above catch naming/feature regressions, while this probe proves that both
// owners can actually admit, use, and shut down in one process.
#[cfg(all(feature = "blocking", feature = "async", feature = "runtime-tokio"))]
mod runtime_coexistence {
    use std::{collections::VecDeque, future::Future, time::Duration};

    use grafton_visca::{
        blocking::{Session as BlockingSession, SessionConfig as BlockingConfig},
        command::CommandKind,
        profiles::PtzOpticsG2,
        runtime::TokioRuntime,
        transport::{
            AsyncTransport, BlockingTransport, HasTransportConfig, SendSemantics, TransportConfig,
        },
        Error, Session as AsyncSession, SessionConfig as AsyncConfig,
    };

    #[derive(Debug)]
    struct BlockingProbe {
        config: TransportConfig,
        responses: VecDeque<Vec<u8>>,
    }

    impl BlockingProbe {
        fn new() -> Self {
            Self {
                config: TransportConfig::default(),
                responses: VecDeque::new(),
            }
        }

        fn receive(&mut self, dst: &mut [u8]) -> Result<usize, Error> {
            let Some(response) = self.responses.pop_front() else {
                return Err(Error::Timeout);
            };
            dst[..response.len()].copy_from_slice(&response);
            Ok(response.len())
        }
    }

    impl HasTransportConfig for BlockingProbe {
        fn transport_config(&self) -> &TransportConfig {
            &self.config
        }
    }

    impl BlockingTransport for BlockingProbe {
        fn send_with_timeout(
            &mut self,
            _bytes: &[u8],
            _kind: CommandKind,
            _timeout: Duration,
        ) -> Result<(), Error> {
            self.responses.push_back(vec![0x90, 0x41, 0xff]);
            self.responses.push_back(vec![0x90, 0x51, 0xff]);
            Ok(())
        }

        fn recv_into_with_timeout(
            &mut self,
            dst: &mut [u8],
            _timeout: Duration,
        ) -> Result<usize, Error> {
            self.receive(dst)
        }

        fn send_semantics(&self) -> SendSemantics {
            SendSemantics::Datagram
        }
    }

    #[derive(Debug)]
    struct AsyncProbe {
        config: TransportConfig,
        responses: flume::Receiver<Vec<u8>>,
        response_tx: flume::Sender<Vec<u8>>,
    }

    impl AsyncProbe {
        fn new() -> Self {
            let (response_tx, responses) = flume::unbounded();
            Self {
                config: TransportConfig::default(),
                responses,
                response_tx,
            }
        }
    }

    impl HasTransportConfig for AsyncProbe {
        fn transport_config(&self) -> &TransportConfig {
            &self.config
        }
    }

    impl AsyncTransport for AsyncProbe {
        fn send(&mut self, _bytes: &[u8]) -> impl Future<Output = Result<(), Error>> + Send {
            let response_tx = self.response_tx.clone();
            async move {
                response_tx
                    .send_async(vec![0x90, 0x41, 0xff])
                    .await
                    .map_err(|_| Error::ConnectionClosed { reason: None })?;
                response_tx
                    .send_async(vec![0x90, 0x51, 0xff])
                    .await
                    .map_err(|_| Error::ConnectionClosed { reason: None })?;
                Ok(())
            }
        }

        #[allow(clippy::manual_async_fn)]
        fn recv_into<'a>(
            &'a mut self,
            dst: &'a mut [u8],
        ) -> impl Future<Output = Result<usize, Error>> + Send {
            async move {
                let response = self
                    .responses
                    .recv_async()
                    .await
                    .map_err(|_| Error::ConnectionClosed { reason: None })?;
                dst[..response.len()].copy_from_slice(&response);
                Ok(response.len())
            }
        }

        fn send_semantics(&self) -> SendSemantics {
            SendSemantics::Datagram
        }
    }

    #[tokio::test]
    async fn tokio_blocking_and_async_owners_open_use_and_close() {
        let blocking = BlockingSession::open(
            BlockingProbe::new(),
            BlockingConfig::from_compile_time::<PtzOpticsG2>().unwrap(),
        )
        .unwrap();
        blocking
            .camera::<PtzOpticsG2>()
            .unwrap()
            .zoom()
            .stop()
            .unwrap()
            .applied()
            .unwrap();

        let async_session = AsyncSession::open(
            AsyncProbe::new(),
            AsyncConfig::from_compile_time::<PtzOpticsG2>().unwrap(),
            TokioRuntime::from_current().unwrap(),
        )
        .await
        .unwrap();
        async_session
            .camera::<PtzOpticsG2>()
            .unwrap()
            .zoom()
            .stop()
            .await
            .unwrap()
            .applied()
            .await
            .unwrap();

        blocking.close().unwrap();
        async_session.close().await.unwrap();
    }
}

#[cfg(all(feature = "blocking", feature = "async", feature = "runtime-smol"))]
use std::{collections::VecDeque, future::Future, time::Duration};

#[cfg(all(feature = "blocking", feature = "async", feature = "runtime-smol"))]
#[test]
fn smol_blocking_and_async_owners_open_use_and_close() {
    smol::block_on(async {
        // Reuse the same behavioral probe with smol's runtime adapter.  Keep
        // the Tokio module separate so each test compiles only its driver.
        use grafton_visca::{
            blocking::{Session as BlockingSession, SessionConfig as BlockingConfig},
            profiles::PtzOpticsG2,
            runtime::SmolRuntime,
            transport::{
                AsyncTransport, BlockingTransport, HasTransportConfig, SendSemantics,
                TransportConfig,
            },
            Error, Session as AsyncSession, SessionConfig as AsyncConfig,
        };

        #[derive(Debug)]
        struct BlockingProbe {
            config: TransportConfig,
            responses: VecDeque<Vec<u8>>,
        }

        impl BlockingProbe {
            fn new() -> Self {
                Self {
                    config: TransportConfig::default(),
                    responses: VecDeque::new(),
                }
            }
        }

        impl HasTransportConfig for BlockingProbe {
            fn transport_config(&self) -> &TransportConfig {
                &self.config
            }
        }

        impl BlockingTransport for BlockingProbe {
            fn send_with_timeout(
                &mut self,
                _bytes: &[u8],
                _kind: grafton_visca::command::CommandKind,
                _timeout: Duration,
            ) -> Result<(), Error> {
                self.responses.push_back(vec![0x90, 0x41, 0xff]);
                self.responses.push_back(vec![0x90, 0x51, 0xff]);
                Ok(())
            }

            fn recv_into_with_timeout(
                &mut self,
                dst: &mut [u8],
                _timeout: Duration,
            ) -> Result<usize, Error> {
                let response = self.responses.pop_front().ok_or(Error::Timeout)?;
                dst[..response.len()].copy_from_slice(&response);
                Ok(response.len())
            }

            fn send_semantics(&self) -> SendSemantics {
                SendSemantics::Datagram
            }
        }

        #[derive(Debug)]
        struct AsyncProbe {
            config: TransportConfig,
            responses: flume::Receiver<Vec<u8>>,
            response_tx: flume::Sender<Vec<u8>>,
        }

        impl AsyncProbe {
            fn new() -> Self {
                let (response_tx, responses) = flume::unbounded();
                Self {
                    config: TransportConfig::default(),
                    responses,
                    response_tx,
                }
            }
        }

        impl HasTransportConfig for AsyncProbe {
            fn transport_config(&self) -> &TransportConfig {
                &self.config
            }
        }

        impl AsyncTransport for AsyncProbe {
            fn send(&mut self, _bytes: &[u8]) -> impl Future<Output = Result<(), Error>> + Send {
                let response_tx = self.response_tx.clone();
                async move {
                    response_tx
                        .send_async(vec![0x90, 0x41, 0xff])
                        .await
                        .map_err(|_| Error::ConnectionClosed { reason: None })?;
                    response_tx
                        .send_async(vec![0x90, 0x51, 0xff])
                        .await
                        .map_err(|_| Error::ConnectionClosed { reason: None })?;
                    Ok(())
                }
            }

            #[allow(clippy::manual_async_fn)]
            fn recv_into<'a>(
                &'a mut self,
                dst: &'a mut [u8],
            ) -> impl Future<Output = Result<usize, Error>> + Send {
                async move {
                    let response = self
                        .responses
                        .recv_async()
                        .await
                        .map_err(|_| Error::ConnectionClosed { reason: None })?;
                    dst[..response.len()].copy_from_slice(&response);
                    Ok(response.len())
                }
            }

            fn send_semantics(&self) -> SendSemantics {
                SendSemantics::Datagram
            }
        }

        let blocking = BlockingSession::open(
            BlockingProbe::new(),
            BlockingConfig::from_compile_time::<PtzOpticsG2>().unwrap(),
        )
        .unwrap();
        blocking
            .camera::<PtzOpticsG2>()
            .unwrap()
            .zoom()
            .stop()
            .unwrap()
            .applied()
            .unwrap();

        let async_session = AsyncSession::open(
            AsyncProbe::new(),
            AsyncConfig::from_compile_time::<PtzOpticsG2>().unwrap(),
            SmolRuntime::new(),
        )
        .await
        .unwrap();
        async_session
            .camera::<PtzOpticsG2>()
            .unwrap()
            .zoom()
            .stop()
            .await
            .unwrap()
            .applied()
            .await
            .unwrap();

        blocking.close().unwrap();
        async_session.close().await.unwrap();
    });
}
