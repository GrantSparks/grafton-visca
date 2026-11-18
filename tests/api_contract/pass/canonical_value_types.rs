// #806: noun parameters, request construction and request getters share one
// checked value type at its canonical topical home.
use grafton_visca::{
    command::{focus, motion_sync},
    types::{CameraId, FocusSpeed, MotionSyncSpeed, Speed, VersionInfo},
};

fn main() {
    let speed = FocusSpeed::new(3).unwrap();
    let far = focus::FarVariable::new(speed);
    let near = focus::NearVariable::new(speed);
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
