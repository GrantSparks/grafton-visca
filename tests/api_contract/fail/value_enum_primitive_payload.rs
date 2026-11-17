#[grafton_visca::value_enum]
enum Unchecked {
    Unknown(u8),
}

fn main() {}

//~ E0599
//~ "__wire_domain_contains"
