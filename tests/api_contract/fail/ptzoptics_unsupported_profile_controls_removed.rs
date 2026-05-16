use grafton_visca::{
    profiles::PtzOpticsG2, transport::BlockingTransportHandle, BlockingCamera, UnitInterval,
    ZoomDomain,
};

fn main() {
    let camera: Option<BlockingCamera<PtzOpticsG2, BlockingTransportHandle>> = None;
    let camera = camera.unwrap();

    let _ = camera.set_digital_zoom(true);
    let _ = camera.zoom_absolute_normalized(
        UnitInterval::new(0.75).unwrap(),
        ZoomDomain::OpticalPlusDigital,
    );
    let _ = camera.focus().one_push();
}
