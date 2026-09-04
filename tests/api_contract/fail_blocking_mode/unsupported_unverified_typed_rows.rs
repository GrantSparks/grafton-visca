#![cfg(feature = "blocking")]

use grafton_visca::{
    blocking::Camera,
    profiles::{PtzOpticsG2, SonyBRCH900, SonyFR7},
};

fn main() {
    let image: Option<Camera<'static, PtzOpticsG2>> = None;
    let image = image.as_ref().unwrap();
    let _ = image.image().freeze_on();
    let _ = image.image().freeze_off();
    let _ = image.image().defog_level();

    let tally: Option<Camera<'static, SonyFR7>> = None;
    let tally = tally.as_ref().unwrap();
    let _ = tally.tally().bright_lo();
    let _ = tally.tally().bright_hi();
    let _ = tally.tally().status();
    let _ = tally.tally().flash();
    let _ = tally.tally().on();
    let _ = tally.tally().off();
    let _ = tally.tally().auto_adjust_enabled();

    let brc_h900: Option<Camera<'static, SonyBRCH900>> = None;
    let brc_h900 = brc_h900.as_ref().unwrap();
    let _ = brc_h900.tally().red_on();
    let _ = brc_h900.tally().red_off();
    let _ = brc_h900.tally().green_on();
    let _ = brc_h900.tally().green_off();
}

//~ E0277
//~ E0599
//~ "does not declare validated image-freeze support"
//~ "does not declare validated defog-level inquiry support"
//~ "does not declare validated tally-brightness support"
//~ "does not declare validated PTZOptics tally-extension support"
//~ "profile `SonyBRCH900` does not declare typed tally-light support"
