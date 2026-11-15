use grafton_visca::{blocking::Camera, profiles::PtzOpticsG2};

fn unsupported(camera: &Camera<PtzOpticsG2>) {
    let _ = camera.exposure().dynamic_range();
    let _ = camera.focus().range();
    let _ = camera.advanced().night_day_mode();
    let _ = camera.advanced().standby_enabled();
    let _ = camera.advanced().digital_ptz_enabled();
    let _ = camera.advanced().auto_trace_enabled();
    let _ = camera.advanced().focus_unlock();
    let _ = camera.advanced().broadcast_domain();
    let _ = camera.advanced().two_tone_mode_enabled();
    let _ = camera.advanced().digital_mode_enabled();
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
