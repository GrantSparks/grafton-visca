#![cfg(feature = "async")]

use grafton_visca::{
    command::ExposureMode,
    profiles::{
        GenericVisca, NearusBRC300, PtzOptics30X, PtzOpticsG2, PtzOpticsG3, SonyBRC300,
        SonyBRCH900, SonyEVIH100,
    },
    types::IrisLevel,
    Camera,
};

fn ptzoptics_g2(camera: &Camera<PtzOpticsG2>) {
    let _ = camera.exposure().mode();
    let _ = camera.exposure().set_mode(ExposureMode::Auto);
}

fn ptzoptics_g3(camera: &Camera<PtzOpticsG3>) {
    let _ = camera.exposure().mode();
    let _ = camera.exposure().set_mode(ExposureMode::Auto);
    let _ = camera.exposure().iris();
    let _ = camera.exposure().iris_direct(IrisLevel::MIN);
}

fn ptzoptics_30x(camera: &Camera<PtzOptics30X>) {
    let _ = camera.exposure().mode();
    let _ = camera.exposure().set_mode(ExposureMode::Auto);
}

macro_rules! source_backed_sony_exposure {
    ($function:ident, $profile:ty) => {
        fn $function(camera: &Camera<$profile>) {
            let _ = camera.exposure().mode();
            let _ = camera.exposure().set_mode(ExposureMode::Auto);
            let _ = camera.exposure().iris();
            let _ = camera.exposure().iris_direct(IrisLevel::MIN);
        }
    };
}

source_backed_sony_exposure!(sony_brch900, SonyBRCH900);
source_backed_sony_exposure!(sony_evih100, SonyEVIH100);
source_backed_sony_exposure!(sony_brc300, SonyBRC300);
source_backed_sony_exposure!(nearus_brc300, NearusBRC300);
source_backed_sony_exposure!(generic_visca, GenericVisca);

fn main() {
    let _: fn(&Camera<PtzOpticsG2>) = ptzoptics_g2;
    let _: fn(&Camera<PtzOpticsG3>) = ptzoptics_g3;
    let _: fn(&Camera<PtzOptics30X>) = ptzoptics_30x;
    let _: fn(&Camera<SonyBRCH900>) = sony_brch900;
    let _: fn(&Camera<SonyEVIH100>) = sony_evih100;
    let _: fn(&Camera<SonyBRC300>) = sony_brc300;
    let _: fn(&Camera<NearusBRC300>) = nearus_brc300;
    let _: fn(&Camera<GenericVisca>) = generic_visca;
}
