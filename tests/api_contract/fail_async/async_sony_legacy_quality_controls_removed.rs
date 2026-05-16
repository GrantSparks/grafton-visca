use grafton_visca::{
    camera::CameraSession,
    mode::Async,
    profiles::{SonyBRC300, SonyEVIH100},
    types::{BrightnessLevel, ContrastLevel, SharpnessLevel},
    Executor,
};

fn unsupported_brc300<Tr, Exec>(session: &CameraSession<Async, SonyBRC300, Tr, Exec>)
where
    Tr: grafton_visca::transport::AsyncTransport + Send + Sync + 'static,
    Exec: Executor + Send + Sync + Clone + 'static,
{
    let _ = session
        .exposure()
        .set_brightness(BrightnessLevel::new(1).unwrap());
    let _ = session.exposure().brightness();
    let _ = session.image().set_contrast(ContrastLevel::new(1).unwrap());
    let _ = session.image().contrast();
    let _ = session
        .image()
        .set_sharpness(SharpnessLevel::new(1).unwrap());
    let _ = session.image().sharpness_level();
}

fn unsupported_evih100<Tr, Exec>(session: &CameraSession<Async, SonyEVIH100, Tr, Exec>)
where
    Tr: grafton_visca::transport::AsyncTransport + Send + Sync + 'static,
    Exec: Executor + Send + Sync + Clone + 'static,
{
    let _ = session
        .exposure()
        .set_brightness(BrightnessLevel::new(1).unwrap());
    let _ = session.exposure().brightness();
    let _ = session.image().set_contrast(ContrastLevel::new(1).unwrap());
    let _ = session.image().contrast();
    let _ = session
        .image()
        .set_sharpness(SharpnessLevel::new(1).unwrap());
    let _ = session.image().sharpness_level();
}

fn main() {}
