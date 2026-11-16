use grafton_visca::{
    capabilities::HasAutoSlowShutter, capabilities::HasBrightness,
    capabilities::HasColorTemperature, capabilities::HasColorTemperatureInquiry,
    capabilities::HasContrast, capabilities::HasExposureCompensation,
    capabilities::HasExposureMode, capabilities::HasFocusNearLimit, capabilities::HasFocusZone,
    capabilities::HasFocusZoneInquiry, capabilities::HasImageFlip, capabilities::HasImageMirror,
    capabilities::HasIris, capabilities::HasMotionSync, capabilities::HasNdFilter,
    capabilities::HasNoiseReduction2D, capabilities::HasNoiseReduction3D,
    capabilities::HasPanTiltSpeedMode, capabilities::HasPictureEffect, capabilities::HasRgbGain,
    capabilities::HasRgbTuning, capabilities::HasSaturation, capabilities::HasSharpness,
    capabilities::HasSharpnessMode, capabilities::HasSpotlight, capabilities::HasTally,
    capabilities::HasUsbAudio, capabilities::HasVersionInquiry, capabilities::HasWideDynamicRange,
    profiles::GenericVisca, profiles::NearusBrc300, profiles::PtzOptics30x, profiles::PtzOpticsG2,
    profiles::PtzOpticsG3, profiles::SonyBrc300, profiles::SonyBrcH900, profiles::SonyEviH100,
    profiles::SonyFr7,
};

fn requires_color_temperature<P: HasColorTemperature>() {}
fn requires_color_temperature_inquiry<P: HasColorTemperatureInquiry>() {}
fn requires_nd<P: HasNdFilter>() {}
fn requires_tally<P: HasTally>() {}
fn requires_motion_sync<P: HasMotionSync>() {}
fn requires_variable_speed<P: HasPanTiltSpeedMode>() {}
fn requires_brightness<P: HasBrightness>() {}
fn requires_exposure_mode<P: HasExposureMode>() {}
fn requires_iris<P: HasIris>() {}
fn requires_nr_2d<P: HasNoiseReduction2D>() {}
fn requires_nr_3d<P: HasNoiseReduction3D>() {}
fn requires_contrast<P: HasContrast>() {}
fn requires_sharpness<P: HasSharpness>() {}
fn requires_sharpness_mode<P: HasSharpnessMode>() {}
fn requires_focus_zone<P: HasFocusZone>() {}
fn requires_focus_zone_inquiry<P: HasFocusZoneInquiry>() {}
fn requires_picture_effect<P: HasPictureEffect>() {}
fn requires_usb_audio<P: HasUsbAudio>() {}
fn requires_spotlight<P: HasSpotlight>() {}
fn requires_auto_slow_shutter<P: HasAutoSlowShutter>() {}
fn requires_version_inquiry<P: HasVersionInquiry>() {}
fn requires_focus_near_limit_inquiry<P: HasFocusNearLimit>() {}
fn requires_image_flip<P: HasImageFlip>() {}
fn requires_saturation<P: HasSaturation>() {}
fn requires_wdr<P: HasWideDynamicRange>() {}
fn requires_mirror<P: HasImageMirror>() {}
fn requires_rgb_tuning<P: HasRgbTuning>() {}
fn requires_exposure_compensation<P: HasExposureCompensation>() {}
fn requires_rgb_gain<P: HasRgbGain>() {}

