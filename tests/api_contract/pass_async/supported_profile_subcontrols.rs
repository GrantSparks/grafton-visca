use grafton_visca::{
    camera::CameraSession,
    mode::Async,
    profiles::{SonyBRCH900, SonyEVIH100, SonyFR7},
    types::{ColorTemp, IrisLevel},
    ColorTemperatureControl, ColorTemperatureInquiryControl, DigitalZoomControl,
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

fn use_sony_brch900<Tr, Exec>(
    session: &CameraSession<Async, SonyBRCH900, Tr, Exec>,
) -> Result<(), Error>
where
    Tr: grafton_visca::transport::AsyncTransport + Send + Sync + 'static,
    Exec: Executor + Send + Sync + Clone + 'static,
{
    let _ = session.white_balance().color_temperature_mode();
    let _ = session.set_color_temperature(ColorTemp::from_kelvin(5600)?);
    let _ = session.white_balance().color_temperature();
    let _ = session.color_temperature();
    Ok(())
}

fn use_sony_evih100<Tr, Exec>(
    session: &CameraSession<Async, SonyEVIH100, Tr, Exec>,
) -> Result<(), Error>
where
    Tr: grafton_visca::transport::AsyncTransport + Send + Sync + 'static,
    Exec: Executor + Send + Sync + Clone + 'static,
{
    let _ = session.white_balance().color_temperature_mode();
    let _ = session.set_color_temperature(ColorTemp::from_kelvin(5600)?);
    let _ = session.white_balance().color_temperature();
    let _ = session.color_temperature();
    Ok(())
}

fn main() {}
