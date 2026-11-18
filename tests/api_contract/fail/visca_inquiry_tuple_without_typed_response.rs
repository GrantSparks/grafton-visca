use grafton_visca::ViscaInquiry;

#[derive(Debug, Clone, Copy, ViscaInquiry)]
#[visca(opcode = 0x00, response = Power, typed_is_tuple)]
//~^ "typed_is_tuple requires typed_response"
struct IgnoredBuiltinTuple;

#[derive(Debug, Clone, Copy, ViscaInquiry)]
#[visca(opcode = 0x00, response = Raw, typed_is_tuple)]
//~^ "typed_is_tuple requires typed_response"
struct IgnoredRawTuple;

fn main() {}
