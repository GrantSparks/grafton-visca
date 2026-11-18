#![cfg(feature = "async")]

use grafton_visca::{
    profiles::PtzOpticsG2, profiles::SonyBrcH900, profiles::SonyFr7, r#async::Camera,
};

fn main() {
    let image: Option<Camera<PtzOpticsG2>> = None;
    let image = image.as_ref().unwrap();
    let _ = image.image().set_freeze(true);
    //~^ E0277 "camera profile `grafton_visca::profiles::PtzOpticsG2` does not support `grafton_visca::capabilities::gate::ImageFreeze`"
    let _ = image.image().set_freeze(false);
    //~^ E0277 "camera profile `grafton_visca::profiles::PtzOpticsG2` does not support `grafton_visca::capabilities::gate::ImageFreeze`"

    let tally: Option<Camera<SonyFr7>> = None;
    let tally = tally.as_ref().unwrap();
    let _ = tally
        .tally()
        .set_brightness(grafton_visca::types::TallyBrightness::Low);
        //~^ E0277 "camera profile `grafton_visca::profiles::SonyFr7` does not support `grafton_visca::capabilities::gate::TallyBrightness`"
    let _ = tally
        .tally()
        .set_brightness(grafton_visca::types::TallyBrightness::High);
        //~^ E0277 "camera profile `grafton_visca::profiles::SonyFr7` does not support `grafton_visca::capabilities::gate::TallyBrightness`"
    let _ = tally
        .tally()
        .set_mode(grafton_visca::types::TallyMode::Flash);
        //~^ E0277 "camera profile `grafton_visca::profiles::SonyFr7` does not support `grafton_visca::capabilities::gate::TallyMode`"
    let _ = tally.tally().set_mode(grafton_visca::types::TallyMode::On);
    //~^ E0277 "camera profile `grafton_visca::profiles::SonyFr7` does not support `grafton_visca::capabilities::gate::TallyMode`"
    let _ = tally.tally().set_mode(grafton_visca::types::TallyMode::Off);
    //~^ E0277 "camera profile `grafton_visca::profiles::SonyFr7` does not support `grafton_visca::capabilities::gate::TallyMode`"

    let brc_h900: Option<Camera<SonyBrcH900>> = None;
    let brc_h900 = brc_h900.as_ref().unwrap();
    let _ = brc_h900.tally().set_red(true);
    //~^ E0277 "camera profile `grafton_visca::profiles::SonyBrcH900` does not support `grafton_visca::capabilities::gate::Tally`"
    //~^^ E0599 "the method `set_red` exists"
    let _ = brc_h900.tally().set_red(false);
    //~^ E0277 "camera profile `grafton_visca::profiles::SonyBrcH900` does not support `grafton_visca::capabilities::gate::Tally`"
    //~^^ E0599 "the method `set_red` exists"
    let _ = brc_h900.tally().set_green(true);
    //~^ E0277 "camera profile `grafton_visca::profiles::SonyBrcH900` does not support `grafton_visca::capabilities::gate::Tally`"
    //~^^ E0599 "the method `set_green` exists"
    let _ = brc_h900.tally().set_green(false);
    //~^ E0277 "camera profile `grafton_visca::profiles::SonyBrcH900` does not support `grafton_visca::capabilities::gate::Tally`"
    //~^^ E0599 "the method `set_green` exists"
}
