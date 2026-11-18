use grafton_visca::ViscaInquiry;

#[derive(Debug, Clone, Copy, ViscaInquiry)]
#[visca(opcode = 0, response = Power, parser = Mode)]
//~^ "remove parser to use the canonical inquiry decoder"
struct InvalidInquiry;

fn main() {}
