#[cfg(all(feature = "blocking", not(feature = "async")))]
use grafton_visca::blocking::Camera;
use grafton_visca::profiles::PtzOpticsG2;
#[cfg(feature = "async")]
use grafton_visca::r#async::Camera;

fn withheld(camera: &Camera<PtzOpticsG2>) {
    let _ = camera.focus().infinity();
    let _ = camera.menu().set_visible(true);
}
fn withheld_h900(camera: &Camera<grafton_visca::profiles::SonyBrcH900>) {
    let _ = camera
        .exposure()
        .set_shutter(grafton_visca::types::ShutterSpeed::new(0).expect("valid value"));
}

fn main() {}

//~ E0277
//~ "HasFocusInfinity"
//~ "HasMenuDisplay"
//~ "HasDirectShutter"
