use grafton_visca::ViscaInquiry;

#[derive(Debug, Clone, Copy, ViscaInquiry)]
#[visca(opcode = 0x00, response = Power, parser = Bool, field = on)]
struct InvalidInquiry;

fn main() {}

//~ "`field` has no effect with `parser = Bool`"
