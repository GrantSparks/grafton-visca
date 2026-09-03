#![cfg(feature = "async")]

use grafton_visca::{
    profiles::{PtzOpticsG2, SonyEVIH100, SonyFR7},
    Camera,
};

fn ptzoptics_vendor_commands(camera: &Camera<PtzOpticsG2>) {
    let _ = camera
        .exposure()
        .set_anti_flicker(grafton_visca::command::AntiFlickerMode::Hz50);
    let _ = camera.exposure().flicker_mode();
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

fn sony_spotlight_commands(camera: &Camera<SonyFR7>) {
    let _ = camera.exposure().spotlight_on();
    let _ = camera.exposure().spotlight_off();
}

fn sony_tally_commands(camera: &Camera<SonyFR7>) {
    let _ = camera.tally().red_on();
    let _ = camera.tally().red_off();
    let _ = camera.tally().green_on();
    let _ = camera.tally().green_off();
}

fn sony_auto_slow_shutter_commands(camera: &Camera<SonyEVIH100>) {
    let _ = camera.exposure().auto_slow_shutter_on();
    let _ = camera.exposure().auto_slow_shutter_off();
}

fn main() {
    let _: fn(&Camera<PtzOpticsG2>) = ptzoptics_vendor_commands;
    let _: fn(&Camera<SonyFR7>) = sony_spotlight_commands;
    let _: fn(&Camera<SonyFR7>) = sony_tally_commands;
    let _: fn(&Camera<SonyEVIH100>) = sony_auto_slow_shutter_commands;
}
