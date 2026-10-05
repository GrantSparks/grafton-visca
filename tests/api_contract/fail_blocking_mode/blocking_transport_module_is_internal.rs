// The blocking TCP/UDP transports are reached through the public transport
// builders; the module implementing them is private.
//
// This fixture used to live under `tests/camera_first_contract/fail`, a
// directory no test binary referenced, so it never ran.

fn main() {
    let _ = grafton_visca::transport::blocking::Tcp::connect_with_config(
        "127.0.0.1:5678",
        grafton_visca::transport::TransportConfig::for_tcp(),
    );
}

//~ E0603
//~ "module `blocking` is private"
