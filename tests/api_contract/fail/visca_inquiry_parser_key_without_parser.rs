use grafton_visca::ViscaInquiry;

#[derive(Debug, Clone, Copy, ViscaInquiry)]
#[visca(opcode = 0x00, response = Power, parse_with = custom)]
struct InvalidInquiry;

fn main() {}

//~ "parse_with requires parser = Custom"
