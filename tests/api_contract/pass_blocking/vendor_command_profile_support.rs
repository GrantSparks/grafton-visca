#![cfg(feature = "blocking")]

use grafton_visca::{
    blocking::Camera,
    profiles::{PtzOpticsG2, SonyFR7},
};

fn ptzoptics_vendor_commands<'session>(camera: &Camera<'session, PtzOpticsG2>) {
    let _ = camera
        .exposure()
        .set_anti_flicker(grafton_visca::command::AntiFlickerMode::Hz50);
    let _ = camera.system().save_settings();
    let _ = camera.presets().set_recall_speed(
        grafton_visca::command::PresetRecallSpeed::new(12).expect("valid recall speed"),
    );
    let _ = camera.advanced().multicast_on();
    let _ = camera.advanced().multicast_off();
    let _ = camera
        .advanced()
        .set_ndi_quality(grafton_visca::types::NdiQuality::High);
}

fn sony_vendor_commands<'session>(camera: &Camera<'session, SonyFR7>) {
    let _ = camera.exposure().spotlight_on();
    let _ = camera.exposure().spotlight_off();
    let _ = camera.exposure().auto_slow_shutter_on();
    let _ = camera.exposure().auto_slow_shutter_off();
}

fn main() {
    let _: for<'session> fn(&'session Camera<'session, PtzOpticsG2>) = ptzoptics_vendor_commands;
    let _: for<'session> fn(&'session Camera<'session, SonyFR7>) = sony_vendor_commands;
}
