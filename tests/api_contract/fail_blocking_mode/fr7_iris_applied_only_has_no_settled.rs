use grafton_visca::{blocking::Camera, profiles::SonyFr7, types::IrisStep};

fn invalid(camera: &Camera<SonyFr7>) {
    let mut iris = camera.exposure().iris_up_by(IrisStep::MIN).unwrap();
    let _ = iris.settled();
    //~^ E0599 "named `settled` found for struct"
}

fn main() {}
