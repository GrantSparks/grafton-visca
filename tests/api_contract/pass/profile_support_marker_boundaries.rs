use grafton_visca::{
    capabilities::{
        HasBrightnessControl, HasFocusZone, HasFocusZoneInquiry, HasPictureEffect, HasUsbAudio,
    },
    profiles::{PtzOptics30X, PtzOpticsG2, PtzOpticsG3},
};

fn requires_brightness<P: HasBrightnessControl>() {}
fn requires_focus_zone<P: HasFocusZone>() {}
fn requires_focus_zone_inquiry<P: HasFocusZoneInquiry>() {}
fn requires_picture_effect<P: HasPictureEffect>() {}
fn requires_usb_audio<P: HasUsbAudio>() {}

fn main() {
    requires_brightness::<PtzOpticsG2>();
    requires_brightness::<PtzOpticsG3>();
    requires_brightness::<PtzOptics30X>();

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
}
