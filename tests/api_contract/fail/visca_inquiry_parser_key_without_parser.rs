use grafton_visca::ViscaInquiry;

#[derive(Debug, Clone, Copy, ViscaInquiry)]
#[visca(opcode = 0x00, response = Power, parse_with = custom)]
//~^ "parse_with requires parser = Custom"
struct InvalidInquiry;

fn main() {}
