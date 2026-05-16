use grafton_visca::{
    camera::AsyncCamera, profiles::PtzOpticsG2, runtime::TransportHandle, UnitInterval,
};

fn main() {
    let camera: Option<
        AsyncCamera<
            PtzOpticsG2,
            TransportHandle<grafton_visca::TokioRuntime>,
            grafton_visca::TokioExecutor,
        >,
    > = None;
    let camera = camera.unwrap();

    let _ = camera.set_zoom_op(UnitInterval::new(0.5).unwrap());
}
