//! Regression tests for issue #523: typed runtime boundary and shutdown semantics.

#![cfg(all(
    feature = "mode-async",
    feature = "runtime-tokio",
    feature = "test-utils"
))]

use std::time::Duration;
use std::{
    future::{pending, Future},
    sync::{
        atomic::{AtomicUsize, Ordering},
        Arc, Mutex,
    },
};

use grafton_visca::{
    camera::profiles::{GenericVisca, PtzOpticsG2},
    command::{
        CommandBehavior, InquiryKind, InquiryResponseSpec, ViscaCommand, Zoom, VISCA_TERMINATOR,
    },
    runtime::testing::RuntimeHandle,
    testing::testkit::{helpers, ScriptedTransport, Step},
    timeout::CommandCategory,
    transport::{AsyncTransport, HasTransportConfig, SendSemantics, TransportConfig},
    CameraId, Error, TokioExecutor, ViscaSocket,
};

struct HangingSendTransport {
    sent: Arc<Mutex<Vec<Vec<u8>>>>,
    send_count: Arc<AtomicUsize>,
    response_tx: flume::Sender<Result<Vec<u8>, Error>>,
    response_rx: flume::Receiver<Result<Vec<u8>, Error>>,
    config: TransportConfig,
    semantics: SendSemantics,
    hang_from_send: usize,
    first_send_responses: Vec<Vec<u8>>,
}

impl HangingSendTransport {
    fn new(hang_from_send: usize, write_timeout: Duration, semantics: SendSemantics) -> Self {
        let (response_tx, response_rx) = flume::unbounded();
        Self {
            sent: Arc::new(Mutex::new(Vec::new())),
            send_count: Arc::new(AtomicUsize::new(0)),
            response_tx,
            response_rx,
            config: TransportConfig {
                write_timeout,
                ..TransportConfig::default()
            },
            semantics,
            hang_from_send,
            first_send_responses: Vec::new(),
        }
    }

    fn with_first_send_responses(mut self, responses: Vec<Vec<u8>>) -> Self {
        self.first_send_responses = responses;
        self
    }

    fn sent_handle(&self) -> Arc<Mutex<Vec<Vec<u8>>>> {
        Arc::clone(&self.sent)
    }

    fn response_sender(&self) -> flume::Sender<Result<Vec<u8>, Error>> {
        self.response_tx.clone()
    }
}

impl AsyncTransport for HangingSendTransport {
    fn send(&mut self, bytes: &[u8]) -> impl Future<Output = Result<(), Error>> + Send {
        let bytes = bytes.to_vec();
        self.sent
            .lock()
            .expect("sent transport log mutex poisoned")
            .push(bytes);

        let send_index = self.send_count.fetch_add(1, Ordering::SeqCst);
        let should_hang = send_index >= self.hang_from_send;
        let responses = if send_index == 0 {
            self.first_send_responses.clone()
        } else {
            Vec::new()
        };
        let response_tx = self.response_tx.clone();

        async move {
            if should_hang {
                pending::<Result<(), Error>>().await
            } else {
                for response in responses {
                    let _ = response_tx.send(Ok(response));
                }
                Ok(())
            }
        }
    }

    fn recv_into<'a>(
        &'a mut self,
        dst: &'a mut [u8],
    ) -> impl Future<Output = Result<usize, Error>> + Send {
        let response_rx = self.response_rx.clone();
        async move {
            match response_rx.recv_async().await {
                Ok(Ok(bytes)) => {
                    let len = bytes.len().min(dst.len());
                    dst[..len].copy_from_slice(&bytes[..len]);
                    Ok(len)
                }
                Ok(Err(error)) => Err(error),
                Err(_) => Err(Error::Timeout),
            }
        }
    }

    fn send_semantics(&self) -> SendSemantics {
        self.semantics
    }
}

impl HasTransportConfig for HangingSendTransport {
    fn transport_config(&self) -> &TransportConfig {
        &self.config
    }
}

struct PowerInquiry;

impl ViscaCommand for PowerInquiry {
    const MAX_SIZE: usize = 5;
    const TIMEOUT_CATEGORY: CommandCategory = CommandCategory::Quick;

    fn write_into(&self, camera_id: CameraId, buffer: &mut [u8]) -> Result<usize, Error> {
        if buffer.len() < Self::MAX_SIZE {
            return Err(Error::BufferTooSmall {
                required: Self::MAX_SIZE,
                actual: buffer.len(),
            });
        }

        buffer[..Self::MAX_SIZE].copy_from_slice(&[
            camera_id.to_address_byte(),
            0x09,
            0x04,
            0x00,
            VISCA_TERMINATOR,
        ]);
        Ok(Self::MAX_SIZE)
    }

