//! Gain-limit values use their register's full nibble, independently of gain levels.

#![cfg(feature = "blocking")]

use grafton_visca::{
    blocking::Session,
    config::SessionConfig,
    profiles::{
        CompileTimeProfile, GenericVisca, NearusBrc300, ProfileSpec, PtzOpticsG2, SonyBrc300,
        SonyBrcH900, SonyEviH100, SonyFr7,
    },
    types::GainLimit,
    Error,
};
use grafton_visca_test_support::fake_camera::FakeCamera;

#[test]
fn blocking_gain_limit_writes_the_documented_upper_boundary() {
    let fake = FakeCamera::acking(1);
    let profile = ProfileSpec::from_compile_time::<PtzOpticsG2>().expect("G2 profile");
    let session =
        Session::open(fake.blocking_wire(), &SessionConfig::new(profile)).expect("session");
    let camera = session
        .camera::<PtzOpticsG2>(grafton_visca::types::IndividualCameraId::CAMERA_1)
        .expect("camera");
    camera
        .exposure()
        .set_gain_limit(GainLimit::new(0x0F).expect("full nibble"))
        .expect("documented gain limit");
    assert_eq!(fake.writes(), [vec![0x81, 1, 4, 0x2C, 0x0F, 0xFF]]);
    session.close().expect("close");
}

fn assert_unsourced_limit_does_not_write<P: CompileTimeProfile>() {
    let fake = FakeCamera::silent();
    let profile = ProfileSpec::from_compile_time::<P>().expect("built-in profile");
    let session =
        Session::open(fake.blocking_wire(), &SessionConfig::new(profile)).expect("session");
    let camera = session
        .camera::<P>(grafton_visca::types::IndividualCameraId::CAMERA_1)
        .expect("camera");
    assert!(matches!(
        camera.execute(&grafton_visca::command::exposure::SetGainLimit::new(
            GainLimit::new(0x0F).expect("full nibble")
        )),
        Err(Error::FeatureNotSupported { .. })
    ));
    assert!(fake.writes().is_empty());
    session.close().expect("close");
}

#[test]
fn unsourced_gain_limits_never_reach_the_transport() {
    assert_unsourced_limit_does_not_write::<SonyFr7>();
    assert_unsourced_limit_does_not_write::<SonyBrcH900>();
    assert_unsourced_limit_does_not_write::<SonyEviH100>();
    assert_unsourced_limit_does_not_write::<SonyBrc300>();
    assert_unsourced_limit_does_not_write::<NearusBrc300>();
    assert_unsourced_limit_does_not_write::<GenericVisca>();
}
