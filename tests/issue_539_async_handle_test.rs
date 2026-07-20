//! Issue #539: async operation-handle completion parity.

#![cfg(all(feature = "mode-async", feature = "test-utils"))]

#[cfg(any(feature = "runtime-tokio", feature = "runtime-smol"))]
use std::time::Duration;

#[cfg(any(feature = "runtime-tokio", feature = "runtime-smol"))]
use grafton_visca::testing::testkit::{helpers, Step};
use grafton_visca::{
    camera::{Axes, OperationMetadata},
    command::{PanTilt as PanTiltCmd, ViscaCommand, Zoom},
};

#[cfg(any(feature = "runtime-tokio", feature = "runtime-smol"))]
const TIMEOUT: Duration = Duration::from_secs(2);
#[cfg(any(feature = "runtime-tokio", feature = "runtime-smol"))]
const HOME: &[u8] = &[0x81, 0x01, 0x06, 0x04, 0xFF];
#[cfg(any(feature = "runtime-tokio", feature = "runtime-smol"))]
const PAN_TILT_POSITION_INQUIRY: &[u8] = &[0x81, 0x09, 0x06, 0x12, 0xFF];

#[cfg(any(feature = "runtime-tokio", feature = "runtime-smol"))]
fn ack_and_complete() -> Step {
    Step::OnSend {
        matches: Some(HOME.to_vec()),
        responses: vec![helpers::ack(1), helpers::complete(1)],
    }
}

#[cfg(any(feature = "runtime-tokio", feature = "runtime-smol"))]
fn pan_tilt_position(pan: u16, tilt: u16) -> Vec<u8> {
    let mut response = vec![0x90, 0x50];
    for value in [pan, tilt] {
        response.extend([
            ((value >> 12) & 0x0f) as u8,
            ((value >> 8) & 0x0f) as u8,
            ((value >> 4) & 0x0f) as u8,
            (value & 0x0f) as u8,
        ]);
    }
    response.push(0xff);
    response
}

#[cfg(any(feature = "runtime-tokio", feature = "runtime-smol"))]
fn polling_settle_steps() -> Vec<Step> {
    vec![
        ack_and_complete(),
        helpers::inquiry_response(
            PAN_TILT_POSITION_INQUIRY.to_vec(),
            1,
            pan_tilt_position(0, 0),
        ),
        helpers::inquiry_response(
            PAN_TILT_POSITION_INQUIRY.to_vec(),
            1,
            pan_tilt_position(0, 0),
        ),
    ]
}

#[test]
fn built_in_metadata_is_command_derived() {
    assert_eq!(
        PanTiltCmd::Home.operation_metadata(),
        Some(OperationMetadata::targeted(Axes::PAN_TILT))
    );
    assert_eq!(
        Zoom::TeleStd.operation_metadata(),
        Some(OperationMetadata::applied_only(Axes::ZOOM))
    );
    assert_eq!(
        Zoom::Position(grafton_visca::types::ZoomPosition::MIN).operation_metadata(),
        Some(OperationMetadata::targeted(Axes::ZOOM))
    );
}

#[cfg(feature = "runtime-tokio")]
mod tokio_tests {
    use grafton_visca::{
        camera::{profiles::GenericVisca, profiles::PtzOpticsG2, CameraBuilder},
        runtime::TokioRuntime,
        testing::testkit::ScriptedTransport,
        Error, TokioExecutor,
    };

    use super::*;

    async fn build_camera<P>(
        steps: Vec<Step>,
    ) -> (
        grafton_visca::camera::AsyncCamera<P, ScriptedTransport<TokioExecutor>, TokioRuntime>,
        ScriptedTransport<TokioExecutor>,
    )
    where
        P: grafton_visca::capabilities::Profile + Default,
    {
        let transport: ScriptedTransport<TokioExecutor> = ScriptedTransport::new(steps);
        let observed = transport.clone();
        let runtime = TokioRuntime::from_current().expect("runtime");
        let camera = CameraBuilder::with_executor(runtime)
            .open_async::<P, _>(transport)
            .await
            .expect("camera");
        (camera, observed)
    }

    #[tokio::test]
    async fn submit_await_applied_and_continuous_guard() {
        let (camera, _) = build_camera::<PtzOpticsG2>(vec![ack_and_complete()]).await;
        camera
            .submit(&PanTiltCmd::Home)
            .await
            .expect("submit")
            .await_applied(TIMEOUT)
            .await
            .expect("applied");

        let (camera, _) = build_camera::<PtzOpticsG2>(vec![]).await;
        let result = camera
            .submit(&Zoom::TeleStd)
            .await
            .expect("submit continuous command")
            .await_settled(TIMEOUT)
            .await;
        assert!(matches!(result, Err(Error::NotSupported)));
    }

