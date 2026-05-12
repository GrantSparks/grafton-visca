use grafton_visca::{
    camera::{CameraConfig, Connect, ConnectBuilder},
    profiles::PtzOpticsG2,
    transport::{RetryConfig, TransportConfig},
};

fn main() {
    let builder: ConnectBuilder = Connect::builder()
        .tcp("127.0.0.1")
        .with_default_port()
        .udp("127.0.0.1:1259");
    let _ = format!("{builder:?}");

    let config = CameraConfig::<PtzOpticsG2>::new()
        .tcp()
        .address("127.0.0.1")
        .timeouts(Default::default())
        .retry_config(RetryConfig::default())
        .transport_config(TransportConfig::default())
        .camera_id(1)
        .unwrap();
    let _ = config.clone();
}
