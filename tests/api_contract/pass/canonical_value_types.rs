// #806: one public type per value. The root, `types`, command, and request
// paths all name the same `FocusSpeed`.
use grafton_visca::{
    command::{Focus, SetMotionSyncPreset, TallyStatusState, VersionInfo},
    request::builtin::FocusDrive,
    types, FocusSpeed, MotionSyncSpeed,
};

fn main() {
    let root = FocusSpeed::new(3).unwrap();
    let typed: types::FocusSpeed = root;
    let _ = Focus::FarWithSpeed(root);
    let _ = Focus::NearWithSpeed(typed);
    let _ = FocusDrive::FarVariable(root);
    let _ = VersionInfo {
        vendor: 0,
        model: 0,
        rom_version: 0,
        max_socket: 1,
    };
    let _ = TallyStatusState {
        red_on: true,
        green_on: false,
    };
    assert_eq!(
        SetMotionSyncPreset::new(MotionSyncSpeed::SLOW).speed(),
        MotionSyncSpeed::from(grafton_visca::MotionSyncPreset::Slow)
    );
}
