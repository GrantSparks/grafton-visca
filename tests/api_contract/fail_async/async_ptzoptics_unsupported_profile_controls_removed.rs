use grafton_visca::{
    camera::CameraSession, mode::Async, profiles::PtzOpticsG2, types::IrisLevel,
    DigitalZoomControl, DigitalZoomRangeControl, Executor, IrisControl, ZoomDomain,
};

fn main() {
    fn unsupported<Tr, Exec>(session: &CameraSession<Async, PtzOpticsG2, Tr, Exec>)
    where
        Tr: grafton_visca::transport::AsyncTransport + Send + Sync + 'static,
        Exec: Executor + Send + Sync + Clone + 'static,
    {
        let _ = session.set_digital_zoom(true);
        let _ = session.zoom_absolute_normalized(
            grafton_visca::Normalized::new(0.75).unwrap(),
            ZoomDomain::OpticalPlusDigital,
        );
        let _ = session.exposure().iris_priority();
        let _ = session.set_iris(IrisLevel::new(1).unwrap());
        let _ = session.focus().one_push();
    }

    let _ = unsupported::<
        grafton_visca::runtime::TransportHandle<grafton_visca::TokioRuntime>,
        grafton_visca::TokioExecutor,
    >;
}
