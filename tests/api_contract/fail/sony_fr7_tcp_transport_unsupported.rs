use grafton_visca::{camera::Connect, profiles::SonyFR7};

fn main() {
    let _ = Connect::open_tcp_blocking::<SonyFR7>("127.0.0.1");
}
