use grafton_visca::{
    capabilities::{
        HasAutoFocusSensitivity, HasBrightnessControl, HasFocusZone, HasIrisControl,
        HasNoiseReduction, HasNoiseReduction2D, HasNoiseReduction3D, HasPictureEffect,
    },
    profiles::SonyFR7,
};

fn requires_iris<P: HasIrisControl>() {}
fn requires_brightness<P: HasBrightnessControl>() {}
fn requires_focus_zone<P: HasFocusZone>() {}
fn requires_af_sensitivity<P: HasAutoFocusSensitivity>() {}
fn requires_noise_reduction<P: HasNoiseReduction>() {}
fn requires_noise_reduction_2d<P: HasNoiseReduction2D>() {}
fn requires_noise_reduction_3d<P: HasNoiseReduction3D>() {}
fn requires_picture_effect<P: HasPictureEffect>() {}

fn main() {
    requires_iris::<SonyFR7>();
    requires_brightness::<SonyFR7>();
    requires_focus_zone::<SonyFR7>();
    requires_af_sensitivity::<SonyFR7>();
    requires_noise_reduction::<SonyFR7>();
    requires_noise_reduction_2d::<SonyFR7>();
    requires_noise_reduction_3d::<SonyFR7>();
    requires_picture_effect::<SonyFR7>();
}
