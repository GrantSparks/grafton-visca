fn main() {
    let _ =
        grafton_visca::Camera::open_tcp::<grafton_visca::profiles::PtzOpticsG2>("127.0.0.1:5678");
}
