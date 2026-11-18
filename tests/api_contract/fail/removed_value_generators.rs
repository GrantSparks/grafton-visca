use grafton_visca::ViscaEnum;
//~^ E0432 "unresolved import `grafton_visca::ViscaEnum`"
use grafton_visca::ViscaValue;
//~^ E0432 "unresolved import `grafton_visca::ViscaValue`"

grafton_visca::visca_range_type! { OldRange: u8 { min: 0, max: 1 } }
//~^ E0433 "cannot find `visca_range_type` in `grafton_visca`"

fn main() {}
