//! Owner-facade coverage for the isolated typed raw request classes.

#![cfg(any(
    feature = "blocking",
    all(
        feature = "async",
        any(feature = "runtime-tokio", feature = "runtime-smol")
    )
))]

#[cfg(feature = "blocking")]
use std::collections::VecDeque;
use std::sync::{Arc, Mutex};

use grafton_visca::{
    raw, AffectedAxes, ControlClass, Error, InquiryRoute, ProfileSpec, ResponseDecoder, RetryClass,
    TimeoutClass,
};

const ACK: &[u8] = &[0x90, 0x41, 0xff];
const COMPLETE: &[u8] = &[0x90, 0x51, 0xff];
const INQUIRY_RESPONSE: &[u8] = &[0x90, 0x50, 0x01, 0x02, 0xff];

fn decode_first(payload: &[u8]) -> grafton_visca::Result<u8> {
    payload
        .first()
        .copied()
        .ok_or_else(|| Error::InvalidRequest("empty raw inquiry payload".into()))
}

fn raw_values() -> (
    raw::Plain,
    raw::Inquiry<u8>,
    raw::Targeted,
    raw::AppliedOnly,
) {
    let plain = raw::Plain::new(
        [0x81, 0x01, 0x01, 0xff],
        TimeoutClass::Quick,
        RetryClass::Never,
        ControlClass::Normal,
    )
    .expect("plain frame");
    let inquiry = raw::Inquiry::new(
        [0x81, 0x09, 0x01, 0xff],
        InquiryRoute::RAW,
        ResponseDecoder::from_fn(decode_first),
        TimeoutClass::Inquiry,
        RetryClass::Inquiry,
        ControlClass::Normal,
    )
    .expect("inquiry frame");
    let targeted = raw::Targeted::new(
        [0x81, 0x01, 0x06, 0xff],
        AffectedAxes::PAN_TILT,
        TimeoutClass::Movement,
        RetryClass::Movement,
        ControlClass::User,
    )
    .expect("targeted frame");
    // A raw applied-only operation classifies itself `User`, the highest lane a
    // raw caller may select; the urgent safety lane is owner-only (#679).
    let applied = raw::AppliedOnly::new(
        [0x81, 0x01, 0x07, 0xff],
        AffectedAxes::ZOOM,
        TimeoutClass::Quick,
        RetryClass::Never,
        ControlClass::User,
    )
    .expect("applied-only frame");
    (plain, inquiry, targeted, applied)
}

fn profile() -> ProfileSpec {
    ProfileSpec::from_compile_time::<grafton_visca::profiles::PtzOpticsG2>().expect("profile")
}

#[cfg(all(
    feature = "async",
    any(feature = "runtime-tokio", feature = "runtime-smol")
))]
fn response_for(bytes: &[u8]) -> &'static [u8] {
    if bytes.get(1) == Some(&0x09) {
        INQUIRY_RESPONSE
    } else {
        ACK
    }
}

#[cfg(feature = "blocking")]
mod blocking_tests {
    use super::*;
    use grafton_visca::{
        blocking::{Session, SessionConfig},
        command::CommandKind,
        transport::{BlockingTransport, HasTransportConfig, SendSemantics, TransportConfig},
    };

    #[derive(Debug, Clone)]
    struct Transport {
        config: TransportConfig,
        responses: Arc<Mutex<VecDeque<Vec<u8>>>>,
        writes: Arc<Mutex<Vec<Vec<u8>>>>,
    }

    impl Transport {
        fn new() -> Self {
            Self {
                config: TransportConfig::default(),
                responses: Arc::new(Mutex::new(VecDeque::new())),
                writes: Arc::new(Mutex::new(Vec::new())),
            }
        }
    }

    impl HasTransportConfig for Transport {
        fn transport_config(&self) -> &TransportConfig {
            &self.config
        }
    }

    impl BlockingTransport for Transport {
        fn send_with_timeout(
            &mut self,
            bytes: &[u8],
            _kind: CommandKind,
            _timeout: std::time::Duration,
        ) -> grafton_visca::Result<()> {
            self.writes
                .lock()
                .expect("writes lock")
                .push(bytes.to_vec());
            let mut responses = self.responses.lock().expect("responses lock");
            if bytes.get(1) == Some(&0x09) {
                responses.push_back(INQUIRY_RESPONSE.to_vec());
            } else {
                responses.push_back(ACK.to_vec());
                responses.push_back(COMPLETE.to_vec());
            }
            Ok(())
        }

        fn recv_into_with_timeout(
            &mut self,
            dst: &mut [u8],
            _timeout: std::time::Duration,
        ) -> grafton_visca::Result<usize> {
            let bytes = self
                .responses
                .lock()
                .expect("responses lock")
                .pop_front()
                .ok_or(Error::Timeout)?;
            dst[..bytes.len()].copy_from_slice(&bytes);
            Ok(bytes.len())
        }

        fn send_semantics(&self) -> SendSemantics {
            SendSemantics::Datagram
        }
    }

