use grafton_visca::{Inquiry, PayloadShape};

#[allow(dead_code)]
struct None;
#[allow(dead_code)]
struct Some;

#[derive(grafton_visca::ViscaInquiry)]
#[visca(opcode = 0x00, response = Power)]
struct Power;

fn main() {
    assert_eq!(Power.payload_shape(), PayloadShape::Bytes(1));
}
