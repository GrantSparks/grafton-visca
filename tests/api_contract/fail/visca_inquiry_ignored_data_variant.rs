use grafton_visca::ViscaInquiry;

#[derive(Debug, Clone, Copy, ViscaInquiry)]
#[visca(opcode = 0x00, response = Power, parser = Bool, data_variant = Digital)]
struct InvalidInquiry;

fn main() {}

//~ "`data_variant` has no effect with `parser = Bool`"
