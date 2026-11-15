use grafton_visca::ViscaInquiry;

#[derive(Debug, Clone, Copy, ViscaInquiry)]
#[visca(opcode = 0, response = Power, parser = SharpnessMode)]
struct InvalidInquiry;

fn main() {}

//~ "parser = SharpnessMode is unsupported"
//~ "remove parser to use the canonical inquiry decoder"
