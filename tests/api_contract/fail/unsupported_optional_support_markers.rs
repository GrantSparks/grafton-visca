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
    //~^ E0277 "profile `grafton_visca::profiles::SonyFr7` does not declare color-temperature support"
    requires_nd::<PtzOpticsG2>();
    //~^ E0277 "profile `grafton_visca::profiles::PtzOpticsG2` does not declare typed ND filter support"
    requires_nd::<GenericVisca>();
    //~^ E0277 "profile `grafton_visca::profiles::GenericVisca` does not declare typed ND filter support"
    requires_tally::<PtzOpticsG2>();
    //~^ E0277 "profile `grafton_visca::profiles::PtzOpticsG2` does not declare typed tally-light support"
    requires_tally::<GenericVisca>();
    //~^ E0277 "profile `grafton_visca::profiles::GenericVisca` does not declare typed tally-light support"
    requires_variable_speed::<PtzOpticsG2>();
    //~^ E0277 "profile `grafton_visca::profiles::PtzOpticsG2` does not declare typed variable-speed support"
    requires_variable_speed::<GenericVisca>();
    //~^ E0277 "profile `grafton_visca::profiles::GenericVisca` does not declare typed variable-speed support"
    requires_motion_sync::<PtzOpticsG2>();
    //~^ E0277 "profile `grafton_visca::profiles::PtzOpticsG2` does not declare typed Motion Sync support"
    requires_motion_sync::<PtzOpticsG3>();
    //~^ E0277 "profile `grafton_visca::profiles::PtzOpticsG3` does not declare typed Motion Sync support"
    requires_motion_sync::<PtzOptics30x>();
    //~^ E0277 "profile `grafton_visca::profiles::PtzOptics30x` does not declare typed Motion Sync support"
    requires_motion_sync::<GenericVisca>();
    //~^ E0277 "profile `grafton_visca::profiles::GenericVisca` does not declare typed Motion Sync support"
    requires_motion_sync::<SonyFr7>();
    //~^ E0277 "profile `grafton_visca::profiles::SonyFr7` does not declare typed Motion Sync support"
    requires_brightness::<GenericVisca>();
    //~^ E0277 "profile `grafton_visca::profiles::GenericVisca` does not declare exposure brightness support"
    requires_brightness::<SonyFr7>();
    //~^ E0277 "profile `grafton_visca::profiles::SonyFr7` does not declare exposure brightness support"
    requires_brightness::<SonyBrcH900>();
    //~^ E0277 "profile `grafton_visca::profiles::SonyBrcH900` does not declare exposure brightness support"
    requires_exposure_mode::<SonyFr7>();
    //~^ E0277 "profile `grafton_visca::profiles::SonyFr7` does not declare shared exposure-mode support"
    requires_iris::<SonyFr7>();
    //~^ E0277 "profile `grafton_visca::profiles::SonyFr7` does not declare iris control support"
    requires_nr_2d::<SonyFr7>();
    //~^ E0277 "profile `grafton_visca::profiles::SonyFr7` does not declare 2D noise reduction support"
    requires_nr_3d::<SonyFr7>();
    //~^ E0277 "profile `grafton_visca::profiles::SonyFr7` does not declare 3D noise reduction support"
    requires_focus_zone::<SonyFr7>();
    //~^ E0277 "profile `grafton_visca::profiles::SonyFr7` does not declare focus-zone support"
    requires_focus_zone_inquiry::<SonyFr7>();
    //~^ E0277 "profile `grafton_visca::profiles::SonyFr7` does not declare focus-zone inquiry support"
    requires_focus_zone_inquiry::<PtzOpticsG3>();
    //~^ E0277 "profile `grafton_visca::profiles::PtzOpticsG3` does not declare focus-zone inquiry support"
    requires_picture_effect::<SonyFr7>();
    //~^ E0277 "profile `grafton_visca::profiles::SonyFr7` does not declare picture-effect support"
    requires_picture_effect::<SonyBrcH900>();
    //~^ E0277 "profile `grafton_visca::profiles::SonyBrcH900` does not declare picture-effect support"
    requires_usb_audio::<PtzOpticsG3>();
    //~^ E0277 "profile `grafton_visca::profiles::PtzOpticsG3` does not declare USB audio support"
    requires_contrast::<GenericVisca>();
    //~^ E0277 "profile `grafton_visca::profiles::GenericVisca` does not declare contrast control support"
    requires_contrast::<SonyEviH100>();
    //~^ E0277 "profile `grafton_visca::profiles::SonyEviH100` does not declare contrast control support"
    requires_contrast::<SonyBrc300>();
    //~^ E0277 "profile `grafton_visca::profiles::SonyBrc300` does not declare contrast control support"
    requires_sharpness::<GenericVisca>();
    //~^ E0277 "profile `grafton_visca::profiles::GenericVisca` does not declare sharpness control support"
    requires_sharpness_mode::<SonyEviH100>();
    //~^ E0277 "profile `grafton_visca::profiles::SonyEviH100` does not declare sharpness mode support"
    requires_sharpness_mode::<SonyFr7>();
    //~^ E0277 "profile `grafton_visca::profiles::SonyFr7` does not declare sharpness mode support"
    requires_sharpness::<SonyBrc300>();
    //~^ E0277 "profile `grafton_visca::profiles::SonyBrc300` does not declare sharpness control support"
    requires_auto_slow_shutter::<SonyFr7>();
    //~^ E0277 "profile `grafton_visca::profiles::SonyFr7` does not declare Sony auto slow-shutter support"
    requires_auto_slow_shutter::<SonyBrcH900>();
    //~^ E0277 "profile `grafton_visca::profiles::SonyBrcH900` does not declare Sony auto slow-shutter support"
    requires_spotlight::<SonyEviH100>();
    //~^ E0277 "profile `grafton_visca::profiles::SonyEviH100` does not declare Sony spotlight support"
    requires_spotlight::<SonyBrcH900>();
    //~^ E0277 "profile `grafton_visca::profiles::SonyBrcH900` does not declare Sony spotlight support"
    requires_spotlight::<SonyBrc300>();
    //~^ E0277 "profile `grafton_visca::profiles::SonyBrc300` does not declare Sony spotlight support"
    requires_spotlight::<NearusBrc300>();
    //~^ E0277 "profile `grafton_visca::profiles::NearusBrc300` does not declare Sony spotlight support"
    // #828 grant audit: the model sources document none of these.
    requires_color_temperature::<SonyEviH100>();
    //~^ E0277 "profile `grafton_visca::profiles::SonyEviH100` does not declare color-temperature support"
    requires_image_flip::<SonyEviH100>();
    //~^ E0277 "profile `grafton_visca::profiles::SonyEviH100` does not declare image flip support"
    requires_focus_near_limit_inquiry::<SonyBrc300>();
    //~^ E0277 "profile `grafton_visca::profiles::SonyBrc300` does not declare focus near-limit inquiry support"
    requires_focus_near_limit_inquiry::<GenericVisca>();
    //~^ E0277 "profile `grafton_visca::profiles::GenericVisca` does not declare focus near-limit inquiry support"
    requires_saturation::<NearusBrc300>();
    //~^ E0277 "profile `grafton_visca::profiles::NearusBrc300` does not declare saturation control support"
    requires_rgb_tuning::<SonyEviH100>();
    //~^ E0277 "profile `grafton_visca::profiles::SonyEviH100` does not declare RGB tuning support"
    // R8's Wide-D is `04 3D`, not the typed `04 25` command: metadata only.
    requires_wdr::<SonyEviH100>();
    //~^ E0277 "profile `grafton_visca::profiles::SonyEviH100` does not declare wide dynamic range support"
    // R12/R21 document `04 66` flip but no `04 61` mirror.
    requires_mirror::<SonyBrc300>();
    //~^ E0277 "profile `grafton_visca::profiles::SonyBrc300` does not declare image mirror support"
    requires_mirror::<NearusBrc300>();
    //~^ E0277 "profile `grafton_visca::profiles::NearusBrc300` does not declare image mirror support"
    // Generic VISCA keeps only the families all three Sony sources share.
    requires_exposure_compensation::<GenericVisca>();
    //~^ E0277 "profile `grafton_visca::profiles::GenericVisca` does not declare exposure-compensation support"
    requires_rgb_gain::<GenericVisca>();
    //~^ E0277 "profile `grafton_visca::profiles::GenericVisca` does not declare RGB gain support"
    requires_image_flip::<GenericVisca>();
    //~^ E0277 "profile `grafton_visca::profiles::GenericVisca` does not declare image flip support"
    // PTZOptics replies to `09 00 02` are not the decoded Sony layout.
    requires_version_inquiry::<PtzOpticsG2>();
    //~^ E0277 "profile `grafton_visca::profiles::PtzOpticsG2` does not declare a Sony-format version-inquiry reply"
    requires_version_inquiry::<PtzOpticsG3>();
    //~^ E0277 "profile `grafton_visca::profiles::PtzOpticsG3` does not declare a Sony-format version-inquiry reply"
    requires_version_inquiry::<PtzOptics30x>();
    //~^ E0277 "profile `grafton_visca::profiles::PtzOptics30x` does not declare a Sony-format version-inquiry reply"
    // Only the PTZOptics G2 reply layout is sourced (`y0 50 pq FF`).
    requires_color_temperature_inquiry::<PtzOpticsG3>();
    //~^ E0277 "profile `grafton_visca::profiles::PtzOpticsG3` does not declare a sourced color-temperature reply"
    requires_color_temperature_inquiry::<SonyBrcH900>();
    //~^ E0277 "profile `grafton_visca::profiles::SonyBrcH900` does not declare a sourced color-temperature reply"
}

// Keep every profile named below imported. This fixture pins unsatisfied
// capability bounds, so an unresolved profile name would test only a typo.
