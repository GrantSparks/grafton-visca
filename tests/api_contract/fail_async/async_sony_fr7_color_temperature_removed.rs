use grafton_visca::{
    camera::CameraSession, mode::Async, profiles::SonyFR7, types::ColorTemp,
    ColorTemperatureControl, ColorTemperatureInquiryControl, Executor,
};

fn main() {
    fn unsupported<Tr, Exec>(session: &CameraSession<Async, SonyFR7, Tr, Exec>)
    where
        Tr: grafton_visca::transport::AsyncTransport + Send + Sync + 'static,
        Exec: Executor + Send + Sync + Clone + 'static,
    {
        let _ = session.white_balance().color_temperature_mode();
        let _ = session.set_color_temperature(ColorTemp::from_kelvin(5600).unwrap());
        let _ = session.white_balance().color_temperature();
        let _ = session.color_temperature();
    }

    let _ = unsupported::<
        grafton_visca::runtime::TransportHandle<grafton_visca::TokioRuntime>,
        grafton_visca::TokioExecutor,
    >;
}
