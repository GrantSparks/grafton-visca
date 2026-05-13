fn main() {
    let _ = grafton_visca::BlockingCamera::<
        grafton_visca::profiles::PtzOpticsG2,
        grafton_visca::transport::BlockingTransportHandle,
    >::open_tcp("127.0.0.1:5678");
}
