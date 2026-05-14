use grafton_visca::{
    camera::CameraSession, mode::Async, profiles::SonyFR7, types::IrisLevel, DigitalZoomControl,
    DigitalZoomRangeControl, Error, Executor, IrisControl, ZoomDomain,
};

fn use_sony_fr7<Tr, Exec>(session: &CameraSession<Async, SonyFR7, Tr, Exec>) -> Result<(), Error>
where
    Tr: grafton_visca::transport::AsyncTransport + Send + Sync + 'static,
    Exec: Executor + Send + Sync + Clone + 'static,
{
    let normalized = grafton_visca::Normalized::new(0.75)?;
    let iris = IrisLevel::new(1)?;

    let _ = session.set_digital_zoom(true);
    let _ = session.zoom_absolute_normalized(normalized, ZoomDomain::OpticalPlusDigital);
    let _ = session.exposure().iris_priority();
    let _ = session.set_iris(iris);
    let _ = session.exposure().iris();
    Ok(())
}

fn main() {}
