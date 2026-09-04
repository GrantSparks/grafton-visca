use grafton_visca::{
    capabilities::{
        HasBrightnessControl, HasColorTemperature, HasContrastControl, HasExposureMode,
        HasFocusZone, HasFocusZoneInquiry, HasIrisControl, HasIrisControlInquiry, HasMotionSync,
        HasNdFilter, HasNoiseReduction2D, HasNoiseReduction2DControl, HasNoiseReduction3D,
        HasNoiseReduction3DControl, HasPictureEffect, HasSharpnessControl, HasSonyAutoSlowShutter,
        HasSonySpotlight, HasTally, HasUsbAudio, HasVariableSpeed,
    },
    profiles::{
        GenericVisca, NearusBRC300, PtzOptics30X, PtzOpticsG2, PtzOpticsG3, SonyBRC300,
        SonyBRCH900, SonyEVIH100, SonyFR7,
    },
};

fn requires_color_temperature<P: HasColorTemperature>() {}
fn requires_nd<P: HasNdFilter>() {}
fn requires_tally<P: HasTally>() {}
fn requires_motion_sync<P: HasMotionSync>() {}
fn requires_variable_speed<P: HasVariableSpeed>() {}
fn requires_brightness<P: HasBrightnessControl>() {}
fn requires_exposure_mode<P: HasExposureMode>() {}
fn requires_iris<P: HasIrisControl>() {}
fn requires_iris_control_inquiry<P: HasIrisControlInquiry>() {}
fn requires_nr_2d<P: HasNoiseReduction2D>() {}
fn requires_nr_3d<P: HasNoiseReduction3D>() {}
fn requires_nr_2d_control<P: HasNoiseReduction2DControl>() {}
fn requires_nr_3d_control<P: HasNoiseReduction3DControl>() {}
fn requires_contrast<P: HasContrastControl>() {}
fn requires_sharpness<P: HasSharpnessControl>() {}
fn requires_focus_zone<P: HasFocusZone>() {}
fn requires_focus_zone_inquiry<P: HasFocusZoneInquiry>() {}
fn requires_picture_effect<P: HasPictureEffect>() {}
fn requires_usb_audio<P: HasUsbAudio>() {}
fn requires_spotlight<P: HasSonySpotlight>() {}
fn requires_auto_slow_shutter<P: HasSonyAutoSlowShutter>() {}

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
    requires_brightness::<SonyFR7>();
    requires_brightness::<SonyBRCH900>();
    requires_exposure_mode::<SonyFR7>();
    requires_iris::<SonyFR7>();
    requires_iris_control_inquiry::<PtzOpticsG3>();
    requires_nr_2d::<SonyFR7>();
    requires_nr_3d::<SonyFR7>();
    requires_nr_2d_control::<SonyFR7>();
    requires_nr_3d_control::<SonyFR7>();
    requires_focus_zone::<SonyFR7>();
    requires_focus_zone_inquiry::<SonyFR7>();
    requires_focus_zone_inquiry::<PtzOpticsG3>();
    requires_picture_effect::<SonyFR7>();
    requires_picture_effect::<SonyBRCH900>();
    requires_usb_audio::<PtzOpticsG3>();
    requires_contrast::<GenericVisca>();
    requires_contrast::<SonyEVIH100>();
    requires_contrast::<SonyBRC300>();
    requires_sharpness::<GenericVisca>();
    requires_sharpness::<SonyEVIH100>();
    requires_sharpness::<SonyBRC300>();
    requires_auto_slow_shutter::<SonyFR7>();
    requires_auto_slow_shutter::<SonyBRCH900>();
    requires_spotlight::<SonyEVIH100>();
    requires_spotlight::<SonyBRC300>();
    requires_spotlight::<NearusBRC300>();
    requires_auto_slow_shutter::<NearusBRC300>();
}

// Keep every profile named below imported. This fixture pins unsatisfied
// capability bounds, so an unresolved profile name would test only a typo.

//~ E0277
//~ "profile `SonyEVIH100` does not declare exposure brightness support"
//~ "profile `SonyBRC300` does not declare contrast control support"
//~ "profile `SonyEVIH100` does not declare sharpness control support"
//~ "profile `SonyFR7` does not declare exposure brightness support"
//~ "profile `SonyBRCH900` does not declare exposure brightness support"
//~ "profile `SonyFR7` does not declare shared exposure-mode support"
//~ "profile `SonyFR7` does not declare iris control support"
//~ "profile `grafton_visca::profiles::PtzOpticsG3` does not declare iris control-status inquiry support"
//~ "profile `SonyFR7` does not declare 2D noise-reduction inquiry support"
//~ "profile `SonyFR7` does not declare 3D noise-reduction inquiry support"
//~ "profile `SonyFR7` does not declare 2D noise-reduction control support"
//~ "profile `SonyFR7` does not declare 3D noise-reduction control support"
//~ "profile `SonyFR7` does not declare focus-zone support"
//~ "profile `SonyFR7` does not declare focus-zone inquiry support"
//~ "profile `grafton_visca::profiles::PtzOpticsG3` does not declare focus-zone inquiry support"
//~ "profile `SonyFR7` does not declare picture-effect support"
//~ "profile `SonyBRCH900` does not declare picture-effect support"
//~ "profile `grafton_visca::profiles::PtzOpticsG3` does not declare USB audio support"
//~ "profile `SonyFR7` does not declare Sony auto slow-shutter support"
//~ "profile `SonyBRCH900` does not declare Sony auto slow-shutter support"
//~ "profile `SonyEVIH100` does not declare Sony spotlight support"
//~ "profile `SonyBRC300` does not declare Sony spotlight support"
//~ "profile `NearusBRC300` does not declare Sony spotlight support"
//~ "profile `NearusBRC300` does not declare Sony auto slow-shutter support"
