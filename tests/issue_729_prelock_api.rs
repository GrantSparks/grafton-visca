//! Focused contracts for the pre-2.0 API-lock cleanup (#729).

use grafton_visca::profiles::{ProfileSpec, PtzOpticsG2};

#[test]
fn runtime_profiles_have_a_canonical_name_accessor() {
    let profile =
        ProfileSpec::from_compile_time::<PtzOpticsG2>().expect("built-in runtime profile");

    assert_eq!(profile.name(), "PtzOptics G2");
    assert_eq!(profile.name(), profile.capabilities().model_name);
}

#[cfg(feature = "blocking")]
#[test]
fn blocking_session_and_camera_views_are_send_and_sync() {
    use grafton_visca::blocking::{Camera, Session};

    fn assert_send_sync<T: Send + Sync>() {}

    assert_send_sync::<Session>();
    assert_send_sync::<Camera<PtzOpticsG2>>();
}

#[cfg(feature = "blocking")]
#[test]
fn blocking_network_constructors_have_the_profile_bound_return_type() {
    use grafton_visca::{blocking::CameraSession, config::TransportOptions};

    fn direct() -> grafton_visca::Result<CameraSession<PtzOpticsG2>> {
        grafton_visca::blocking::CameraSession::<PtzOpticsG2>::connect_tcp("127.0.0.1:5678")
    }

    fn runtime_selected() -> grafton_visca::Result<CameraSession<PtzOpticsG2>> {
        grafton_visca::blocking::CameraSession::<PtzOpticsG2>::connect(
            &grafton_visca::config::CameraConfig::new(
                grafton_visca::profiles::ProfileSpec::from_compile_time::<PtzOpticsG2>()
                    .expect("source profile"),
                TransportOptions::tcp("127.0.0.1:5678"),
            )
            .into_typed::<PtzOpticsG2>()
            .expect("typed config"),
        )
    }

    let _ = direct;
    let _ = runtime_selected;
}

#[cfg(feature = "blocking")]
#[test]
fn runtime_selected_standard_open_rejects_non_network_transports_before_io() {
    use grafton_visca::{config::TransportOptions, Error};

    let error = grafton_visca::blocking::CameraSession::<PtzOpticsG2>::connect(
        &grafton_visca::config::CameraConfig::new(
            grafton_visca::profiles::ProfileSpec::from_compile_time::<PtzOpticsG2>()
                .expect("source profile"),
            TransportOptions::Custom,
        )
        .into_typed::<PtzOpticsG2>()
        .expect("typed config"),
    )
    .expect_err("custom transports require the explicit Session::open path");

    assert!(
        matches!(&error, Error::InvalidState { reason } if reason.contains("Session::open")),
        "unexpected preflight error: {error:?}"
    );
}

#[cfg(feature = "runtime-tokio")]
#[tokio::test]
async fn async_runtime_selected_standard_open_rejects_non_network_transports_before_io() {
    use grafton_visca::{config::TransportOptions, runtime::TokioRuntime, Error};

    let runtime = TokioRuntime::from_current().expect("Tokio runtime");
    let error = grafton_visca::r#async::CameraSession::<PtzOpticsG2>::connect(
        &grafton_visca::config::CameraConfig::new(
            grafton_visca::profiles::ProfileSpec::from_compile_time::<PtzOpticsG2>()
                .expect("source profile"),
            TransportOptions::Custom,
        )
        .into_typed::<PtzOpticsG2>()
        .expect("typed config"),
        runtime,
    )
    .await
    .expect_err("custom transports require the explicit Session::open path");

    assert!(
        matches!(&error, Error::InvalidState { reason } if reason.contains("Session::open")),
        "unexpected preflight error: {error:?}"
    );
}
