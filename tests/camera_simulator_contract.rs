//! The public simulator's options exercise real owner sessions (#850).

#![cfg(all(feature = "runtime-tokio", feature = "test-utils"))]

use std::time::Duration;

use grafton_visca::{
    camera::profiles::GenericVisca,
    profile::ProfileSpec,
    raw,
    testing::camera_simulator::{CommandType, ViscaCameraSimulator},
    transport::{AsyncTransport, HasTransportConfig, SendSemantics},
    CameraId, CancellationOutcome, Certainty, ControlClass, Error, ErrorKind, FailureContext,
    FailureStage, Inquiry, Recovery, Request, ResponseDecoder, RetryClass, Session, SessionConfig,
    SessionStatus, TimeoutClass, TokioExecutor,
};

const POWER_ON: &[u8] = &[0x81, 0x01, 0x04, 0x00, 0x02, 0xff];

async fn open(simulator: ViscaCameraSimulator) -> Session {
    Session::open(
        simulator,
        SessionConfig::from_compile_time::<GenericVisca>().expect("generic configuration"),
        TokioExecutor::from_current().expect("Tokio runtime"),
    )
    .await
    .expect("simulator session")
}

async fn reply(simulator: &mut ViscaCameraSimulator) -> Vec<u8> {
    let mut bytes = [0; 128];
    let count = simulator
        .recv_into(&mut bytes)
        .await
        .expect("simulator reply")
        .copied_len();
    bytes[..count].to_vec()
}

fn plain_power() -> raw::Plain {
    raw::Plain::new(
        POWER_ON,
        grafton_visca::CommandCategory::Quick,
        RetryClass::Never,
        grafton_visca::SubmissionClass::Normal,
    )
    .expect("valid power command")
}

#[tokio::test(start_paused = true)]
async fn ack_and_execution_delays_control_session_observations() {
    let simulator = ViscaCameraSimulator::builder()
        .with_ack_delay(Duration::from_millis(200))
        .with_command_execution_time(CommandType::Zoom, Duration::from_millis(700))
        .build();
    let mut monitor = simulator.clone();
    let session = open(simulator).await;
    let camera = session.camera::<GenericVisca>().expect("generic camera");
    let started = tokio::time::Instant::now();
    let mut movement = camera.zoom().tele().await.expect("zoom admitted");

    assert_eq!(reply(&mut monitor).await, [0x90, 0x41, 0xff]);
    assert_eq!(started.elapsed(), Duration::from_millis(200));
    movement.applied().await.expect("zoom completed");
    assert_eq!(started.elapsed(), Duration::from_millis(900));
    session.close().await.expect("close");
}

#[tokio::test(start_paused = true)]
async fn busy_probability_refuses_a_session_request_without_ending_the_session() {
    let simulator = ViscaCameraSimulator::builder()
        .with_busy_probability(1.0)
        .build();
    let session = open(simulator).await;
    let camera = session.camera::<GenericVisca>().expect("generic camera");

    assert!(matches!(
        camera.execute(&plain_power()).await,
        Err(Error::CommandBufferFull)
    ));
    let metrics = session.metrics().await.expect("live metrics");
    assert_eq!(metrics.session, SessionStatus::Running);
    assert_eq!(metrics.busy_errors, 1);
    assert_eq!(metrics.write_failures, 0);
    // Refusing commands does not prevent inquiry replies.
    camera.power().state().await.expect("inquiry while busy");
    session.close().await.expect("close");
}

#[tokio::test(start_paused = true)]
async fn complete_reply_loss_is_a_request_outcome_and_never_a_failed_write() {
    let simulator = ViscaCameraSimulator::builder()
        .with_packet_loss(1.0)
        .build();
    assert_eq!(
        simulator.identity().send_semantics(),
        SendSemantics::Datagram
    );
    let session = open(simulator).await;
    let camera = session.camera::<GenericVisca>().expect("generic camera");

    let error = camera
        .execute(&plain_power())
        .await
        .expect_err("no reply can confirm this command");
    assert!(matches!(error, Error::UnsequencedCommandUnconfirmed));
    let metrics = session.metrics().await.expect("session survived loss");
    assert_eq!(metrics.session, SessionStatus::Running);
    assert_eq!(metrics.write_failures, 0);
    assert_eq!(metrics.received_frames, 0);
    assert_eq!(metrics.terminal, 1);
    session.close().await.expect("close");
}

#[tokio::test(start_paused = true)]
async fn socket_count_changes_backpressure_seen_by_a_real_session() {
    let simulator = ViscaCameraSimulator::builder()
        .with_socket_count(1)
        .with_command_execution_time(CommandType::Zoom, Duration::from_secs(60))
        .build();
    let mut monitor = simulator.clone();
    let session = open(simulator).await;
    let camera = session.camera::<GenericVisca>().expect("generic camera");
    let mut movement = camera.zoom().tele().await.expect("first socket admitted");
    assert_eq!(reply(&mut monitor).await, [0x90, 0x41, 0xff]);
    assert!(matches!(
        camera.execute(&plain_power()).await,
        Err(Error::CommandBufferFull)
    ));
    assert_eq!(
        movement.cancel().await.expect("cancel"),
        CancellationOutcome::Cancelled
    );
    camera
        .execute(&plain_power())
        .await
        .expect("freed socket is reusable");
    session.close().await.expect("close");
}

