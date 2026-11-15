use grafton_visca::ViscaInquiry;

#[derive(Debug, Clone, Copy, ViscaInquiry)]
#[visca(opcode = 0x00, response = Power, parser = Custom)]
struct InvalidInquiry;

fn main() {}

//~ "custom parser requires parse_with"
