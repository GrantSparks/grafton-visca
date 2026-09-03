//! Focused contracts for the pre-2.0 API-lock cleanup (#729).

use grafton_visca::{profiles::PtzOpticsG2, ProfileSpec};

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
    assert_send_sync::<Camera<'static, PtzOpticsG2>>();
}

#[cfg(feature = "blocking")]
#[test]
fn blocking_network_constructors_have_the_profile_bound_return_type() {
    use grafton_visca::{
        blocking::{CameraSession, Connect},
        camera::TransportOptions,
    };

    fn direct() -> grafton_visca::Result<CameraSession<PtzOpticsG2>> {
        Connect::open_tcp::<PtzOpticsG2>("127.0.0.1:5678")
    }

    fn runtime_selected() -> grafton_visca::Result<CameraSession<PtzOpticsG2>> {
        Connect::open::<PtzOpticsG2>(TransportOptions::tcp("127.0.0.1:5678"))
    }

    let _ = direct;
    let _ = runtime_selected;
}

#[cfg(feature = "blocking")]
#[test]
fn runtime_selected_standard_open_rejects_non_network_transports_before_io() {
    use grafton_visca::{blocking::Connect, camera::TransportOptions, Error};

    let error = Connect::open::<PtzOpticsG2>(TransportOptions::Custom)
        .expect_err("custom transports require the explicit Session::open path");

    assert!(
        matches!(&error, Error::InvalidState(reason) if reason.contains("Session::open")),
        "unexpected preflight error: {error:?}"
    );
}

#[cfg(feature = "runtime-tokio")]
#[tokio::test]
async fn async_runtime_selected_standard_open_rejects_non_network_transports_before_io() {
    use grafton_visca::{camera::TransportOptions, runtime::TokioRuntime, Connect, Error};

    let runtime = TokioRuntime::from_current().expect("Tokio runtime");
    let error = Connect::open::<PtzOpticsG2, _>(TransportOptions::Custom, runtime)
        .await
        .expect_err("custom transports require the explicit Session::open path");

    assert!(
        matches!(&error, Error::InvalidState(reason) if reason.contains("Session::open")),
        "unexpected preflight error: {error:?}"
    );
}
