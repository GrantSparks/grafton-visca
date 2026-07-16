//! Issue #539: async operation handle surface (`InFlight`) parity.
//!
//! Verifies the async half of the mode-honest `submit` primitive exposes the same
//! surface as the blocking handle: `submit` / `submit_continuous` returning a
//! handle driven by `await_applied` / `await_settled` / `detach`, with the
//! continuous/stop guard on `await_settled`.

#![cfg(all(feature = "runtime-tokio", feature = "test-utils"))]

use std::time::Duration;

use grafton_visca::{
    camera::{profiles::PtzOpticsG2, CameraBuilder},
    command::PanTilt as PanTiltCmd,
    runtime::TokioRuntime,
    testing::testkit::{helpers, ScriptedTransport, Step},
    Error, TokioExecutor,
};

const TIMEOUT: Duration = Duration::from_secs(2);

fn ack_and_complete() -> Vec<Step> {
    vec![Step::OnSend {
        matches: None,
        responses: vec![helpers::ack(1), helpers::complete(1)],
    }]
}

async fn camera(
    steps: Vec<Step>,
) -> grafton_visca::camera::AsyncCamera<PtzOpticsG2, ScriptedTransport<TokioExecutor>, TokioRuntime>
{
    let transport: ScriptedTransport<TokioExecutor> = ScriptedTransport::new(steps);
    let runtime = TokioRuntime::from_current().expect("runtime");
    CameraBuilder::with_executor(runtime)
        .open_async::<PtzOpticsG2, _>(transport)
        .await
        .expect("camera")
}

/// `submit(...).await_applied(...)` resolves when the command is applied.
#[tokio::test]
async fn test_async_submit_await_applied() {
    let camera = camera(ack_and_complete()).await;

    let handle = camera.submit(&PanTiltCmd::Home).await.expect("submit");
    handle
        .await_applied(TIMEOUT)
        .await
        .expect("command should be applied");
}

/// `await_settled` on a continuous/stop handle is `Error::NotSupported`.
#[tokio::test]
async fn test_async_submit_continuous_await_settled_not_supported() {
    let camera = camera(ack_and_complete()).await;

    let handle = camera
        .submit_continuous(&PanTiltCmd::Home)
        .await
        .expect("submit_continuous");
    let result = handle.await_settled(TIMEOUT).await;

    assert!(
        matches!(result, Err(Error::NotSupported)),
        "await_settled on a continuous handle must be NotSupported, got {result:?}"
    );
}

/// `detach()` is the explicit fire-and-forget; it consumes the handle and never
/// stops the (already-dispatched) command.
#[tokio::test]
async fn test_async_detach_is_fire_and_forget() {
    let camera = camera(ack_and_complete()).await;

    let handle = camera.submit(&PanTiltCmd::Home).await.expect("submit");
    handle.detach();
}
