//! Runtime-neutral caller-owned session construction.
//!
//! This intentionally runs only when `async` is selected without Tokio or
//! smol. It proves that a consumer-defined [`Executor`] can start and close a
//! public [`Session`] over a caller-owned [`AsyncTransport`].

#![cfg(all(
    feature = "async",
    not(any(feature = "runtime-tokio", feature = "runtime-smol"))
))]

use std::{future::Future, pin::Pin, time::Duration};

use grafton_visca::{
    profiles::PtzOpticsG2,
    transport::{
        AddressingMode, AsyncTransport, HasTransportConfig, SendSemantics, TransportConfig,
    },
    Error, ExecError, Executor, Session, SessionConfig,
};

/// A small caller-owned executor that needs no crate runtime feature.
#[derive(Clone, Copy, Debug)]
struct ThreadExecutor;

impl Executor for ThreadExecutor {
    type Join<T>
        = Pin<Box<dyn Future<Output = std::result::Result<T, ExecError>> + Send + 'static>>
    where
        T: Send + 'static;

    type Detach = ();

    fn spawn_with_detach<F>(&self, future: F) -> (Self::Join<F::Output>, Self::Detach)
    where
        F: Future + Send + 'static,
        F::Output: Send + 'static,
    {
        let (result_tx, result_rx) = flume::bounded(1);
        let spawn_error_tx = result_tx.clone();
        let spawn_result = std::thread::Builder::new()
            .name("grafton-visca-runtime-neutral-test".to_owned())
            .spawn(move || {
                let _ = result_tx.send(Ok(pollster::block_on(future)));
            });
        if let Err(error) = spawn_result {
            let _ = spawn_error_tx.send(Err(ExecError::TaskFailed(error.to_string())));
        }

        let join = async move {
            result_rx
                .recv_async()
                .await
                .map_err(|error| ExecError::JoinFailed(error.to_string()))?
        };
        (Box::pin(join), ())
    }

    fn block_on<F: Future>(&self, future: F) -> F::Output {
        pollster::block_on(future)
    }

    #[allow(clippy::manual_async_fn)]
    fn sleep(&self, duration: Duration) -> impl Future<Output = ()> + Send + '_ {
        async move {
            async_io::Timer::after(duration).await;
        }
    }

    #[allow(clippy::manual_async_fn)]
    fn timeout<'a, F, T>(
        &'a self,
        duration: Duration,
        future: F,
    ) -> impl Future<Output = std::result::Result<T, Error>> + Send + 'a
    where
        F: Future<Output = T> + Send + 'a,
        T: Send + 'a,
    {
        async move {
            futures_lite::future::race(async move { Ok(future.await) }, async move {
                async_io::Timer::after(duration).await;
                Err(Error::Timeout)
            })
            .await
        }
    }
}

#[derive(Debug)]
struct IdleTransport {
    config: TransportConfig,
}

impl IdleTransport {
    fn new() -> Self {
        Self {
            config: TransportConfig::default(),
        }
    }
}

impl HasTransportConfig for IdleTransport {
    fn transport_config(&self) -> &TransportConfig {
        &self.config
    }
}

impl AsyncTransport for IdleTransport {
    async fn send(&mut self, _bytes: &[u8]) -> std::result::Result<(), Error> {
        Ok(())
    }

    async fn recv_into(&mut self, _dst: &mut [u8]) -> std::result::Result<usize, Error> {
        std::future::pending().await
    }

    fn addressing_mode_hint(&self) -> Option<AddressingMode> {
        Some(AddressingMode::Ip)
    }

    fn send_semantics(&self) -> SendSemantics {
        SendSemantics::Datagram
    }
}

#[test]
fn caller_owned_session_opens_and_closes_on_a_custom_executor() {
    let executor = ThreadExecutor;
    executor
        .block_on(async move {
            let session = Session::open(
                IdleTransport::new(),
                SessionConfig::from_compile_time::<PtzOpticsG2>()?,
                executor,
            )
            .await?;
            assert!(session.camera::<PtzOpticsG2>().is_ok());
            session.close().await
        })
        .expect("runtime-neutral caller-owned session closes");
}