    #[tokio::test]
    async fn submit_rejects_inquiries_before_io() {
        let (camera, transport) = build_camera::<PtzOpticsG2>(vec![]).await;
        let result = camera
            .submit(&grafton_visca::command::PanTiltPositionInquiry)
            .await;
        assert!(matches!(result, Err(Error::InquiryNotCancelable)));
        assert!(transport.sent().is_empty());
    }

    #[tokio::test]
    async fn settled_uses_exact_completion_when_profile_supports_it() {
        let (camera, transport) = build_camera::<PtzOpticsG2>(vec![ack_and_complete()]).await;
        camera
            .submit(&PanTiltCmd::Home)
            .await
            .expect("submit")
            .await_settled(TIMEOUT)
            .await
            .expect("settled");
        assert_eq!(transport.sent(), vec![HOME.to_vec()]);
    }

    #[tokio::test]
    async fn settled_polls_only_affected_axes_without_operation_complete() {
        let (camera, transport) = build_camera::<GenericVisca>(polling_settle_steps()).await;
        camera
            .submit(&PanTiltCmd::Home)
            .await
            .expect("submit")
            .await_settled(TIMEOUT)
            .await
            .expect("settled");

        let sent = transport.sent();
        assert_eq!(sent.len(), 3);
        assert_eq!(sent[0], HOME);
        assert_eq!(sent[1], PAN_TILT_POSITION_INQUIRY);
        assert_eq!(sent[2], PAN_TILT_POSITION_INQUIRY);
    }

    #[tokio::test]
    async fn settled_deadline_bounds_a_stalled_position_inquiry() {
        let (camera, _) = build_camera::<GenericVisca>(vec![ack_and_complete()]).await;
        let started = std::time::Instant::now();
        let result = camera
            .submit(&PanTiltCmd::Home)
            .await
            .expect("submit")
            .await_settled(Duration::from_millis(40))
            .await;

        assert!(matches!(result, Err(Error::Timeout)));
        assert!(
            started.elapsed() < Duration::from_millis(500),
            "the position inquiry must share the handle deadline"
        );
    }

    #[tokio::test]
    async fn settled_propagates_the_exact_command_error() {
        let (camera, _) = build_camera::<PtzOpticsG2>(vec![helpers::errors::syntax_error(1)]).await;
        let result = camera
            .submit(&PanTiltCmd::Home)
            .await
            .expect("submit")
            .await_settled(TIMEOUT)
            .await;

        assert!(
            matches!(result, Err(Error::SyntaxError)),
            "the operation's own camera error must propagate: {result:?}"
        );
    }

    #[tokio::test]
    async fn detach_is_explicit_fire_and_forget() {
        let (camera, _) = build_camera::<PtzOpticsG2>(vec![ack_and_complete()]).await;
        camera
            .submit(&PanTiltCmd::Home)
            .await
            .expect("submit")
            .detach();
    }
}

#[cfg(all(feature = "runtime-smol", not(feature = "runtime-tokio")))]
mod smol_tests {
    use std::sync::Arc;

    use grafton_visca::{
        camera::{profiles::GenericVisca, profiles::PtzOpticsG2, CameraBuilder},
        testing::testkit::ScriptedTransport,
        SmolExecutor,
    };

    use super::*;

    #[test]
    fn smol_applied_and_settled_profile_paths() {
        smol::block_on(async {
            let executor = Arc::new(SmolExecutor::new());
            let transport =
                ScriptedTransport::new(vec![ack_and_complete()]).with_executor(executor.clone());
            let observed = transport.clone();
            let camera = CameraBuilder::<SmolExecutor>::with_executor(executor)
                .open_async::<PtzOpticsG2, _>(transport)
                .await
                .expect("camera");
            camera
                .submit(&PanTiltCmd::Home)
                .await
                .expect("submit")
                .await_settled(TIMEOUT)
                .await
                .expect("settled from completion");
            assert_eq!(observed.sent(), vec![HOME.to_vec()]);

            let executor = Arc::new(SmolExecutor::new());
            let transport =
                ScriptedTransport::new(polling_settle_steps()).with_executor(executor.clone());
            let observed = transport.clone();
            let camera = CameraBuilder::<SmolExecutor>::with_executor(executor)
                .open_async::<GenericVisca, _>(transport)
                .await
                .expect("camera");
            camera
                .submit(&PanTiltCmd::Home)
                .await
                .expect("submit")
                .await_settled(TIMEOUT)
                .await
                .expect("settled by polling");
            assert_eq!(observed.sent().len(), 3);
        });
    }
}
