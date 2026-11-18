//@ build
use grafton_visca::wire::Nibbles;

fn main() {
    let nibbles = Nibbles::<1>::from_array(&[1]).unwrap();
    let _ = nibbles.zero_extended_u8();
    //~^ E0080 "evaluation panicked: zero-extended reply width is too small"
}
