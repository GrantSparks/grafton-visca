use grafton_visca::ViscaInquiry;

#[derive(Debug, Clone, Copy, ViscaInquiry)]
#[visca(opcode = 0x00, response = Power, typed_is_tuple)]
struct IgnoredBuiltinTuple;

#[derive(Debug, Clone, Copy, ViscaInquiry)]
#[visca(opcode = 0x00, response = Raw, typed_is_tuple)]
struct IgnoredRawTuple;

fn main() {}

//~ "typed_is_tuple requires typed_response"
