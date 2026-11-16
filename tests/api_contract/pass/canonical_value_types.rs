// #806: noun parameters, request construction and request getters share one
// checked value type at its canonical topical home.
use grafton_visca::{
    command::{focus, motion_sync},
    config::TargetRegistration,
    profiles::{ProfileSpec, PtzOpticsG2},
    types::{CameraId, FocusSpeed, MotionSyncSpeed, Speed, VersionInfo},
};

fn main() {
    let speed = FocusSpeed::new(3).unwrap();
    let target = TargetRegistration::new(
        grafton_visca::types::IndividualCameraId::CAMERA_1,
        ProfileSpec::from_compile_time::<PtzOpticsG2>().expect("source profile"),
    );
    let far = focus::FarVariable::for_target(&target, speed).expect("profile-aware request");
    let near = focus::NearVariable::for_target(&target, speed).expect("profile-aware request");
    let _: Speed<FocusSpeed> = far.speed();
    assert_eq!(far.speed(), Speed::Exact(speed));
    assert_eq!(near.speed(), Speed::Exact(speed));
    let _ = VersionInfo {
        vendor: 0,
        model: 0,
        rom_version: 0,
        max_socket: 1,
    };
    assert_eq!(
        motion_sync::SetSpeed::new(MotionSyncSpeed::MIN).speed(),
        MotionSyncSpeed::MIN,
    );
}
