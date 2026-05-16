use grafton_visca::{types::ZoomPosition, units::Raw};

fn main() {
    let _ = ZoomPosition::from(Raw(0x4000_u16));
}
