use grafton_visca::ViscaInquiry;

#[derive(Debug, Clone, Copy, ViscaInquiry)]
#[visca(opcode = 0, response = Power, parser = AutoWbSensitivity)]
struct InvalidInquiry;

fn main() {}

//~ "parser = AutoWbSensitivity is unsupported"
//~ "remove parser to use the canonical inquiry decoder"
