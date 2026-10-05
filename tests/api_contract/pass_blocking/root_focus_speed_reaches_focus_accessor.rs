#![cfg(feature = "blocking")]

// #806: the root `FocusSpeed` is the type the focus accessor takes.
use grafton_visca::{blocking::Camera, profiles::PtzOpticsG2, FocusSpeed};

fn variable_focus(camera: &Camera<PtzOpticsG2>, speed: FocusSpeed) {
    let _ = camera.focus().far_variable(speed);
    let _ = camera.focus().near_variable(speed);
}

fn main() {
    let _: fn(&Camera<PtzOpticsG2>, FocusSpeed) = variable_focus;
}
