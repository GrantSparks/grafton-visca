//! Regression tests for issue #523: typed runtime boundary and shutdown semantics.

#![cfg(all(
    feature = "mode-async",
    feature = "runtime-tokio",
    feature = "test-utils"
))]

use std::sync::Arc;
use std::time::Duration;

use grafton_visca::{
    camera::profiles::PtzOpticsG2,
    command::{InquiryKind, ViscaCommand, Zoom, VISCA_TERMINATOR},
    runtime::testing::RuntimeHandle,
    testing::testkit::{helpers, ScriptedTransport, Step},
    timeout::CommandCategory,
    CameraId, Error, TokioExecutor, ViscaSocket,
};

struct PowerInquiry;

impl ViscaCommand for PowerInquiry {
    type Response = ();

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

    fn response_kind(&self) -> Option<InquiryKind> {
        Some(InquiryKind::Power)
    }
}

fn assert_runtime_shutdown<T: std::fmt::Debug>(result: Result<T, Error>, context: &str) {
    assert!(
        matches!(result, Err(Error::RuntimeShutdown)),
        "{context} should fail with RuntimeShutdown, got {result:?}"
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

    let (_cmd_id, response) = runtime
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

    let later_result = runtime
        .send_command(&Zoom::WideStd, CameraId::CAMERA_1, None)
        .await;
    assert!(
        !matches!(later_result, Err(Error::RuntimeShutdown)),
        "unexpected transport termination must not be normalized to RuntimeShutdown"
    );
}
