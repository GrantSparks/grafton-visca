use grafton_visca::{
    capabilities::{
        HasBrightnessControl, HasColorTemperature, HasContrastControl, HasMotionSync, HasNdFilter,
        HasSharpnessControl, HasTally, HasVariableSpeed,
    },
    profiles::{GenericVisca, PtzOptics30X, PtzOpticsG2, PtzOpticsG3, SonyFR7},
};

fn requires_color_temperature<P: HasColorTemperature>() {}
fn requires_nd<P: HasNdFilter>() {}
fn requires_tally<P: HasTally>() {}
fn requires_motion_sync<P: HasMotionSync>() {}
fn requires_variable_speed<P: HasVariableSpeed>() {}
fn requires_brightness<P: HasBrightnessControl>() {}
fn requires_contrast<P: HasContrastControl>() {}
fn requires_sharpness<P: HasSharpnessControl>() {}

fn main() {
    requires_color_temperature::<SonyFR7>();
    requires_nd::<PtzOpticsG2>();
    requires_nd::<GenericVisca>();
    requires_tally::<PtzOpticsG2>();
    requires_tally::<GenericVisca>();
    requires_variable_speed::<PtzOpticsG2>();
    requires_variable_speed::<GenericVisca>();
    requires_motion_sync::<PtzOpticsG2>();
    requires_motion_sync::<PtzOpticsG3>();
    requires_motion_sync::<PtzOptics30X>();
    requires_motion_sync::<GenericVisca>();
    requires_motion_sync::<SonyFR7>();
    requires_brightness::<GenericVisca>();
    requires_brightness::<SonyEVIH100>();
    requires_brightness::<SonyBRC300>();
    requires_contrast::<GenericVisca>();
    requires_contrast::<SonyEVIH100>();
    requires_contrast::<SonyBRC300>();
    requires_sharpness::<GenericVisca>();
    requires_sharpness::<SonyEVIH100>();
    requires_sharpness::<SonyBRC300>();
}
