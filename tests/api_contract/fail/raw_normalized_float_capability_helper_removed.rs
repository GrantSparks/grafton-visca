use grafton_visca::{camera::profiles::PtzOpticsG2, capabilities::zoom::ZoomExt};

fn main() {
    let _ = PtzOpticsG2.normalized_to_zoom_units(0.5);
}

//~ E0599
//~ "named `normalized_to_zoom_units` found for struct"
