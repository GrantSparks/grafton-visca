#![cfg(feature = "blocking")]

use grafton_visca::{
    blocking::Camera, profiles::PtzOpticsG2, profiles::SonyEviH100, profiles::SonyFr7,
};

fn ptzoptics_vendor_commands(camera: &Camera<PtzOpticsG2>) {
    let _ = camera
        .exposure()
        .set_anti_flicker(grafton_visca::types::AntiFlickerMode::Hz50);
    let _ = camera.exposure().anti_flicker();
    let _ = camera.system().save_settings();
    let _ = camera
        .presets()
        .set_recall_speed(grafton_visca::types::PresetSpeed::new(12).expect("valid recall speed"));
    let _ = camera.streaming().set_multicast(true);
    let _ = camera.streaming().set_multicast(false);
    let _ = camera
        .streaming()
        .set_ndi_quality(grafton_visca::types::NdiQuality::High);
}

fn sony_spotlight_commands(camera: &Camera<SonyFr7>) {
    let _ = camera.exposure().set_spotlight(true);
    let _ = camera.exposure().set_spotlight(false);
}

fn sony_tally_commands(camera: &Camera<SonyFr7>) {
    let _ = camera.tally().set_red(true);
    let _ = camera.tally().set_red(false);
    let _ = camera.tally().set_green(true);
    let _ = camera.tally().set_green(false);
}

fn sony_auto_slow_shutter_commands(camera: &Camera<SonyEviH100>) {
    let _ = camera.exposure().set_auto_slow_shutter(true);
    let _ = camera.exposure().set_auto_slow_shutter(false);
}

fn main() {
    let _: fn(&Camera<PtzOpticsG2>) = ptzoptics_vendor_commands;
    let _: fn(&Camera<SonyFr7>) = sony_spotlight_commands;
    let _: fn(&Camera<SonyFr7>) = sony_tally_commands;
    let _: fn(&Camera<SonyEviH100>) = sony_auto_slow_shutter_commands;
}
