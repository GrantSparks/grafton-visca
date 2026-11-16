//! Runtime permission validation must never manufacture static source claims.
use grafton_visca::{
    capabilities::{HasNdFilter, HasZoom},
    profiles::AnyProfile,
};

fn needs_zoom<P: HasZoom>() {}
fn needs_nd_filter<P: HasNdFilter>() {}

fn main() {
    needs_zoom::<AnyProfile>();
    needs_nd_filter::<AnyProfile>();
}

//~ E0277
//~ "profile `AnyProfile` does not declare Zoom support"
//~ "profile `AnyProfile` does not declare typed ND filter support"
