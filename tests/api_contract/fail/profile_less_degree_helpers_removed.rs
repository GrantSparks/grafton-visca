// #808: no helper converts angles without a profile's units-per-degree scale,
// and the profile-less position value types are gone.
use grafton_visca::{types::PanPosition, types::PanTiltPosition};

fn main() {
    let _ = PanTiltPosition::from_degrees(0.0, 0.0)
        .unwrap()
        .as_degrees();
}

// rustc may print `PanTiltPosition` path-qualified; each line still pins the
// item and the type.
//~ E0432
//~ E0599
//~ "no `PanPosition` in `types`"
//~ "no method named `as_degrees` found for struct"
//~ "PanTiltPosition` in the current scope"
