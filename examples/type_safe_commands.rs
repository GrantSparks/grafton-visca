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
        nd_filter::NdFilterMetadataExt, Exposure, HasExposureCompensation, HasFocusLock,
        HasNdFilter, HasPushAutoFocus, HasVariableSpeed, NdFilterMetadata, PanTilt, Presets,
        ProfileMetadata, WhiteBalance, Zoom,
    },
    zoom_from_normalized, UnitInterval, ZoomDomain,
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
    P: ProfileMetadata
        + PanTilt
        + Zoom
        + Exposure
        + WhiteBalance
        + Presets
        + NdFilterMetadata
        + Default,
{
    let profile = P::default();
    let tcp_port = P::PROFILE_ID
        .and_then(|id| id.default_tcp_port())
        .map_or_else(|| "n/a".to_string(), |port| port.to_string());
    let udp_port = P::PROFILE_ID
        .and_then(|id| id.default_udp_port())
        .map_or_else(|| "n/a".to_string(), |port| port.to_string());

    println!("{}", P::MODEL_NAME);
    println!("  TCP default port: {tcp_port}");
    println!("  UDP default port: {udp_port}");
    println!("  Inquiry support: {:?}", P::INQUIRY_SUPPORT);
    println!(
        "  Pan range: {}..{} VISCA units",
        P::PAN_RANGE.min(),
        P::PAN_RANGE.max()
    );
    println!(
        "  Tilt range: {}..{} VISCA units",
        P::TILT_RANGE.min(),
        P::TILT_RANGE.max()
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
    let sony = SonyFR7;

    println!("Validation");
    match zoom_from_normalized(
        UnitInterval::new(0.5).expect("literal is in range"),
        ZoomDomain::Optical,
        PtzOpticsG2::OPTICAL_ZOOM_MAX,
        PtzOpticsG2::DIGITAL_ZOOM_MAX,
    ) {
        Ok(position) => println!(
            "  PTZOptics G2 50% optical zoom: 0x{:04X}",
            position.value()
        ),
        Err(error) => println!("  PTZOptics G2 zoom validation failed: {error}"),
    }

    match UnitInterval::new(1.25) {
        Ok(value) => println!("  Unexpected zoom validation success: {value:?}"),
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
    requires_nd_filter::<SonyFR7>();
    requires_variable_speed::<SonyFR7>();
    requires_exposure_compensation::<PtzOpticsG2>();
    requires_exposure_compensation::<SonyFR7>();

    // These lines intentionally do not compile if uncommented:
    // requires_nd_filter::<PtzOpticsG2>();
    // requires_variable_speed::<PtzOpticsG2>();
    // requires_push_af::<PtzOpticsG2>();
    // requires_focus_lock::<SonyFR7>();
}

fn requires_focus_lock<P>()
where
    P: ProfileMetadata + HasFocusLock,
{
    println!("  {} supports focus lock", P::MODEL_NAME);
}

fn requires_nd_filter<P>()
where
    P: ProfileMetadata + HasNdFilter,
{
    println!("  {} supports typed ND filter control", P::MODEL_NAME);
}

fn requires_variable_speed<P>()
where
    P: ProfileMetadata + HasVariableSpeed,
{
    println!("  {} supports variable speed mode", P::MODEL_NAME);
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
