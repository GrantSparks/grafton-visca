use grafton_visca::{
    profiles::PtzOpticsG2, transport::BlockingTransportHandle, types::IrisLevel, BlockingCamera,
    Normalized, ZoomDomain,
};

fn main() {
    let camera: Option<BlockingCamera<PtzOpticsG2, BlockingTransportHandle>> = None;
    let camera = camera.unwrap();

    let _ = camera.set_digital_zoom(true);
    let _ = camera.zoom_absolute_normalized(
        Normalized::new(0.75).unwrap(),
        ZoomDomain::OpticalPlusDigital,
    );
    let _ = camera.exposure().iris_priority();
    let _ = camera.exposure().iris();
    let _ = camera.set_iris(IrisLevel::new(1).unwrap());
    let _ = camera.focus().one_push();
}
