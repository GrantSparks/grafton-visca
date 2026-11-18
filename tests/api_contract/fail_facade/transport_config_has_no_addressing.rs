use grafton_visca::{transport::AddressingMode, transport::IoBounds};

fn main() {
    let mut config = IoBounds::default();
    config.addressing = AddressingMode::Ip;
    //~^ E0609 "no field `addressing`"
}
