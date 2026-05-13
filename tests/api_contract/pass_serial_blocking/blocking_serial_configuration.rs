use grafton_visca::{
    camera::CameraConfig,
    profiles::PtzOpticsG2,
    transport::{BlockingTransportHandle, TransportConfig},
    Error,
};

fn serial_blocking_contract() -> Result<(), Error> {
    let camera = CameraConfig::<PtzOpticsG2>::new()
        .serial("/dev/ttyUSB0", 9600)
        .transport_config(TransportConfig::default())
        .open_serial_blocking()?;

    let _: grafton_visca::BlockingCamera<PtzOpticsG2, BlockingTransportHandle> = camera;
    Ok(())
}

fn main() {
    let _ = serial_blocking_contract;
}
