//! Behavioral coverage for the canonical async owner facade on smol.

#![cfg(all(feature = "async", feature = "runtime-smol"))]

use std::future::Future;

use grafton_visca::{
    profiles::PtzOpticsG2,
    runtime::SmolRuntime,
    transport::{AsyncTransport, HasTransportConfig, SendSemantics, TransportConfig},
    Error, Session, SessionConfig,
};

#[derive(Debug)]
struct ProbeTransport {
    config: TransportConfig,
    responses: flume::Receiver<Vec<u8>>,
    response_tx: flume::Sender<Vec<u8>>,
}

impl ProbeTransport {
    fn new() -> Self {
        let (response_tx, responses) = flume::unbounded();
        Self {
            config: TransportConfig::default(),
            responses,
            response_tx,
        }
    }
}

impl HasTransportConfig for ProbeTransport {
    fn transport_config(&self) -> &TransportConfig {
        &self.config
    }
}

impl AsyncTransport for ProbeTransport {
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

#[test]
fn smol_async_owner_admits_uses_and_closes() {
    smol::block_on(async {
        let session = Session::open(
            ProbeTransport::new(),
            SessionConfig::from_compile_time::<PtzOpticsG2>().unwrap(),
            SmolRuntime::new(),
        )
        .await
        .unwrap();

        session
            .camera::<PtzOpticsG2>()
            .unwrap()
            .zoom()
            .stop()
            .await
            .unwrap()
            .applied()
            .await
            .unwrap();

        // Exercise the explicit close path as well as the operation path.
        session.close().await.unwrap();
    });
}
