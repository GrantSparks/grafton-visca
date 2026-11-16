use grafton_visca::{
    capabilities::HasAutoSlowShutter, capabilities::HasBrightness,
    capabilities::HasExposureCompensation, capabilities::HasExposureMode,
    capabilities::HasFocusZone, capabilities::HasFocusZoneInquiry, capabilities::HasImageFlip,
    capabilities::HasIris, capabilities::HasNoiseReduction2D, capabilities::HasNoiseReduction3D,
    capabilities::HasOnePushWhiteBalance, capabilities::HasPictureEffect, capabilities::HasRgbGain,
    capabilities::HasSpotlight, capabilities::HasUsbAudio, capabilities::HasVersionInquiry,
    profiles::GenericVisca, profiles::NearusBrc300, profiles::PtzOptics30x, profiles::PtzOpticsG2,
    profiles::PtzOpticsG3, profiles::SonyBrc300, profiles::SonyBrcH900, profiles::SonyEviH100,
    profiles::SonyFr7,
};

fn requires_brightness<P: HasBrightness>() {}
fn requires_exposure_mode<P: HasExposureMode>() {}
fn requires_iris<P: HasIris>() {}
fn requires_nr_2d<P: HasNoiseReduction2D>() {}
fn requires_nr_3d<P: HasNoiseReduction3D>() {}
fn requires_focus_zone<P: HasFocusZone>() {}
fn requires_focus_zone_inquiry<P: HasFocusZoneInquiry>() {}
fn requires_picture_effect<P: HasPictureEffect>() {}
fn requires_usb_audio<P: HasUsbAudio>() {}
fn requires_spotlight<P: HasSpotlight>() {}
fn requires_auto_slow_shutter<P: HasAutoSlowShutter>() {}
fn requires_version_inquiry<P: HasVersionInquiry>() {}
fn requires_exposure_compensation<P: HasExposureCompensation>() {}
fn requires_rgb_gain<P: HasRgbGain>() {}
fn requires_image_flip<P: HasImageFlip>() {}
fn requires_one_push_wb<P: HasOnePushWhiteBalance>() {}

fn main() {
    // #828 grant audit: rows documented by R8, R12 and R21.
    for_sony_standard_exposure::<SonyEviH100>();
    for_sony_standard_exposure::<SonyBrc300>();
    for_sony_standard_exposure::<NearusBrc300>();
    requires_image_flip::<SonyBrc300>();
    requires_image_flip::<NearusBrc300>();
    requires_one_push_wb::<SonyBrc300>();
    requires_one_push_wb::<NearusBrc300>();
    requires_auto_slow_shutter::<NearusBrc300>();

    requires_one_push_wb::<SonyFr7>();
    requires_fr7_exposure_and_presets::<SonyFr7>();
    requires_version_inquiry::<SonyFr7>();
    requires_version_inquiry::<SonyBrcH900>();
    requires_version_inquiry::<SonyEviH100>();
    requires_version_inquiry::<SonyBrc300>();
    requires_version_inquiry::<NearusBrc300>();
    requires_version_inquiry::<GenericVisca>();

    requires_brightness::<PtzOpticsG2>();
    requires_brightness::<PtzOpticsG3>();
    requires_brightness::<PtzOptics30x>();

    requires_exposure_mode::<PtzOpticsG2>();
    requires_exposure_mode::<PtzOpticsG3>();
    requires_exposure_mode::<PtzOptics30x>();
    requires_exposure_mode::<SonyBrcH900>();
    requires_exposure_mode::<SonyEviH100>();
    requires_exposure_mode::<SonyBrc300>();
    requires_exposure_mode::<NearusBrc300>();
    requires_exposure_mode::<GenericVisca>();

    requires_iris::<PtzOpticsG2>();
    requires_iris::<PtzOpticsG3>();
    requires_iris::<PtzOptics30x>();
    requires_iris::<SonyBrcH900>();
    requires_iris::<SonyEviH100>();
    requires_iris::<SonyBrc300>();
    requires_iris::<NearusBrc300>();
    requires_iris::<GenericVisca>();

    requires_nr_2d::<PtzOpticsG2>();
    requires_nr_3d::<PtzOpticsG2>();
    requires_nr_2d::<PtzOpticsG3>();
    requires_nr_3d::<PtzOpticsG3>();
    requires_nr_2d::<PtzOptics30x>();
    requires_nr_3d::<PtzOptics30x>();

    requires_focus_zone::<PtzOpticsG2>();
    requires_focus_zone::<PtzOpticsG3>();
    requires_focus_zone::<PtzOptics30x>();

    requires_focus_zone_inquiry::<PtzOpticsG2>();
    requires_focus_zone_inquiry::<PtzOptics30x>();
    requires_usb_audio::<PtzOpticsG2>();
    requires_usb_audio::<PtzOptics30x>();

    requires_picture_effect::<PtzOpticsG2>();
    requires_picture_effect::<PtzOpticsG3>();
    requires_picture_effect::<PtzOptics30x>();

    requires_spotlight::<SonyFr7>();
    requires_auto_slow_shutter::<SonyEviH100>();
    requires_auto_slow_shutter::<SonyBrc300>();
}

fn for_sony_standard_exposure<P>()
where
    P: HasBrightness + HasExposureCompensation + HasRgbGain,
{
    requires_brightness::<P>();
    requires_exposure_compensation::<P>();
    requires_rgb_gain::<P>();
}

fn requires_fr7_exposure_and_presets<P>()
where
    P: grafton_visca::capabilities::HasIrisSteps
        + grafton_visca::capabilities::HasAutoIris
        + grafton_visca::capabilities::HasSeparatePresetSpeed
        + grafton_visca::capabilities::HasPresetRecallSpeed
        + grafton_visca::capabilities::HasPresetRecallSpeedInquiry,
{
}
