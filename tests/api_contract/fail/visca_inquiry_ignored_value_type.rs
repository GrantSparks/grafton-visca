use grafton_visca::ViscaInquiry;

#[derive(Debug, Clone, Copy, ViscaInquiry)]
#[visca(opcode = 0x00, response = Power, parser = Bool, value_type = bool)]
struct InvalidInquiry;

fn main() {}

//~ "`value_type` has no effect with `parser = Bool`"
