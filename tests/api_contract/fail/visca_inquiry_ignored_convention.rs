use grafton_visca::ViscaInquiry;

#[derive(Debug, Clone, Copy, ViscaInquiry)]
#[visca(opcode = 0x00, response = Power, parser = Bool, convention = OnIs03)]
struct InvalidInquiry;

fn main() {}

//~ "`convention` has no effect with `parser = Bool`"