    fn behavior(&self) -> CommandBehavior {
        CommandBehavior::Inquiry(InquiryResponseSpec::Builtin(InquiryKind::Power))
    }
}

fn assert_runtime_shutdown<T: std::fmt::Debug>(result: Result<T, Error>, context: &str) {
    assert!(
        matches!(result, Err(Error::RuntimeShutdown)),
        "{context} should fail with RuntimeShutdown, got {result:?}"
    );
}

fn assert_peer_closed<T: std::fmt::Debug>(result: Result<T, Error>, context: &str) {
    assert!(
        matches!(
            &result,
            Err(Error::ConnectionClosed {
                reason: Some(reason)
            }) if reason.as_ref() == "peer closed"
        ),
        "{context} should preserve the peer-closed cause, got {result:?}"
    );
}

#[tokio::test]
async fn explicit_shutdown_fails_pending_and_later_requests() {
    let executor = Arc::new(TokioExecutor::from_current().unwrap());
    let transport: ScriptedTransport<TokioExecutor> =
        ScriptedTransport::new(Vec::<Step>::new()).with_executor(executor.clone());

    let runtime: RuntimeHandle<PtzOpticsG2, TokioExecutor> =
        RuntimeHandle::new(transport, executor)
            .await
            .expect("runtime should start");

    let (cmd_id, response) = runtime
        .send_command_with_id(&Zoom::TeleStd, CameraId::CAMERA_1, None)
        .await
        .expect("command should be admitted");

    runtime.shutdown().await.expect("shutdown should complete");

    let response_result = tokio::time::timeout(Duration::from_secs(1), response)
        .await
        .expect("response future should not hang after shutdown");
    assert_runtime_shutdown(response_result, "pending command response");

    assert_runtime_shutdown(
        runtime
            .send_command(&Zoom::WideStd, CameraId::CAMERA_1, None)
            .await,
        "post-shutdown command",
    );
    assert_runtime_shutdown(
        runtime
            .send_inquiry(&PowerInquiry, CameraId::CAMERA_1)
            .await,
        "post-shutdown inquiry",
    );
    assert_runtime_shutdown(
        runtime.cancel(CameraId::CAMERA_1, cmd_id).await,
        "post-shutdown cancel",
    );
    assert_runtime_shutdown(
        runtime
            .cancel_socket(CameraId::CAMERA_1, ViscaSocket::S1)
            .await,
        "post-shutdown socket cancel",
    );
    assert_runtime_shutdown(runtime.metrics().await, "post-shutdown metrics");
    assert_runtime_shutdown(
        runtime.subscribe_completions().await,
        "post-shutdown completion subscription",
    );
}

#[tokio::test]
async fn queued_command_cancel_removes_without_sending_cancel_frame() {
    let executor = Arc::new(TokioExecutor::from_current().unwrap());

    let transport: ScriptedTransport<TokioExecutor> = ScriptedTransport::new(vec![
        Step::OnSend {
            matches: None,
            responses: vec![helpers::ack(1)],
        },
        Step::OnSend {
            matches: None,
            responses: vec![helpers::ack(2)],
        },
    ])
    .with_executor(executor.clone());
    let sent_transport = transport.clone();

    let runtime: RuntimeHandle<PtzOpticsG2, TokioExecutor> =
        RuntimeHandle::new(transport, executor.clone())
            .await
            .expect("runtime should start");

    let (_first_id, _first_response) = runtime
        .send_command_with_id(&Zoom::TeleStd, CameraId::CAMERA_1, None)
        .await
        .expect("first command should be admitted");
    let (_second_id, _second_response) = runtime
        .send_command_with_id(&Zoom::WideStd, CameraId::CAMERA_1, None)
        .await
        .expect("second command should be admitted");

    tokio::time::sleep(Duration::from_millis(100)).await;

    let (queued_id, queued_response) = runtime
        .send_command_with_id(&Zoom::Stop, CameraId::CAMERA_1, None)
        .await
        .expect("third command should be admitted but remain queued");

    runtime
        .cancel(CameraId::CAMERA_1, queued_id)
        .await
        .expect("queued cancel should be processed");

    let queued_result = tokio::time::timeout(Duration::from_secs(1), queued_response)
        .await
        .expect("queued command response should not hang");
    assert!(
        matches!(queued_result, Err(Error::CommandCanceled)),
        "queued command should be canceled, got {queued_result:?}"
    );

    let sent = sent_transport.sent();
    assert_eq!(
        sent.len(),
        2,
        "only the two socket-occupying commands should have reached the transport: {sent:?}"
    );
    assert!(
        sent.iter()
            .all(|frame| !frame.starts_with(&[0x81, 0x21]) && !frame.starts_with(&[0x81, 0x22])),
        "queued cancellation must not send a VISCA cancel frame: {sent:?}"
    );

    runtime.shutdown().await.expect("runtime shutdown");
}

