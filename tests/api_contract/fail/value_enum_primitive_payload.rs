#[grafton_visca::value_enum]
//~^ E0599 "__wire_domain_contains"
enum Unchecked {
    Unknown(u8),
}

fn main() {}
