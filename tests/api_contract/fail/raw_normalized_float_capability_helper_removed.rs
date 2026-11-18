use grafton_visca::profiles::PtzOpticsG2;
fn main() {
    let _ = PtzOpticsG2.normalized_to_zoom_units(0.5);
    //~^ E0599 "named `normalized_to_zoom_units` found for struct"
}
