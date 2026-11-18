use grafton_visca::ViscaInquiry;

#[derive(Debug, Clone, Copy, ViscaInquiry)]
#[visca(opcode = 0x00, response = Power, parser = Custom)]
//~^ "custom parser requires parse_with"
struct InvalidInquiry;

fn main() {}
