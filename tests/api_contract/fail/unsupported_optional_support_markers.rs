use grafton_visca::{
    capabilities::{
        HasBrightnessControl, HasColorTemperature, HasContrastControl, HasFocusZone,
        HasFocusZoneInquiry, HasMotionSync, HasNdFilter, HasPictureEffect, HasSharpnessControl,
        HasTally, HasUsbAudio, HasVariableSpeed,
    },
    profiles::{
        GenericVisca, PtzOptics30X, PtzOpticsG2, PtzOpticsG3, SonyBRC300, SonyBRCH900, SonyEVIH100,
        SonyFR7,
    },
};

fn requires_color_temperature<P: HasColorTemperature>() {}
fn requires_nd<P: HasNdFilter>() {}
fn requires_tally<P: HasTally>() {}
fn requires_motion_sync<P: HasMotionSync>() {}
fn requires_variable_speed<P: HasVariableSpeed>() {}
fn requires_brightness<P: HasBrightnessControl>() {}
fn requires_contrast<P: HasContrastControl>() {}
fn requires_sharpness<P: HasSharpnessControl>() {}
fn requires_focus_zone<P: HasFocusZone>() {}
fn requires_focus_zone_inquiry<P: HasFocusZoneInquiry>() {}
fn requires_picture_effect<P: HasPictureEffect>() {}
fn requires_usb_audio<P: HasUsbAudio>() {}

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
}

// `SonyEVIH100` and `SonyBRC300` were named here without being imported, so
// six of these twenty-one lines were rejected as unresolved names (E0425)
// rather than as unsatisfied capability bounds. The imports are now present
// and every line fails on the bound it is here to pin.

//~ E0277
//~ "profile `SonyEVIH100` does not declare exposure brightness support"
//~ "profile `SonyBRC300` does not declare contrast control support"
//~ "profile `SonyEVIH100` does not declare sharpness control support"
//~ "profile `SonyFR7` does not declare exposure brightness support"
//~ "profile `SonyBRCH900` does not declare exposure brightness support"
//~ "profile `SonyFR7` does not declare focus-zone support"
//~ "profile `SonyFR7` does not declare focus-zone inquiry support"
//~ "profile `grafton_visca::profiles::PtzOpticsG3` does not declare focus-zone inquiry support"
//~ "profile `SonyFR7` does not declare picture-effect support"
//~ "profile `SonyBRCH900` does not declare picture-effect support"
//~ "profile `grafton_visca::profiles::PtzOpticsG3` does not declare USB audio support"
