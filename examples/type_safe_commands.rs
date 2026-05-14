//! Type-safe profile and capability example.
//!
//! This example does not connect to a camera. It demonstrates the parts of the
//! public API that can be checked before any I/O happens: profile metadata,
//! range validation, protocol envelope selection, and optional capability bounds.
//! It complements the command builder type-state pattern used to guarantee
//! terminator-safe VISCA byte construction around `VISCA_TERMINATOR`.
//!
//! Run with:
//! ```sh
//! cargo run --example type_safe_commands
//! ```

use grafton_visca::{
    camera::profiles::{GenericVisca, PtzOpticsG2, SonyFR7},
    capabilities::{
        nd_filter::NdFilterExt, zoom::ZoomExt, Exposure, HasExposureCompensation, HasFocusLock,
        HasPushAutoFocus, NdFilter, PanTilt, Presets, ProfileMetadata, WhiteBalance, Zoom,
    },
};

fn main() {
    print_profile::<PtzOpticsG2>();
    print_profile::<SonyFR7>();
    print_profile::<GenericVisca>();

    demonstrate_value_validation();
    demonstrate_optional_capability_bounds();
}

fn print_profile<P>()
where
    P: ProfileMetadata + PanTilt + Zoom + Exposure + WhiteBalance + Presets + NdFilter + Default,
{
    let profile = P::default();

    println!("{}", P::MODEL_NAME);
    println!("  TCP default port: {}", P::DEFAULT_TCP_PORT);
    println!("  UDP default port: {}", P::DEFAULT_UDP_PORT);
    println!("  Inquiry support: {:?}", P::INQUIRY_SUPPORT);
    println!(
        "  Pan range: {}..{} VISCA units",
        P::PAN_RANGE.start,
        P::PAN_RANGE.end - 1
    );
    println!(
        "  Tilt range: {}..{} VISCA units",
        P::TILT_RANGE.start,
        P::TILT_RANGE.end - 1
    );
    println!("  Optical zoom max: 0x{:04X}", P::OPTICAL_ZOOM_MAX);
    println!("  Digital zoom max: {:?}", P::DIGITAL_ZOOM_MAX);
    println!("  Exposure modes: {}", P::EXPOSURE_MODES.len());
    println!("  White-balance modes: {}", P::WB_MODES.len());
    println!("  Preset max: {}", P::MAX_PRESETS);
    println!("  ND filter: {}", profile.nd_filter_description());
    println!();
}

fn demonstrate_value_validation() {
    let g2 = PtzOpticsG2;
    let sony = SonyFR7;

    println!("Validation");
    match g2.normalized_to_zoom_units(0.5) {
        Ok(units) => println!("  PTZOptics G2 50% zoom: 0x{units:04X}"),
        Err(error) => println!("  PTZOptics G2 zoom validation failed: {error}"),
    }

    match g2.normalized_to_zoom_units(1.25) {
        Ok(units) => println!("  Unexpected zoom validation success: 0x{units:04X}"),
        Err(error) => println!("  Out-of-range zoom rejected: {error}"),
    }

    println!(
        "  Sony FR7 variable ND accepts midpoint value: {}",
        sony.validate_nd_filter(128).is_ok()
    );
    println!();
}

fn demonstrate_optional_capability_bounds() {
    println!("Optional capability bounds");

    requires_focus_lock::<PtzOpticsG2>();
    requires_push_af::<SonyFR7>();
    requires_exposure_compensation::<PtzOpticsG2>();
    requires_exposure_compensation::<SonyFR7>();

    // These lines intentionally do not compile if uncommented:
    // requires_push_af::<PtzOpticsG2>();
    // requires_focus_lock::<SonyFR7>();
}

fn requires_focus_lock<P>()
where
    P: ProfileMetadata + HasFocusLock,
{
    println!("  {} supports focus lock", P::MODEL_NAME);
}

fn requires_push_af<P>()
where
    P: ProfileMetadata + HasPushAutoFocus,
{
    println!("  {} supports Push AF", P::MODEL_NAME);
}

fn requires_exposure_compensation<P>()
where
    P: ProfileMetadata + HasExposureCompensation,
{
    println!("  {} supports exposure compensation", P::MODEL_NAME);
}
