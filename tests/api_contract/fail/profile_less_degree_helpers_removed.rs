// #808: no helper converts angles without a profile's units-per-degree scale,
// and the profile-less position value types are gone.
use grafton_visca::{types::PanPosition, types::PanTiltPosition};
//~^ E0432 "unresolved import `grafton_visca::types::PanPosition`"

fn main() {
    let _ = PanTiltPosition::from_degrees(0.0, 0.0)
        .unwrap()
        .as_degrees();
        //~^ E0599 "no method named `as_degrees` found for struct"
}

// rustc may print `PanTiltPosition` path-qualified; each line still pins the
// item and the type.