    #[test]
    fn blocking_owner_admits_all_typed_raw_classes() {
        let transport = Transport::new();
        let writes = Arc::clone(&transport.writes);
        let session = Session::open(transport, SessionConfig::new(profile())).expect("session");
        let camera = session
            .camera::<grafton_visca::profiles::PtzOpticsG2>()
            .expect("camera");
        let (plain, inquiry, targeted, applied) = raw_values();
        let iris_reset = grafton_visca::request::builtin::IrisReset::new();

        camera.execute(&plain).expect("plain");
        assert_eq!(camera.inquire(&inquiry).expect("inquiry"), 1);
        camera
            .submit::<grafton_visca::completion::Targeted, _>(&targeted)
            .expect("targeted")
            .settled()
            .expect("settled");
        camera
            .submit::<grafton_visca::completion::AppliedOnly, _>(&applied)
            .expect("applied-only")
            .applied()
            .expect("applied");
        camera
            .submit::<grafton_visca::completion::Targeted, _>(&iris_reset)
            .expect("typed iris targeted")
            .settled()
            .expect("typed iris settled");

        assert_eq!(
            *writes.lock().expect("writes lock"),
            vec![
                plain.bytes().to_vec(),
                inquiry.bytes().to_vec(),
                targeted.bytes().to_vec(),
                applied.bytes().to_vec(),
                {
                    let mut bytes = [0_u8; 8];
                    let length = grafton_visca::Request::write_into(
                        &iris_reset,
                        grafton_visca::CameraId::CAMERA_1,
                        &mut bytes,
                    )
                    .expect("typed iris wire");
                    bytes[..length].to_vec()
                },
            ]
        );
        session.shutdown().expect("shutdown");
    }
}

#[cfg(all(
    feature = "async",
    any(feature = "runtime-tokio", feature = "runtime-smol")
))]
mod async_tests {
    use super::*;
    use grafton_visca::{
        transport::{AsyncTransport, HasTransportConfig, SendSemantics, TransportConfig},
        Executor, Session, SessionConfig,
    };

    #[derive(Debug)]
    struct Transport {
        config: TransportConfig,
        responses: flume::Receiver<Vec<u8>>,
        response_tx: flume::Sender<Vec<u8>>,
        writes: Arc<Mutex<Vec<Vec<u8>>>>,
    }

    impl Transport {
        fn new() -> (Self, Arc<Mutex<Vec<Vec<u8>>>>) {
            let (response_tx, responses) = flume::unbounded();
            let writes = Arc::new(Mutex::new(Vec::new()));
            (
                Self {
                    config: TransportConfig::default(),
                    responses,
                    response_tx,
                    writes: Arc::clone(&writes),
                },
                writes,
            )
        }
    }

    impl HasTransportConfig for Transport {
        fn transport_config(&self) -> &TransportConfig {
            &self.config
        }
    }

    impl AsyncTransport for Transport {
        async fn send(&mut self, bytes: &[u8]) -> grafton_visca::Result<()> {
            self.writes
                .lock()
                .expect("writes lock")
                .push(bytes.to_vec());
            self.response_tx
                .send(response_for(bytes).to_vec())
                .map_err(|_| Error::ConnectionClosed { reason: None })?;
            if bytes.get(1) != Some(&0x09) {
                self.response_tx
                    .send(COMPLETE.to_vec())
                    .map_err(|_| Error::ConnectionClosed { reason: None })?;
            }
            Ok(())
        }

        async fn recv_into(&mut self, dst: &mut [u8]) -> grafton_visca::Result<usize> {
            let bytes = self
                .responses
                .recv_async()
                .await
                .map_err(|_| Error::ConnectionClosed { reason: None })?;
            dst[..bytes.len()].copy_from_slice(&bytes);
            Ok(bytes.len())
        }

        fn send_semantics(&self) -> SendSemantics {
            SendSemantics::Datagram
        }
    }

    async fn exercise<E>(executor: E)
    where
        E: Executor + Clone,
    {
        let (transport, writes) = Transport::new();
        let session = Session::open(transport, SessionConfig::new(profile()), executor)
            .await
            .expect("session");
        let camera = session
            .camera::<grafton_visca::profiles::PtzOpticsG2>()
            .expect("camera");
        let (plain, inquiry, targeted, applied) = raw_values();
        let iris_reset = grafton_visca::request::builtin::IrisReset::new();

        camera.execute(&plain).await.expect("plain");
        assert_eq!(camera.inquire(&inquiry).await.expect("inquiry"), 1);
        camera
            .submit::<grafton_visca::completion::Targeted, _>(&targeted)
            .await
            .expect("targeted")
            .settled()
            .await
            .expect("settled");
        camera
            .submit::<grafton_visca::completion::AppliedOnly, _>(&applied)
            .await
            .expect("applied-only")
            .applied()
            .await
            .expect("applied");
        camera
            .submit::<grafton_visca::completion::Targeted, _>(&iris_reset)
            .await
            .expect("typed iris targeted")
            .settled()
            .await
            .expect("typed iris settled");

        assert_eq!(
            *writes.lock().expect("writes lock"),
            vec![
                plain.bytes().to_vec(),
                inquiry.bytes().to_vec(),
                targeted.bytes().to_vec(),
                applied.bytes().to_vec(),
                {
                    let mut bytes = [0_u8; 8];
                    let length = grafton_visca::Request::write_into(
                        &iris_reset,
                        grafton_visca::CameraId::CAMERA_1,
                        &mut bytes,
                    )
                    .expect("typed iris wire");
                    bytes[..length].to_vec()
                },
            ]
        );
        session.shutdown().await.expect("shutdown");
    }

    #[cfg(feature = "runtime-tokio")]
    #[tokio::test]
    async fn tokio_owner_admits_all_typed_raw_classes() {
        exercise(grafton_visca::TokioExecutor::from_current().expect("runtime")).await;
    }

    #[cfg(all(feature = "runtime-smol", not(feature = "runtime-tokio")))]
    #[test]
    fn smol_owner_admits_all_typed_raw_classes() {
        smol::block_on(exercise(grafton_visca::SmolExecutor::new()));
    }
}
