//@ build
use grafton_visca::{
    capabilities::{focus::FocusExt, Focus},
    UnitInterval,
};

struct Inverted;
impl Focus for Inverted {
    const SUPPORTS_FOCUS_NEAR_LIMIT_INQUIRY: bool = true;

    const MAX_FOCUS_SPEED: u8 = 7;

    const FOCUS_RANGE: grafton_visca::capabilities::CapabilityRange<u16> =
        grafton_visca::capabilities::CapabilityRange::<u16>::new(2, 1);
    const FOCUS_NEAR_END: grafton_visca::capabilities::FocusNearEnd =
        grafton_visca::capabilities::FocusNearEnd::Minimum;
    const SUPPORTS_AUTO_FOCUS: bool = false;
    const SUPPORTS_ONE_PUSH_FOCUS: bool = false;
}
fn main() {
    let _ = Inverted.normalized_to_focus_units(UnitInterval::MAX);
}

//~ E0080
//~ "capability range minimum exceeds maximum"
