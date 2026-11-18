#[cfg(all(feature = "blocking", not(feature = "async")))]
use grafton_visca::blocking::Camera;
use grafton_visca::profiles::PtzOpticsG2;
#[cfg(feature = "async")]
use grafton_visca::r#async::Camera;

fn withheld(camera: &Camera<PtzOpticsG2>) {
    let _ = camera.focus().infinity();
    //~^ E0277 "camera profile `grafton_visca::profiles::PtzOpticsG2` does not support `grafton_visca::capabilities::gate::FocusInfinity`"
    let _ = camera.menu().set_visible(true);
    //~^ E0277 "camera profile `grafton_visca::profiles::PtzOpticsG2` does not support `grafton_visca::capabilities::gate::MenuDisplay`"
}
fn withheld_h900(camera: &Camera<grafton_visca::profiles::SonyBrcH900>) {
    let _ = camera
        .exposure()
        .set_shutter(grafton_visca::types::ShutterSpeed::new(0).expect("valid value"));
        //~^ E0277 "camera profile `grafton_visca::profiles::SonyBrcH900` does not support `grafton_visca::capabilities::gate::DirectShutter`"
}

fn main() {}
