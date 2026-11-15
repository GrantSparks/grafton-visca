use grafton_visca::transport::{AddressingMode, TransportConfig};

fn main() {
    let mut config = TransportConfig::default();
    config.addressing = AddressingMode::Ip;
}

//~ E0609
//~ "no field `addressing`"
