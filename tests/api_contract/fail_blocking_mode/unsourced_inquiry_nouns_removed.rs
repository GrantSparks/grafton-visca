use grafton_visca::{blocking::Camera, profiles::PtzOpticsG2};

fn unsupported(camera: &Camera<PtzOpticsG2>) {
    let _ = camera.exposure().dynamic_range();
    let _ = camera.focus().range();
    let _ = camera.image().night_day_mode();
    let _ = camera.power().standby_enabled();
    let _ = camera.pan_tilt().digital_ptz_enabled();
    let _ = camera.presets().auto_trace_enabled();
    let _ = camera.focus().focus_unlock();
    let _ = camera.system().broadcast_domain();
    let _ = camera.image().two_tone_mode_enabled();
    let _ = camera.zoom().digital_mode_enabled();
}

fn main() {}

//~ E0599
//~ "no method named `range`"
//~ "no method named `night_day_mode`"
//~ "no method named `standby_enabled`"
//~ "no method named `digital_ptz_enabled`"
//~ "no method named `auto_trace_enabled`"
//~ "no method named `focus_unlock`"
//~ "no method named `broadcast_domain`"
//~ "no method named `two_tone_mode_enabled`"
//~ "no method named `digital_mode_enabled`"
//~ "no method named `dynamic_range`"