#[tokio::test(start_paused = true)]
async fn cancelling_an_executing_socket_suppresses_its_pending_completion() {
    let simulator = ViscaCameraSimulator::builder()
        .with_command_execution_time(CommandType::Zoom, Duration::from_secs(10))
        .build();
    let mut monitor = simulator.clone();
    let session = open(simulator).await;
    let camera = session.camera::<GenericVisca>().expect("generic camera");
    let mut movement = camera.zoom().tele().await.expect("zoom admitted");
    assert_eq!(reply(&mut monitor).await, [0x90, 0x41, 0xff]);

    assert_eq!(
        movement.cancel().await.expect("cancel"),
        CancellationOutcome::Cancelled
    );
    assert_eq!(reply(&mut monitor).await, [0x90, 0x61, 0x04, 0xff]);
    assert!(matches!(
        movement.applied().await,
        Err(Error::CommandCanceled)
    ));
    tokio::time::advance(Duration::from_secs(11)).await;
    tokio::task::yield_now().await;
    let metrics = session.metrics().await.expect("live metrics");
    assert_eq!(
        metrics.received_frames, 2,
        "no completion follows cancellation"
    );
    assert_eq!(metrics.active, 0);
    session.close().await.expect("close");
}

#[tokio::test(start_paused = true)]
async fn deterministic_jitter_delays_session_replies_once() {
    async fn trace() -> (Duration, Duration) {
        let simulator = ViscaCameraSimulator::builder()
            .with_ack_delay(Duration::ZERO)
            .with_command_execution_time(CommandType::Zoom, Duration::ZERO)
            .with_network_jitter(80)
            .build();
        let mut monitor = simulator.clone();
        let session = open(simulator).await;
        let camera = session.camera::<GenericVisca>().expect("generic camera");
        let started = tokio::time::Instant::now();
        let mut movement = camera.zoom().tele().await.expect("zoom admitted");
        assert_eq!(reply(&mut monitor).await, [0x90, 0x41, 0xff]);
        let ack = started.elapsed();
        movement.applied().await.expect("zoom completed");
        let completed = started.elapsed();
        session.close().await.expect("close");
        (ack, completed)
    }

    let first = trace().await;
    assert!(first.0 > Duration::ZERO);
    assert!(first.0 <= Duration::from_millis(80));
    assert!(first.1 >= first.0);
    assert!(first.1 <= Duration::from_millis(160));
    assert_eq!(
        first,
        trace().await,
        "identical sends use the same fixed seed"
    );
}

struct UnknownInquiry;

impl Request for UnknownInquiry {
    type Class = grafton_visca::request::Inquiry;
    const MAX_SIZE: usize = 5;
    const TIMEOUT_CLASS: TimeoutClass = TimeoutClass::Inquiry;
    const RETRY_CLASS: RetryClass = RetryClass::Never;
    const CONTROL_CLASS: ControlClass = ControlClass::Normal;

    fn write_into(&self, target: CameraId, buffer: &mut [u8]) -> Result<usize, Error> {
        buffer[..5].copy_from_slice(&[0x80 | target.id(), 0x09, 0x04, 0x7e, 0xff]);
        Ok(5)
    }
}

impl Inquiry for UnknownInquiry {
    type Response = ();

    fn decoder(&self) -> ResponseDecoder<()> {
        ResponseDecoder::from_fn(|_| Ok(()))
    }
}

#[tokio::test(start_paused = true)]
async fn unknown_inquiry_gets_a_syntax_error_instead_of_ack_and_completion() {
    let simulator = ViscaCameraSimulator::new();
    let mut monitor = simulator.clone();
    let session = open(simulator).await;
    let camera = session.camera::<GenericVisca>().expect("generic camera");

    let error = camera
        .inquire(&UnknownInquiry)
        .await
        .expect_err("an unknown inquiry receives the camera's syntax refusal");
    let Error::InquiryFailed { source, .. } = &error else {
        panic!("the unknown inquiry must report its terminal failure, got {error:?}");
    };
    assert!(matches!(source.as_ref(), Error::SyntaxError));
    assert_eq!(error.kind(), ErrorKind::InvalidParameter);
    assert_eq!(
        error.failure_context(),
        Some(FailureContext::new(
            FailureStage::Terminal,
            Certainty::FailedConclusively
        ))
    );
    assert_eq!(error.recovery(), Recovery::Abandon);
    assert!(!error.requires_new_session());
    assert_eq!(reply(&mut monitor).await, [0x90, 0x60, 0x02, 0xff]);
    let metrics = session.metrics().await.expect("metrics");
    assert_eq!(
        metrics.received_frames, 1,
        "no ACK or completion follows the refusal"
    );
    assert_eq!(
        metrics.writes, 1,
        "a custom syntax refusal is never replayed"
    );
    assert_eq!(metrics.retries_scheduled, 0);
    assert_eq!(metrics.active, 0);
    assert_eq!(metrics.session, SessionStatus::Running);
    session.close().await.expect("close");
}

#[tokio::test(start_paused = true)]
async fn one_simulator_routes_replies_to_each_registered_camera() {
    let simulator = ViscaCameraSimulator::new();
    let mut monitor = simulator.clone();
    let profile = ProfileSpec::from_compile_time::<GenericVisca>().expect("generic profile");
    let config = SessionConfig::new(profile.clone())
        .with_target(CameraId::CAMERA_2, profile)
        .expect("second target");
    let session = Session::open(
        simulator,
        config,
        TokioExecutor::from_current().expect("Tokio runtime"),
    )
    .await
    .expect("multi-camera session");
    for (target, address) in [(CameraId::CAMERA_1, 0x90), (CameraId::CAMERA_2, 0xa0)] {
        let camera = session
            .camera_for::<GenericVisca>(target)
            .expect("target camera");
        camera.power().state().await.expect("addressed inquiry");
        assert_eq!(reply(&mut monitor).await, [address, 0x50, 0x02, 0xff]);
    }
    session.close().await.expect("close");
}