#[tokio::test]
async fn cancel_socket_send_uses_configured_write_timeout() {
    let executor = Arc::new(TokioExecutor::from_current().unwrap());
    let transport = HangingSendTransport::new(0, Duration::from_millis(50), SendSemantics::Stream);

    let runtime: RuntimeHandle<GenericVisca, TokioExecutor> =
        RuntimeHandle::new(transport, executor)
            .await
            .expect("runtime should start");

    let result = tokio::time::timeout(
        Duration::from_secs(1),
        runtime.cancel_socket(CameraId::CAMERA_1, ViscaSocket::S1),
    )
    .await
    .expect("cancel_socket should be bounded by write_timeout");

    assert!(
        matches!(result, Err(Error::Timeout)),
        "cancel send timeout should be reported to the caller, got {result:?}"
    );
}

#[tokio::test]
async fn cancel_on_ack_send_uses_configured_write_timeout() {
    let executor = Arc::new(TokioExecutor::from_current().unwrap());
    let transport = HangingSendTransport::new(1, Duration::from_millis(50), SendSemantics::Stream)
        .with_first_send_responses(Vec::new());
    let response_tx = transport.response_sender();
    let sent = transport.sent_handle();

    let runtime: RuntimeHandle<GenericVisca, TokioExecutor> =
        RuntimeHandle::new(transport, executor)
            .await
            .expect("runtime should start");

    let (cmd_id, response) = runtime
        .send_command_with_id(&Zoom::TeleStd, CameraId::CAMERA_1, None)
        .await
        .expect("command should be admitted");

    runtime
        .cancel(CameraId::CAMERA_1, cmd_id)
        .await
        .expect("awaiting-ACK cancel should be marked");
    response_tx
        .send(Ok(helpers::ack(1)))
        .expect("ACK should be injectable");

    let response_result = tokio::time::timeout(Duration::from_secs(1), response)
        .await
        .expect("cancel-on-ACK write timeout should release the response future");
    assert!(
        matches!(response_result, Err(Error::StreamPoisoned { .. })),
        "stream cancel-on-ACK timeout should poison pending work, got {response_result:?}"
    );

    let sent = sent
        .lock()
        .expect("sent transport log mutex poisoned")
        .clone();
    assert!(
        sent.iter().any(|frame| frame.starts_with(&[0x81, 0x21])),
        "cancel-on-ACK should attempt the VISCA cancel frame: {sent:?}"
    );
}

#[tokio::test]
async fn concurrent_shutdown_waits_for_cleanup_acknowledgement() {
    let executor = Arc::new(TokioExecutor::from_current().unwrap());
    let transport = HangingSendTransport::new(0, Duration::from_millis(300), SendSemantics::Stream);

    let runtime: RuntimeHandle<PtzOpticsG2, TokioExecutor> =
        RuntimeHandle::new(transport, executor)
            .await
            .expect("runtime should start");

    let (_cmd_id, _response) = runtime
        .send_command_with_id(&Zoom::TeleStd, CameraId::CAMERA_1, None)
        .await
        .expect("command should be admitted before the stalled send");

    let first_runtime = runtime.clone();
    let mut first_shutdown = tokio::spawn(async move { first_runtime.shutdown().await });
    tokio::time::sleep(Duration::from_millis(20)).await;

    let second_runtime = runtime.clone();
    let mut second_shutdown = tokio::spawn(async move { second_runtime.shutdown().await });

    assert!(
        tokio::time::timeout(Duration::from_millis(50), &mut second_shutdown)
            .await
            .is_err(),
        "a concurrent shutdown caller must not return before runtime cleanup is acknowledged"
    );

    let first_result = tokio::time::timeout(Duration::from_secs(2), &mut first_shutdown)
        .await
        .expect("first shutdown should complete after stalled send timeout")
        .expect("first shutdown task should not panic");
    let second_result = tokio::time::timeout(Duration::from_secs(2), &mut second_shutdown)
        .await
        .expect("second shutdown should complete with the shared result")
        .expect("second shutdown task should not panic");

    assert!(
        matches!(first_result, Err(Error::StreamPoisoned { .. })),
        "first shutdown should preserve the stalled stream failure, got {first_result:?}"
    );
    assert!(
        matches!(second_result, Err(Error::StreamPoisoned { .. })),
        "second shutdown should receive the same cleanup result, got {second_result:?}"
    );
}

