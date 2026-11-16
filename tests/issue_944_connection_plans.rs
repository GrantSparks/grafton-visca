#![cfg(any(feature = "blocking", feature = "runtime-tokio"))]

use grafton_visca::{
    config::{CameraConfig, SessionConfig, TransportKind, TransportOptions},
    profiles::ProfileId,
    Error,
};

fn refused_configs() -> [(CameraConfig, Option<TransportKind>); 4] {
    [
        (
            CameraConfig::new(
                ProfileId::SonyFr7,
                TransportOptions::tcp("issue-944-no-dns.invalid"),
            ),
            Some(TransportKind::Tcp),
        ),
        (
            CameraConfig::new(
                ProfileId::SonyBrc300,
                TransportOptions::udp("issue-944-no-dns.invalid"),
            ),
            Some(TransportKind::Udp),
        ),
        (
            CameraConfig::new(
                ProfileId::SonyFr7,
                TransportOptions::serial("/dev/issue-944-never-opened", 9600),
            ),
            Some(TransportKind::Serial),
        ),
        (
            CameraConfig::new(ProfileId::GenericVisca, TransportOptions::Custom),
            None,
        ),
    ]
}

fn assert_refusal(error: Error, transport: Option<TransportKind>) {
    match transport {
        Some(expected) => assert!(
            matches!(error, Error::UnsupportedTransport { transport, .. } if transport == expected)
        ),
        None => assert!(
            matches!(error, Error::InvalidState { reason } if reason == "custom transport requires Session::open")
        ),
    }
}

#[cfg(feature = "blocking")]
#[test]
fn blocking_connect_preflights_every_transport_before_io() {
    for (config, transport) in refused_configs() {
        assert_refusal(
            grafton_visca::blocking::CameraSession::connect(&config).expect_err("camera preflight"),
            transport,
        );
        let session = SessionConfig::new(config.profile().expect("profile"))
            .with_transport(config.transport().clone());
        assert_refusal(
            grafton_visca::blocking::Session::connect(&session).expect_err("session preflight"),
            transport,
        );
    }
    #[cfg(not(feature = "transport-serial"))]
    {
        let config = CameraConfig::new(
            ProfileId::GenericVisca,
            TransportOptions::serial("/dev/issue-944-never-opened", 9600),
        );
        assert!(matches!(
            grafton_visca::blocking::CameraSession::connect(&config),
            Err(Error::FeatureNotSupported {
                feature: "blocking serial transport",
                ..
            })
        ));
    }
}

#[cfg(feature = "runtime-tokio")]
#[tokio::test]
async fn async_connect_preflights_every_transport_before_io() {
    let runtime = grafton_visca::runtime::TokioRuntime::from_current().expect("runtime");
    for (config, transport) in refused_configs() {
        assert_refusal(
            grafton_visca::r#async::CameraSession::connect(&config, runtime.clone())
                .await
                .expect_err("camera preflight"),
            transport,
        );
        let session = SessionConfig::new(config.profile().expect("profile"))
            .with_transport(config.transport().clone());
        assert_refusal(
            grafton_visca::r#async::Session::connect(&session, runtime.clone())
                .await
                .expect_err("session preflight"),
            transport,
        );
    }
    #[cfg(not(feature = "transport-serial-tokio"))]
    {
        let config = CameraConfig::new(
            ProfileId::GenericVisca,
            TransportOptions::serial("/dev/issue-944-never-opened", 9600),
        );
        assert!(matches!(
            grafton_visca::r#async::CameraSession::connect(&config, runtime.clone()).await,
            Err(Error::FeatureNotSupported {
                feature: "async serial transport",
                ..
            })
        ));
    }
}

#[cfg(feature = "runtime-tokio")]
#[tokio::test]
async fn async_camera_open_preserves_configured_transport_validation() {
    use grafton_visca::{
        profiles::SonyFr7,
        r#async::{CameraSession, Session},
        runtime::Runtime,
    };
    let runtime = grafton_visca::runtime::TokioRuntime::from_current().expect("runtime");
    let camera = CameraConfig::new(ProfileId::SonyFr7, TransportOptions::tcp("unused.invalid"))
        .with_camera_id(grafton_visca::types::CameraId::CAMERA_3);
    let udp = runtime
        .connect_udp(
            "127.0.0.1:9",
            grafton_visca::transport::TransportConfig::for_udp(),
        )
        .await
        .expect("local UDP transport");
    assert!(matches!(
        CameraSession::open(udp, &camera, runtime.clone()).await,
        Err(Error::UnsupportedTransport {
            transport: TransportKind::Tcp,
            ..
        })
    ));
    let config = SessionConfig::from_compile_time::<SonyFr7>()
        .expect("profile")
        .with_transport(TransportOptions::tcp("unused.invalid"));
    let udp = runtime
        .connect_udp(
            "127.0.0.1:9",
            grafton_visca::transport::TransportConfig::for_udp(),
        )
        .await
        .expect("local UDP transport");
    Session::open(udp, &config, runtime.clone())
        .await
        .expect("session checks actual UDP transport")
        .close()
        .await
        .expect("close");
    let udp = runtime
        .connect_udp(
            "127.0.0.1:9",
            grafton_visca::transport::TransportConfig::for_udp(),
        )
        .await
        .expect("local UDP transport");
    let single = CameraSession::open(
        udp,
        &camera.with_transport(TransportOptions::Custom),
        runtime,
    )
    .await
    .expect("custom selection uses opened UDP facts");
    assert_eq!(single.target(), grafton_visca::types::CameraId::CAMERA_3);
    single.close().await.expect("close");
}
