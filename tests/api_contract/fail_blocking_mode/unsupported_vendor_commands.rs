use grafton_visca::{blocking::Camera, profiles::GenericVisca};

fn main() {
    let camera: Option<Camera<GenericVisca>> = None;
    let camera = camera.as_ref().unwrap();

    let _ = camera
        .exposure()
        .set_anti_flicker(grafton_visca::types::AntiFlickerMode::Hz50);
        //~^ E0277 "camera profile `grafton_visca::profiles::GenericVisca` does not support `grafton_visca::capabilities::gate::AntiFlicker`"
    let _ = camera.exposure().anti_flicker();
    //~^ E0277 "camera profile `grafton_visca::profiles::GenericVisca` does not support `grafton_visca::capabilities::gate::AntiFlicker`"
    let _ = camera.system().save_settings();
    //~^ E0277 "camera profile `grafton_visca::profiles::GenericVisca` does not support `grafton_visca::capabilities::gate::SettingsSave`"
    let _ = camera
        .presets()
        .set_recall_speed(grafton_visca::types::PresetSpeed::new(12).expect("valid recall speed"));
        //~^ E0277 "camera profile `grafton_visca::profiles::GenericVisca` does not support `grafton_visca::capabilities::gate::PresetRecallSpeed`"
    let _ = camera.exposure().set_spotlight(true);
    //~^ E0277 "camera profile `grafton_visca::profiles::GenericVisca` does not support `grafton_visca::capabilities::gate::Spotlight`"
    let _ = camera.exposure().set_spotlight(false);
    //~^ E0277 "camera profile `grafton_visca::profiles::GenericVisca` does not support `grafton_visca::capabilities::gate::Spotlight`"
    let _ = camera.exposure().set_auto_slow_shutter(true);
    //~^ E0277 "camera profile `grafton_visca::profiles::GenericVisca` does not support `grafton_visca::capabilities::gate::AutoSlowShutter`"
    let _ = camera.exposure().set_auto_slow_shutter(false);
    //~^ E0277 "camera profile `grafton_visca::profiles::GenericVisca` does not support `grafton_visca::capabilities::gate::AutoSlowShutter`"
    let _ = camera.streaming().set_multicast(true);
    //~^ E0277 "camera profile `grafton_visca::profiles::GenericVisca` does not support `grafton_visca::capabilities::gate::MulticastStreaming`"
    let _ = camera.streaming().set_multicast(false);
    //~^ E0277 "camera profile `grafton_visca::profiles::GenericVisca` does not support `grafton_visca::capabilities::gate::MulticastStreaming`"
    let _ = camera
        .streaming()
        .set_ndi_quality(grafton_visca::types::NdiQuality::High);
        //~^ E0277 "camera profile `grafton_visca::profiles::GenericVisca` does not support `grafton_visca::capabilities::gate::NdiQuality`"
}
