use grafton_visca::{
    capabilities::{
        HasBrightnessControl, HasExposureMode, HasFocusZone, HasFocusZoneInquiry, HasIrisControl,
        HasNoiseReduction2D, HasNoiseReduction2DControl, HasNoiseReduction3D,
        HasNoiseReduction3DControl, HasPictureEffect, HasSonyAutoSlowShutter, HasSonySpotlight,
        HasUsbAudio,
    },
    profiles::{
        GenericVisca, NearusBRC300, PtzOptics30X, PtzOpticsG2, PtzOpticsG3, SonyBRC300,
        SonyBRCH900, SonyEVIH100, SonyFR7,
    },
};

fn requires_brightness<P: HasBrightnessControl>() {}
fn requires_exposure_mode<P: HasExposureMode>() {}
fn requires_iris<P: HasIrisControl>() {}
fn requires_nr_2d<P: HasNoiseReduction2D>() {}
fn requires_nr_3d<P: HasNoiseReduction3D>() {}
fn requires_nr_2d_control<P: HasNoiseReduction2DControl>() {}
fn requires_nr_3d_control<P: HasNoiseReduction3DControl>() {}
fn requires_focus_zone<P: HasFocusZone>() {}
fn requires_focus_zone_inquiry<P: HasFocusZoneInquiry>() {}
fn requires_picture_effect<P: HasPictureEffect>() {}
fn requires_usb_audio<P: HasUsbAudio>() {}
fn requires_spotlight<P: HasSonySpotlight>() {}
fn requires_auto_slow_shutter<P: HasSonyAutoSlowShutter>() {}

fn main() {
    requires_brightness::<PtzOpticsG2>();
    requires_brightness::<PtzOpticsG3>();
    requires_brightness::<PtzOptics30X>();

    requires_exposure_mode::<PtzOpticsG2>();
    requires_exposure_mode::<PtzOpticsG3>();
    requires_exposure_mode::<PtzOptics30X>();
    requires_exposure_mode::<SonyBRCH900>();
    requires_exposure_mode::<SonyEVIH100>();
    requires_exposure_mode::<SonyBRC300>();
    requires_exposure_mode::<NearusBRC300>();
    requires_exposure_mode::<GenericVisca>();

    requires_iris::<PtzOpticsG2>();
    requires_iris::<PtzOpticsG3>();
    requires_iris::<PtzOptics30X>();
    requires_iris::<SonyBRCH900>();
    requires_iris::<SonyEVIH100>();
    requires_iris::<SonyBRC300>();
    requires_iris::<NearusBRC300>();
    requires_iris::<GenericVisca>();

    requires_nr_2d::<PtzOpticsG2>();
    requires_nr_3d::<PtzOpticsG2>();
    requires_nr_2d_control::<PtzOpticsG2>();
    requires_nr_3d_control::<PtzOpticsG2>();
    requires_nr_2d::<PtzOpticsG3>();
    requires_nr_3d::<PtzOpticsG3>();
    requires_nr_2d_control::<PtzOpticsG3>();
    requires_nr_3d_control::<PtzOpticsG3>();
    requires_nr_2d::<PtzOptics30X>();
    requires_nr_3d::<PtzOptics30X>();
    requires_nr_2d_control::<PtzOptics30X>();
    requires_nr_3d_control::<PtzOptics30X>();

    requires_focus_zone::<PtzOpticsG2>();
    requires_focus_zone::<PtzOpticsG3>();
    requires_focus_zone::<PtzOptics30X>();

    requires_focus_zone_inquiry::<PtzOpticsG2>();
    requires_focus_zone_inquiry::<PtzOptics30X>();
    requires_usb_audio::<PtzOpticsG2>();
    requires_usb_audio::<PtzOptics30X>();

    requires_picture_effect::<PtzOpticsG2>();
    requires_picture_effect::<PtzOpticsG3>();
    requires_picture_effect::<PtzOptics30X>();

    requires_spotlight::<SonyFR7>();
    requires_spotlight::<SonyBRCH900>();
    requires_auto_slow_shutter::<SonyEVIH100>();
    requires_auto_slow_shutter::<SonyBRC300>();
}
