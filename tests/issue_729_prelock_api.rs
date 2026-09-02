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
