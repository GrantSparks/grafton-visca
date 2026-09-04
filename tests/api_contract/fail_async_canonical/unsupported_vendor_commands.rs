use grafton_visca::{profiles::GenericVisca, Camera};

fn main() {
    let camera: Option<Camera<GenericVisca>> = None;
    let camera = camera.as_ref().unwrap();

    let _ = camera
        .exposure()
        .set_anti_flicker(grafton_visca::command::AntiFlickerMode::Hz50);
    let _ = camera.exposure().flicker_mode();
    let _ = camera.system().save_settings();
    let _ = camera.presets().set_recall_speed(
        grafton_visca::command::PresetRecallSpeed::new(12).expect("valid recall speed"),
    );
    let _ = camera.exposure().spotlight_on();
    let _ = camera.exposure().spotlight_off();
    let _ = camera.exposure().auto_slow_shutter_on();
    let _ = camera.exposure().auto_slow_shutter_off();
    let _ = camera.advanced().multicast_on();
    let _ = camera.advanced().multicast_off();
    let _ = camera
        .advanced()
        .set_ndi_quality(grafton_visca::types::NdiQuality::High);
}

//~ E0277
//~ "does not declare PTZOptics anti-flicker support"
//~ "does not declare PTZOptics settings-save support"
//~ "does not declare PTZOptics preset-recall speed support"
//~ "does not declare Sony spotlight support"
//~ "does not declare Sony auto slow-shutter support"
//~ "does not declare PTZOptics multicast-streaming support"
//~ "does not declare PTZOptics NDI-quality support"