fn main() {
    requires_color_temperature::<SonyFr7>();
    requires_nd::<PtzOpticsG2>();
    requires_nd::<GenericVisca>();
    requires_tally::<PtzOpticsG2>();
    requires_tally::<GenericVisca>();
    requires_variable_speed::<PtzOpticsG2>();
    requires_variable_speed::<GenericVisca>();
    requires_motion_sync::<PtzOpticsG2>();
    requires_motion_sync::<PtzOpticsG3>();
    requires_motion_sync::<PtzOptics30x>();
    requires_motion_sync::<GenericVisca>();
    requires_motion_sync::<SonyFr7>();
    requires_brightness::<GenericVisca>();
    requires_brightness::<SonyFr7>();
    requires_brightness::<SonyBrcH900>();
    requires_exposure_mode::<SonyFr7>();
    requires_iris::<SonyFr7>();
    requires_nr_2d::<SonyFr7>();
    requires_nr_3d::<SonyFr7>();
    requires_focus_zone::<SonyFr7>();
    requires_focus_zone_inquiry::<SonyFr7>();
    requires_focus_zone_inquiry::<PtzOpticsG3>();
    requires_picture_effect::<SonyFr7>();
    requires_picture_effect::<SonyBrcH900>();
    requires_usb_audio::<PtzOpticsG3>();
    requires_contrast::<GenericVisca>();
    requires_contrast::<SonyEviH100>();
    requires_contrast::<SonyBrc300>();
    requires_sharpness::<GenericVisca>();
    requires_sharpness_mode::<SonyEviH100>();
    requires_sharpness_mode::<SonyFr7>();
    requires_sharpness::<SonyBrc300>();
    requires_auto_slow_shutter::<SonyFr7>();
    requires_auto_slow_shutter::<SonyBrcH900>();
    requires_spotlight::<SonyEviH100>();
    requires_spotlight::<SonyBrcH900>();
    requires_spotlight::<SonyBrc300>();
    requires_spotlight::<NearusBrc300>();
    // #828 grant audit: the model sources document none of these.
    requires_color_temperature::<SonyEviH100>();
    requires_image_flip::<SonyEviH100>();
    requires_focus_near_limit_inquiry::<SonyBrc300>();
    requires_focus_near_limit_inquiry::<GenericVisca>();
    requires_saturation::<NearusBrc300>();
    requires_rgb_tuning::<SonyEviH100>();
    // R8's Wide-D is `04 3D`, not the typed `04 25` command: metadata only.
    requires_wdr::<SonyEviH100>();
    // R12/R21 document `04 66` flip but no `04 61` mirror.
    requires_mirror::<SonyBrc300>();
    requires_mirror::<NearusBrc300>();
    // Generic VISCA keeps only the families all three Sony sources share.
    requires_exposure_compensation::<GenericVisca>();
    requires_rgb_gain::<GenericVisca>();
    requires_image_flip::<GenericVisca>();
    // PTZOptics replies to `09 00 02` are not the decoded Sony layout.
    requires_version_inquiry::<PtzOpticsG2>();
    requires_version_inquiry::<PtzOpticsG3>();
    requires_version_inquiry::<PtzOptics30x>();
    // Only the PTZOptics G2 reply layout is sourced (`y0 50 pq FF`).
    requires_color_temperature_inquiry::<PtzOpticsG3>();
    requires_color_temperature_inquiry::<SonyBrcH900>();
}

// Keep every profile named below imported. This fixture pins unsatisfied
// capability bounds, so an unresolved profile name would test only a typo.

