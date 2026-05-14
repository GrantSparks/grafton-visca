use grafton_visca::{
    capabilities::{HasMotionSync, HasNdFilter, HasVariableSpeed},
    profiles::{GenericVisca, PtzOptics30X, PtzOpticsG2, PtzOpticsG3, SonyFR7},
};

fn requires_nd<P: HasNdFilter>() {}
fn requires_motion_sync<P: HasMotionSync>() {}
fn requires_variable_speed<P: HasVariableSpeed>() {}

fn main() {
    requires_nd::<PtzOpticsG2>();
    requires_nd::<GenericVisca>();
    requires_variable_speed::<PtzOpticsG2>();
    requires_variable_speed::<GenericVisca>();
    requires_motion_sync::<PtzOpticsG2>();
    requires_motion_sync::<PtzOpticsG3>();
    requires_motion_sync::<PtzOptics30X>();
    requires_motion_sync::<GenericVisca>();
    requires_motion_sync::<SonyFR7>();
}
