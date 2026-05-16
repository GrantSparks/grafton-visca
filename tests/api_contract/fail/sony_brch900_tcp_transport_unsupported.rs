use grafton_visca::{camera::Connect, profiles::SonyBRCH900};

fn main() {
    let _ = Connect::open_tcp_blocking::<SonyBRCH900>("127.0.0.1");
}
