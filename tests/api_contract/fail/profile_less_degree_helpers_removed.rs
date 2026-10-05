// #808: no helper converts angles without a profile's units-per-degree scale,
// and the profile-less position value types are gone.
use grafton_visca::{camera::PanTiltPosition, types::PanPosition};

fn main() {
    let _ = PanTiltPosition::new(0, 0).as_degrees();
}

// rustc may print `PanTiltPosition` path-qualified; each line still pins the
// item and the type.
//~ E0432
//~ E0599
//~ "no `PanPosition` in `types`"
//~ "no method named `as_degrees` found for struct"
//~ "PanTiltPosition` in the current scope"
