#![cfg(feature = "blocking")]

use grafton_visca::{
    blocking::Camera, profiles::GenericVisca, profiles::NearusBrc300, profiles::PtzOptics30x,
    profiles::PtzOpticsG2, profiles::PtzOpticsG3, profiles::SonyBrc300, profiles::SonyBrcH900,
    profiles::SonyEviH100, types::ExposureMode, types::IrisLevel,
};

fn ptzoptics_g2(camera: &Camera<PtzOpticsG2>) {
    let _ = camera.exposure().mode();
    let _ = camera.exposure().set_mode(ExposureMode::Auto);
}

fn ptzoptics_g3(camera: &Camera<PtzOpticsG3>) {
    let _ = camera.exposure().mode();
    let _ = camera.exposure().set_mode(ExposureMode::Auto);
    let _ = camera.exposure().iris();
    let _ = camera.exposure().set_iris(IrisLevel::MIN);
}

fn ptzoptics_30x(camera: &Camera<PtzOptics30x>) {
    let _ = camera.exposure().mode();
    let _ = camera.exposure().set_mode(ExposureMode::Auto);
}

macro_rules! source_backed_sony_exposure {
    ($function:ident, $profile:ty) => {
        fn $function(camera: &Camera<$profile>) {
            let _ = camera.exposure().mode();
            let _ = camera.exposure().set_mode(ExposureMode::Auto);
            let _ = camera.exposure().iris();
            let _ = camera.exposure().set_iris(IrisLevel::MIN);
        }
    };
}

source_backed_sony_exposure!(sony_brch900, SonyBrcH900);
source_backed_sony_exposure!(sony_evih100, SonyEviH100);
source_backed_sony_exposure!(sony_brc300, SonyBrc300);
source_backed_sony_exposure!(nearus_brc300, NearusBrc300);
source_backed_sony_exposure!(generic_visca, GenericVisca);

fn main() {
    let _: fn(&Camera<PtzOpticsG2>) = ptzoptics_g2;
    let _: fn(&Camera<PtzOpticsG3>) = ptzoptics_g3;
    let _: fn(&Camera<PtzOptics30x>) = ptzoptics_30x;
    let _: fn(&Camera<SonyBrcH900>) = sony_brch900;
    let _: fn(&Camera<SonyEviH100>) = sony_evih100;
    let _: fn(&Camera<SonyBrc300>) = sony_brc300;
    let _: fn(&Camera<NearusBrc300>) = nearus_brc300;
    let _: fn(&Camera<GenericVisca>) = generic_visca;
}