#[tokio::test]
async fn shutdown_is_not_blocked_by_metrics_flood() {
    let executor = Arc::new(TokioExecutor::from_current().unwrap());
    let transport: ScriptedTransport<TokioExecutor> =
        ScriptedTransport::new(Vec::<Step>::new()).with_executor(executor.clone());

    let runtime: RuntimeHandle<PtzOpticsG2, TokioExecutor> =
        RuntimeHandle::new(transport, executor)
            .await
            .expect("runtime should start");

    let mut metrics_tasks = Vec::new();
    for _ in 0..128 {
        let runtime = runtime.clone();
        metrics_tasks.push(tokio::spawn(async move { runtime.metrics().await }));
    }
    tokio::task::yield_now().await;

    tokio::time::timeout(Duration::from_secs(1), runtime.shutdown())
        .await
        .expect("shutdown should not wait behind normal control traffic")
        .expect("shutdown should complete");

    for task in metrics_tasks {
        let result = tokio::time::timeout(Duration::from_secs(1), task)
            .await
            .expect("metrics waiter should be released by shutdown")
            .expect("metrics task should not panic");
        assert!(
            result.is_ok() || matches!(result, Err(Error::RuntimeShutdown)),
            "metrics request should either complete before shutdown or fail as shutdown, got {result:?}"
        );
    }
}

#[tokio::test]
async fn transport_termination_releases_concurrent_admission_waiter() {
    let executor = Arc::new(TokioExecutor::from_current().unwrap());
    let transport: ScriptedTransport<TokioExecutor> =
        ScriptedTransport::new(vec![Step::InjectError(Error::ConnectionClosed {
            reason: Some("peer closed".into()),
        })])
        .with_executor(executor.clone());

    let runtime: RuntimeHandle<PtzOpticsG2, TokioExecutor> =
        RuntimeHandle::new(transport, executor)
            .await
            .expect("runtime should start");

    let admission_result = tokio::time::timeout(
        Duration::from_secs(1),
        runtime.send_command_with_id(&Zoom::TeleStd, CameraId::CAMERA_1, None),
    )
    .await
    .expect("admission waiter should not hang if runtime terminates first");

    match admission_result {
        Ok((_cmd_id, response)) => {
            let response_result = tokio::time::timeout(Duration::from_secs(1), response)
                .await
                .expect("admitted response future should not hang after transport termination");
            assert!(
                matches!(response_result, Err(Error::ConnectionClosed { .. })),
                "admitted command should preserve ConnectionClosed, got {response_result:?}"
            );
        }
        Err(error) => {
            assert!(
                !matches!(error, Error::RuntimeShutdown),
                "unexpected transport termination must not be normalized to RuntimeShutdown: {error:?}"
            );
        }
    }
}

#[tokio::test]
async fn admitted_work_failed_by_transport_termination_preserves_cause() {
    let executor = Arc::new(TokioExecutor::from_current().unwrap());
    let transport: ScriptedTransport<TokioExecutor> = ScriptedTransport::new(vec![
        Step::OnSend {
            matches: None,
            responses: vec![],
        },
        Step::InjectError(Error::ConnectionClosed {
            reason: Some("peer closed".into()),
        }),
    ])
    .with_executor(executor.clone());

    let runtime: RuntimeHandle<PtzOpticsG2, TokioExecutor> =
        RuntimeHandle::new(transport, executor)
            .await
            .expect("runtime should start");

    let (cmd_id, response) = runtime
        .send_command_with_id(&Zoom::TeleStd, CameraId::CAMERA_1, None)
        .await
        .expect("command should be admitted before the scripted transport error");

    let response_result = tokio::time::timeout(Duration::from_secs(1), response)
        .await
        .expect("response future should not hang after transport termination");
    assert!(
        matches!(response_result, Err(Error::ConnectionClosed { .. })),
        "transport termination should preserve ConnectionClosed, got {response_result:?}"
    );

    tokio::time::sleep(Duration::from_millis(50)).await;

    assert_peer_closed(
        runtime
            .send_command(&Zoom::WideStd, CameraId::CAMERA_1, None)
            .await,
        "post-termination command",
    );
    assert_peer_closed(
        runtime
            .send_inquiry(&PowerInquiry, CameraId::CAMERA_1)
            .await,
        "post-termination inquiry",
    );
    assert_peer_closed(
        runtime.cancel(CameraId::CAMERA_1, cmd_id).await,
        "post-termination cancel",
    );
    assert_peer_closed(runtime.metrics().await, "post-termination metrics request");
    assert_peer_closed(
        runtime.subscribe_completions().await,
        "post-termination completion subscription",
    );
    assert_peer_closed(runtime.shutdown().await, "post-termination shutdown");
}
