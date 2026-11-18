//@ build
use grafton_visca::capabilities::CapabilityRange;
fn main() {
    let _ = CapabilityRange::<u16> { min: 2, max: 1 };
    //~^ E0451 "private"
}
