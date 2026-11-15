use grafton_visca::ViscaInquiry;

#[derive(Debug, Clone, Copy, ViscaInquiry)]
#[visca(opcode = 0x42, response = NdFilter, parser = NdFilter, data_variant = Power)]
struct InvalidInquiry;

fn main() {}

//~ "`data_variant` has no effect with `parser = NdFilter`"
