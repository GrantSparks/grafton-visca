use grafton_visca::{blocking::Camera, profiles::GenericVisca};

fn main() {
    let camera: Option<Camera<GenericVisca>> = None;
    let camera = camera.as_ref().unwrap();

    let _ = camera
        .exposure()
        .set_anti_flicker(grafton_visca::types::AntiFlickerMode::Hz50);
    let _ = camera.exposure().anti_flicker();
    let _ = camera.system().save_settings();
    let _ = camera
        .presets()
        .set_recall_speed(grafton_visca::types::PresetSpeed::new(12).expect("valid recall speed"));
    let _ = camera.exposure().set_spotlight(true);
    let _ = camera.exposure().set_spotlight(false);
    let _ = camera.exposure().set_auto_slow_shutter(true);
    let _ = camera.exposure().set_auto_slow_shutter(false);
    let _ = camera.streaming().set_multicast(true);
    let _ = camera.streaming().set_multicast(false);
    let _ = camera
        .streaming()
        .set_ndi_quality(grafton_visca::types::NdiQuality::High);
}

//~ E0277
//~ "camera profile `grafton_visca::profiles::GenericVisca` does not support `grafton_visca::capabilities::gate::AntiFlicker`"
//~ "camera profile `grafton_visca::profiles::GenericVisca` does not support `grafton_visca::capabilities::gate::SettingsSave`"
//~ "camera profile `grafton_visca::profiles::GenericVisca` does not support `grafton_visca::capabilities::gate::Spotlight`"
//~ "camera profile `grafton_visca::profiles::GenericVisca` does not support `grafton_visca::capabilities::gate::AutoSlowShutter`"
//~ "camera profile `grafton_visca::profiles::GenericVisca` does not support `grafton_visca::capabilities::gate::MulticastStreaming`"
//~ "camera profile `grafton_visca::profiles::GenericVisca` does not support `grafton_visca::capabilities::gate::NdiQuality`"
//~ "camera profile `grafton_visca::profiles::GenericVisca` does not support `grafton_visca::capabilities::gate::PresetRecallSpeed`"
