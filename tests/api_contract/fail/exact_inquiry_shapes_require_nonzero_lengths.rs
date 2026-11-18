use grafton_visca::request::PayloadShape;

fn main() {
    let _ = PayloadShape::Bytes(0);
    //~^ E0308 "mismatched types"
    let _ = PayloadShape::Nibbles(0);
    //~^ E0308 "mismatched types"
}
