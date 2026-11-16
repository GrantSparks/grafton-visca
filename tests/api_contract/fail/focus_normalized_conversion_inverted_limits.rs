//@ build
use grafton_visca::capabilities::CapabilityRange;
const INVERTED_FOCUS_RANGE: CapabilityRange<u16> = CapabilityRange::<u16>::new(2, 1);
fn main() {
    let _ = INVERTED_FOCUS_RANGE;
}

//~ E0080
//~ "capability range minimum exceeds maximum"
