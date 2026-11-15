#[cfg(feature = "async")]
use grafton_visca::Camera;
#[cfg(all(feature = "blocking", not(feature = "async")))]
use grafton_visca::blocking::Camera;
use grafton_visca::profiles::PtzOpticsG2;

fn withheld(camera: &Camera<PtzOpticsG2>) {
    let _ = camera.focus().infinity();
    let _ = camera.menu().display(true);
}
fn withheld_h900(camera: &Camera<grafton_visca::profiles::SonyBRCH900>) {
    let _ = camera.exposure().shutter_direct(grafton_visca::types::ShutterSpeed::new(0));
}

fn main() {}

//~ E0277
//~ "HasFocusInfinity"
//~ "HasMenuDisplay"
//~ "HasShutterDirect"