//~ E0277
//~ "profile `grafton_visca::profiles::SonyFr7` does not declare color-temperature support"
//~ "profile `grafton_visca::profiles::PtzOpticsG2` does not declare typed ND filter support"
//~ "profile `grafton_visca::profiles::GenericVisca` does not declare typed ND filter support"
//~ "profile `grafton_visca::profiles::PtzOpticsG2` does not declare typed tally-light support"
//~ "profile `grafton_visca::profiles::GenericVisca` does not declare typed tally-light support"
//~ "profile `grafton_visca::profiles::PtzOpticsG2` does not declare typed variable-speed support"
//~ "profile `grafton_visca::profiles::GenericVisca` does not declare typed variable-speed support"
//~ "profile `grafton_visca::profiles::PtzOpticsG2` does not declare typed Motion Sync support"
//~ "profile `grafton_visca::profiles::PtzOpticsG3` does not declare typed Motion Sync support"
//~ "profile `grafton_visca::profiles::PtzOptics30x` does not declare typed Motion Sync support"
//~ "profile `grafton_visca::profiles::GenericVisca` does not declare typed Motion Sync support"
//~ "profile `grafton_visca::profiles::SonyFr7` does not declare typed Motion Sync support"
//~ "profile `grafton_visca::profiles::GenericVisca` does not declare exposure brightness support"
//~ "profile `grafton_visca::profiles::SonyFr7` does not declare exposure brightness support"
//~ "profile `grafton_visca::profiles::SonyBrcH900` does not declare exposure brightness support"
//~ "profile `grafton_visca::profiles::SonyFr7` does not declare shared exposure-mode support"
//~ "profile `grafton_visca::profiles::SonyFr7` does not declare iris control support"
//~ "profile `grafton_visca::profiles::SonyFr7` does not declare 2D noise reduction support"
//~ "profile `grafton_visca::profiles::SonyFr7` does not declare 3D noise reduction support"
//~ "profile `grafton_visca::profiles::SonyFr7` does not declare focus-zone support"
//~ "profile `grafton_visca::profiles::SonyFr7` does not declare focus-zone inquiry support"
//~ "profile `grafton_visca::profiles::PtzOpticsG3` does not declare focus-zone inquiry support"
//~ "profile `grafton_visca::profiles::SonyFr7` does not declare picture-effect support"
//~ "profile `grafton_visca::profiles::SonyBrcH900` does not declare picture-effect support"
//~ "profile `grafton_visca::profiles::PtzOpticsG3` does not declare USB audio support"
//~ "profile `grafton_visca::profiles::GenericVisca` does not declare contrast control support"
//~ "profile `grafton_visca::profiles::SonyEviH100` does not declare contrast control support"
//~ "profile `grafton_visca::profiles::SonyBrc300` does not declare contrast control support"
//~ "profile `grafton_visca::profiles::GenericVisca` does not declare sharpness control support"
//~ "profile `grafton_visca::profiles::SonyEviH100` does not declare sharpness mode support"
//~ "profile `grafton_visca::profiles::SonyFr7` does not declare sharpness mode support"
//~ "profile `grafton_visca::profiles::SonyBrc300` does not declare sharpness control support"
//~ "profile `grafton_visca::profiles::SonyFr7` does not declare Sony auto slow-shutter support"
//~ "profile `grafton_visca::profiles::SonyBrcH900` does not declare Sony auto slow-shutter support"
//~ "profile `grafton_visca::profiles::SonyEviH100` does not declare Sony spotlight support"
//~ "profile `grafton_visca::profiles::SonyBrcH900` does not declare Sony spotlight support"
//~ "profile `grafton_visca::profiles::SonyBrc300` does not declare Sony spotlight support"
//~ "profile `grafton_visca::profiles::NearusBrc300` does not declare Sony spotlight support"
//~ "profile `grafton_visca::profiles::SonyEviH100` does not declare color-temperature support"
//~ "profile `grafton_visca::profiles::SonyEviH100` does not declare image flip support"
//~ "profile `grafton_visca::profiles::SonyBrc300` does not declare focus near-limit inquiry support"
//~ "profile `grafton_visca::profiles::GenericVisca` does not declare focus near-limit inquiry support"
//~ "profile `grafton_visca::profiles::NearusBrc300` does not declare saturation control support"
//~ "profile `grafton_visca::profiles::SonyEviH100` does not declare RGB tuning support"
//~ "profile `grafton_visca::profiles::SonyEviH100` does not declare wide dynamic range support"
//~ "profile `grafton_visca::profiles::SonyBrc300` does not declare image mirror support"
//~ "profile `grafton_visca::profiles::NearusBrc300` does not declare image mirror support"
//~ "profile `grafton_visca::profiles::GenericVisca` does not declare exposure-compensation support"
//~ "profile `grafton_visca::profiles::GenericVisca` does not declare RGB gain support"
//~ "profile `grafton_visca::profiles::GenericVisca` does not declare image flip support"
//~ "profile `grafton_visca::profiles::PtzOpticsG2` does not declare a Sony-format version-inquiry reply"
//~ "profile `grafton_visca::profiles::PtzOpticsG3` does not declare a Sony-format version-inquiry reply"
//~ "profile `grafton_visca::profiles::PtzOptics30x` does not declare a Sony-format version-inquiry reply"
//~ "profile `grafton_visca::profiles::PtzOpticsG3` does not declare a sourced color-temperature reply"
//~ "profile `grafton_visca::profiles::SonyBrcH900` does not declare a sourced color-temperature reply"
