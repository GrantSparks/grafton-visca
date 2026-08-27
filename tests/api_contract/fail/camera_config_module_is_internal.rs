fn main() {
    let _ = grafton_visca::camera::config::TransportOptions::tcp("127.0.0.1");
}

//~ E0603
//~ "module `config` is private"
