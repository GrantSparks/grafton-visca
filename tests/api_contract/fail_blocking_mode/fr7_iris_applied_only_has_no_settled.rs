use grafton_visca::{blocking::Camera, profiles::SonyFR7, types::IrisStep};

fn invalid(camera: &Camera<SonyFR7>) {
    let mut iris = camera.exposure().iris_up_by(IrisStep::MIN).unwrap();
    let _ = iris.settled();
}

fn main() {}

//~ E0599
//~ "named `settled` found for struct"
