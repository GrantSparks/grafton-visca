use grafton_visca::ViscaInquiry;

#[derive(Debug, Clone, Copy, ViscaInquiry)]
#[visca(opcode = 0x00, response = Power, parser = Bool, parse_with = this::path::does::not::exist)]
struct InvalidInquiry;

fn main() {}

//~ "`parse_with` has no effect with `parser = Bool`"
