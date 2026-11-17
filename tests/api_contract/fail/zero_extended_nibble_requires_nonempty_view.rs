//@ build
use grafton_visca::wire::Nibbles;

fn main() {
    let nibbles = Nibbles::<0>::from_array(&[]).unwrap();
    let _ = nibbles.zero_extended_nibble();
}

//~ E0080
//~ "evaluation panicked: zero-extended reply width is too small"
